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

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::BinaryColor;
use crate::display::layout::LayoutConfig;
use crate::display::field::Field;
use crate::textable::{TextScroller, ScrollMode};

/// Simple scroll state for one line of text
struct ScrollState {
    text: String,
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

    fn update(&mut self, display_width: u32, scroll_mode: ScrollMode) {
        use embedded_graphics::mono_font::iso_8859_13::FONT_6X10;
        use embedded_graphics::mono_font::MonoTextStyle;
        use embedded_graphics::text::Text;
        use embedded_graphics::geometry::Point;

        if self.text.is_empty() {
            return;
        }

        // Calculate text width in pixels (approximate: 6 pixels per char)
        let text_width = (self.text.len() * 6) as i32;
        let display_width = display_width as i32;

        // If text fits on screen, no scrolling needed
        if text_width <= display_width {
            self.offset = 0;
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
                if self.offset <= -(text_width - display_width) {
                    self.direction = 1;
                    self.pause_counter = 30;
                } else if self.offset >= 0 {
                    self.direction = -1;
                    self.pause_counter = 30;
                }
            }
            ScrollMode::Static => {
                self.offset = 0;
            }
        }
    }

    fn get_offset(&self) -> i32 {
        self.offset
    }
}

/// Scrolling text component for artist and title
pub struct ScrollingText {
    artist_scroller: Option<TextScroller>,
    title_scroller: Option<TextScroller>,
    scroll_mode: ScrollMode,
    layout: LayoutConfig,
    // Simple synchronous scroll states
    artist_scroll: ScrollState,
    album_scroll: ScrollState,
    title_scroll: ScrollState,
    display_width: u32,
}

impl ScrollingText {
    /// Create a new scrolling text component
    pub fn new(layout: LayoutConfig, scroll_mode: ScrollMode) -> Self {
        let display_width = layout.width;
        Self {
            artist_scroller: None,
            title_scroller: None,
            scroll_mode,
            display_width,
            layout,
            artist_scroll: ScrollState::new(),
            album_scroll: ScrollState::new(),
            title_scroll: ScrollState::new(),
        }
    }

    /// Update artist text
    pub fn set_artist(&mut self, artist: String) {
        self.artist_scroll.set_text(artist);
    }

    /// Update title text
    pub fn set_title(&mut self, title: String) {
        self.title_scroll.set_text(title);
    }

    /// Update album text
    pub fn set_album(&mut self, album: String) {
        self.album_scroll.set_text(album);
    }

    /// Update both artist and title
    pub fn set_track_info(&mut self, artist: String, title: String) {
        self.set_artist(artist);
        self.set_title(title);
    }

    /// Update artist, title, and album
    pub fn set_full_track_info(&mut self, artist: String, title: String, album: String) {
        self.set_artist(artist);
        self.set_title(title);
        self.set_album(album);
    }

    /// Update scroll position (called on each frame)
    pub fn update(&mut self) {
        self.artist_scroll.update(self.display_width, self.scroll_mode);
        self.album_scroll.update(self.display_width, self.scroll_mode);
        self.title_scroll.update(self.display_width, self.scroll_mode);
    }

    /// Update scroll position using field widths
    pub fn update_with_fields(&mut self, artist_field: &Field, album_field: &Field, title_field: &Field) {
        self.artist_scroll.update(artist_field.width(), self.scroll_mode);
        self.album_scroll.update(album_field.width(), self.scroll_mode);
        self.title_scroll.update(title_field.width(), self.scroll_mode);
    }

    /// Render the scrolling text
    pub fn render<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        use embedded_graphics::mono_font::{iso_8859_13::FONT_6X10, MonoTextStyle};
        use embedded_graphics::text::Text;
        use embedded_graphics::geometry::Point;

        let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

        // Artist at y=18 (below status bar at y=8)
        if !self.artist_scroll.text.is_empty() {
            let x = self.artist_scroll.get_offset();
            Text::new(&self.artist_scroll.text, Point::new(x, 18), text_style).draw(target)?;

            // For continuous loop mode, draw the text again after a gap
            if self.scroll_mode == ScrollMode::ScrollLeft {
                let text_width = (self.artist_scroll.text.len() * 6) as i32;
                let loop_x = x + text_width + 12; // 12px gap
                Text::new(&self.artist_scroll.text, Point::new(loop_x, 18), text_style).draw(target)?;
            }
        }

        // Album at y=28 (below artist)
        if !self.album_scroll.text.is_empty() {
            let x = self.album_scroll.get_offset();
            Text::new(&self.album_scroll.text, Point::new(x, 28), text_style).draw(target)?;

            if self.scroll_mode == ScrollMode::ScrollLeft {
                let text_width = (self.album_scroll.text.len() * 6) as i32;
                let loop_x = x + text_width + 12;
                Text::new(&self.album_scroll.text, Point::new(loop_x, 28), text_style).draw(target)?;
            }
        }

        // Title at y=38 (below album)
        if !self.title_scroll.text.is_empty() {
            let x = self.title_scroll.get_offset();
            Text::new(&self.title_scroll.text, Point::new(x, 38), text_style).draw(target)?;

            if self.scroll_mode == ScrollMode::ScrollLeft {
                let text_width = (self.title_scroll.text.len() * 6) as i32;
                let loop_x = x + text_width + 12;
                Text::new(&self.title_scroll.text, Point::new(loop_x, 38), text_style).draw(target)?;
            }
        }

        Ok(())
    }

    /// Render to a specific field by name
    pub fn render_field<D, C>(&self, field: &Field, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
        C: PixelColor,
        crate::display::color::Color: crate::display::color_proxy::ConvertColor<C>,
    {
        use embedded_graphics::mono_font::MonoTextStyle;
        use embedded_graphics::text::Text;
        use embedded_graphics::geometry::Point;

        // Get the scroll state based on field name
        let scroll_state = match field.name.as_str() {
            "artist" => &self.artist_scroll,
            "album" => &self.album_scroll,
            "title" => &self.title_scroll,
            _ => return Ok(()), // Unknown field, skip
        };

        // Skip if no text
        if scroll_state.text.is_empty() {
            return Ok(());
        }

        // Use field's font and colors (convert to appropriate color depth)
        let font = field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_6X10);
        use crate::display::color_proxy::ConvertColor;
        let text_style = MonoTextStyle::new(font, field.fg_color.to_color());

        // Get field position (baseline is at bottom of field)
        let field_pos = field.position();
        let baseline_y = field_pos.y + field.height() as i32;

        // Calculate scroll offset
        let x = field_pos.x + scroll_state.get_offset();

        // Draw main text
        Text::new(&scroll_state.text, Point::new(x, baseline_y), text_style).draw(target)?;

        // For continuous loop mode, draw the text again after a gap
        if field.scrollable && self.scroll_mode == ScrollMode::ScrollLeft {
            let text_width = (scroll_state.text.len() * 6) as i32;
            let loop_x = x + text_width + 12; // 12px gap
            Text::new(&scroll_state.text, Point::new(loop_x, baseline_y), text_style).draw(target)?;
        }

        Ok(())
    }

    /// Stop scrolling
    pub fn stop(&mut self) {
        self.artist_scroller = None;
        self.title_scroller = None;
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
