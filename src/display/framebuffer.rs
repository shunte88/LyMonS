/*
 *  display/framebuffer.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Framebuffer abstraction with enum dispatch for different color types
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

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::{BinaryColor, Gray4};
use crate::vframebuf::VarFrameBuf;
use crate::display::traits::{DisplayCapabilities, ColorDepth};

/// Enum dispatch for zero-cost color abstraction
///
/// This enum allows us to support different color types (monochrome, grayscale)
/// without runtime overhead. The correct variant is selected at initialization
/// time based on the display capabilities.
pub enum FrameBuffer {
    /// Monochrome framebuffer (1-bit per pixel)
    Mono(VarFrameBuf<BinaryColor>),

    /// 4-bit grayscale framebuffer (16 levels)
    Gray4(VarFrameBuf<Gray4>),
}

impl FrameBuffer {
    /// Create a new framebuffer based on display capabilities
    pub fn new(capabilities: &DisplayCapabilities) -> Self {
        match capabilities.color_depth {
            ColorDepth::Monochrome => {
                FrameBuffer::Mono(VarFrameBuf::new(
                    capabilities.width,
                    capabilities.height,
                    BinaryColor::Off,
                ))
            }
            ColorDepth::Gray4 => {
                FrameBuffer::Gray4(VarFrameBuf::new(
                    capabilities.width,
                    capabilities.height,
                    Gray4::new(0),
                ))
            }
        }
    }

    /// Get dimensions as (width, height)
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            FrameBuffer::Mono(fb) => (fb.width() as u32, fb.height() as u32),
            FrameBuffer::Gray4(fb) => (fb.width() as u32, fb.height() as u32),
        }
    }

    /// Clear the framebuffer
    pub fn clear(&mut self) {
        match self {
            FrameBuffer::Mono(fb) => fb.clear(BinaryColor::Off).ok(),
            FrameBuffer::Gray4(fb) => fb.clear(Gray4::new(0)).ok(),
        };
    }

    /// Get mutable reference to monochrome framebuffer
    ///
    /// Panics if the framebuffer is not monochrome. Use this only when
    /// you've verified the color depth.
    pub fn as_mono_mut(&mut self) -> &mut VarFrameBuf<BinaryColor> {
        match self {
            FrameBuffer::Mono(fb) => fb,
            _ => panic!("Framebuffer is not monochrome"),
        }
    }

    /// Get immutable reference to monochrome framebuffer
    pub fn as_mono(&self) -> Option<&VarFrameBuf<BinaryColor>> {
        match self {
            FrameBuffer::Mono(fb) => Some(fb),
            _ => None,
        }
    }

    /// Convert framebuffer to packed byte array for write_buffer()
    ///
    /// Returns a Vec<u8> with pixels packed according to color depth:
    /// - Monochrome: 8 pixels per byte (LSB first)
    /// - Gray4: 2 pixels per byte (high nibble first)
    pub fn to_packed_bytes(&self) -> Vec<u8> {
        match self {
            FrameBuffer::Mono(fb) => {
                let pixels = fb.as_slice();
                let num_bytes = (pixels.len() + 7) / 8;  // Round up
                let mut bytes = vec![0u8; num_bytes];

                for (i, &pixel) in pixels.iter().enumerate() {
                    let byte_idx = i / 8;
                    let bit_idx = i % 8;
                    if pixel.is_on() {
                        bytes[byte_idx] |= 1 << bit_idx;
                    }
                }

                bytes
            }
            FrameBuffer::Gray4(fb) => {
                let pixels = fb.as_slice();
                let num_bytes = (pixels.len() + 1) / 2;  // Round up
                let mut bytes = vec![0u8; num_bytes];

                for (i, &pixel) in pixels.iter().enumerate() {
                    let byte_idx = i / 2;
                    let value = pixel.luma();
                    if i % 2 == 0 {
                        // High nibble
                        bytes[byte_idx] |= (value & 0x0F) << 4;
                    } else {
                        // Low nibble
                        bytes[byte_idx] |= value & 0x0F;
                    }
                }

                bytes
            }
        }
    }

    /// Get mutable reference to grayscale framebuffer
    ///
    /// Panics if the framebuffer is not grayscale. Use this only when
    /// you've verified the color depth.
    pub fn as_gray4_mut(&mut self) -> &mut VarFrameBuf<Gray4> {
        match self {
            FrameBuffer::Gray4(fb) => fb,
            _ => panic!("Framebuffer is not grayscale"),
        }
    }

    /// Get immutable reference to grayscale framebuffer
    pub fn as_gray4(&self) -> Option<&VarFrameBuf<Gray4>> {
        match self {
            FrameBuffer::Gray4(fb) => Some(fb),
            _ => None,
        }
    }

    /// Get raw buffer data as bytes
    ///
    /// For monochrome displays, this packs 8 pixels per byte.
    /// For grayscale displays, this packs 2 pixels per byte (4 bits each).
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            FrameBuffer::Mono(fb) => {
                // Pack BinaryColor pixels into bytes (8 pixels per byte)
                let pixels = fb.as_slice();
                let mut bytes = Vec::new();

                for chunk in pixels.chunks(8) {
                    let mut byte = 0u8;
                    for (i, pixel) in chunk.iter().enumerate() {
                        if *pixel == BinaryColor::On {
                            byte |= 1 << i;
                        }
                    }
                    bytes.push(byte);
                }
                bytes
            }
            FrameBuffer::Gray4(fb) => {
                // Pack Gray4 pixels into bytes (2 pixels per byte)
                let pixels = fb.as_slice();
                let mut bytes = Vec::new();

                for chunk in pixels.chunks(2) {
                    let mut byte = 0u8;
                    if let Some(pixel) = chunk.get(0) {
                        byte |= (pixel.luma() & 0x0F) << 4;
                    }
                    if let Some(pixel) = chunk.get(1) {
                        byte |= pixel.luma() & 0x0F;
                    }
                    bytes.push(byte);
                }
                bytes
            }
        }
    }

    // Note: We can't provide a generic draw() method that takes a closure with DrawTarget
    // because DrawTarget is not dyn compatible (it has generic methods).
    // Instead, users should match on the FrameBuffer enum and call the appropriate method.
}
