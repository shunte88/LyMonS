/*
 *  display/drivers/ssd1322.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SSD1322 OLED display driver implementation (256x64 grayscale, stub)
 *
 *  The SSD1322 is a 256x64 grayscale OLED controller supporting 4-bit
 *  (16-level) grayscale. It uses SPI only - there is no I2C mode.
 *
 *  Typical wiring (Raspberry Pi, BCM pin numbering):
 *
 *    VCC  → 3.3V (or 5V depending on module)
 *    GND  → GND
 *    CLK  → GPIO 11 (SCLK, pin 23)
 *    MOSI → GPIO 10 (MOSI, pin 19)
 *    CS   → GPIO 8  (CE0,  pin 24)  or GPIO 7 (CE1, pin 26)
 *    DC   → GPIO 24 (pin 18) - Data/Command select (HIGH=data, LOW=command)
 *    RST  → GPIO 25 (pin 22) - Active-low reset (pulse low ≥100µs)
 *
 *  The SSD1322 uses a 4-bit-per-pixel RAM format. Each byte holds two
 *  pixels: high nibble = first pixel, low nibble = second pixel.
 *  Gray level 0 = off, 15 = full brightness.
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

#![allow(dead_code)] // SSD1322 driver helpers; some methods reserved for future display modes

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use crate::config::{DisplayConfig, BusConfig};
use crate::display::error::DisplayError;
use crate::display::traits::{
    DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth,
    BusInterface, SpiInfo,
};
use crate::vframebuf::VarFrameBuf;

use log::info;

/// Default SPI clock speed (10 MHz - SSD1322 supports up to 10 MHz)
pub const DEFAULT_SPI_SPEED_HZ: u32 = 10_000_000;
/// Default DC (Data/Command) GPIO pin (BCM)
pub const DEFAULT_DC_PIN: u32 = 24;
/// Default RST (Reset) GPIO pin (BCM)
pub const DEFAULT_RST_PIN: u32 = 25;

/// SSD1322 display driver wrapper (grayscale, stub implementation)
///
/// The SSD1322 supports 4-bit grayscale (16 levels) at 256x64 resolution.
/// SPI only - no I2C mode available on this controller.
pub struct Ssd1322Driver {
    /// Framebuffer for drawing operations (grayscale)
    framebuffer: VarFrameBuf<Gray4>,
    /// Display capabilities
    capabilities: DisplayCapabilities,
}

impl Ssd1322Driver {

    /// Returns default DisplayConfig for SSD1322 over SPI
    pub fn default_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Ssd1322),
            width: Some(256),
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
            color_depth: ColorDepth::Gray4,
            interface: BusInterface::Spi(SpiInfo {
                max_speed_hz: DEFAULT_SPI_SPEED_HZ,
                dc_pin_desc: "Data/Command select - HIGH=data, LOW=command",
                rst_pin_desc: "Active-low reset - pulse low ≥100µs to reset controller",
                rst_required: true,
            }),
            supports_rotation: false,
            max_fps: 60,
            supports_brightness: true,
            supports_invert: false,
        }
    }

    /// Create a new SSD1322 driver using SPI
    ///
    /// # Arguments
    /// * `spi_bus_path` - Path to SPI device (e.g. "/dev/spidev0.0")
    /// * `dc_pin`       - Data/Command GPIO pin (BCM), typically 24
    /// * `rst_pin`      - Reset GPIO pin (BCM), typically 25
    /// * `config`       - Display configuration
    pub fn new_spi(
        spi_bus_path: &str,
        dc_pin: u32,
        rst_pin: u32,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SSD1322 SPI on {} DC={} RST={}",
              spi_bus_path, dc_pin, rst_pin);

        let width = config.width.unwrap_or(256);
        let height = config.height.unwrap_or(64);

        if width != 256 || height != 64 {
            return Err(DisplayError::InvalidConfiguration(
                format!("SSD1322 supports 256x64, got {}x{}", width, height)
            ));
        }

        // TODO: open SPI bus, configure DC/RST GPIO pins
        // TODO: send SSD1322 initialisation sequence:
        //   - Set Command Lock (0xFD)
        //   - Display Off (0xAE)
        //   - Set Column Address (0x15, 0x1C, 0x5B) for 256px across 4-bit segments
        //   - Set Row Address (0x75, 0x00, 0x3F)
        //   - Set Display Clock (0xB3, 0x91)
        //   - Set MUX Ratio (0xCA, 0x3F)
        //   - Set Display Offset (0xA2, 0x00)
        //   - Set Display Start Line (0xA1, 0x00)
        //   - Set Remap & Dual COM (0xA0, 0x14, 0x11)
        //   - Set GPIO (0xB5, 0x00)
        //   - Set Function Selection (0xAB, 0x01)
        //   - Set Contrast (0xC1, 0x9F)
        //   - Set Phase Length (0xB1, 0xE2)
        //   - Select Default Linear Gray Scale (0xB9)
        //   - Set Pre-charge Voltage (0xBB, 0x1F)
        //   - Set Pre-charge Period (0xB6, 0x08)
        //   - Set VCOMH (0xBE, 0x07)
        //   - Set Normal Display Mode (0xA6)
        //   - Display On (0xAF)

        Ok(Self {
            framebuffer: VarFrameBuf::new(width, height, Gray4::new(0)),
            capabilities: Self::make_capabilities(width, height),
        })
    }
}

impl DisplayDriver for Ssd1322Driver {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn capabilities(&self) -> &DisplayCapabilities { &self.capabilities }

    fn init(&mut self) -> Result<(), DisplayError> {
        // TODO: send SSD1322 init sequence (see new_spi comments above)
        Ok(())
    }

    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        // TODO: send Set Contrast Control command (0xC1, value >> 4)
        // SSD1322 contrast range is 0-255 but typical useful range is 0x0F-0xCF
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        // TODO: set column/row window then stream 4-bit pixel data via SPI
        // Each byte = two pixels (high nibble first)
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear(Gray4::new(0))
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        // SSD1322 Gray4: 2 pixels per byte (4 bits each)
        let expected = (self.capabilities.width * self.capabilities.height / 2) as usize;
        if buffer.len() != expected {
            return Err(DisplayError::BufferSizeMismatch { expected, actual: buffer.len() });
        }
        let fb = self.framebuffer.as_mut_slice();
        for (byte_idx, &byte) in buffer.iter().enumerate() {
            let px = byte_idx * 2;
            if px < fb.len()     { fb[px]     = Gray4::new((byte >> 4) & 0x0F); }
            if px + 1 < fb.len() { fb[px + 1] = Gray4::new(byte & 0x0F); }
        }
        self.flush()
    }
}

impl DrawableDisplay for Ssd1322Driver {
    type Color = Gray4;
}

impl DrawTarget for Ssd1322Driver {
    type Color = Gray4;
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

impl OriginDimensions for Ssd1322Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
