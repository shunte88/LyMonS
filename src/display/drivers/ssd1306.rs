/*
 *  display/drivers/ssd1306.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SSD1306 OLED display driver implementation
 *
 *  The SSD1306 controller supports both I2C and SPI (4-wire) interfaces,
 *  selected by hardware pin strapping. The interface actually used is
 *  chosen at runtime from the BusConfig via new_i2c() / new_spi().
 *
 *  I2C wiring (Raspberry Pi, BCM):  SDA→GPIO2, SCL→GPIO3, ADDR→GND(0x3C)/VCC(0x3D)
 *  SPI wiring (Raspberry Pi, BCM):  SCLK→GPIO11, MOSI→GPIO10, CS→CE0/CE1,
 *                                   DC→GPIO24, RST→GPIO25
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

#![allow(dead_code)] // SSD1306 driver helpers; some methods reserved

use linux_embedded_hal::{I2cdev, SpidevDevice, CdevPin};
use linux_embedded_hal::spidev::{SpidevOptions, SpiModeFlags};
use lymons_gpio::{self as gpio};
use embedded_hal::digital::OutputPin;
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
/// Maximum supported I2C clock speed
pub const DEFAULT_I2C_SPEED_HZ: u32 = 400_000;
/// Default SPI clock speed
pub const DEFAULT_SPI_SPEED_HZ: u32 = 8_000_000;
/// Default DC (Data/Command) GPIO line — BCM 24 on a Raspberry Pi header.
///
/// These are cdev *line offsets* on whichever controller `gpio_chip` selects.
/// On a Pi the header is a single controller whose offsets are the BCM numbers,
/// so 24/25 read as BCM 24/25.  On Orange Pi / Rockchip each bank is its own
/// controller and the offset is bank-relative — see the `lymons-gpio` crate.
pub const DEFAULT_DC_PIN: u32 = 24;
/// Default RST (Reset) GPIO line — BCM 25 on a Raspberry Pi header.
pub const DEFAULT_RST_PIN: u32 = 25;
/// Fallback GPIO character device when no controller is configured or detected.
pub const DEFAULT_GPIO_CHIP: &str = lymons_gpio::DEFAULT_GPIO_CHIP;

/// SSD1306 display driver wrapper
pub struct Ssd1306Driver {
    /// The underlying ssd1306 driver, over whichever bus was selected
    display: Ssd1306Variants,

    /// Framebuffer for drawing operations
    framebuffer: VarFrameBuf<BinaryColor>,

    /// Display capabilities
    capabilities: DisplayCapabilities,

    /// Reset line held high for the lifetime of the driver (SPI only).
    /// Kept alive so the kernel does not release the line back to input.
    _rst: Option<CdevPin>,
}

/// Enum to handle the SSD1306 across both bus types and supported sizes
enum Ssd1306Variants {
    I2c128x64(Ssd1306<I2CInterface<I2cdev>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>),
    I2c128x32(Ssd1306<I2CInterface<I2cdev>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>),
    Spi128x64(Ssd1306<SPIInterface<SpidevDevice, CdevPin>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>),
    Spi128x32(Ssd1306<SPIInterface<SpidevDevice, CdevPin>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>),
}

/// Run an expression against the active display variant, regardless of bus/size.
/// The body is type-checked independently per arm, so the same source works for
/// all four concrete `Ssd1306<..>` types.
macro_rules! with_display {
    ($self:ident, $d:ident => $body:expr) => {
        match &mut $self.display {
            Ssd1306Variants::I2c128x64($d) => $body,
            Ssd1306Variants::I2c128x32($d) => $body,
            Ssd1306Variants::Spi128x64($d) => $body,
            Ssd1306Variants::Spi128x32($d) => $body,
        }
    };
}

impl Ssd1306Driver {

    /// Returns default DisplayConfig for SSD1306 over I2C (128x64)
    pub fn default_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Ssd1306),
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

    /// Returns default DisplayConfig for SSD1306 over SPI (128x64)
    pub fn default_spi_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Ssd1306),
            width: Some(128),
            height: Some(64),
            bus: Some(BusConfig::Spi {
                bus: "/dev/spidev0.0".to_string(),
                speed_hz: Some(DEFAULT_SPI_SPEED_HZ),
                dc_pin: DEFAULT_DC_PIN,
                rst_pin: Some(DEFAULT_RST_PIN),
                cs_pin: None,
                gpio_chip: None,
            }),
            brightness: Some(200),
            invert: Some(false),
            rotate_deg: Some(0),
            emulated: Some(false),
        }
    }

    /// SSD1306 supports both I2C and SPI; advertise both regardless of the
    /// bus currently in use (the active bus is fixed at construction).
    fn either_interface_info() -> BusInterface {
        BusInterface::Either {
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
        }
    }

    fn make_caps(width: u32, height: u32, max_fps: u32) -> DisplayCapabilities {
        DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Monochrome,
            interface: Self::either_interface_info(),
            supports_rotation: true,
            max_fps,
            supports_brightness: true,
            supports_invert: true,
            driver_name: "ssd1306".to_string(),
        }
    }

    /// Create a new SSD1306 driver using I2C
    ///
    /// # Arguments
    ///
    /// * `i2c_bus_path` - Path to I2C device (e.g., "/dev/i2c-1")
    /// * `address` - I2C address (typically 0x3C or 0x3D)
    /// * `config` - Display configuration
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
                (Ssd1306Variants::I2c128x64(display), Self::make_caps(128, 64, 30))
            }
            (128, 32) => {
                let interface = I2CDisplayInterface::new(i2c);
                let display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();
                (Ssd1306Variants::I2c128x32(display), Self::make_caps(128, 32, 30))
            }
            _ => {
                return Err(DisplayError::InvalidConfiguration(
                    format!("Unsupported SSD1306 size: {}x{}", width, height)
                ));
            }
        };

        Self::finish(display, capabilities, None, width, height, config)
    }

    /// Create a new SSD1306 driver using SPI (4-wire)
    ///
    /// # Arguments
    ///
    /// * `spi_bus_path` - Path to SPI device (e.g., "/dev/spidev0.0")
    /// * `dc_pin`       - Data/Command GPIO pin (BCM), typically 24
    /// * `rst_pin`      - Reset GPIO pin (BCM), typically 25 (optional)
    /// * `config`       - Display configuration
    pub fn new_spi(
        spi_bus_path: &str,
        dc_pin: u32,
        rst_pin: Option<u32>,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SSD1306 SPI on {} DC={} RST={:?}",
              spi_bus_path, dc_pin, rst_pin);

        let width = config.width.unwrap_or(128);
        let height = config.height.unwrap_or(64);

        // Open and configure the SPI bus (kernel manages CS via the spidev path)
        let spi_speed = match config.bus.as_ref() {
            Some(BusConfig::Spi { speed_hz, .. }) => speed_hz.unwrap_or(DEFAULT_SPI_SPEED_HZ),
            _ => DEFAULT_SPI_SPEED_HZ,
        };
        let mut spi = SpidevDevice::open(spi_bus_path)
            .map_err(|e| DisplayError::SpiError(format!("Failed to open {}: {:?}", spi_bus_path, e)))?;
        let options = SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(spi_speed)
            .mode(SpiModeFlags::SPI_MODE_0)
            .build();
        spi.0.configure(&options)
            .map_err(|e| DisplayError::SpiError(format!("Failed to configure SPI: {:?}", e)))?;

        // Acquire GPIO lines for DC and (optionally) RST from the header controller
        let gpio_chip = match config.bus.as_ref() {
            Some(BusConfig::Spi { gpio_chip, .. }) => gpio_chip.clone(),
            _ => None,
        };
        let mut chip = gpio::open_header_chip(gpio_chip.as_deref())
            .map_err(|e| DisplayError::GpioError(e.into_message()))?;

        let dc = gpio::request_output(&mut chip, dc_pin, 0, "lymons-ssd1306-dc")
            .map_err(|e| DisplayError::GpioError(e.into_message()))?;

        // Hardware reset pulse (held high afterwards for the driver's lifetime)
        let rst = match rst_pin {
            Some(pin) => {
                let mut rst = gpio::request_output(&mut chip, pin, 1, "lymons-ssd1306-rst")
                    .map_err(|e| DisplayError::GpioError(e.into_message()))?;
                rst.set_high().ok();
                std::thread::sleep(std::time::Duration::from_millis(1));
                rst.set_low().ok();
                std::thread::sleep(std::time::Duration::from_millis(10));
                rst.set_high().ok();
                std::thread::sleep(std::time::Duration::from_millis(10));
                Some(rst)
            }
            None => None,
        };

        let interface = SPIInterface::new(spi, dc);

        let (display, capabilities) = match (width, height) {
            (128, 64) => {
                let display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();
                (Ssd1306Variants::Spi128x64(display), Self::make_caps(128, 64, 60))
            }
            (128, 32) => {
                let display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();
                (Ssd1306Variants::Spi128x32(display), Self::make_caps(128, 32, 60))
            }
            _ => {
                return Err(DisplayError::InvalidConfiguration(
                    format!("Unsupported SSD1306 size: {}x{}", width, height)
                ));
            }
        };

        Self::finish(display, capabilities, rst, width, height, config)
    }


    /// Shared post-construction: build the driver and apply config options.
    fn finish(
        display: Ssd1306Variants,
        capabilities: DisplayCapabilities,
        rst: Option<CdevPin>,
        width: u32,
        height: u32,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        let framebuffer = VarFrameBuf::new(width, height, BinaryColor::Off);

        let mut driver = Self {
            display,
            framebuffer,
            capabilities,
            _rst: rst,
        };

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

    /// Blit the framebuffer into the given ssd1306 display and flush to hardware.
    /// Generic over interface and size so a single code path serves every variant.
    fn blit_and_flush<DI, SIZE>(
        display: &mut Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>,
        framebuffer: &VarFrameBuf<BinaryColor>,
        width: u32,
        height: u32,
    ) -> Result<(), DisplayError>
    where
        DI: WriteOnlyDataCommand,
        SIZE: DisplaySize,
    {
        display.clear(BinaryColor::Off)
            .map_err(|_| DisplayError::Other("Failed to clear display".to_string()))?;

        for y in 0..height {
            for x in 0..width {
                let point = Point::new(x as i32, y as i32);
                let idx = (y * width + x) as usize;
                if let Some(&color) = framebuffer.as_slice().get(idx) {
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
        Ok(())
    }

    /// Convert framebuffer to display format and flush
    fn flush_framebuffer(&mut self) -> Result<(), DisplayError> {
        let (w, h) = (self.capabilities.width, self.capabilities.height);
        match &mut self.display {
            Ssd1306Variants::I2c128x64(d) => Self::blit_and_flush(d, &self.framebuffer, w, h),
            Ssd1306Variants::I2c128x32(d) => Self::blit_and_flush(d, &self.framebuffer, w, h),
            Ssd1306Variants::Spi128x64(d) => Self::blit_and_flush(d, &self.framebuffer, w, h),
            Ssd1306Variants::Spi128x32(d) => Self::blit_and_flush(d, &self.framebuffer, w, h),
        }
    }
}

impl DisplayDriver for Ssd1306Driver {
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
        // Display is already initialized in new() constructor.
        // The ssd1306 crate doesn't expose a re-init method in BufferedGraphicsMode.
        Ok(())
    }

    fn set_brightness(&mut self, value: u8) -> Result<(), DisplayError> {
        let brightness = match value {
            0..=63 => Brightness::DIMMEST,
            64..=127 => Brightness::DIM,
            128..=191 => Brightness::NORMAL,
            _ => Brightness::BRIGHTEST,
        };
        with_display!(self, d =>
            d.set_brightness(brightness)
                .map_err(|e| DisplayError::Other(format!("Set brightness failed: {:?}", e)))?
        );
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
        with_display!(self, d =>
            d.set_display_on(!inverted)
                .map_err(|e| DisplayError::Other(format!("Set invert failed: {:?}", e)))?
        );
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
        with_display!(self, d =>
            d.set_rotation(rotation)
                .map_err(|e| DisplayError::Other(format!("Set rotation failed: {:?}", e)))?
        );
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
