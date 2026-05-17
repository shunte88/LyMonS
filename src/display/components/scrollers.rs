/*
 *  display/components/scrollers.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Scrolling text component for displaying track information
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

#![allow(dead_code)] // scroller component helpers; some methods reserved

use std::sync::Arc;
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::BinaryColor;
use crate::display::layout::LayoutConfig;
use crate::display::field::Field;
use crate::display::ttf_font::{BlendCoverage, TtfFont};
use crate::textable::{TextScroller, ScrollMode};

/// Simple scroll state for one line of text
struct ScrollState {
    text: String,
    char_width: usize,
    scroll_width: u32,
    offset: i32,
    direction: i32, // -1 for left, 1 for right
    pause_counter: u32,
    log_counter: u32, // For debug logging
}

impl ScrollState {
    fn new() -> Self {
        Self {
            text: String::new(),
            offset: 0,
            char_width: 6,
            scroll_width: 0,
            direction: -1,
            pause_counter: 0,
            log_counter: 0,
        }
    }

    fn set_text(&mut self, text: String) {
        // Only reset scroll state if text actually changed
        if self.text != text {
            self.text = text;
            self.offset = 0;
            self.pause_counter = 30; // Pause for 30 frames before scrolling
            self.log_counter = 0; // Reset log counter for new text
        }
    }

    fn update(&mut self, scroll_mode: ScrollMode, ttf: Option<&TtfFont>) {

        if self.text.is_empty() {
            return;
        }

        let text_width = match ttf {
            Some(f) => f.measure_text(&self.text),
            None    => (self.text.len() * self.char_width) as i32,
        };

        // If text fits on screen, no scrolling needed
        if text_width <= self.scroll_width as i32{
            let cx: i32 = (self.scroll_width as i32 - text_width) / 2;
            self.offset = cx;
            return;
        }

        // Handle pause at start/end
        if self.pause_counter > 0 {
            self.pause_counter -= 1;
            return;
        }

        match scroll_mode {
            ScrollMode::ScrollLeft => {
                // Continuous left scroll with loop
                self.offset -= 1;
                if self.offset < -(text_width + 12) {
                    self.offset = 0;
                    self.pause_counter = 30;
                }
            }
            ScrollMode::ScrollCylon => {
                // Bounce back and forth
                self.offset += self.direction;
                if self.offset <= -(text_width - self.scroll_width as i32) {
                    self.direction = 1;
                    self.pause_counter = 30;
                } else if self.offset >= 0 {
                    self.direction = -1;
                    self.pause_counter = 30;
                }
            }
            ScrollMode::Static => {
                // center the text
                let cx: i32 = if text_width < self.scroll_width as i32 {(self.scroll_width as i32 - text_width) / 2} else { 0 };
                self.offset = cx;
            }
        }
    }

    fn get_offset(&self) -> i32 {
        self.offset
    }
}

/// Scrolling text component for artist and title
pub struct ScrollingText {
    album_artist_scroller: Option<TextScroller>,
    album_scroller: Option<TextScroller>,
    title_scroller: Option<TextScroller>,
    artist_scroller: Option<TextScroller>,
    combination_scroller: Option<TextScroller>,
    year_scroller: Option<TextScroller>,
    // Simple synchronous scroll states
    album_artist_scroll: ScrollState,
    album_scroll: ScrollState,
    title_scroll: ScrollState,
    artist_scroll: ScrollState,
    combination_scroll: ScrollState,
    year_scroll: ScrollState,
    // attribute drivers
    scroll_mode: ScrollMode,
    layout: LayoutConfig,
    display_width: u32,
    /// Optional TTF renderer; when set, replaces MonoFont bitmap rendering.
    ttf_font: Option<Arc<TtfFont>>,
}

impl ScrollingText {
    /// Create a new scrolling text component
    pub fn new(layout: LayoutConfig, scroll_mode: ScrollMode) -> Self {
        let display_width = layout.width;
        Self {
            album_artist_scroller: None,
            album_scroller: None,
            title_scroller: None,
            artist_scroller: None,
            combination_scroller: None,
            year_scroller: None,
            album_artist_scroll: ScrollState::new(),
            album_scroll: ScrollState::new(),
            title_scroll: ScrollState::new(),
            artist_scroll: ScrollState::new(),
            combination_scroll: ScrollState::new(),
            year_scroll: ScrollState::new(),
            scroll_mode,
            layout,
            display_width,
            ttf_font: None,
        }
    }

    /// Attach a TTF font.  Replaces MonoFont rendering in `render_field` and
    /// enables accurate pixel-width measurement for CJK and variable-width text.
    pub fn set_ttf_font(&mut self, font: Arc<TtfFont>) {
        self.ttf_font = Some(font);
    }

    /// Update album artist text
    pub fn set_album_artist(&mut self, album_artist: String) {
        self.album_artist_scroll.set_text(album_artist);
    }

    /// Update album text
    pub fn set_album(&mut self, album: String) {
        self.album_scroll.set_text(album);
    }

    /// Update title text
    pub fn set_title(&mut self, title: String) {
        self.title_scroll.set_text(title);
    }
 
    /// Update artist text
    pub fn set_artist(&mut self, artist: String) {
        self.artist_scroll.set_text(artist);
    }

    /// Update combination text
    pub fn set_combination(&mut self, combination: String) {
        self.combination_scroll.set_text(combination);
    }

    /// Update year text
    pub fn set_year(&mut self, year: String) {
        self.year_scroll.set_text(year);
    }

    /// Update both artist and title
    pub fn set_track_info(&mut self, artist: String, title: String) 
    {
        self.set_artist(artist);
        self.set_title(title);
    }

    /// Update artist, title, and album
    pub fn set_full_track_info(
        &mut self, 
        album_artist: String,
        album: String, 
        title: String, 
        artist: String, 
        year: String, 
    ) 
    {
        self.set_album_artist(album_artist.clone());
        self.set_album(album.clone());
        self.set_title(title.clone());
        self.set_artist(artist.clone());
        self.set_year(year.clone());

        // Set combination text (artist - title or partial)
        let scroll_text = match (artist.is_empty(), album.is_empty(), title.is_empty()) {
            (false, false, false) => format!("{} - {} - {}", artist.clone(), album.clone(), title.clone()),
            (false, false, true)  => format!("{} - {}", artist.clone(), album.clone()),
            (false, true, true)  => artist.clone(),
            (true, false, false) => format!("{} - {}", album.clone(), title.clone()),
            (true, true, false) => title.clone(),
            _ => String::new(),
        };
        self.set_combination(scroll_text);

    }

    /// Update scroll position (called on each frame)
    pub fn update(&mut self) {
        let ttf = self.ttf_font.as_deref();
        self.album_artist_scroll.update(self.scroll_mode, ttf);
        self.album_scroll.update(self.scroll_mode, ttf);
        self.title_scroll.update(self.scroll_mode, ttf);
        self.artist_scroll.update(self.scroll_mode, ttf);
        self.combination_scroll.update(self.scroll_mode, ttf);
        self.year_scroll.update(self.scroll_mode, ttf);
    }

    /// Update combination scroll position (called on each frame)
    pub fn update_combination(&mut self) {
        let ttf = self.ttf_font.as_deref();
        self.combination_scroll.update(self.scroll_mode, ttf);
    }

    /// Update combination scroll position using actual field width
    pub fn update_combination_with_field(&mut self, field: &Field) {
        let ttf = self.ttf_font.as_deref();
        self.combination_scroll.scroll_width = field.width();
        if let Some(f) = field.font {
            self.combination_scroll.char_width =
                f.character_size.width as usize + f.character_spacing as usize;
        }
        self.combination_scroll.update(self.scroll_mode, ttf);
    }

    /// Update scroll position using field widths
    pub fn update_with_fields(
        &mut self,
        album_artist_field: &Field,
        album_field:        &Field,
        title_field:        &Field,
        artist_field:       &Field,
        year_field:         &Field,
    ) {
        let ttf = self.ttf_font.as_deref();

        self.album_artist_scroll.scroll_width = album_artist_field.width();
        if let Some(f) = album_artist_field.font {
            self.album_artist_scroll.char_width =
                f.character_size.width as usize + f.character_spacing as usize;
        }
        self.album_artist_scroll.update(self.scroll_mode, ttf);

        self.album_scroll.scroll_width = album_field.width();
        if let Some(f) = album_field.font {
            self.album_scroll.char_width =
                f.character_size.width as usize + f.character_spacing as usize;
        }
        self.album_scroll.update(self.scroll_mode, ttf);

        self.title_scroll.scroll_width = title_field.width();
        if let Some(f) = title_field.font {
            self.title_scroll.char_width =
                f.character_size.width as usize + f.character_spacing as usize;
        }
        self.title_scroll.update(self.scroll_mode, ttf);

        self.artist_scroll.scroll_width = artist_field.width();
        if let Some(f) = artist_field.font {
            self.artist_scroll.char_width =
                f.character_size.width as usize + f.character_spacing as usize;
        }

        self.year_scroll.update(self.scroll_mode, ttf);
        self.year_scroll.scroll_width = year_field.width();
        if let Some(f) = year_field.font {
            self.year_scroll.char_width =
                f.character_size.width as usize + f.character_spacing as usize;
        }
        self.year_scroll.update(self.scroll_mode, ttf);

    }

    /// Update scroll width + advance one tick for a single named field.
    ///
    /// Call this before `render_field()` for each scrollable egg overlay field so
    /// the scroller knows the field width (for centering / overflow detection).
    pub fn update_field_scroll(&mut self, field: &Field) {
        let ttf = self.ttf_font.as_deref();
        let state = match field.name.as_str() {
            "album_artist" => &mut self.album_artist_scroll,
            "artist"       => &mut self.artist_scroll,
            "album"        => &mut self.album_scroll,
            "title"        => &mut self.title_scroll,
            "combination"  => &mut self.combination_scroll,
            "year"         => &mut self.year_scroll,
            _              => return,
        };
        state.scroll_width = field.width();
        if let Some(f) = field.font {
            state.char_width = f.character_size.width as usize + f.character_spacing as usize;
        }
        state.update(self.scroll_mode, ttf);
    }

    /// Render the scrolling text
    pub fn render<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        use embedded_graphics::mono_font::{iso_8859_13::FONT_5X8, MonoTextStyle};
        use embedded_graphics::text::Text;
        use embedded_graphics::geometry::Point;

        let font = &FONT_5X8;
        let text_style = MonoTextStyle::new(font, BinaryColor::On);
        let char_width = font.character_size.width as usize + font.character_spacing as usize;
        let word_gap = 3 * char_width as i32;
        let mut text_y = 15;
        let text_height = font.character_size.height as i32;
        let last_spacing = font.character_spacing as i32;

        if !self.album_artist_scroll.text.is_empty() {
            let x = self.album_artist_scroll.get_offset();
            Text::new(&self.album_artist_scroll.text, Point::new(x, text_y), text_style).draw(target)?;

            // For continuous loop mode, draw the text again after a gap
            if self.scroll_mode == ScrollMode::ScrollLeft {
                let text_width = (self.album_artist_scroll.text.len() * char_width) as i32 - last_spacing;
                let loop_x = x + text_width + word_gap; // 3 char gap
                Text::new(&self.album_artist_scroll.text, Point::new(loop_x, text_y), text_style).draw(target)?;
            }
        }

        text_y  += text_height;
        if !self.album_scroll.text.is_empty() {
            let x = self.album_scroll.get_offset();
            Text::new(&self.album_scroll.text, Point::new(x, text_y), text_style).draw(target)?;

            if self.scroll_mode == ScrollMode::ScrollLeft {
                let text_width = (self.album_scroll.text.len() * char_width) as i32 - last_spacing;
                let loop_x = x + text_width + word_gap;
                Text::new(&self.album_scroll.text, Point::new(loop_x, text_y), text_style).draw(target)?;
            }
        }

        text_y += text_height;
        if !self.title_scroll.text.is_empty() {
            let x = self.title_scroll.get_offset();
            Text::new(&self.title_scroll.text, Point::new(x, text_y), text_style).draw(target)?;

            if self.scroll_mode == ScrollMode::ScrollLeft {
                let text_width = (self.title_scroll.text.len() * char_width) as i32 - last_spacing;
                let loop_x = x + text_width + word_gap;
                Text::new(&self.title_scroll.text, Point::new(loop_x, text_y), text_style).draw(target)?;
            }
        }

        text_y += text_height;
        if !self.artist_scroll.text.is_empty() {
            let x = self.artist_scroll.get_offset();
            Text::new(&self.artist_scroll.text, Point::new(x, text_y), text_style).draw(target)?;

            // For continuous loop mode, draw the text again after a gap
            if self.scroll_mode == ScrollMode::ScrollLeft {
                let text_width = (self.artist_scroll.text.len() * char_width) as i32 - last_spacing;
                let loop_x = x + text_width + word_gap; // 12px gap
                Text::new(&self.artist_scroll.text, Point::new(loop_x, text_y), text_style).draw(target)?;
            }
        }

        Ok(())
    }

    /// Render to a specific field by name.
    ///
    /// When a TTF font is attached (`set_ttf_font`), it is used for rendering
    /// and for measuring text width (loop gap, centering).  Otherwise the field's
    /// MonoFont is used, preserving the original behaviour exactly.
    pub fn render_field<D, C>(&self, field: &Field, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
        C: PixelColor + BlendCoverage,
        crate::display::color::Color: crate::display::color_proxy::ConvertColor<C>,
    {
        use crate::display::color_proxy::ConvertColor;

        let scroll_state = match field.name.as_str() {
            "album_artist" => &self.album_artist_scroll,
            "artist"       => &self.artist_scroll,
            "album"        => &self.album_scroll,
            "title"        => &self.title_scroll,
            "combination"  => &self.combination_scroll,
            "year"         => &self.year_scroll,
            _              => return Ok(()),
        };

        if scroll_state.text.is_empty() {
            return Ok(());
        }

        let field_pos  = field.position();
        let fg: C      = field.fg_color.to_color();
        let mut clipped = target.clipped(&field.bounds);

        if let Some(ttf) = &self.ttf_font {
            // TTF path — vertically centre the text within the field using real metrics.
            let ascent= ttf.ascent();
            let line_h= ttf.line_height();
            let field_h= field.height() as i32;
            let baseline_y= field_pos.y + (field_h - line_h) / 2 + ascent;
            let x= field_pos.x + scroll_state.get_offset();

            ttf.render_text(&scroll_state.text, x, baseline_y, fg, &mut clipped)?;

            if field.scrollable && self.scroll_mode == ScrollMode::ScrollLeft {
                let text_px = ttf.measure_text(&scroll_state.text);
                let gap     = (ttf.pixel_size() * 2.0).round() as i32;
                ttf.render_loop_copy(
                    &scroll_state.text, x, baseline_y, fg, text_px, gap, &mut clipped,
                )?;
            }
        } else {
            // MonoFont path — identical to the previous implementation.
            use embedded_graphics::mono_font::MonoTextStyle;
            use embedded_graphics::text::Text;
            use embedded_graphics::geometry::Point;

            let font = field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_5X8);
            let text_style = MonoTextStyle::new(font, fg);
            let char_width = font.character_size.width as usize + font.character_spacing as usize;
            let word_gap= 3 * char_width as usize;
            let baseline_y= field_pos.y + font.baseline as i32;
            let x= field_pos.x + scroll_state.get_offset();
            let last_spacing = font.character_spacing as i32;


            Text::new(&scroll_state.text, Point::new(x, baseline_y), text_style)
                .draw(&mut clipped)?;

            if field.scrollable && self.scroll_mode == ScrollMode::ScrollLeft {
                let text_width = (scroll_state.text.len() * char_width) as i32 - last_spacing;
                let loop_x     = x + text_width + word_gap as i32;
                Text::new(&scroll_state.text, Point::new(loop_x, baseline_y), text_style)
                    .draw(&mut clipped)?;
            }
        }

        Ok(())
    }

    /// Stop scrolling
    pub fn stop(&mut self) {
        self.album_artist_scroller = None;
        self.title_scroller = None;
        self.artist_scroller = None;
        self.album_scroller = None;
        self.combination_scroller = None;
        self.year_scroller = None;
    }

    /// Get scroll mode
    pub fn scroll_mode(&self) -> ScrollMode {
        self.scroll_mode
    }

    /// Set scroll mode
    pub fn set_scroll_mode(&mut self, mode: ScrollMode) {
        self.scroll_mode = mode;
    }
    
}
