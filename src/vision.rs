// vision.rs
// Linux-only reader for Squeezelite visualization shared memory.
//
// Cargo dependencies:
//   memmap2 = "0.9"
//   libc    = "0.2"
//
// Behavior:
// - scans /dev/shm for "squeezelite-*" and maps to it

use libc::{open, pthread_rwlock_t, pthread_rwlock_tryrdlock, pthread_rwlock_unlock, time_t, EBUSY};
use memmap2::{MmapMut, MmapOptions};
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::mem::size_of;
use log::{info, error};
use std::ptr;
use std::sync::atomic::{fence, Ordering};
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use crate::func_timer::FunctionTimer;
use std::thread::sleep;
use rustfft::{FftPlanner, num_complex::Complex, Fft};
use std::sync::Arc;

use crate::shm_path::find_squeezelite_shm_path;

const VIS_BUF_SIZE:usize = 16_384;         // Predefined in Squeezelite.
pub const PEAK_METER_LEVELS_MAX:u8 = 48;   // Number of peak meter intervals
pub const SPECTRUM_POWER_MAP_MAX:u8 = 32;  // Number of spectrum bands
pub const METER_CHANNELS:usize = 2;        // Number of metered channels.
pub const OVERLOAD_PEAKS:u8 = 3;           // Number of consecutive 0dBFS peaks for overload.
pub const X_SCALE_LOG:usize = 20;
pub const MAX_SAMPLE_WINDOW:usize = 1024 * X_SCALE_LOG;
pub const MAX_SUBBANDS:usize = MAX_SAMPLE_WINDOW as usize/ METER_CHANNELS / X_SCALE_LOG;
pub const MIN_SUBBANDS:usize = 16;

const MIN_FFT_INPUT_SAMPLES:usize = 128;
const MAX_FFT_INPUT_SAMPLES: usize = 4096; // cap for perf
const SPECTRUM_MIN_HZ: f32 = 20.0;         // start of spectrum
const EPS: f32 = 1e-12;
const FLOOR_DB: f32 = -72.0;               // meter floor
const CEIL_DB: f32 =   0.0;                // ~0 dBFS
const DECAY_STEPS_PER_FRAME: u8 = 1;       // visual fall rate (levels / frame)
const LOCK_TRY_WINDOW_MS: u32 = 5;         // total budget for try-loop

// Timings
pub const POLL_ENABLED: Duration = Duration::from_millis(16); // ~60 FPS
pub const POLL_IDLE: Duration    = Duration::from_millis(48); // chill when idle

// --- internal RAII read-guard used by timed/try lock paths ---
struct ReadGuard {
    p: *mut pthread_rwlock_t,
}
impl Drop for ReadGuard {
    fn drop(&mut self) {
        unsafe { let _ = pthread_rwlock_unlock(self.p); }
    }
}

/// The shared memory structure.
#[repr(C)]
struct VisT {
    rwlock: pthread_rwlock_t, // initialized with PTHREAD_PROCESS_SHARED by writer
    buf_size: u32,
    buf_index: u32,
    running: u8, // C99 bool (0/1)
    // padding likely here
    rate: u32,
    updated: time_t,          // platform time_t
    buffer: [i16; VIS_BUF_SIZE],
}

#[derive(Clone, Debug)]
pub struct VisFrame {
    pub sample_rate: u32,
    pub timestamp: i64, // normalized for convenience
    pub running: bool,
    /// Interleaved i16 samples, linearized oldest→newest
    pub samples: Vec<i16>,
}

pub struct VisReader {
    _mmap: MmapMut,    // keep mapping alive
    shm_path: PathBuf,
    base: *const VisT, // mapped struct base
    last_seen: time_t,
    last_idx: u32,
    last_progress: Instant, // for stale detection
    // reusable stereo scratch
    samples_l: Vec<i16>,
    samples_r: Vec<i16>,
}

// These are safe because we only do read access and the OS rwlock is process-shared.
unsafe impl Send for VisReader {}
unsafe impl Sync for VisReader {}

pub struct SpectrumEngine {
    sr: u32,
    nfft: usize,
    fft: Arc<dyn Fft<f32> + Send + Sync>,
    window: Vec<f32>,
    buf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    mags: Vec<f32>,                // length nfft/2
    bands: usize,
    band_edges: Vec<(usize, usize)>,
    levels_l: Vec<u8>,
    levels_r: Vec<u8>,
    last_levels_l: Vec<u8>,
    last_levels_r: Vec<u8>,
}

impl SpectrumEngine {
    pub fn new(sr: u32, samples_len: usize, bands: usize) -> Self {
        let nmax = samples_len.clamp(MIN_FFT_INPUT_SAMPLES, MAX_FFT_INPUT_SAMPLES);
        let nfft = 1usize.next_power_of_two()
            .min(nmax).min(MAX_FFT_INPUT_SAMPLES)
            .min(
                if samples_len == 0 { 
                    MIN_FFT_INPUT_SAMPLES 
                } else { 
                    samples_len
                }.next_power_of_two())
            .max( MIN_FFT_INPUT_SAMPLES);

        let mut planner = FftPlanner::<f32>::new();
        let fft: Arc<dyn Fft<f32> + Send + Sync> = planner.plan_fft_forward(nfft);

        // Hann window
        let window = (0..nfft)
            .map(|i| {
                let x = std::f32::consts::TAU * (i as f32) / (nfft as f32);
                0.5 * (1.0 - x.cos())
            })
            .collect::<Vec<_>>();

        let buf = vec![Complex::<f32>::new(0.0, 0.0); nfft];
        let scratch = vec![Complex::<f32>::new(0.0, 0.0); fft.get_inplace_scratch_len()];
        let mags = vec![0.0; nfft / 2];

        let band_edges = Self::build_log_bands(sr, nfft, bands);
        Self { sr, nfft, fft, window, buf, scratch, mags, bands, band_edges ,
            levels_l: vec![0; bands], levels_r: vec![0; bands],
            last_levels_l: vec![0; bands], last_levels_r: vec![0; bands],
}
    }

    pub fn ensure(&mut self, sr: u32, samples_len: usize) {
        if self.sr == sr && self.nfft <= samples_len && self.nfft <= MAX_FFT_INPUT_SAMPLES { return; }
        *self = Self::new(sr, samples_len, self.bands);
    }

    pub fn build_log_bands(sr: u32, nfft: usize, bands: usize) -> Vec<(usize, usize)> {
        let nyq = sr as f32 / 2.0;
        let fmin = SPECTRUM_MIN_HZ.min(nyq - 1.0).max(1.0);
        let fmax = (nyq * 0.98).max(fmin + 1.0);
        let mut edges = Vec::with_capacity(bands + 1);
        for i in 0..=bands {
            let t = i as f32 / (bands as f32);
            // log spacing
            let f = fmin * (fmax / fmin).powf(t);
            let k = ((f * (nfft as f32) / (sr as f32)).floor() as isize)
                .clamp(1, (nfft as isize / 2) - 1) as usize;
            edges.push(k);
        }
        // turn to (start,end) per band, ensure non-empty ranges
        let mut out = Vec::with_capacity(bands);
        for i in 0..bands {
            let mut a = edges[i];
            let mut b = edges[i + 1];
            if b <= a { b = (a + 1).min(nfft / 2); }
            out.push((a, b));
        }
        out
    }

    /// Compute band dB (per-channel) from newest `nfft` samples of `pcm`.
    pub fn compute_db(&mut self, pcm: &[i16]) -> Vec<f32> {
        let n = self.nfft.min(pcm.len());
        let start = pcm.len() - n;

        // time-domain buffer with window
        for i in 0..n {
            let s = pcm[start + i] as f32 / (i16::MAX as f32);
            self.buf[i].re = s * self.window[i];
            self.buf[i].im = 0.0;
        }
        for i in n..self.nfft {
            self.buf[i].re = 0.0;
            self.buf[i].im = 0.0;
        }

        // FFT
        self.fft.process_with_scratch(&mut self.buf, &mut self.scratch);

        // magnitudes (one-sided)
        let half = self.nfft / 2;
        for k in 0..half {
            self.mags[k] = self.buf[k].re.hypot(self.buf[k].im);
        }

        // simple energy sum per band, then 20*log10 (amplitude) or 10*log10 (power)
        // We'll use amplitude dBFS here; switch to power by squaring + 10*log10.
        let mut bands_db = vec![0.0; self.bands];
        for (bi, (a, b)) in self.band_edges.iter().enumerate() {
            let mut acc = 0.0f32;
            for k in *a..*b {
                acc += self.mags[k].max(0.0);
            }
            // Normalize roughly by number of bins to keep levels sane
            let avg = acc / ((*b - *a) as f32).max(1.0);
            let db = 20.0 * (avg.max(EPS)).log10(); // 0 dBFS ≈ full-scale tone (approx; Hann attenuates a bit)
            bands_db[bi] = db;
        }
        bands_db
    }     

    /// Compute dBFS bands for one channel from the newest `nfft` samples.
    pub fn compute_db_bands(&mut self, pcm: &[i16]) -> Vec<f32> {
        let need = self.nfft.min(pcm.len());
        let start = pcm.len().saturating_sub(need);

        // windowed real signal into buf
        for i in 0..need {
            let s = (pcm[start + i] as f32) / (i16::MAX as f32);
            self.buf[i].re = s * self.window[i];
            self.buf[i].im = 0.0;
        }
        for i in need..self.nfft {
            self.buf[i].re = 0.0;
            self.buf[i].im = 0.0;
        }

        // FFT
        self.fft.process_with_scratch(&mut self.buf, &mut self.scratch);

        // magnitude^2 (power), one-sided
        let half = self.nfft / 2;
        for k in 0..half {
            let re = self.buf[k].re;
            let im = self.buf[k].im;
            self.mags[k] = re.mul_add(re, im * im);
        }

        // sum power per band, normalize roughly by bin count, convert to dBFS
        let mut out = vec![FLOOR_DB; self.bands];
        for (bi, (a, b)) in self.band_edges.iter().enumerate() {
            let mut acc = 0.0f32;
            for k in *a..*b { acc += self.mags[k]; }
            let avg = acc / ((*b - *a) as f32).max(1.0);
            // Hann window & FFT scaling shift absolute level; for visuals we just want monotonic
            out[bi] = 10.0 * (avg.max(EPS)).log10(); // power dB
        }
        out
    }

    /// Compute **meter levels** [0..=PEAK_METER_LEVELS_MAX] per band with simple falloff.
    pub fn compute_levels(&mut self, left: &[i16], right: &[i16]) -> (Vec<u8>, Vec<u8>) {
        let db_l = self.compute_db_bands(left);
        let db_r = self.compute_db_bands(right);

        self.levels_l = self.last_levels_l.clone();
        self.levels_r = self.last_levels_r.clone();
        
        // Map to levels
        let mut lv_l = vec![0u8; self.bands];
        let mut lv_r = vec![0u8; self.bands];
        for i in 0..self.bands {
            lv_l[i] = db_to_level(db_l[i]);
            lv_r[i] = db_to_level(db_r[i]);

            // peak-hold with decay (visual smoothing)
            lv_l[i] = lv_l[i].max(self.last_levels_l[i].saturating_sub(DECAY_STEPS_PER_FRAME));
            lv_r[i] = lv_r[i].max(self.last_levels_r[i].saturating_sub(DECAY_STEPS_PER_FRAME));
        }
        self.last_levels_l.copy_from_slice(&lv_l);
        self.last_levels_r.copy_from_slice(&lv_r);
        (lv_l, lv_r)
    }

}

impl VisReader {
    /// Discover the active Squeezelite shm in /dev/shm and map it.
    pub fn new() -> io::Result<Self> {

        // Discover the active squeezelite segment (e.g., "/dev/shm/squeezelite-aa:bb:cc:dd:ee:ff")
        let shm_path = find_squeezelite_shm_path()?;

        // IMPORTANT: open RDWR
        let file = OpenOptions::new()
            .read(true).write(true)
            .open(&shm_path)?;

        // Sanity: ensure the region is at least one VisT
        let len = file.metadata()?.len() as usize;
        if len < size_of::<VisT>() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("squeezelite shm too small: {} < {}", len, size_of::<VisT>()),
            ));
        }

        // Map writable so the rwlock works; we still only *read* audio data
        let mmap = unsafe { MmapOptions::new().len(size_of::<VisT>()).map_mut(&file)? };
        let base = mmap.as_ptr() as *const VisT;

        Ok(Self {
            _mmap: mmap, // NOTE: field type should be memmap2::MmapMut
            shm_path,
            base,
            last_seen: 0,
            last_idx: u32::MAX,
            last_progress: Instant::now(),
            samples_l: Vec::with_capacity(VIS_BUF_SIZE / 2),
            samples_r: Vec::with_capacity(VIS_BUF_SIZE / 2),
        })
    }

    #[inline]
    fn sd(&self) -> &VisT {
        // Safety: base points into a live read-only mapping of VisT.
        unsafe { &*self.base }
    }

    /// Never blocks indefinitely: quick try loop, else fallback to an unlocked,
    /// double-checked snapshot. Returns None if no new, stable data.
    pub fn snapshot_if_new(&mut self) -> io::Result<Option<VisFrame>> {
        //let _ = FunctionTimer::new("vision::snapshot_if_new");
        use libc::pthread_rwlock_t;

        let mut out: Option<VisFrame> = None;
        let mut new_stamp: libc::time_t = 0;
        let mut new_idx: u32 = self.last_idx;

        // ----- inner scope: borrow &self and any lock guard only here -----
        {
            let sd = self.sd();
            let p = &sd.rwlock as *const _ as *mut pthread_rwlock_t;

            match unsafe { lock_read_best_effort(p, LOCK_TRY_WINDOW_MS) }? {
                Some(_guard) => {
                    let updated = sd.updated;
                    let idx_u32 = sd.buf_index;

                    if (updated != 0 && updated != self.last_seen) || (idx_u32 != self.last_idx) {
                        let size = (sd.buf_size as usize).min(VIS_BUF_SIZE);
                        let rate = sd.rate;
                        let running = sd.running != 0;
                        let idx = if size > 0 { (sd.buf_index as usize) % size } else { 0 };

                        if header_looks_good(size, idx, rate) {
                            let mut samples = Vec::with_capacity(size);
                            if size > 0 {
                                samples.extend_from_slice(&sd.buffer[idx..size]);
                                samples.extend_from_slice(&sd.buffer[..idx]);
                            }
                            out = Some(VisFrame {
                                sample_rate: rate,
                                timestamp: updated as i64,
                                running,
                                samples,
                            });
                            new_stamp = updated;
                            new_idx = idx_u32;
                        }
                    }
                    // _guard drops here
                }
                None => {
                    // Unlocked fallback AFTER we drop the borrow below
                }
            }
        } // ----- borrow on &self (sd) ends here -----

        // If we didn't get a frame under lock, try the unlocked double-check path.
        if out.is_none() {
            if let Some(frame) = self.unlocked_snapshot_if_new() {
                new_stamp = frame.timestamp as libc::time_t;
                out = Some(frame);
            }
        }
        // Now it's safe to mutate self.
        if out.is_some() {
            self.last_seen = new_stamp;
            self.last_idx = new_idx;
            self.last_progress = std::time::Instant::now();
        }
        Ok(out)
    }

    fn unlocked_snapshot_if_new_using_index(&self) -> Option<VisFrame> {
        let sd = self.sd();

        let upd1 = unsafe { std::ptr::read_volatile(&sd.updated) };
        let size_raw = unsafe { std::ptr::read_volatile(&sd.buf_size) } as usize;
        let size = size_raw.min(VIS_BUF_SIZE);
        let idx1_u32 = unsafe { std::ptr::read_volatile(&sd.buf_index) };
        let rate = unsafe { std::ptr::read_volatile(&sd.rate) };
        let running = unsafe { std::ptr::read_volatile(&sd.running) } != 0;

        if size == 0 { return None; }

        // consider new if either updated advanced OR index advanced
        let changed = (upd1 != 0 && upd1 != self.last_seen) || (idx1_u32 != self.last_idx);

        if !changed || !header_looks_good(size, idx1_u32 as usize % size, rate) {
            return None;
        }

        std::sync::atomic::fence(Ordering::Acquire);

        let idx = (idx1_u32 as usize) % size;
        let mut samples = Vec::with_capacity(size);
        samples.extend_from_slice(&sd.buffer[idx..size]);
        samples.extend_from_slice(&sd.buffer[..idx]);

        std::sync::atomic::fence(Ordering::Acquire);

        let upd2 = unsafe { std::ptr::read_volatile(&sd.updated) };
        let idx2_u32 = unsafe { std::ptr::read_volatile(&sd.buf_index) };

        // accept only if stable snapshot (both match) to avoid tearing
        if upd2 == upd1 && idx2_u32 == idx1_u32 {
            Some(VisFrame {
                sample_rate: rate,
                timestamp: upd2 as i64,
                running,
                samples,
            })
        } else {
            None
        }
    }

    /// De-interleaves to (left, right) and invokes `f` only when new data arrive.
    /// Returns Ok(true) if called, Ok(false) if nothing new.
    pub fn with_data<F>(&mut self, f: F) -> io::Result<bool>
    where
        F: FnOnce(&[i16], &[i16]),
    {
        match self.snapshot_if_new()? {
            Some(frame) => {
                let n = frame.samples.len() / 2; // ignore odd tail if any
                self.samples_l.resize(n, 0);
                self.samples_r.resize(n, 0);

                for (i, pair) in frame.samples.chunks_exact(2).enumerate() {
                    self.samples_l[i] = pair[0];
                    self.samples_r[i] = pair[1];
                }

                f(&self.samples_l, &self.samples_r);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Variant that also provides the frame header to the closure.
    pub fn with_data_extended<F>(&mut self, f: F) -> io::Result<bool>
    where
        F: FnOnce(&VisFrame, &[i16], &[i16]),
    {
        if let Some(frame) = self.snapshot_if_new()? {
            let n = frame.samples.len() / 2;
            self.samples_l.resize(n, 0);
            self.samples_r.resize(n, 0);
            for (i, p) in frame.samples.chunks_exact(2).enumerate() {
                self.samples_l[i] = p[0];
                self.samples_r[i] = p[1];
            }
            f(&frame, &self.samples_l, &self.samples_r);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Deinterleave with optional channel swap; returns (L, R) slices to your closure.
    pub fn with_data_swapped<F>(&mut self, swap_lr: bool, f: F) -> std::io::Result<bool>
    where
        F: FnOnce(&[i16], &[i16]),
    {
        if let Some(frame) = self.snapshot_if_new()? {
            let n = frame.samples.len() / 2;
            self.samples_l.resize(n, 0);
            self.samples_r.resize(n, 0);
            for (i, pair) in frame.samples.chunks_exact(2).enumerate() {
                if swap_lr {
                    self.samples_l[i] = pair[1];
                    self.samples_r[i] = pair[0];
                } else {
                    self.samples_l[i] = pair[0];
                    self.samples_r[i] = pair[1];
                }
            }
            f(&self.samples_l, &self.samples_r);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Optional: re-scan and remap when the writer has likely restarted and data have gone stale.
    pub fn reopen_if_stale(&mut self) -> io::Result<()> {
        //let _ = FunctionTimer::new("vision::reopen_if_stale");
        if self.last_progress.elapsed() < POLL_IDLE{
            return Ok(());
        }
        let shm_path = self.shm_path.clone();
        let file = OpenOptions::new().read(true).write(true).open(&shm_path)?;
        let mmap = unsafe { MmapOptions::new().len(size_of::<VisT>()).map_mut(&file)? };
        self._mmap = mmap;
        self.base = self._mmap.as_ptr() as *const VisT;
        self.last_progress = Instant::now();
        Ok(())
    }

    // ---------- unlocked (seqlock-style) snapshot fallback ----------
    fn unlocked_snapshot_if_new(&self) -> Option<VisFrame> {
        //let _ = FunctionTimer::new("vision::unlocked_snapshot_if_new");
        let sd = self.sd();

        // read updated once (volatile)
        let first = unsafe { ptr::read_volatile(&sd.updated) };
        if first == 0 || first == self.last_seen {
            return None;
        }

        fence(Ordering::Acquire);

        let raw_size = unsafe { ptr::read_volatile(&sd.buf_size) } as usize;
        let size = raw_size.min(VIS_BUF_SIZE);
        let rate = unsafe { ptr::read_volatile(&sd.rate) };
        let running = unsafe { ptr::read_volatile(&sd.running) } != 0;
        let idx = if size > 0 {
            (unsafe { ptr::read_volatile(&sd.buf_index) } as usize) % size
        } else {
            0
        };

        if !header_looks_good(size, idx, rate) {
            return None;
        }

        let mut samples = Vec::with_capacity(size);
        if size > 0 {
            samples.extend_from_slice(&sd.buffer[idx..size]);
            samples.extend_from_slice(&sd.buffer[..idx]);
        }

        fence(Ordering::Acquire);

        // read updated again — accept only if unchanged
        let second = unsafe { ptr::read_volatile(&sd.updated) };
        if second == first && second != self.last_seen {
            Some(VisFrame {
                sample_rate: rate,
                timestamp: second as i64,
                running,
                samples,
            })
        } else {
            None
        }
    }

}

#[inline]
fn header_looks_good(size: usize, idx: usize, rate: u32) -> bool {
    //let _ = FunctionTimer::new("vision::header_looks_good");
    (1..=VIS_BUF_SIZE).contains(&size)
        && idx < size
        && (8_000..=384_000).contains(&rate)
}

// Try to acquire a read lock without stalling the loop.
// Returns Some(ReadGuard) when locked, None on timeout, or Err on hard error.
unsafe fn lock_read_best_effort(
    p: *mut pthread_rwlock_t,
    window_ms: u32,
) -> io::Result<Option<ReadGuard>> {
    //let _ = FunctionTimer::new("vision::lock_read_best_effort");
    let rc = pthread_rwlock_tryrdlock(p);
    if rc == 0 {
        return Ok(Some(ReadGuard { p }));
    }
    if rc != EBUSY {
        return Err(io::Error::from_raw_os_error(rc));
    }

    let deadline = Instant::now() + Duration::from_millis(window_ms as u64);
    while Instant::now() < deadline {
        let rc2 = pthread_rwlock_tryrdlock(p);
        if rc2 == 0 {
            return Ok(Some(ReadGuard { p }));
        }
        if rc2 != EBUSY {
            return Err(io::Error::from_raw_os_error(rc2));
        }
        // short nap yields CPU and keeps shutdown responsive
        sleep(POLL_IDLE);
    }
    Ok(None) // let caller fall back to unlocked snapshot
}

/// Fast peak/RMS helpers (reuse your slices each call)
pub fn peak_and_rms(ch: &[i16]) -> (i16, f32) {
    let mut peak = 0i32;
    let mut sumsq = 0f64;
    for &s in ch {
        let v = s as i32;
        let a = v.abs();
        if a > peak { peak = a; }
        sumsq += (v as f64) * (v as f64);
    }
    let n = ch.len().max(1) as f64;
    let rms = (sumsq / n).sqrt() as f32;
    (peak.clamp(0, i16::MAX as i32) as i16, rms)
}

/// Map dBFS to integer meter steps [0..=PEAK_METER_LEVELS_MAX]
#[inline]
fn db_to_level(db: f32) -> u8 {
    let x = ((db - FLOOR_DB) / (CEIL_DB - FLOOR_DB)).clamp(0.0, 1.0);
    (x * (PEAK_METER_LEVELS_MAX as f32)).round() as u8
}

pub fn dbfs(x: f32) -> f32 {
    let refv = i16::MAX as f32;
    20.0 * (x.max(1e-9) / refv).log10()
}

fn stereo_channel_peak_rms(samples_l: &[i16],samples_r: &[i16]) -> ((i16, f32), (i16, f32)) {
    // Returns ((peak_l, rms_l), (peak_r, rms_r)), RMS in raw amplitude units (0..=32767)
    let mut peak_l: i32 = 0;
    let mut peak_r: i32 = 0;
    let mut sumsq_l: f64 = 0.0;
    let mut sumsq_r: f64 = 0.0;
    let mut n: usize = 0;

    for idx in 0..samples_l.len() {
        let l = samples_l[idx] as i32;
        let r = samples_r[idx] as i32;
        let la = l.abs();
        let ra = r.abs();
        if la > peak_l { peak_l = la; }
        if ra > peak_r { peak_r = ra; }
        sumsq_l += (l as f64) * (l as f64);
        sumsq_r += (r as f64) * (r as f64);
        n += 1;
    }

    if n == 0 {
        return ((0, 0.0), (0, 0.0));
    }

    let rms_l = (sumsq_l / n as f64).sqrt() as f32;
    let rms_r = (sumsq_r / n as f64).sqrt() as f32;
    ((peak_l.clamp(0, i16::MAX as i32) as i16, rms_l),
     (peak_r.clamp(0, i16::MAX as i32) as i16, rms_r))
    
}

fn stereo_peak_rms(samples: &[i16]) -> ((i16, f32), (i16, f32)) {
    // Returns ((peak_l, rms_l), (peak_r, rms_r)), RMS in raw amplitude units (0..=32767)
    let mut peak_l: i32 = 0;
    let mut peak_r: i32 = 0;
    let mut sumsq_l: f64 = 0.0;
    let mut sumsq_r: f64 = 0.0;
    let mut n: usize = 0;

    for chunk in samples.chunks_exact(2) {
        let l = chunk[0] as i32;
        let r = chunk[1] as i32;
        let la = l.abs();
        let ra = r.abs();
        if la > peak_l { peak_l = la; }
        if ra > peak_r { peak_r = ra; }
        sumsq_l += (l as f64) * (l as f64);
        sumsq_r += (r as f64) * (r as f64);
        n += 1;
    }

    if n == 0 {
        return ((0, 0.0), (0, 0.0));
    }
    let rms_l = (sumsq_l / n as f64).sqrt() as f32;
    let rms_r = (sumsq_r / n as f64).sqrt() as f32;
    ((peak_l.clamp(0, i16::MAX as i32) as i16, rms_l),
     (peak_r.clamp(0, i16::MAX as i32) as i16, rms_r))
    
}

pub fn test_data_stream() -> std::io::Result<()> {

    let breaker = Instant::now() + Duration::from_secs(30);
    let mut reader = VisReader::new().unwrap();
    info!("== Peak ===|== dBFS ===|== RMS ==|== dBFS ===|");
    loop {
        match reader.with_data_extended(|frame, l, r| {
            if frame.running {
                let ((peak_l, rms_l), (peak_r, rms_r)) = stereo_channel_peak_rms(&l, &r);
                info!(
                    "{:>5}|{:>5}|{:>5.1}|{:>5.1}|{:>4.0}|{:>4.0}|{:>5.1}|{:>5.1}|",
                    peak_l, peak_r,
                    dbfs(peak_l as f32), dbfs(peak_r as f32),
                    rms_l, rms_r,
                    dbfs(rms_l), dbfs(rms_r),
                );
            }else{sleep(POLL_ENABLED);}
        }){
            Ok(true) => {
                // processed a fresh frame
            }
            Ok(false) => {
                // no new data; small nap and try to recover if stale
                sleep(POLL_ENABLED);
                if let Err(e) = reader.reopen_if_stale() {
                    error!("reopen_if_stale error: {e}");
                }
            }
            Err(_) => {}
        }
        if breaker < Instant::now() {
            info!("testing completed.");
            return Ok(());
        }
    }

}

pub fn test_data_stream_with_histogram() -> std::io::Result<()> {

    let mut eng: Option<SpectrumEngine> = None;
    let breaker = Instant::now() + Duration::from_secs(30);
    let mut reader = VisReader::new().unwrap();

    info!("== Peak ===|== dBFS ===|== RMS ==|== dBFS ===| Histogram (FIRST 8)  |");
    loop {
        match reader.with_data_extended(|frame, l, r| {
            if frame.running {
                let bands = SPECTRUM_POWER_MAP_MAX as usize;
                if eng.is_none() {
                    eng = Some(SpectrumEngine::new(frame.sample_rate, l.len(), bands));
                } else {
                    eng.as_mut().unwrap().ensure(frame.sample_rate, l.len());
                }
                let e = eng.as_mut().unwrap();
                //Peaks / RMS
                let ((pk_l, rms_l), (pk_r, rms_r)) = stereo_channel_peak_rms(l,r);

            // Histograms (32 bands, log spaced)
            let hist_l = e.compute_db(l);
            let hist_r = e.compute_db(r);

            // quick publish: show a few bands; swap to your bus/IPC as needed
            let show = 8.min(hist_l.len());
            let mut line = String::new();
            for i in 0..show {
                line.push_str(&format!("{:5.1}/{:5.1} ", hist_l[i], hist_r[i]));
            }

            info!(
                "{:>5}|{:>5}|{:>5.1}|{:>5.1}|{:>4.0}|{:>4.0}|{:>5.1}|{:>5.1}| H[0..{}]: {}",
                pk_l, pk_r, dbfs(pk_l as f32), dbfs(pk_r as f32),
                rms_l, rms_r, dbfs(rms_l), dbfs(rms_r),
                show-1, line
            );

        }
        }) {
            Ok(true) => {
                // processed a fresh frame
            }
            Ok(false) => {
                // no new data; small nap and try to recover if stale
                std::thread::sleep(POLL_ENABLED);
                if let Err(e) = reader.reopen_if_stale() {
                    error!("reopen_if_stale error: {e}");
                }
            }
            Err(_) => {}
        }
        if breaker < Instant::now() {
            info!("testing completed.");
            return Ok(());
        }
           
    }
}
