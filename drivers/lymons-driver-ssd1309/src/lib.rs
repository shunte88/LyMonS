/*
 *  LyMonS SSD1309 Plugin
 *
 *  SSD1309 OLED display driver plugin for LyMonS
 */

//! # LyMonS SSD1309 Display Driver Plugin
//!
//! 128x64 monochrome OLED display support via I2C

mod ffi;
mod plugin;

pub use plugin::lymons_plugin_register;
