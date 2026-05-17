/*
 *  clock_font_svg.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  SVG-based clock font loader.
 *
 *  Each font lives in `./data/{font}.zip` and contains 13 SVG files:
 *    {font}_0.svg … {font}_9.svg  — digits
 *    {font}_colon.svg             — colon separator
 *    {font}_space.svg             — blank (for blinking colon)
 *    {font}_minus.svg             — minus sign
 *
 *  All SVGs have a 25×44 viewBox.  Characters are rendered at load time
 *  to a 1bpp packed pixel mask at the display-appropriate size:
 *    display height ≤ 70px  →  25 × 44  (1:1)
 *    display height > 70px  →  60 × 105 (≈2.5×, for ST7789 320×170)
 *
 *  The resulting ClockFontData owns all pixel data (no lifetime param),
 *  and provides the same get_char_image_raw() interface as the binary
 *  implementation so that clock.rs render methods need no changes.
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
 */

#![allow(dead_code)]

use log::{info, warn};
use std::io::Read;

// Character slot indices (13 total)
const IDX_0: usize = 0;
const IDX_COLON: usize = 10;
const IDX_SPACE: usize = 11;
const IDX_MINUS: usize = 12;
const CHAR_COUNT: usize = 13;

// Source SVG dimensions (all fonts share this viewBox)
const SVG_WIDTH: u32  = 25;
const SVG_HEIGHT: u32 = 44;

// Rendered sizes by display capability
// will be modified based on display dimensions
const SIZE_NORMAL: (u32, u32) = (25, 44);   // height ≤ 70
const SIZE_LARGE:  (u32, u32) = (60, 105);  // height > 70 (ST7789)

// Clock digit layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockDigitLayout {
    StandardTime,  // Clock HH:MM mode
    LongFormTime,  // Clock HH:MM.SS mode
    SquareTime,    // Clock HH over MM mode
}

/// Clock font with SVG-rendered RGBA character data.
///
/// Each character is stored as `digit_width * digit_height * 4` bytes,
/// in RGBA order (pre-multiplied by tiny-skia), row-major.  This preserves
/// the SVG's own colours and anti-aliasing.
///
/// Draw helpers interpret RGBA per colour depth:
///   BinaryColor → alpha >= 128 → On
///   Gray4       → BT.601 luma * alpha/255 → 0-15
///   Rgb565      → R>>3, G>>2, B>>3, premultiplied by alpha
///
/// Owns all pixel data — no lifetime parameter.
pub struct ClockFontData {
    pub digit_width:  u32,
    pub digit_height: u32,
    pub digit_layout: ClockDigitLayout,
    /// RGBA per pixel: `digit_width * digit_height * 4` bytes per character.
    /// [0..9] = digits, [10] = colon, [11] = space, [12] = minus.
    chars: [Vec<u8>; CHAR_COUNT],
}

impl ClockFontData {
    /// Return the RGBA data for `c` as a flat slice (`w * h * 4` bytes).
    pub fn get_char_rgba(&self, c: char) -> Option<&[u8]> {
        let data = match c {
            '0'..='9' => &self.chars[c as usize - '0' as usize],
            ':'       => &self.chars[IDX_COLON],
            ' '       => &self.chars[IDX_SPACE],
            '-'       => &self.chars[IDX_MINUS],
            _         => return None,
        };
        Some(data.as_slice())
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Render `svg_bytes` at `(width, height)` → RGBA, four bytes per pixel, row-major.
///
/// Preserves the SVG's own colours and anti-aliasing.
fn render_svg_to_rgba(svg_bytes: &[u8], width: u32, height: u32) -> Vec<u8> {
    let opt = usvg::Options::default();
    let tree = match usvg::Tree::from_data(svg_bytes, &opt) {
        Ok(t) => t,
        Err(e) => {
            warn!("SVG parse error: {}", e);
            return vec![0u8; (width * height * 4) as usize];
        }
    };

    let mut pixmap = match tiny_skia::Pixmap::new(width, height) {
        Some(p) => p,
        None => {
            warn!("Failed to create pixmap {}x{}", width, height);
            return vec![0u8; (width * height * 4) as usize];
        }
    };

    let sx = width  as f32 / tree.size().width();
    let sy = height as f32 / tree.size().height();
    resvg::render(&tree, tiny_skia::Transform::from_scale(sx, sy), &mut pixmap.as_mut());

    // Return full RGBA data (4 bytes per pixel)
    pixmap.data().to_vec()
}

/// Name mapping: character → filename stem suffix used inside the zip.
fn char_name(idx: usize) -> &'static str {
    match idx {
        0..=9 => ["0","1","2","3","4","5","6","7","8","9"][idx],
        10    => "colon",
        11    => "space",
        12    => "minus",
        13    => "period",
        _     => "space",
    }
}

/// Load and render all 13 characters from `./data/{font_name}.zip`.
///
/// Returns `None` if the zip cannot be opened or is missing entries.
fn load_from_zip(font_name: &str, width: u32, height: u32) -> Option<[Vec<u8>; CHAR_COUNT]> {
    let path = format!("./data/{}.zip", font_name);
    let file = std::fs::File::open(&path).map_err(|e| {
        warn!("Cannot open font zip {}: {}", path, e);
    }).ok()?;

    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        warn!("Cannot read zip {}: {}", path, e);
    }).ok()?;

    // Build the array; using a loop + collect into array
    let mut chars: [Vec<u8>; CHAR_COUNT] = Default::default();
    for idx in 0..CHAR_COUNT {
        let entry_name = format!("{}_{}.svg", font_name, char_name(idx));
        match archive.by_name(&entry_name) {
            Ok(mut entry) => {
                let mut svg_bytes = Vec::new();
                if entry.read_to_end(&mut svg_bytes).is_ok() {
                    chars[idx] = render_svg_to_rgba(&svg_bytes, width, height);
                } else {
                    warn!("Failed to read {} from zip", entry_name);
                    chars[idx] = vec![0u8; (width * height * 4) as usize];
                }
            }
            Err(_) => {
                warn!("Entry {} not found in {}", entry_name, path);
                chars[idx] = vec![0u8; (width * height) as usize];
            }
        }
    }

    Some(chars)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Load a clock font by name for the given display height.
///
/// Supported fonts (zip files in `./data/`):
///   7seg, dejavu, dotty, gawker, grandes, ledreal,
///   marvel, moomy, noto, poppins, roboto
///
/// Falls back to `7seg` if the requested font cannot be loaded.
///
/// Added logic for additional screen sizes (ST7789)
/// 
pub fn set_clock_font(font_name: &str, display_width: u32, display_height: u32) -> ClockFontData {
    let mut layout = ClockDigitLayout::StandardTime;
    let (mut width, mut height) = if display_height > 70 { SIZE_LARGE } else { SIZE_NORMAL };

    if 5*width > display_width {
        warn!("Display width {} too small for font width {}, scaling down", display_width, width);
        let scale = display_width as f32 / (6.0 * width as f32); // 6 as we need buffer at start end
        width  = (width  as f32 * scale) as u32;
        height = (height as f32 * scale) as u32;
    }

    if display_width==display_height {
        layout = ClockDigitLayout::SquareTime;
        width  = (display_width  as f32 / 3.0) as u32;
        height = (display_height as f32 / 3.0) as u32;
    }
    info!("Load SVG clock font: {} @ {}×{}", font_name, width, height);

    // Try requested font, then fall back to 7seg
    let chars = load_from_zip(font_name, width, height)
        .or_else(|| {
            if font_name != "7seg" {
                warn!("Font '{}' not found, falling back to 7seg", font_name);
                load_from_zip("7seg", width, height)
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            warn!("7seg zip also missing — using blank glyphs");
            Default::default()
        });

    ClockFontData { digit_width: width, digit_height: height, digit_layout: layout, chars }
}
