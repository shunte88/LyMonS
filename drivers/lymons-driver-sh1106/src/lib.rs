/*
 *  LyMonS SH1106 Plugin
 */

//! # LyMonS SH1106 Display Driver Plugin
//!
//! 132x64 monochrome OLED display support via I2C

mod ffi;
mod plugin;

pub use plugin::lymons_plugin_register;
