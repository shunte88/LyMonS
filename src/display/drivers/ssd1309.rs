/*
 *  display/drivers/ssd1309.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SSD1309 OLED display driver implementation (stub)
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
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use crate::config::DisplayConfig;
use crate::display::error::DisplayError;
use crate::display::traits::{DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth};
use crate::vframebuf::VarFrameBuf;

use log::info;

/// SSD1309 display driver wrapper (stub implementation)
///
/// This is a stub implementation that provides the correct trait implementations
/// but requires actual hardware-specific code to be filled in for the ssd1309
/// crate version 0.4.0.
pub struct Ssd1309Driver {
    /// Framebuffer for drawing operations
    framebuffer: VarFrameBuf<BinaryColor>,

    /// Display capabilities
    capabilities: DisplayCapabilities,
}

impl Ssd1309Driver {
    /// Create a new SSD1309 driver using I2C
    ///
    /// # Arguments
    ///
    /// * `i2c_bus_path` - Path to I2C device (e.g., "/dev/i2c-1")
    /// * `address` - I2C address (typically 0x3C or 0x3D)
    /// * `config` - Display configuration
    ///
    /// # Returns
    ///
    /// A configured SSD1309 driver or an error
    ///
    /// # Note
    ///
    /// This is currently a stub implementation. The actual ssd1309 crate (v0.4.0)
    /// initialization code needs to be added here once the exact API is confirmed
    /// with hardware testing.
    pub fn new_i2c(
        i2c_bus_path: &str,
        address: u8,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SSD1309 (stub) on {} at address 0x{:02X}", i2c_bus_path, address);

        // Determine display size from config or default to 128x64
        let width = config.width.unwrap_or(128);
        let height = config.height.unwrap_or(64);

        // Validate size
        if width != 128 || height != 64 {
            return Err(DisplayError::InvalidConfiguration(
                format!("SSD1309 only supports 128x64, got {}x{}", width, height)
            ));
        }

        // TODO: Initialize actual ssd1309 hardware driver here
        // For now, this is a stub that creates the framebuffer

        let capabilities = DisplayCapabilities {
            width: 128,
            height: 64,
            color_depth: ColorDepth::Monochrome,
            supports_rotation: false,
            max_fps: 30,
            supports_brightness: true,
            supports_invert: false,
        };

        let framebuffer = VarFrameBuf::new(width, height, BinaryColor::Off);

        info!("SSD1309 (stub) initialized - needs hardware-specific implementation");

        Ok(Self {
            framebuffer,
            capabilities,
        })
    }
}

impl DisplayDriver for Ssd1309Driver {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn capabilities(&self) -> &DisplayCapabilities {
        &self.capabilities
    }

    fn init(&mut self) -> Result<(), DisplayError> {
        // TODO: Initialize hardware
        Ok(())
    }

    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        // TODO: Set hardware brightness/contrast
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        // TODO: Flush framebuffer to hardware
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear(BinaryColor::Off)
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        let expected_size = (self.capabilities.width * self.capabilities.height / 8) as usize;

        if buffer.len() != expected_size {
            return Err(DisplayError::BufferSizeMismatch {
                expected: expected_size,
                actual: buffer.len(),
            });
        }

        // Unpack buffer into framebuffer
        for (byte_idx, &byte) in buffer.iter().enumerate() {
            for bit in 0..8 {
                let pixel_idx = byte_idx * 8 + bit;
                if pixel_idx < self.framebuffer.as_slice().len() {
                    let color = if (byte & (1 << bit)) != 0 {
                        BinaryColor::On
                    } else {
                        BinaryColor::Off
                    };
                    let fb_slice = self.framebuffer.as_mut_slice();
                    fb_slice[pixel_idx] = color;
                }
            }
        }

        self.flush()
    }
}

impl DrawableDisplay for Ssd1309Driver {
    type Color = BinaryColor;
}

impl DrawTarget for Ssd1309Driver {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.framebuffer.draw_iter(pixels)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.framebuffer.clear(color)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        self.framebuffer.fill_contiguous(area, colors)
    }
}

impl OriginDimensions for Ssd1309Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
