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

#![allow(dead_code)] // color system types; variants reserved for future display color depth support

use embedded_graphics::pixelcolor::{BinaryColor, Gray4, Rgb565, RgbColor};
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

    /// Cyan (bright blue-green, maps to LightGray on grayscale displays)
    Cyan,

    /// Green (maps to Gray on grayscale displays)
    Green,

    /// Yellow (bright, maps to LightGray on grayscale displays)
    Yellow,

    /// Red (maps to DarkGray on grayscale displays)
    Red,

    /// Blue (maps to DarkGray on grayscale displays)
    Blue,

    /// Orange (maps to Gray on grayscale displays)
    Orange,

    /// Magenta (maps to Gray on grayscale displays)
    Magenta,

    /// Custom grayscale value (0-255)
    Grayscale(u8),

    /// Arbitrary 24-bit RGB — full gamut on Rgb565 displays, luminance-mapped on others
    Rgb(u8, u8, u8),
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
            Color::Cyan => BinaryColor::On, // Cyan is a bright color
            Color::Green => BinaryColor::On, // Green is visible, maps to On
            Color::Yellow  => BinaryColor::On,
            Color::Red     => BinaryColor::Off, // low luminance
            Color::Blue    => BinaryColor::Off, // low luminance
            Color::Orange  => BinaryColor::On,  // bright warm
            Color::Magenta => BinaryColor::On,  // mid-bright
            Color::Grayscale(val) => if *val >= 128 { BinaryColor::On } else { BinaryColor::Off },
            Color::Rgb(r, g, b) => {
                let lum = (*r as u16 * 77 + *g as u16 * 150 + *b as u16 * 29) >> 8;
                if lum >= 128 { BinaryColor::On } else { BinaryColor::Off }
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
            Color::Cyan => Gray4::new(11), // Map to LightGray (bright but not full white)
            Color::Green => Gray4::new(8), // Map to Gray (medium brightness, ~50%)
            Color::Yellow  => Gray4::new(12),
            Color::Red     => Gray4::new(4),   // dark — low luminance
            Color::Blue    => Gray4::new(3),   // darkest — minimal luminance
            Color::Orange  => Gray4::new(10),  // warm mid-bright
            Color::Magenta => Gray4::new(8),   // mid gray
            Color::Grayscale(val) => Gray4::new(((*val as u16 * 15) / 255) as u8),
            Color::Rgb(r, g, b) => {
                let lum = (*r as u16 * 77 + *g as u16 * 150 + *b as u16 * 29) >> 8;
                Gray4::new(((lum * 15) / 255) as u8)
            }
        }
    }

    /// Convert to Rgb565 for full-colour displays
    pub fn to_rgb565(&self) -> Rgb565 {
        match self {
            Color::Black          => Rgb565::new(0,   0,  0),
            Color::DarkGray       => Rgb565::new(10,  21, 10),
            Color::Gray           => Rgb565::new(15,  31, 15),
            Color::LightGray      => Rgb565::new(22,  44, 22),
            Color::White          => Rgb565::new(31,  63, 31),
            Color::Cyan           => Rgb565::new(0,   63, 31),
            Color::Green          => Rgb565::new(0,   63, 0),
            Color::Yellow         => Rgb565::new(31,  63,  0),
            Color::Red            => Rgb565::new(31,   0,  0),
            Color::Blue           => Rgb565::new(0,    0, 31),
            Color::Orange         => Rgb565::new(31,  40,  0),
            Color::Magenta        => Rgb565::new(31,   0, 31),
            Color::Grayscale(val) => {
                let v5 = (*val >> 3) as u8;
                let v6 = (*val >> 2) as u8;
                Rgb565::new(v5, v6, v5)
            }
            Color::Rgb(r, g, b)   => Rgb565::new(*r >> 3, *g >> 2, *b >> 3),
        }
    }

    /// Convert to appropriate color based on display color depth
    pub fn to_color_depth(&self, depth: ColorDepth) -> ColorValue {
        match depth {
            ColorDepth::Monochrome => ColorValue::Binary(self.to_binary()),
            ColorDepth::Gray4 => ColorValue::Gray4(self.to_gray4()),
            ColorDepth::Rgb565 => ColorValue::Rgb565(self.to_rgb565()),
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
            Color::Cyan => 180, // Bright cyan (~70% luminance)
            Color::Green => 128, // Green (~50% luminance)
            Color::Yellow  => 200,
            Color::Red     => 76,
            Color::Blue    => 29,
            Color::Orange  => 166,
            Color::Magenta => 105,
            Color::Grayscale(val) => *val,
            Color::Rgb(r, g, b) => {
                ((*r as u16 * 77 + *g as u16 * 150 + *b as u16 * 29) >> 8) as u8
            }
        }
    }
}

/// Concrete color value for a specific color depth
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorValue {
    Binary(BinaryColor),
    Gray4(Gray4),
    Rgb565(Rgb565),
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
            ColorValue::Rgb565(c) => {
                // Scale R(0-31)×8, G(0-63)×4, B(0-31)×8 → sum 0-748, /3 → 0-249
                let lum = (c.r() as u16 * 8 + c.g() as u16 * 4 + c.b() as u16 * 8) / 3;
                if lum >= 128 { BinaryColor::On } else { BinaryColor::Off }
            }
        }
    }

    /// Get as Gray4 (converts if needed)
    pub fn as_gray4(&self) -> Gray4 {
        match self {
            ColorValue::Binary(c) => {
                if c.is_on() { Gray4::new(15) } else { Gray4::new(0) }
            }
            ColorValue::Gray4(c) => *c,
            ColorValue::Rgb565(c) => {
                // Scale to 0-249 then map to 0-15
                let lum = (c.r() as u16 * 8 + c.g() as u16 * 4 + c.b() as u16 * 8) / 3;
                Gray4::new(((lum as u32 * 15) / 249) as u8)
            }
        }
    }

    /// Get as Rgb565 (converts if needed)
    pub fn as_rgb565(&self) -> Rgb565 {
        match self {
            ColorValue::Binary(c) => if c.is_on() { Rgb565::WHITE } else { Rgb565::BLACK },
            ColorValue::Gray4(c) => {
                let val = unsafe { std::mem::transmute::<Gray4, u8>(*c) };
                let v5 = (val * 2 + 1).min(31);
                let v6 = (val * 4 + 2).min(63);
                Rgb565::new(v5, v6, v5)
            }
            ColorValue::Rgb565(c) => *c,
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
