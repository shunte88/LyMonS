/*
 *  display/mod.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Display subsystem - modular architecture with dynamic driver loading
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

// Core trait definitions
pub mod traits;
pub mod error;
pub mod framebuffer;
pub mod factory;
pub mod color;
pub mod color_proxy;

// Display drivers (conditionally compiled based on features)
#[cfg(any(
    feature = "driver-ssd1306",
    feature = "driver-ssd1309",
    feature = "driver-ssd1322",
    feature = "driver-sh1106"
))]
pub mod drivers;

// Plugin system (conditionally compiled with plugin-system feature)
#[cfg(feature = "plugin-system")]
pub mod plugin;

// Layout system for adaptive UI
pub mod layout;

// Display manager
pub mod manager;

// UI components
pub mod components;

// Field-based layout system
pub mod field;
pub mod page;
pub mod layout_manager;

// Display mode controller
pub mod mode_controller;

// Emulator window (only with emulator feature)
#[cfg(feature = "emulator")]
pub mod emulator_window;

// Emulator display controller (only with emulator feature)
#[cfg(feature = "emulator")]
pub mod emulator_controller;

// Re-exports for convenience
pub use traits::{DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth};
pub use error::{DisplayError, DisplayFactoryError};
pub use framebuffer::FrameBuffer;
pub use factory::{DisplayDriverFactory, BoxedDriver};
pub use layout::{LayoutConfig, LayoutCategory, AssetType, FontSize};
pub use manager::DisplayManager;
pub use field::{Field, FieldType};
pub use page::PageLayout;
pub use layout_manager::LayoutManager;
pub use color::{Color, ColorValue};
pub use mode_controller::{DisplayModeController, ModeControllerConfig};

/// Display mode enum - controls what content is shown on the display
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum DisplayMode {
    Clock,           // Clock mode
    Scrolling,       // Now Playing mode
    Visualizer,      // Visualizations - meters, meters, meters
    EasterEggs,      // Easter Eggs - just for fun eye-candy
    WeatherCurrent,  // Current Weather mode
    WeatherForecast, // Weather Forecast mode
}

// Re-export OledDisplay from the old display module when available
#[cfg(feature = "driver-ssd1306")]
pub use crate::display_old::OledDisplay;

// Re-export driver types when features are enabled
#[cfg(feature = "driver-ssd1306")]
pub use drivers::ssd1306::Ssd1306Driver;

#[cfg(feature = "driver-ssd1309")]
pub use drivers::ssd1309::Ssd1309Driver;

#[cfg(feature = "driver-ssd1322")]
pub use drivers::ssd1322::Ssd1322Driver;

#[cfg(feature = "driver-sh1106")]
pub use drivers::sh1106::Sh1106Driver;
