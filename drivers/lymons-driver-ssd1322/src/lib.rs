/*
 *  LyMonS SSD1322 Plugin
 */

//! # LyMonS SSD1322 Display Driver Plugin
//!
//! 256x64 grayscale (4-bit) OLED display support via SPI

mod ffi;
mod plugin;

pub use plugin::lymons_plugin_register;
