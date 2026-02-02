/*
 *  display/plugin/mod.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Dynamic plugin system for display drivers
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

//! Dynamic plugin system for LyMonS display drivers
//!
//! This module provides infrastructure for loading display drivers as dynamic
//! plugins (.so/.dll files) at runtime. This enables:
//!
//! - No recompilation needed to add/test new drivers
//! - Plugins distributed independently as separate files
//! - Runtime driver loading from filesystem
//! - Backward compatibility with existing static drivers
//!
//! ## Architecture
//!
//! The plugin system consists of three layers:
//!
//! 1. **FFI Layer** (`ffi.rs`) - C ABI types for stable plugin interface
//! 2. **Loader** (`loader.rs`) - Discovers and loads .so/.dll files
//! 3. **Adapter** (`adapter.rs`) - Wraps C ABI plugins as Rust trait objects
//!
//! ## Plugin Discovery
//!
//! Plugins are searched in the following locations (in priority order):
//!
//! 1. `$LYMONS_DRIVER_PATH` (environment variable)
//! 2. `./target/release/drivers/` (development)
//! 3. `~/.local/lib/lymons/drivers/` (user-local)
//! 4. `~/.lymons/drivers/` (user-local alt)
//! 5. `/usr/local/lib/lymons/drivers/` (system)
//! 6. `/usr/lib/lymons/drivers/` (system)
//!
//! ## Plugin Naming Convention
//!
//! - Linux: `liblymons_ssd1306.so` or `liblymons-ssd1306.so`
//! - macOS: `liblymons_ssd1306.dylib`
//! - Windows: `lymons_ssd1306.dll`

pub mod ffi;
pub mod loader;
pub mod adapter;

// Re-exports for convenience
pub use ffi::{
    LyMonsPluginVTable,
    LyMonsDriverHandle,
    LyMonsErrorCode,
    LyMonsError,
    LyMonsDisplayConfig,
    LyMonsDisplayCapabilities,
    LyMonsBusConfig,
    LyMonsBusType,
    LyMonsColorDepth,
};

pub use loader::{PluginLoader, LoadedPlugin};
pub use adapter::PluginDriverAdapter;
