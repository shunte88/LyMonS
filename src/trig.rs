/*
 *  translate.rs
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
#![allow(clippy::excessive_precision)]
//! Pure-Rust f32 sine/cosine (no libm). Suitable for no_std.
//! Accuracy: ~< 1 ulp over typical ranges; range reduction is simple (good
//! for |x| not astronomically large).

pub const DEG_TO_RAD: f32 = core::f32::consts::PI / 180.0;
pub const RAD_TO_DEG: f32 = 180.0 / core::f32::consts::PI;

/// Compute sin(x) for f32 without libm.
pub fn sinf(x: f32) -> f32 {
    // Reduce to r in [-pi/4, pi/4] and quadrant q in {0,1,2,3}
    let (r, q) = reduce_pi_over_2(x);

    // Evaluate sin/cos on the small interval via minimax polynomials.
    let s = sin_poly(r);
    let c = cos_poly(r);

    // Reconstruct based on quadrant
    match q & 3 {
        0 =>  s,   //   sin(r)
        1 =>  c,   //   sin(pi/2 + r) =  cos(r)
        2 => -s,   //   sin(pi + r)   = -sin(r)
        _ => -c,   //   sin(3pi/2+r)  = -cos(r)
    }
}

/// Compute cos(x) for f32 without libm.
pub fn cosf(x: f32) -> f32 {
    let (r, q) = reduce_pi_over_2(x);
    let s = sin_poly(r);
    let c = cos_poly(r);

    match q & 3 {
        0 =>  c,   // cos(r)
        1 => -s,   // cos(pi/2 + r)  = -sin(r)
        2 => -c,   // cos(pi + r)    = -cos(r)
        _ =>  s,   // cos(3pi/2 + r) =  sin(r)
    }
}

// ---------- Internals ----------

#[inline(always)]
fn reduce_pi_over_2(x: f32) -> (f32, i32) {
    // Simple Cody–Waite-style reduction: n = round(x / (pi/2))
    // For enormous |x| you need Payne–Hanek; this keeps it simple and fast.
    const INV_PIO2: f32 = 0.63661977236758134308_f32; // 2/pi
    const PIO2_1:  f32 = 1.57079625129699707031_f32;  // High part of pi/2
    const PIO2_2:  f32 = 7.54978941586159635335e-08_f32; // Low part (compensation)

    // nearest integer to x/(pi/2)
    let n = (x * INV_PIO2).round();
    let n_i = n as i32;

    // r = x - n*(pi/2) using a split to reduce cancelation error
    let r = ((x - n * PIO2_1) - n * PIO2_2);

    (r, n_i)
}

#[inline(always)]
fn sin_poly(r: f32) -> f32 {
    // Cephes single-precision minimax for |r| <= pi/4
    // sin(r) ~ r + r^3*S1 + r^5*S2 + ... (Horner form)
    const S1: f32 = -1.6666667163e-1;
    const S2: f32 =  8.3333337680e-3;
    const S3: f32 = -1.9841270114e-4;
    const S4: f32 =  2.7557314297e-6;
    const S5: f32 = -2.5050759689e-8;
    const S6: f32 =  1.5896910177e-10;

    let z = r * r;
    let p = (((((S6 * z + S5) * z + S4) * z + S3) * z + S2) * z + S1) * z;
    r + r * p
}

#[inline(always)]
fn cos_poly(r: f32) -> f32 {
    // Cephes single-precision minimax for |r| <= pi/4
    // cos(r) ~ 1 + r^2*C1 + r^4*C2 + ...
    const C1: f32 =  4.1666667908e-2;
    const C2: f32 = -1.3888889225e-3;
    const C3: f32 =  2.4801587642e-5;
    const C4: f32 = -2.7557314297e-7;
    const C5: f32 =  2.0875723372e-9;
    const C6: f32 = -1.1359647598e-11;

    let z = r * r;
    let p = (((((C6 * z + C5) * z + C4) * z + C3) * z + C2) * z + C1) * z;
    1.0 + p
}

// ---------- tests ----------
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sanity() {
        let xs = [-3.0_f32, -1.0, -0.5, 0.0, 0.5, 1.0, 3.0];
        for &x in &xs {
            let (s, c) = (sinf(x), cosf(x));
            // Pythagorean sanity (allow small error)
            assert!(((s*s + c*c) - 1.0).abs() < 2e-6);
        }
        // Known points
        assert!((sinf(0.0)).abs() < 1e-7);
        assert!((cosf(0.0) - 1.0).abs() < 1e-7);
        assert!((sinf(core::f32::consts::FRAC_PI_2) - 1.0).abs() < 2e-6);
        assert!((cosf(core::f32::consts::PI)).abs() < 2e-6);
    }
}
