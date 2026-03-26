/*
 *  display/drivers/sh1106.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SH1106 OLED display driver implementation (stub)
 *
 *  The SH1106 is a 132x64 monochrome OLED controller. Most modules expose
 *  128 of the 132 columns (the active area is offset by 2 pixels). It uses
 *  a page-addressed write scheme (unlike the SSD1306's column/row window
 *  mode), requiring one SPI/I2C transaction per page row.
 *
 *  Typical wiring (Raspberry Pi, BCM pin numbering):
 *
 *  I2C mode:
 *    VCC  → 3.3V
 *    GND  → GND
 *    SCL  → GPIO 3 (SCL, pin 5)
 *    SDA  → GPIO 2 (SDA, pin 3)
 *    ADDR → GND (0x3C) or VCC (0x3D)
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

#![allow(dead_code)] // SH1106 driver helpers; some methods reserved for future display modes

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use crate::config::{DisplayConfig, BusConfig};
use crate::display::error::DisplayError;
use crate::display::traits::{
    DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth,
    BusInterface, I2cInfo,
};
use crate::vframebuf::VarFrameBuf;

use log::info;

/// Default I2C address (ADDR pin → GND)
pub const DEFAULT_I2C_ADDRESS: u8 = 0x3C;
/// Alternate I2C address (ADDR pin → VCC)
pub const ALT_I2C_ADDRESS: u8 = 0x3D;
/// Maximum supported I2C clock speed
pub const DEFAULT_I2C_SPEED_HZ: u32 = 400_000;
/// Physical column count of SH1106 controller RAM (132 columns)
pub const CONTROLLER_COLUMNS: u32 = 132;
/// Visible column offset (RAM col 2 = display col 0 on most modules)
pub const COLUMN_OFFSET: u8 = 2;

/// SH1106 display driver wrapper (stub implementation)
///
/// The SH1106 controller has 132 columns of RAM but most modules only
/// expose 128, starting at column offset 2. Flush uses page-addressed
/// writes (8 rows of 1 byte per column per page).
pub struct Sh1106Driver {
    /// Framebuffer for drawing operations
    framebuffer: VarFrameBuf<BinaryColor>,
    /// Display capabilities
    capabilities: DisplayCapabilities,
}

impl Sh1106Driver {

    /// Returns default DisplayConfig for SH1106 over I2C
    pub fn default_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Sh1106),
            width: Some(128),
            height: Some(64),
            bus: Some(BusConfig::I2c {
                bus: "/dev/i2c-1".to_string(),
                address: DEFAULT_I2C_ADDRESS,
                speed_hz: Some(DEFAULT_I2C_SPEED_HZ),
            }),
            brightness: Some(200),
            invert: Some(false),
            rotate_deg: Some(0),
            emulated: Some(false),
        }
    }

    fn make_capabilities(width: u32, height: u32) -> DisplayCapabilities {
        DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Monochrome,
            interface: BusInterface::I2c(I2cInfo {
                default_address: DEFAULT_I2C_ADDRESS,
                alt_address: Some(ALT_I2C_ADDRESS),
                max_speed_hz: DEFAULT_I2C_SPEED_HZ,
            }),
            supports_rotation: false,
            max_fps: 30,
            supports_brightness: true,
            supports_invert: true,
            driver_name: "sh1106".to_string(),
        }
    }

    /// Create a new SH1106 driver using I2C
    ///
    /// # Arguments
    /// * `i2c_bus_path` - Path to I2C device (e.g. "/dev/i2c-1")
    /// * `address`      - I2C address: 0x3C (ADDR→GND) or 0x3D (ADDR→VCC)
    /// * `config`       - Display configuration
    pub fn new_i2c(
        i2c_bus_path: &str,
        address: u8,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SH1106 I2C on {} at 0x{:02X}", i2c_bus_path, address);

        let width = config.width.unwrap_or(128);
        let height = config.height.unwrap_or(64);

        // SH1106 supports 128x64 (most modules) or 132x64 (full controller width)
        if (width != 128 && width != 132) || height != 64 {
            return Err(DisplayError::InvalidConfiguration(
                format!("SH1106 supports 128x64 or 132x64, got {}x{}", width, height)
            ));
        }

        // TODO: open I2C bus and send SH1106 init sequence:
        //   - Display Off (0xAE)
        //   - Set Display Clock (0xD5, 0x80)
        //   - Set Multiplex (0xA8, 0x3F)
        //   - Set Display Offset (0xD3, 0x00)
        //   - Set Start Line (0x40)
        //   - Charge Pump (0xAD, 0x8B) - internal VCC
        //   - Set Segment Remap (0xA1)
        //   - Set COM Scan Direction (0xC8)
        //   - Set COM Pins (0xDA, 0x12)
        //   - Set Contrast (0x81, 0xCF)
        //   - Set Pre-charge Period (0xD9, 0x1F)
        //   - Set VCOMH Deselect (0xDB, 0x40)
        //   - All Pixels On disable (0xA4)
        //   - Normal Display (0xA6)
        //   - Display On (0xAF)

        Ok(Self {
            framebuffer: VarFrameBuf::new(width, height, BinaryColor::Off),
            capabilities: Self::make_capabilities(width, height),
        })
    }
}

impl DisplayDriver for Sh1106Driver {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn capabilities(&self) -> &DisplayCapabilities { &self.capabilities }

    fn init(&mut self) -> Result<(), DisplayError> {
        // TODO: send SH1106 init sequence (see new_i2c comments above)
        Ok(())
    }

    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        // TODO: send Set Contrast Control command (0x81, value)
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        // TODO: page-addressed flush - SH1106 requires per-page writes:
        //   for page in 0..8 {
        //     send: Set Page Address (0xB0 | page)
        //     send: Set Column Low  (0x00 | (COLUMN_OFFSET & 0x0F))
        //     send: Set Column High (0x10 | (COLUMN_OFFSET >> 4))
        //     send: 128 bytes of pixel data for this page
        //   }
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear(BinaryColor::Off)
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        let expected = (self.capabilities.width * self.capabilities.height / 8) as usize;
        if buffer.len() != expected {
            return Err(DisplayError::BufferSizeMismatch { expected, actual: buffer.len() });
        }
        let fb = self.framebuffer.as_mut_slice();
        for (byte_idx, &byte) in buffer.iter().enumerate() {
            for bit in 0..8 {
                let px = byte_idx * 8 + bit;
                if px < fb.len() {
                    fb[px] = if (byte & (1 << bit)) != 0 { BinaryColor::On } else { BinaryColor::Off };
                }
            }
        }
        self.flush()
    }
}

impl DrawableDisplay for Sh1106Driver {
    type Color = BinaryColor;
}

impl DrawTarget for Sh1106Driver {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where I: IntoIterator<Item = Pixel<Self::Color>> {
        self.framebuffer.draw_iter(pixels)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.framebuffer.clear(color)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where I: IntoIterator<Item = Self::Color> {
        self.framebuffer.fill_contiguous(area, colors)
    }
}

impl OriginDimensions for Sh1106Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
