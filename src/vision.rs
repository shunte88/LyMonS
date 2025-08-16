//! Rust translation of vision.c using the libc crate and rustfft.

use libc::{
    c_char, c_int, c_void, close, ioctl, mmap, munmap, pthread_rwlock_rdlock, pthread_rwlock_t,
    pthread_rwlock_unlock, shm_open, socket, time, time_t, AF_INET, MAP_FAILED, MAP_SHARED, O_RDWR,
    PROT_READ, PROT_WRITE, SOCK_DGRAM, SIOCGIFCONF, SIOCGIFHWADDR,
};
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::ffi::CString;
use std::mem;
use std::ptr;
use std::slice;
use std::sync::{Arc, Mutex};

pub const VIS_BUF_SIZE = 16384;        // Predefined in Squeezelite.
pub const PEAK_METER_LEVELS_MAX = 48;  // Number of peak meter intervals
pub const SPECTRUM_POWER_MAP_MAX = 32; // Number of spectrum bands
pub const METER_CHANNELS = 2;          // Number of metered channels.
pub const OVERLOAD_PEAKS = 3; // Number of consecutive 0dBFS peaks for overload.
pub const X_SCALE_LOG = 20;
pub const MAX_SAMPLE_WINDOW = 1024 * X_SCALE_LOG;
pub const MAX_SUBBANDS = MAX_SAMPLE_WINDOW / 2 / X_SCALE_LOG;
pub const MIN_SUBBANDS = 16;
pub const MIN_FFT_INPUT_SAMPLES = 128;

enum VIZMODES {
    VEMODE_NA = -1,
    VEMODE_RN = 0,
    VEMODE_VU = 1,
    VEMODE_PK = 2,
    VEMODE_SA = 3,
    VEMODE_ST = 4,
    VEMODE_SM = 5,
    VEMODE_MX = 6,
    VEMODE_A1 = 7,
    VEMODE_A1S = 8,
    VEMODE_A1V = 9,
}                  // note all-in-one modes out of "bounds"

struct peak_meter_t {
    int_time: u16,  // Integration time (ms).
    samples: u16,   // Samples for integration time.
    hold_time: u16, // Peak hold time (ms).
    hold_incs: u16, // Hold time counter.
    fall_time: u16, // Fall time (ms).
    fall_incs: u16, // Fall time counter.
    over_peaks: u16, // Number of consecutive 0dBFS samples for overload.
    over_time: u16, // Overload indicator time (ms).
    over_incs: u16, // Overload indicator count.
    num_levels: u8, // Number of display levels
    floor: i8,      // Noise floor for meter (dB).
    reference: u16, // Reference level.
    overload: Vec(METER_CHANNELS, bool),    // Overload flags.
    dBfs: Vec(METER_CHANNELS, i8),          // dBfs values.
    bar_index: Vec(METER_CHANNELS, u8),     // Index for bar display.
    dot_index: Vec(METER_CHANNELS, u8),     // Index for dot display (peak hold).
    elapsed: Vec(METER_CHANNELS, u32),      // Elapsed time (us).
    scale: Vec(PEAK_METER_LEVELS_MAX, u16), // Scale intervals.
}

struct bin_chan_t {
    bin: Vec(METER_CHANNELS, Vec(MAX_SUBBANDS, float64)),
}

struct {
    metric: Vec(METER_CHANNELS, i32),
    percent: Vec(METER_CHANNELS, float64),
    erase: Vec(METER_CHANNELS, bool),
}

/*
struct vissy_meter_t {
    vis_type_t meter_type;
    char channel_name[METER_CHANNELS][2];
    int is_mono;
    int64_t sample_accum[METER_CHANNELS]; // VU raw peak values.
    int8_t floor;                         // Noise floor for meter (dB).
    uint16_t reference;                   // Reference level.
    int64_t dBfs[METER_CHANNELS];         // dBfs values.
    int64_t dB[METER_CHANNELS];           // dB values.
    int64_t linear[METER_CHANNELS];       // linear dB (min->max)
    uint8_t rms_bar[METER_CHANNELS];
    uint8_t rms_levels;
    char rms_charbar[METER_CHANNELS][PEAK_METER_LEVELS_MAX + 1];
    int16_t rms_scale[PEAK_METER_LEVELS_MAX];
    int32_t power_map[SPECTRUM_POWER_MAP_MAX];
    int channel_width[METER_CHANNELS];
    int bar_size[METER_CHANNELS];
    int subbands_in_bar[METER_CHANNELS];
    int num_bars[METER_CHANNELS];
    int channel_flipped[METER_CHANNELS];
    int clip_subbands[METER_CHANNELS];
    int num_subbands;
    int sample_window;
    int num_windows;
    double filter_window[MAX_SAMPLE_WINDOW];
    double preemphasis[MAX_SUBBANDS];
    int decade_idx[MAX_SUBBANDS];
    int decade_len[MAX_SUBBANDS];
    int numFFT[METER_CHANNELS];
    int sample_bin_chan[METER_CHANNELS][MAX_SUBBANDS];
    float avg_power[2 * MAX_SUBBANDS];
    kiss_fft_cfg cfg;
};

*/

mod visdata {
    use super::Fft;
    use std::sync::Arc;

    pub const VIS_BUF_SIZE: usize = 8192;
    pub const METER_CHANNELS: usize = 2;
    pub const MAX_SUBBANDS: usize = 64;
    pub const MAX_SAMPLE_WINDOW: usize = 4096;
    pub const MIN_SUBBANDS: usize = 16;
    pub const X_SCALE_LOG: usize = 4;
    pub const MIN_FFT_INPUT_SAMPLES: usize = 256;

    #[repr(C)]
    pub struct vissy_meter_t {
        pub channel_width: [u32; METER_CHANNELS],
        pub bar_size: [u32; METER_CHANNELS],
        pub num_subbands: i32,
        pub clip_subbands: [bool; METER_CHANNELS],
        pub num_bars: [i32; METER_CHANNELS],
        pub subbands_in_bar: [i32; METER_CHANNELS],
        pub is_mono: bool,
        pub sample_window: i32,
        pub num_windows: i32,
        // --- FFT related fields ---
        #[repr(C)]
        pub fft_plan: Option<Arc<dyn Fft<f32>>>,
        pub filter_window: [f32; MAX_SAMPLE_WINDOW],
        // --- End FFT ---
        pub decade_idx: [i32; MAX_SUBBANDS],
        pub decade_len: [i32; MAX_SUBBANDS],
        pub preemphasis: [f64; MAX_SUBBANDS],
        pub reference: f64,
        pub floor: f64,
        pub sample_accum: [u64; METER_CHANNELS],
        pub sample_bin_chan: [[i32; MAX_SUBBANDS]; METER_CHANNELS],
        pub avg_power: [f32; MAX_SUBBANDS * 2],
        pub dB: [f64; METER_CHANNELS],
        pub dBfs: [f64; METER_CHANNELS],
        pub linear: [i32; METER_CHANNELS],
        pub rms_scale: [i32; METER_CHANNELS],
        pub power_map: [i32; 32],
    }
}

use visdata::*;

const VUMETER_DEFAULT_SAMPLE_WINDOW: usize = 1024 * 2;

#[repr(C)]
struct vis_t {
    rwlock: pthread_rwlock_t,
    buf_size: u32,
    buf_index: u32,
    running: bool,
    rate: u32,
    updated: time_t,
    buffer: [i16; VIS_BUF_SIZE],
}

static mut VIS_MMAP: *mut vis_t = ptr::null_mut();
static mut VIS_FD: c_int = -1;
static MAC_ADDRESS: Mutex<Option<String>> = Mutex::new(None);

fn get_mac_address_shmem() -> Option<String> {
    unsafe {
        let sd = socket(AF_INET, SOCK_DGRAM, 0);
        if sd < 0 {
            return None;
        }

        let mut mac = [0u8; 6];
        let mut ifc: libc::ifconf = mem::zeroed();
        let mut ifs: [libc::ifreq; 3] = mem::zeroed();

        ifc.ifc_len = mem::size_of_val(&ifs) as i32;
        ifc.ifc_buf = ifs.as_mut_ptr() as *mut c_char;

        if ioctl(sd, SIOCGIFCONF, &mut ifc) == 0 {
            let ifend = (ifs.as_ptr() as *const u8).add(ifc.ifc_len as usize) as *const libc::ifreq;
            let mut ifr = ifc.ifc_req;

            while ifr < ifend {
                if (*ifr).ifr_ifru.ifru_addr.sa_family as i32 == AF_INET {
                    let mut ifreq: libc::ifreq = mem::zeroed();
                    ptr::copy_nonoverlapping(
                        (*ifr).ifr_name.as_ptr(),
                        ifreq.ifr_name.as_mut_ptr(),
                        libc::IFNAMSIZ,
                    );

                    if ioctl(sd, SIOCGIFHWADDR, &mut ifreq) == 0 {
                        mac.copy_from_slice(slice::from_raw_parts(
                            ifreq.ifr_ifru.ifru_hwaddr.sa_data.as_ptr() as *const u8,
                            6,
                        ));
                        if mac.iter().sum::<u8>() != 0 {
                            break;
                        }
                    }
                }
                ifr = (ifr as *const u8).add(mem::size_of::<libc::ifreq>()) as *const libc::ifreq;
            }
        }

        close(sd);

        Some(format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        ))
    }
}

fn vissy_reopen() {
    unsafe {
        if !VIS_MMAP.is_null() {
            munmap(VIS_MMAP as *mut c_void, mem::size_of::<vis_t>());
            VIS_MMAP = ptr::null_mut();
        }

        if VIS_FD != -1 {
            close(VIS_FD);
            VIS_FD = -1;
        }

        let mut mac_lock = MAC_ADDRESS.lock().unwrap();
        if mac_lock.is_none() {
            *mac_lock = get_mac_address_shmem();
        }

        if let Some(mac) = mac_lock.as_deref() {
            let shm_path_str = format!("/squeezelite-{}", mac);
            let shm_path = CString::new(shm_path_str).unwrap();

            VIS_FD = shm_open(shm_path.as_ptr(), O_RDWR, 0o666);
            if VIS_FD > 0 {
                VIS_MMAP = mmap(
                    ptr::null_mut(),
                    mem::size_of::<vis_t>(),
                    PROT_READ | PROT_WRITE,
                    MAP_SHARED,
                    VIS_FD,
                    0,
                ) as *mut vis_t;

                if VIS_MMAP == MAP_FAILED as *mut vis_t {
                    close(VIS_FD);
                    VIS_FD = -1;
                    VIS_MMAP = ptr::null_mut();
                }
            }
        }
    }
}

pub fn vissy_close() {
    unsafe {
        if VIS_FD != -1 {
            close(VIS_FD);
            VIS_FD = -1;
            VIS_MMAP = ptr::null_mut();
        }
    }
}

pub fn vissy_check() {
    static mut LAST_OPEN: time_t = 0;
    unsafe {
        let now = time(ptr::null_mut());

        if VIS_MMAP.is_null() {
            if now - LAST_OPEN > 5 {
                vissy_reopen();
                LAST_OPEN = now;
            }
            if VIS_MMAP.is_null() {
                return;
            }
        }

        pthread_rwlock_rdlock(&mut (*VIS_MMAP).rwlock);
        let running = (*VIS_MMAP).running;
        let updated = (*VIS_MMAP).updated;
        pthread_rwlock_unlock(&mut (*VIS_MMAP).rwlock);

        if running && (now - updated > 5) {
            vissy_reopen();
            LAST_OPEN = now;
        }
    }
}

fn vissy_lock() {
    unsafe {
        if !VIS_MMAP.is_null() {
            pthread_rwlock_rdlock(&mut (*VIS_MMAP).rwlock);
        }
    }
}

fn vissy_unlock() {
    unsafe {
        if !VIS_MMAP.is_null() {
            pthread_rwlock_unlock(&mut (*VIS_MMAP).rwlock);
        }
    }
}

fn vissy_is_playing() -> bool {
    unsafe {
        if VIS_MMAP.is_null() {
            false
        } else {
            (*VIS_MMAP).running
        }
    }
}

pub fn vissy_get_rate() -> u32 {
    unsafe {
        if VIS_MMAP.is_null() {
            0
        } else {
            (*VIS_MMAP).rate
        }
    }
}

fn vissy_get_buffer() -> *const i16 {
    unsafe {
        if VIS_MMAP.is_null() {
            ptr::null()
        } else {
            (*VIS_MMAP).buffer.as_ptr()
        }
    }
}

fn vissy_get_buffer_len() -> u32 {
    unsafe {
        if VIS_MMAP.is_null() {
            0
        } else {
            (*VIS_MMAP).buf_size
        }
    }
}

fn vissy_get_buffer_idx() -> u32 {
    unsafe {
        if VIS_MMAP.is_null() {
            0
        } else {
            (*VIS_MMAP).buf_index
        }
    }
}

pub fn vissy_meter_init(vissy_meter: &mut vissy_meter_t) {
    // ... (logic for calculating num_subbands, num_bars etc. remains the same)

    // Setup rustfft plan
    let mut planner = FftPlanner::<f32>::new();
    vissy_meter.fft_plan = Some(
        planner.plan_fft_forward(vissy_meter.sample_window as usize),
    );

    // --- The rest of the initialization logic from the C version ---
    let const1 = 0.54;
    let const2 = 0.46;
    for w in 0..vissy_meter.sample_window as usize {
        let twopi = std::f64::consts::PI * 2.0;
        vissy_meter.filter_window[w] = (const1
            - (const2 * (twopi * w as f64 / vissy_meter.sample_window as f64).cos()))
            as f32;
    }
    // ... (decade and preemphasis calculations would go here)
}

pub fn vissy_meter_calc(vissy_meter: &mut vissy_meter_t, samode: bool) -> bool {
    vissy_check();

    for channel in 0..METER_CHANNELS {
        vissy_meter.sample_accum[channel] = 0;
        // ... reset other fields
    }

    let num_samples = VUMETER_DEFAULT_SAMPLE_WINDOW;
    let mut buffer: Vec<Complex<f32>> = vec![Complex::default(); num_samples];
    let mut ret = false;

    vissy_lock();
    if vissy_is_playing() {
        ret = true;
        unsafe {
            let mut offs =
                vissy_get_buffer_idx() as i32 - (num_samples * 2) as i32;
            while offs < 0 {
                offs += vissy_get_buffer_len() as i32;
            }

            let mut ptr = vissy_get_buffer().offset(offs as isize);
            let mut samples_until_wrap = vissy_get_buffer_len() as isize - offs as isize;

            for i in 0..num_samples {
                let sample_l = (*ptr.offset(0) >> 7) as f32;
                let sample_r = (*ptr.offset(1) >> 7) as f32;

                // Combine stereo into a complex signal for one FFT
                buffer[i] = Complex::new(
                    sample_l * vissy_meter.filter_window[i],
                    sample_r * vissy_meter.filter_window[i],
                );

                ptr = ptr.offset(2);
                samples_until_wrap -= 2;
                if samples_until_wrap <= 0 {
                    ptr = vissy_get_buffer();
                    samples_until_wrap = vissy_get_buffer_len() as isize;
                }
            }
        }
    }
    vissy_unlock();

    if ret {
        if samode {
            if let Some(plan) = &vissy_meter.fft_plan {
                plan.process(&mut buffer);

                // --- Process FFT output ---
                let mut avg_ptr = 0;
                for s in 0..vissy_meter.num_subbands as usize {
                    let mut kr_sum = 0.0;
                    let mut ki_sum = 0.0;

                    for x in vissy_meter.decade_idx[s] as usize
                        ..vissy_meter.decade_idx[s] as usize + vissy_meter.decade_len[s] as usize
                    {
                        let ck = buffer[x];
                        let cnk = buffer[vissy_meter.sample_window as usize - x];

                        // Reconstruct left channel from complex FFT result
                        let l_r = (ck.re + cnk.re) / 2.0;
                        let l_i = (ck.im - cnk.im) / 2.0;
                        kr_sum += l_r * l_r + l_i * l_i;

                        // Reconstruct right channel
                        let r_r = (ck.im + cnk.im) / 2.0;
                        let r_i = (cnk.re - ck.re) / 2.0;
                        ki_sum += r_r * r_r + r_i * r_i;
                    }

                    vissy_meter.avg_power[avg_ptr] = kr_sum / vissy_meter.decade_len[s] as f32;
                    vissy_meter.avg_power[avg_ptr + 1] = ki_sum / vissy_meter.decade_len[s] as f32;
                    
                    avg_ptr += 2;
                }
                 // ... (Further processing like preemphasis and mapping to bars)
            }
        }
        // ... (Calculate dB, dBfs, etc.)
    }

    ret
}
