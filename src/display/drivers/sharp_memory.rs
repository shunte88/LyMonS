/*
 *  display/drivers/sharp_memory.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SHARP Memory LCD driver (LS027B7DH01, 400x240 monochrome)
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

//! SHARP Memory LCD (Memory In Pixel) driver.
//!
//! These panels have nothing in common with the OLED controllers used by the
//! other drivers in this module beyond the fact that they hang off SPI:
//!
//! * There is no controller framebuffer and no addressable RAM.  Each pixel
//!   holds its own state in a one bit SRAM cell, so anything already on the
//!   panel stays there for free until it is overwritten.
//! * There is no command register.  The three mode bits M0 (write line),
//!   M1 (VCOM) and M2 (clear all) travel in band as the first byte of every
//!   transfer, so the panel has no D/C pin and no reset pin.
//! * Addressing is by gate line only, one indexed, with no column address.
//!   A transfer may carry any subset of the 240 gate lines, which makes
//!   partial update the natural mode of operation rather than an extra.
//! * Chip select is active HIGH, and the wire order is LSB first.  Broadcom
//!   and RP1 SPI controllers cannot do LSB first, so the bit order is baked
//!   into the way this driver packs bytes (see `pack_frame`) instead.
//! * A set bit is a WHITE pixel, the opposite of the OLED convention.
//!
//! Two consequences drive the design here.  First, the panel must never sit
//! at one VCOM polarity for long or a DC bias builds up across the liquid
//! crystal and damages it, so the driver keeps a 1 Hz VCOM square wave going
//! even when the picture is frozen.  Second, the frame rate ceiling is 20 Hz
//! and a full 400x240 frame is 12482 bytes, which is very nearly 50 ms of
//! wire time at the 2 MHz clock ceiling, so the driver diffs each gate line
//! against what it last sent and transmits only the lines that changed.
//!
//! Reference: SHARP LS027B7DH01 datasheet, document LCP-2110015A.

use linux_embedded_hal::{SpidevDevice, CdevPin};
use linux_embedded_hal::spidev::{SpidevOptions, SpiModeFlags};
use lymons_gpio::{self as gpio};
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice as _;

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use std::time::{Duration, Instant};

use crate::config::{DisplayConfig, BusConfig};
use crate::display::error::DisplayError;
use crate::display::traits::{
    DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth,
    BusInterface, SpiInfo,
};
use crate::vframebuf::VarFrameBuf;

use log::{info, debug, trace, warn};

/// Panel width in pixels.  Only the 400x240 LS027B7DH01 is supported for now.
pub const WIDTH: u32 = 400;

/// Panel height in pixels, which is also the number of gate lines.
pub const HEIGHT: u32 = 240;

/// Packed bytes per gate line.
pub const BYTES_PER_LINE: usize = (WIDTH as usize) / 8;

/// Panel sizes this driver accepts.
pub const SUPPORTED_SIZES: [(u32, u32); 1] = [(WIDTH, HEIGHT)];

/// Default SPI clock.  The datasheet gives 1 MHz typical and 2 MHz maximum.
pub const DEFAULT_SPI_SPEED_HZ: u32 = 2_000_000;

/// Hard ceiling on the SPI clock, from the datasheet.
pub const MAX_SPI_SPEED_HZ: u32 = 2_000_000;

/// Default GPIO controller, overridable per board.
pub const DEFAULT_GPIO_CHIP: &str = lymons_gpio::DEFAULT_GPIO_CHIP;

// M o d e   b i t s
//
// The panel reads these LSB first, so on the wire M0 arrives first.  This
// driver clocks bytes out MSB first, which puts M0 in bit 7.  Adafruit's
// library quotes the mirrored values (0x01, 0x02, 0x04) because it configures
// the bus for LSB first, which is not an option on a Pi.

/// M0: the bytes that follow are gate line addresses and pixel data.
const BIT_WRITE_LINE: u8 = 0x80;

/// M1: VCOM polarity for this transfer.
const BIT_VCOM: u8 = 0x40;

/// M2: set every pixel on the panel to white and ignore any data that follows.
const BIT_CLEAR_ALL: u8 = 0x20;

// T i m i n g

/// Minimum gap between pixel frames.  fSCS is 1 to 20 Hz, and a full frame is
/// already about 50 ms of wire time at 2 MHz, so 20 Hz is both the datasheet
/// limit and the practical one.
const MIN_FRAME_INTERVAL: Duration = Duration::from_millis(50);

/// Half period of the VCOM square wave, giving fCOM = 1 Hz.  The datasheet
/// allows 0.5 to 10 Hz and asks for the two polarities to be as equal as
/// possible, which is why the phase is derived from the clock rather than
/// toggled per frame.
const VCOM_HALF_PERIOD_MS: u128 = 500;

/// How long the bus may stay quiet before a two byte maintain command is sent
/// purely to hand the current VCOM polarity to the panel.  Half of a half
/// period guarantees the panel sees both polarities every cycle.
const VCOM_MAINTAIN_EVERY: Duration = Duration::from_millis(250);

/// SCS rising edge to first clock.  Datasheet tsSCS is 3 us minimum.
const CS_SETUP: Duration = Duration::from_micros(6);

/// Last clock to SCS falling edge.  Datasheet thSCS is 1 us minimum.
const CS_HOLD: Duration = Duration::from_micros(2);

/// SHARP Memory LCD driver, 400x240 monochrome.
pub struct SharpMemoryDriver {
    /// SPI bus, MSB first, mode 0.
    spi: SpidevDevice,

    /// SCS line driven by this driver, active HIGH.  `None` means the kernel
    /// drives its own chip select with `SPI_CS_HIGH` set instead.
    cs: Option<CdevPin>,

    /// Optional DISP line, held HIGH for the lifetime of the driver.  Kept
    /// alive so the kernel does not release the line back to input.
    _disp: Option<CdevPin>,

    /// Logical framebuffer, one entry per pixel, in display orientation.
    framebuffer: VarFrameBuf<BinaryColor>,

    /// Current frame packed into panel wire order, HEIGHT * BYTES_PER_LINE.
    line_buf: Vec<u8>,

    /// The frame the panel is actually showing, for gate line diffing.
    sent_buf: Vec<u8>,

    /// False until the panel contents are known, which is after the first
    /// clear-all in `init`.
    sent_valid: bool,

    /// Gate lines that differ from `sent_buf`, rebuilt on every flush.
    dirty: Vec<u16>,

    /// Transfer staging buffer, reused so a flush allocates nothing.
    tx_buf: Vec<u8>,

    /// Display capabilities.
    capabilities: DisplayCapabilities,

    /// Rotation applied while packing.  Only 0 and 180 fit a 400x240 panel.
    rotation: u16,

    /// When set, `BinaryColor::On` packs as black and the background as white,
    /// which is the paper look most reflective panels are used for.
    inverted: bool,

    /// Reference instant for the VCOM square wave.
    epoch: Instant,

    /// Last time any command reached the panel.
    last_tx: Instant,

    /// Last time pixel data was pushed, used to hold fSCS under 20 Hz.
    last_frame: Option<Instant>,

    /// Gate lines pushed by the most recent flush, for logging and tests.
    last_lines_sent: usize,
}

impl SharpMemoryDriver {

    /// Default configuration for a SHARP Memory LCD on a Raspberry Pi header.
    ///
    /// `dc_pin` is carried by the shared bus config but is unused: the panel
    /// has no D/C line.  `rst_pin` is the optional DISP enable, and `cs_pin`
    /// is SCS, which must be a free GPIO rather than the spidev node's own
    /// chip select because SCS is active HIGH.
    pub fn default_config() -> DisplayConfig {
        DisplayConfig {
            driver: Some(crate::config::DriverKind::SharpMemory),
            width: Some(WIDTH),
            height: Some(HEIGHT),
            bus: Some(BusConfig::Spi {
                bus: "/dev/spidev0.0".to_string(),
                speed_hz: Some(DEFAULT_SPI_SPEED_HZ),
                dc_pin: 0,
                rst_pin: None,
                cs_pin: Some(6),
                gpio_chip: None,
            }),
            brightness: None,
            // false renders lit content on a dark panel, matching every other
            // driver and the shipped layouts.  Set true for the paper look.
            invert: Some(false),
            rotate_deg: Some(0),
            emulated: Some(false),
        }
    }

    /// Validate a requested panel size.
    pub fn validate_size(w: u32, h: u32) -> Result<(u32, u32), DisplayError> {
        if SUPPORTED_SIZES.contains(&(w, h)) {
            Ok((w, h))
        } else {
            let supported = SUPPORTED_SIZES.iter()
                .map(|(a, b)| format!("{}x{}", a, b))
                .collect::<Vec<_>>()
                .join(", ");
            Err(DisplayError::InvalidConfiguration(format!(
                "SharpMemory: unsupported panel size {}x{}; supported: {}",
                w, h, supported
            )))
        }
    }

    fn spi_interface_info() -> BusInterface {
        BusInterface::Spi(SpiInfo {
            max_speed_hz: MAX_SPI_SPEED_HZ,
            dc_pin_desc: "Unused - SHARP panels carry the mode bits in the data stream",
            rst_pin_desc: "Optional DISP enable - HIGH shows the panel, LOW blanks it (no reset line)",
            rst_required: false,
        })
    }

    fn make_capabilities(width: u32, height: u32) -> DisplayCapabilities {
        DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Monochrome,
            interface: Self::spi_interface_info(),
            // 0 and 180 only; 90 and 270 would need 400 gate lines.
            supports_rotation: true,
            // fSCS ceiling from the datasheet.
            max_fps: 20,
            // Reflective panel with no backlight and no contrast register.
            supports_brightness: false,
            supports_invert: true,
            driver_name: "sharpmemory".to_string(),
        }
    }

    /// Create a driver over SPI.
    ///
    /// # Arguments
    /// * `spi_bus_path` - SPI device node, for example "/dev/spidev0.0"
    /// * `cs_pin`       - GPIO driving SCS, active HIGH.  When `None` the
    ///   kernel chip select is used with `SPI_CS_HIGH` set, which needs
    ///   controller support.
    /// * `disp_pin`     - GPIO driving DISP, held HIGH.  Omit when DISP is
    ///   strapped high on the breakout.
    /// * `config`       - Display configuration
    pub fn new_spi(
        spi_bus_path: &str,
        cs_pin: Option<u32>,
        disp_pin: Option<u32>,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        info!("Initializing SHARP Memory LCD on {} SCS={:?} DISP={:?}",
              spi_bus_path, cs_pin, disp_pin);

        let (width, height) = Self::size_from_config(config)?;

        let requested = match config.bus.as_ref() {
            Some(BusConfig::Spi { speed_hz, .. }) => speed_hz.unwrap_or(DEFAULT_SPI_SPEED_HZ),
            _ => DEFAULT_SPI_SPEED_HZ,
        };
        let spi_speed = if requested > MAX_SPI_SPEED_HZ {
            warn!("SharpMemory: {} Hz exceeds the {} Hz datasheet ceiling, clamping",
                  requested, MAX_SPI_SPEED_HZ);
            MAX_SPI_SPEED_HZ
        } else {
            requested
        };

        // SCS is active HIGH.  A dedicated GPIO is the portable way to get
        // that; falling back to the kernel chip select needs SPI_CS_HIGH,
        // which not every controller honours.
        let mut mode = SpiModeFlags::SPI_MODE_0;
        if cs_pin.is_none() {
            info!("SharpMemory: no cs_pin configured, asking the kernel for an active-high chip select");
            mode |= SpiModeFlags::SPI_CS_HIGH;
        }

        let mut spi = SpidevDevice::open(spi_bus_path)
            .map_err(|e| DisplayError::SpiError(format!("Failed to open {}: {:?}", spi_bus_path, e)))?;
        let options = SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(spi_speed)
            .mode(mode)
            .build();
        spi.0.configure(&options)
            .map_err(|e| DisplayError::SpiError(format!("Failed to configure SPI: {:?}", e)))?;

        // Acquire the GPIO lines this wiring needs, if any.
        let (cs, disp) = if cs_pin.is_some() || disp_pin.is_some() {
            let gpio_chip = match config.bus.as_ref() {
                Some(BusConfig::Spi { gpio_chip, .. }) => gpio_chip.clone(),
                _ => None,
            };
            let mut chip = gpio::open_header_chip(gpio_chip.as_deref())
                .map_err(|e| DisplayError::GpioError(e.into_message()))?;

            let cs = match cs_pin {
                // Idle low: SCS is active high on this panel.
                Some(pin) => Some(gpio::request_output(&mut chip, pin, 0, "lymons-sharp-scs")
                    .map_err(|e| DisplayError::GpioError(e.into_message()))?),
                None => None,
            };
            let disp = match disp_pin {
                Some(pin) => Some(gpio::request_output(&mut chip, pin, 1, "lymons-sharp-disp")
                    .map_err(|e| DisplayError::GpioError(e.into_message()))?),
                None => None,
            };
            (cs, disp)
        } else {
            (None, None)
        };

        Self::finish(spi, cs, disp, width, height, config)
    }

    /// Resolve and validate the panel size from configuration.
    fn size_from_config(config: &DisplayConfig) -> Result<(u32, u32), DisplayError> {
        let raw_w = config.width.unwrap_or(WIDTH);
        let raw_h = config.height.unwrap_or(HEIGHT);
        Self::validate_size(raw_w, raw_h)
    }

    /// Shared post-construction: build the driver and run the init sequence.
    fn finish(
        spi: SpidevDevice,
        cs: Option<CdevPin>,
        disp: Option<CdevPin>,
        width: u32,
        height: u32,
        config: &DisplayConfig,
    ) -> Result<Self, DisplayError> {
        let rotation = config.rotate_deg.unwrap_or(0);
        Self::check_rotation(rotation)?;

        let frame_len = (height as usize) * BYTES_PER_LINE;
        let now = Instant::now();

        let mut driver = Self {
            spi,
            cs,
            _disp: disp,
            framebuffer: VarFrameBuf::new(width, height, BinaryColor::Off),
            line_buf: vec![0u8; frame_len],
            sent_buf: vec![0u8; frame_len],
            sent_valid: false,
            dirty: Vec::with_capacity(height as usize),
            tx_buf: Vec::with_capacity(2 + (height as usize) * (BYTES_PER_LINE + 2)),
            capabilities: Self::make_capabilities(width, height),
            rotation,
            // Polarity is decided before the first pack, so `init` already
            // pushes the frame the user asked for.
            inverted: config.invert.unwrap_or(false),
            epoch: now,
            last_tx: now,
            last_frame: None,
            last_lines_sent: 0,
        };

        driver.init()?;

        info!("SHARP Memory LCD initialized successfully ({}x{} rot {} deg{})",
              width, height, rotation,
              if driver.inverted { ", inverted" } else { "" });
        Ok(driver)
    }

    /// A 400x240 panel has 240 gate lines, so a quarter turn does not fit.
    fn check_rotation(degrees: u16) -> Result<(), DisplayError> {
        match degrees {
            0 | 180 => Ok(()),
            90 | 270 => Err(DisplayError::InvalidConfiguration(format!(
                "SharpMemory: {} deg rotation needs a {}x{} frame, but the panel has {} gate lines",
                degrees, HEIGHT, WIDTH, HEIGHT
            ))),
            _ => Err(DisplayError::InvalidRotation(degrees)),
        }
    }

    // B u s   p r i m i t i v e s

    /// Clock `tx_buf` out with SCS asserted for the whole transfer.
    fn transfer(&mut self) -> Result<(), DisplayError> {
        let Self { spi, cs, tx_buf, .. } = self;

        if let Some(cs) = cs.as_mut() {
            cs.set_high()
                .map_err(|e| DisplayError::GpioError(format!("SCS high: {:?}", e)))?;
            std::thread::sleep(CS_SETUP);
        }

        let result = spi.write(tx_buf)
            .map_err(|e| DisplayError::SpiError(format!("transfer of {} bytes: {:?}", tx_buf.len(), e)));

        if let Some(cs) = cs.as_mut() {
            std::thread::sleep(CS_HOLD);
            // Deasserting matters more than the error does; a failure here
            // will resurface on the next transfer.
            cs.set_low().ok();
        }

        result?;
        self.last_tx = Instant::now();
        Ok(())
    }

    /// Send a bare mode byte plus its trailing dummy byte.
    fn send_mode(&mut self, bits: u8) -> Result<(), DisplayError> {
        self.tx_buf.clear();
        self.tx_buf.push(bits);
        self.tx_buf.push(0x00);
        self.transfer()
    }

    /// VCOM bit for the current instant, derived from the clock so the two
    /// polarities get equal time regardless of how often frames are pushed.
    fn vcom_bit(&self) -> u8 {
        let half_periods = self.epoch.elapsed().as_millis() / VCOM_HALF_PERIOD_MS;
        if half_periods % 2 == 1 { BIT_VCOM } else { 0 }
    }

    /// Hand the current VCOM polarity to the panel if the bus has gone quiet.
    ///
    /// EXTMODE is tied low on the common breakouts, so the panel only learns
    /// the polarity from the mode byte of whatever command it last received.
    /// Leaving one polarity applied puts a DC bias across the liquid crystal
    /// and damages it, so an idle panel still gets two bytes several times a
    /// second.
    fn maintain_vcom(&mut self) -> Result<(), DisplayError> {
        if self.last_tx.elapsed() < VCOM_MAINTAIN_EVERY {
            return Ok(());
        }
        let vcom = self.vcom_bit();
        self.send_mode(vcom)
    }

    // F r a m e   h a n d l i n g

    /// Rebuild `line_buf` from the logical framebuffer.
    fn pack_framebuffer(&mut self) {
        let Self { framebuffer, line_buf, rotation, inverted, .. } = self;
        pack_frame(framebuffer.as_slice(), *rotation, *inverted, line_buf);
    }

    /// Push every gate line in `dirty` in a single transfer.
    fn push_lines(&mut self) -> Result<(), DisplayError> {
        let vcom = self.vcom_bit();
        {
            let Self { tx_buf, dirty, line_buf, .. } = self;
            build_line_frame(BIT_WRITE_LINE | vcom, dirty, line_buf, tx_buf);
        }
        self.transfer()?;

        {
            let Self { dirty, line_buf, sent_buf, .. } = self;
            for &line in dirty.iter() {
                let at = line as usize * BYTES_PER_LINE;
                sent_buf[at..at + BYTES_PER_LINE].copy_from_slice(&line_buf[at..at + BYTES_PER_LINE]);
            }
        }

        self.sent_valid = true;
        self.last_frame = Some(Instant::now());
        self.last_lines_sent = self.dirty.len();
        trace!("SharpMemory: pushed {} of {} gate lines ({} bytes)",
               self.last_lines_sent, HEIGHT, self.tx_buf.len());
        Ok(())
    }

    /// Pack, diff and push.
    ///
    /// `force` skips the frame rate gate; it is used for init, clear and the
    /// full repacks that follow an invert or rotation change.
    fn flush_inner(&mut self, force: bool) -> Result<(), DisplayError> {
        self.pack_framebuffer();

        {
            let Self { line_buf, sent_buf, dirty, sent_valid, .. } = self;
            dirty.clear();
            if *sent_valid {
                for line in 0..HEIGHT as usize {
                    let at = line * BYTES_PER_LINE;
                    if line_buf[at..at + BYTES_PER_LINE] != sent_buf[at..at + BYTES_PER_LINE] {
                        dirty.push(line as u16);
                    }
                }
            } else {
                dirty.extend(0..HEIGHT as u16);
            }
        }

        // Nothing changed, so the pixels already hold the right state and the
        // only thing the panel still needs is its VCOM square wave.
        if self.dirty.is_empty() {
            self.last_lines_sent = 0;
            return self.maintain_vcom();
        }

        // Hold fSCS under 20 Hz.  Skipping loses nothing: `sent_buf` is left
        // alone, so the next flush rediscovers the same dirty lines.
        if !force && self.last_frame.is_some_and(|t| t.elapsed() < MIN_FRAME_INTERVAL) {
            self.last_lines_sent = 0;
            return self.maintain_vcom();
        }

        self.push_lines()
    }

    /// Gate lines transmitted by the most recent flush.
    pub fn last_lines_sent(&self) -> usize {
        self.last_lines_sent
    }
}

/// Pack a logical framebuffer into panel wire order.
///
/// The panel takes the leftmost pixel of a byte first.  This driver clocks
/// bytes out MSB first, so pixel x of a byte goes into bit `7 - (x % 8)`.
/// A set bit is white, so `BinaryColor::On` packs as 1 unless `invert` is set,
/// which gives dark content on a light background.
fn pack_frame(fb: &[BinaryColor], rotation: u16, invert: bool, out: &mut [u8]) {
    let w = WIDTH as usize;
    let h = HEIGHT as usize;
    let flip = rotation == 180;

    for y in 0..h {
        let src_y = if flip { h - 1 - y } else { y };
        let row = &fb[src_y * w..][..w];
        let line = &mut out[y * BYTES_PER_LINE..][..BYTES_PER_LINE];
        for (bx, byte) in line.iter_mut().enumerate() {
            let mut packed = 0u8;
            for bit in 0..8 {
                let x = bx * 8 + bit;
                let src_x = if flip { w - 1 - x } else { x };
                if (row[src_x] == BinaryColor::On) != invert {
                    packed |= 0x80 >> bit;
                }
            }
            *byte = packed;
        }
    }
}

/// Build a multiple line update transfer.
///
/// The layout is one mode byte, then per line a gate address, its packed
/// pixels and eight dummy clocks, then eight more dummy clocks to close the
/// last line.  Gate lines are one indexed and the panel wants AG0 first, so
/// the address byte is bit reversed for an MSB first bus.
fn build_line_frame(mode: u8, lines: &[u16], line_buf: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(2 + lines.len() * (BYTES_PER_LINE + 2));
    out.push(mode);
    for &line in lines {
        out.push(gate_address(line));
        let at = line as usize * BYTES_PER_LINE;
        out.extend_from_slice(&line_buf[at..at + BYTES_PER_LINE]);
        out.push(0x00);
    }
    out.push(0x00);
}

/// Wire form of a zero indexed gate line number.
#[inline]
fn gate_address(line: u16) -> u8 {
    ((line + 1) as u8).reverse_bits()
}

impl DisplayDriver for SharpMemoryDriver {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn capabilities(&self) -> &DisplayCapabilities { &self.capabilities }

    /// Bring the panel up.
    ///
    /// Clear-all sets every pixel white in two bytes, which is far cheaper
    /// than writing a blank frame and leaves the driver knowing exactly what
    /// is on the glass, so the first real flush can already be a partial one.
    fn init(&mut self) -> Result<(), DisplayError> {
        debug!("SharpMemory: clearing panel");

        let vcom = self.vcom_bit();
        self.send_mode(BIT_CLEAR_ALL | vcom)?;

        self.sent_buf.fill(0xFF);
        self.sent_valid = true;
        self.last_frame = Some(Instant::now());

        // Bring the glass in line with the blank framebuffer.
        self.flush_inner(true)
    }

    /// SHARP panels are reflective, with no backlight and no contrast
    /// register, so this is a no-op.  `supports_brightness` is false.
    fn set_brightness(&mut self, _value: u8) -> Result<(), DisplayError> {
        debug!("SharpMemory: no brightness control on a reflective panel, ignoring");
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        self.flush_inner(false)
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear(BinaryColor::Off)
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush_inner(true)
    }

    /// Accepts the packed form produced by `FrameBuffer::to_packed_bytes`,
    /// which is 8 pixels per byte in raster order, LSB first.  That is not the
    /// panel's own bit order; `pack_frame` handles the conversion on flush.
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

    /// There is no inversion register, so the frame is repacked and every
    /// gate line is rewritten.
    fn set_invert(&mut self, inverted: bool) -> Result<(), DisplayError> {
        if inverted == self.inverted {
            return Ok(());
        }
        self.inverted = inverted;
        self.flush_inner(true)
    }

    /// Rotation is applied in software while packing.  Only 0 and 180 are
    /// possible: a quarter turn would need 400 gate lines and the panel has
    /// 240.
    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        Self::check_rotation(degrees)?;
        if degrees == self.rotation {
            return Ok(());
        }
        self.rotation = degrees;
        self.flush_inner(true)
    }
}

impl DrawableDisplay for SharpMemoryDriver {
    type Color = BinaryColor;
}

impl DrawTarget for SharpMemoryDriver {
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

impl OriginDimensions for SharpMemoryDriver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_fb() -> Vec<BinaryColor> {
        vec![BinaryColor::Off; (WIDTH * HEIGHT) as usize]
    }

    #[test]
    fn validate_size_accepts_only_the_400x240_panel() {
        assert_eq!(SharpMemoryDriver::validate_size(400, 240).unwrap(), (400, 240));
        // The 144x168 panel is a separate part and is not wired up yet.
        assert!(SharpMemoryDriver::validate_size(144, 168).is_err());
        assert!(SharpMemoryDriver::validate_size(240, 400).is_err());
    }

    #[test]
    fn rotation_is_limited_to_the_half_turns_that_fit() {
        assert!(SharpMemoryDriver::check_rotation(0).is_ok());
        assert!(SharpMemoryDriver::check_rotation(180).is_ok());
        assert!(SharpMemoryDriver::check_rotation(90).is_err());
        assert!(SharpMemoryDriver::check_rotation(270).is_err());
        assert!(matches!(SharpMemoryDriver::check_rotation(45),
                         Err(DisplayError::InvalidRotation(45))));
    }

    #[test]
    fn gate_addresses_are_one_indexed_and_bit_reversed() {
        // Line 0 is gate line 1, and AG0 must go out first.
        assert_eq!(gate_address(0), 0x80);
        assert_eq!(gate_address(1), 0x40);
        assert_eq!(gate_address(2), 0xC0);
        // Last gate line, 240 = 0b1111_0000, reversed is 0b0000_1111.
        assert_eq!(gate_address(HEIGHT as u16 - 1), 0x0F);
    }

    #[test]
    fn leftmost_pixel_lands_in_the_first_bit_clocked_out() {
        let mut fb = blank_fb();
        fb[0] = BinaryColor::On;                       // x = 0
        fb[7] = BinaryColor::On;                       // x = 7
        fb[(WIDTH - 1) as usize] = BinaryColor::On;    // x = 399

        let mut out = vec![0u8; (HEIGHT as usize) * BYTES_PER_LINE];
        pack_frame(&fb, 0, false, &mut out);

        assert_eq!(out[0], 0b1000_0001);
        assert_eq!(out[BYTES_PER_LINE - 1], 0b0000_0001);
        // Nothing bled into the next gate line.
        assert!(out[BYTES_PER_LINE..2 * BYTES_PER_LINE].iter().all(|&b| b == 0));
    }

    #[test]
    fn set_pixels_are_white_and_invert_flips_the_whole_field() {
        let mut fb = blank_fb();
        fb[0] = BinaryColor::On;

        let mut normal = vec![0u8; (HEIGHT as usize) * BYTES_PER_LINE];
        pack_frame(&fb, 0, false, &mut normal);
        assert_eq!(normal[0], 0x80);
        assert_eq!(normal[1], 0x00);

        let mut inverted = vec![0u8; (HEIGHT as usize) * BYTES_PER_LINE];
        pack_frame(&fb, 0, true, &mut inverted);
        assert_eq!(inverted[0], 0x7F);
        assert_eq!(inverted[1], 0xFF);
    }

    #[test]
    fn half_turn_maps_the_first_pixel_to_the_last() {
        let mut fb = blank_fb();
        fb[0] = BinaryColor::On;

        let mut out = vec![0u8; (HEIGHT as usize) * BYTES_PER_LINE];
        pack_frame(&fb, 180, false, &mut out);

        // Logical (0,0) becomes (399, 239): last gate line, last bit out.
        let last = (HEIGHT as usize - 1) * BYTES_PER_LINE;
        assert_eq!(out[last + BYTES_PER_LINE - 1], 0b0000_0001);
        assert!(out[..last].iter().all(|&b| b == 0));
    }

    #[test]
    fn a_full_frame_is_12482_bytes_on_the_wire() {
        let line_buf = vec![0u8; (HEIGHT as usize) * BYTES_PER_LINE];
        let lines: Vec<u16> = (0..HEIGHT as u16).collect();
        let mut out = Vec::new();
        build_line_frame(BIT_WRITE_LINE, &lines, &line_buf, &mut out);

        // 1 mode byte + 240 * (address + 50 data + 8 dummy clocks) + 8 more.
        assert_eq!(out.len(), 1 + 240 * (1 + BYTES_PER_LINE + 1) + 1);
        assert_eq!(out.len(), 12482);
    }

    #[test]
    fn a_partial_frame_carries_only_the_lines_it_was_given() {
        let mut line_buf = vec![0u8; (HEIGHT as usize) * BYTES_PER_LINE];
        line_buf[3 * BYTES_PER_LINE] = 0xA5;

        let mut out = Vec::new();
        build_line_frame(BIT_WRITE_LINE | BIT_VCOM, &[3, 9], &line_buf, &mut out);

        assert_eq!(out.len(), 1 + 2 * (BYTES_PER_LINE + 2) + 1);
        assert_eq!(out[0], 0xC0);                  // M0 and M1 set
        assert_eq!(out[1], gate_address(3));
        assert_eq!(out[2], 0xA5);
        assert_eq!(out[1 + BYTES_PER_LINE + 1], 0x00);         // end of line 3
        assert_eq!(out[2 + BYTES_PER_LINE + 1], gate_address(9));
        assert_eq!(*out.last().unwrap(), 0x00);
    }

    #[test]
    fn mode_bits_match_the_datasheet_wire_order() {
        // M0 first, then M1, then M2, MSB first on this bus.
        assert_eq!(BIT_WRITE_LINE, 0x80);
        assert_eq!(BIT_VCOM, 0x40);
        assert_eq!(BIT_CLEAR_ALL, 0x20);
    }
}
