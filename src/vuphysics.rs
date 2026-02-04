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

use log::{warn};
use std::time::Instant;
use core::any::Any;
use core::convert::Infallible;
use embedded_graphics::{
    geometry::{Point, Size},
    pixelcolor::{BinaryColor, PixelColor},
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
};

use crate::vframebuf::VarFrameBuf;

// classic visual range for scale
#[allow(dead_code)]
pub const VU_FLOOR_DB: f32 = -23.0;
#[allow(dead_code)]
pub const VU_CEIL_DB:  f32 = 5.0;
#[allow(dead_code)]
pub const VU_GAMMA:    f32 = 0.7; // lift lows a bit; 1.0 = linear

/// Row-major snapshot of a rectangle.
#[derive(Debug, Clone, PartialEq)]
struct SavedRegion<C: PixelColor> {
    rect: Rectangle,
    pixels: Vec<C>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Scale <C: PixelColor + Clone + Default>{
    sweep_min: f64,
    sweep_max: f64,
    scale_min: f64,
    scale_max: f64,
    top_y: i32,
    bottom_y: i32,
    pub color: C,
    saved: Option<SavedRegion<C>>,
}


impl <C: PixelColor + Clone + Default>Scale<C> {

    pub fn init(
        sweep_min: f64, 
        sweep_max: f64, 
        scale_min: f64, 
        scale_max: f64, 
        top_y: i32, 
        bottom_y: i32,
    ) -> Self {
        Self {
            sweep_min,
            sweep_max,
            scale_min,
            scale_max,
            top_y,
            bottom_y,
            color: C::default(),
            saved: None,
        }
    }

    fn map_value_to_angle(&self, value: f64) -> f64 {
    
        let mut value = value;
        if value < self.scale_min {
            value = self.scale_min as f64;
        } else if value > self.scale_max {
            value = self.scale_max as f64;
        }
        
        let normalized = (value - self.scale_min) / (self.scale_max - self.scale_min);
        self.sweep_min + normalized * (self.sweep_max - self.sweep_min)

    }

    // BlitBlat needle draw
    pub fn draw_vu_needle<D>(
        &mut self,
        target: &mut D,
        panel: Rectangle,
        db: f32,
        stroke_width: u32,
        color: C,
        border_color: C,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C> + OriginDimensions + 'static,
        C: PixelColor + Clone + Default + 'static
    {

        // Restore prior background and erase last needle draw
        //if let Some(saved) = self.saved.clone().take() {
        if let Some(saved) = self.saved.take() {
            restore_region(target, &saved).unwrap();
        }

        let origin = panel.top_left;
        let width = panel.size.width;
        
        // layout
        let mut cx = width as i32/ 2;
        cx += origin.x;
        let cy = self.bottom_y;
        let r_arc  = self.bottom_y - self.top_y;

        let style = PrimitiveStyle::with_stroke(
            color.clone(),
            stroke_width
        );

        // construct needle
        let ang = self.map_value_to_angle(db as f64);
        let p0 = Point::new(cx, cy);
        let p1 = polar_point(cx, cy, r_arc, ang as f32);

        // Build minimal rect and inflate by stroke width so we capture full needle thickness
        let mut rect = rect_from_line(p0, p1);
        rect = inflate_rect(rect, ((style.stroke_width as i32) + 2) / 2);

        // Clip to framebuffer bounds - should limit to the panel specified
        // so we can incorporate the "well and pivot"
        let fb_rect = target.bounding_box();
        let clipped = match rect.intersection(&fb_rect) {
            r if r.size.width > 0 && r.size.height > 0 => r,
            _ => {
                Line::new(p0, p1).into_styled(style).draw(target)?;
                return Ok(());
            }
        };

        // save region for next draw
        let saved = save_region(target, clipped);
        self.saved = Some(saved);

        // draw the new needle position with a drop-shadow
        let cleanstyle = PrimitiveStyle::with_stroke(
            border_color.clone(),
            stroke_width
        );
        let shadow = Point::new(1,1); // bump down, right
        Line::new(p0+shadow, p1+shadow).into_styled(cleanstyle).draw(target)?;
        Line::new(p0, p1).into_styled(style).draw(target)?;

        Ok(())

    }

}

/// Classic 2nd-order needle: m ẍ + c ẋ + k x + c2 |ẋ| ẋ = g * u
/// x in [0,1] is needle deflection, u in [0,1] is drive (from audio level map).
#[allow(dead_code)]
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
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct VuNeedleNew {
    pub needle: NeedleNew,
    sweep_min: i32, 
    sweep_max: i32,
    last_t: Instant,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
impl VuNeedleNew {
    pub fn new_vu(sweep_min: i32, sweep_max: i32) -> Self {
        Self { 
            needle: NeedleNew::vu_classic(),
            sweep_min, 
            sweep_max,
            last_t: Instant::now() }
    }
    pub fn reset(&mut self) {
        self.needle.x = 0.0;
        self.needle.v = 0.0;
        self.last_t = Instant::now();
    }
    /// Call once per draw with current dB value. Returns needle displacement (-3dB center).
    pub fn update_db(&mut self, db: f32) -> f32 {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_t).as_secs_f32().clamp(0.0, 0.05); // cap dt
        self.last_t = now;
        let u = vu_db_to_meter_angle(db, self.sweep_min, self.sweep_max);
        self.needle.step(dt, u)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct NeedleMetrics {
    pub mass: f32,      // needle/spring mass (kg-ish units, relative)
    pub k: f32,         // spring constant
    pub damp: f32,      // linear damping
    pub dispx: f32,     // displacement in [0,1]
    pub velocity: f32,  // velocity
    pub over: u32,      // overdrive counter (>=5ms => LED on)
}

#[allow(dead_code)]
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

/// Integrate the needle for `millis` milliseconds with drive `force` in [0,1].
/// Port of `needlePhysics()` from LMSMonitor loop (Euler at 1 ms; clamps + velocity flip).
#[allow(dead_code)]
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

    // Overload pump - trigger at 0.3 VU
    if sm.dispx > 0.3 { sm.over += 1; } else { sm.over = 0; }

    sm.dispx
}

/// Time-based wrapper
/// Give it your raw `metric` (e.g. `sample_accum`) and it does:
///   force = get_force(metric / fudge)
///   steps Euler at 1ms over the real elapsed time since last call
/// future color support
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct VuNeedle<C: PixelColor + Clone + Default> {
    pub n: NeedleMetrics,
    last_t: Instant,
    color: C,
    saved: Option<SavedRegion<C>>,
    pub fudge: f32,   // C used 9000.0
    pub x_min: f32,   // 0.0
    pub x_max: f32,   // 1.0
}

#[allow(dead_code)]
impl<C: PixelColor + Clone + Default> VuNeedle<C> {
    pub fn new() -> Self {
        Self {
            n: NeedleMetrics::default(),
            last_t: Instant::now(),
            fudge: 1.0, // unity
            x_min: -22.0,
            x_max: 4.8,
            color: C::default(),
            saved: None,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.n.dispx = 0.0;
        self.n.velocity = 0.0;
        self.n.over = 0;
        self.last_t = Instant::now();
        self.saved = None;
    }

    /// Update by normalized drive u (dB).
    pub fn update_drive(&mut self, u: f32) -> (f32, bool) {

        let now = std::time::Instant::now();
        let mut dt_ms = now.saturating_duration_since(self.last_t).as_millis() as u32;
        self.last_t = now;
        if dt_ms > 50 { dt_ms = 50; }
        let x = step_ms(&mut self.n, u, self.x_min, self.x_max, dt_ms);
        (x, self.n.over > 4)
    }

    /// Update from raw metric (i32/float). Returns (deflection in 0..1, over_flag).
    pub fn update_db(&mut self, accum: f32) -> (f32, bool) {

        let now = Instant::now();
        let mut dt_ms = now.saturating_duration_since(self.last_t).as_millis() as u32;
        self.last_t = now;

        // clamp to avoid huge catch-up if we hiccup
        if dt_ms > 50 { dt_ms = 50; } // integrate at most 50 ms

        let force = get_force((accum / self.fudge).max(0.0));
        let x = step_ms(&mut self.n, force, self.x_min, self.x_max, dt_ms);
        (x, self.n.over > 4) // “LED on” after ~5 ms above 0.7, like C
    }

}

/// Utility: map VU dB (e.g. −20..+5) to a 0..1 drive.
/// Gamma < 1.0 makes low levels more visible; tweakable
#[allow(dead_code)]
#[inline]
pub fn db_to_drive(db: f32, floor_db: f32, ceil_db: f32, gamma: f32) -> f32 {
    let norm = ((db - floor_db) / (ceil_db - floor_db)).clamp(0.0, 1.0);
    norm.powf(gamma)
}

/// Exact port of my original C `getForce()` from LMSMonitor.
#[allow(dead_code)]
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

/// Geometry helper: convert deflection (0..1) into a tip point for a 90° sweep
/// centered on 45° (C code refactor).
#[allow(dead_code)]
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

/// Map VU dB to the **meter angle in degrees** with exponential spacing:
/// angle = θ * 10^(db/20) shifted so −3 dB is 0°.
/// Positive angles sweep to the right; negative to the left.
#[allow(dead_code)]
#[inline]
pub fn vu_db_to_meter_angle(db: f32, sweep_min: i32, sweep_max: i32) -> f32 {
    // θ from original C: sweep / sqrt(2)
    let sweep = sweep_min.abs() as f32 + sweep_max.abs() as f32;
    let theta = sweep / 2.0_f32.sqrt();
    //let theta = 90.0 / 2.0_f32.sqrt();
    let a_db   = theta * 10f32.powf(db / 20.0);
    let a_m3db = theta * 10f32.powf(-3.0 / 20.0); // reference at −3 dB
    a_db - a_m3db
}

/// Convert a *relative* meter angle (deg, − left .. + right) into the
/// absolute drawing angle for the panel arc sweep. Here we keep it equal,
/// but may remap if coordinate system needs it.
#[allow(dead_code)]
#[inline]
fn vu_meter_angle_deg(rel_deg: f32) -> f32 {
    rel_deg
}

/// Simple polar helper (degrees).
#[allow(dead_code)]
#[inline]
fn polar_point(cx: i32, cy: i32, r: i32, deg: f32) -> Point {
    let rad = deg.to_radians();
    let (s, c) = (rad.sin(), rad.cos());
    Point::new(cx + (s * r as f32) as i32, cy - (c * r as f32) as i32)
}

fn rect_from_line(a: Point, b: Point) -> Rectangle {
    use core::cmp::{max, min};
    let x0 = min(a.x, b.x);
    let y0 = min(a.y, b.y);
    let x1 = max(a.x, b.x);
    let y1 = max(a.y, b.y);
    Rectangle::with_corners(Point::new(x0, y0), Point::new(x1, y1))
}

fn inflate_rect(r: Rectangle, pad: i32) -> Rectangle {
    let tl = Point::new(r.top_left.x - pad, r.top_left.y - pad);
    let br = Point::new(
        r.bottom_right().unwrap().x + pad,
        r.bottom_right().unwrap().y + pad,
    );
    Rectangle::with_corners(tl, br)
}

// this how the magic happens
fn save_region<D, C>(
    target: &mut D,
    rect: Rectangle
) -> SavedRegion<C> 
where
    D: DrawTarget<Color = C> + OriginDimensions + 'static,
    C: PixelColor + Clone + Default + 'static
{
    // Safe: rect already clipped to fb bounds
    let Size { width, height } = rect.size;
    let w = target.size().width as usize;

    let mut pixels = Vec::with_capacity((width * height) as usize);

    if let Some(fb) = (target as &mut dyn Any).downcast_mut::<VarFrameBuf<C>>() {

        let buf = fb.as_slice();

        let x0 = rect.top_left.x as usize;
        let y0 = rect.top_left.y as usize;

        for row in 0..(height as usize) {
            let y = y0 + row;
            let base = y * w + x0;
            let slice = &buf[base .. base + (width as usize)];
            pixels.extend_from_slice(slice);
        }

    } else {
        // Handle the case where the DrawTarget is not a Framebuffer flavor
        warn!("The provided DrawTarget is not a VarFrameBuf. BlitBlat is not supported");
    }

    SavedRegion { rect, pixels }

}

// BlitBlat restore region
fn restore_region<D, C>(
    target: &mut D,
    saved: &SavedRegion<C>,
) -> Result<(), Infallible> 
where
    D: DrawTarget<Color = C> + OriginDimensions + 'static,
    C: PixelColor + Clone + Default + 'static
{
    let Size { width, height } = saved.rect.size;
    if width == 0 || height == 0 { return Ok(()); }

    // fast path: write back into fb buffer directly
    if let Some(fb) = (target as &mut dyn Any).downcast_mut::<VarFrameBuf<C>>() {
        
        let w = fb.width();
        let x0 = saved.rect.top_left.x as usize;
        let y0 = saved.rect.top_left.y as usize;

        let mut src_idx = 0usize;
        let fb_buf = fb.as_mut_slice();

        for row in 0..(height as usize) {
            let y = y0 + row;
            let dst_base = y * w + x0;
            let row_len = width as usize;

            // copy and paste one row
            let dst = &mut fb_buf[dst_base .. dst_base + row_len];
            let src = &saved.pixels[src_idx .. src_idx + row_len];
            dst.copy_from_slice(src);

            src_idx += row_len;
        }
    }

    Ok(())

}
