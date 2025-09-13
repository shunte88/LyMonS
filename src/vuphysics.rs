/*
 *  vuphysics.rs
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

// vu_physics.rs
use std::time::{Duration, Instant};
use embedded_graphics::prelude::*;

// classic visual range for scale
pub const VU_FLOOR_DB: f32 = -20.0;
pub const VU_CEIL_DB:  f32 = 5.0;
pub const VU_GAMMA:    f32 = 0.7; // lift lows a bit; 1.0 = linear


/// Classic 2nd-order needle: m ẍ + c ẋ + k x + c2 |ẋ| ẋ = g * u
/// x in [0,1] is needle deflection, u in [0,1] is drive (from audio level map).
#[derive(Debug, Clone, PartialEq)]
pub struct NeedleNew {
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

/// Convenience wrapper that tracks time between calls.
#[derive(Debug, Clone, PartialEq)]
pub struct VuNeedleNew {
    pub needle: NeedleNew,
    last_t: Instant,
}

impl NeedleNew {
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

/// Utility: map VU dB (e.g. −20..+5) to a 0..1 drive.
/// Gamma < 1.0 makes low levels more visible; tweakable
#[inline]
pub fn db_to_drive(db: f32, floor_db: f32, ceil_db: f32, gamma: f32) -> f32 {
    let norm = ((db - floor_db) / (ceil_db - floor_db)).clamp(0.0, 1.0);
    norm.powf(gamma)
}

impl VuNeedleNew {
    pub fn new_vu() -> Self {
        Self { needle: NeedleNew::vu_classic(), last_t: Instant::now() }
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

#[derive(Debug, Clone, PartialEq)]
pub struct NeedleMetrics {
    pub mass: f32,      // needle/spring mass (kg-ish units, relative)
    pub k: f32,         // spring constant
    pub damp: f32,      // linear damping
    pub dispx: f32,     // displacement in [0,1]
    pub velocity: f32,  // velocity
    pub over: u32,      // overdrive counter (>=5 => LED on, like your C demo)
}

impl Default for NeedleMetrics {
    fn default() -> Self {
        Self {
            mass: 0.005,
            k: 1.0,
            damp: 0.08,
            dispx: 0.0,
            velocity: 0.0,
            over: 0,
        }
    }
}

/// Exact port of my original C `getForce()` from LMSMonitor.
#[inline]
pub fn get_force(metric: f32) -> f32 {
    // metric must be positive; if zero/neg, force→0
    if metric <= 0.0 {
        return 0.0;
    }

    let six_dba: f32 = 10.0_f32.powf(0.3); // ≈ 1.99526… (6 dB per “A” step)

    // (log(metric)/log(SIX_DBA) + 7)/7 => ~0..1-ish then shaped by atan
    let mut force = (metric.ln() / six_dba.ln() + 7.0) / 7.0;
    if force < 0.0 {
        force = 0.0;
    }
    // "(atanf(force * 2 - 1) * M_1_PI * 4 + 1) / 2"  -> 0..1 S-curve
    ((force * 2.0 - 1.0).atan() * std::f32::consts::FRAC_1_PI * 4.0 + 1.0) / 2.0
}

/// Integrate the needle for `millis` milliseconds with drive `force` in [0,1].
/// Port of `needlePhysics()` from LMSMonitor loop (Euler at 1 ms; clamps + velocity flip).
pub fn step_ms(sm: &mut NeedleMetrics, force: f32, x_min: f32, x_max: f32, millis: u32) -> f32 {
    let dt = 0.001_f32; // 1 ms
    for _t in 0..millis {
        if sm.dispx < x_min {
            sm.dispx = x_min;
            if sm.velocity < 0.0 { sm.velocity = -sm.velocity; }
        }
        if sm.dispx > x_max {
            sm.dispx = x_max;
            if sm.velocity > 0.0 { sm.velocity = -sm.velocity; }
        }
        // F = m a  => a = (force - k x - c v)/m
        let a = (force - sm.k * sm.dispx - sm.damp * sm.velocity) / sm.mass;
        sm.velocity += a * dt;
        sm.dispx   += sm.velocity * dt;
    }

    // Overload pump (same threshold as C)
    if sm.dispx > 0.7 { sm.over += 1; } else { sm.over = 0; }

    sm.dispx
}

/// Time-based wrapper
/// Give it your raw `metric` (e.g. `sample_accum`) and it does:
///   force = get_force(metric / fudge)
///   steps Euler at 1ms over the real elapsed time since last call
#[derive(Debug, Clone, PartialEq)]
pub struct VuNeedle {
    pub n: NeedleMetrics,
    last_t: Instant,
    pub fudge: f32,   // C used 9000.0
    pub x_min: f32,   // 0.0
    pub x_max: f32,   // 1.0
}

impl VuNeedle {
    pub fn new() -> Self {
        Self {
            n: NeedleMetrics::default(),
            last_t: Instant::now(),
            fudge: 9000.0,
            x_min: 0.0,
            x_max: 1.0,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.n.dispx = 0.0;
        self.n.velocity = 0.0;
        self.n.over = 0;
        self.last_t = Instant::now();
    }

    /// Update by normalized drive u in [0,1] (skip get_force/fudge).
    pub fn update_drive(&mut self, u: f32) -> (f32, bool) {

        let now = std::time::Instant::now();
        let mut dt_ms = now.saturating_duration_since(self.last_t).as_millis() as u32;
        self.last_t = now;
        if dt_ms > 50 { dt_ms = 50; }
        let x = step_ms(&mut self.n, u, self.x_min, self.x_max, dt_ms);
        (x, self.n.over > 4)
    }

    /// Update from raw metric (i32/float). Returns (deflection in 0..1, over_flag).
    pub fn update_metric_ignore(&mut self, metric: f32) -> (f32, bool) {
        let now = Instant::now();
        let mut dt_ms = now.saturating_duration_since(self.last_t).as_millis() as u32;
        self.last_t = now;

        // clamp to avoid huge catch-up if we hiccup
        if dt_ms > 50 { dt_ms = 50; } // integrate at most 50 ms

        // C divides by ~9000 to get a sane range, then getForce().
        let force = get_force((metric / self.fudge).max(0.0));

        let x = step_ms(&mut self.n, force, self.x_min, self.x_max, dt_ms);
        (x, self.n.over > 4) // “LED on” after ~5 ms above 0.7, like C
    }
}

/// Geometry helper: convert deflection (0..1) into a tip point for a 90° sweep
/// centered on 45° (same math as your `drawNeedle()`).
#[inline]
pub fn needle_tip(deflection_0_1: f32, x_offset: i32, width: i32, pivot_y: i32) -> (Point, Point)
{
    let angle = deflection_0_1 * std::f32::consts::FRAC_PI_2 - std::f32::consts::FRAC_PI_4;
    let x1 = width / 2 + (width * 8 / 16) as i32 * angle.sin() as i32; // tip
    let y1 = width * 9 / 16 - (width * 8 / 16) as i32 * angle.cos() as i32;
    let x2 = width / 2 + (width * 3 / 16) as i32 * angle.tan() as i32; // base
    let y2 = width * 3 / 8;
    (
        Point::new(x_offset + x1, y1),                            // tip
        Point::new(x_offset + x2, y2 + pivot_y - (width * 3 / 8)) // base
    )
}
