/*
 *  LyMonS SSD1322 Plugin - Simplified Implementation
 *  Note: Full hardware implementation requires SPI HAL compatibility
 */

use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use linux_embedded_hal::SpidevDevice;
use crate::ffi::*;

pub struct Ssd1322PluginDriver {
    _spi: SpidevDevice,
    capabilities: LyMonsDisplayCapabilities,
    brightness: u8,
}

impl Ssd1322PluginDriver {
    pub fn new(config: &LyMonsDisplayConfig) -> Result<Self, String> {
        if config.bus.bus_type != LyMonsBusType::Spi {
            return Err("SSD1322 requires SPI bus".to_string());
        }

        let spi_config = unsafe { &config.bus.config.spi };
        let bus_path = extract_string_from_buffer(&spi_config.bus_path);

        let spi = SpidevDevice::open(&bus_path)
            .map_err(|e| format!("Failed to open SPI: {:?}", e))?;

        let capabilities = LyMonsDisplayCapabilities {
            width: 256,
            height: 64,
            color_depth: LyMonsColorDepth::Gray4,
            supports_rotation: true,
            max_fps: 100,
            supports_brightness: true,
            supports_invert: false,
        };

        Ok(Self {
            _spi: spi,
            capabilities,
            brightness: if config.has_brightness { config.brightness } else { 128 },
        })
    }

    pub fn init(&mut self) -> Result<(), String> { Ok(()) }
    pub fn set_brightness(&mut self, value: u8) -> Result<(), String> { self.brightness = value; Ok(()) }
    pub fn flush(&mut self) -> Result<(), String> { Ok(()) }
    pub fn clear(&mut self) -> Result<(), String> { Ok(()) }
    pub fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), String> {
        let expected_size = 8192;
        if buffer.len() != expected_size {
            return Err(format!("Buffer size mismatch: expected {} got {}", expected_size, buffer.len()));
        }
        Ok(())
    }
    pub fn set_invert(&mut self, _inverted: bool) -> Result<(), String> {
        Err("Inversion not supported by SSD1322".to_string())
    }
    pub fn set_rotation(&mut self, _degrees: u16) -> Result<(), String> { Ok(()) }
    pub fn capabilities(&self) -> &LyMonsDisplayCapabilities { &self.capabilities }
}

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

extern "C" fn abi_version(major: *mut u32, minor: *mut u32, patch: *mut u32) {
    if !major.is_null() && !minor.is_null() && !patch.is_null() {
        unsafe {
            *major = LYMONS_PLUGIN_ABI_VERSION_MAJOR;
            *minor = LYMONS_PLUGIN_ABI_VERSION_MINOR;
            *patch = LYMONS_PLUGIN_ABI_VERSION_PATCH;
        }
    }
}

extern "C" fn plugin_info(name: *mut c_char, version: *mut c_char, driver_type: *mut c_char) {
    copy_str_to_buffer("LyMonS SSD1322 Driver", name, 64);
    copy_str_to_buffer("1.0.0", version, 32);
    copy_str_to_buffer("ssd1322", driver_type, 32);
}

extern "C" fn create(config: *const LyMonsDisplayConfig, handle: *mut *mut LyMonsDriverHandle, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if config.is_null() || handle.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = match Ssd1322PluginDriver::new(unsafe { &*config }) {
            Ok(d) => d,
            Err(e) => {
                unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorInitialization, &e); }
                return LyMonsErrorCode::ErrorInitialization;
            }
        };
        unsafe { *handle = Box::into_raw(Box::new(driver)) as *mut LyMonsDriverHandle; }
        LyMonsErrorCode::Success
    })
}

extern "C" fn destroy(handle: *mut LyMonsDriverHandle) {
    if !handle.is_null() {
        unsafe { let _ = Box::from_raw(handle as *mut Ssd1322PluginDriver); }
    }
}

extern "C" fn capabilities(handle: *const LyMonsDriverHandle, caps: *mut LyMonsDisplayCapabilities, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || caps.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = unsafe { &*(handle as *const Ssd1322PluginDriver) };
        unsafe { *caps = *driver.capabilities(); }
        LyMonsErrorCode::Success
    })
}

extern "C" fn init(handle: *mut LyMonsDriverHandle, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = unsafe { &mut *(handle as *mut Ssd1322PluginDriver) };
        match driver.init() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorInitialization, &e); }
                LyMonsErrorCode::ErrorInitialization
            }
        }
    })
}

extern "C" fn set_brightness(handle: *mut LyMonsDriverHandle, value: u8, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = unsafe { &mut *(handle as *mut Ssd1322PluginDriver) };
        match driver.set_brightness(value) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e); }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

extern "C" fn flush(handle: *mut LyMonsDriverHandle, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = unsafe { &mut *(handle as *mut Ssd1322PluginDriver) };
        match driver.flush() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e); }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

extern "C" fn clear(handle: *mut LyMonsDriverHandle, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = unsafe { &mut *(handle as *mut Ssd1322PluginDriver) };
        match driver.clear() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e); }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

extern "C" fn write_buffer(handle: *mut LyMonsDriverHandle, buffer: *const u8, length: usize, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || buffer.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = unsafe { &mut *(handle as *mut Ssd1322PluginDriver) };
        let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, length) };
        match driver.write_buffer(buffer_slice) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorInvalidArgument, &e); }
                LyMonsErrorCode::ErrorInvalidArgument
            }
        }
    })
}

extern "C" fn set_invert(handle: *mut LyMonsDriverHandle, inverted: bool, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = unsafe { &mut *(handle as *mut Ssd1322PluginDriver) };
        match driver.set_invert(inverted) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorUnsupportedOperation, &e); }
                LyMonsErrorCode::ErrorUnsupportedOperation
            }
        }
    })
}

extern "C" fn set_rotation(handle: *mut LyMonsDriverHandle, degrees: u16, error: *mut LyMonsError) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorNullPointer, "Null pointer"); }
            return LyMonsErrorCode::ErrorNullPointer;
        }
        let driver = unsafe { &mut *(handle as *mut Ssd1322PluginDriver) };
        match driver.set_rotation(degrees) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe { *error = LyMonsError::new(LyMonsErrorCode::ErrorInvalidRotation, &e); }
                LyMonsErrorCode::ErrorInvalidRotation
            }
        }
    })
}

static VTABLE: LyMonsPluginVTable = LyMonsPluginVTable {
    abi_version, plugin_info, create, destroy, capabilities, init,
    set_brightness, flush, clear, write_buffer, set_invert, set_rotation,
};

#[no_mangle]
pub extern "C" fn lymons_plugin_register() -> *const LyMonsPluginVTable {
    &VTABLE
}
