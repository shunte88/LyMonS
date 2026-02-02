/*
 *  vframebuf.rs
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

use core::convert::Infallible;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::pixelcolor::PixelColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;

/// A runtime-sized framebuffer for embedded-graphics.
#[derive(Debug, Clone)]
pub struct VarFrameBuf<C: PixelColor> {
    buf: Vec<C>,
    w: usize,
    h: usize,
}

impl<C: PixelColor + Clone> VarFrameBuf<C> {
    pub fn new(width: u32, height: u32, fill: C) -> Self {
        let (w, h) = (width as usize, height as usize);
        Self { buf: vec![fill; w * h], w, h }
    }

    pub fn width(&self) -> usize { self.w }
    pub fn height(&self) -> usize { self.h }

    /// Mutable raw access (useful for pushing regions to the panel)
    pub fn as_mut_slice(&mut self) -> &mut [C] { &mut self.buf }

    /// Immutable raw access
    pub fn as_slice(&self) -> &[C] { &self.buf }

    /// Clear to a color
    pub fn clear_color(&mut self, color: C) {
        self.buf.fill(color);
    }

    /// Map (x,y) to linear index; returns None if out of bounds
    #[inline]
    fn idx(&self, p: Point) -> Option<usize> {
        if p.x >= 0 && p.y >= 0 {
            let (x, y) = (p.x as usize, p.y as usize);
            if x < self.w && y < self.h {
                return Some(y * self.w + x);
            }
        }
        None
    }
}

impl<C: PixelColor> OriginDimensions for VarFrameBuf<C> {
    fn size(&self) -> Size {
        Size::new(self.w as u32, self.h as u32)
    }
}

impl<C: PixelColor + Clone> DrawTarget for VarFrameBuf<C> {
    type Color = C;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            if let Some(i) = self.idx(p) {
                self.buf[i] = c;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear_color(color);
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        // fast path for rectangular fills the primitives use
        let Size { width, height } = area.size;
        if width == 0 || height == 0 { return Ok(()); }
        let (x0, y0) = (area.top_left.x.max(0) as usize, area.top_left.y.max(0) as usize);
        let w = width as usize;
        let h = height as usize;

        let mut it = colors.into_iter();
        for row in 0..h {
            let base = (y0 + row) * self.w + x0;
            for col in 0..w {
                if let Some(c) = it.next() {
                    let i = base + col;
                    if i < self.buf.len() { self.buf[i] = c; }
                } else {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
    
}