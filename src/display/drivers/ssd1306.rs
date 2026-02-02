/*
 *  display/drivers/ssd1306.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SSD1306 OLED display driver implementation
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

use linux_embedded_hal::I2cdev;
use ssd1306::{
    mode::BufferedGraphicsMode,
    prelude::*,
    size::{DisplaySize128x64, DisplaySize128x32},
    I2CDisplayInterface,
    Ssd1306,
};

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use crate::config::DisplayConfig;
use crate::display::error::DisplayError;
use crate::display::traits::{DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth};
use crate::vframebuf::VarFrameBuf;

use log::info;

/// SSD1306 display driver wrapper
pub struct Ssd1306Driver {
    /// The underlying ssd1306 driver
    display: Ssd1306Variants,

    /// Framebuffer for drawing operations
    framebuffer: VarFrameBuf<BinaryColor>,

    /// Display capabilities
    capabilities: DisplayCapabilities,
}

/// Enum to handle different SSD1306 display sizes
enum Ssd1306Variants {
    Size128x64(Ssd1306<I2CInterface<I2cdev>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>),
    Size128x32(Ssd1306<I2CInterface<I2cdev>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>),
}

impl Ssd1306Driver {
    /// Create a new SSD1306 driver using I2C
    ///
    /// # Arguments
    ///
    /// * `i2c_bus_path` - Path to I2C device (e.g., "/dev/i2c-1")
    /// * `address` - I2C address (typically 0x3C or 0x3D)
    /// * `config` - Display configuration
    ///
    /// # Returns
    ///
    /// A configured SSD1306 driver or an error
    pub fn new_i2c(
        i2c_bus_path: &str,
        address: u8,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SSD1306 on {} at address 0x{:02X}", i2c_bus_path, address);

        // Open I2C device
        let i2c = I2cdev::new(i2c_bus_path)
            .map_err(|e| DisplayError::I2cError(format!("Failed to open {}: {}", i2c_bus_path, e)))?;

        // Determine display size from config or default to 128x64
        let width = config.width.unwrap_or(128);
        let height = config.height.unwrap_or(64);

        // Create the appropriate display variant based on size
        let (display, capabilities) = match (width, height) {
            (128, 64) => {
                let interface = I2CDisplayInterface::new(i2c);
                let display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();

                // BufferedGraphicsMode is initialized automatically
                // The ssd1306 crate handles initialization internally

                let caps = DisplayCapabilities {
                    width: 128,
                    height: 64,
                    color_depth: ColorDepth::Monochrome,
                    supports_rotation: true,
                    max_fps: 30, // I2C is slower
                    supports_brightness: true,
                    supports_invert: true,
                };

                (Ssd1306Variants::Size128x64(display), caps)
            }
            (128, 32) => {
                let interface = I2CDisplayInterface::new(i2c);
                let display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();

                // BufferedGraphicsMode is initialized automatically
                // The ssd1306 crate handles initialization internally

                let caps = DisplayCapabilities {
                    width: 128,
                    height: 32,
                    color_depth: ColorDepth::Monochrome,
                    supports_rotation: true,
                    max_fps: 30,
                    supports_brightness: true,
                    supports_invert: true,
                };

                (Ssd1306Variants::Size128x32(display), caps)
            }
            _ => {
                return Err(DisplayError::InvalidConfiguration(
                    format!("Unsupported SSD1306 size: {}x{}", width, height)
                ));
            }
        };

        // Create framebuffer
        let framebuffer = VarFrameBuf::new(width, height, BinaryColor::Off);

        let mut driver = Self {
            display,
            framebuffer,
            capabilities,
        };

        // Apply configuration options
        if let Some(brightness) = config.brightness {
            driver.set_brightness(brightness)?;
        }

        if let Some(invert) = config.invert {
            driver.set_invert(invert)?;
        }

        if let Some(rotation) = config.rotate_deg {
            driver.set_rotation(rotation)?;
        }

        info!("SSD1306 initialized successfully ({}x{})", width, height);

        Ok(driver)
    }

    /// Helper method to convert framebuffer to display format and flush
    fn flush_framebuffer(&mut self) -> Result<(), DisplayError> {
        // Copy framebuffer contents to the ssd1306 display
        match &mut self.display {
            Ssd1306Variants::Size128x64(display) => {
                // Clear the display buffer
                display.clear(BinaryColor::Off)
                    .map_err(|_| DisplayError::Other("Failed to clear display".to_string()))?;

                // Draw framebuffer contents to display
                for y in 0..self.capabilities.height {
                    for x in 0..self.capabilities.width {
                        let point = Point::new(x as i32, y as i32);
                        let idx = (y * self.capabilities.width + x) as usize;

                        if let Some(&color) = self.framebuffer.as_slice().get(idx) {
                            if color == BinaryColor::On {
                                Pixel(point, BinaryColor::On)
                                    .draw(display)
                                    .map_err(|_| DisplayError::DrawingError("Failed to draw pixel".to_string()))?;
                            }
                        }
                    }
                }

                // Flush to hardware
                display.flush()
                    .map_err(|e| DisplayError::Other(format!("Flush failed: {:?}", e)))?;
            }
            Ssd1306Variants::Size128x32(display) => {
                // Similar logic for 128x32
                display.clear(BinaryColor::Off)
                    .map_err(|_| DisplayError::Other("Failed to clear display".to_string()))?;

                for y in 0..self.capabilities.height {
                    for x in 0..self.capabilities.width {
                        let point = Point::new(x as i32, y as i32);
                        let idx = (y * self.capabilities.width + x) as usize;

                        if let Some(&color) = self.framebuffer.as_slice().get(idx) {
                            if color == BinaryColor::On {
                                Pixel(point, BinaryColor::On)
                                    .draw(display)
                                    .map_err(|_| DisplayError::DrawingError("Failed to draw pixel".to_string()))?;
                            }
                        }
                    }
                }

                display.flush()
                    .map_err(|e| DisplayError::Other(format!("Flush failed: {:?}", e)))?;
            }
        }

        Ok(())
    }
}

impl DisplayDriver for Ssd1306Driver {
    fn capabilities(&self) -> &DisplayCapabilities {
        &self.capabilities
    }

    fn init(&mut self) -> Result<(), DisplayError> {
        // Display is already initialized in new() constructor
        // The ssd1306 crate doesn't expose a re-init method in BufferedGraphicsMode
        Ok(())
    }

    fn set_brightness(&mut self, value: u8) -> Result<(), DisplayError> {
        match &mut self.display {
            Ssd1306Variants::Size128x64(display) => {
                // Use the prelude Brightness enum
                let brightness = match value {
                    0..=63 => Brightness::DIMMEST,
                    64..=127 => Brightness::DIM,
                    128..=191 => Brightness::NORMAL,
                    _ => Brightness::BRIGHTEST,
                };
                display.set_brightness(brightness)
                    .map_err(|e| DisplayError::Other(format!("Set brightness failed: {:?}", e)))?;
            }
            Ssd1306Variants::Size128x32(display) => {
                let brightness = match value {
                    0..=63 => Brightness::DIMMEST,
                    64..=127 => Brightness::DIM,
                    128..=191 => Brightness::NORMAL,
                    _ => Brightness::BRIGHTEST,
                };
                display.set_brightness(brightness)
                    .map_err(|e| DisplayError::Other(format!("Set brightness failed: {:?}", e)))?;
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        self.flush_framebuffer()
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear(BinaryColor::Off)
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        // For SSD1306, we need to convert the packed byte format to our framebuffer
        let expected_size = (self.capabilities.width * self.capabilities.height / 8) as usize;

        if buffer.len() != expected_size {
            return Err(DisplayError::BufferSizeMismatch {
                expected: expected_size,
                actual: buffer.len(),
            });
        }

        // Unpack the buffer into the framebuffer
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

    fn set_invert(&mut self, inverted: bool) -> Result<(), DisplayError> {
        match &mut self.display {
            Ssd1306Variants::Size128x64(display) => {
                display.set_display_on(!inverted)
                    .map_err(|e| DisplayError::Other(format!("Set invert failed: {:?}", e)))?;
            }
            Ssd1306Variants::Size128x32(display) => {
                display.set_display_on(!inverted)
                    .map_err(|e| DisplayError::Other(format!("Set invert failed: {:?}", e)))?;
            }
        }
        Ok(())
    }

    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        let rotation = match degrees {
            0 => DisplayRotation::Rotate0,
            90 => DisplayRotation::Rotate90,
            180 => DisplayRotation::Rotate180,
            270 => DisplayRotation::Rotate270,
            _ => return Err(DisplayError::InvalidRotation(degrees)),
        };

        match &mut self.display {
            Ssd1306Variants::Size128x64(display) => {
                display.set_rotation(rotation)
                    .map_err(|e| DisplayError::Other(format!("Set rotation failed: {:?}", e)))?;
            }
            Ssd1306Variants::Size128x32(display) => {
                display.set_rotation(rotation)
                    .map_err(|e| DisplayError::Other(format!("Set rotation failed: {:?}", e)))?;
            }
        }
        Ok(())
    }
}

impl DrawableDisplay for Ssd1306Driver {
    type Color = BinaryColor;
}

// Provide direct DrawTarget access on the driver itself
impl DrawTarget for Ssd1306Driver {
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

impl OriginDimensions for Ssd1306Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
