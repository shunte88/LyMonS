/*
 *  display/components/clock.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Clock display component
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *  Public License.
 *
 */

#![allow(dead_code)] // clock component helpers; some constants reserved

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::{BinaryColor, Gray4, Rgb565};
use crate::display::layout::LayoutConfig;
use crate::clock_font_svg::ClockFontData;
use std::time::Instant;

/// Clock display state
#[derive(Debug, Clone)]
pub struct ClockState {
    /// Last displayed time digits [H, H, :, M, M]
    pub last_clock_digits: [char; 5],

    /// Whether colon is currently shown (for blinking)
    pub colon_on: bool,

    /// Last time the colon was toggled
    pub last_colon_toggle_time: Instant,

    /// Last second drawn (for progress bar)
    pub last_second_drawn: f32,

    /// Last date string drawn
    pub last_date_drawn: String,
}

impl Default for ClockState {
    fn default() -> Self {
        Self {
            last_clock_digits: ['0', '0', ':', '0', '0'],
            colon_on: true,
            last_colon_toggle_time: Instant::now(),
            last_second_drawn: 0.0,
            last_date_drawn: String::new(),
        }
    }
}

/// Clock display component
pub struct ClockDisplay {
    state: ClockState,
    clock_font: ClockFontData,
    layout: LayoutConfig,
    metrics: bool,
}

impl ClockDisplay {
    /// Create a new clock display component
    pub fn new(layout: LayoutConfig, clock_font: ClockFontData, metrics: bool) -> Self {
        Self {
            state: ClockState::default(),
            clock_font,
            layout,
            metrics,
        }
    }

    /// Update the clock with current time
    pub fn update(&mut self, current_time_secs: f32) {
        // TODO: Update clock state based on current time
        self.state.last_second_drawn = current_time_secs;
    }

    /// Toggle colon for blinking effect
    pub fn toggle_colon(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.state.last_colon_toggle_time).as_millis() >= 500 {
            self.state.colon_on = !self.state.colon_on;
            self.state.last_colon_toggle_time = now;
        }
    }

    /// Render the clock display with specified color.
    ///
    /// `y_start` is the top-left Y of the `clock_digits` layout field —
    /// determined at layout-creation time so digits never overlap the progress bar.
    pub fn render<D>(&self, target: &mut D, y_start: i32, color: BinaryColor) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::primitives::{Rectangle as EgRectangle, PrimitiveStyleBuilder};
        use chrono::Local;

        let current_time = Local::now();

        // Get display dimensions from layout
        let w = self.layout.width;
        let _h = self.layout.height;

        // Format time into HH:MM string
        let hours_str = format!("{:02}", current_time.format("%H"));
        let minutes_str = format!("{:02}", current_time.format("%M"));

        // Determine colon state for blinking
        let current_second: u32 = current_time.format("%S").to_string().parse().unwrap_or(0);
        let colon_on = current_second % 2 == 0;

        let time_chars: [char; 5] = [
            hours_str.chars().nth(0).unwrap_or('0'),
            hours_str.chars().nth(1).unwrap_or('0'),
            if colon_on { ':' } else { ' ' },
            minutes_str.chars().nth(0).unwrap_or('0'),
            minutes_str.chars().nth(1).unwrap_or('0'),
        ];

        let digit_width = self.clock_font.digit_width as i32;
        let _digit_height = self.clock_font.digit_height as i32;

        // Constants for spacing (from original code)
        const CLOCK_DIGIT_GAP_HORIZONTAL: i32 = 1;
        const CLOCK_COLON_MINUTE_GAP: i32 = 1;

        // Calculate total width of clock digits
        let mut total_clock_visual_width: i32 = (digit_width * 5) +
                                             CLOCK_DIGIT_GAP_HORIZONTAL * 2 + // H-H and H-Colon gaps
                                             CLOCK_COLON_MINUTE_GAP +          // Colon-M1 gap
                                             CLOCK_DIGIT_GAP_HORIZONTAL;       // M1-M2 gap

        if total_clock_visual_width > w as i32 { total_clock_visual_width = w as i32; }

        let clock_x_start: i32 = (w as i32 - total_clock_visual_width) / 2;
        let x_positions: [i32; 5] = [
            clock_x_start,
            clock_x_start + digit_width + CLOCK_DIGIT_GAP_HORIZONTAL,
            clock_x_start + (digit_width * 2) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2),
            clock_x_start + (digit_width * 3) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP,
            clock_x_start + (digit_width * 4) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP,
        ];

        for i in 0..5 {
            EgRectangle::new(
                Point::new(x_positions[i], y_start),
                Size::new(self.clock_font.digit_width, self.clock_font.digit_height),
            )
            .into_styled(PrimitiveStyleBuilder::new().fill_color(BinaryColor::Off).build())
            .draw(target)?;
            self.draw_clock_char(target, time_chars[i], x_positions[i], y_start, color)?;
        }
        Ok(())
    }

    /// Render the clock display on grayscale displays with specified color.
    pub fn render_gray4<D>(&self, target: &mut D, y_start: i32, color: Gray4) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::primitives::{Rectangle as EgRectangle, PrimitiveStyleBuilder};
        use chrono::Local;

        let current_time = Local::now();
        let w = self.layout.width;
        let hours_str   = format!("{:02}", current_time.format("%H"));
        let minutes_str = format!("{:02}", current_time.format("%M"));
        let current_second: u32 = current_time.format("%S").to_string().parse().unwrap_or(0);
        let colon_on = current_second % 2 == 0;
        let time_chars: [char; 5] = [
            hours_str.chars().nth(0).unwrap_or('0'),
            hours_str.chars().nth(1).unwrap_or('0'),
            if colon_on { ':' } else { ' ' },
            minutes_str.chars().nth(0).unwrap_or('0'),
            minutes_str.chars().nth(1).unwrap_or('0'),
        ];
        let digit_width = self.clock_font.digit_width as i32;
        const CLOCK_DIGIT_GAP_HORIZONTAL: i32 = 1;
        const CLOCK_COLON_MINUTE_GAP: i32 = 1;
        let mut total_w = (digit_width * 5) + CLOCK_DIGIT_GAP_HORIZONTAL * 2
            + CLOCK_COLON_MINUTE_GAP + CLOCK_DIGIT_GAP_HORIZONTAL;
        if total_w > w as i32 { total_w = w as i32; }
        let clock_x_start = (w as i32 - total_w) / 2;
        let x_positions: [i32; 5] = [
            clock_x_start,
            clock_x_start + digit_width + CLOCK_DIGIT_GAP_HORIZONTAL,
            clock_x_start + (digit_width * 2) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2),
            clock_x_start + (digit_width * 3) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP,
            clock_x_start + (digit_width * 4) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP,
        ];
        for i in 0..5 {
            EgRectangle::new(
                Point::new(x_positions[i], y_start),
                Size::new(self.clock_font.digit_width, self.clock_font.digit_height),
            )
            .into_styled(PrimitiveStyleBuilder::new().fill_color(Gray4::BLACK).build())
            .draw(target)?;
            self.draw_clock_char_gray4(target, time_chars[i], x_positions[i], y_start, color)?;
        }
        Ok(())
    }

    /// Render the clock display on Rgb565 displays with specified colour.
    pub fn render_rgb565<D>(&self, target: &mut D, y_start: i32, color: Rgb565) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::primitives::{Rectangle as EgRectangle, PrimitiveStyleBuilder};
        use chrono::Local;

        let current_time = Local::now();
        let w = self.layout.width;
        let hours_str   = format!("{:02}", current_time.format("%H"));
        let minutes_str = format!("{:02}", current_time.format("%M"));
        let current_second: u32 = current_time.format("%S").to_string().parse().unwrap_or(0);
        let colon_on = current_second % 2 == 0;
        let time_chars: [char; 5] = [
            hours_str.chars().nth(0).unwrap_or('0'),
            hours_str.chars().nth(1).unwrap_or('0'),
            if colon_on { ':' } else { ' ' },
            minutes_str.chars().nth(0).unwrap_or('0'),
            minutes_str.chars().nth(1).unwrap_or('0'),
        ];
        let digit_width = self.clock_font.digit_width as i32;
        const CLOCK_DIGIT_GAP_HORIZONTAL: i32 = 1;
        const CLOCK_COLON_MINUTE_GAP: i32 = 1;
        let mut total_w = (digit_width * 5) + CLOCK_DIGIT_GAP_HORIZONTAL * 2
            + CLOCK_COLON_MINUTE_GAP + CLOCK_DIGIT_GAP_HORIZONTAL;
        if total_w > w as i32 { total_w = w as i32; }
        let clock_x_start = (w as i32 - total_w) / 2;
        let x_positions: [i32; 5] = [
            clock_x_start,
            clock_x_start + digit_width + CLOCK_DIGIT_GAP_HORIZONTAL,
            clock_x_start + (digit_width * 2) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2),
            clock_x_start + (digit_width * 3) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP,
            clock_x_start + (digit_width * 4) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP,
        ];
        for i in 0..5 {
            EgRectangle::new(
                Point::new(x_positions[i], y_start),
                Size::new(self.clock_font.digit_width, self.clock_font.digit_height),
            )
            .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::BLACK).build())
            .draw(target)?;
            self.draw_clock_char_rgb565(target, time_chars[i], x_positions[i], y_start, color)?;
        }
        Ok(())
    }

    /// Draw a single clock character on Rgb565 display with specified color
    fn draw_clock_char_rgb565<D>(&self, target: &mut D, c: char, x: i32, y: i32, color: Rgb565) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        if let Some(alpha) = self.clock_font.get_char_alpha(c) {
            let width  = self.clock_font.digit_width;
            let height = self.clock_font.digit_height;
            let (r5, g6, b5) = (color.r(), color.g(), color.b());
            for dy in 0..height {
                for dx in 0..width {
                    let a = alpha[(dy * width + dx) as usize];
                    if a > 0 {
                        let blend = |ch: u8, max: u8| -> u8 { ((ch as u32 * a as u32 / 255) as u8).min(max) };
                        target.draw_iter(core::iter::once(Pixel(
                            Point::new(x + dx as i32, y + dy as i32),
                            Rgb565::new(blend(r5, 31), blend(g6, 63), blend(b5, 31)),
                        )))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Draw a single clock character at the specified position
    fn draw_clock_char<D>(&self, target: &mut D, c: char, x: i32, y: i32, _color: BinaryColor) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        if let Some(alpha) = self.clock_font.get_char_alpha(c) {
            let width  = self.clock_font.digit_width;
            let height = self.clock_font.digit_height;
            for dy in 0..height {
                for dx in 0..width {
                    if alpha[(dy * width + dx) as usize] >= 128 {
                        target.draw_iter(core::iter::once(Pixel(
                            Point::new(x + dx as i32, y + dy as i32),
                            BinaryColor::On,
                        )))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Draw a single clock character on grayscale display with specified color
    fn draw_clock_char_gray4<D>(&self, target: &mut D, c: char, x: i32, y: i32, color: Gray4) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        if let Some(alpha) = self.clock_font.get_char_alpha(c) {
            let width  = self.clock_font.digit_width;
            let height = self.clock_font.digit_height;
            let luma = color.luma(); // 0–15
            for dy in 0..height {
                for dx in 0..width {
                    let a = alpha[(dy * width + dx) as usize];
                    if a > 0 {
                        let level = (luma as u32 * a as u32 / 255) as u8;
                        target.draw_iter(core::iter::once(Pixel(
                            Point::new(x + dx as i32, y + dy as i32),
                            Gray4::new(level),
                        )))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Get current state
    pub fn state(&self) -> &ClockState {
        &self.state
    }

    /// Set date string
    pub fn set_date(&mut self, date: String) {
        self.state.last_date_drawn = date;
    }
}
