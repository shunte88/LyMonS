/*
 *  display/drivers/sh1122.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SH1122 OLED display driver implementation (256x64 grayscale, stub)
 *
 *  The SH1122 is a 256x64 grayscale OLED controller supporting 4-bit
 *  (16-level) grayscale. It uses 4-wire SPI only.
 *
 *  Typical wiring (Raspberry Pi, BCM pin numbering):
 *
 *    VCC  → 3.3V
 *    GND  → GND
 *    CLK  → GPIO 11 (SCLK, pin 23)
 *    MOSI → GPIO 10 (MOSI, pin 19)
 *    CS   → GPIO 8  (CE0,  pin 24)  or GPIO 7 (CE1, pin 26)
 *    DC   → GPIO 24 (pin 18) - Data/Command select (HIGH=data, LOW=command)
 *    RST  → GPIO 25 (pin 22) - Active-low reset (pulse low ≥10µs)
 *
 *  Pixel format: 2 pixels per byte, high nibble = left pixel, low nibble =
 *  right pixel. Gray level 0 = off, 15 = full brightness. 128 bytes per row.
 *
 *  Page-based addressing (unlike SSD1322's window addressing):
 *    8 pages × 8 rows = 64 rows total.
 *    Per page: set 0xB0|page, col-high 0x10|(col>>4), col-low (col&0x0F).
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

#![allow(dead_code)] // SH1122 driver helpers; some methods reserved for future display modes

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

/// Default SPI clock speed (SH1122 supports up to 10 MHz)
pub const DEFAULT_SPI_SPEED_HZ: u32 = 10_000_000;
/// Default DC (Data/Command) GPIO pin (BCM)
pub const DEFAULT_DC_PIN: u32 = 24;
/// Default RST (Reset) GPIO pin (BCM)
pub const DEFAULT_RST_PIN: u32 = 25;

/// SH1122 display driver wrapper (256×64 grayscale, stub implementation)
///
/// The SH1122 supports 4-bit grayscale (16 levels) at 256×64 resolution.
/// SPI only — no I2C mode on this controller.  Uses page-based addressing.
pub struct Sh1122Driver {
    framebuffer:  VarFrameBuf<Gray4>,
    capabilities: DisplayCapabilities,
}

impl Sh1122Driver {

    /// Returns default DisplayConfig for SH1122 over SPI.
    pub fn default_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Sh1122),
            width: Some(256),
            height: Some(64),
            bus: Some(BusConfig::Spi {
                bus:      "/dev/spidev0.0".to_string(),
                speed_hz: Some(DEFAULT_SPI_SPEED_HZ),
                dc_pin:   DEFAULT_DC_PIN,
                rst_pin:  Some(DEFAULT_RST_PIN),
                cs_pin:   None,
            }),
            brightness: Some(200),
            invert:     Some(false),
            rotate_deg: Some(0),
            emulated:   Some(false),
        }
    }

    fn make_capabilities(width: u32, height: u32) -> DisplayCapabilities {
        DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Gray4,
            interface: BusInterface::Spi(SpiInfo {
                max_speed_hz:  DEFAULT_SPI_SPEED_HZ,
                dc_pin_desc:   "Data/Command select - HIGH=data, LOW=command",
                rst_pin_desc:  "Active-low reset - pulse low ≥10µs to reset controller",
                rst_required:  true,
            }),
            supports_rotation:   false,
            max_fps:             60,
            supports_brightness: true,
            supports_invert:     false,
        }
    }

    /// Create a new SH1122 driver using SPI.
    ///
    /// # Arguments
    /// * `spi_bus_path` - Path to SPI device (e.g. "/dev/spidev0.0")
    /// * `dc_pin`       - Data/Command GPIO pin (BCM), typically 24
    /// * `rst_pin`      - Reset GPIO pin (BCM), typically 25
    /// * `config`       - Display configuration
    pub fn new_spi(
        spi_bus_path: &str,
        dc_pin:       u32,
        rst_pin:      u32,
        config:       &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SH1122 SPI on {} DC={} RST={}",
              spi_bus_path, dc_pin, rst_pin);

        let width  = config.width.unwrap_or(256);
        let height = config.height.unwrap_or(64);

        if width != 256 || height != 64 {
            return Err(DisplayError::InvalidConfiguration(
                format!("SH1122 supports 256x64, got {}x{}", width, height)
            ));
        }

        // TODO: open SPI bus, configure DC/RST GPIO pins
        // TODO: send SH1122 initialisation sequence:
        //   - Display Off            (0xAE)
        //   - Set Display Clock      (0xD5, 0x50)  divide=1, osc=5
        //   - Set MUX Ratio          (0xA8, 0x3F)  1/64 duty
        //   - Set Display Offset     (0xD3, 0x00)
        //   - Set Display Start Line (0xDC, 0x00)
        //   - Set DC-DC              (0xAD, 0x8B)  internal Vcc, ~7.4V
        //   - Segment Remap          (0xA0)        SEG0=col0; use 0xA1 to mirror
        //   - COM Scan Direction     (0xC0)        top→bottom; use 0xC8 to flip
        //   - Set Contrast           (0x81, 0x80)
        //   - Set Pre-charge Period  (0xD9, 0x22)  phase1=2, phase2=2
        //   - Set VCOM Deselect      (0xDB, 0x35)
        //   - Resume from RAM        (0xA4)
        //   - Normal Display         (0xA6)
        //   - Display On             (0xAF)
        //
        // TODO: write_buffer() should iterate 8 pages:
        //   for page in 0..8 {
        //       send_cmd(0xB0 | page)           // set page address
        //       send_cmd(0x10)                  // col addr high = 0
        //       send_cmd(0x00)                  // col addr low  = 0
        //       send_data(&row_bytes[page])     // 128 bytes = 256 pixels
        //   }

        Ok(Self {
            framebuffer:  VarFrameBuf::new(width, height, Gray4::new(0)),
            capabilities: Self::make_capabilities(width, height),
        })
    }
}

impl DisplayDriver for Sh1122Driver {
    fn as_any(&self)     -> &dyn std::any::Any     { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn capabilities(&self) -> &DisplayCapabilities { &self.capabilities }

    fn init(&mut self) -> Result<(), DisplayError> {
        // TODO: send SH1122 init sequence (see new_spi comments above)
        Ok(())
    }

    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        // TODO: send Set Contrast command (0x81, value)
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        // TODO: page-scan 256-pixel rows via SPI (see new_spi comments above)
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear(Gray4::new(0))
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        // SH1122 Gray4: 2 pixels per byte (4 bits each), 128 bytes × 64 rows = 8192 bytes
        let expected = (self.capabilities.width * self.capabilities.height / 2) as usize;
        if buffer.len() != expected {
            return Err(DisplayError::BufferSizeMismatch { expected, actual: buffer.len() });
        }
        let fb = self.framebuffer.as_mut_slice();
        for (byte_idx, &byte) in buffer.iter().enumerate() {
            let px = byte_idx * 2;
            if px     < fb.len() { fb[px]     = Gray4::new((byte >> 4) & 0x0F); }
            if px + 1 < fb.len() { fb[px + 1] = Gray4::new(byte & 0x0F); }
        }
        self.flush()
    }
}

impl DrawableDisplay for Sh1122Driver {
    type Color = Gray4;
}

impl DrawTarget for Sh1122Driver {
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

impl OriginDimensions for Sh1122Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
