/*
 *  LyMonS SSD1306 Plugin - Driver Implementation
 *
 *  Implements the SSD1306 OLED display driver as a LyMonS plugin
 */

use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use ssd1306::{
    prelude::*,
    I2CDisplayInterface,
    Ssd1306,
};
use linux_embedded_hal::I2cdev;

use crate::ffi::*;

/// Internal SSD1306 driver state
pub struct Ssd1306PluginDriver {
    /// The actual SSD1306 driver from the ssd1306 crate
    display: Ssd1306<
        I2CInterface<I2cdev>,
        DisplaySize128x64,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>
    >,

    /// Display capabilities
    capabilities: LyMonsDisplayCapabilities,

    /// Current brightness (0-255)
    brightness: u8,

    /// Current inversion state
    inverted: bool,
}

impl Ssd1306PluginDriver {
    /// Create a new SSD1306 driver from configuration
    pub fn new(config: &LyMonsDisplayConfig) -> Result<Self, String> {
        // Extract I2C configuration
        if config.bus.bus_type != LyMonsBusType::I2c {
            return Err("SSD1306 requires I2C bus".to_string());
        }

        let i2c_config = unsafe { &config.bus.config.i2c };

        // Extract bus path
        let bus_path = extract_string_from_buffer(&i2c_config.bus_path);
        let _address = i2c_config.address;

        // Open I2C device
        let i2c = I2cdev::new(&bus_path)
            .map_err(|e| format!("Failed to open I2C device {}: {:?}", bus_path, e))?;

        // Create display interface
        let interface = I2CDisplayInterface::new(i2c);

        // Create display driver
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();

        // Initialize the display
        display.init()
            .map_err(|e| format!("Failed to initialize display: {:?}", e))?;

        // Apply initial configuration
        let brightness = if config.has_brightness {
            config.brightness
        } else {
            128
        };

        // Map brightness (0-255) to Brightness enum (BRIGHTEST, NORMAL, DIM, DIMMEST)
        let brightness_level = match brightness {
            0..=63 => Brightness::DIMMEST,
            64..=127 => Brightness::DIM,
            128..=191 => Brightness::NORMAL,
            192..=255 => Brightness::BRIGHTEST,
        };

        display.set_brightness(brightness_level)
            .map_err(|e| format!("Failed to set brightness: {:?}", e))?;

        // Apply rotation if specified
        if config.has_rotation {
            let rotation = match config.rotation {
                0 => DisplayRotation::Rotate0,
                90 => DisplayRotation::Rotate90,
                180 => DisplayRotation::Rotate180,
                270 => DisplayRotation::Rotate270,
                _ => return Err(format!("Invalid rotation: {}", config.rotation)),
            };

            display.set_rotation(rotation)
                .map_err(|e| format!("Failed to set rotation: {:?}", e))?;
        }

        // Apply inversion if specified
        if config.inverted {
            display.set_display_on(true)
                .map_err(|e| format!("Failed to set display on: {:?}", e))?;
        }

        let capabilities = LyMonsDisplayCapabilities {
            width: 128,
            height: 64,
            color_depth: LyMonsColorDepth::Monochrome,
            supports_rotation: true,
            max_fps: 60,
            supports_brightness: true,
            supports_invert: true,
        };

        Ok(Self {
            display,
            capabilities,
            brightness,
            inverted: config.inverted,
        })
    }

    /// Initialize the display (called after creation)
    pub fn init(&mut self) -> Result<(), String> {
        self.display.clear_buffer();
        self.display.flush()
            .map_err(|e| format!("Failed to flush display: {:?}", e))?;
        Ok(())
    }

    /// Set display brightness (0-255)
    pub fn set_brightness(&mut self, value: u8) -> Result<(), String> {
        self.brightness = value;

        let brightness_level = match value {
            0..=63 => Brightness::DIMMEST,
            64..=127 => Brightness::DIM,
            128..=191 => Brightness::NORMAL,
            192..=255 => Brightness::BRIGHTEST,
        };

        self.display.set_brightness(brightness_level)
            .map_err(|e| format!("Failed to set brightness: {:?}", e))?;

        Ok(())
    }

    /// Flush the framebuffer to the display
    pub fn flush(&mut self) -> Result<(), String> {
        self.display.flush()
            .map_err(|e| format!("Failed to flush: {:?}", e))
    }

    /// Clear the display
    pub fn clear(&mut self) -> Result<(), String> {
        self.display.clear_buffer();
        self.display.flush()
            .map_err(|e| format!("Failed to clear: {:?}", e))
    }

    /// Write raw buffer to display
    pub fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), String> {
        // SSD1306 uses 128x64 = 8192 pixels = 1024 bytes
        let expected_size = 1024;

        if buffer.len() != expected_size {
            return Err(format!(
                "Buffer size mismatch: expected {} bytes, got {}",
                expected_size,
                buffer.len()
            ));
        }

        // Copy buffer to display buffer
        // The ssd1306 crate doesn't expose direct buffer access, so we need to
        // draw pixels individually or use the buffer if available
        // For now, we'll clear and redraw
        self.display.clear_buffer();

        // Convert buffer to pixels
        for (byte_idx, &byte) in buffer.iter().enumerate() {
            let page = byte_idx / 128; // 8 pages (8 pixels high each)
            let col = byte_idx % 128;

            for bit in 0..8 {
                let y = (page * 8 + bit) as i32;
                let x = col as i32;

                if (byte >> bit) & 1 == 1 {
                    let _ = self.display.set_pixel(
                        x as u32,
                        y as u32,
                        true
                    );
                }
            }
        }

        self.display.flush()
            .map_err(|e| format!("Failed to write buffer: {:?}", e))
    }

    /// Set display inversion
    pub fn set_invert(&mut self, inverted: bool) -> Result<(), String> {
        self.inverted = inverted;

        // SSD1306 supports inversion via command
        self.display.set_display_on(!inverted)
            .map_err(|e| format!("Failed to set invert: {:?}", e))
    }

    /// Set display rotation
    pub fn set_rotation(&mut self, degrees: u16) -> Result<(), String> {
        let rotation = match degrees {
            0 => DisplayRotation::Rotate0,
            90 => DisplayRotation::Rotate90,
            180 => DisplayRotation::Rotate180,
            270 => DisplayRotation::Rotate270,
            _ => return Err(format!("Invalid rotation: {}", degrees)),
        };

        self.display.set_rotation(rotation)
            .map_err(|e| format!("Failed to set rotation: {:?}", e))
    }

    /// Get display capabilities
    pub fn capabilities(&self) -> &LyMonsDisplayCapabilities {
        &self.capabilities
    }
}

/// Macro to catch panics in FFI functions
macro_rules! catch_panic {
    ($error:expr, $code:block) => {
        match catch_unwind(AssertUnwindSafe(|| $code)) {
            Ok(result) => result,
            Err(panic_info) => {
                let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    format!("Plugin panic: {}", s)
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    format!("Plugin panic: {}", s)
                } else {
                    "Plugin panic: unknown error".to_string()
                };

                unsafe {
                    *$error = LyMonsError::new(LyMonsErrorCode::ErrorPanic, &message);
                }
                LyMonsErrorCode::ErrorPanic
            }
        }
    };
}

// ============================================================================
// FFI Vtable Implementations
// ============================================================================

/// Get plugin ABI version
extern "C" fn abi_version(major: *mut u32, minor: *mut u32, patch: *mut u32) {
    if !major.is_null() && !minor.is_null() && !patch.is_null() {
        unsafe {
            *major = LYMONS_PLUGIN_ABI_VERSION_MAJOR;
            *minor = LYMONS_PLUGIN_ABI_VERSION_MINOR;
            *patch = LYMONS_PLUGIN_ABI_VERSION_PATCH;
        }
    }
}

/// Get plugin metadata
extern "C" fn plugin_info(
    name: *mut c_char,
    version: *mut c_char,
    driver_type: *mut c_char
) {
    copy_str_to_buffer("LyMonS SSD1306 Driver", name, 64);
    copy_str_to_buffer("1.0.0", version, 32);
    copy_str_to_buffer("ssd1306", driver_type, 32);
}

/// Create a new driver instance
extern "C" fn create(
    config: *const LyMonsDisplayConfig,
    handle: *mut *mut LyMonsDriverHandle,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        // Validate pointers
        if config.is_null() || handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to create"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        // Create driver
        let driver = match Ssd1306PluginDriver::new(unsafe { &*config }) {
            Ok(d) => d,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorInitialization, &e);
                }
                return LyMonsErrorCode::ErrorInitialization;
            }
        };

        // Convert to opaque handle
        unsafe {
            *handle = Box::into_raw(Box::new(driver)) as *mut LyMonsDriverHandle;
        }

        LyMonsErrorCode::Success
    })
}

/// Destroy a driver instance
extern "C" fn destroy(handle: *mut LyMonsDriverHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle as *mut Ssd1306PluginDriver);
        }
    }
}

/// Get driver capabilities
extern "C" fn capabilities(
    handle: *const LyMonsDriverHandle,
    caps: *mut LyMonsDisplayCapabilities,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || caps.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to capabilities"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &*(handle as *const Ssd1306PluginDriver) };
        unsafe {
            *caps = *driver.capabilities();
        }

        LyMonsErrorCode::Success
    })
}

/// Initialize the display
extern "C" fn init(
    handle: *mut LyMonsDriverHandle,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to init"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.init() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorInitialization, &e);
                }
                LyMonsErrorCode::ErrorInitialization
            }
        }
    })
}

/// Set display brightness
extern "C" fn set_brightness(
    handle: *mut LyMonsDriverHandle,
    value: u8,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to set_brightness"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.set_brightness(value) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e);
                }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

/// Flush framebuffer to display
extern "C" fn flush(
    handle: *mut LyMonsDriverHandle,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to flush"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.flush() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e);
                }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

/// Clear the display
extern "C" fn clear(
    handle: *mut LyMonsDriverHandle,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to clear"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.clear() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e);
                }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

/// Write raw buffer to display
extern "C" fn write_buffer(
    handle: *mut LyMonsDriverHandle,
    buffer: *const u8,
    length: usize,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || buffer.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to write_buffer"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };
        let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, length) };

        match driver.write_buffer(buffer_slice) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorInvalidArgument, &e);
                }
                LyMonsErrorCode::ErrorInvalidArgument
            }
        }
    })
}

/// Set display inversion
extern "C" fn set_invert(
    handle: *mut LyMonsDriverHandle,
    inverted: bool,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to set_invert"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.set_invert(inverted) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e);
                }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

/// Set display rotation
extern "C" fn set_rotation(
    handle: *mut LyMonsDriverHandle,
    degrees: u16,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to set_rotation"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.set_rotation(degrees) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorInvalidRotation, &e);
                }
                LyMonsErrorCode::ErrorInvalidRotation
            }
        }
    })
}

// ============================================================================
// Plugin Registration
// ============================================================================

/// Static vtable
static VTABLE: LyMonsPluginVTable = LyMonsPluginVTable {
    abi_version,
    plugin_info,
    create,
    destroy,
    capabilities,
    init,
    set_brightness,
    flush,
    clear,
    write_buffer,
    set_invert,
    set_rotation,
};

/// Plugin entry point - returns the vtable
#[no_mangle]
pub extern "C" fn lymons_plugin_register() -> *const LyMonsPluginVTable {
    &VTABLE
}
