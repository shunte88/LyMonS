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
 *  SPI only - no I2C mode.
 *
 *  Typical wiring (Raspberry Pi, BCM pin numbering):
 *
 *    VCC  → 3.3V
 *    GND  → GND
 *    CLK  → GPIO 11 (SCLK, pin 23)
 *    MOSI → GPIO 10 (MOSI, pin 19)
 *    CS   → GPIO 8  (CE0,  pin 24)
 *    DC   → GPIO 24 (pin 18) - Data/Command select (HIGH=data, LOW=command)
 *    RST  → GPIO 25 (pin 22) - Active-low reset (optional, see below)
 *    BL   → GPIO 18 (pin 12) - Backlight PWM (optional; tie HIGH for max)
 *
 *  RST is optional.  Some boards (e.g. Pimoroni Pirate Audio, 240x240 on
 *  /dev/spidev0.1, DC=GPIO 9, BL=GPIO 13) tie RST to 3.3V; leave `rst_pin`
 *  unset for those.  A software reset (SWRESET 0x01) is sent on every init
 *  regardless, after the hardware pulse when an RST line is wired.
 *
 *  Pixel format: Rgb565 big-endian (2 bytes per pixel, R5 G6 B5).
 *  Init sequence is the mipidsi ST7789 model (MIPI DCS):
 *    SoftReset      (0x01)  → 150ms delay
 *    ExitSleepMode  (0x11)
 *    SetAddressMode (0x36)  → MADCTL for the configured rotation
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

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use embedded_hal::digital::OutputPin;
use linux_embedded_hal::{CdevPin, Delay, SpidevDevice};
use linux_embedded_hal::spidev::{SpiModeFlags, SpidevOptions};
use lymons_gpio as gpio;
use mipidsi::{Builder, NoResetPin};
use mipidsi::interface::{Interface, SpiInterface};
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, Orientation, Rotation};

use crate::config::{DisplayConfig, BusConfig};
use crate::display::error::DisplayError;
use crate::display::traits::{
    DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth,
    BusInterface, SpiInfo,
};
use crate::vframebuf::VarFrameBuf;

use log::info;

/// Default SPI clock speed (40 MHz - ST7789 supports up to ~62.5 MHz on Pi)
pub const DEFAULT_SPI_SPEED_HZ: u32 = 40_000_000;
/// Default DC (Data/Command) GPIO pin (BCM)
pub const DEFAULT_DC_PIN: u32 = 24;
/// Default RST (Reset) GPIO pin (BCM)
pub const DEFAULT_RST_PIN: u32 = 25;

/// Bytes mipidsi batches per SPI write.  Matches the spidev default `bufsiz`
/// (4096); a larger value fails with EMSGSIZE unless the module is reloaded.
const SPI_CHUNK: usize = 4096;

/// Pimoroni's power and gamma values for their 240x240 ST7789 panels (Pirate
/// Audio, 1.3" SPI LCD), from `pimoroni/st7789-python`.  Sent after the
/// mipidsi init, which leaves these at controller reset defaults.
const PIMORONI_TUNING: &[(u8, &[u8])] = &[
    (0xB2, &[0x0C, 0x0C, 0x00, 0x33, 0x33]),  // PORCTRL porch setting
    (0xB7, &[0x14]),                          // GCTRL gate voltages
    (0xBB, &[0x37]),                          // VCOMS
    (0xC0, &[0x2C]),                          // LCMCTRL
    (0xC2, &[0x01]),                          // VDVVRHEN
    (0xC3, &[0x12]),                          // VRHS
    (0xC4, &[0x20]),                          // VDVS
    (0xD0, &[0xA4, 0xA1]),                    // PWCTRL1
    (0xC6, &[0x0F]),                          // FRCTRL2 60 Hz
    (0xE0, &[0xD0, 0x04, 0x0D, 0x11, 0x13, 0x2B, 0x3F,
             0x54, 0x4C, 0x18, 0x0D, 0x0B, 0x1F, 0x23]),  // PVGAMCTRL
    (0xE1, &[0xD0, 0x04, 0x0C, 0x11, 0x13, 0x2C, 0x3F,
             0x44, 0x51, 0x2F, 0x1F, 0x1F, 0x20, 0x23]),  // NVGAMCTRL
];

/// Supported ST7789 panel sizes in `(long_axis, short_axis)` form.
pub const SUPPORTED_SIZES: &[(u32, u32)] = &[
    (320, 240),  // layout done, inclusive easter eggs
    (320, 170),  // layout done, inclusive easter eggs
    (280, 240),  // layout done, inclusive easter eggs
    (240, 240),  // layout done, inclusive easter eggs
    (240, 135),  // layout done, eggs mostly good -fix if we get a ticket 
    (160, 80),   // layout done, eggs are a mess - fix if we get a ticket
];

type St7789Display = mipidsi::Display<SpiInterface<'static, SpidevDevice, CdevPin>, ST7789, NoResetPin>;

/// ST7789 display driver (Rgb565, SPI only)
///
/// Full-colour 16-bit per pixel (Rgb565). Uses MIPI DCS commands via mipidsi.
pub struct St7789Driver {
    /// Framebuffer for drawing operations (Rgb565), landscape (long axis first)
    framebuffer: VarFrameBuf<Rgb565>,
    /// Display capabilities
    capabilities: DisplayCapabilities,
    /// Panel handle
    display: St7789Display,
    /// Hardware reset line, held high for the driver's lifetime.  `None` when
    /// RST is strapped high on the board (Pirate Audio).
    _rst: Option<CdevPin>,
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
                gpio_chip: None,
            }),
            brightness: Some(255),
            invert: Some(false),
            rotate_deg: Some(0),
            emulated: Some(false),
            panel_tuning: None,
        }
    }

    /// Normalise and validate ST7789 panel dimensions.
    ///
    /// The ST7789 controller drives a fixed set of TFT panels. The user may
    /// supply `(width, height)` in either orientation; we always return them
    /// with the long axis first (`width >= height`) so the framebuffer is
    /// landscape-canonical and physical orientation is handled by `rotate_deg`.
    pub fn validate_size(w: u32, h: u32) -> Result<(u32, u32), DisplayError> {
        let (long, short) = if w >= h { (w, h) } else { (h, w) };
        if SUPPORTED_SIZES.contains(&(long, short)) {
            Ok((long, short))
        } else {
            let supported = SUPPORTED_SIZES.iter()
                .map(|(a, b)| format!("{}x{}", a, b))
                .collect::<Vec<_>>()
                .join(", ");
            Err(DisplayError::InvalidConfiguration(format!(
                "ST7789: unsupported panel size {}x{}; supported (any orientation): {}",
                w, h, supported
            )))
        }
    }

    /// Offset of the visible panel inside the controller's 240x320 RAM, in
    /// native (portrait, rotation 0) coordinates.  Panels are centered in RAM
    /// except 240x240, which sits at row 0; mipidsi mirrors the offset when
    /// the panel is rotated.
    // ponytail: 160x80 offset is the centered guess, unverified on hardware.
    fn panel_offset(long: u32, short: u32) -> (u16, u16) {
        if long == short {
            return (0, 0);
        }
        (((240 - short) / 2) as u16, ((320 - long) / 2) as u16)
    }

    /// Controller rotation for a landscape framebuffer.  The panel is native
    /// portrait, so non-square panels start a quarter turn round; a further
    /// quarter turn would make them portrait, which the layouts do not cover.
    fn hw_rotation(width: u32, height: u32, rotate_deg: u16) -> Result<Rotation, DisplayError> {
        let base = if width == height { 0 } else { 90 };
        if width != height && !rotate_deg.is_multiple_of(180) {
            return Err(DisplayError::InvalidConfiguration(format!(
                "ST7789: rotate_deg {} would make a {}x{} panel portrait; use 0 or 180",
                rotate_deg, width, height
            )));
        }
        Rotation::try_from_degree(base + rotate_deg as i32)
            .map_err(|_| DisplayError::InvalidRotation(rotate_deg))
    }

    fn make_capabilities(width: u32, height: u32) -> DisplayCapabilities {
        DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Rgb565,
            interface: BusInterface::Spi(SpiInfo {
                max_speed_hz: DEFAULT_SPI_SPEED_HZ,
                dc_pin_desc: "Data/Command select - HIGH=data, LOW=command",
                rst_pin_desc: "Optional active-low reset; SWRESET is sent when absent",
                rst_required: false,
            }),
            supports_rotation: true,
            max_fps: 60,
            supports_brightness: false,
            supports_invert: true,
            driver_name: "st7789".to_string(),
        }
    }

    /// Create a new ST7789 driver using SPI and run the panel init sequence.
    ///
    /// # Arguments
    /// * `spi_bus_path` - Path to SPI device (e.g. "/dev/spidev0.0")
    /// * `dc_pin`       - Data/Command GPIO pin, typically 24
    /// * `rst_pin`      - Reset GPIO pin, typically 25; `None` when RST is
    ///   tied high on the board
    /// * `config`       - Display configuration
    pub fn new_spi(
        spi_bus_path: &str,
        dc_pin: u32,
        rst_pin: Option<u32>,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing ST7789 SPI on {} DC={} RST={:?}",
              spi_bus_path, dc_pin, rst_pin);

        let raw_w = config.width.unwrap_or(320);
        let raw_h = config.height.unwrap_or(170);
        let (width, height) = Self::validate_size(raw_w, raw_h)?;
        if (raw_w, raw_h) != (width, height) {
            info!("ST7789: normalised input {}x{} → {}x{} (long axis first)",
                  raw_w, raw_h, width, height);
        }
        let rotation = Self::hw_rotation(width, height, config.rotate_deg.unwrap_or(0))?;

        let (speed_hz, gpio_chip) = match config.bus.as_ref() {
            Some(BusConfig::Spi { speed_hz, gpio_chip, .. }) =>
                (speed_hz.unwrap_or(DEFAULT_SPI_SPEED_HZ), gpio_chip.clone()),
            _ => (DEFAULT_SPI_SPEED_HZ, None),
        };

        let mut spi = SpidevDevice::open(spi_bus_path)
            .map_err(|e| DisplayError::SpiError(format!("Failed to open {}: {:?}", spi_bus_path, e)))?;
        let options = SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(speed_hz)
            .mode(SpiModeFlags::SPI_MODE_0)
            .build();
        spi.0.configure(&options)
            .map_err(|e| DisplayError::SpiError(format!("Failed to configure SPI: {:?}", e)))?;

        let mut chip = gpio::open_header_chip(gpio_chip.as_deref())
            .map_err(|e| DisplayError::GpioError(e.into_message()))?;
        let dc = gpio::request_output(&mut chip, dc_pin, 0, "lymons-st7789-dc")
            .map_err(|e| DisplayError::GpioError(e.into_message()))?;

        let mut delay = Delay;

        // Hardware reset pulse when wired.  mipidsi is never handed the pin,
        // so it always follows up with SWRESET (0x01): the only reset boards
        // with RST strapped high get.
        let rst = match rst_pin {
            Some(pin) => {
                let mut rst = gpio::request_output(&mut chip, pin, 1, "lymons-st7789-rst")
                    .map_err(|e| DisplayError::GpioError(e.into_message()))?;
                rst.set_low().ok();
                std::thread::sleep(std::time::Duration::from_micros(20));
                rst.set_high().ok();
                std::thread::sleep(std::time::Duration::from_millis(120));
                Some(rst)
            }
            None => {
                info!("ST7789: no rst_pin configured, relying on software reset");
                None
            }
        };

        // SpiInterface borrows its batch buffer; the driver lives for the
        // whole run, so leaking one 4 KiB buffer is the simplest 'static.
        let buffer: &'static mut [u8] = Box::leak(vec![0u8; SPI_CHUNK].into_boxed_slice());
        let di = SpiInterface::new(spi, dc, buffer);

        let (offset_x, offset_y) = Self::panel_offset(width, height);
        let inversion = if config.invert.unwrap_or(false) {
            ColorInversion::Normal
        } else {
            ColorInversion::Inverted
        };

        let mut display = Builder::new(ST7789, di)
            .display_size(height as u16, width as u16) // native portrait
            .display_offset(offset_x, offset_y)
            .invert_colors(inversion)
            .orientation(Orientation::new().rotate(rotation))
            .init(&mut delay)
            .map_err(|e| DisplayError::InitializationFailed(format!("ST7789 init: {:?}", e)))?;

        if config.panel_tuning.unwrap_or(width == 240 && height == 240) {
            info!("ST7789: applying Pimoroni power/gamma tuning (display.panel_tuning)");
            // SAFETY: power and gamma registers only; mipidsi tracks
            // addressing and MADCTL state, which these do not touch.
            let di = unsafe { display.dcs() };
            for (cmd, args) in PIMORONI_TUNING {
                di.send_command(*cmd, args)
                    .map_err(|e| DisplayError::SpiError(format!("ST7789 tuning 0x{:02X}: {:?}", cmd, e)))?;
            }
        }

        Ok(Self {
            framebuffer: VarFrameBuf::new(width, height, Rgb565::BLACK),
            capabilities: Self::make_capabilities(width, height),
            display,
            _rst: rst,
        })
    }
}

impl DisplayDriver for St7789Driver {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn capabilities(&self) -> &DisplayCapabilities { &self.capabilities }

    fn init(&mut self) -> Result<(), DisplayError> {
        // The panel is brought up in new_spi; start from a black screen.
        DisplayDriver::clear(self)
    }

    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        // Backlight is a separate GPIO/PWM line, not a controller command.
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        let (w, h) = (self.capabilities.width as u16, self.capabilities.height as u16);
        self.display
            .set_pixels(0, 0, w - 1, h - 1, self.framebuffer.as_slice().iter().copied())
            .map_err(|e| DisplayError::SpiError(format!("ST7789 flush: {:?}", e)))
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

    fn set_invert(&mut self, _inverted: bool) -> Result<(), DisplayError> {
        // Applied once at init from `config.invert`; mipidsi has no runtime toggle.
        Ok(())
    }

    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        let (w, h) = (self.capabilities.width, self.capabilities.height);
        let rotation = Self::hw_rotation(w, h, degrees)?;
        self.display.set_orientation(Orientation::new().rotate(rotation))
            .map_err(|e| DisplayError::SpiError(format!("ST7789 rotation: {:?}", e)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_offsets_fit_controller_ram() {
        assert_eq!(St7789Driver::panel_offset(240, 240), (0, 0));   // Pirate Audio
        assert_eq!(St7789Driver::panel_offset(320, 170), (35, 0));
        assert_eq!(St7789Driver::panel_offset(240, 135), (52, 40)); // TTGO T-Display
        assert_eq!(St7789Driver::panel_offset(280, 240), (0, 20));
        for &(long, short) in SUPPORTED_SIZES {
            let (x, y) = St7789Driver::panel_offset(long, short);
            assert!(short + x as u32 <= 240 && long + y as u32 <= 320, "{}x{}", long, short);
        }
    }

    #[test]
    fn rotation_keeps_landscape() {
        assert_eq!(St7789Driver::hw_rotation(240, 240, 90).unwrap(), Rotation::Deg90);
        assert_eq!(St7789Driver::hw_rotation(320, 170, 0).unwrap(), Rotation::Deg90);
        assert_eq!(St7789Driver::hw_rotation(320, 170, 180).unwrap(), Rotation::Deg270);
        assert!(St7789Driver::hw_rotation(320, 170, 90).is_err());
    }
}
