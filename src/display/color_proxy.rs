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

use embedded_graphics::pixelcolor::{BinaryColor, Gray4, GrayColor};

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

// Extend ConvertColor to work with Color enum
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
    type Output;

    /// Convert a palette color based on display capabilities
    fn proxy(color: Pal16) -> Self::Output;

    /// Get the "on" color (white/max brightness)
    fn on() -> Self::Output;

    /// Get the "off" color (black/min brightness)
    fn off() -> Self::Output;
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
