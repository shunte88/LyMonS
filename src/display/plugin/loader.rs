/*
 *  display/plugin/loader.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Plugin loader - discovers and loads .so/.dll files
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

use std::path::{Path, PathBuf};
use std::ffi::CStr;
use log::{debug, info, warn};
use libloading::{Library, Symbol};

use super::ffi::{
    LyMonsPluginVTable,
    PluginRegisterFn,
    LYMONS_PLUGIN_ABI_VERSION_MAJOR,
    LYMONS_PLUGIN_ABI_VERSION_MINOR,
    LYMONS_PLUGIN_ABI_VERSION_PATCH,
    LYMONS_PLUGIN_NAME_SIZE,
    LYMONS_PLUGIN_VERSION_SIZE,
    LYMONS_PLUGIN_DRIVER_TYPE_SIZE,
};

/// Plugin metadata extracted from the plugin
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Plugin name (e.g., "LyMonS SSD1306 Driver")
    pub name: String,

    /// Plugin version (e.g., "1.0.0")
    pub version: String,

    /// Driver type (e.g., "ssd1306")
    pub driver_type: String,

    /// ABI version (major, minor, patch)
    pub abi_version: (u32, u32, u32),
}

/// A loaded plugin with its library and vtable
pub struct LoadedPlugin {
    /// The loaded shared library (must be kept alive)
    #[allow(dead_code)]
    library: Library,

    /// Plugin vtable
    vtable: &'static LyMonsPluginVTable,

    /// Plugin metadata
    metadata: PluginMetadata,
}

impl LoadedPlugin {
    /// Get the plugin vtable
    pub fn vtable(&self) -> &'static LyMonsPluginVTable {
        self.vtable
    }

    /// Get plugin metadata
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
}

/// Plugin loader - searches for and loads display driver plugins
pub struct PluginLoader;

impl PluginLoader {
    /// Get the search paths for plugins in priority order
    pub fn search_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Environment variable override
        if let Ok(path) = std::env::var("LYMONS_DRIVER_PATH") {
            paths.push(PathBuf::from(path));
        }

        // 2. Development directory (relative to cwd)
        paths.push(PathBuf::from("./target/release/drivers"));

        // 3. User-local directories
        if let Some(home) = dirs_next::home_dir() {
            paths.push(home.join(".local/lib/lymons/drivers"));
            paths.push(home.join(".lymons/drivers"));
        }

        // 4. System directories
        paths.push(PathBuf::from("/usr/local/lib/lymons/drivers"));
        paths.push(PathBuf::from("/usr/lib/lymons/drivers"));

        paths
    }

    /// Get possible plugin filenames for a driver type
    ///
    /// For example, for "ssd1306" this returns:
    /// - Linux: ["liblymons_ssd1306.so", "liblymons-ssd1306.so"]
    /// - macOS: ["liblymons_ssd1306.dylib", "liblymons-ssd1306.dylib"]
    /// - Windows: ["lymons_ssd1306.dll", "lymons-ssd1306.dll"]
    pub fn plugin_filenames(driver_type: &str) -> Vec<String> {
        let mut names = Vec::new();

        #[cfg(target_os = "linux")]
        {
            names.push(format!("liblymons_{}.so", driver_type));
            names.push(format!("liblymons-{}.so", driver_type));
        }

        #[cfg(target_os = "macos")]
        {
            names.push(format!("liblymons_{}.dylib", driver_type));
            names.push(format!("liblymons-{}.dylib", driver_type));
        }

        #[cfg(target_os = "windows")]
        {
            names.push(format!("lymons_{}.dll", driver_type));
            names.push(format!("lymons-{}.dll", driver_type));
        }

        names
    }

    /// Find a plugin file for the given driver type
    ///
    /// Returns the path to the plugin if found, or None if not found.
    pub fn find_plugin(driver_type: &str) -> Option<PathBuf> {
        let search_paths = Self::search_paths();
        let filenames = Self::plugin_filenames(driver_type);

        for path in &search_paths {
            if !path.exists() {
                continue;
            }

            for filename in &filenames {
                let plugin_path = path.join(filename);
                if plugin_path.exists() {
                    debug!("Found plugin at: {}", plugin_path.display());
                    return Some(plugin_path);
                }
            }
        }

        debug!("Plugin not found for driver: {}", driver_type);
        None
    }

    /// Load a plugin from a specific path
    ///
    /// This performs the following steps:
    /// 1. Load the shared library
    /// 2. Get the registration function symbol
    /// 3. Call the registration function to get the vtable
    /// 4. Verify ABI version compatibility
    /// 5. Extract plugin metadata
    pub fn load_plugin<P: AsRef<Path>>(path: P) -> Result<LoadedPlugin, String> {
        let path = path.as_ref();
        info!("Loading plugin from: {}", path.display());

        // Load the shared library
        let library = unsafe {
            Library::new(path)
                .map_err(|e| format!("Failed to load library: {}", e))?
        };

        // Get the registration function
        let register_fn: Symbol<PluginRegisterFn> = unsafe {
            library.get(b"lymons_plugin_register\0")
                .map_err(|e| format!("Failed to find registration function: {}", e))?
        };

        // Call the registration function to get the vtable
        let vtable_ptr = register_fn();
        if vtable_ptr.is_null() {
            return Err("Plugin registration returned null vtable".to_string());
        }

        let vtable: &'static LyMonsPluginVTable = unsafe { &*vtable_ptr };

        // Verify ABI version
        let mut major = 0u32;
        let mut minor = 0u32;
        let mut patch = 0u32;

        (vtable.abi_version)(&mut major, &mut minor, &mut patch);

        debug!("Plugin ABI version: {}.{}.{}", major, minor, patch);
        debug!("Host ABI version: {}.{}.{}",
            LYMONS_PLUGIN_ABI_VERSION_MAJOR,
            LYMONS_PLUGIN_ABI_VERSION_MINOR,
            LYMONS_PLUGIN_ABI_VERSION_PATCH
        );

        // Check ABI compatibility
        if major != LYMONS_PLUGIN_ABI_VERSION_MAJOR {
            return Err(format!(
                "ABI version mismatch: plugin {}.{}.{} incompatible with host {}.{}.{}",
                major, minor, patch,
                LYMONS_PLUGIN_ABI_VERSION_MAJOR,
                LYMONS_PLUGIN_ABI_VERSION_MINOR,
                LYMONS_PLUGIN_ABI_VERSION_PATCH
            ));
        }

        if minor > LYMONS_PLUGIN_ABI_VERSION_MINOR {
            warn!("Plugin has newer minor version {}.{}.{} than host {}.{}.{} - may have extra features",
                major, minor, patch,
                LYMONS_PLUGIN_ABI_VERSION_MAJOR,
                LYMONS_PLUGIN_ABI_VERSION_MINOR,
                LYMONS_PLUGIN_ABI_VERSION_PATCH
            );
        }

        // Extract plugin metadata
        let mut name_buf = vec![0i8; LYMONS_PLUGIN_NAME_SIZE];
        let mut version_buf = vec![0i8; LYMONS_PLUGIN_VERSION_SIZE];
        let mut driver_type_buf = vec![0i8; LYMONS_PLUGIN_DRIVER_TYPE_SIZE];

        (vtable.plugin_info)(
            name_buf.as_mut_ptr(),
            version_buf.as_mut_ptr(),
            driver_type_buf.as_mut_ptr()
        );

        let name = Self::extract_string(&name_buf);
        let version = Self::extract_string(&version_buf);
        let driver_type = Self::extract_string(&driver_type_buf);

        info!("Loaded plugin: {} v{} ({})", name, version, driver_type);

        let metadata = PluginMetadata {
            name,
            version,
            driver_type,
            abi_version: (major, minor, patch),
        };

        Ok(LoadedPlugin {
            library,
            vtable,
            metadata,
        })
    }

    /// Load a plugin by driver type
    ///
    /// This searches for the plugin in the standard search paths
    /// and loads it if found.
    pub fn load_by_driver_type(driver_type: &str) -> Result<LoadedPlugin, String> {
        let path = Self::find_plugin(driver_type)
            .ok_or_else(|| format!("Plugin not found for driver: {}", driver_type))?;

        Self::load_plugin(path)
    }

    /// Extract a null-terminated string from a C buffer
    fn extract_string(buffer: &[i8]) -> String {
        let len = buffer.iter()
            .position(|&c| c == 0)
            .unwrap_or(buffer.len());

        let bytes: Vec<u8> = buffer[..len]
            .iter()
            .map(|&c| c as u8)
            .collect();

        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_paths() {
        let paths = PluginLoader::search_paths();
        assert!(!paths.is_empty());

        // Should always have at least the development and system paths
        assert!(paths.iter().any(|p| p.to_string_lossy().contains("target/release/drivers")));
    }

    #[test]
    fn test_plugin_filenames() {
        let names = PluginLoader::plugin_filenames("ssd1306");
        assert!(!names.is_empty());

        #[cfg(target_os = "linux")]
        {
            assert!(names.contains(&"liblymons_ssd1306.so".to_string()));
            assert!(names.contains(&"liblymons-ssd1306.so".to_string()));
        }

        #[cfg(target_os = "windows")]
        {
            assert!(names.contains(&"lymons_ssd1306.dll".to_string()));
        }
    }
}
