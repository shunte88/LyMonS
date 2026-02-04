/*
 *  display/field.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Field-based layout system for declarative UI positioning
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
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::mono_font::MonoFont;
use super::color::Color;

/// Field alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

/// Field type determines rendering behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// Text field - renders string content
    Text,
    /// Glyph field - renders bitmap image
    Glyph,
    /// Custom field - component handles rendering
    Custom,
}

/// Field definition - declarative UI element positioning
///
/// A field defines a rectangular region on the display with
/// properties for rendering content within that region.
#[derive(Debug, Clone)]
pub struct Field {
    /// Field identifier (e.g., "status_bar", "artist", "weather_icon")
    pub name: String,

    /// Field type
    pub field_type: FieldType,

    /// Bounding rectangle (x, y, width, height)
    pub bounds: Rectangle,

    /// Border, >0 draw border of specified width
    pub border: u8,

    /// Whether content should scroll if it exceeds bounds
    pub scrollable: bool,

    /// Font for text rendering (None for glyph fields)
    pub font: Option<&'static MonoFont<'static>>,

    /// Foreground color (adapts to display color depth)
    pub fg_color: Color,

    /// Background color (None for transparent, adapts to display color depth)
    pub bg_color: Option<Color>,

    /// Text alignment/justification within field
    pub alignment: Alignment,
}

impl Field {
    /// Create a new text field
    pub fn new_text(
        name: impl Into<String>,
        bounds: Rectangle,
        font: &'static MonoFont<'static>,
    ) -> Self {
        Self {
            name: name.into(),
            field_type: FieldType::Text,
            bounds,
            border: 0,
            scrollable: false,
            font: Some(font),
            fg_color: Color::White,
            bg_color: None,
            alignment: Alignment::Left,
        }
    }

    /// Create a new glyph field
    pub fn new_glyph(
        name: impl Into<String>,
        bounds: Rectangle,
    ) -> Self {
        Self {
            name: name.into(),
            field_type: FieldType::Glyph,
            bounds,
            border: 0,
            scrollable: false,
            font: None,
            fg_color: Color::White,
            bg_color: None,
            alignment: Alignment::Left,
        }
    }

    /// Create a new custom field (component-rendered)
    pub fn new_custom(
        name: impl Into<String>,
        bounds: Rectangle,
    ) -> Self {
        Self {
            name: name.into(),
            field_type: FieldType::Custom,
            bounds,
            border: 0,
            scrollable: false,
            font: None,
            fg_color: Color::White,
            bg_color: None,
            alignment: Alignment::Left,
        }
    }

    /// Builder: set scrollable
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    /// Builder: set border
    pub fn border(mut self, border: u8) -> Self {
        self.border = border;
        self
    }

    /// Builder: set alignment
    pub fn align(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Builder: set colors (will adapt to display color depth)
    pub fn colors(mut self, fg: Color, bg: Option<Color>) -> Self {
        self.fg_color = fg;
        self.bg_color = bg;
        self
    }

    /// Get foreground color as BinaryColor for monochrome displays
    pub fn fg_binary(&self) -> BinaryColor {
        self.fg_color.to_binary()
    }

    /// Get background color as BinaryColor for monochrome displays
    pub fn bg_binary(&self) -> Option<BinaryColor> {
        self.bg_color.map(|c| c.to_binary())
    }

    /// Get field width
    pub fn width(&self) -> u32 {
        self.bounds.size.width
    }

    /// Get field height
    pub fn height(&self) -> u32 {
        self.bounds.size.height
    }

    /// Get top-left position
    pub fn position(&self) -> Point {
        self.bounds.top_left
    }
}
