/*
 *  const_oled.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-25 Stuart Hunter
 *
 *	TODO:
 *
 *	This program is free software: you can redistribute it and/or modify
 *	it under the terms of the GNU General Public License as published by
 *	the Free Software Foundation, either version 3 of the License, or
 *	(at your option) any later version.
 *
 *	This program is distributed in the hope that it will be useful,
 *	but WITHOUT ANY WARRANTY; without even the implied warranty of
 *	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *	GNU General Public License for more details.
 *
 *	See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *	Public License.
 *
 */

#[allow(dead_code)]
 /// The total width of the SSD1309 OLED display in pixels.
pub const DISPLAY_WIDTH_1309: u32 = 128;
/// The total height of the OLED display in pixels.
#[allow(dead_code)]
pub const DISPLAY_HEIGHT_1309: u32 = 64;

/// The total width of the SSD1322 OLED display in pixels.
#[allow(dead_code)]
pub const DISPLAY_WIDTH_1322: u32 = 256;
/// The total height of the OLED display in pixels.
#[allow(dead_code)]
pub const DISPLAY_HEIGHT_1322: u32 = 64;

// ??? 420 x 128 ???

// why is this not an enum???
#[allow(dead_code)]
pub const OLED_ADAFRUIT_SPI_128X32: u8 = 0;
#[allow(dead_code)]
pub const OLED_ADAFRUIT_SPI_128X64: u8 = 1;
#[allow(dead_code)]
pub const OLED_ADAFRUIT_I2C_128X32: u8 = 2;
#[allow(dead_code)]
pub const OLED_ADAFRUIT_I2C_128X64: u8 = 3;
#[allow(dead_code)]
pub const OLED_SEEED_I2C_128X64: u8 = 4;
#[allow(dead_code)]
pub const OLED_SEEED_I2C_96X96: u8 = 5;
#[allow(dead_code)]
pub const OLED_SH1106_I2C_128X64: u8 = 6;
#[allow(dead_code)]
pub const OLED_SH1106_SPI_128X64: u8 = 7;
#[allow(dead_code)]
pub const OLED_SSD1322G_SPI_256X64: u8 = 8;
#[allow(dead_code)]
pub const OLED_SSD1322M_SPI_256X64: u8 = 9;

#[allow(dead_code)]
pub const OLED_LAST_OLED: u8 = 10; // always last type, used in code to end array

#[allow(dead_code)]
pub const OLED_TYPE_STR: [&str; OLED_LAST_OLED as usize] = [
    "Adafruit SPI 128x32",
    "Adafruit SPI 128x64", 
    "Adafruit I2C 128x32",
    "Adafruit I2C 128x64",
    "Seeed I2C 128x64",
    "Seeed I2C 96x96",
    "SH1106 I2C 128x64",
    "SH1106 SPI 128x64",
    "SSD1322 Gray SPI 256x64",
    "SSD1322 Mono SPI 256x64",
];

