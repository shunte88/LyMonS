/*
 *  display/components/status_bar.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Status bar component - displays volume, playback mode, bitrate, etc.
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
use embedded_graphics::pixelcolor::BinaryColor;
use crate::display::layout::LayoutConfig;
use crate::display::field::Field;
use arrayvec::ArrayString;
use core::fmt::Write;
use crate::glyphs;

/// Repeat mode for playback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

/// Shuffle mode for playback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShuffleMode {
    Off,
    ByTracks,
    ByAlbums,
}

/// Audio bitrate information (stack-allocated for zero heap allocations)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioBitrate {
    Unknown,
    Bitrate(ArrayString<16>), // e.g., "24/192" - max 16 chars on stack
}

/// Status bar state
#[derive(Debug, Clone)]
pub struct StatusBarState {
    /// Volume level (0-100)
    pub volume_percent: u8,

    /// Whether audio is muted
    pub is_muted: bool,

    /// Repeat mode
    pub repeat_mode: RepeatMode,

    /// Shuffle mode
    pub shuffle_mode: ShuffleMode,

    /// Audio bitrate information
    pub audio_bitrate: AudioBitrate,

    /// Sample rate (for logic, not always displayed) - stack allocated
    pub samplerate: ArrayString<8>,

    /// Sample size (for logic, not always displayed) - stack allocated
    pub samplesize: ArrayString<8>,

    /// Formatted bitrate text for display - stack allocated
    pub bitrate_text: ArrayString<16>,
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self {
            volume_percent: 50,
            is_muted: false,
            repeat_mode: RepeatMode::Off,
            shuffle_mode: ShuffleMode::Off,
            audio_bitrate: AudioBitrate::Unknown,
            samplerate: ArrayString::new(),
            samplesize: ArrayString::new(),
            bitrate_text: ArrayString::new(),
        }
    }
}

/// Status bar component
pub struct StatusBar {
    state: StatusBarState,
    layout: LayoutConfig,
}

impl StatusBar {
    /// Create a new status bar component
    pub fn new(layout: LayoutConfig) -> Self {
        Self {
            state: StatusBarState::default(),
            layout,
        }
    }

    /// Helper to draw a monochrome glyph (8x8) on any color target
    /// Converts BinaryColor pixels to target color type
    fn draw_glyph<D, C>(&self, target: &mut D, glyph_data: &[u8; 8], x: i32, y: i32, color: C) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
        C: PixelColor,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::Pixel;

        // Iterate over 8x8 glyph bitmap
        for row in 0..8 {
            let byte = glyph_data[row];
            for col in 0..8 {
                // Check if bit is set (MSB first)
                if (byte & (1 << (7 - col))) != 0 {
                    let pixel = Pixel(Point::new(x + col, y + row as i32), color);
                    target.draw_iter(core::iter::once(pixel))?;
                }
            }
        }
        Ok(())
    }

    /// Update status bar state
    pub fn update(&mut self, state: StatusBarState) {
        self.state = state;
    }

    /// Get current state
    pub fn state(&self) -> &StatusBarState {
        &self.state
    }

    /// Render the status bar
    pub fn render<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        use embedded_graphics::mono_font::{iso_8859_13::FONT_6X10, MonoTextStyle};
        use embedded_graphics::text::Text;
        use embedded_graphics::geometry::Point;

        let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

        // Format status line: "V:43 24/48k R S"
        let status_text = {
            let mut s = String::new();

            // Volume (V for volume, M for muted)
            if self.state.is_muted {
                s.push_str("V:M ");
            } else {
                use std::fmt::Write;
                let _ = write!(&mut s, "V:{} ", self.state.volume_percent);
            }

            // Bitrate - handle DSD/DSF specially
            if !self.state.samplesize.is_empty() && !self.state.samplerate.is_empty() {
                use std::fmt::Write;

                // Check for DSD/DSF (1-bit formats)
                if self.state.samplesize.as_str() == "1" || self.state.samplesize.as_str().starts_with("DSD") {
                    // Parse DSD rate: 2822400 -> DSD64, 5644800 -> DSD128, etc.
                    if let Ok(rate) = self.state.samplerate.parse::<u32>() {
                        let dsd_multiple = rate / 44100;  // Base DSD rate is 64x CD (44.1kHz)
                        let _ = write!(&mut s, "DSD{} ", dsd_multiple);
                    } else {
                        let _ = write!(&mut s, "DSD ");
                    }
                } else {
                    // Regular PCM: convert sample rate to kHz (e.g., 48000 -> 48kHz)
                    let rate_str = if let Ok(rate) = self.state.samplerate.parse::<u32>() {
                        if rate >= 1000 {
                            format!("{}k", rate / 1000)
                        } else {
                            rate.to_string()
                        }
                    } else {
                        self.state.samplerate.to_string()
                    };
                    let _ = write!(&mut s, "{}/{} ", self.state.samplesize, rate_str);
                }
            }

            // Repeat mode (R for repeat)
            match self.state.repeat_mode {
                RepeatMode::Off => {},
                RepeatMode::All => { s.push_str("R "); },
                RepeatMode::One => { s.push_str("R1 "); },
            }

            // Shuffle mode (S for shuffle)
            match self.state.shuffle_mode {
                ShuffleMode::Off => {},
                ShuffleMode::ByTracks => { s.push_str("S"); },
                ShuffleMode::ByAlbums => { s.push_str("SA"); },
            }

            s
        };

        // Draw at top of screen (y=0)
        Text::new(&status_text, Point::new(0, 8), text_style).draw(target)?;

        Ok(())
    }

    /// Render to a specific field with glyphs
    pub fn render_field<D, C>(&self, field: &Field, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
        C: PixelColor,
        crate::display::color::Color: crate::display::color_proxy::ConvertColor<C>,
    {
        use embedded_graphics::mono_font::{iso_8859_13::FONT_5X8, MonoTextStyle};
        use embedded_graphics::text::Text;
        use embedded_graphics::geometry::Point;

        // Only render if this is a status_bar field
        if field.name != "status_bar" {
            return Ok(());
        }

        use crate::display::color_proxy::ConvertColor;
        let text_color = field.fg_color.to_color();
        let text_style = MonoTextStyle::new(&FONT_5X8, text_color);

        let field_pos = field.position();
        let field_width = field.width() as i32;
        let glyph_y = field_pos.y;  // Top of field for glyphs
        let text_y = glyph_y + 7;   // Baseline for text (7 = glyph height - 1px)

        // LEFT: Volume glyph + text
        let mut current_x = field_pos.x;

        // Draw volume glyph
        let vol_glyph = if self.state.is_muted || self.state.volume_percent == 0 {
            &glyphs::GLYPH_VOLUME_OFF
        } else {
            &glyphs::GLYPH_VOLUME_ON
        };
        self.draw_glyph(target, vol_glyph, current_x, glyph_y, text_color)?;
        current_x += 8; // Move past glyph

        // Draw volume text
        let vol_text = if self.state.is_muted || self.state.volume_percent == 0 {
            current_x += 3;
            "mute".to_string()
        } else {
            format!("{:>3}%", self.state.volume_percent)
        };
        Text::new(&vol_text, Point::new(current_x, text_y), text_style).draw(target)?;

        // CENTER: Bitrate text (centered)
        let bitrate_text = if !self.state.samplesize.is_empty() && !self.state.samplerate.is_empty() {
            // Check for DSD/DSF (1-bit formats)
            if self.state.samplesize.as_str() == "1" || self.state.samplesize.as_str().starts_with("DSD") {
                if let Ok(rate) = self.state.samplerate.parse::<u32>() {
                    let dsd_multiple = rate / 44100;
                    format!("DSD{}", dsd_multiple)
                } else {
                    "DSD".to_string()
                }
            } else {
                // Regular PCM: convert sample rate to kHz
                let rate_str = if let Ok(rate) = self.state.samplerate.parse::<u32>() {
                    if rate >= 1000 {
                        format!("{}k", rate / 1000)
                    } else {
                        rate.to_string()
                    }
                } else {
                    self.state.samplerate.to_string()
                };
                format!("{}/{}", self.state.samplesize, rate_str)
            }
        } else {
            String::new()
        };

        if !bitrate_text.is_empty() {
            let text_width = bitrate_text.len() * 5; // Approximate width for FONT_5X8
            let center_x = field_pos.x + (field_width - text_width as i32) / 2;
            Text::new(&bitrate_text, Point::new(center_x, text_y), text_style).draw(target)?;
        }

        // RIGHT: Repeat + Shuffle glyphs (right-justified)
        let glyph_width = 8;
        let glyph_gap = 2;

        // Shuffle glyph (rightmost)
        let shuffle_glyph = match self.state.shuffle_mode {
            ShuffleMode::ByTracks => &glyphs::GLYPH_SHUFFLE_TRACKS,
            ShuffleMode::ByAlbums => &glyphs::GLYPH_SHUFFLE_ALBUMS,
            ShuffleMode::Off => &glyphs::GLYPH_NONE,
        };
        let shuffle_x = field_pos.x + field_width - glyph_width;
        self.draw_glyph(target, shuffle_glyph, shuffle_x, glyph_y, text_color)?;

        // Repeat glyph (to the left of shuffle)
        let repeat_glyph = match self.state.repeat_mode {
            RepeatMode::One => &glyphs::GLYPH_REPEAT_ONE,
            RepeatMode::All => &glyphs::GLYPH_REPEAT_ALL,
            RepeatMode::Off => &glyphs::GLYPH_NONE,
        };
        let repeat_x = shuffle_x - glyph_width - glyph_gap;
        self.draw_glyph(target, repeat_glyph, repeat_x, glyph_y, text_color)?;

        Ok(())
    }

    /// Update volume
    pub fn set_volume(&mut self, volume: u8) {
        self.state.volume_percent = volume.min(100);
    }

    /// Update mute state
    pub fn set_muted(&mut self, muted: bool) {
        self.state.is_muted = muted;
    }

    /// Update repeat mode
    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.state.repeat_mode = mode;
    }

    /// Update shuffle mode
    pub fn set_shuffle_mode(&mut self, mode: ShuffleMode) {
        self.state.shuffle_mode = mode;
    }

    /// Update bitrate information (zero heap allocations!)
    pub fn set_bitrate(&mut self, samplerate: &str, samplesize: &str) {
        // Clear and populate stack-allocated strings
        self.state.samplerate.clear();
        self.state.samplesize.clear();
        self.state.bitrate_text.clear();

        // Truncate if needed (shouldn't happen with normal audio specs)
        let _ = self.state.samplerate.try_push_str(samplerate);
        let _ = self.state.samplesize.try_push_str(samplesize);

        // Format bitrate text (e.g., "24/192") - no heap allocation!
        let _ = write!(&mut self.state.bitrate_text, "{}/{}",
            self.state.samplesize, self.state.samplerate);

        self.state.audio_bitrate = AudioBitrate::Bitrate(self.state.bitrate_text);
    }

    /// Format volume text to stack-allocated string (zero allocations!)
    pub fn format_volume(&self) -> ArrayString<8> {
        let mut buf = ArrayString::new();
        if self.state.is_muted {
            let _ = write!(&mut buf, "MUTE");
        } else {
            let _ = write!(&mut buf, "{:>3}%", self.state.volume_percent);
        }
        buf
    }
}

/// Helper function to transpose repeat mode from integer
pub fn transpose_repeat_mode(mode: i32) -> RepeatMode {
    match mode {
        0 => RepeatMode::Off,
        1 => RepeatMode::All,
        2 => RepeatMode::One,
        _ => RepeatMode::Off,
    }
}

/// Helper function to transpose shuffle mode from integer
pub fn transpose_shuffle_mode(mode: i32) -> ShuffleMode {
    match mode {
        0 => ShuffleMode::Off,
        1 => ShuffleMode::ByTracks,
        2 => ShuffleMode::ByAlbums,
        _ => ShuffleMode::Off,
    }
}
