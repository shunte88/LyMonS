/*
 *  display/layout_manager.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Layout manager - owns all page definitions for consistent display
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

use super::field::{Field, FieldType, Alignment};
use super::page::PageLayout;
use super::layout::LayoutConfig;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::ascii::{FONT_5X7, FONT_5X8, FONT_6X10};

/// Layout manager - creates and owns all page definitions
pub struct LayoutManager {
    layout_config: LayoutConfig,
}

impl LayoutManager {
    /// Create a new layout manager
    pub fn new(layout_config: LayoutConfig) -> Self {
        Self { layout_config }
    }

    /// Create the scrolling music page layout
    pub fn create_scrolling_page(&self) -> PageLayout {
        let width = self.layout_config.width;
        let height = self.layout_config.height;
        let width_adj = width-4;
        let border_adj = 2;

        PageLayout::new("scrolling")
            // Status bar at top (y=0)
            .add_field(
                Field::new_text(
                    "status_bar",
                    Rectangle::new(Point::new(border_adj, border_adj), Size::new(width_adj, 10)),
                    &FONT_6X10
                )
            )
            // didn't we have album_artist too ??? so for v/a we would have performer etc?
            // Artist (y=10)
            .add_field(
                Field::new_text(
                    "artist",
                    Rectangle::new(Point::new(border_adj, border_adj+10), Size::new(width_adj, 10)),
                    &FONT_6X10
                )
                .scrollable(true)
            )
            // Album (y=20)
            .add_field(
                Field::new_text(
                    "album",
                    Rectangle::new(Point::new(border_adj, border_adj+20), Size::new(width_adj, 10)),
                    &FONT_6X10
                )
                .scrollable(true)
            )
            // Title (y=30)
            .add_field(
                Field::new_text(
                    "title",
                    Rectangle::new(Point::new(border_adj, border_adj+30), Size::new(width_adj, 10)),
                    &FONT_6X10
                )
                .scrollable(true)
            )
            // Progress bar (y=40, height=4)
            .add_field(
                Field::new_custom(
                    "progress_bar",
                    Rectangle::new(Point::new(border_adj, height.saturating_sub(16) as i32), Size::new(width_adj, 4))
                )
            )
            // Info line (times) at bottom (y=62 for 128x64 displays)
            .add_field(
                Field::new_text(
                    "info_line",
                    Rectangle::new(Point::new(border_adj, height.saturating_sub(10) as i32), Size::new(width_adj, 10)),
                    &FONT_6X10
                )
            )
    }

    /// Create the clock page layout
    pub fn create_clock_page(&self) -> PageLayout {
        let width = self.layout_config.width;
        let width_adj = width-4;
        let border_adj = 2;

        // Constants from original code
        const CLOCK_DIGIT_HEIGHT: i32 = 44; // custom font height
        const CLOCK_DIGIT_WIDTH: i32 = 25; // custom font width
        const PROGRESS_BAR_HEIGHT: i32 = 6;
        const CLOCK_PROGRESS_BAR_GAP: i32 = 4;
        const PROGRESS_BAR_DATE_GAP: i32 = 2;
        const DATE_FONT_HEIGHT: i32 = 10;

        // Start clock at top of display (no offset)
        let clock_y_start = border_adj;
        let progress_bar_y = clock_y_start + CLOCK_DIGIT_HEIGHT + CLOCK_PROGRESS_BAR_GAP - 2;
        let date_y = progress_bar_y + PROGRESS_BAR_HEIGHT + PROGRESS_BAR_DATE_GAP;

        PageLayout::new("clock")
            // metrics if requested (centered)
            .add_field(
                Field::new_text(
                    "metrics",
                    Rectangle::new(Point::new(border_adj, border_adj), Size::new(width_adj, 7)),
                    &FONT_5X7
                )
                .align(Alignment::Center)
            )
            // Clock digits - custom font with 2px border
            .add_field(
                Field::new_custom(
                    "clock_digits",
                    Rectangle::new(Point::new(border_adj, clock_y_start), Size::new(width_adj, CLOCK_DIGIT_HEIGHT as u32))
                )
            )
            // Seconds progress bar (full width)
            .add_field(
                Field::new_custom(
                    "seconds_progress",
                    Rectangle::new(Point::new(border_adj, progress_bar_y), Size::new(width_adj, PROGRESS_BAR_HEIGHT as u32))
                )
            )
            // Date at bottom (centered)
            .add_field(
                Field::new_text(
                    "date",
                    Rectangle::new(Point::new(border_adj, date_y), Size::new(width_adj, DATE_FONT_HEIGHT as u32)),
                    &FONT_6X10
                )
                .align(Alignment::Center)
            )
    }

    /// Create the current weather page layout
    pub fn create_weather_current_page(&self) -> PageLayout {
        let width = self.layout_config.width;

        PageLayout::new("weather_current")
            // Status bar at top
            .add_field(
                Field::new_text(
                    "status_bar",
                    Rectangle::new(Point::new(0, 0), Size::new(width, 10)),
                    &FONT_6X10
                )
            )
            // Weather icon (glyph) - 32x32 centered left
            .add_field(
                Field::new_glyph(
                    "weather_icon",
                    Rectangle::new(Point::new(8, 16), Size::new(32, 32))
                )
            )
            // Temperature (large text, right of icon)
            .add_field(
                Field::new_text(
                    "temperature",
                    Rectangle::new(Point::new(48, 20), Size::new(width - 48, 15)),
                    &FONT_6X10
                )
            )
            // Conditions text (right of icon, below temp)
            .add_field(
                Field::new_text(
                    "conditions",
                    Rectangle::new(Point::new(48, 35), Size::new(width - 48, 10)),
                    &FONT_5X8
                )
            )
            // Location at bottom
            .add_field(
                Field::new_text(
                    "location",
                    Rectangle::new(Point::new(0, 52), Size::new(width, 10)),
                    &FONT_6X10
                )
                .align(Alignment::Center)
            )
    }

    /// Create the weather forecast page layout
    pub fn create_weather_forecast_page(&self) -> PageLayout {
        let width = self.layout_config.width;

        PageLayout::new("weather_forecast")
            // Status bar at top
            .add_field(
                Field::new_text(
                    "status_bar",
                    Rectangle::new(Point::new(0, 0), Size::new(width, 10)),
                    &FONT_6X10
                )
            )
            // Title "3 Day Forecast"
            .add_field(
                Field::new_text(
                    "forecast_title",
                    Rectangle::new(Point::new(0, 12), Size::new(width, 10)),
                    &FONT_6X10
                )
                .align(Alignment::Center)
            )
            // Day 1 (left third)
            .add_field(
                Field::new_glyph(
                    "day1_icon",
                    Rectangle::new(Point::new(4, 24), Size::new(16, 16))
                )
            )
            .add_field(
                Field::new_text(
                    "day1_temp",
                    Rectangle::new(Point::new(0, 42), Size::new(width / 3, 10)),
                    &FONT_5X8
                )
                .align(Alignment::Center)
            )
            // Day 2 (middle third)
            .add_field(
                Field::new_glyph(
                    "day2_icon",
                    Rectangle::new(Point::new((width / 2).saturating_sub(8) as i32, 24), Size::new(16, 16))
                )
            )
            .add_field(
                Field::new_text(
                    "day2_temp",
                    Rectangle::new(Point::new((width / 3) as i32, 42), Size::new(width / 3, 10)),
                    &FONT_5X8
                )
                .align(Alignment::Center)
            )
            // Day 3 (right third)
            .add_field(
                Field::new_glyph(
                    "day3_icon",
                    Rectangle::new(Point::new((width - 20) as i32, 24), Size::new(16, 16))
                )
            )
            .add_field(
                Field::new_text(
                    "day3_temp",
                    Rectangle::new(Point::new(((width / 3) * 2) as i32, 42), Size::new(width / 3, 10)),
                    &FONT_5X8
                )
                .align(Alignment::Center)
            )
    }

    /// Create the visualizer page layout
    pub fn create_visualizer_page(&self) -> PageLayout {
        let width = self.layout_config.width;
        let height = self.layout_config.height;

        PageLayout::new("visualizer")
            // Status bar at top
            .add_field(
                Field::new_text(
                    "status_bar",
                    Rectangle::new(Point::new(0, 0), Size::new(width, 10)),
                    &FONT_6X10
                )
            )
            // Visualizer area (custom rendering)
            .add_field(
                Field::new_custom(
                    "visualizer",
                    Rectangle::new(Point::new(0, 10), Size::new(width, height - 10))
                )
            )
    }

    /// Create the easter eggs page layout
    pub fn create_easter_eggs_page(&self) -> PageLayout {
        let width = self.layout_config.width;
        let height = self.layout_config.height;

        PageLayout::new("easter_eggs")
            // Full screen custom rendering for animations
            .add_field(
                Field::new_custom(
                    "animation",
                    Rectangle::new(Point::new(0, 0), Size::new(width, height))
                )
            )
    }

    /// Get the layout configuration
    pub fn layout_config(&self) -> &LayoutConfig {
        &self.layout_config
    }
}
