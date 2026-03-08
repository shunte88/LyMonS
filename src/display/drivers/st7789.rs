/*
 *  display/drivers/st7789.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  ST7789 full-colour display driver (320x170, Rgb565, SPI only)
 *
 *  The ST7789V is a 262K-colour TFT LCD controller with a MIPI DCS command set.
 *  Supported variant: 320×170 (TTGO T-Display / Waveshare 1.9" form factor).
 *  SPI only — no I2C mode.
 *
 *  Typical wiring (Raspberry Pi, BCM pin numbering):
 *
 *    VCC  → 3.3V
 *    GND  → GND
 *    CLK  → GPIO 11 (SCLK, pin 23)
 *    MOSI → GPIO 10 (MOSI, pin 19)
 *    CS   → GPIO 8  (CE0,  pin 24)
 *    DC   → GPIO 24 (pin 18) - Data/Command select (HIGH=data, LOW=command)
 *    RST  → GPIO 25 (pin 22) - Active-low reset
 *    BL   → GPIO 18 (pin 12) - Backlight PWM (optional; tie HIGH for max)
 *
 *  Pixel format: Rgb565 big-endian (2 bytes per pixel, R5 G6 B5).
 *  Init sequence follows the mipidsi ST7789 model (MIPI DCS):
 *    ExitSleepMode  (0x11)  → 120ms delay
 *    SetAddressMode (0x36)  → 0x00 (normal orientation)
 *    SetInvertMode  (0x21)  → panel is inverted by default; invert to correct
 *    SetPixelFormat (0x3A)  → 0x55 (Rgb565)
 *    EnterNormalMode(0x13)
 *    SetDisplayOn   (0x29)
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

#![allow(dead_code)] // ST7789 driver helpers; some methods reserved for future display modes

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Rgb565;
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

/// Default SPI clock speed (40 MHz — ST7789 supports up to ~62.5 MHz on Pi)
pub const DEFAULT_SPI_SPEED_HZ: u32 = 40_000_000;
/// Default DC (Data/Command) GPIO pin (BCM)
pub const DEFAULT_DC_PIN: u32 = 24;
/// Default RST (Reset) GPIO pin (BCM)
pub const DEFAULT_RST_PIN: u32 = 25;

/// ST7789 display driver (320×170 Rgb565, SPI only, stub implementation)
///
/// Full-colour 16-bit per pixel (Rgb565) at 320×170 resolution.
/// Uses MIPI DCS commands — no I2C mode.
pub struct St7789Driver {
    /// Framebuffer for drawing operations (Rgb565)
    framebuffer: VarFrameBuf<Rgb565>,
    /// Display capabilities
    capabilities: DisplayCapabilities,
}

impl St7789Driver {

    /// Returns default DisplayConfig for ST7789 over SPI
    pub fn default_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::St7789),
            width: Some(320),
            height: Some(170),
            bus: Some(BusConfig::Spi {
                bus: "/dev/spidev0.0".to_string(),
                speed_hz: Some(DEFAULT_SPI_SPEED_HZ),
                dc_pin: DEFAULT_DC_PIN,
                rst_pin: Some(DEFAULT_RST_PIN),
                cs_pin: None,
            }),
            brightness: Some(255),
            invert: Some(false),
            rotate_deg: Some(0),
            emulated: Some(false),
        }
    }

    fn make_capabilities(width: u32, height: u32) -> DisplayCapabilities {
        DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Rgb565,
            interface: BusInterface::Spi(SpiInfo {
                max_speed_hz: DEFAULT_SPI_SPEED_HZ,
                dc_pin_desc: "Data/Command select - HIGH=data, LOW=command",
                rst_pin_desc: "Active-low reset - pulse low ≥10µs to reset controller",
                rst_required: true,
            }),
            supports_rotation: true,
            max_fps: 60,
            supports_brightness: true,
            supports_invert: true,
        }
    }

    /// Create a new ST7789 driver using SPI
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
        info!("Initializing ST7789 SPI on {} DC={} RST={}",
              spi_bus_path, dc_pin, rst_pin);

        let width  = config.width.unwrap_or(320);
        let height = config.height.unwrap_or(170);

        if width != 320 || height != 170 {
            return Err(DisplayError::InvalidConfiguration(
                format!("ST7789 supports 320x170, got {}x{}", width, height)
            ));
        }

        // TODO: open SPI bus at DEFAULT_SPI_SPEED_HZ, configure DC/RST GPIO pins
        // TODO: send ST7789 initialisation sequence (MIPI DCS):
        //   - Assert RST low ≥10µs, then release high; wait 5ms
        //   - ExitSleepMode  (0x11); wait 120ms
        //   - SetAddressMode (0x36, 0x00)  — no flip, no mirror
        //   - SetInvertMode  (0x21)        — ST7789 panel is normally inverted
        //   - SetPixelFormat (0x3A, 0x55)  — Rgb565
        //   - SetColumnAddr  (0x2A, 0x00, 0x00, 0x01, 0x3F)  — 0..319
        //   - SetRowAddr     (0x2B, 0x00, 0x00, 0x00, 0xA9)  — 0..169
        //   - EnterNormalMode (0x13)
        //   - SetDisplayOn   (0x29)
        //
        // Using mipidsi 0.10.0 as reference (not stored in struct due to generics):
        //   let di = SpiInterface::new(spi, dc_pin, &mut spi_buffer);
        //   let mut display = Builder::new(ST7789, di)
        //       .display_size(320, 170)
        //       .invert_colors(ColorInversion::Inverted)
        //       .init(&mut delay)?;

        Ok(Self {
            framebuffer: VarFrameBuf::new(width, height, Rgb565::BLACK),
            capabilities: Self::make_capabilities(width, height),
        })
    }
}

impl DisplayDriver for St7789Driver {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn capabilities(&self) -> &DisplayCapabilities { &self.capabilities }

    fn init(&mut self) -> Result<(), DisplayError> {
        // TODO: send ST7789 init sequence (see new_spi comments above)
        Ok(())
    }

    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        // TODO: PWM backlight on BL pin (GPIO 18 typical)
        // value 0 = off, 255 = full brightness
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        // TODO: set column/row window (0x2A/0x2B), then MemoryWrite (0x2C)
        // Stream self.framebuffer as big-endian Rgb565 bytes via SPI
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear(Rgb565::BLACK)
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        // ST7789 Rgb565: 2 bytes per pixel, big-endian
        let expected = (self.capabilities.width * self.capabilities.height * 2) as usize;
        if buffer.len() != expected {
            return Err(DisplayError::BufferSizeMismatch { expected, actual: buffer.len() });
        }
        let fb = self.framebuffer.as_mut_slice();
        for (i, chunk) in buffer.chunks_exact(2).enumerate() {
            if i < fb.len() {
                let pixel_u16 = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
                // Rgb565::from_u16 equivalent: unpack big-endian word
                let r = ((pixel_u16 >> 11) & 0x1F) as u8;
                let g = ((pixel_u16 >> 5)  & 0x3F) as u8;
                let b = (pixel_u16          & 0x1F) as u8;
                fb[i] = Rgb565::new(r, g, b);
            }
        }
        self.flush()
    }

    fn set_invert(&mut self, inverted: bool) -> Result<(), DisplayError> {
        // TODO: send INVON (0x21) or INVOFF (0x20) command via SPI
        let _ = inverted;
        Ok(())
    }

    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        if degrees != 0 && degrees != 90 && degrees != 180 && degrees != 270 {
            return Err(DisplayError::InvalidRotation(degrees));
        }
        // TODO: send SetAddressMode (0x36) with appropriate MADCTL byte:
        //   0° = 0x00, 90° = 0x60, 180° = 0xC0, 270° = 0xA0
        Ok(())
    }
}

impl DrawableDisplay for St7789Driver {
    type Color = Rgb565;
}

impl DrawTarget for St7789Driver {
    type Color = Rgb565;
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

impl OriginDimensions for St7789Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
