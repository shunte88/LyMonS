/*
 *  display/color.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Universal color system that adapts to driver capabilities
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

use embedded_graphics::pixelcolor::{BinaryColor, Gray4};
use super::traits::ColorDepth;

/// Universal color value that adapts to display capabilities
///
/// This allows defining colors once in field layouts, then automatically
/// converting to the appropriate format based on driver capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Black/Off (0% intensity)
    Black,

    /// Dark gray (33% intensity)
    DarkGray,

    /// Gray (50% intensity)
    Gray,

    /// Light gray (67% intensity)
    LightGray,

    /// White/On (100% intensity)
    White,

    /// Custom grayscale value (0-255)
    Grayscale(u8),
}

impl Color {
    /// Convert to BinaryColor for monochrome displays
    pub fn to_binary(&self) -> BinaryColor {
        match self {
            Color::Black => BinaryColor::Off,
            Color::White => BinaryColor::On,
            // Threshold at 50% for other colors
            Color::Gray => BinaryColor::On,
            Color::DarkGray => BinaryColor::Off,
            Color::LightGray => BinaryColor::On,
            Color::Grayscale(val) => {
                if *val >= 128 {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                }
            }
        }
    }

    /// Convert to Gray4 (4-bit grayscale: 0-15)
    pub fn to_gray4(&self) -> Gray4 {
        match self {
            Color::Black => Gray4::new(0),
            Color::DarkGray => Gray4::new(5),
            Color::Gray => Gray4::new(8),
            Color::LightGray => Gray4::new(11),
            Color::White => Gray4::new(15),
            Color::Grayscale(val) => Gray4::new(((*val as u16 * 15) / 255) as u8),
        }
    }

    /// Convert to appropriate color based on display color depth
    pub fn to_color_depth(&self, depth: ColorDepth) -> ColorValue {
        match depth {
            ColorDepth::Monochrome => ColorValue::Binary(self.to_binary()),
            ColorDepth::Gray4 => ColorValue::Gray4(self.to_gray4()),
        }
    }

    /// Get luminance value (0-255)
    pub fn luminance(&self) -> u8 {
        match self {
            Color::Black => 0,
            Color::DarkGray => 85,
            Color::Gray => 128,
            Color::LightGray => 170,
            Color::White => 255,
            Color::Grayscale(val) => *val,
        }
    }
}

/// Concrete color value for a specific color depth
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorValue {
    Binary(BinaryColor),
    Gray4(Gray4),
}

impl ColorValue {
    /// Get as BinaryColor (converts if needed)
    pub fn as_binary(&self) -> BinaryColor {
        match self {
            ColorValue::Binary(c) => *c,
            ColorValue::Gray4(c) => {
                // Gray4 values are 0-15, threshold at 8 (50%)
                let val = unsafe { std::mem::transmute::<Gray4, u8>(*c) };
                if val >= 8 {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                }
            }
        }
    }

    /// Get as Gray4 (converts if needed)
    pub fn as_gray4(&self) -> Gray4 {
        match self {
            ColorValue::Binary(c) => {
                if c.is_on() {
                    Gray4::new(15)
                } else {
                    Gray4::new(0)
                }
            }
            ColorValue::Gray4(c) => *c,
        }
    }
}

/// Common color presets
impl Color {
    pub const OFF: Color = Color::Black;
    pub const ON: Color = Color::White;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_conversion() {
        assert_eq!(Color::Black.to_binary(), BinaryColor::Off);
        assert_eq!(Color::White.to_binary(), BinaryColor::On);
        assert_eq!(Color::Grayscale(64).to_binary(), BinaryColor::Off);
        assert_eq!(Color::Grayscale(192).to_binary(), BinaryColor::On);
    }

    #[test]
    fn test_gray4_conversion() {
        assert_eq!(Color::Black.to_gray4(), Gray4::new(0));
        assert_eq!(Color::White.to_gray4(), Gray4::new(15));
        assert_eq!(Color::Gray.to_gray4(), Gray4::new(8));
    }

    #[test]
    fn test_luminance() {
        assert_eq!(Color::Black.luminance(), 0);
        assert_eq!(Color::White.luminance(), 255);
        assert_eq!(Color::Gray.luminance(), 128);
    }

    #[test]
    fn test_color_depth_conversion() {
        let white = Color::White;
        assert_eq!(white.to_color_depth(ColorDepth::Monochrome), ColorValue::Binary(BinaryColor::On));
        assert_eq!(white.to_color_depth(ColorDepth::Gray4), ColorValue::Gray4(Gray4::new(15)));
    }
}
