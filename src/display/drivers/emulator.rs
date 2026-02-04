/*
 *  display/drivers/emulator.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Display emulator driver for desktop testing
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

#[cfg(feature = "emulator")]
use embedded_graphics::prelude::*;
#[cfg(feature = "emulator")]
use embedded_graphics::pixelcolor::{BinaryColor, Gray4};
#[cfg(feature = "emulator")]
use embedded_graphics::primitives::Rectangle;
#[cfg(feature = "emulator")]
use embedded_graphics::geometry::Size;

#[cfg(feature = "emulator")]
use crate::config::DisplayConfig;
#[cfg(feature = "emulator")]
use crate::display::error::DisplayError;
#[cfg(feature = "emulator")]
use crate::display::traits::{DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth};
#[cfg(feature = "emulator")]
use crate::vframebuf::VarFrameBuf;

#[cfg(feature = "emulator")]
use std::sync::{Arc, Mutex};

/// Color type for emulator (can be monochrome or grayscale)
#[cfg(feature = "emulator")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmulatorColor {
    Mono(BinaryColor),
    Gray(Gray4),
}

#[cfg(feature = "emulator")]
impl EmulatorColor {
    pub fn to_rgba(&self) -> [u8; 4] {
        match self {
            EmulatorColor::Mono(BinaryColor::Off) => [0, 0, 0, 255],      // Black
            EmulatorColor::Mono(BinaryColor::On) => [0, 255, 128, 255],   // Green (OLED color)
            EmulatorColor::Gray(gray) => {
                let level = gray.luma() as u8;
                let value = (level * 17) as u8; // 0-15 -> 0-255
                [value, value, value, 255]
            }
        }
    }
}

/// Shared emulator state (for window access)
#[cfg(feature = "emulator")]
#[derive(Debug)]
pub struct EmulatorState {
    /// Current framebuffer contents
    pub buffer: Vec<EmulatorColor>,

    /// Display dimensions
    pub width: u32,
    pub height: u32,

    /// Current brightness (0-255)
    pub brightness: u8,

    /// Current rotation (0, 90, 180, 270)
    pub rotation: u16,

    /// Whether display is inverted
    pub inverted: bool,

    /// Frame counter
    pub frame_count: u64,

    /// Display type name (for window title)
    pub display_type: String,

    /// Requested display mode (for keyboard triggers)
    pub requested_mode: Option<crate::display::DisplayMode>,

    /// Manual mode override active (disables automatic mode switching)
    pub manual_mode_override: bool,

    /// Current display mode (what's actually showing)
    pub current_display_mode: crate::display::DisplayMode,
}

/// Emulator display driver
///
/// This driver renders to a desktop window instead of physical hardware.
/// Useful for development and testing without needing actual display hardware.
///
/// # Features
///
/// - Real-time framebuffer rendering
/// - Supports monochrome and grayscale displays
/// - Keyboard controls (see window for shortcuts)
/// - Performance metrics overlay
/// - Screenshot capture
///
/// # Example
///
/// ```no_run
/// use LyMonS::display::drivers::emulator::EmulatorDriver;
/// use LyMonS::config::DisplayConfig;
///
/// let config = DisplayConfig::default();
/// let driver = EmulatorDriver::new_monochrome(128, 64, "SSD1306").unwrap();
/// ```
#[cfg(feature = "emulator")]
pub struct EmulatorDriver {
    /// Framebuffer for drawing operations
    framebuffer: EmulatorFramebuffer,

    /// Display capabilities
    capabilities: DisplayCapabilities,

    /// Shared state (for window rendering)
    state: Arc<Mutex<EmulatorState>>,
}

#[cfg(feature = "emulator")]
#[derive(Debug, Clone)]
enum EmulatorFramebuffer {
    Mono(VarFrameBuf<BinaryColor>),
    Gray(VarFrameBuf<Gray4>),
}

#[cfg(feature = "emulator")]
impl EmulatorDriver {
    /// Create a monochrome emulator driver
    pub fn new_monochrome(width: u32, height: u32, display_type: &str) -> Result<Self, DisplayError> {
        let capabilities = DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Monochrome,
            supports_rotation: true,
            max_fps: 60,
            supports_brightness: true,
            supports_invert: true,
        };

        let framebuffer = EmulatorFramebuffer::Mono(
            VarFrameBuf::new(width, height, BinaryColor::Off)
        );

        let state = Arc::new(Mutex::new(EmulatorState {
            buffer: vec![EmulatorColor::Mono(BinaryColor::Off); (width * height) as usize],
            width,
            height,
            brightness: 200,
            rotation: 0,
            inverted: false,
            frame_count: 0,
            display_type: display_type.to_string(),
            requested_mode: None,
            manual_mode_override: false,
            current_display_mode: crate::display::DisplayMode::Clock,
        }));

        Ok(Self {
            framebuffer,
            capabilities,
            state,
        })
    }

    /// Create a grayscale emulator driver
    pub fn new_grayscale(width: u32, height: u32, display_type: &str) -> Result<Self, DisplayError> {
        let capabilities = DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Gray4,
            supports_rotation: true,
            max_fps: 60,
            supports_brightness: true,
            supports_invert: true,
        };

        let framebuffer = EmulatorFramebuffer::Gray(
            VarFrameBuf::new(width, height, Gray4::new(0))
        );

        let state = Arc::new(Mutex::new(EmulatorState {
            buffer: vec![EmulatorColor::Gray(Gray4::new(0)); (width * height) as usize],
            width,
            height,
            brightness: 255,
            rotation: 0,
            inverted: false,
            frame_count: 0,
            display_type: display_type.to_string(),
            requested_mode: None,
            manual_mode_override: false,
            current_display_mode: crate::display::DisplayMode::Clock,
        }));

        Ok(Self {
            framebuffer,
            capabilities,
            state,
        })
    }

    /// Create from configuration
    pub fn new(config: &DisplayConfig) -> Result<Self, DisplayError> {
        let width = config.width.unwrap_or(128);
        let height = config.height.unwrap_or(64);

        // Determine if grayscale based on driver type
        let is_grayscale = match &config.driver {
            Some(crate::config::DriverKind::Ssd1322) => true,
            _ => false,
        };

        let display_type = match &config.driver {
            Some(kind) => format!("{:?}", kind),
            None => "Emulator".to_string(),
        };

        if is_grayscale {
            Self::new_grayscale(width, height, &display_type)
        } else {
            Self::new_monochrome(width, height, &display_type)
        }
    }

    /// Get shared state for window rendering
    pub fn state(&self) -> Arc<Mutex<EmulatorState>> {
        Arc::clone(&self.state)
    }

    /// Sync framebuffer to shared state
    fn sync_to_state(&self) {
        let mut state = self.state.lock().unwrap();

        match &self.framebuffer {
            EmulatorFramebuffer::Mono(fb) => {
                for (i, &pixel) in fb.as_slice().iter().enumerate() {
                    state.buffer[i] = EmulatorColor::Mono(pixel);
                }
            }
            EmulatorFramebuffer::Gray(fb) => {
                for (i, &pixel) in fb.as_slice().iter().enumerate() {
                    state.buffer[i] = EmulatorColor::Gray(pixel);
                }
            }
        }

        state.frame_count += 1;
    }

    /// Check and consume requested display mode (for keyboard triggers)
    pub fn take_requested_mode(&self) -> Option<crate::display::DisplayMode> {
        let mut state = self.state.lock().unwrap();
        state.requested_mode.take()
    }
}

#[cfg(feature = "emulator")]
impl DisplayDriver for EmulatorDriver {
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
        // Nothing to initialize for emulator
        Ok(())
    }

    fn set_brightness(&mut self, value: u8) -> Result<(), DisplayError> {
        let mut state = self.state.lock().unwrap();
        state.brightness = value;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        // Sync framebuffer to shared state for window rendering
        self.sync_to_state();
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        match &mut self.framebuffer {
            EmulatorFramebuffer::Mono(fb) => {
                fb.clear(BinaryColor::Off)
                    .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
            }
            EmulatorFramebuffer::Gray(fb) => {
                fb.clear(Gray4::new(0))
                    .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
            }
        }
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        match &mut self.framebuffer {
            EmulatorFramebuffer::Mono(fb) => {
                let expected_size = (self.capabilities.width * self.capabilities.height / 8) as usize;

                if buffer.len() != expected_size {
                    return Err(DisplayError::BufferSizeMismatch {
                        expected: expected_size,
                        actual: buffer.len(),
                    });
                }

                // Unpack buffer into framebuffer
                for (byte_idx, &byte) in buffer.iter().enumerate() {
                    for bit in 0..8 {
                        let pixel_idx = byte_idx * 8 + bit;
                        if pixel_idx < fb.as_slice().len() {
                            let color = if (byte & (1 << bit)) != 0 {
                                BinaryColor::On
                            } else {
                                BinaryColor::Off
                            };
                            let fb_slice = fb.as_mut_slice();
                            fb_slice[pixel_idx] = color;
                        }
                    }
                }
            }
            EmulatorFramebuffer::Gray(fb) => {
                let expected_size = (self.capabilities.width * self.capabilities.height / 2) as usize;

                if buffer.len() != expected_size {
                    return Err(DisplayError::BufferSizeMismatch {
                        expected: expected_size,
                        actual: buffer.len(),
                    });
                }

                // Unpack buffer into framebuffer (2 pixels per byte)
                let fb_slice = fb.as_mut_slice();
                for (byte_idx, &byte) in buffer.iter().enumerate() {
                    let pixel_idx = byte_idx * 2;
                    if pixel_idx < fb_slice.len() {
                        fb_slice[pixel_idx] = Gray4::new((byte >> 4) & 0x0F);
                    }
                    if pixel_idx + 1 < fb_slice.len() {
                        fb_slice[pixel_idx + 1] = Gray4::new(byte & 0x0F);
                    }
                }
            }
        }

        self.flush()
    }

    fn set_invert(&mut self, inverted: bool) -> Result<(), DisplayError> {
        let mut state = self.state.lock().unwrap();
        state.inverted = inverted;
        Ok(())
    }

    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        if degrees != 0 && degrees != 90 && degrees != 180 && degrees != 270 {
            return Err(DisplayError::InvalidRotation(degrees));
        }
        let mut state = self.state.lock().unwrap();
        state.rotation = degrees;
        Ok(())
    }
}

#[cfg(feature = "emulator")]
impl DrawableDisplay for EmulatorDriver {
    type Color = BinaryColor;
}

// Implement DrawTarget for monochrome
#[cfg(feature = "emulator")]
impl DrawTarget for EmulatorDriver {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        match &mut self.framebuffer {
            EmulatorFramebuffer::Mono(fb) => fb.draw_iter(pixels),
            _ => Ok(()),
        }
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        match &mut self.framebuffer {
            EmulatorFramebuffer::Mono(fb) => fb.clear(color),
            _ => Ok(()),
        }
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        match &mut self.framebuffer {
            EmulatorFramebuffer::Mono(fb) => fb.fill_contiguous(area, colors),
            _ => Ok(()),
        }
    }
}

#[cfg(feature = "emulator")]
impl OriginDimensions for EmulatorDriver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}
