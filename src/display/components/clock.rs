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

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::{BinaryColor, Gray4};
use crate::display::layout::LayoutConfig;
use crate::clock_font::ClockFontData;
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
    clock_font: ClockFontData<'static>,
    layout: LayoutConfig,
    metrics: bool,
}

impl ClockDisplay {
    /// Create a new clock display component
    pub fn new(layout: LayoutConfig, clock_font: ClockFontData<'static>, metrics: bool) -> Self {
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

    /// Render the clock display
    pub fn render<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::primitives::{Rectangle as EgRectangle, PrimitiveStyleBuilder};
        use chrono::Local;

        let current_time = Local::now();

        // Get display dimensions from layout
        let w = self.layout.width;
        let h = self.layout.height;

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
        let digit_height = self.clock_font.digit_height as i32;

        // Constants for spacing (from original code)
        const CLOCK_DIGIT_GAP_HORIZONTAL: i32 = 1;
        const CLOCK_COLON_MINUTE_GAP: i32 = 1;

        // Calculate total width of clock digits
        let mut total_clock_visual_width: i32 = (digit_width * 5) +
                                             CLOCK_DIGIT_GAP_HORIZONTAL * 2 + // H-H and H-Colon gaps
                                             CLOCK_COLON_MINUTE_GAP +          // Colon-M1 gap
                                             CLOCK_DIGIT_GAP_HORIZONTAL;       // M1-M2 gap

        if total_clock_visual_width > w as i32{
            total_clock_visual_width = w as i32;
        }
                                             // Calculate Y position to center the clock vertically
        // this should be coming from field definition
        let clock_y_start = ((h as i32 - (digit_height+2)) / 2).max(0);

        // Calculate X positions for horizontal centering
        let clock_x_start: i32 = (w as i32 - total_clock_visual_width) / 2;

        let x_positions: [i32; 5] = [
            clock_x_start, // H1
            clock_x_start + digit_width + CLOCK_DIGIT_GAP_HORIZONTAL, // H2
            clock_x_start + (digit_width * 2) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2), // Colon
            clock_x_start + (digit_width * 3) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP, // M1
            clock_x_start + (digit_width * 4) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP, // M2
        ];

        // Draw each digit
        for i in 0..5 {
            let current_char = time_chars[i];
            let x_offset = x_positions[i];
            let y_offset = clock_y_start;

            // Clear the digit area
            EgRectangle::new(
                Point::new(x_offset, y_offset),
                Size::new(self.clock_font.digit_width, self.clock_font.digit_height),
            )
            .into_styled(PrimitiveStyleBuilder::new()
                .fill_color(BinaryColor::Off)
                .build())
            .draw(target)?;

            // Draw the clock character using the font bitmap
            self.draw_clock_char(target, current_char, x_offset, y_offset)?;
        }

        Ok(())
    }

    /// Render the clock display on grayscale displays (monochrome-only, white on black)
    pub fn render_gray4<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::primitives::{Rectangle as EgRectangle, PrimitiveStyleBuilder};
        use chrono::Local;

        let current_time = Local::now();

        // Get display dimensions from layout
        let w = self.layout.width;
        let h = self.layout.height;

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
        let digit_height = self.clock_font.digit_height as i32;

        // Constants for spacing (from original code)
        const CLOCK_DIGIT_GAP_HORIZONTAL: i32 = 1;
        const CLOCK_COLON_MINUTE_GAP: i32 = 1;

        // Calculate total width of clock digits
        let mut total_clock_visual_width: i32 = (digit_width * 5) +
                                             CLOCK_DIGIT_GAP_HORIZONTAL * 2 + // H-H and H-Colon gaps
                                             CLOCK_COLON_MINUTE_GAP +          // Colon-M1 gap
                                             CLOCK_DIGIT_GAP_HORIZONTAL;       // M1-M2 gap

        if total_clock_visual_width > w as i32{
            total_clock_visual_width = w as i32;
        }

        // Center the clock horizontally and vertically in the layout
        let clock_x_start = ((w as i32) - total_clock_visual_width) / 2;
        let clock_y_start = ((h as i32) - digit_height) / 2;

        // Calculate x positions for each digit
        let x_positions: [i32; 5] = [
            clock_x_start, // H1
            clock_x_start + digit_width + CLOCK_DIGIT_GAP_HORIZONTAL, // H2
            clock_x_start + (digit_width * 2) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2), // Colon
            clock_x_start + (digit_width * 3) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP, // M1
            clock_x_start + (digit_width * 4) + (CLOCK_DIGIT_GAP_HORIZONTAL * 2) + CLOCK_COLON_MINUTE_GAP, // M2
        ];

        // Draw each digit
        for i in 0..5 {
            let current_char = time_chars[i];
            let x_offset = x_positions[i];
            let y_offset = clock_y_start;

            // Clear the digit area (black background)
            EgRectangle::new(
                Point::new(x_offset, y_offset),
                Size::new(self.clock_font.digit_width, self.clock_font.digit_height),
            )
            .into_styled(PrimitiveStyleBuilder::new()
                .fill_color(Gray4::BLACK)
                .build())
            .draw(target)?;

            // Draw the clock character using the font bitmap
            self.draw_clock_char_gray4(target, current_char, x_offset, y_offset)?;
        }

        Ok(())
    }

    /// Draw a single clock character at the specified position
    fn draw_clock_char<D>(&self, target: &mut D, c: char, x: i32, y: i32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::image::Image;

        // Get the image for this character from the clock font
        if let Some(image_raw) = self.clock_font.get_char_image_raw(c) {
            // Draw the image at the specified position
            Image::new(image_raw, Point::new(x, y))
                .draw(target)?;
        }

        Ok(())
    }

    /// Draw a single clock character on grayscale display (convert BinaryColor to Gray4::WHITE)
    fn draw_clock_char_gray4<D>(&self, target: &mut D, c: char, x: i32, y: i32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::primitives::Rectangle as EgRectangle;
        use embedded_graphics::image::GetPixel;

        // Get the image for this character from the clock font
        if let Some(image_raw) = self.clock_font.get_char_image_raw(c) {
            // Manually convert BinaryColor pixels to Gray4::WHITE
            // ImageRaw is stored as packed bits, so we need to iterate and convert
            let width = self.clock_font.digit_width;
            let height = self.clock_font.digit_height;

            for dy in 0..height {
                for dx in 0..width {
                    let px = Point::new(dx as i32, dy as i32);
                    if let Some(color) = image_raw.pixel(px) {
                        if color == BinaryColor::On {
                            // Draw white pixel on grayscale display
                            target.draw_iter(core::iter::once(Pixel(
                                Point::new(x + dx as i32, y + dy as i32),
                                Gray4::WHITE,
                            )))?;
                        }
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
