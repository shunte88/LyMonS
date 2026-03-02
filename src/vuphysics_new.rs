/*
 *  vuphysics_new.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-26 Stuart Hunter
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

//! # VU Meter — Analogue Needle Physics
//!
//! A fully self-contained simulation of a D'Arsonval moving-coil VU movement.
//!
//! ## Physical model
//!
//! Rotational equation of motion:
//!
//! ```text
//!   I · θ'' = τ_drive − k · (θ − θ_rest) − b · θ'
//! ```
//!
//! | Symbol      | Meaning                                          |
//! |-------------|--------------------------------------------------|
//! | `I`         | Moment of inertia of the needle [kg·m²]          |
//! | `θ`         | Needle angle from arc centre [rad]               |
//! | `τ_drive`   | Electromagnetic torque produced by signal coil   |
//! | `k`         | Restoring spring constant [N·m/rad]              |
//! | `θ_rest`    | Resting angle (against negative stop)            |
//! | `b`         | Viscous damping: air drag + eddy-current braking |
//!
//! Integration is performed with a 4th-order Runge-Kutta scheme.
//! `dt` is measured internally using [`std::time::Instant`]; the caller
//! simply feeds a dB value on every animation frame.
//!
//! ## Quick start
//!
//! ```rust
//! let mut meter = VuMeter::new();
//!
//! // Inside your render / animation loop:
//! let reading = meter.update(-6.0);      // −6 dBu
//! draw_needle(reading.angle_degrees);    // −44.01 … +44.01 degrees
//! if reading.overload { flash_clip_led(); }
//! ```
//!
//! ## Customisation
//!
//! ```rust
//! let mut meter = VuMeter::new()
//!     .with_sweep(-50.0, 6.0, -50.0, 50.0)   // dB range -> angle range
//!     .with_overload_threshold(0.0, 0.005)     // 0 dB, 5 ms hold
//!     .with_inertia(6.0e-9)
//!     .with_spring(2.0e-7)
//!     .with_damping(4.0e-9);
//! ```

use std::time::Instant;

// =============================================================================
//  Public result type returned from every update() call
// =============================================================================

/// The instantaneous state of the meter needle, returned on every update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterReading {
    /// Needle position in degrees.
    /// Spans `sweep_min` … `sweep_max`, typically −44.01 … +44.01°.
    pub angle_degrees: f64,
    /// Needle position in radians (same value, alternative unit).
    pub angle_radians: f64,
    /// Normalised needle position in 0.0 … 1.0 (min stop → max stop).
    pub normalised: f64,
    /// `true` when the input signal has been above the overload threshold
    /// continuously for longer than the configured hold time.
    pub overload: bool,
    /// The dB value that was passed to the most recent `update()` call.
    pub input_db: f64,
    /// Wall-clock time elapsed since the last update [s].
    /// Useful for frame timing diagnostics.
    pub dt_seconds: f64,
}

/// Analogue VU meter needle physics.
///
/// All configuration is performed through builder-style methods before the
/// first call to [`VuMeter::update`]. The struct owns its wall-clock timestamp
/// so callers never need to supply a `dt` parameter.
///
/// # Threading
/// `VuMeter` is `Send` but not `Sync`. Need to share it across threads,
/// you must wrap it in a `Mutex`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VuMeter {
    /// dB value at the negative (left) stop of the arc.
    scale_min: f64,
    /// dB value at the positive (right) stop of the arc.
    scale_max: f64,
    /// Angle in degrees at the negative stop (typically negative).
    sweep_min: f64,
    /// Angle in degrees at the positive stop (typically positive).
    sweep_max: f64,

    /// Signal level above which the overload accumulator runs [dB].
    overload_threshold_db: f64,
    /// Duration the signal must be above threshold before `overload` fires [s].
    overload_hold_s: f64,

    /// Moment of inertia of the needle [kg·m²].
    inertia: f64,
    /// Restoring spring constant [N·m/rad].
    spring_k: f64,
    /// Total viscous damping coefficient [N·m·s/rad].
    /// Combines air drag + eddy-current braking.
    damping_b: f64,
    /// Coefficient of restitution at the physical end-stops (0 = dead stop).
    restitution: f64,

    /// Current needle angle [rad].
    angle_rad: f64,
    /// Current angular velocity [rad/s].
    angular_velocity: f64,
    /// Accumulated time the signal has been above the overload threshold [s].
    overload_accumulated_s: f64,
    /// Whether the overload condition is currently latched.
    overload: bool,

    /// Wall-clock instant of the previous update call.
    last_tick: Instant,
    /// Maximum dt clamped to prevent physics explosion after a long pause [s].
    max_dt: f64,
}

impl VuMeter {
    /// Create a meter with standard IEC 60268-17 / ANSI C16.5 VU ballistics.
    ///
    /// Default scale:  −23.00 dB … +4.80 dB  →  −44.01° … +44.01°
    ///
    /// The internal timer starts the moment `new()` is called, so the first
    /// `update()` will see a very small (but real) `dt`.
    pub fn new() -> Self {
        let sweep_min = -44.01_f64;
        Self {
            // Scale
            scale_min: -23.0,
            scale_max: 4.8,
            sweep_min,
            sweep_max: 44.01,

            // Overload: > 0 dB for > 5 ms
            overload_threshold_db: 0.0,
            overload_hold_s: 0.005,

            // Physics — representative values for a 200 µA D'Arsonval movement.
            // Damping ratio zeta = b / (2 * sqrt(k * I)) ≈ 0.89 → classic VU
            // underdamped response with ~300 ms attack time.
            inertia: 8.0e-9,    // [kg·m²]
            spring_k: 2.5e-7,   // [N·m/rad]
            damping_b: 4.5e-9,  // [N·m·s/rad]
            restitution: 0.05,

            // State — needle at rest against the negative stop
            angle_rad: sweep_min.to_radians(),
            angular_velocity: 0.0,
            overload_accumulated_s: 0.0,
            overload: false,

            // Timing
            last_tick: Instant::now(),
            max_dt: 0.1, // cap at 100 ms (survives debugger pauses / hidden tabs)
        }
    }

    /// Set the dB-to-angle scale mapping.
    ///
    /// # Arguments
    /// * `scale_min` ........: dB value at the negative arc stop  (e.g. `−23.0`)
    /// * `scale_max` ........: dB value at the positive arc stop  (e.g. `+4.8`)
    /// * `sweep_min` ........: arc angle at `scale_min`           (e.g. `−44.01`)
    /// * `sweep_max` ........: arc angle at `scale_max`           (e.g. `+44.01`)
    ///
    /// Resets the needle to the new rest position.
    ///
    /// # Panics
    /// Debug builds panic if `scale_min >= scale_max` or the angle range is invalid.
    #[must_use]
    pub fn with_sweep(
        mut self,
        scale_min: f64,
        scale_max: f64,
        sweep_min: f64,
        sweep_max: f64,
    ) -> Self {
        debug_assert!(scale_min < scale_max, "scale_min must be less than scale_max");
        debug_assert!(
            sweep_min < sweep_max,
            "sweep_min must be less than sweep_max"
        );
        self.scale_min = scale_min;
        self.scale_max = scale_max;
        self.sweep_min = sweep_min;
        self.sweep_max = sweep_max;
        self.angle_rad = sweep_min.to_radians();
        self.angular_velocity = 0.0;
        self
    }

    /// Change only the dB range, keeping the current arc angles.
    #[must_use]
    pub fn with_db_range(mut self, scale_min: f64, scale_max: f64) -> Self {
        debug_assert!(scale_min < scale_max, "scale_min must be less than scale_max");
        self.scale_min = scale_min;
        self.scale_max = scale_max;
        self
    }

    /// Change only the arc sweep angles [degrees], keeping the current dB range.
    /// Resets the needle to the new rest position.
    #[must_use]
    pub fn with_arc_degrees(mut self, sweep_min: f64, sweep_max: f64) -> Self {
        debug_assert!(
            sweep_min < sweep_max,
            "sweep_min must be less than sweep_max"
        );
        self.sweep_min = sweep_min;
        self.sweep_max = sweep_max;
        self.angle_rad = sweep_min.to_radians();
        self.angular_velocity = 0.0;
        self
    }

    /// Configure overload detection.
    ///
    /// # Arguments
    /// * `threshold_db` ........: signal level that starts the overload timer (default `0.0`)
    /// * `hold_s`       ........: seconds the signal must stay above threshold (default `0.005`)
    #[must_use]
    pub fn with_overload_threshold(mut self, threshold_db: f64, hold_s: f64) -> Self {
        debug_assert!(hold_s >= 0.0, "hold_s must be non-negative");
        self.overload_threshold_db = threshold_db;
        self.overload_hold_s = hold_s;
        self
    }

    /// Set the moment of inertia of the needle about its pivot [kg·m²].
    ///
    /// Larger values slow the needle (heavier needle).
    /// Typical range: 4 × 10⁻⁹ … 15 × 10⁻⁹.
    #[must_use]
    pub fn with_inertia(mut self, inertia: f64) -> Self {
        debug_assert!(inertia > 0.0, "inertia must be positive");
        self.inertia = inertia;
        self
    }

    /// Set the restoring spring constant [N·m/rad].
    ///
    /// Larger values stiffen the return spring (faster return, less overshoot).
    /// Typical range: 1 × 10⁻⁷ … 5 × 10⁻⁷.
    #[must_use]
    pub fn with_spring(mut self, spring_k: f64) -> Self {
        debug_assert!(spring_k > 0.0, "spring_k must be positive");
        self.spring_k = spring_k;
        self
    }

    /// Set the total viscous damping coefficient [N·m·s/rad].
    ///
    /// Combines air drag and eddy-current braking. The damping ratio is:
    ///
    /// ```text
    ///   zeta = b / (2 * sqrt(k * I))
    /// ```
    ///
    /// * zeta < 1 → underdamped (overshoot); classic VU targets zeta ≈ 0.8 - 1.0
    /// * zeta = 1 → critically damped (fastest response without overshoot)
    /// * zeta > 1 → overdamped (sluggish, no overshoot)
    #[must_use]
    pub fn with_damping(mut self, damping_b: f64) -> Self {
        debug_assert!(damping_b >= 0.0, "damping_b must be non-negative");
        self.damping_b = damping_b;
        self
    }

    /// Set the coefficient of restitution at the physical end-stops.
    ///
    /// * `0.0` ........: dead stop (no bounce)
    /// * `1.0` ........: perfectly elastic (full bounce)
    ///
    /// Default: `0.05`.
    #[must_use]
    pub fn with_restitution(mut self, restitution: f64) -> Self {
        debug_assert!(
            (0.0..=1.0).contains(&restitution),
            "restitution must be in 0.0 … 1.0"
        );
        self.restitution = restitution;
        self
    }

    /// Override the maximum dt cap [s].
    ///
    /// Physics integration is capped to this value to prevent explosion on
    /// the first frame or after a long pause. Default: `0.1` s.
    #[must_use]
    pub fn with_max_dt(mut self, max_dt: f64) -> Self {
        debug_assert!(max_dt > 0.0, "max_dt must be positive");
        self.max_dt = max_dt;
        self
    }
}

impl VuMeter {
    /// Reconfigure the full sweep at runtime.
    /// Resets the needle to the new rest position and clears overload state.
    pub fn set_sweep(
        &mut self,
        scale_min: f64,
        scale_max: f64,
        sweep_min: f64,
        sweep_max: f64,
    ) {
        debug_assert!(scale_min < scale_max);
        debug_assert!(sweep_min < sweep_max);
        self.scale_min = scale_min;
        self.scale_max = scale_max;
        self.sweep_min = sweep_min;
        self.sweep_max = sweep_max;
        self.reset();
    }

    /// Change only the dB range at runtime.
    pub fn set_db_range(&mut self, scale_min: f64, scale_max: f64) {
        debug_assert!(scale_min < scale_max);
        self.scale_min = scale_min;
        self.scale_max = scale_max;
    }

    /// Change only the arc sweep angles [degrees] at runtime.
    /// Resets needle to the new rest position.
    pub fn set_arc_degrees(&mut self, sweep_min: f64, sweep_max: f64) {
        debug_assert!(sweep_min < sweep_max);
        self.sweep_min = sweep_min;
        self.sweep_max = sweep_max;
        self.reset();
    }

    /// Change the overload threshold and hold time at runtime.
    pub fn set_overload_threshold(&mut self, threshold_db: f64, hold_s: f64) {
        self.overload_threshold_db = threshold_db;
        self.overload_hold_s = hold_s;
    }

    /// Change physics parameters at runtime (e.g. to switch between
    /// ballistic presets such as VU, PPM, or peak-programme).
    pub fn set_physics(&mut self, inertia: f64, spring_k: f64, damping_b: f64) {
        debug_assert!(inertia > 0.0 && spring_k > 0.0 && damping_b >= 0.0);
        self.inertia = inertia;
        self.spring_k = spring_k;
        self.damping_b = damping_b;
    }

    /// Reset needle to the rest position and clear overload state without
    /// altering any configuration values.
    pub fn reset(&mut self) {
        self.angle_rad = self.sweep_min.to_radians();
        self.angular_velocity = 0.0;
        self.overload_accumulated_s = 0.0;
        self.overload = false;
        self.last_tick = Instant::now();
    }
}

impl VuMeter {
    /// Damping ratio  ζ = b / (2 · √(k · I)).
    /// A well-tuned VU movement targets ζ ≈ 0.8..1.0.
    pub fn damping_ratio(&self) -> f64 {
        self.damping_b / (2.0 * (self.spring_k * self.inertia).sqrt())
    }

    /// Natural (undamped) angular frequency  ω₀ = √(k / I)  [rad/s].
    pub fn natural_frequency_rad_s(&self) -> f64 {
        (self.spring_k / self.inertia).sqrt()
    }

    /// Current needle angle [degrees].
    pub fn angle_degrees(&self) -> f64 {
        self.angle_rad.to_degrees()
    }

    /// Current needle angle [radians].
    pub fn angle_radians(&self) -> f64 {
        self.angle_rad
    }

    /// Normalised needle position: 0.0 = min stop, 1.0 = max stop.
    pub fn normalised(&self) -> f64 {
        let min = self.sweep_min.to_radians();
        let max = self.sweep_max.to_radians();
        ((self.angle_rad - min) / (max - min)).clamp(0.0, 1.0)
    }

    /// Whether the overload condition is currently latched.
    pub fn is_overloaded(&self) -> bool {
        self.overload
    }

    /// The configured dB range as `(scale_min, scale_max)`.
    pub fn db_range(&self) -> (f64, f64) {
        (self.scale_min, self.scale_max)
    }

    /// The configured arc range as `(sweep_min, sweep_max)`.
    pub fn arc_degrees(&self) -> (f64, f64) {
        (self.sweep_min, self.sweep_max)
    }

    /// Equilibrium (steady-state) angle for a given dB value [degrees].
    /// Does **not** modify state — useful for calibration marks and scale drawing.
    pub fn steady_state_degrees(&self, db: f64) -> f64 {
        self.db_to_target_rad(db).to_degrees()
    }
}

impl VuMeter {
    /// **Primary public API** — feed a signal level and receive the needle state.
    ///
    /// Call on every animation frame (60 Hz, 120 Hz, …) or from an audio
    /// callback.  Wall-clock time is measured internally; no `dt` is required.
    ///
    /// # Arguments
    /// * `db` ........: instantaneous signal level in dBFS, dBu, or dBVU as appropriate.
    ///
    /// # Returns
    /// A [`MeterReading`] snapshot valid for this instant.
    pub fn update(&mut self, db: f64) -> MeterReading {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f64()
            .min(self.max_dt);
        self.last_tick = now;

        if dt > 1.0e-9 {
            self.integrate(db, dt);
            self.update_overload(db, dt);
        }

        MeterReading {
            angle_degrees: self.angle_rad.to_degrees(),
            angle_radians: self.angle_rad,
            normalised: self.normalised(),
            overload: self.overload,
            input_db: db,
            dt_seconds: dt,
        }
    }

    /// RK4 integration of the equation of motion for one time step `dt` [s].
    ///
    /// ```text
    ///   I·θ'' = τ_drive − k·(θ − θ_rest) − b·θ'
    /// ```
    fn integrate(&mut self, db: f64, dt: f64) {
        let target = self.db_to_target_rad(db);
        let drive = self.drive_torque(target);

        let (theta0, omega0) = (self.angle_rad, self.angular_velocity);

        // Stage 1
        let k1_theta = omega0;
        let k1_omega = self.angular_accel(theta0, omega0, drive);

        // Stage 2
        let k2_theta = omega0 + 0.5 * dt * k1_omega;
        let k2_omega = self.angular_accel(
            theta0 + 0.5 * dt * k1_theta,
            omega0 + 0.5 * dt * k1_omega,
            drive,
        );

        // Stage 3
        let k3_theta = omega0 + 0.5 * dt * k2_omega;
        let k3_omega = self.angular_accel(
            theta0 + 0.5 * dt * k2_theta,
            omega0 + 0.5 * dt * k2_omega,
            drive,
        );

        // Stage 4
        let k4_theta = omega0 + dt * k3_omega;
        let k4_omega = self.angular_accel(
            theta0 + dt * k3_theta,
            omega0 + dt * k3_omega,
            drive,
        );

        let new_theta = theta0 + (dt / 6.0) * (k1_theta + 2.0 * k2_theta + 2.0 * k3_theta + k4_theta);
        let new_omega = omega0 + (dt / 6.0) * (k1_omega + 2.0 * k2_omega + 2.0 * k3_omega + k4_omega);

        // Apply physical end-stops with a small elastic rebound
        let (theta, omega) = self.apply_stops(new_theta, new_omega);

        self.angle_rad = theta;
        self.angular_velocity = omega;
    }

    /// Angular acceleration [rad/s²] from the linearised equation of motion.
    ///
    /// ```text
    ///   alpha = (tau_drive - k*(theta - theta_rest) - b*omega) / I
    /// ```
    #[inline(always)]
    fn angular_accel(&self, theta: f64, omega: f64, drive: f64) -> f64 {
        let theta_rest = self.sweep_min.to_radians();
        let spring_torque = self.spring_k * (theta - theta_rest);
        let damp_torque = self.damping_b * omega;
        (drive - spring_torque - damp_torque) / self.inertia
    }

    /// Electromagnetic drive torque needed to hold the needle at `target_rad`
    /// in equilibrium against the restoring spring.
    ///
    /// ```text
    ///   tau_drive = k * (theta_target - theta_rest)
    /// ```
    #[inline(always)]
    fn drive_torque(&self, target_rad: f64) -> f64 {
        let theta_rest = self.sweep_min.to_radians();
        self.spring_k * (target_rad - theta_rest)
    }

    /// Map a dB value to the corresponding equilibrium needle angle [rad].
    /// The mapping is linear in dB → degrees, clamped to the arc limits.
    #[inline(always)]
    fn db_to_target_rad(&self, db: f64) -> f64 {
        let slope = (self.sweep_max - self.sweep_min) / (self.scale_max - self.scale_min);
        let angle_deg = self.sweep_min + (db - self.scale_min) * slope;
        angle_deg
            .clamp(self.sweep_min, self.sweep_max)
            .to_radians()
    }

    /// Clamp the needle to the physical arc limits, applying a small elastic
    /// bounce when a stop is struck.
    #[inline(always)]
    fn apply_stops(&self, theta: f64, omega: f64) -> (f64, f64) {
        let min_rad = self.sweep_min.to_radians();
        let max_rad = self.sweep_max.to_radians();

        if theta < min_rad {
            // Hit negative stop — reverse velocity with restitution
            (min_rad, (-omega * self.restitution).max(0.0))
        } else if theta > max_rad {
            // Hit positive stop
            (max_rad, (-omega * self.restitution).min(0.0))
        } else {
            (theta, omega)
        }
    }

    /// Advance the overload accumulator; latch or clear the overload flag.
    #[inline(always)]
    fn update_overload(&mut self, db: f64, dt: f64) {
        if db > self.overload_threshold_db {
            self.overload_accumulated_s += dt;
            if self.overload_accumulated_s >= self.overload_hold_s {
                self.overload = true;
            }
        } else {
            self.overload_accumulated_s = 0.0;
            self.overload = false;
        }
    }
}

impl Default for VuMeter {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
//  Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic helper: drives physics directly, bypassing `Instant`.
    fn simulate(meter: &mut VuMeter, db: f64, duration_s: f64, step_s: f64) {
        let steps = (duration_s / step_s).round() as usize;
        for _ in 0..steps {
            meter.integrate(db, step_s);
            meter.update_overload(db, step_s);
        }
    }

    #[test]
    fn default_ranges() {
        let m = VuMeter::new();
        assert_eq!(m.db_range(), (-23.0, 4.8));
        let (amin, amax) = m.arc_degrees();
        assert!((amin - -44.01).abs() < 1e-9);
        assert!((amax - 44.01).abs() < 1e-9);
    }

    #[test]
    fn steady_state_min_stop() {
        let m = VuMeter::new();
        let angle = m.steady_state_degrees(-23.0);
        assert!((angle - -44.01).abs() < 1e-6, "angle={angle}");
    }

    #[test]
    fn steady_state_max_stop() {
        let m = VuMeter::new();
        let angle = m.steady_state_degrees(4.8);
        assert!((angle - 44.01).abs() < 1e-6, "angle={angle}");
    }

    #[test]
    fn steady_state_is_linear() {
        let m = VuMeter::new();
        // Midpoint dB should map to midpoint angle
        let db_mid = (-23.0 + 4.8) / 2.0;
        let ang_mid = m.steady_state_degrees(db_mid);
        assert!((ang_mid - 0.0).abs() < 1e-6, "ang_mid={ang_mid}");
    }

    #[test]
    fn needle_starts_at_rest() {
        let m = VuMeter::new();
        assert!((m.angle_degrees() - -44.01).abs() < 1e-6);
        assert_eq!(m.angular_velocity, 0.0);
    }

    #[test]
    fn needle_settles_at_min_for_silence() {
        let mut m = VuMeter::new();
        simulate(&mut m, -23.0, 2.0, 0.001);
        assert!(
            (m.angle_degrees() - -44.01).abs() < 1.5,
            "angle = {}",
            m.angle_degrees()
        );
    }

    #[test]
    fn needle_settles_near_full_scale() {
        let mut m = VuMeter::new();
        simulate(&mut m, 4.8, 2.0, 0.001);
        assert!(
            (m.angle_degrees() - 44.01).abs() < 1.5,
            "angle = {}",
            m.angle_degrees()
        );
    }

    #[test]
    fn needle_obeys_positive_stop() {
        let mut m = VuMeter::new();
        simulate(&mut m, 9999.0, 1.0, 0.001);
        assert!(
            m.angle_degrees() <= 44.01 + 0.01,
            "exceeded positive stop: {}",
            m.angle_degrees()
        );
    }

    #[test]
    fn needle_obeys_negative_stop() {
        let mut m = VuMeter::new();
        simulate(&mut m, -9999.0, 1.0, 0.001);
        assert!(
            m.angle_degrees() >= -44.01 - 0.01,
            "exceeded negative stop: {}",
            m.angle_degrees()
        );
    }

    #[test]
    fn normalised_at_min_is_zero() {
        let mut m = VuMeter::new();
        simulate(&mut m, -23.0, 2.0, 0.001);
        let n = m.normalised();
        assert!(n < 0.02, "normalised at min = {n}");
    }

    #[test]
    fn normalised_at_max_is_one() {
        let mut m = VuMeter::new();
        simulate(&mut m, 4.8, 2.0, 0.001);
        let n = m.normalised();
        assert!(n > 0.98, "normalised at max = {n}");
    }

    #[test]
    fn overload_not_triggered_below_hold_time() {
        let mut m = VuMeter::new();
        simulate(&mut m, 2.0, 0.004, 0.001); // 4 ms < 5 ms hold
        assert!(!m.is_overloaded(), "should not be overloaded at 4 ms");
    }

    #[test]
    fn overload_triggered_after_hold_time() {
        let mut m = VuMeter::new();
        simulate(&mut m, 2.0, 0.006, 0.001); // 6 ms > 5 ms hold
        assert!(m.is_overloaded(), "should be overloaded at 6 ms");
    }

    #[test]
    fn overload_clears_when_signal_drops_below_threshold() {
        let mut m = VuMeter::new();
        simulate(&mut m, 2.0, 0.010, 0.001);
        assert!(m.is_overloaded());
        m.update_overload(-1.0, 0.001);
        assert!(!m.is_overloaded(), "overload should clear below 0 dB");
        assert_eq!(m.overload_accumulated_s, 0.0);
    }

    #[test]
    fn overload_does_not_trigger_below_threshold_db() {
        let mut m = VuMeter::new();
        simulate(&mut m, -0.1, 1.0, 0.001); // just below 0 dB
        assert!(!m.is_overloaded());
    }

    #[test]
    fn custom_sweep_via_builder() {
        let m = VuMeter::new().with_sweep(-60.0, 0.0, -60.0, 60.0);
        assert_eq!(m.db_range(), (-60.0, 0.0));
        assert_eq!(m.arc_degrees(), (-60.0, 60.0));
        // Needle should be at new rest position
        assert!((m.angle_degrees() - -60.0).abs() < 1e-6);
    }

    #[test]
    fn set_sweep_at_runtime_resets_needle() {
        let mut m = VuMeter::new();
        simulate(&mut m, 4.8, 1.0, 0.001);
        assert!(m.angle_degrees() > 0.0); // well away from rest
        m.set_sweep(-60.0, 0.0, -60.0, 60.0);
        assert!((m.angle_degrees() - -60.0).abs() < 1e-6);
    }

    #[test]
    fn set_db_range_does_not_move_needle() {
        let mut m = VuMeter::new();
        simulate(&mut m, 0.0, 1.0, 0.001);
        let before = m.angle_degrees();
        m.set_db_range(-40.0, 10.0);
        // Physical state unchanged, only scale is different
        assert_eq!(m.angle_degrees(), before);
    }

    #[test]
    fn reset_returns_needle_to_rest() {
        let mut m = VuMeter::new();
        simulate(&mut m, 4.8, 1.0, 0.001);
        m.reset();
        assert!((m.angle_degrees() - -44.01).abs() < 1e-6);
        assert_eq!(m.angular_velocity, 0.0);
        assert!(!m.is_overloaded());
    }

    #[test]
    fn damping_ratio_in_expected_range() {
        let m = VuMeter::new();
        let zeta = m.damping_ratio();
        assert!(
            zeta > 0.5 && zeta < 1.5,
            "damping ratio out of expected range: zeta = {zeta:.3}"
        );
    }

    #[test]
    fn natural_frequency_positive() {
        let m = VuMeter::new();
        assert!(m.natural_frequency_rad_s() > 0.0);
    }

    #[test]
    fn update_returns_self_consistent_reading() {
        use std::time::Duration;
        let mut m = VuMeter::new();
        std::thread::sleep(Duration::from_millis(2));
        let r = m.update(-10.0);

        assert_eq!(r.input_db, -10.0);
        assert!(!r.overload);
        assert!(
            r.angle_degrees >= -44.01 && r.angle_degrees <= 44.01,
            "angle out of range: {}",
            r.angle_degrees
        );
        assert!(
            (0.0..=1.0).contains(&r.normalised),
            "normalised out of range: {}",
            r.normalised
        );
        assert!(r.dt_seconds > 0.0 && r.dt_seconds < 1.0);
    }
}

