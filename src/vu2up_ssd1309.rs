/*
 *  vu2up_ssd1309.rs
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
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
};

/// Draw the SSD1309 2-up VU needle
#[allow(dead_code)]
pub fn draw_vu_needle<D>(
    display: &mut D,
    panel: Rectangle,
    db: f32,
    sweep_min: i32,
    sweep_max: i32,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    let origin = panel.top_left;
    let Size { width, height } = panel.size;

    if width < 40 || height < 32 {
        return Ok(());
    }
    
    // Layout: pivot near bottom center; radius sized to panel
    let h = height as i32;
    let cx = origin.x + width as i32/ 2;
    let cy = origin.y + h - 6;
    let r_arc  = h/2 + h/6;
    let r_needle = r_arc - 28;
    let style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

    let ang = vu_db_to_meter_angle(db, sweep_min, sweep_max); // degrees; −3 dB => 0°
    let p_out = polar_point(cx, cy, r_arc, ang);
    let p_in  = polar_point(cx, cy, r_needle, ang);
    Line::new(p_in, p_out).into_styled(style).draw(display)?;

    Ok(())
}

/// Draw the SSD1309 2-up VU face (one panel): arc + ticks.
/// `panel`: the rectangle already laid out for this meter panel.
#[allow(dead_code)]
pub fn draw_vu_face<D>(
    display: &mut D,
    panel: Rectangle,
    sweep_min: i32, // = -48;
    sweep_max: i32, // = 48;
) -> Result<Point, D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    let origin = panel.top_left;
    let Size { width, height } = panel.size;

    if width < 40 || height < 32 {
        return Ok(Point::zero()); // too small to draw meaningfully
    }

    // Layout: pivot near bottom center; radius sized to panel
    let w = width as i32;
    let h = height as i32;
    let cx = origin.x + w / 2;
    let cy = origin.y + h - 6;   // a few px above panel bottom
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
            Line::new(pp, p).into_styled(style).draw(display)?;
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
        Line::new(p_in, p_out).into_styled(style).draw(display)?;
    }

    // Draw minor ticks
    for &db in &DB_MINOR {
        let ang = vu_db_to_meter_angle(db, sweep_min, sweep_max);
        let p_out = polar_point(cx, cy, r_tick, ang);
        let p_in  = polar_point(cx, cy, r_in_minor, ang);
        Line::new(p_in, p_out).into_styled(style).draw(display)?;
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
