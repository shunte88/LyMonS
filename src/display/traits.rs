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

#![allow(dead_code)] // display driver trait abstractions; written for multi-driver support; may be extended

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

    /// 16-bit full colour (Rgb565)
    /// Used by: ST7789
    Rgb565,
}

/// I2C interface parameters
#[derive(Debug, Clone)]
pub struct I2cInfo {
    /// Default I2C address (ADDR pin low / tied to GND)
    pub default_address: u8,
    /// Alternate I2C address (ADDR pin high / tied to VCC)
    pub alt_address: Option<u8>,
    /// Maximum supported I2C clock speed in Hz (typically 400_000 or 1_000_000)
    pub max_speed_hz: u32,
}

/// SPI interface parameters
#[derive(Debug, Clone)]
pub struct SpiInfo {
    /// Maximum supported SPI clock speed in Hz (typically 4_000_000 to 10_000_000)
    pub max_speed_hz: u32,
    /// Data/Command select pin (BCM GPIO numbering on Raspberry Pi)
    /// HIGH = data, LOW = command
    pub dc_pin_desc: &'static str,
    /// Reset pin - active LOW pulse resets the controller
    /// Set to None if reset is handled by power-on or shared line
    pub rst_pin_desc: &'static str,
    /// Whether a hardware RST pin is required (vs optional/tied high)
    pub rst_required: bool,
}

/// Hardware bus interface supported by the display controller
#[derive(Debug, Clone)]
pub enum BusInterface {
    /// I2C only (e.g. SSD1306, SH1106)
    I2c(I2cInfo),
    /// SPI only (e.g. SSD1322, st7789)
    Spi(SpiInfo),
    /// Supports both I2C and SPI - selected by hardware pin strapping (e.g. SSD1309)
    Either { i2c: I2cInfo, spi: SpiInfo },
}

impl BusInterface {
    /// Returns I2C info if this interface supports I2C
    pub fn i2c(&self) -> Option<&I2cInfo> {
        match self {
            Self::I2c(info) => Some(info),
            Self::Either { i2c, .. } => Some(i2c),
            _ => None,
        }
    }

    /// Returns SPI info if this interface supports SPI
    pub fn spi(&self) -> Option<&SpiInfo> {
        match self {
            Self::Spi(info) => Some(info),
            Self::Either { spi, .. } => Some(spi),
            _ => None,
        }
    }

    /// Returns true if this driver can use I2C
    pub fn supports_i2c(&self) -> bool { self.i2c().is_some() }

    /// Returns true if this driver can use SPI
    pub fn supports_spi(&self) -> bool { self.spi().is_some() }
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
    /// Hardware bus interface info (I2C, SPI, or either)
    pub interface: BusInterface,
    /// Whether the display supports hardware rotation
    pub supports_rotation: bool,
    /// Maximum recommended frame rate
    pub max_fps: u32,
    /// Whether the display supports brightness control
    pub supports_brightness: bool,
    /// Whether the display supports inversion
    pub supports_invert: bool,
    /// Canonical lowercase driver identifier (e.g. "st7789", "ssd1322").
    /// Used to derive the correct asset folder path.
    /// Empty string for plugin-loaded drivers.
    pub driver_name: String,
}

/// Minimal hardware abstraction - all display drivers must implement this trait
///
/// This trait defines the core operations that every display driver must support,
/// regardless of the specific hardware implementation. It focuses on the essential
/// operations needed to initialize, configure, and update the display.
pub trait DisplayDriver: Send + Sync {
    /// Downcast support for accessing concrete driver types
    ///
    /// This allows runtime type inspection and downcasting to concrete types.
    /// Useful for extracting emulator state or other driver-specific features.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Mutable downcast support
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

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
    fn set_invert(&mut self, _inverted: bool) -> Result<(), DisplayError> {
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
