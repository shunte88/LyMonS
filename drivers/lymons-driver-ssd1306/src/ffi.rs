/*
 *  LyMonS SSD1306 Plugin - FFI Types
 *
 *  C ABI types matching the LyMonS plugin interface
 *  These types must match exactly with the host's FFI types
 */

use std::ffi::c_char;
use std::mem::ManuallyDrop;

/// Plugin ABI version
pub const LYMONS_PLUGIN_ABI_VERSION_MAJOR: u32 = 1;
pub const LYMONS_PLUGIN_ABI_VERSION_MINOR: u32 = 0;
pub const LYMONS_PLUGIN_ABI_VERSION_PATCH: u32 = 0;

/// Maximum length for error messages
pub const LYMONS_ERROR_MESSAGE_SIZE: usize = 256;

/// Opaque handle to a plugin driver instance
#[repr(C)]
pub struct LyMonsDriverHandle {
    _private: [u8; 0],
}

/// Error codes returned by plugin functions
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyMonsErrorCode {
    Success = 0,
    ErrorGeneric = 1,
    ErrorInvalidArgument = 2,
    ErrorUnsupportedOperation = 3,
    ErrorCommunication = 4,
    ErrorInitialization = 5,
    ErrorInvalidRotation = 6,
    ErrorNullPointer = 7,
    ErrorPanic = 8,
    ErrorAbiMismatch = 9,
}

/// Error information structure
#[repr(C)]
pub struct LyMonsError {
    pub code: LyMonsErrorCode,
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
}

impl Default for LyMonsError {
    fn default() -> Self {
        Self::success()
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
#[derive(Clone, Copy)]
pub struct LyMonsI2cConfig {
    pub bus_path: [c_char; 256],
    pub address: u8,
    pub speed_hz: u32,
}

/// SPI bus configuration
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LyMonsSpiConfig {
    pub bus_path: [c_char; 256],
    pub dc_pin: u8,
    pub rst_pin: u8,
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

/// Display configuration passed to plugin
#[repr(C)]
pub struct LyMonsDisplayConfig {
    pub bus: LyMonsBusConfig,
    pub rotation: u16,
    pub has_rotation: bool,
    pub brightness: u8,
    pub has_brightness: bool,
    pub inverted: bool,
}

/// Display capabilities
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LyMonsDisplayCapabilities {
    pub width: u32,
    pub height: u32,
    pub color_depth: LyMonsColorDepth,
    pub supports_rotation: bool,
    pub max_fps: u32,
    pub supports_brightness: bool,
    pub supports_invert: bool,
}

/// Plugin vtable - function pointers for all driver operations
#[repr(C)]
pub struct LyMonsPluginVTable {
    pub abi_version: extern "C" fn(*mut u32, *mut u32, *mut u32),
    pub plugin_info: extern "C" fn(*mut c_char, *mut c_char, *mut c_char),
    pub create: extern "C" fn(
        *const LyMonsDisplayConfig,
        *mut *mut LyMonsDriverHandle,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
    pub destroy: extern "C" fn(*mut LyMonsDriverHandle),
    pub capabilities: extern "C" fn(
        *const LyMonsDriverHandle,
        *mut LyMonsDisplayCapabilities,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
    pub init: extern "C" fn(
        *mut LyMonsDriverHandle,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
    pub set_brightness: extern "C" fn(
        *mut LyMonsDriverHandle,
        u8,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
    pub flush: extern "C" fn(
        *mut LyMonsDriverHandle,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
    pub clear: extern "C" fn(
        *mut LyMonsDriverHandle,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
    pub write_buffer: extern "C" fn(
        *mut LyMonsDriverHandle,
        *const u8,
        usize,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
    pub set_invert: extern "C" fn(
        *mut LyMonsDriverHandle,
        bool,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
    pub set_rotation: extern "C" fn(
        *mut LyMonsDriverHandle,
        u16,
        *mut LyMonsError
    ) -> LyMonsErrorCode,
}

/// Helper to copy string to C buffer
pub fn copy_str_to_buffer(s: &str, buffer: *mut c_char, max_len: usize) {
    if buffer.is_null() {
        return;
    }

    let bytes = s.as_bytes();
    let len = bytes.len().min(max_len - 1);

    unsafe {
        for (i, &byte) in bytes.iter().take(len).enumerate() {
            *buffer.add(i) = byte as c_char;
        }
        *buffer.add(len) = 0; // Null terminator
    }
}

/// Helper to extract string from C buffer
pub fn extract_string_from_buffer(buffer: &[c_char]) -> String {
    let len = buffer.iter()
        .position(|&c| c == 0)
        .unwrap_or(buffer.len());

    let bytes: Vec<u8> = buffer[..len]
        .iter()
        .map(|&c| c as u8)
        .collect();

    String::from_utf8_lossy(&bytes).into_owned()
}
