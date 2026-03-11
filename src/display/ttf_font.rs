/*
 *  display/ttf_font.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  TTF/OTF font renderer using ab_glyph, with per-character glyph cache.
 *
 *  Fonts are loaded from a zip archive (same packaging as clock SVG fonts).
 *  The first `.ttf` or `.otf` entry found in the archive is used.
 *
 *  Anti-aliasing is handled via the `BlendCoverage` trait, which maps
 *  0.0–1.0 coverage to a pixel color appropriate for the target depth:
 *    BinaryColor  — threshold at 0.5 (crisp, no intermediate values)
 *    Gray4        — full 16-level grayscale (smooth sub-pixel rendering)
 *    Rgb565       — per-channel scaling (blends against black background)
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 */

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::io::Read;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use embedded_graphics::pixelcolor::{BinaryColor, Gray4, Rgb565};
use embedded_graphics::pixelcolor::RgbColor;
use embedded_graphics::prelude::*;
use log::{info, warn};

/// Pre-rasterized coverage bitmap for one character at the font's pixel size.
struct CachedGlyph {
    /// Row-major coverage values, 0.0 (transparent) to 1.0 (fully opaque).
    bitmap:   Vec<f32>,
    width:    u32,
    height:   u32,
    /// X offset from the text cursor to the left edge of the bitmap.
    offset_x: i32,
    /// Y offset from the text baseline to the top edge of the bitmap.
    /// Typically negative for ascenders (glyph pixels appear above the baseline).
    offset_y: i32,
    /// Horizontal advance (cursor increment after this character), in pixels.
    advance:  i32,
}

/// Blend a 0.0–1.0 coverage value into a typed pixel color.
///
/// Each color depth has its own AA strategy:
/// - `BinaryColor` — 1-bit threshold, no blending
/// - `Gray4`       — full 4-bit grayscale, maximally smooth on gray OLEDs
/// - `Rgb565`      — per-channel scale (assumes black background)
pub trait BlendCoverage: PixelColor + Copy {
    fn blend(color: Self, coverage: f32) -> Self;
}

impl BlendCoverage for BinaryColor {
    #[inline]
    fn blend(_color: Self, coverage: f32) -> Self {
        if coverage >= 0.5 { BinaryColor::On } else { BinaryColor::Off }
    }
}

impl BlendCoverage for Gray4 {
    #[inline]
    fn blend(_color: Self, coverage: f32) -> Self {
        Gray4::new((coverage * 15.0).round().min(15.0) as u8)
    }
}

impl BlendCoverage for Rgb565 {
    #[inline]
    fn blend(color: Self, coverage: f32) -> Self {
        Rgb565::new(
            ((color.r() as f32) * coverage).round() as u8,
            ((color.g() as f32) * coverage).round() as u8,
            ((color.b() as f32) * coverage).round() as u8,
        )
    }
}

/// A TTF/OTF font loaded at a fixed pixel size, with a lazy per-character
/// raster cache.  May be shared across components via `Arc<TtfFont>`.
pub struct TtfFont {
    font:       FontVec,
    pixel_size: f32,
    cache:      Mutex<HashMap<char, Option<CachedGlyph>>>,
}

impl TtfFont {
    /// Load the first `.ttf`/`.otf` entry from a zip archive.
    ///
    /// Returns `None` if the file cannot be opened, the zip is unreadable,
    /// or no font entry is found inside.  Logs warnings in all error cases.
    pub fn load_from_zip(zip_path: &str, pixel_size: f32) -> Option<Arc<Self>> {
        let file = match std::fs::File::open(zip_path) {
            Ok(f) => f,
            Err(e) => {
                warn!("ttf_font: cannot open {}: {}", zip_path, e);
                return None;
            }
        };

        let mut archive = match zip::ZipArchive::new(file) {
            Ok(a) => a,
            Err(e) => {
                warn!("ttf_font: cannot read zip {}: {}", zip_path, e);
                return None;
            }
        };

        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let is_font = {
                let n = entry.name().to_lowercase();
                n.ends_with(".ttf") || n.ends_with(".otf")
            };

            if is_font {
                let entry_name = entry.name().to_owned();
                let mut bytes = Vec::new();
                if entry.read_to_end(&mut bytes).is_err() {
                    warn!("ttf_font: failed to read {} from {}", entry_name, zip_path);
                    continue;
                }

                match FontVec::try_from_vec(bytes) {
                    Ok(font) => {
                        info!("ttf_font: loaded {} @ {}px from {}", entry_name, pixel_size, zip_path);
                        return Some(Arc::new(Self {
                            font,
                            pixel_size,
                            cache: Mutex::new(HashMap::new()),
                        }));
                    }
                    Err(e) => {
                        warn!("ttf_font: invalid font data in {}: {}", entry_name, e);
                        continue;
                    }
                }
            }
        }

        warn!("ttf_font: no TTF/OTF entry found in {}", zip_path);
        None
    }

    /// Pixel size this font was loaded at.
    pub fn pixel_size(&self) -> f32 { self.pixel_size }

    /// Ascent in pixels: distance from baseline to the top of the tallest glyph.
    /// Always positive.
    pub fn ascent(&self) -> i32 {
        let scaled = self.font.as_scaled(PxScale::from(self.pixel_size));
        scaled.ascent().round() as i32
    }

    /// Descent in pixels: distance from baseline to the bottom of descenders.
    /// Always negative (descenders go below the baseline).
    pub fn descent(&self) -> i32 {
        let scaled = self.font.as_scaled(PxScale::from(self.pixel_size));
        scaled.descent().round() as i32
    }

    /// Total line height (ascent − descent), in pixels.
    pub fn line_height(&self) -> i32 {
        self.ascent() - self.descent()
    }

    /// Measure the rendered pixel width of a string at this font's size.
    ///
    /// Uses horizontal advance values (ignores kerning for simplicity).
    pub fn measure_text(&self, text: &str) -> i32 {
        let scale  = PxScale::from(self.pixel_size);
        let scaled = self.font.as_scaled(scale);
        text.chars().map(|c| {
            let id = scaled.glyph_id(c);
            scaled.h_advance(id).round() as i32
        }).sum()
    }

    /// Rasterize a single character and store it in the cache.
    fn rasterize(&self, ch: char) -> Option<CachedGlyph> {
        let scale  = PxScale::from(self.pixel_size);
        let scaled = self.font.as_scaled(scale);

        let glyph_id = scaled.glyph_id(ch);

        let glyph = ab_glyph::Glyph {
            id: glyph_id,
            scale,
            position: ab_glyph::point(0.0, 0.0),
        };

        let advance = scaled.h_advance(glyph_id).round() as i32;

        match scaled.outline_glyph(glyph) {
            Some(outlined) => {
                let bounds = outlined.px_bounds();
                let width  = (bounds.max.x - bounds.min.x).ceil() as u32;
                let height = (bounds.max.y - bounds.min.y).ceil() as u32;

                let mut bitmap = vec![0.0f32; (width * height) as usize];
                outlined.draw(|x, y, v| {
                    if let Some(p) = bitmap.get_mut((y * width + x) as usize) {
                        *p = v;
                    }
                });

                Some(CachedGlyph {
                    bitmap,
                    width,
                    height,
                    offset_x: bounds.min.x.round() as i32,
                    offset_y: bounds.min.y.round() as i32,
                    advance,
                })
            }
            None => {
                // Whitespace or non-printable — no pixels, but advance is known.
                Some(CachedGlyph {
                    bitmap:   Vec::new(),
                    width:    0,
                    height:   0,
                    offset_x: 0,
                    offset_y: 0,
                    advance,
                })
            }
        }
    }

    /// Render `text` to `target` using anti-aliasing appropriate for `D::Color`.
    ///
    /// - `x`          — left edge of the text run (cursor start position)
    /// - `baseline_y` — vertical baseline position (same convention as `MonoTextStyle`)
    /// - `color`      — foreground color; coverage-blended per pixel
    ///
    /// The caller is responsible for clipping `target` to the desired bounds before
    /// calling this function (e.g. `target.clipped(&field.bounds)`).
    pub fn render_text<D>(
        &self,
        text:       &str,
        x:          i32,
        baseline_y: i32,
        color:      D::Color,
        target:     &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget,
        D::Color: BlendCoverage,
    {
        let mut cursor_x = x;
        let mut cache = self.cache.lock().unwrap();

        for ch in text.chars() {
            let glyph = cache.entry(ch).or_insert_with(|| self.rasterize(ch));

            if let Some(g) = glyph {
                if !g.bitmap.is_empty() {
                    let glyph_x = cursor_x + g.offset_x;
                    let glyph_y = baseline_y + g.offset_y;

                    for py in 0..g.height as i32 {
                        for px in 0..g.width as i32 {
                            let coverage = g.bitmap[(py * g.width as i32 + px) as usize];
                            if coverage > 0.0 {
                                let pixel_color = D::Color::blend(color, coverage);
                                Pixel(Point::new(glyph_x + px, glyph_y + py), pixel_color)
                                    .draw(target)?;
                            }
                        }
                    }
                }
                cursor_x += g.advance;
            }
        }

        Ok(())
    }

    /// Render a second copy of `text` at `x + text_pixel_width + gap`, for
    /// seamless loop scrolling (the trailing copy enters from the right).
    pub fn render_loop_copy<D>(
        &self,
        text:            &str,
        x:               i32,
        baseline_y:      i32,
        color:           D::Color,
        text_pixel_width: i32,
        gap:             i32,
        target:          &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget,
        D::Color: BlendCoverage,
    {
        self.render_text(text, x + text_pixel_width + gap, baseline_y, color, target)
    }
}
