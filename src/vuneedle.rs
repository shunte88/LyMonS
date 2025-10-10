/*
 *  vuneedle.rs
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

use core::cmp::{max, min};
use core::convert::Infallible;
use embedded_graphics::{
    geometry::{Point, Size},
    image::{Image, ImageRaw},
    pixelcolor::{BinaryColor, PixelColor},
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
};
use crate::vframebuf::VarFrameBuf;

/// Row-major snapshot of a rectangle.
#[derive(Clone)]
struct SavedRegion<C: PixelColor> {
    rect: Rectangle,
    pixels: Vec<C>,
}

/// VU-needle with save/restore blit.
pub struct VuNeedle<C: PixelColor + Clone + Default> {
    pub pivot: Point,
    pub length: i32,
    pub stroke_width: u32,
    pub color: C,
    saved: Option<SavedRegion<C>>,
}

impl<C: PixelColor + Clone + Default> VuNeedle<C> {
    pub fn new(pivot: Point, length: i32, stroke_width: u32, color: C) -> Self {
        Self {
            pivot,
            length,
            stroke_width: stroke_width.max(1),
            color,
            saved: None,
        }
    }

    /// Draw the next position of the needle at `angle_rad` (radians).
    /// Flow: restore prev region -> compute new -> save -> draw.
    pub fn draw_next(
        &mut self, 
        fb: &mut VarFrameBuf<C>, 
        angle_rad: f32
    ) -> Result<(), Infallible>
    where 
        C: PixelColor + Clone + Default 
    {
        // 1) Restore prior background (erase old needle)
        if let Some(saved) = self.saved.take() {
            restore_region(fb, &saved)?;
        }

        // 2) Compute line endpoints from pivot/length/angle
        let (p0, p1) = line_from_pivot(self.pivot, self.length, angle_rad);

        // Build minimal rect and inflate by stroke width so we capture full needle thickness
        let mut rect = rect_from_line(p0, p1);
        rect = inflate_rect(rect, ((self.stroke_width as i32) + 1) / 2);

        // Clip to framebuffer bounds
        let fb_rect = Rectangle::new(Point::new(0, 0), Size::new(fb.width() as u32, fb.height() as u32));
        let clipped = match rect.intersection(&fb_rect) {
            r if r.size.width > 0 && r.size.height > 0 => r,
            _ => {
                // Nothing visible; just draw (EG will clip) and skip save to avoid empty region
                let style = PrimitiveStyle::with_stroke(self.color.clone(), self.stroke_width);
                Line::new(p0, p1).into_styled(style).draw(fb)?;
                return Ok(());
            }
        };

        // 3) Save region
        let saved = save_region(fb, clipped);
        self.saved = Some(saved);

        // 4) Draw the needle
        let style = PrimitiveStyle::with_stroke(self.color.clone(), self.stroke_width);
        Line::new(p0, p1).into_styled(style).draw(fb)?;

        Ok(())
    }

    /// Optional: explicitly clear any saved background (e.g., on mode change)
    pub fn clear_saved(&mut self) {
        self.saved = None;
    }
}

fn line_from_pivot(pivot: Point, length: i32, angle_rad: f32) -> (Point, Point) {
    let dx = (angle_rad.cos() * length as f32) as i32;
    let dy = (angle_rad.sin() * length as f32) as i32;

    let p0 = pivot; // pivot
    let p1 = Point::new(pivot.x + dx, pivot.y + dy); // tip
    (p0, p1)
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

fn save_region<C: PixelColor + Clone + Default>(
    fb: &VarFrameBuf<C>, 
    rect: Rectangle
) -> SavedRegion<C> {
    // Safe: rect already clipped to fb bounds
    let Size { width, height } = rect.size;
    let (w, h) = (fb.width(), fb.height());

    let mut pixels = Vec::with_capacity((width * height) as usize);
    let buf = fb.as_slice();

    let x0 = rect.top_left.x as usize;
    let y0 = rect.top_left.y as usize;

    for row in 0..(height as usize) {
        let y = y0 + row;
        let base = y * w + x0;
        let slice = &buf[base .. base + (width as usize)];
        pixels.extend_from_slice(slice);
    }

    SavedRegion { rect, pixels }
}

fn restore_region<C: PixelColor + Clone + Default>(
    fb: &mut VarFrameBuf<C>,
    saved: &SavedRegion<C>,
) -> Result<(), Infallible> 
{
    let Size { width, height } = saved.rect.size;
    if width == 0 || height == 0 { return Ok(()); }

    // Fast path: write back into fb buffer directly
    let w = fb.width();
    let x0 = saved.rect.top_left.x as usize;
    let y0 = saved.rect.top_left.y as usize;

    let mut src_idx = 0usize;
    let fb_buf = fb.as_mut_slice();

    for row in 0..(height as usize) {
        let y = y0 + row;
        let dst_base = y * w + x0;
        let row_len = width as usize;

        // Copy one row
        let dst = &mut fb_buf[dst_base .. dst_base + row_len];
        let src = &saved.pixels[src_idx .. src_idx + row_len];
        dst.copy_from_slice(src);

        src_idx += row_len;
    }

    Ok(())

}

/// Draw the SSD1309 2-up VU needle
#[allow(dead_code)]
pub fn draw_vu_needle<D>(
    target: &mut D,
    panel: Rectangle,
    db: f32,
    sweep_min: i32,
    sweep_max: i32,
    mode: bool,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    let origin = panel.top_left;
    let Size { width, height } = panel.size;

    if width < 40 || height < 32 {
        return Ok(());
    }
    
    // Layout: pivot near bottom pivot; radius sized to panel
    let h = height as i32;
    let cx = origin.x + width as i32/ 2;
    let cy = origin.y + h - 6;
    let r_arc  = h/2 + h/6;
    let r_needle = r_arc - 28;

    let style = PrimitiveStyle::with_stroke(
        if mode { BinaryColor::On } else { BinaryColor::Off },
        2
    );

    let ang = vu_db_to_meter_angle(db, sweep_min, sweep_max); // degrees; −3 dB => 0°
    let p_out = polar_point(cx, cy, r_arc, ang);
    let p_in  = polar_point(cx, cy, r_needle, ang);
    Line::new(p_in, p_out).into_styled(style).draw(target)?;

    Ok(())

}

/// Draw the SSD1309 2-up VU face (one panel): arc + ticks.
/// `panel`: the rectangle already laid out for this meter panel.
#[allow(dead_code)]
pub fn draw_vu_face<D>(
    target: &mut D,
    panel: Rectangle,
    sweep_min: i32, // = -48;
    sweep_max: i32, // = 48;
    buffer: Vec<u8>,
) -> Result<Point, D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    let origin = panel.top_left;
    let Size { width, height } = panel.size;

    if width < 40 || height < 32 {
        return Ok(Point::zero()); // too small to draw meaningfully
    }

    // Layout: pivot near bottom pivot; radius sized to panel
    let w = width as i32;
    let h = height as i32;
    let cx = origin.x + w / 2;
    let mut cy = origin.y + h - 6;   // a few px above panel bottom
    if buffer.len() == 0 {

        let r_arc  = h/2 + h/6;
        let _r_arc  = (h * 3) / 4;    // arc radius
        let r_tick = r_arc;          // outer tick radius
        let r_in_major = r_tick - 8; // major tick length
        let r_in_minor = r_tick - 4; // minor tick length

        // major
        let style0 = PrimitiveStyle::with_stroke(BinaryColor::On, 2);
        // minor
        let style1 = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
        let mut style = style1;
        // --- Arc: polyline at radius r_arc across sweep (1° steps) ---
        let mut prev: Option<Point> = None;
        for deg in sweep_min..=sweep_max {
            let p = polar_point(cx, cy, r_arc, vu_meter_angle_deg(deg as f32));
            if let Some(pp) = prev {
                if deg == 5 {
                    style = style0;
                }
                Line::new(pp, p).into_styled(style).draw(target)?;
            }
            prev = Some(p);
        }

        // --- Ticks ---
        // Major: sparse, longer lines
        const DB_MAJOR: [f32; 5] = [-20.0, -10.0, -3.0, 0.0, 3.0];
        // Minor: the intermediates; shorter
        const DB_MINOR: [f32; 8] = [-7.0, -6.0, -5.0, -4.0, -2.0, -1.0, 1.0, 2.0];

        style = style1;
        // Draw major ticks
        for &db in &DB_MAJOR {
            let ang = vu_db_to_meter_angle(db, sweep_min, sweep_max); // degrees; −3 dB => 0°
            let p_out = polar_point(cx, cy, r_tick, ang);
            let p_in  = polar_point(cx, cy, r_in_major, ang);
            Line::new(p_in, p_out).into_styled(style).draw(target)?;
        }

        // Draw minor ticks
        for &db in &DB_MINOR {
            let ang = vu_db_to_meter_angle(db, sweep_min, sweep_max);
            let p_out = polar_point(cx, cy, r_tick, ang);
            let p_in  = polar_point(cx, cy, r_in_minor, ang);
            Line::new(p_in, p_out).into_styled(style).draw(target)?;
        }
    } else {
        // Blit to target
        let raw = ImageRaw::<BinaryColor>::new(&buffer, width);
        Image::new(&raw, Point::new(panel.top_left.x, panel.top_left.y))
            .draw(target)
            .map_err(|e|D::Error::from(e))?;
        cy = 84;
    }
    Ok(Point::new(cx,cy))

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
    let theta = 90.00 / 2.0_f32.sqrt();
    let a_db   = theta * 10f32.powf(db / 20.0);
    let a_m3db = theta * 10f32.powf(-3.0 / 20.0); // reference at −3 dB
    a_db - a_m3db
}

/// Convert a *relative* meter angle (deg, − left .. + right) into the
/// absolute drawing angle for the panel arc sweep. Here we keep it equal,
/// but you can remap if your coordinate system needs it.
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
