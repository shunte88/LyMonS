/*
 *  display/drivers/sh1107.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SH1107 OLED display driver implementation
 *
 *  The SH1107 is a 128x128 monochrome OLED controller with both I2C and
 *  4-wire SPI interfaces (selected by hardware strapping on the module).
 *  Modules ship in two common panel sizes: 128x128 (1.5"/1.12") and
 *  128x64 (0.96"/1.3").
 *
 *  GDDRAM model
 *  ------------
 *  The controller always carries 128x128 bits of RAM, addressed as 16 pages
 *  of 128 columns.  A page spans 8 rows; bit d0 of a data byte is the top row
 *  of that page (same bit order as the SSD1306/SH1106).  Writes in page
 *  addressing mode (0x20) advance the column pointer, so a frame is pushed
 *  one page at a time: set page, set column, blast `native_columns` bytes.
 *
 *  Unlike the SSD1306 the SH1107 has no hardware 90 degrees rotation — only
 *  segment remap (0xA0/0xA1) and COM scan direction (0xC0/0xC8), which give
 *  0 degrees and 180 degrees.  Rotation is therefore applied here, while the
 *  framebuffer is packed into controller RAM, so all four angles are
 *  available.  This matters for 128x64 modules: many of them are a portrait
 *  64x128 panel mounted sideways, and need `rotate_deg: 90` to read the right
 *  way up.  Try 0 first, then 90/270 if the image is sideways.
 *
 *  Typical wiring (Raspberry Pi, BCM pin numbering):
 *
 *  I2C mode:
 *    VCC  -> 3.3V         SCL -> GPIO 3 (pin 5)
 *    GND  -> GND          SDA -> GPIO 2 (pin 3)
 *    ADDR -> GND (0x3C) or VCC (0x3D)
 *
 *  SPI mode (4-wire):
 *    SCLK -> GPIO 11      MOSI -> GPIO 10     CS -> CE0/CE1
 *    DC   -> GPIO 24      RST  -> GPIO 25
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

#![allow(dead_code)] // SH1107 driver helpers; some methods reserved for future display modes

use linux_embedded_hal::{I2cdev, SpidevDevice, CdevPin};
use linux_embedded_hal::spidev::{SpidevOptions, SpiModeFlags};
use linux_embedded_hal::gpio_cdev::{self, Chip, LineRequestFlags};
use embedded_hal::digital::OutputPin;
use embedded_hal::i2c::I2c as _;
use embedded_hal::spi::SpiDevice as _;

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

use log::{info, debug};

/// Default I2C address (ADDR pin -> GND)
pub const DEFAULT_I2C_ADDRESS: u8 = 0x3C;
/// Alternate I2C address (ADDR pin -> VCC)
pub const ALT_I2C_ADDRESS: u8 = 0x3D;
/// Maximum supported I2C clock speed
pub const DEFAULT_I2C_SPEED_HZ: u32 = 400_000;
/// Default SPI clock speed
pub const DEFAULT_SPI_SPEED_HZ: u32 = 8_000_000;
/// Default DC (Data/Command) GPIO pin (BCM)
pub const DEFAULT_DC_PIN: u32 = 24;
/// Default RST (Reset) GPIO pin (BCM)
pub const DEFAULT_RST_PIN: u32 = 25;
/// Fallback GPIO character device if the header controller can't be detected
/// by label (see `open_header_gpio_chip`).
pub const DEFAULT_GPIO_CHIP: &str = "/dev/gpiochip0";

/// Panel sizes this driver accepts, long axis first.
pub const SUPPORTED_SIZES: [(u32, u32); 2] = [(128, 128), (128, 64)];

/// I2C control byte prefixing a command stream (Co=0, D/C#=0)
const I2C_CTRL_CMD:  u8 = 0x00;
/// I2C control byte prefixing a data stream (Co=0, D/C#=1)
const I2C_CTRL_DATA: u8 = 0x40;

/// Largest data payload sent in one I2C transaction (excluding control byte).
/// A full 128-byte page fits comfortably within the Linux i2c-dev limit.
const I2C_DATA_CHUNK: usize = 128;

// S H 1 1 0 7   C o m m a n d s
const CMD_DISPLAY_OFF:       u8 = 0xAE;
const CMD_DISPLAY_ON:        u8 = 0xAF;
const CMD_SET_CLOCK_DIV:     u8 = 0xD5;
const CMD_MEMORY_MODE_PAGE:  u8 = 0x20; // page addressing (0x21 = vertical)
const CMD_SET_CONTRAST:      u8 = 0x81;
const CMD_DCDC_CONTROL:      u8 = 0xAD;
const CMD_SEG_REMAP_NORMAL:  u8 = 0xA0; // SEG0 -> column 0
const CMD_SEG_REMAP_REVERSE: u8 = 0xA1; // SEG0 -> column 127
const CMD_COM_SCAN_INC:      u8 = 0xC0;
const CMD_COM_SCAN_DEC:      u8 = 0xC8;
const CMD_SET_START_LINE:    u8 = 0xDC; // SH1107 uses DCh (SSD1306 uses 40h)
const CMD_SET_DISPLAY_OFFS:  u8 = 0xD3;
const CMD_SET_PRECHARGE:     u8 = 0xD9;
const CMD_SET_VCOM_DETECT:   u8 = 0xDB;
const CMD_SET_MULTIPLEX:     u8 = 0xA8;
const CMD_ENTIRE_ON_RESUME:  u8 = 0xA4; // follow RAM contents
const CMD_NORMAL_DISPLAY:    u8 = 0xA6;
const CMD_INVERT_DISPLAY:    u8 = 0xA7;
const CMD_SET_PAGE_ADDR:     u8 = 0xB0; // 0xB0 | page (0-15)

/// DC-DC converter control argument: 0x8A = off (module supplies VPP),
/// 0x8B = on (controller's internal charge pump).  Both the Adafruit and
/// luma.oled SH1107 drivers use 0x8A, which suits the common breakout modules.
const DCDC_OFF: u8 = 0x8A;
const DCDC_ON:  u8 = 0x8B;

/// Oscillator frequency / clock divide ratio
const CLOCK_DIV: u8 = 0x51;
/// Pre-charge period
const PRECHARGE: u8 = 0x22;
/// VCOMH deselect level
const VCOM_DESELECT: u8 = 0x35;

/// The bus this driver instance talks over — fixed at construction.
enum Sh1107Bus {
    I2c { dev: I2cdev, address: u8 },
    Spi { dev: SpidevDevice, dc: CdevPin },
}

/// SH1107 display driver
pub struct Sh1107Driver {
    /// Active bus (I2C or 4-wire SPI)
    bus: Sh1107Bus,

    /// Logical framebuffer, one entry per pixel, in display orientation
    framebuffer: VarFrameBuf<BinaryColor>,

    /// Page-packed staging buffer written to controller RAM, reused each flush
    page_buf: Vec<u8>,

    /// Display capabilities
    capabilities: DisplayCapabilities,

    /// Rotation applied while packing the framebuffer into controller RAM
    rotation: u16,

    /// First controller column of the visible area (0 on every module seen so far)
    column_offset: u8,

    /// Reset line held high for the lifetime of the driver (SPI only).
    /// Kept alive so the kernel does not release the line back to input.
    _rst: Option<CdevPin>,
}

impl Sh1107Driver {

    /// Returns default DisplayConfig for SH1107 over I2C (128x128)
    pub fn default_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Sh1107),
            width: Some(128),
            height: Some(128),
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

    /// Returns default DisplayConfig for SH1107 over SPI (128x128)
    pub fn default_spi_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::Sh1107),
            width: Some(128),
            height: Some(128),
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

    /// Validate a requested panel size, returning it long-axis-first.
    ///
    /// SH1107 modules are sold as 128x128 and 128x64; a 128x64 panel mounted
    /// portrait (64x128) is the same panel and normalises to 128x64 — use
    /// `rotate_deg` to orient it.
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
                "SH1107: unsupported panel size {}x{}; supported (any orientation): {}",
                w, h, supported
            )))
        }
    }

    /// SH1107 supports both I2C and SPI; advertise both regardless of the
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
                rst_pin_desc: "Active-low reset - pulse low min 10µs to reset controller",
                rst_required: false,
            },
        }
    }

    fn make_capabilities(width: u32, height: u32, max_fps: u32) -> DisplayCapabilities {
        DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Monochrome,
            interface: Self::either_interface_info(),
            supports_rotation: true,
            max_fps,
            supports_brightness: true,
            supports_invert: true,
            driver_name: "sh1107".to_string(),
        }
    }

    /// Create a new SH1107 driver using I2C
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
        info!("Initializing SH1107 I2C on {} at 0x{:02X}", i2c_bus_path, address);

        let (width, height) = Self::size_from_config(config)?;

        let dev = I2cdev::new(i2c_bus_path)
            .map_err(|e| DisplayError::I2cError(format!("Failed to open {}: {}", i2c_bus_path, e)))?;

        Self::finish(
            Sh1107Bus::I2c { dev, address },
            None,
            width,
            height,
            30,
            config,
        )
    }

    /// Create a new SH1107 driver using SPI (4-wire)
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
        info!("Initializing SH1107 SPI on {} DC={} RST={:?}",
              spi_bus_path, dc_pin, rst_pin);

        let (width, height) = Self::size_from_config(config)?;

        // Open and configure the SPI bus (kernel manages CS via the spidev path)
        let spi_speed = match config.bus.as_ref() {
            Some(BusConfig::Spi { speed_hz, .. }) => speed_hz.unwrap_or(DEFAULT_SPI_SPEED_HZ),
            _ => DEFAULT_SPI_SPEED_HZ,
        };
        let mut dev = SpidevDevice::open(spi_bus_path)
            .map_err(|e| DisplayError::SpiError(format!("Failed to open {}: {:?}", spi_bus_path, e)))?;
        let options = SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(spi_speed)
            .mode(SpiModeFlags::SPI_MODE_0)
            .build();
        dev.0.configure(&options)
            .map_err(|e| DisplayError::SpiError(format!("Failed to configure SPI: {:?}", e)))?;

        // Acquire GPIO lines for DC and (optionally) RST from the header controller
        let mut chip = Self::open_header_gpio_chip()?;
        let dc = Self::request_output(&mut chip, dc_pin, 0, "lymons-sh1107-dc")?;

        // Hardware reset pulse (held high afterwards for the driver's lifetime)
        let rst = match rst_pin {
            Some(pin) => {
                let mut rst = Self::request_output(&mut chip, pin, 1, "lymons-sh1107-rst")?;
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

        Self::finish(
            Sh1107Bus::Spi { dev, dc },
            rst,
            width,
            height,
            60,
            config,
        )
    }

    /// Resolve and validate the panel size from configuration.
    fn size_from_config(config: &DisplayConfig) -> Result<(u32, u32), DisplayError> {
        let raw_w = config.width.unwrap_or(128);
        let raw_h = config.height.unwrap_or(128);
        let (width, height) = Self::validate_size(raw_w, raw_h)?;
        if (raw_w, raw_h) != (width, height) {
            info!("SH1107: normalised input {}x{} → {}x{} (long axis first; use rotate_deg to orient)",
                  raw_w, raw_h, width, height);
        }
        Ok((width, height))
    }

    /// Open the GPIO chip that drives the Raspberry Pi 40-pin header.
    ///
    /// The gpiochip *number* for the header has moved between Pi generations and
    /// kernel versions (Pi 5 was gpiochip4, then gpiochip0 on kernel 6.6+), so we
    /// match on the controller's stable label instead of a hardcoded path.
    /// Falls back to `DEFAULT_GPIO_CHIP` if no known controller is present.
    fn open_header_gpio_chip() -> Result<Chip, DisplayError> {
        // 40-pin header controllers, most specific (newest) first
        const HEADER_LABELS: [&str; 4] = [
            "pinctrl-rp1",     // Pi 5
            "pinctrl-bcm2711", // Pi 4 / CM4
            "pinctrl-bcm2835", // Pi 0/1/2/3 / Zero
            "pinctrl-bcm2708", // very old kernels
        ];

        if let Ok(chips) = gpio_cdev::chips() {
            let mut found: Vec<Chip> = chips.flatten().collect();
            for label in HEADER_LABELS {
                if let Some(pos) = found.iter().position(|c| c.label() == label) {
                    let chip = found.swap_remove(pos);
                    info!("GPIO header controller: {} ({})", label, chip.path().display());
                    return Ok(chip);
                }
            }
        }

        info!("GPIO header controller not detected by label; falling back to {}", DEFAULT_GPIO_CHIP);
        Chip::new(DEFAULT_GPIO_CHIP)
            .map_err(|e| DisplayError::GpioError(format!("Failed to open {}: {:?}", DEFAULT_GPIO_CHIP, e)))
    }

    /// Request a GPIO line as an output with the given default level.
    fn request_output(
        chip: &mut Chip,
        pin: u32,
        default: u8,
        consumer: &str,
    ) -> Result<CdevPin, DisplayError> {
        let line = chip.get_line(pin)
            .map_err(|e| DisplayError::GpioError(format!("GPIO line {}: {:?}", pin, e)))?;
        let handle = line.request(LineRequestFlags::OUTPUT, default, consumer)
            .map_err(|e| DisplayError::GpioError(format!("GPIO request {}: {:?}", pin, e)))?;
        CdevPin::new(handle)
            .map_err(|e| DisplayError::GpioError(format!("GPIO pin {}: {:?}", pin, e)))
    }

    /// Shared post-construction: build the driver, run the init sequence and
    /// apply the configured brightness / inversion.
    fn finish(
        bus: Sh1107Bus,
        rst: Option<CdevPin>,
        width: u32,
        height: u32,
        max_fps: u32,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        let rotation = config.rotate_deg.unwrap_or(0);
        if !matches!(rotation, 0 | 90 | 180 | 270) {
            return Err(DisplayError::InvalidRotation(rotation));
        }

        let mut driver = Self {
            bus,
            framebuffer: VarFrameBuf::new(width, height, BinaryColor::Off),
            page_buf: Vec::new(),
            capabilities: Self::make_capabilities(width, height, max_fps),
            rotation,
            column_offset: 0,
            _rst: rst,
        };
        driver.resize_page_buffer();

        driver.init()?;

        if let Some(brightness) = config.brightness {
            driver.set_brightness(brightness)?;
        }
        if let Some(invert) = config.invert {
            driver.set_invert(invert)?;
        }

        info!("SH1107 initialized successfully ({}x{} rot {}°)", width, height, rotation);
        Ok(driver)
    }

    // B u s   p r i m i t i v e s

    /// Send one or more command bytes.
    fn write_commands(&mut self, cmds: &[u8]) -> Result<(), DisplayError> {
        match &mut self.bus {
            Sh1107Bus::I2c { dev, address } => {
                // Each command byte carries its own control byte so a stream of
                // commands can be sent in a single burst without Co chaining.
                for &c in cmds {
                    dev.write(*address, &[I2C_CTRL_CMD, c])
                        .map_err(|e| DisplayError::I2cError(format!("command 0x{:02X}: {:?}", c, e)))?;
                }
                Ok(())
            }
            Sh1107Bus::Spi { dev, dc } => {
                dc.set_low()
                    .map_err(|e| DisplayError::GpioError(format!("DC low: {:?}", e)))?;
                dev.write(cmds)
                    .map_err(|e| DisplayError::SpiError(format!("command write: {:?}", e)))
            }
        }
    }

    /// Send a block of display data (RAM contents).
    fn write_data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        match &mut self.bus {
            Sh1107Bus::I2c { dev, address } => {
                let mut frame = Vec::with_capacity(I2C_DATA_CHUNK + 1);
                for chunk in data.chunks(I2C_DATA_CHUNK) {
                    frame.clear();
                    frame.push(I2C_CTRL_DATA);
                    frame.extend_from_slice(chunk);
                    dev.write(*address, &frame)
                        .map_err(|e| DisplayError::I2cError(format!("data write: {:?}", e)))?;
                }
                Ok(())
            }
            Sh1107Bus::Spi { dev, dc } => {
                dc.set_high()
                    .map_err(|e| DisplayError::GpioError(format!("DC high: {:?}", e)))?;
                dev.write(data)
                    .map_err(|e| DisplayError::SpiError(format!("data write: {:?}", e)))
            }
        }
    }

    // G e o m e t r y

    /// Controller-space dimensions (columns, rows) after rotation is applied.
    ///
    /// At 0°/180° the logical width maps to controller columns; at 90°/270°
    /// the axes swap, so a 128x64 panel is driven as 64 columns x 128 rows.
    fn native_dims(&self) -> (u32, u32) {
        let (w, h) = (self.capabilities.width, self.capabilities.height);
        match self.rotation {
            90 | 270 => (h, w),
            _        => (w, h),
        }
    }

    /// Number of 8-row pages needed to cover the native height.
    fn native_pages(&self) -> u32 {
        let (_, nh) = self.native_dims();
        nh.div_ceil(8)
    }

    /// Size the page-packing buffer to the current native geometry.
    fn resize_page_buffer(&mut self) {
        let (nw, _) = self.native_dims();
        let len = (nw * self.native_pages()) as usize;
        self.page_buf.resize(len, 0);
    }

    /// Send the geometry-dependent commands (multiplex ratio, display offset,
    /// start line).  Re-sent whenever rotation changes the native height.
    fn apply_geometry(&mut self) -> Result<(), DisplayError> {
        let (_, native_rows) = self.native_dims();
        let multiplex = (native_rows.clamp(1, 128) - 1) as u8;
        self.write_commands(&[
            CMD_SET_MULTIPLEX,   multiplex,
            CMD_SET_DISPLAY_OFFS, 0x00,
            CMD_SET_START_LINE,   0x00,
        ])
    }

    /// Map a logical pixel to controller-space coordinates for the current rotation.
    #[inline]
    fn rotate_point(rotation: u16, x: u32, y: u32, w: u32, h: u32) -> (u32, u32) {
        match rotation {
            90  => (h - 1 - y, x),
            180 => (w - 1 - x, h - 1 - y),
            270 => (y, w - 1 - x),
            _   => (x, y),
        }
    }

    /// Pack the logical framebuffer into the page-oriented staging buffer.
    fn pack_framebuffer(&mut self) {
        let (w, h) = (self.capabilities.width, self.capabilities.height);
        let (nw, _) = self.native_dims();
        let rotation = self.rotation;

        self.page_buf.fill(0);

        let fb = self.framebuffer.as_slice();
        for y in 0..h {
            let row = (y * w) as usize;
            for x in 0..w {
                if fb.get(row + x as usize).copied().unwrap_or(BinaryColor::Off) != BinaryColor::On {
                    continue;
                }
                let (nx, ny) = Self::rotate_point(rotation, x, y, w, h);
                let idx = ((ny / 8) * nw + nx) as usize;
                if let Some(byte) = self.page_buf.get_mut(idx) {
                    *byte |= 1 << (ny % 8);
                }
            }
        }
    }

    /// Push the staging buffer to controller RAM, one page at a time.
    fn write_pages(&mut self) -> Result<(), DisplayError> {
        let (nw, _) = self.native_dims();
        let pages = self.native_pages();
        let col = self.column_offset;

        for page in 0..pages {
            let start = (page * nw) as usize;
            let end = start + nw as usize;
            if end > self.page_buf.len() {
                break;
            }
            self.write_commands(&[
                CMD_SET_PAGE_ADDR | (page as u8 & 0x0F),
                0x00 | (col & 0x0F),
                0x10 | (col >> 4),
            ])?;
            // Copy out so the immutable borrow of page_buf ends before write_data
            let row: Vec<u8> = self.page_buf[start..end].to_vec();
            self.write_data(&row)?;
        }
        Ok(())
    }
}

impl DisplayDriver for Sh1107Driver {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn capabilities(&self) -> &DisplayCapabilities { &self.capabilities }

    fn init(&mut self) -> Result<(), DisplayError> {
        debug!("SH1107: sending init sequence");

        self.write_commands(&[CMD_DISPLAY_OFF])?;
        self.write_commands(&[
            CMD_SET_CLOCK_DIV,    CLOCK_DIV,
            CMD_MEMORY_MODE_PAGE,
            CMD_DCDC_CONTROL,     DCDC_OFF,
            CMD_SEG_REMAP_NORMAL,
            CMD_COM_SCAN_INC,
        ])?;
        self.apply_geometry()?;
        self.write_commands(&[
            CMD_SET_PRECHARGE,   PRECHARGE,
            CMD_SET_VCOM_DETECT, VCOM_DESELECT,
            CMD_ENTIRE_ON_RESUME,
            CMD_NORMAL_DISPLAY,
        ])?;

        // Blank RAM before switching the panel on so no garbage is shown
        self.page_buf.fill(0);
        self.write_pages()?;

        self.write_commands(&[CMD_DISPLAY_ON])?;
        // Panel needs a moment after the charge pump comes up
        std::thread::sleep(std::time::Duration::from_millis(100));
        Ok(())
    }

    fn set_brightness(&mut self, value: u8) -> Result<(), DisplayError> {
        self.write_commands(&[CMD_SET_CONTRAST, value])
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        self.pack_framebuffer();
        self.write_pages()
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

    fn set_invert(&mut self, inverted: bool) -> Result<(), DisplayError> {
        let cmd = if inverted { CMD_INVERT_DISPLAY } else { CMD_NORMAL_DISPLAY };
        self.write_commands(&[cmd])
    }

    /// Rotation is applied in software while packing the framebuffer — the
    /// SH1107 itself only offers 0°/180° via segment remap and COM scan order.
    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        if !matches!(degrees, 0 | 90 | 180 | 270) {
            return Err(DisplayError::InvalidRotation(degrees));
        }
        if degrees == self.rotation {
            return Ok(());
        }
        self.rotation = degrees;
        self.resize_page_buffer();
        // 90°/270° swap the native axes, so the multiplex ratio changes too
        self.apply_geometry()?;
        self.flush()
    }
}

impl DrawableDisplay for Sh1107Driver {
    type Color = BinaryColor;
}

impl DrawTarget for Sh1107Driver {
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

impl OriginDimensions for Sh1107Driver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_size_accepts_both_panels() {
        assert_eq!(Sh1107Driver::validate_size(128, 128).unwrap(), (128, 128));
        assert_eq!(Sh1107Driver::validate_size(128, 64).unwrap(), (128, 64));
        // Portrait mount of the 128x64 panel normalises to long-axis-first
        assert_eq!(Sh1107Driver::validate_size(64, 128).unwrap(), (128, 64));
    }

    #[test]
    fn validate_size_rejects_unknown_panels() {
        assert!(Sh1107Driver::validate_size(128, 32).is_err());
        assert!(Sh1107Driver::validate_size(256, 64).is_err());
    }

    #[test]
    fn rotation_maps_corners() {
        // 128x64 logical panel
        let (w, h) = (128, 64);
        assert_eq!(Sh1107Driver::rotate_point(0, 0, 0, w, h), (0, 0));
        assert_eq!(Sh1107Driver::rotate_point(0, 127, 63, w, h), (127, 63));

        // 90°: logical (0,0) → native top-right of a 64x128 frame
        assert_eq!(Sh1107Driver::rotate_point(90, 0, 0, w, h), (63, 0));
        assert_eq!(Sh1107Driver::rotate_point(90, 127, 63, w, h), (0, 127));

        assert_eq!(Sh1107Driver::rotate_point(180, 0, 0, w, h), (127, 63));
        assert_eq!(Sh1107Driver::rotate_point(270, 0, 0, w, h), (0, 127));
        assert_eq!(Sh1107Driver::rotate_point(270, 127, 63, w, h), (63, 0));
    }
}
