/*
 *  display/plugin/ffi.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  C ABI types for plugin interface
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

//! FFI types for the LyMonS plugin system
//!
//! This module defines C-compatible types that form the stable ABI
//! between the host application and plugins. All types use `#[repr(C)]`
//! to ensure consistent memory layout across compilation units.

use std::ffi::{c_void, c_char};
use std::os::raw::c_int;
use std::mem::ManuallyDrop;
use crate::config::{DisplayConfig, BusConfig};
use crate::display::{DisplayCapabilities, ColorDepth};
use crate::display::error::DisplayError;

/// Plugin ABI version
pub const LYMONS_PLUGIN_ABI_VERSION_MAJOR: u32 = 1;
pub const LYMONS_PLUGIN_ABI_VERSION_MINOR: u32 = 0;
pub const LYMONS_PLUGIN_ABI_VERSION_PATCH: u32 = 0;

/// Maximum length for error messages
pub const LYMONS_ERROR_MESSAGE_SIZE: usize = 256;

/// Maximum length for plugin metadata strings
pub const LYMONS_PLUGIN_NAME_SIZE: usize = 64;
pub const LYMONS_PLUGIN_VERSION_SIZE: usize = 32;
pub const LYMONS_PLUGIN_DRIVER_TYPE_SIZE: usize = 32;

/// Opaque handle to a plugin driver instance
#[repr(C)]
pub struct LyMonsDriverHandle {
    _private: [u8; 0],
}

/// Error codes returned by plugin functions
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyMonsErrorCode {
    /// Operation completed successfully
    Success = 0,

    /// Generic error
    ErrorGeneric = 1,

    /// Invalid argument passed to function
    ErrorInvalidArgument = 2,

    /// Unsupported operation for this driver
    ErrorUnsupportedOperation = 3,

    /// Hardware communication error
    ErrorCommunication = 4,

    /// Initialization failed
    ErrorInitialization = 5,

    /// Invalid rotation angle
    ErrorInvalidRotation = 6,

    /// Null pointer passed where non-null expected
    ErrorNullPointer = 7,

    /// Panic occurred in plugin code
    ErrorPanic = 8,

    /// ABI version mismatch
    ErrorAbiMismatch = 9,
}

/// Error information structure
#[repr(C)]
pub struct LyMonsError {
    /// Error code
    pub code: LyMonsErrorCode,

    /// Human-readable error message (null-terminated)
    pub message: [c_char; LYMONS_ERROR_MESSAGE_SIZE],
}

impl LyMonsError {
    /// Create a new error with code and message
    pub fn new(code: LyMonsErrorCode, message: &str) -> Self {
        let mut error = Self {
            code,
            message: [0; LYMONS_ERROR_MESSAGE_SIZE],
        };

        let bytes = message.as_bytes();
        let len = bytes.len().min(LYMONS_ERROR_MESSAGE_SIZE - 1);

        for (i, &byte) in bytes.iter().take(len).enumerate() {
            error.message[i] = byte as c_char;
        }

        error
    }

    /// Create a success error (no error)
    pub fn success() -> Self {
        Self::new(LyMonsErrorCode::Success, "")
    }

    /// Extract error message as Rust string
    pub fn message_str(&self) -> String {
        let len = self.message.iter()
            .position(|&c| c == 0)
            .unwrap_or(LYMONS_ERROR_MESSAGE_SIZE);

        let bytes: Vec<u8> = self.message[..len]
            .iter()
            .map(|&c| c as u8)
            .collect();

        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Default for LyMonsError {
    fn default() -> Self {
        Self::success()
    }
}

/// Convert DisplayError to LyMonsError
impl From<DisplayError> for LyMonsError {
    fn from(error: DisplayError) -> Self {
        let (code, message) = match error {
            DisplayError::UnsupportedOperation =>
                (LyMonsErrorCode::ErrorUnsupportedOperation, "Unsupported operation".to_string()),
            DisplayError::InvalidRotation(deg) =>
                (LyMonsErrorCode::ErrorInvalidRotation, format!("Invalid rotation: {}", deg)),
            DisplayError::I2cError(msg) | DisplayError::SpiError(msg) =>
                (LyMonsErrorCode::ErrorCommunication, msg),
            DisplayError::InitializationFailed(msg) =>
                (LyMonsErrorCode::ErrorInitialization, msg),
            DisplayError::InvalidConfiguration(msg) =>
                (LyMonsErrorCode::ErrorInvalidArgument, msg),
            DisplayError::GpioError(msg) | DisplayError::DrawingError(msg) | DisplayError::Other(msg) =>
                (LyMonsErrorCode::ErrorGeneric, msg),
            DisplayError::InterfaceError(e) =>
                (LyMonsErrorCode::ErrorCommunication, format!("{:?}", e)),
            DisplayError::BufferSizeMismatch { expected, actual } =>
                (LyMonsErrorCode::ErrorInvalidArgument, format!("Buffer size mismatch: expected {}, got {}", expected, actual)),
        };

        Self::new(code, &message)
    }
}

/// Convert LyMonsError to DisplayError
impl From<LyMonsError> for DisplayError {
    fn from(error: LyMonsError) -> Self {
        let message = error.message_str();

        match error.code {
            LyMonsErrorCode::Success => DisplayError::Other("No error".to_string()),
            LyMonsErrorCode::ErrorUnsupportedOperation => DisplayError::UnsupportedOperation,
            LyMonsErrorCode::ErrorInvalidRotation =>
                DisplayError::InvalidRotation(0), // Can't extract value from message
            LyMonsErrorCode::ErrorCommunication => DisplayError::I2cError(message),
            LyMonsErrorCode::ErrorInitialization => DisplayError::InitializationFailed(message),
            LyMonsErrorCode::ErrorInvalidArgument => DisplayError::InvalidConfiguration(message),
            _ => DisplayError::Other(message),
        }
    }
}

/// Bus type for display communication
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyMonsBusType {
    I2c = 0,
    Spi = 1,
}

/// I2C bus configuration
#[repr(C)]
pub struct LyMonsI2cConfig {
    /// I2C bus device path (e.g., "/dev/i2c-1")
    pub bus_path: [c_char; 256],

    /// I2C address (e.g., 0x3C)
    pub address: u8,

    /// Optional speed in Hz (0 = default)
    pub speed_hz: u32,
}

/// SPI bus configuration
#[repr(C)]
pub struct LyMonsSpiConfig {
    /// SPI bus device path (e.g., "/dev/spidev0.0")
    pub bus_path: [c_char; 256],

    /// Data/Command pin (GPIO pin number)
    pub dc_pin: u8,

    /// Reset pin (GPIO pin number)
    pub rst_pin: u8,

    /// Optional clock speed in Hz (0 = default)
    pub speed_hz: u32,
}

/// Bus configuration union
#[repr(C)]
pub union LyMonsBusConfigUnion {
    pub i2c: ManuallyDrop<LyMonsI2cConfig>,
    pub spi: ManuallyDrop<LyMonsSpiConfig>,
}

/// Display bus configuration
#[repr(C)]
pub struct LyMonsBusConfig {
    pub bus_type: LyMonsBusType,
    pub config: LyMonsBusConfigUnion,
}

/// Color depth enum
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyMonsColorDepth {
    Monochrome = 0,
    Gray4 = 1,
}

impl From<ColorDepth> for LyMonsColorDepth {
    fn from(depth: ColorDepth) -> Self {
        match depth {
            ColorDepth::Monochrome => LyMonsColorDepth::Monochrome,
            ColorDepth::Gray4 => LyMonsColorDepth::Gray4,
        }
    }
}

impl From<LyMonsColorDepth> for ColorDepth {
    fn from(depth: LyMonsColorDepth) -> Self {
        match depth {
            LyMonsColorDepth::Monochrome => ColorDepth::Monochrome,
            LyMonsColorDepth::Gray4 => ColorDepth::Gray4,
        }
    }
}

/// Display configuration passed to plugin
#[repr(C)]
pub struct LyMonsDisplayConfig {
    /// Bus configuration
    pub bus: LyMonsBusConfig,

    /// Optional rotation in degrees (0, 90, 180, 270)
    pub rotation: u16,

    /// Whether rotation is specified
    pub has_rotation: bool,

    /// Optional brightness (0-255)
    pub brightness: u8,

    /// Whether brightness is specified
    pub has_brightness: bool,

    /// Whether display should be inverted
    pub inverted: bool,
}

/// Convert Rust DisplayConfig to FFI config
pub fn display_config_to_ffi(config: &DisplayConfig) -> Result<LyMonsDisplayConfig, String> {
    let bus_config = config.bus.as_ref()
        .ok_or("No bus configuration")?;

    let bus = match bus_config {
        BusConfig::I2c { bus, address, speed_hz } => {
            let mut bus_path = [0i8; 256];
            let bytes = bus.as_bytes();
            let len = bytes.len().min(255);

            for (i, &byte) in bytes.iter().take(len).enumerate() {
                bus_path[i] = byte as c_char;
            }

            LyMonsBusConfig {
                bus_type: LyMonsBusType::I2c,
                config: LyMonsBusConfigUnion {
                    i2c: ManuallyDrop::new(LyMonsI2cConfig {
                        bus_path,
                        address: *address,
                        speed_hz: speed_hz.unwrap_or(0),
                    })
                }
            }
        }

        BusConfig::Spi { bus, dc_pin, rst_pin, speed_hz, cs_pin: _ } => {
            let mut bus_path = [0i8; 256];
            let bytes = bus.as_bytes();
            let len = bytes.len().min(255);

            for (i, &byte) in bytes.iter().take(len).enumerate() {
                bus_path[i] = byte as c_char;
            }

            LyMonsBusConfig {
                bus_type: LyMonsBusType::Spi,
                config: LyMonsBusConfigUnion {
                    spi: ManuallyDrop::new(LyMonsSpiConfig {
                        bus_path,
                        dc_pin: *dc_pin as u8,
                        rst_pin: rst_pin.unwrap_or(0) as u8,
                        speed_hz: speed_hz.unwrap_or(0),
                    })
                }
            }
        }
    };

    Ok(LyMonsDisplayConfig {
        bus,
        rotation: config.rotate_deg.unwrap_or(0),
        has_rotation: config.rotate_deg.is_some(),
        brightness: config.brightness.unwrap_or(128),
        has_brightness: config.brightness.is_some(),
        inverted: config.invert.unwrap_or(false),
    })
}

/// Display capabilities
#[repr(C)]
pub struct LyMonsDisplayCapabilities {
    pub width: u32,
    pub height: u32,
    pub color_depth: LyMonsColorDepth,
    pub supports_rotation: bool,
    pub max_fps: u32,
    pub supports_brightness: bool,
    pub supports_invert: bool,
}

impl From<&DisplayCapabilities> for LyMonsDisplayCapabilities {
    fn from(caps: &DisplayCapabilities) -> Self {
        Self {
            width: caps.width,
            height: caps.height,
            color_depth: caps.color_depth.into(),
            supports_rotation: caps.supports_rotation,
            max_fps: caps.max_fps,
            supports_brightness: caps.supports_brightness,
            supports_invert: caps.supports_invert,
        }
    }
}

impl From<LyMonsDisplayCapabilities> for DisplayCapabilities {
    fn from(caps: LyMonsDisplayCapabilities) -> Self {
        Self {
            width: caps.width,
            height: caps.height,
            color_depth: caps.color_depth.into(),
            supports_rotation: caps.supports_rotation,
            max_fps: caps.max_fps,
            supports_brightness: caps.supports_brightness,
            supports_invert: caps.supports_invert,
        }
    }
}

/// Plugin vtable - function pointers for all driver operations
#[repr(C)]
pub struct LyMonsPluginVTable {
    /// Get plugin ABI version (major, minor, patch)
    pub abi_version: extern "C" fn(
        major: *mut u32,
        minor: *mut u32,
        patch: *mut u32
    ),

    /// Get plugin metadata (name, version, driver_type)
    pub plugin_info: extern "C" fn(
        name: *mut c_char,
        version: *mut c_char,
        driver_type: *mut c_char
    ),

    /// Create a new driver instance
    pub create: extern "C" fn(
        config: *const LyMonsDisplayConfig,
        handle: *mut *mut LyMonsDriverHandle,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,

    /// Destroy a driver instance
    pub destroy: extern "C" fn(
        handle: *mut LyMonsDriverHandle
    ),

    /// Get driver capabilities
    pub capabilities: extern "C" fn(
        handle: *const LyMonsDriverHandle,
        caps: *mut LyMonsDisplayCapabilities,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,

    /// Initialize the display hardware
    pub init: extern "C" fn(
        handle: *mut LyMonsDriverHandle,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,

    /// Set display brightness (0-255)
    pub set_brightness: extern "C" fn(
        handle: *mut LyMonsDriverHandle,
        value: u8,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,

    /// Flush framebuffer to display
    pub flush: extern "C" fn(
        handle: *mut LyMonsDriverHandle,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,

    /// Clear the display
    pub clear: extern "C" fn(
        handle: *mut LyMonsDriverHandle,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,

    /// Write raw buffer to display
    pub write_buffer: extern "C" fn(
        handle: *mut LyMonsDriverHandle,
        buffer: *const u8,
        length: usize,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,

    /// Set display inversion
    pub set_invert: extern "C" fn(
        handle: *mut LyMonsDriverHandle,
        inverted: bool,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,

    /// Set display rotation (0, 90, 180, 270)
    pub set_rotation: extern "C" fn(
        handle: *mut LyMonsDriverHandle,
        degrees: u16,
        error: *mut LyMonsError
    ) -> LyMonsErrorCode,
}

/// Plugin registration function type
///
/// Each plugin must export a function with this signature:
/// ```c
/// #[no_mangle]
/// pub extern "C" fn lymons_plugin_register() -> *const LyMonsPluginVTable
/// ```
pub type PluginRegisterFn = extern "C" fn() -> *const LyMonsPluginVTable;
