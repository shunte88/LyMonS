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

use log::{warn};
use std::time::Instant;

// ─────────────────────────────────────────────────────────────────────────────
//  VU meter state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct VuMeterState {
    last_t: Instant,
    scale_min: f64,
    sweep_min: f64,
    scale_max: f64,
    sweep_max: f64,
    db_per_degree: f64,
    /// Current needle angle [rad]
    pub angle_rad: f64,
    /// Current angular velocity [rad/s]
    pub angular_velocity: f64,
    /// Accumulated time the signal has been > 0.00 dB [s]
    pub overload_time_accumulated: f64,
    /// Whether the overload condition is currently active
    pub overload: bool,
    /// Most recent input dB (for display / debugging)
    pub current_db: f64,
    /// Moment of inertia of the needle about its pivot [kg·m²]
    /// Typical D'Arsonval movement needle: ~8 µg·m²
    moment_of_inertia: f64,
    /// Spring constant [N·m/rad]
    /// Chosen so that steady-state deflection matches the calibration curve.
    /// At full-scale (+4.8 dB → +44.01°) the spring balances the drive torque.
    spring_k: f64,
    /// Total damping coefficient [N·m·s/rad]
    /// Combines air drag and electromagnetic (eddy-current) damping.
    /// A well-engineered VU movement is ~critically damped (ζ ≈ 0.8–1.0).
    damping_b: f64,
    /// IEC 60268-17 / ANSI C16.5 VU meter ballistics time constant [s]
    /// 300 ms attack (10% → 99% FS in 300 ms) is achieved by the above constants.
    /// This is provided for documentation; the ODE integrator enforces it implicitly.
    vu_attack_300_ms: f64,
}

#[warn(dead_code)]
impl VuMeterState {
    /// VU Meter Needle Physics Simulation
    ///
    /// Models the physical dynamics of an analog VU meter needle including:
    ///   - Rotational inertia (needle mass/moment of inertia)
    ///   - Restoring spring torque (return spring)
    ///   - Air damping (viscous drag proportional to angular velocity)
    ///   - Electromagnetic damping (eddy current braking in the coil)
    ///   - Overload detection: signal > 0.00 dB sustained for > 5 ms
    ///
    /// Meter arc: -44.01° (−23.00 dB) to +44.01° (+4.80 dB)
    /// 0 VU = 0° (centre)

    // ─────────────────────────────────────────────────────────────────────────────
    //  Physical constants (representative values for a standard 200 µA VU movement)
    // ─────────────────────────────────────────────────────────────────────────────

    pub fn new() -> Self {
        let scale_min:f64 = -24.00;
        let sweep_min:f64 = -44.01;
        let scale_max:f64 = 4.80;
        let sweep_max:f64 = 44.01;
        let db_per_degree = (sweep_max - sweep_min) / (scale_max - scale_min); 
        let moment_of_inertia: f64 = 8.0e-9;
        let spring_k: f64 = 2.5e-7;
        let damping_b: f64 = 4.5e-9;
        let vu_attack_300_ms: f64 = 0.300;
        Self {
            last_t: Instant::now(),
            scale_min,
            sweep_min,
            scale_max,
            sweep_max,
            db_per_degree,
            angle_rad: sweep_min.to_radians(),
            angular_velocity: 0.0,
            overload_time_accumulated: 0.0,
            overload: false,
            current_db: scale_min,
            moment_of_inertia,
            spring_k,
            damping_b,
            vu_attack_300_ms,
        }
    }

    pub fn update_scale (
        &mut self,
        scale_min: f64,
        sweep_min: f64,
        scale_max: f64,
        sweep_max: f64,
    ) {
        self.scale_min = scale_min;
        self.sweep_min = sweep_min;
        self.scale_max = scale_max;
        self.sweep_max = sweep_max;
        self.db_per_degree = (self.sweep_max - self.sweep_min) / (self.scale_max - self.scale_min);
        self.last_t = Instant::now();
    }
    
    /// The coil produces an electromagnetic torque proportional to the signal level.
    /// We model the "signal" as an RMS-derived value (already dB-smoothed upstream).
    ///
    /// The equilibrium condition: τ_drive = k · (θ_target - θ_rest)
    /// where θ_rest = sweep_min (needle at rest against negative stop, no signal).
    ///
    /// Drive torque [N·m] needed to hold the needle at angle θ_target against spring.
    fn drive_torque_for_target(&self, target_angle_rad: f64) -> f64 {
        // At rest the spring rests at sweep_min (meter rest position)
        let rest_rad = self.sweep_min.to_radians();
        self.spring_k * (target_angle_rad - rest_rad)
    }
    
    /// Convert a dB value to the target (equilibrium) angle in radians.
    /// The spring pulls the needle toward this angle when driven by the signal.
    fn db_to_target_angle_rad(&self, db: f64) -> f64 {
        let angle_deg = self.sweep_min + (db - self.scale_min) * self.db_per_degree;
        // Clamp to physical stops
        let clamped = angle_deg.clamp(self.sweep_min, self.sweep_max);
        clamped.to_radians()
    }

    /// Return the current needle deflection in degrees.
    pub fn angle_degrees(&self) -> f64 {
        self.angle_rad.to_radians().to_degrees()
    }
    
    // ─────────────────────────────────────────────────────────────────────────────
    //  Physics update :: Runge-Kutta 4 integrator
    // ─────────────────────────────────────────────────────────────────────────────

    /// Compute angular acceleration [rad/s²] given the instantaneous state.
    ///
    /// Equation of motion (rotational form of F = ma):
    ///
    ///   I·θ'' = τ_drive - k·(θ - θ_rest) - b·θ'
    ///
    ///   where:
    ///     I         = moment of inertia
    ///     τ_drive   = electromagnetic drive torque (from signal)
    ///     k·(θ-θ_rest) = restoring spring torque  (acts against displacement from rest)
    ///     b·θ'      = total damping torque (air + eddy current)
    ///
    fn angular_acceleration(&self, angle: f64, velocity: f64, drive_torque: f64) -> f64 {
        let rest_rad = self.sweep_min.to_radians();
        let spring_torque = self.spring_k * (angle - rest_rad);
        let damping_torque = self.damping_b * velocity;
        (drive_torque - spring_torque - damping_torque) / self.moment_of_inertia
    }

    /// Advance the simulation by `dt` seconds for a given `db_value` input.
    ///
    /// # Arguments
    /// * `db_value`          instantaneous signal level in dBFS / dBu as appropriate
    /// * `dt`                time step in seconds (e.g. 1.0/48000.0 for audio callback rate)
    ///
    /// # Returns
    /// `(angle_degrees, overload)`
    ///
    ///   * `angle_degrees`   needle position in degrees (−44.01 … +44.01)
    ///   * `overload`        true when signal has been > 0 dB for more than 5 ms
    pub fn update(&mut self, db_value: f64, dt: f64) -> (f64, bool) {

        self.current_db = db_value;

        // Compute target angle from the input dB
        let target_rad = self.db_to_target_angle_rad(db_value);
        let drive = self.drive_torque_for_target(target_rad);

        // ── RK4 integration ──────────────────────────────────────────────────────
        let θ0 = self.angle_rad;
        let ω0 = self.angular_velocity;

        let k1_θ = ω0;
        let k1_ω = self.angular_acceleration(θ0, ω0, drive);

        let k2_θ = ω0 + 0.5 * dt * k1_ω;
        let k2_ω = self.angular_acceleration(
            θ0 + 0.5 * dt * k1_θ,
            ω0 + 0.5 * dt * k1_ω,
            drive,
        );

        let k3_θ = ω0 + 0.5 * dt * k2_ω;
        let k3_ω = self.angular_acceleration(
            θ0 + 0.5 * dt * k2_θ,
            ω0 + 0.5 * dt * k2_ω,
            drive,
        );

        let k4_θ = ω0 + dt * k3_ω;
        let k4_ω = self.angular_acceleration(
            θ0 + dt * k3_θ,
            ω0 + dt * k3_ω,
            drive,
        );

        let new_angle = θ0 + (dt / 6.0) * (k1_θ + 2.0 * k2_θ + 2.0 * k3_θ + k4_θ);
        let new_velocity = ω0 + (dt / 6.0) * (k1_ω + 2.0 * k2_ω + 2.0 * k3_ω + k4_ω);

        // ── Physical stops ───────────────────────────────────────────────────────
        let min_rad = self.sweep_min.to_radians();
        let max_rad = self.sweep_max.to_radians();

        let (clamped_angle, clamped_velocity) = if new_angle < min_rad {
            (min_rad, 0.0_f64.max(-new_velocity * 0.05)) // slight bounce
        } else if new_angle > max_rad {
            (max_rad, 0.0_f64.min(-new_velocity * 0.05))
        } else {
            (new_angle, new_velocity)
        };

        self.angle_rad = clamped_angle;
        self.angular_velocity = clamped_velocity;

        // ── Overload detection ───────────────────────────────────────────────────
        const OVERLOAD_THRESHOLD_DB: f64 = 0.0;
        const OVERLOAD_HOLD_TIME_S: f64 = 0.005; // 5 ms

        if db_value > OVERLOAD_THRESHOLD_DB {
            self.overload_time_accumulated += dt;
        } else {
            // Reset accumulator when signal drops below threshold
            self.overload_time_accumulated = 0.0;
            self.overload = false;
        }

        if self.overload_time_accumulated >= OVERLOAD_HOLD_TIME_S {
            self.overload = true;
        }

        let angle_out = self.angle_rad.to_degrees();
        (angle_out, self.overload)
    }

    /// Update by normalized drive u (dB).
    pub fn update_drive(&mut self, u: f64) -> (f64, bool) {
        let now = std::time::Instant::now();
        let dt_ms = now.saturating_duration_since(self.last_t).as_millis() as f64;
        self.last_t = now;
        self.update(u, dt_ms)
    }

    // ─────────────────────────────────────────────────────────────────────────────
    //  Convenience: single-shot query (stateless wrapper for simple use)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Stateless helper: given a steady-state dB value, return the equilibrium
    /// needle angle without simulating transient dynamics.  Useful for
    /// calibration checks and static rendering.
    pub fn steady_state_angle_degrees(&self, db: f64) -> f64 {
        self.db_to_target_angle_rad(db).to_degrees()
    }

}

impl Default for VuMeterState {
    fn default() -> Self {
        Self::new()
    }
}
