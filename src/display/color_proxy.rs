/*
 *  display/color_proxy.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Color proxy pattern - simple color depth abstraction
 *  "color in -> if mono then white else color out"
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

#![allow(dead_code)] // color proxy helpers; some conversion methods reserved

use embedded_graphics::pixelcolor::{BinaryColor, Gray4, GrayColor, Rgb565, RgbColor};
use embedded_graphics::prelude::PixelColor;

/// Convert BinaryColor to any PixelColor type via ColorProxy
pub trait ConvertColor<C> {
    fn to_color(self) -> C;
}

impl ConvertColor<BinaryColor> for BinaryColor {
    fn to_color(self) -> BinaryColor {
        self
    }
}

impl ConvertColor<Gray4> for BinaryColor {
    fn to_color(self) -> Gray4 {
        match self {
            BinaryColor::Off => Gray4::BLACK,
            BinaryColor::On => Gray4::WHITE,
        }
    }
}

//  Extend ConvertColor to work with Color enum
impl ConvertColor<BinaryColor> for crate::display::color::Color {
    fn to_color(self) -> BinaryColor {
        self.to_binary()
    }
}

impl ConvertColor<Gray4> for crate::display::color::Color {
    fn to_color(self) -> Gray4 {
        self.to_gray4()
    }
}

/// 16-color palette for 4bpp grayscale - it's like 1990 baby!
/// Simple 4-bit color palette compatible with grayscale displays
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pal16 {
    Black = 0x0,        // Black
    Blue = 0x1,         // Blue
    Green = 0x2,        // Green
    Cyan = 0x3,         // Cyan
    Red = 0x4,          // Red
    Magenta = 0x5,      // Magenta
    Brown = 0x6,        // Brown
    Gray = 0x7,         // Gray
    DarkGray = 0x8,     // Dark Gray
    LightBlue = 0x9,    // Light blue
    LightGreen = 0xA,   // Light green
    LightCyan = 0xB,    // Light cyan
    LightRed = 0xC,     // Light red
    LightMagenta = 0xD, // Light magenta
    Yellow = 0xE,       // Yellow
    White = 0xF,        // White
}

impl Pal16 {
    /// Convert palette color to grayscale value (0-15)
    pub fn to_gray4(self) -> Gray4 {
        Gray4::new(self as u8)
    }

    /// Convert palette color to binary (threshold at mid-gray)
    pub fn to_binary(self) -> BinaryColor {
        if (self as u8) >= 0x8 {
            BinaryColor::On
        } else {
            BinaryColor::Off
        }
    }
}

/// Color proxy trait - abstracts color depth conversion
/// "color in -> if mono then white else color out"
pub trait ColorProxy {
    type Output: PixelColor + Default;

    /// Convert a palette color based on display capabilities
    fn proxy(color: Pal16) -> Self::Output;

    /// Get the "on" color (white/max brightness)
    fn on() -> Self::Output;

    /// Get the "off" color (black/min brightness)
    fn off() -> Self::Output;

    /// Map a 0-255 intensity value to a pixel color.
    /// For monochrome: threshold at 128. For Gray4: map to 0-15.
    fn spectrum_pixel(intensity: u8) -> Self::Output;
}

/// Color proxy for monochrome displays
pub struct MonoProxy;

impl ColorProxy for MonoProxy {
    type Output = BinaryColor;

    fn proxy(color: Pal16) -> BinaryColor {
        color.to_binary()
    }

    fn on() -> BinaryColor {
        BinaryColor::On
    }

    fn off() -> BinaryColor {
        BinaryColor::Off
    }

    fn spectrum_pixel(intensity: u8) -> BinaryColor {
        if intensity > 128 { BinaryColor::On } else { BinaryColor::Off }
    }
}

/// Color proxy for 4-bit grayscale displays
pub struct Gray4Proxy;

impl ColorProxy for Gray4Proxy {
    type Output = Gray4;

    fn proxy(color: Pal16) -> Gray4 {
        color.to_gray4()
    }

    fn on() -> Gray4 {
        Gray4::WHITE
    }

    fn off() -> Gray4 {
        Gray4::BLACK
    }

    fn spectrum_pixel(intensity: u8) -> Gray4 {
        Gray4::new((intensity as u32 * 15 / 255) as u8)
    }
}

/// Color proxy for 16-bit full-colour (Rgb565) displays
pub struct Rgb565Proxy;

impl ColorProxy for Rgb565Proxy {
    type Output = Rgb565;

    fn proxy(color: Pal16) -> Rgb565 {
        color.to_rgb565()
    }

    fn on() -> Rgb565 {
        Rgb565::WHITE
    }

    fn off() -> Rgb565 {
        Rgb565::BLACK
    }

    fn spectrum_pixel(intensity: u8) -> Rgb565 {
        // Map 0-255 intensity to a blue→cyan→green→yellow→red gradient
        // not getting much color spread so attempting to push a tad
        // not a fill spectrim spread but a little more interesting
        let i = (intensity as u16 * 6).clamp(0, 255);
        let (r, g, b) = if i < 64 {
            (0u8, 0u8, ((i * 255) / 63) as u8)
        } else if i < 128 {
            (0u8, (((i - 64) * 255) / 63) as u8, 255u8)
        } else if i < 192 {
            (0u8, 255u8, (255 - ((i - 128) * 255) / 63) as u8)
        } else {
            ((((i - 192) * 255) / 63) as u8, 255u8, 0u8)
        };
        Rgb565::new(r >> 3, g >> 2, b >> 3)
    }
}

impl Pal16 {
    /// Convert palette colour to Rgb565
    pub fn to_rgb565(self) -> Rgb565 {
        match self {
            Pal16::Black       => Rgb565::new(0,   0,  0),
            Pal16::Blue        => Rgb565::new(0,   0,  31),
            Pal16::Green       => Rgb565::new(0,   63, 0),
            Pal16::Cyan        => Rgb565::new(0,   63, 31),
            Pal16::Red         => Rgb565::new(31,  0,  0),
            Pal16::Magenta     => Rgb565::new(31,  0,  31),
            Pal16::Brown       => Rgb565::new(16,  20, 0),
            Pal16::Gray        => Rgb565::new(15,  31, 15),
            Pal16::DarkGray    => Rgb565::new(8,   16, 8),
            Pal16::LightBlue   => Rgb565::new(8,   24, 31),
            Pal16::LightGreen  => Rgb565::new(8,   63, 8),
            Pal16::LightCyan   => Rgb565::new(8,   63, 31),
            Pal16::LightRed    => Rgb565::new(31,  16, 8),
            Pal16::LightMagenta=> Rgb565::new(31,  16, 31),
            Pal16::Yellow      => Rgb565::new(31,  63, 0),
            Pal16::White       => Rgb565::new(31,  63, 31),
        }
    }
}

impl ConvertColor<Rgb565> for BinaryColor {
    fn to_color(self) -> Rgb565 {
        match self {
            BinaryColor::Off => Rgb565::BLACK,
            BinaryColor::On  => Rgb565::WHITE,
        }
    }
}

impl ConvertColor<Rgb565> for crate::display::color::Color {
    fn to_color(self) -> Rgb565 {
        self.to_rgb565()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pal16_grayscale() {
        assert_eq!(Pal16::Black.to_gray4(), Gray4::new(0));
        assert_eq!(Pal16::White.to_gray4(), Gray4::new(15));
        assert_eq!(Pal16::Gray.to_gray4(), Gray4::new(7));
    }

    #[test]
    fn test_pal16_binary() {
        assert_eq!(Pal16::Black.to_binary(), BinaryColor::Off);
        assert_eq!(Pal16::White.to_binary(), BinaryColor::On);
        assert_eq!(Pal16::Gray.to_binary(), BinaryColor::Off); // Below threshold
        assert_eq!(Pal16::DarkGray.to_binary(), BinaryColor::On); // At threshold
    }

    #[test]
    fn test_mono_proxy() {
        assert_eq!(MonoProxy::proxy(Pal16::White), BinaryColor::On);
        assert_eq!(MonoProxy::proxy(Pal16::Black), BinaryColor::Off);
        assert_eq!(MonoProxy::on(), BinaryColor::On);
        assert_eq!(MonoProxy::off(), BinaryColor::Off);
    }

    #[test]
    fn test_gray4_proxy() {
        assert_eq!(Gray4Proxy::proxy(Pal16::White), Gray4::WHITE);
        assert_eq!(Gray4Proxy::proxy(Pal16::Black), Gray4::BLACK);
        assert_eq!(Gray4Proxy::on(), Gray4::WHITE);
        assert_eq!(Gray4Proxy::off(), Gray4::BLACK);
    }
}
