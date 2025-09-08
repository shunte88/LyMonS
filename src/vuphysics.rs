// vu_physics.rs
use std::time::{Duration, Instant};

/// Classic 2nd-order needle: m ẍ + c ẋ + k x + c2 |ẋ| ẋ = g * u
/// x in [0,1] is needle deflection, u in [0,1] is drive (from audio level map).
#[derive(Debug, Clone)]
pub struct Needle {
    // state
    pub x: f32,     // position 0..1
    pub v: f32,     // velocity
    // params
    pub m: f32,     // mass
    pub k: f32,     // spring
    pub c: f32,     // linear damping
    pub c2: f32,    // quadratic "air" damping
    pub g: f32,     // drive gain -> steady-state x = u when g == k
    // asymmetric release (slower fall like real VU): multiply damping when u < x
    pub release_damp_mult: f32,
}

impl Needle {
    /// Calibrated to feel like a classic VU:
    /// ≈300 ms to ~99% on a step up, ~1–1.5 s return near floor after release.
    pub fn vu_classic() -> Self {
        // choose natural freq & damping ratio
        let zeta = 0.9;                    // heavy damping => little overshoot
        let wn   = 2.0 * std::f32::consts::PI * 2.5; // ~2.5 Hz => ~0.3 s step response
        let m = 1.0;
        let k = m * wn * wn;
        let c = 2.0 * zeta * wn * m;
        Self {
            x: 0.0, v: 0.0,
            m, k, c,
            c2: 0.0,               // start without air drag; can set ~0.5–2.0 for more “weight”
            g: k,                  // unity DC gain: x→u at steady-state
            release_damp_mult: 2.5 // stronger damping on the way down => slower fall
        }
    }

    /// Step the physics by `dt` seconds with drive `u` in [0,1].
    /// Semi-implicit Euler is stable for small dt (e.g., 1/60 s).
    pub fn step(&mut self, dt: f32, mut u: f32) -> f32 {
        u = u.clamp(0.0, 1.0);
        // apply extra damping when falling (u below current x)
        let c_eff = if u < self.x { self.c * self.release_damp_mult } else { self.c };
        // quadratic drag sign
        let q = self.v.abs() * self.v;

        // m a = g u - k x - c v - c2 |v| v
        let a = (self.g * u - self.k * self.x - c_eff * self.v - self.c2 * q) / self.m;

        // semi-implicit euler
        self.v += a * dt;
        self.x += self.v * dt;

        // stops
        if self.x < 0.0 { self.x = 0.0; self.v = 0.0; }
        if self.x > 1.0 { self.x = 1.0; self.v = 0.0; }

        self.x
    }
}

/// Utility: map VU dB (e.g. −60..+3) to a 0..1 drive.
/// Gamma < 1.0 makes low levels more visible; tweak to taste.
#[inline]
pub fn db_to_drive(db: f32, floor_db: f32, ceil_db: f32, gamma: f32) -> f32 {
    let norm = ((db - floor_db) / (ceil_db - floor_db)).clamp(0.0, 1.0);
    norm.powf(gamma)
}

/// Convenience wrapper that tracks time between calls.
#[derive(Debug, Clone)]
pub struct VuNeedle {
    pub needle: Needle,
    last_t: Instant,
}

impl VuNeedle {
    pub fn new_vu() -> Self {
        Self { needle: Needle::vu_classic(), last_t: Instant::now() }
    }
    pub fn reset(&mut self) {
        self.needle.x = 0.0;
        self.needle.v = 0.0;
        self.last_t = Instant::now();
    }
    /// Call once per draw with current dB value. Returns x in 0..1.
    pub fn update_db(&mut self, db: f32, floor_db: f32, ceil_db: f32, gamma: f32) -> f32 {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_t).as_secs_f32().clamp(0.0, 0.05); // cap dt
        self.last_t = now;
        let u = db_to_drive(db, floor_db, ceil_db, gamma);
        self.needle.step(dt, u)
    }
}
