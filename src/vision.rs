/*
 *  vision.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-25 Stuart Hunter
 *
 *	TODO:
 *
 *	This program is free software: you can redistribute it and/or modify
 *	it under the terms of the GNU General Public License as published by
 *	the Free Software Foundation, either version 3 of the License, or
 *	(at your option) any later version.
 *
 *	This program is distributed in the hope that it will be useful,
 *	but WITHOUT ANY WARRANTY; without even the implied warranty of
 *	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *	GNU General Public License for more details.
 *
 *	See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *	Public License.
 *
 */

/// scans /dev/shm for "squeezelite-*" and maps to it

use libc::{pthread_rwlock_t, pthread_rwlock_tryrdlock, pthread_rwlock_unlock, time_t, EBUSY};
use memmap2::{MmapMut, MmapOptions};
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{fence, Ordering};
use std::time::{Duration, Instant};
use std::thread::sleep;
use crate::vuphysics::{VuNeedle};

use crate::shm_path::find_squeezelite_shm_path;

const VIS_BUF_SIZE:usize = 16_384;              // Predefined in Squeezelite.
pub const PEAK_METER_LEVELS_MAX:u8 = 48;        // Number of peak meter intervals
pub const PEAK_METER_SCALE_HEADROOM:f32 = 0.8;  // 20% headroom clipping
pub const METER_CHANNELS:usize = 2;             // Number of metered channels.
pub const OVERLOAD_PEAKS:u8 = 3;                // Number of consecutive 0dBFS peaks for overload.

pub const LEVEL_FLOOR_DB: f32 = -72.0;          // meter floor
pub const LEVEL_CEIL_DB: f32 =   0.0;           // ~0 dBFS
pub const LEVEL_DECAY_STEPS_PER_FRAME: u8 = 1;  // visual fall rate (levels / frame)
const LOCK_TRY_WINDOW_MS: u32 = 5;              // total budget for try-loop


// Timings
pub const POLL_ENABLED: Duration = Duration::from_millis(16); // ~60 FPS
pub const POLL_IDLE: Duration    = Duration::from_millis(48); // chill when idle

/// simple state carried across calls (last metrics + peak-hold)
#[derive(Debug, PartialEq, Clone)]
pub struct LastVizState {

    pub wide: bool,
    pub init_vu: bool,

    pub buffer: Vec<u8>,

    pub last_disp_m: f32,
    pub last_disp_l: f32,
    pub last_disp_r: f32,

    pub last_over_m: bool,
    pub last_over_l: bool,
    pub last_over_r: bool,

    pub last_peak_m: u8,
    pub last_peak_l: u8,
    pub last_peak_r: u8,

    pub last_hold_m: u8,
    pub last_hold_l: u8,
    pub last_hold_r: u8,

    pub last_db_m: f32,
    pub last_db_l: f32,
    pub last_db_r: f32,

    // latest inputs (debug/inspection)
    pub last_bands_m: Vec<u8>,
    pub last_bands_l: Vec<u8>,
    pub last_bands_r: Vec<u8>,

    // bars we actually draw (with our own decay)
    pub draw_bands_m: Vec<u8>,
    pub draw_bands_l: Vec<u8>,
    pub draw_bands_r: Vec<u8>,

    // --- peak caps ---
    pub cap_m: Vec<u8>,
    pub cap_l: Vec<u8>,
    pub cap_r: Vec<u8>,
    pub cap_hold_until_m: Vec<Instant>,  // until this time, hold current cap
    pub cap_hold_until_l: Vec<Instant>,
    pub cap_hold_until_r: Vec<Instant>,
    pub cap_last_update_m: Vec<Instant>, // last time we updated decay
    pub cap_last_update_l: Vec<Instant>,
    pub cap_last_update_r: Vec<Instant>,

    pub init: bool,
    pub vu_init: bool,

    pub vu_m: VuNeedle,
    pub vu_l: VuNeedle,
    pub vu_r: VuNeedle,

    pub last_tick: Instant,
}

impl Default for LastVizState {

    fn default() -> Self {

        Self {
            wide: false,
            init_vu: false,
            buffer: Vec::new(),

            last_peak_m: u8::MIN,
            last_peak_l: u8::MIN,
            last_peak_r: u8::MIN,

            last_disp_m: f32::MIN,
            last_disp_l: f32::MIN,
            last_disp_r: f32::MIN,

            last_over_m: false,
            last_over_l: false,
            last_over_r: false,
            last_hold_m: u8::MIN,
            last_hold_l: u8::MIN,
            last_hold_r: u8::MIN,
            last_db_m: f32::MIN,
            last_db_l: f32::MIN,
            last_db_r: f32::MIN,
            last_bands_m: Vec::new(),
            last_bands_l: Vec::new(),
            last_bands_r: Vec::new(),
            draw_bands_m: Vec::new(),
            draw_bands_l: Vec::new(),
            draw_bands_r: Vec::new(),
            cap_m: Vec::new(),
            cap_l: Vec::new(),
            cap_r: Vec::new(),
            cap_hold_until_m: Vec::new(),
            cap_hold_until_l: Vec::new(),
            cap_hold_until_r: Vec::new(),
            cap_last_update_m: Vec::new(),
            cap_last_update_l: Vec::new(),
            cap_last_update_r: Vec::new(),
            init: true,
            vu_init: true,
 
            vu_m: VuNeedle::new(),
            vu_l: VuNeedle::new(),
            vu_r: VuNeedle::new(),
            last_tick: Instant::now(),
        }
    }
}

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
) -> io::Result<Option<ReadGuard>> { unsafe {
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
}}

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
    let x = ((db - LEVEL_FLOOR_DB) / (LEVEL_CEIL_DB - LEVEL_FLOOR_DB)).clamp(0.0, 1.0);
    (x * PEAK_METER_LEVELS_MAX as f32).round() as u8
}

/// Map dBFS to integer meter steps [0..=PEAK_METER_LEVELS_MAX] and scaled headroom (20%)
#[inline]
fn db_to_level_scale(db: f32) -> u8 {
    let x = ((db - LEVEL_FLOOR_DB) / (LEVEL_CEIL_DB - LEVEL_FLOOR_DB)).clamp(0.0, 1.0);
    (PEAK_METER_SCALE_HEADROOM * x * PEAK_METER_LEVELS_MAX as f32).round() as u8
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

