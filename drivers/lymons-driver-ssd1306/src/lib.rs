/*
 *  LyMonS SSD1306 Plugin
 *
 *  A dynamic plugin for the LyMonS monitoring system that provides
 *  SSD1306 OLED display driver support via the plugin system.
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 */

//! # LyMonS SSD1306 Display Driver Plugin
//!
//! This plugin provides SSD1306 OLED display driver support for LyMonS.
//!
//! ## Features
//!
//! - 128x64 monochrome OLED display support
//! - I2C communication
//! - Brightness control (4 levels)
//! - Display rotation (0, 90, 180, 270 degrees)
//! - Display inversion
//! - Buffered graphics mode
//!
//! ## Hardware Support
//!
//! - SSD1306 128x64 OLED displays
//! - I2C interface
//! - Typical I2C addresses: 0x3C, 0x3D
//!
//! ## Usage
//!
//! This plugin is loaded automatically by LyMonS when configured with:
//!
//! ```yaml
//! display:
//!   driver: ssd1306
//!   bus:
//!     type: i2c
//!     bus: "/dev/i2c-1"
//!     address: 0x3C
//! ```

mod ffi;
mod plugin;

// Re-export the plugin registration function
pub use plugin::lymons_plugin_register;
