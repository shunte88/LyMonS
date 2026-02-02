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
use crate::textable::{TextScroller, ScrollMode};

/// Scrolling text component for artist and title
pub struct ScrollingText {
    artist_scroller: Option<TextScroller>,
    title_scroller: Option<TextScroller>,
    scroll_mode: ScrollMode,
    layout: LayoutConfig,
}

impl ScrollingText {
    /// Create a new scrolling text component
    pub fn new(layout: LayoutConfig, scroll_mode: ScrollMode) -> Self {
        Self {
            artist_scroller: None,
            title_scroller: None,
            scroll_mode,
            layout,
        }
    }

    /// Update artist text
    pub fn set_artist(&mut self, artist: String) {
        // TODO: Initialize TextScroller with artist text
        // For now, this is a placeholder
    }

    /// Update title text
    pub fn set_title(&mut self, title: String) {
        // TODO: Initialize TextScroller with title text
        // For now, this is a placeholder
    }

    /// Update both artist and title
    pub fn set_track_info(&mut self, artist: String, title: String) {
        self.set_artist(artist);
        self.set_title(title);
    }

    /// Update scroll position (called on each frame)
    pub fn update(&mut self) {
        // TODO: Update scroller positions
    }

    /// Render the scrolling text
    pub fn render<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        // TODO: Implement actual rendering of scrolling text
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
