/*
 *  display/drivers/ssd1309.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SSD1309 OLED display driver implementation (stub)
 *
 *  The SSD1309 is a 128x64 monochrome OLED controller compatible with
 *  SSD1306 commands. Unlike the SSD1306, the SSD1309 supports both I2C
 *  and SPI interfaces selectable via hardware pin strapping (BS0/BS1 pins).
 *
 *  Typical wiring (Raspberry Pi, BCM pin numbering):
 *
 *  I2C mode (BS1=1, BS0=0):
 *    VCC  → 3.3V
 *    GND  → GND
 *    SCL  → GPIO 3 (SCL, pin 5)
 *    SDA  → GPIO 2 (SDA, pin 3)
 *    ADDR → GND (0x3C) or VCC (0x3D)
 *
 *  SPI mode (BS1=0, BS0=0):
 *    VCC  → 3.3V
 *    GND  → GND
 *    CLK  → GPIO 11 (SCLK, pin 23)
 *    MOSI → GPIO 10 (MOSI, pin 19)
 *    CS   → GPIO 8  (CE0,  pin 24) or GPIO 7 (CE1, pin 26)
 *    DC   → GPIO 24 (pin 18) - Data/Command select
 *    RST  → GPIO 25 (pin 22) - Active-low reset
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

#![allow(dead_code)] // SSD1309 driver helpers; some methods reserved for future display modes

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use crate::config::{DisplayConfig, BusConfig};
use crate::display::error::DisplayError;
use crate::display::traits::{
    DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth,
    BusInterface, I2cInfo, SpiInfo,
};
use crate::vframebuf::VarFrameBuf;

use log::info;

/// Default I2C address (ADDR pin → GND)
pub const DEFAULT_I2C_ADDRESS: u8 = 0x3C;
/// Alternate I2C address (ADDR pin → VCC)
pub const ALT_I2C_ADDRESS: u8 = 0x3D;
/// Default I2C clock speed
pub const DEFAULT_I2C_SPEED_HZ: u32 = 400_000;
/// Default SPI clock speed
pub const DEFAULT_SPI_SPEED_HZ: u32 = 8_000_000;
/// Default DC (Data/Command) GPIO pin (BCM)
pub const DEFAULT_DC_PIN: u32 = 24;
/// Default RST (Reset) GPIO pin (BCM)
pub const DEFAULT_RST_PIN: u32 = 25;

/// SSD1309 display driver wrapper (stub implementation)
///
/// The SSD1309 is functionally compatible with the SSD1306 but supports
/// both I2C and SPI, selected by hardware strapping of BS0/BS1 pins.
pub struct Ssd1309Driver {
    /// Framebuffer for drawing operations
    framebuffer: VarFrameBuf<BinaryColor>,
    /// Display capabilities (includes interface info)
    capabilities: DisplayCapabilities,
}

impl Ssd1309Driver {

    /// Returns default DisplayConfig for SSD1309 over I2C
    pub fn default_i2c_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Ssd1309),
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

    /// Returns default DisplayConfig for SSD1309 over SPI
    pub fn default_spi_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Ssd1309),
            width: Some(128),
            height: Some(64),
            bus: Some(BusConfig::Spi {
                bus: "/dev/spidev0.0".to_string(),
                speed_hz: Some(DEFAULT_SPI_SPEED_HZ),
                dc_pin: DEFAULT_DC_PIN,
                rst_pin: Some(DEFAULT_RST_PIN),
                cs_pin: None,
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
            interface: BusInterface::Either {
                i2c: I2cInfo {
                    default_address: DEFAULT_I2C_ADDRESS,
                    alt_address: Some(ALT_I2C_ADDRESS),
                    max_speed_hz: DEFAULT_I2C_SPEED_HZ,
                },
                spi: SpiInfo {
                    max_speed_hz: DEFAULT_SPI_SPEED_HZ,
                    dc_pin_desc: "Data/Command select - HIGH=data, LOW=command",
                    rst_pin_desc: "Active-low reset - pulse low min 3µs to reset controller",
                    rst_required: false,
                },
            },
            supports_rotation: false,
            max_fps: 30,
            supports_brightness: true,
            supports_invert: true,
        }
    }

    /// Create a new SSD1309 driver using I2C
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
        info!("Initializing SSD1309 I2C on {} at 0x{:02X}", i2c_bus_path, address);

        let width = config.width.unwrap_or(128);
        let height = config.height.unwrap_or(64);

        if width != 128 || height != 64 {
            return Err(DisplayError::InvalidConfiguration(
                format!("SSD1309 only supports 128x64, got {}x{}", width, height)
            ));
        }

        // TODO: open I2C bus and initialize SSD1309 controller registers

        Ok(Self {
            framebuffer: VarFrameBuf::new(width, height, BinaryColor::Off),
            capabilities: Self::make_capabilities(width, height),
        })
    }

    /// Create a new SSD1309 driver using SPI
    ///
    /// # Arguments
    /// * `spi_bus_path` - Path to SPI device (e.g. "/dev/spidev0.0")
    /// * `dc_pin`       - Data/Command GPIO pin (BCM), typically 24
    /// * `rst_pin`      - Reset GPIO pin (BCM), typically 25 (optional)
    /// * `config`       - Display configuration
    pub fn new_spi(
        spi_bus_path: &str,
        dc_pin: u32,
        rst_pin: Option<u32>,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SSD1309 SPI on {} DC={} RST={:?}",
              spi_bus_path, dc_pin, rst_pin);

        let width = config.width.unwrap_or(128);
        let height = config.height.unwrap_or(64);

        if width != 128 || height != 64 {
            return Err(DisplayError::InvalidConfiguration(
                format!("SSD1309 only supports 128x64, got {}x{}", width, height)
            ));
        }

        // TODO: open SPI bus, configure DC/RST GPIO pins, initialize controller

        Ok(Self {
            framebuffer: VarFrameBuf::new(width, height, BinaryColor::Off),
            capabilities: Self::make_capabilities(width, height),
        })
    }
}

impl DisplayDriver for Ssd1309Driver {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn capabilities(&self) -> &DisplayCapabilities { &self.capabilities }

    fn init(&mut self) -> Result<(), DisplayError> {
        // TODO: send SSD1309 initialisation command sequence
        Ok(())
    }

    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        // TODO: send Set Contrast Control command (0x81, value)
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        // TODO: page-addressed write of framebuffer to controller RAM
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

impl DrawableDisplay for Ssd1309Driver {
    type Color = BinaryColor;
}

impl DrawTarget for Ssd1309Driver {
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

impl OriginDimensions for Ssd1309Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
