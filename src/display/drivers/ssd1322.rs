/*
 *  display/drivers/ssd1322.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SSD1322 OLED display driver implementation (grayscale, stub)
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
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use crate::config::DisplayConfig;
use crate::display::error::DisplayError;
use crate::display::traits::{DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth};
use crate::vframebuf::VarFrameBuf;

use log::info;

/// SSD1322 display driver wrapper (grayscale, stub implementation)
///
/// This is a stub implementation that provides the correct trait implementations
/// but requires actual hardware-specific code to be filled in for the ssd1322
/// crate version 0.3.0.
///
/// The SSD1322 supports 4-bit grayscale (16 levels) at 256x64 resolution.
pub struct Ssd1322Driver {
    /// Framebuffer for drawing operations (grayscale)
    framebuffer: VarFrameBuf<Gray4>,

    /// Display capabilities
    capabilities: DisplayCapabilities,
}

impl Ssd1322Driver {
    /// Create a new SSD1322 driver using SPI
    ///
    /// # Arguments
    ///
    /// * `spi_bus_path` - Path to SPI device (e.g., "/dev/spidev0.0")
    /// * `dc_pin` - Data/Command GPIO pin number
    /// * `rst_pin` - Reset GPIO pin number
    /// * `config` - Display configuration
    ///
    /// # Returns
    ///
    /// A configured SSD1322 driver or an error
    ///
    /// # Note
    ///
    /// This is currently a stub implementation. The actual ssd1322 crate (v0.3.0)
    /// initialization code needs to be added here once the exact API is confirmed
    /// with hardware testing.
    pub fn new_spi(
        spi_bus_path: &str,
        dc_pin: u32,
        rst_pin: u32,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SSD1322 (stub) on {} with DC pin {} and RST pin {}",
              spi_bus_path, dc_pin, rst_pin);

        // Determine display size from config or default to 256x64
        let width = config.width.unwrap_or(256);
        let height = config.height.unwrap_or(64);

        // Validate size (SSD1322 typically supports 256x64)
        if width != 256 || height != 64 {
            return Err(DisplayError::InvalidConfiguration(
                format!("SSD1322 typically supports 256x64, got {}x{}", width, height)
            ));
        }

        // TODO: Initialize actual ssd1322 hardware driver here
        // For now, this is a stub that creates the framebuffer

        let capabilities = DisplayCapabilities {
            width: 256,
            height: 64,
            color_depth: ColorDepth::Gray4,
            supports_rotation: false,
            max_fps: 60, // SPI is faster than I2C
            supports_brightness: true,
            supports_invert: false,
        };

        // Create framebuffer with Gray4 color (4-bit grayscale, 16 levels)
        let framebuffer = VarFrameBuf::new(width, height, Gray4::new(0));

        info!("SSD1322 (stub, grayscale) initialized - needs hardware-specific implementation");

        Ok(Self {
            framebuffer,
            capabilities,
        })
    }
}

impl DisplayDriver for Ssd1322Driver {
    fn capabilities(&self) -> &DisplayCapabilities {
        &self.capabilities
    }

    fn init(&mut self) -> Result<(), DisplayError> {
        // TODO: Initialize hardware
        Ok(())
    }

    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        // TODO: Set hardware brightness/contrast
        // SSD1322 typically uses 0-15 range for contrast
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        // TODO: Flush framebuffer to hardware
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear(Gray4::new(0))
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        // For SSD1322 (Gray4), each pixel is 4 bits, so 2 pixels per byte
        let expected_size = (self.capabilities.width * self.capabilities.height / 2) as usize;

        if buffer.len() != expected_size {
            return Err(DisplayError::BufferSizeMismatch {
                expected: expected_size,
                actual: buffer.len(),
            });
        }

        // Unpack the buffer into the framebuffer (2 pixels per byte)
        let fb_slice = self.framebuffer.as_mut_slice();
        for (byte_idx, &byte) in buffer.iter().enumerate() {
            let pixel_idx = byte_idx * 2;
            if pixel_idx < fb_slice.len() {
                // High nibble is first pixel
                fb_slice[pixel_idx] = Gray4::new((byte >> 4) & 0x0F);
            }
            if pixel_idx + 1 < fb_slice.len() {
                // Low nibble is second pixel
                fb_slice[pixel_idx + 1] = Gray4::new(byte & 0x0F);
            }
        }

        self.flush()
    }
}

impl DrawableDisplay for Ssd1322Driver {
    type Color = Gray4;
}

// Provide direct DrawTarget access on the driver itself
impl DrawTarget for Ssd1322Driver {
    type Color = Gray4;
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

impl OriginDimensions for Ssd1322Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
