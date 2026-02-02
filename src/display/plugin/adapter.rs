/*
 *  display/plugin/adapter.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Plugin adapter - wraps C ABI plugins as Rust trait objects
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

use std::panic::{self, AssertUnwindSafe};
use log::{error, debug};

use crate::config::DisplayConfig;
use crate::display::{DisplayDriver, DisplayCapabilities, DisplayError};
use super::ffi::{
    LyMonsDriverHandle,
    LyMonsErrorCode,
    LyMonsError,
    LyMonsDisplayCapabilities,
    display_config_to_ffi,
};
use super::loader::LoadedPlugin;

/// Adapter that wraps a plugin to implement the DisplayDriver trait
///
/// This struct provides the bridge between the C ABI plugin interface
/// and Rust's DisplayDriver trait. It handles:
///
/// - FFI safety and error conversion
/// - Panic safety for all plugin calls
/// - Proper resource cleanup (RAII)
/// - Caching of display capabilities
pub struct PluginDriverAdapter {
    /// The loaded plugin (kept alive for vtable access)
    plugin: LoadedPlugin,

    /// Opaque handle to the plugin driver instance
    handle: *mut LyMonsDriverHandle,

    /// Cached display capabilities
    capabilities: DisplayCapabilities,
}

// SAFETY: PluginDriverAdapter is safe to share across threads because:
// 1. The plugin handle is only accessed through the FFI vtable which provides its own synchronization
// 2. All vtable calls are wrapped in panic-catching mechanisms
// 3. The handle is never directly dereferenced in Rust code
unsafe impl Sync for PluginDriverAdapter {}

impl PluginDriverAdapter {
    /// Create a new plugin driver adapter
    ///
    /// This creates a driver instance through the plugin's vtable
    /// and caches its capabilities.
    pub fn new(plugin: LoadedPlugin, config: &DisplayConfig) -> Result<Self, DisplayError> {
        let vtable = plugin.vtable();

        // Convert Rust config to FFI config
        let ffi_config = display_config_to_ffi(config)
            .map_err(|e| DisplayError::InvalidConfiguration(e))?;

        // Create driver instance
        let mut handle: *mut LyMonsDriverHandle = std::ptr::null_mut();
        let mut error = LyMonsError::default();

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.create)(&ffi_config, &mut handle, &mut error)
        });

        if let Some(e) = panic_error {
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success || handle.is_null() {
            return Err(error.into());
        }

        debug!("Created plugin driver instance: {:p}", handle);

        // Get capabilities
        let mut ffi_caps = LyMonsDisplayCapabilities {
            width: 0,
            height: 0,
            color_depth: super::ffi::LyMonsColorDepth::Monochrome,
            supports_rotation: false,
            max_fps: 0,
            supports_brightness: false,
            supports_invert: false,
        };

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.capabilities)(handle, &mut ffi_caps, &mut error)
        });

        if let Some(e) = panic_error {
            (vtable.destroy)(handle);
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success {
            // Clean up the handle on error
            (vtable.destroy)(handle);
            return Err(error.into());
        }

        let capabilities = ffi_caps.into();

        debug!("Plugin driver capabilities: {:?}", capabilities);

        Ok(Self {
            plugin,
            handle,
            capabilities,
        })
    }

    /// Get plugin metadata
    pub fn plugin_name(&self) -> &str {
        &self.plugin.metadata().name
    }

    /// Get plugin version
    pub fn plugin_version(&self) -> &str {
        &self.plugin.metadata().version
    }
}

impl DisplayDriver for PluginDriverAdapter {
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
        let vtable = self.plugin.vtable();
        let mut error = LyMonsError::default();

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.init)(self.handle, &mut error)
        });

        if let Some(e) = panic_error {
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success {
            return Err(error.into());
        }

        Ok(())
    }

    fn set_brightness(&mut self, value: u8) -> Result<(), DisplayError> {
        let vtable = self.plugin.vtable();
        let mut error = LyMonsError::default();

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.set_brightness)(self.handle, value, &mut error)
        });

        if let Some(e) = panic_error {
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success {
            return Err(error.into());
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        let vtable = self.plugin.vtable();
        let mut error = LyMonsError::default();

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.flush)(self.handle, &mut error)
        });

        if let Some(e) = panic_error {
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success {
            return Err(error.into());
        }

        Ok(())
    }

    fn clear(&mut self) -> Result<(), DisplayError> {
        let vtable = self.plugin.vtable();
        let mut error = LyMonsError::default();

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.clear)(self.handle, &mut error)
        });

        if let Some(e) = panic_error {
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success {
            return Err(error.into());
        }

        Ok(())
    }

    fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        let vtable = self.plugin.vtable();
        let mut error = LyMonsError::default();

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.write_buffer)(
                self.handle,
                buffer.as_ptr(),
                buffer.len(),
                &mut error
            )
        });

        if let Some(e) = panic_error {
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success {
            return Err(error.into());
        }

        Ok(())
    }

    fn set_invert(&mut self, inverted: bool) -> Result<(), DisplayError> {
        if !self.capabilities.supports_invert {
            return Err(DisplayError::UnsupportedOperation);
        }

        let vtable = self.plugin.vtable();
        let mut error = LyMonsError::default();

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.set_invert)(self.handle, inverted, &mut error)
        });

        if let Some(e) = panic_error {
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success {
            return Err(error.into());
        }

        Ok(())
    }

    fn set_rotation(&mut self, degrees: u16) -> Result<(), DisplayError> {
        if !self.capabilities.supports_rotation {
            return Err(DisplayError::UnsupportedOperation);
        }

        if degrees != 0 && degrees != 90 && degrees != 180 && degrees != 270 {
            return Err(DisplayError::InvalidRotation(degrees));
        }

        let vtable = self.plugin.vtable();
        let mut error = LyMonsError::default();

        let (result, panic_error) = catch_ffi_call(|| {
            (vtable.set_rotation)(self.handle, degrees, &mut error)
        });

        if let Some(e) = panic_error {
            return Err(e.into());
        }

        if result != LyMonsErrorCode::Success {
            return Err(error.into());
        }

        Ok(())
    }
}

impl Drop for PluginDriverAdapter {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            debug!("Destroying plugin driver instance: {:p}", self.handle);

            let vtable = self.plugin.vtable();
            (vtable.destroy)(self.handle);

            self.handle = std::ptr::null_mut();
        }
    }
}

/// Wrap an FFI call with panic safety
///
/// This catches panics that occur in plugin code and converts them
/// to error codes. This prevents plugin panics from unwinding through
/// the FFI boundary, which is undefined behavior.
///
/// Returns (error_code, error_info)
fn catch_ffi_call<F>(f: F) -> (LyMonsErrorCode, Option<LyMonsError>)
where
    F: FnOnce() -> LyMonsErrorCode,
{
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(code) => (code, None),
        Err(panic_info) => {
            let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                format!("Plugin panic: {}", s)
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                format!("Plugin panic: {}", s)
            } else {
                "Plugin panic: unknown error".to_string()
            };

            error!("Caught panic in plugin FFI call: {}", message);
            let panic_error = LyMonsError::new(LyMonsErrorCode::ErrorPanic, &message);
            (LyMonsErrorCode::ErrorPanic, Some(panic_error))
        }
    }
}

// Safety: The plugin handle is only accessed through the vtable functions,
// which are thread-safe if the plugin implementation is thread-safe.
// The Send trait allows the adapter to be moved between threads.
unsafe impl Send for PluginDriverAdapter {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catch_ffi_call_success() {
        let (result, panic_error) = catch_ffi_call(|| LyMonsErrorCode::Success);
        assert_eq!(result, LyMonsErrorCode::Success);
        assert!(panic_error.is_none());
    }

    #[test]
    fn test_catch_ffi_call_panic() {
        let (result, panic_error) = catch_ffi_call(|| panic!("Test panic"));
        assert_eq!(result, LyMonsErrorCode::ErrorPanic);
        assert!(panic_error.is_some());
        let error = panic_error.unwrap();
        assert!(error.message_str().contains("Plugin panic"));
    }
}
