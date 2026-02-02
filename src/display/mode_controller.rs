/*
 *  display/mode_controller.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Display mode controller - handles automatic mode switching based on
 *  player state, time, and configuration
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

use super::DisplayMode;
use chrono::{Local, Timelike};
use std::time::Instant;

/// Configuration for display mode controller
#[derive(Debug, Clone)]
pub struct ModeControllerConfig {
    /// Weather configuration string (empty = disabled)
    pub weather_config: String,

    /// Visualizer type ("no_viz" = disabled)
    pub visualizer_type: String,

    /// Easter egg type (255 = EGGS_TYPE_UNKNOWN = disabled)
    pub egg_type: u8,

    /// Weather display interval in minutes (e.g., 20 = every 20 minutes)
    pub weather_interval_mins: u32,

    /// Duration to show current weather in seconds
    pub weather_current_duration_secs: u32,

    /// Duration to show forecast weather in seconds
    pub weather_forecast_duration_secs: u32,
}

impl Default for ModeControllerConfig {
    fn default() -> Self {
        Self {
            weather_config: String::new(),
            visualizer_type: "no_viz".to_string(),
            egg_type: 255, // EGGS_TYPE_UNKNOWN
            weather_interval_mins: 20,
            weather_current_duration_secs: 30,
            weather_forecast_duration_secs: 30,
        }
    }
}

/// Display mode controller - determines which mode to display based on state and time
pub struct DisplayModeController {
    config: ModeControllerConfig,
    current_mode: DisplayMode,
    last_mode_change: Instant,
    weather_active: bool,
}

impl DisplayModeController {
    /// Create a new display mode controller
    pub fn new(config: ModeControllerConfig) -> Self {
        Self {
            config,
            current_mode: DisplayMode::Clock,
            last_mode_change: Instant::now(),
            weather_active: false,
        }
    }

    /// Update weather active state
    pub fn set_weather_active(&mut self, active: bool) {
        self.weather_active = active;
    }

    /// Get current display mode
    pub fn current_mode(&self) -> DisplayMode {
        self.current_mode
    }

    /// Determine and update display mode based on player state
    /// Returns true if mode changed
    pub fn update_mode(&mut self, is_playing: bool) -> bool {
        let new_mode = if is_playing {
            let mode = self.determine_playing_mode();
            log::debug!("Player is playing, determined mode: {:?}", mode);
            mode
        } else {
            let mode = self.determine_idle_mode();
            log::debug!("Player is idle, determined mode: {:?}", mode);
            mode
        };

        if new_mode != self.current_mode {
            log::info!("Display mode changed: {:?} -> {:?}", self.current_mode, new_mode);
            self.current_mode = new_mode;
            self.last_mode_change = Instant::now();
            true
        } else {
            false
        }
    }

    /// Determine display mode when player is playing
    fn determine_playing_mode(&self) -> DisplayMode {
        // Priority: EasterEggs > Visualizer > Scrolling
        if self.config.egg_type != 255 { // EGGS_TYPE_UNKNOWN
            DisplayMode::EasterEggs
        } else if self.config.visualizer_type != "no_viz" {
            DisplayMode::Visualizer
        } else {
            DisplayMode::Scrolling
        }
    }

    /// Determine display mode when player is idle (not playing)
    fn determine_idle_mode(&self) -> DisplayMode {
        // If weather not configured or not active, always show clock
        if self.config.weather_config.is_empty() || !self.weather_active {
            return DisplayMode::Clock;
        }

        // Check if we're in a weather display window
        let now = Local::now();
        let minute = now.minute();
        let second = now.second();

        // Check if current minute is a weather display minute
        if self.is_weather_minute(minute) {
            self.determine_weather_mode(second)
        } else {
            DisplayMode::Clock
        }
    }

    /// Check if the given minute should display weather
    fn is_weather_minute(&self, minute: u32) -> bool {
        // Calculate weather display minutes based on interval
        // e.g., interval=20 -> minutes [0, 20, 40]
        // e.g., interval=15 -> minutes [0, 15, 30, 45]
        if self.config.weather_interval_mins == 0 {
            return false;
        }

        minute % self.config.weather_interval_mins == 0
    }

    /// Determine which weather mode to show based on elapsed seconds
    fn determine_weather_mode(&self, second: u32) -> DisplayMode {
        let current_duration = self.config.weather_current_duration_secs;
        let total_duration = current_duration + self.config.weather_forecast_duration_secs;

        if second < current_duration {
            DisplayMode::WeatherCurrent
        } else if second < total_duration {
            DisplayMode::WeatherForecast
        } else {
            // After weather window, return to clock
            DisplayMode::Clock
        }
    }

    /// Get time since last mode change
    pub fn time_since_mode_change(&self) -> std::time::Duration {
        self.last_mode_change.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playing_mode_priority() {
        // Test: EasterEggs has highest priority
        let mut config = ModeControllerConfig::default();
        config.egg_type = 1;
        config.visualizer_type = "bars".to_string();
        let controller = DisplayModeController::new(config);
        assert_eq!(controller.determine_playing_mode(), DisplayMode::EasterEggs);

        // Test: Visualizer is second priority
        let mut config = ModeControllerConfig::default();
        config.egg_type = 255; // disabled
        config.visualizer_type = "bars".to_string();
        let controller = DisplayModeController::new(config);
        assert_eq!(controller.determine_playing_mode(), DisplayMode::Visualizer);

        // Test: Scrolling is default
        let config = ModeControllerConfig::default();
        let controller = DisplayModeController::new(config);
        assert_eq!(controller.determine_playing_mode(), DisplayMode::Scrolling);
    }

    #[test]
    fn test_weather_minute_calculation() {
        let mut config = ModeControllerConfig::default();
        config.weather_interval_mins = 20;
        let controller = DisplayModeController::new(config);

        // Should show weather at minutes 20, 40 (not 0)
        assert!(!controller.is_weather_minute(0));
        assert!(!controller.is_weather_minute(5));
        assert!(controller.is_weather_minute(20));
        assert!(!controller.is_weather_minute(25));
        assert!(controller.is_weather_minute(40));
        assert!(!controller.is_weather_minute(45));
    }

    #[test]
    fn test_weather_mode_timing() {
        let mut config = ModeControllerConfig::default();
        config.weather_current_duration_secs = 30;
        config.weather_forecast_duration_secs = 30;
        let controller = DisplayModeController::new(config);

        // First 30 seconds: current weather
        assert_eq!(controller.determine_weather_mode(0), DisplayMode::WeatherCurrent);
        assert_eq!(controller.determine_weather_mode(29), DisplayMode::WeatherCurrent);

        // Next 30 seconds: forecast
        assert_eq!(controller.determine_weather_mode(30), DisplayMode::WeatherForecast);
        assert_eq!(controller.determine_weather_mode(59), DisplayMode::WeatherForecast);

        // After window: clock
        assert_eq!(controller.determine_weather_mode(60), DisplayMode::Clock);
    }

    #[test]
    fn test_mode_change_detection() {
        let config = ModeControllerConfig::default();
        let mut controller = DisplayModeController::new(config);

        // Initial mode should be Clock
        assert_eq!(controller.current_mode(), DisplayMode::Clock);

        // Update with playing=false should not change (already Clock)
        assert!(!controller.update_mode(false));

        // Update with playing=true should change to Scrolling
        assert!(controller.update_mode(true));
        assert_eq!(controller.current_mode(), DisplayMode::Scrolling);

        // Second update with playing=true should not change
        assert!(!controller.update_mode(true));
    }
}
