/*
 *  display/components/weather.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Weather display component
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
use crate::weather::WeatherData;
use std::time::Instant;

/// Weather display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherDisplayMode {
    /// Show current weather
    Current,

    /// Show weather forecast
    Forecast,
}

/// Weather display component
pub struct WeatherDisplay {
    layout: LayoutConfig,
    last_weather_data: Vec<WeatherData>,
    display_mode: WeatherDisplayMode,
    display_switch_timer: Option<Instant>,
}

impl WeatherDisplay {
    /// Create a new weather display component
    pub fn new(layout: LayoutConfig) -> Self {
        Self {
            layout,
            last_weather_data: Vec::new(),
            display_mode: WeatherDisplayMode::Current,
            display_switch_timer: None,
        }
    }

    /// Update weather data
    pub fn update(&mut self, weather_data: Vec<WeatherData>) {
        self.last_weather_data = weather_data;
    }

    /// Get current display mode
    pub fn display_mode(&self) -> WeatherDisplayMode {
        self.display_mode
    }

    /// Set display mode
    pub fn set_display_mode(&mut self, mode: WeatherDisplayMode) {
        self.display_mode = mode;
    }

    /// Toggle between current and forecast
    pub fn toggle_mode(&mut self) {
        self.display_mode = match self.display_mode {
            WeatherDisplayMode::Current => WeatherDisplayMode::Forecast,
            WeatherDisplayMode::Forecast => WeatherDisplayMode::Current,
        };
        self.display_switch_timer = Some(Instant::now());
    }

    /// Check if weather data has changed
    pub fn has_changed(&self, new_data: &[WeatherData]) -> bool {
        if self.last_weather_data.len() != new_data.len() {
            return true;
        }

        for (old, new) in self.last_weather_data.iter().zip(new_data.iter()) {
            if old != new {
                return true;
            }
        }

        false
    }

    /// Render the weather display
    pub fn render<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        // TODO: Implement actual weather rendering
        // This would draw:
        // - Weather icon (SVG)
        // - Temperature
        // - Condition text
        // - Forecast data (if in forecast mode)

        Ok(())
    }

    /// Get weather icon path for current condition
    pub fn get_icon_path(&self) -> Option<String> {
        if let Some(current) = self.last_weather_data.first() {
            Some(current.weather_code.svg.clone())
        } else {
            None
        }
    }

    /// Get weather data
    pub fn weather_data(&self) -> &[WeatherData] {
        &self.last_weather_data
    }
}
