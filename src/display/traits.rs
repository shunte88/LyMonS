/*
 *  display/traits.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Core trait definitions for display driver abstraction
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
use crate::display::error::DisplayError;

/// Color depth capabilities of different display drivers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    /// Monochrome displays (1-bit per pixel)
    /// Used by: SSD1306, SSD1309, SH1106, SHARP Memory LCD
    Monochrome,

    /// 4-bit grayscale (16 levels)
    /// Used by: SSD1322
    Gray4,
}

/// Display capabilities and metadata
#[derive(Debug, Clone)]
pub struct DisplayCapabilities {
    /// Display width in pixels
    pub width: u32,

    /// Display height in pixels
    pub height: u32,

    /// Color depth (monochrome or grayscale)
    pub color_depth: ColorDepth,

    /// Whether the display supports hardware rotation
    pub supports_rotation: bool,

    /// Maximum recommended frame rate
    pub max_fps: u32,

    /// Whether the display supports brightness control
    pub supports_brightness: bool,

    /// Whether the display supports inversion
    pub supports_invert: bool,
}

/// Minimal hardware abstraction - all display drivers must implement this trait
///
/// This trait defines the core operations that every display driver must support,
/// regardless of the specific hardware implementation. It focuses on the essential
/// operations needed to initialize, configure, and update the display.
pub trait DisplayDriver: Send {
    /// Returns the capabilities of this display
    fn capabilities(&self) -> &DisplayCapabilities;

    /// Returns the display dimensions as (width, height)
    fn dimensions(&self) -> (u32, u32) {
        let caps = self.capabilities();
        (caps.width, caps.height)
    }

    /// Initialize the display hardware
    ///
    /// This should configure the display controller, set up any required
    /// communication protocols, and prepare the display for rendering.
    fn init(&mut self) -> Result<(), DisplayError>;

    /// Set display brightness (0-255)
    ///
    /// Returns an error if the display doesn't support brightness control.
    fn set_brightness(&mut self, value: u8) -> Result<(), DisplayError>;

    /// Flush the current framebuffer to the display hardware
    ///
    /// This transfers the buffered pixel data to the display controller.
    fn flush(&mut self) -> Result<(), DisplayError>;

    /// Clear the display to blank/off state
    fn clear(&mut self) -> Result<(), DisplayError>;

    /// Write a raw buffer to the display
    ///
    /// The format of the buffer depends on the display driver implementation.
    /// For monochrome displays, this is typically packed bits (8 pixels per byte).
    /// For grayscale displays, this may be 4 bits per pixel or other formats.
    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError>;

    /// Set display inversion (if supported)
    ///
    /// When inverted, light pixels become dark and vice versa.
    fn set_invert(&mut self, inverted: bool) -> Result<(), DisplayError> {
        if !self.capabilities().supports_invert {
            return Err(DisplayError::UnsupportedOperation);
        }
        // Default implementation returns error; drivers should override
        Err(DisplayError::UnsupportedOperation)
    }

    /// Set display rotation (if supported)
    ///
    /// Rotation angle should be 0, 90, 180, or 270 degrees.
    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        if !self.capabilities().supports_rotation {
            return Err(DisplayError::UnsupportedOperation);
        }
        if degrees != 0 && degrees != 90 && degrees != 180 && degrees != 270 {
            return Err(DisplayError::InvalidRotation(degrees));
        }
        // Default implementation returns error; drivers should override
        Err(DisplayError::UnsupportedOperation)
    }
}

/// Extended trait for embedded-graphics integration
///
/// This trait provides integration with the embedded-graphics library,
/// allowing the display driver to be used as a DrawTarget for rendering
/// graphics primitives, text, and images.
///
/// Note: This trait doesn't provide direct DrawTarget access because DrawTarget
/// is not dyn compatible. Instead, drivers should implement DrawTarget directly
/// on their internal framebuffer type.
pub trait DrawableDisplay: DisplayDriver {
    /// The color type used by this display
    type Color: PixelColor;
}
