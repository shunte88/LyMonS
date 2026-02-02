/*
 *  display/drivers/mock.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Mock display driver for testing without hardware
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
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::Size;

use crate::config::DisplayConfig;
use crate::display::error::DisplayError;
use crate::display::traits::{DisplayDriver, DrawableDisplay, DisplayCapabilities, ColorDepth};
use crate::vframebuf::VarFrameBuf;

use std::sync::{Arc, Mutex};

/// Mock display driver for testing
///
/// This driver simulates a display without requiring hardware. It's useful for:
/// - Unit tests
/// - Integration tests
/// - CI/CD pipelines
/// - Development without hardware
///
/// The mock driver records all operations and provides access to the framebuffer
/// for verification in tests.
#[derive(Debug, Clone)]
pub struct MockDriver {
    /// Framebuffer for drawing operations
    framebuffer: VarFrameBuf<BinaryColor>,

    /// Display capabilities
    capabilities: DisplayCapabilities,

    /// Shared state for testing
    state: Arc<Mutex<MockDriverState>>,
}

/// Internal state for the mock driver (shared for inspection in tests)
#[derive(Debug, Default)]
pub struct MockDriverState {
    /// Number of times init() was called
    pub init_count: usize,

    /// Number of times flush() was called
    pub flush_count: usize,

    /// Number of times clear() was called
    pub clear_count: usize,

    /// Last brightness value set
    pub last_brightness: Option<u8>,

    /// Last rotation set
    pub last_rotation: Option<u16>,

    /// Last invert state set
    pub last_invert: Option<bool>,

    /// Whether the driver is initialized
    pub is_initialized: bool,

    /// Total bytes written via write_buffer
    pub bytes_written: usize,

    /// Simulate failures (for error testing)
    pub simulate_flush_failure: bool,
    pub simulate_init_failure: bool,
}

impl MockDriver {
    /// Create a new mock driver
    ///
    /// # Arguments
    ///
    /// * `config` - Display configuration
    ///
    /// # Returns
    ///
    /// A configured mock driver
    pub fn new(config: &DisplayConfig) -> Result<Self, DisplayError> {
        let width = config.width.unwrap_or(128);
        let height = config.height.unwrap_or(64);

        let capabilities = DisplayCapabilities {
            width,
            height,
            color_depth: ColorDepth::Monochrome,
            supports_rotation: true,
            max_fps: 60,
            supports_brightness: true,
            supports_invert: true,
        };

        let framebuffer = VarFrameBuf::new(width, height, BinaryColor::Off);

        Ok(Self {
            framebuffer,
            capabilities,
            state: Arc::new(Mutex::new(MockDriverState::default())),
        })
    }

    /// Create a mock driver with specific dimensions
    pub fn new_with_size(width: u32, height: u32) -> Result<Self, DisplayError> {
        let config = DisplayConfig {
            width: Some(width),
            height: Some(height),
            ..Default::default()
        };
        Self::new(&config)
    }

    /// Get a snapshot of the framebuffer for testing
    pub fn get_framebuffer(&self) -> Vec<BinaryColor> {
        self.framebuffer.as_slice().to_vec()
    }

    /// Get pixel at position for testing
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<BinaryColor> {
        if x >= self.capabilities.width || y >= self.capabilities.height {
            return None;
        }
        let idx = (y * self.capabilities.width + x) as usize;
        self.framebuffer.as_slice().get(idx).copied()
    }

    /// Get reference to state for inspection in tests
    pub fn state(&self) -> Arc<Mutex<MockDriverState>> {
        Arc::clone(&self.state)
    }

    /// Reset state counters (useful between tests)
    pub fn reset_state(&mut self) {
        let mut state = self.state.lock().unwrap();
        *state = MockDriverState::default();
    }

    /// Count number of pixels set to On
    pub fn count_on_pixels(&self) -> usize {
        self.framebuffer
            .as_slice()
            .iter()
            .filter(|&&p| p == BinaryColor::On)
            .count()
    }

    /// Save framebuffer to PBM file (for visual debugging)
    #[cfg(test)]
    pub fn save_to_pbm(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(path)?;

        // PBM header
        writeln!(file, "P1")?;
        writeln!(file, "{} {}", self.capabilities.width, self.capabilities.height)?;

        // Pixel data
        for (i, &pixel) in self.framebuffer.as_slice().iter().enumerate() {
            write!(file, "{}", if pixel == BinaryColor::On { "1" } else { "0" })?;
            if (i + 1) % self.capabilities.width as usize == 0 {
                writeln!(file)?;
            } else {
                write!(file, " ")?;
            }
        }

        Ok(())
    }
}

impl DisplayDriver for MockDriver {
    fn capabilities(&self) -> &DisplayCapabilities {
        &self.capabilities
    }

    fn init(&mut self) -> Result<(), DisplayError> {
        let mut state = self.state.lock().unwrap();

        if state.simulate_init_failure {
            return Err(DisplayError::Other("Simulated init failure".to_string()));
        }

        state.init_count += 1;
        state.is_initialized = true;
        Ok(())
    }

    fn set_brightness(&mut self, value: u8) -> Result<(), DisplayError> {
        let mut state = self.state.lock().unwrap();
        state.last_brightness = Some(value);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        let mut state = self.state.lock().unwrap();

        if state.simulate_flush_failure {
            return Err(DisplayError::Other("Simulated flush failure".to_string()));
        }

        state.flush_count += 1;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        {
            let mut state = self.state.lock().unwrap();
            state.clear_count += 1;
        } // Release lock before calling flush

        self.framebuffer.clear(BinaryColor::Off)
            .map_err(|_| DisplayError::Other("Failed to clear framebuffer".to_string()))?;
        self.flush()
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        let expected_size = (self.capabilities.width * self.capabilities.height / 8) as usize;

        if buffer.len() != expected_size {
            return Err(DisplayError::BufferSizeMismatch {
                expected: expected_size,
                actual: buffer.len(),
            });
        }

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            state.bytes_written += buffer.len();
        }

        // Unpack buffer into framebuffer
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
        let mut state = self.state.lock().unwrap();
        state.last_invert = Some(inverted);
        Ok(())
    }

    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        if degrees != 0 && degrees != 90 && degrees != 180 && degrees != 270 {
            return Err(DisplayError::InvalidRotation(degrees));
        }
        let mut state = self.state.lock().unwrap();
        state.last_rotation = Some(degrees);
        Ok(())
    }
}

impl DrawableDisplay for MockDriver {
    type Color = BinaryColor;
}

impl DrawTarget for MockDriver {
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

impl OriginDimensions for MockDriver {
    fn size(&self) -> Size {
        Size::new(self.capabilities.width, self.capabilities.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::primitives::{PrimitiveStyle, Line};

    #[test]
    fn test_mock_driver_creation() {
        let driver = MockDriver::new_with_size(128, 64).unwrap();
        assert_eq!(driver.capabilities().width, 128);
        assert_eq!(driver.capabilities().height, 64);
        assert_eq!(driver.count_on_pixels(), 0);
    }

    #[test]
    fn test_mock_driver_init() {
        let mut driver = MockDriver::new_with_size(128, 64).unwrap();

        let state = driver.state();
        assert_eq!(state.lock().unwrap().init_count, 0);
        assert!(!state.lock().unwrap().is_initialized);

        driver.init().unwrap();

        assert_eq!(state.lock().unwrap().init_count, 1);
        assert!(state.lock().unwrap().is_initialized);
    }

    #[test]
    fn test_mock_driver_drawing() {
        let mut driver = MockDriver::new_with_size(128, 64).unwrap();

        // Draw a line
        let line = Line::new(Point::new(0, 0), Point::new(10, 10));
        line.into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut driver)
            .unwrap();

        // Check that pixels were set
        assert!(driver.count_on_pixels() > 0);
        assert_eq!(driver.get_pixel(0, 0), Some(BinaryColor::On));
    }

    #[test]
    fn test_mock_driver_clear() {
        let mut driver = MockDriver::new_with_size(128, 64).unwrap();

        // Draw something
        let line = Line::new(Point::new(0, 0), Point::new(10, 10));
        line.into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut driver)
            .unwrap();

        assert!(driver.count_on_pixels() > 0);

        // Clear using DisplayDriver trait (to disambiguate from DrawTarget::clear)
        DisplayDriver::clear(&mut driver).unwrap();

        assert_eq!(driver.count_on_pixels(), 0);
        assert_eq!(driver.state().lock().unwrap().clear_count, 1);
    }

    #[test]
    fn test_mock_driver_brightness() {
        let mut driver = MockDriver::new_with_size(128, 64).unwrap();

        driver.set_brightness(200).unwrap();

        assert_eq!(driver.state().lock().unwrap().last_brightness, Some(200));
    }

    #[test]
    fn test_mock_driver_rotation() {
        let mut driver = MockDriver::new_with_size(128, 64).unwrap();

        driver.set_rotation(90).unwrap();
        assert_eq!(driver.state().lock().unwrap().last_rotation, Some(90));

        driver.set_rotation(180).unwrap();
        assert_eq!(driver.state().lock().unwrap().last_rotation, Some(180));

        // Invalid rotation should fail
        assert!(driver.set_rotation(45).is_err());
    }

    #[test]
    fn test_mock_driver_simulated_failure() {
        let mut driver = MockDriver::new_with_size(128, 64).unwrap();

        // Enable simulated failure
        driver.state().lock().unwrap().simulate_flush_failure = true;

        // Flush should fail
        assert!(driver.flush().is_err());

        // Disable and try again
        driver.state().lock().unwrap().simulate_flush_failure = false;
        assert!(driver.flush().is_ok());
    }

    #[test]
    fn test_mock_driver_write_buffer() {
        let mut driver = MockDriver::new_with_size(128, 64).unwrap();

        // Create a buffer (128x64 / 8 = 1024 bytes)
        let buffer = vec![0xFF; 1024];

        driver.write_buffer(&buffer).unwrap();

        // All pixels should be on
        assert_eq!(driver.count_on_pixels(), 128 * 64);
        assert_eq!(driver.state().lock().unwrap().bytes_written, 1024);
    }

    #[test]
    fn test_mock_driver_buffer_size_mismatch() {
        let mut driver = MockDriver::new_with_size(128, 64).unwrap();

        // Wrong buffer size
        let buffer = vec![0xFF; 512]; // Should be 1024

        assert!(driver.write_buffer(&buffer).is_err());
    }
}
