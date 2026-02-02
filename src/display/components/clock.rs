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
use embedded_graphics::pixelcolor::BinaryColor;
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
}

impl ClockDisplay {
    /// Create a new clock display component
    pub fn new(layout: LayoutConfig, clock_font: ClockFontData<'static>) -> Self {
        Self {
            state: ClockState::default(),
            clock_font,
            layout,
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
        // TODO: Implement actual clock rendering
        // This would draw:
        // - Large time digits using clock_font
        // - Blinking colon
        // - Date string
        // - Progress bar (if showing track progress)

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
