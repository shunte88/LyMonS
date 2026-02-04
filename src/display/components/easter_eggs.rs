/*
 *  display/components/easter_eggs.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Easter eggs component for whimsical audio-related animations
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

use embedded_graphics::{
    image::Image,
    mono_font::{ascii::FONT_4X6, ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
    text::{Text, Alignment as TextAlignment},
    geometry::Point,
};

use crate::display::field::Field;
use crate::display::layout::LayoutConfig;
use crate::eggs::Eggs;
use crate::deutils::seconds_to_hms;

/// Easter eggs component - whimsical audio-related animations
pub struct EasterEggsComponent {
    #[allow(dead_code)]
    layout: LayoutConfig,
}

impl EasterEggsComponent {
    pub fn new(layout: LayoutConfig) -> Self {
        Self { layout }
    }

    /// Render the main egg SVG image
    pub async fn render_egg_image<D>(
        &self,
        field: &Field,
        target: &mut D,
        egg: &mut Eggs,
        artist: &str,
        title: &str,
        level: u8,
        track_percent: f64,
        track_time: f32,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        // Render the SVG with animations
        let raw_image = egg
            .update_and_render(artist, title, level, track_percent, track_time)
            .await
            .map_err(|_| ())
            .unwrap(); // TODO: Better error handling

        // Draw the rendered SVG image
        Image::new(&raw_image, field.position()).draw(target)?;

        Ok(())
    }

    /// Render artist text field
    pub fn render_artist_text<D>(
        &self,
        field: &Field,
        target: &mut D,
        egg: &Eggs,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let artist_text = egg.get_artist();
        if artist_text.is_empty() {
            return Ok(());
        }

        let rect = egg.get_artist_rect();
        if rect.is_zero_sized() {
            return Ok(());
        }

        let character_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);

        // For combined mode (artist contains both artist and title with newline)
        if egg.is_combined() {
            // Draw multiline text - left-aligned, top
            let lines: Vec<&str> = artist_text.split('\n').collect();
            let line_height = 7; // FONT_4X6 height
            let mut y = rect.top_left.y;

            for line in lines {
                if y + line_height <= rect.bottom_right().unwrap().y {
                    Text::with_alignment(
                        line,
                        Point::new(rect.top_left.x, y),
                        character_style,
                        TextAlignment::Left,
                    )
                    .draw(target)?;
                    y += line_height;
                }
            }
        } else {
            // Draw single line - centered
            let center_x = rect.top_left.x + (rect.size.width / 2) as i32;
            let center_y = rect.top_left.y + (rect.size.height / 2) as i32;

            Text::with_alignment(
                artist_text,
                Point::new(center_x, center_y),
                character_style,
                TextAlignment::Center,
            )
            .draw(target)?;
        }

        Ok(())
    }

    /// Render title text field (only used when not combined)
    pub fn render_title_text<D>(
        &self,
        field: &Field,
        target: &mut D,
        egg: &Eggs,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        if egg.is_combined() {
            return Ok(());
        }

        let title_text = egg.get_title();
        if title_text.is_empty() {
            return Ok(());
        }

        let rect = egg.get_title_rect();
        if rect.is_zero_sized() {
            return Ok(());
        }

        let character_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
        let center_x = rect.top_left.x + (rect.size.width / 2) as i32;
        let center_y = rect.top_left.y + (rect.size.height / 2) as i32;

        Text::with_alignment(
            title_text,
            Point::new(center_x, center_y),
            character_style,
            TextAlignment::Center,
        )
        .draw(target)?;

        Ok(())
    }

    /// Render track time field
    pub fn render_time_text<D>(
        &self,
        field: &Field,
        target: &mut D,
        egg: &Eggs,
        show_remaining: bool,
        remaining_time: f32,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let rect = egg.get_time_rect();
        if rect.is_zero_sized() {
            return Ok(());
        }

        let track_time = egg.get_track_time();
        let time_str = if show_remaining {
            format!("-{}", seconds_to_hms(remaining_time))
        } else {
            seconds_to_hms(track_time)
        };

        let character_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

        // Right-aligned, middle
        let right_x = rect.bottom_right().unwrap().x;
        let middle_y = rect.top_left.y + (rect.size.height / 2) as i32;

        Text::with_alignment(
            &time_str,
            Point::new(right_x, middle_y),
            character_style,
            TextAlignment::Right,
        )
        .draw(target)?;

        Ok(())
    }
}
