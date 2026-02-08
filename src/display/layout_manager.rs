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

use super::field::{Field, FieldType};
use super::page::PageLayout;
use super::layout::LayoutConfig;
use super::color::Color;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::iso_8859_13::{
    FONT_4X6,
    FONT_5X7, FONT_5X8,
    FONT_6X10, FONT_6X13_BOLD,
    FONT_7X14
};
use embedded_text::alignment::{HorizontalAlignment, VerticalAlignment};

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
                    Rectangle::new(
                        Point::new(border_adj, border_adj), 
                        Size::new(width_adj, 10)),
                    &FONT_6X10
                )
            )
            // didn't we have album_artist too ??? so for v/a we would have performer etc?
            // Artist (y=10)
            .add_field(
                Field::new_text(
                    "artist",
                    Rectangle::new(
                        Point::new(border_adj, border_adj+10), 
                        Size::new(width_adj, 10)),
                    &FONT_6X10
                )
                .scrollable(true)
            )
            // Album (y=20)
            .add_field(
                Field::new_text(
                    "album",
                    Rectangle::new(
                        Point::new(border_adj, border_adj+20), 
                        Size::new(width_adj, 10)),
                    &FONT_6X10
                )
                .scrollable(true)
            )
            // Title (y=30)
            .add_field(
                Field::new_text(
                    "title",
                    Rectangle::new(
                        Point::new(border_adj, border_adj+30), 
                        Size::new(width_adj, 10)),
                    &FONT_6X10
                )
                .scrollable(true)
            )
            // Progress bar (y=40, height=4)
            .add_field(
                Field::new_custom(
                    "progress_bar",
                    Rectangle::new(
                        Point::new(border_adj, height.saturating_sub(16) as i32), 
                        Size::new(width_adj, 4))
                )
            )
            // Info line (times) at bottom (y=62 for 128x64 displays)
            .add_field(
                Field::new_text(
                    "info_line",
                    Rectangle::new(
                        Point::new(border_adj, height.saturating_sub(10) as i32), 
                        Size::new(width_adj, 10)),
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
        let progress_bar_y = clock_y_start + CLOCK_DIGIT_HEIGHT + CLOCK_PROGRESS_BAR_GAP;
        let date_y = progress_bar_y + PROGRESS_BAR_HEIGHT + PROGRESS_BAR_DATE_GAP - 2;

        PageLayout::new("clock")
            // metrics if requested (centered)
            .add_field(
                Field::new_text(
                    "metrics",
                    Rectangle::new(
                        Point::new(border_adj, border_adj),
                        Size::new(width_adj, 7)),
                    &FONT_5X7
                )
                .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top)
            )
            // Clock digits - custom font with 2px border (green color)
            .add_field(
                Field::new_custom(
                    "clock_digits",
                    Rectangle::new(
                        Point::new(border_adj, clock_y_start),
                        Size::new(width_adj, CLOCK_DIGIT_HEIGHT as u32))
                )
                .colors(Color::Green, None)
            )
            // Seconds progress bar (full width)
            .add_field(
                Field::new_custom(
                    "seconds_progress",
                    Rectangle::new(
                        Point::new(border_adj, progress_bar_y), 
                        Size::new(width_adj, 4))
                )
            )
            // Date at bottom (centered, cyan color)
            .add_field(
                Field::new_text(
                    "date",
                    Rectangle::new(
                        Point::new(border_adj, date_y),
                        Size::new(width_adj, DATE_FONT_HEIGHT as u32)),
                    &FONT_6X10
                )
                .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top)
                .colors(Color::Cyan, None)
            )
    }

    /// Create the current weather page layout
    /// Based on original display_old.rs rendering at lines 2480-2563
    pub fn create_weather_current_page(&self) -> PageLayout {
        let width = self.layout_config.width;
        let height = self.layout_config.height;
        let is_wide = width > 128;

        // Original: icon_w = height/2 + 2 = 34 for 128x64
        let icon_size = height / 2 + 2;
        let glyph_w = 12;
        let mut glyph_x = 52;
        let mut text_x = glyph_x + 2 + glyph_w; // 66

        let mut page = PageLayout::new("weather_current")
            // Large weather icon - left side (original: 12, 10, 34x34)
            .add_field(
                Field::new_glyph(
                    "weather_icon",
                    Rectangle::new(
                        Point::new(12, 10), 
                        Size::new(icon_size, icon_size))
                )
            )
            // Temperature row with glyph (thermo icon at 52, 2)
            .add_field(
                Field::new_custom(
                    "temp_glyph",
                    Rectangle::new(
                        Point::new(glyph_x as i32, 2),
                        Size::new(glyph_w, glyph_w))
                )
                .colors(Color::Cyan, None)
            )
            .add_field(
                Field::new_text(
                    "temperature",  // "72(68) °F" format
                    Rectangle::new(
                        Point::new(text_x as i32, 2),
                        Size::new(width - text_x, 14)),
                    &FONT_6X13_BOLD
                )
                .colors(Color::Cyan, None)
            )
            // Humidity row with glyph (humidity icon at 52, 15)
            .add_field(
                Field::new_custom(
                    "humidity_glyph",
                    Rectangle::new(
                        Point::new(glyph_x as i32, 14),
                        Size::new(glyph_w, glyph_w))
                )
                .colors(Color::Cyan, None)
            )
            .add_field(
                Field::new_text(
                    "humidity",  // "65%" format
                    Rectangle::new(
                        Point::new(text_x as i32, 15),
                        Size::new(width - text_x, 12)),
                    &FONT_5X8
                )
                .colors(Color::Cyan, None)
            )
            // Wind row with glyph (wind icon at 52, 24)
            .add_field(
                Field::new_custom(
                    "wind_glyph",
                    Rectangle::new(
                        Point::new(glyph_x as i32, 24),
                        Size::new(glyph_w, glyph_w))
                )
                .colors(Color::Cyan, None)
            )
            .add_field(
                Field::new_text(
                    "wind",  // "10 mph NW" format
                    Rectangle::new(
                        Point::new(text_x as i32, 25),
                        Size::new(width - text_x, 12)),
                    &FONT_5X8
                )
                .colors(Color::Cyan, None)
            )
            // Precipitation row with glyph (rain icon at 52, 34)
            .add_field(
                Field::new_custom(
                    "precip_glyph",
                    Rectangle::new(
                        Point::new(glyph_x as i32, 34),
                        Size::new(glyph_w, glyph_w))
                )
                .colors(Color::Cyan, None)
            )
            .add_field(
                Field::new_text(
                    "precipitation",  // "20%" format
                    Rectangle::new(
                        Point::new(text_x as i32, 35),
                        Size::new(width - text_x, 12)),
                    &FONT_5X8
                )
                .colors(Color::Cyan, None)
            )
            // Conditions text centered at bottom (yellow, original: y=46+, FONT_7X14)
            .add_field(
                Field::new_text(
                    "conditions",
                    Rectangle::new(
                        Point::new(2, height as i32 - 16),
                        Size::new(width - 4, 14)),
                    &FONT_7X14
                )
                .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                .colors(Color::Yellow, None)
            );

        // Wide display additions (width > 128): Sunrise/Sunset/Moon on the right
        // we're going to want Moonrise, Moonset, and Moonphase
        if is_wide {

            glyph_x = 133;
            text_x = glyph_x + 2 + glyph_w;
            let astral_field_width = 33;

            page = page
                // Sunrise glyph
                .add_field(
                    Field::new_custom(
                        "sunrise_glyph",
                        Rectangle::new(
                            Point::new(glyph_x as i32, 2),
                            Size::new(glyph_w, glyph_w))
                    )
                    .colors(Color::Yellow, None)
                )
                // Sunrise text
                .add_field(
                    Field::new_text(
                        "sunrise_text",
                        Rectangle::new(
                            Point::new(text_x as i32, 3),
                            Size::new(astral_field_width, 10)),
                        &FONT_5X8
                    )
                    .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                    .colors(Color::Yellow, None)
                )
                .add_field(
                    Field::new_custom(
                        "sunset_glyph",
                        Rectangle::new(
                            Point::new(glyph_x as i32, 16),
                            Size::new(glyph_w, glyph_w))
                    )
                    .colors(Color::Yellow, None)
                )
                // Sunset text
                .add_field(
                    Field::new_text(
                        "sunset_text",
                        Rectangle::new(
                            Point::new(text_x as i32, 17),
                            Size::new(astral_field_width, 10)),
                        &FONT_5X8
                    )
                    .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                    .colors(Color::Yellow, None)
                );

            glyph_x += 40;
            text_x = glyph_x + 2 + glyph_w;
            page = page
                // Moonrise glyph
                .add_field(
                    Field::new_custom(
                        "moonrise_glyph",
                        Rectangle::new(
                            Point::new(glyph_x as i32, 2),
                            Size::new(glyph_w, glyph_w))
                    )
                    .colors(Color::Cyan, None)
                )
                // moonrise text
                .add_field(
                    Field::new_text(
                        "moonrise_text",
                        Rectangle::new(
                            Point::new(text_x as i32, 3),
                            Size::new(astral_field_width, 10)),
                        &FONT_5X8
                    )
                    .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                    .colors(Color::Cyan, None)
                )
                // Moonset glyph
                .add_field(
                    Field::new_custom(
                        "moonset_glyph",
                        Rectangle::new(
                            Point::new(glyph_x as i32, 16),
                            Size::new(glyph_w, glyph_w))
                    )
                    .colors(Color::Cyan, None)
                )
                // moonset text
                .add_field(
                    Field::new_text(
                        "moonset_text",
                        Rectangle::new(
                            Point::new(text_x as i32, 17),
                            Size::new(astral_field_width, 10)),
                        &FONT_5X8
                    )
                    .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                    .colors(Color::Cyan, None)
                )
;
            glyph_x += 40;
            page = page
                // Moonphase svg
                .add_field(
                    Field::new_custom(
                        "moonphase_svg",
                        Rectangle::new(
                            Point::new(glyph_x as i32, 10),
                            Size::new(34, 34))
                    )
                );
        }

        page
    }

    /// Create the weather forecast page layout
    /// Based on original display_old.rs rendering at lines 2565-2664
    pub fn create_weather_forecast_page(&self) -> PageLayout {
        let width = self.layout_config.width;
        let height = self.layout_config.height;
        let is_wide = width > 128;

        // Original: icon_w = height/2 + 2 = 34, icon size = icon_w - 4 = 30
        let header_y = 2;
        let header_height = 10;
        let icon_w = height / 2 + 2;
        let icon_size = icon_w - 4;  // 30 for 128x64
        let spacing = icon_w + 6;    // 40 pixels - column width

        // Start leftmost column at x=4, use spacing (40px) for column width
        let col_x1 = 4;
        let col_x2 = col_x1 + spacing as i32;  // 44
        let col_x3 = col_x2 + spacing as i32;  // 84
        let col_x4 = col_x3 + spacing as i32;  // 124 (wide display)
        let col_x5 = col_x4 + spacing as i32;  // 164 (wide display)
        let col_x6 = col_x5 + spacing as i32;  // 204 (wide display)

        // Center icons (30px) within columns (40px): offset = (40-30)/2 = 5
        let icon_offset = (spacing - icon_size) / 2;
        let icon_x1 = col_x1 + icon_offset as i32;  // 9
        let icon_x2 = col_x2 + icon_offset as i32;  // 49
        let icon_x3 = col_x3 + icon_offset as i32;  // 89
        let icon_x4 = col_x4 + icon_offset as i32;  // 129 (wide display)
        let icon_x5 = col_x5 + icon_offset as i32;  // 169 (wide display)
        let icon_x6 = col_x6 + icon_offset as i32;  // 209 (wide display)

        let mut page = PageLayout::new("weather_forecast")
            // Day 1 column (x=4, width=40)
            .add_field(Field::new_glyph("day1_icon",Rectangle::new(Point::new(icon_x1, 1), Size::new(icon_size, icon_size))))
            // day name. Mon, Tue... etc
            .add_field(Field::new_text("day1_name",Rectangle::new(Point::new(col_x1, icon_size as i32 + header_y),Size::new(spacing, header_height)),&FONT_4X6)
                .styled_alignment(HorizontalAlignment::Center,VerticalAlignment::Middle)
                .colors(Color::Cyan, None)
                .border(1))
            // Bordered box for temp+precip
            .add_field(Field::new_custom("day1_data_box", Rectangle::new(Point::new(col_x1, icon_size as i32 + 12),Size::new(spacing, 22))).border(1))
            // "45°F|62°F" format - 3px down from box top, inset by 1px for border
            .add_field(Field::new_text("day1_temp",Rectangle::new(Point::new(col_x1 + 1, icon_size as i32 + 15),Size::new(spacing - 2, 7)),&FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
            // "20%" format - below temp, inset by 1px for border
            .add_field(Field::new_text("day1_precip",Rectangle::new(Point::new(col_x1 + 1, icon_size as i32 + 22),Size::new(spacing - 2, 10)),&FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
            // Day 2 column (x=44, width=40)
            .add_field(Field::new_glyph("day2_icon",Rectangle::new(Point::new(icon_x2, 1), Size::new(icon_size, icon_size))))
            .add_field(Field::new_text("day2_name",Rectangle::new(Point::new(col_x2, icon_size as i32 + header_y),Size::new(spacing, header_height)),&FONT_4X6)
                .styled_alignment(HorizontalAlignment::Center,VerticalAlignment::Middle)
                .colors(Color::Cyan, None)
                .border(1))
            // Bordered box for temp+precip
            .add_field(Field::new_custom("day2_data_box",Rectangle::new(Point::new(col_x2, icon_size as i32 + 12),Size::new(spacing, 22))).border(1))
            // "45°F|62°F" format
            .add_field(Field::new_text("day2_temp", Rectangle::new(Point::new(col_x2 + 1, icon_size as i32 + 15),Size::new(spacing - 2, 7)),&FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
            // "20%" format
            .add_field(Field::new_text("day2_precip",Rectangle::new(Point::new(col_x2 + 1, icon_size as i32 + 22),Size::new(spacing - 2, 10)),&FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
            // Day 3 column (x=84, width=40)
            .add_field(Field::new_glyph("day3_icon",Rectangle::new(Point::new(icon_x3, 1), Size::new(icon_size, icon_size))))
            .add_field(Field::new_text("day3_name",Rectangle::new(Point::new(col_x3, icon_size as i32 + header_y),Size::new(spacing, header_height)),&FONT_4X6)
                .styled_alignment(HorizontalAlignment::Center,VerticalAlignment::Middle)
                .colors(Color::Cyan, None)
                .border(1))
            // Bordered box for temp+precip
            .add_field(Field::new_custom("day3_data_box",Rectangle::new(Point::new(col_x3, icon_size as i32 + 12),Size::new(spacing, 22))).border(1))
            // "45°F|62°F" format
            .add_field(Field::new_text("day3_temp", Rectangle::new(Point::new(col_x3 + 1, icon_size as i32 + 15),Size::new(spacing - 2, 7)),&FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
            // "20%" format
            .add_field(Field::new_text("day3_precip", Rectangle::new(Point::new(col_x3 + 1, icon_size as i32 + 22),Size::new(spacing - 2, 10)),&FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None));

        // Wide display additions (width > 128): Days 4-6
        if is_wide {
            page = page
                // Day 4 column
                .add_field(Field::new_glyph("day4_icon", Rectangle::new(Point::new(icon_x4, 1), Size::new(icon_size, icon_size))))
                .add_field(Field::new_text("day4_name", Rectangle::new(Point::new(col_x4, icon_size as i32 + header_y), Size::new(spacing, header_height)), &FONT_4X6)
                    .styled_alignment(HorizontalAlignment::Center,VerticalAlignment::Middle)
                    .colors(Color::Cyan, None)
                    .border(1))
                .add_field(Field::new_custom("day4_data_box", Rectangle::new(Point::new(col_x4, icon_size as i32 + 12), Size::new(spacing, 22))).border(1))
                .add_field(Field::new_text("day4_temp", Rectangle::new(Point::new(col_x4 + 1, icon_size as i32 + 15), Size::new(spacing - 2, 7)), &FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
                .add_field(Field::new_text("day4_precip", Rectangle::new(Point::new(col_x4 + 1, icon_size as i32 + 22), Size::new(spacing - 2, 10)), &FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
                // Day 5 column
                .add_field(Field::new_glyph("day5_icon", Rectangle::new(Point::new(icon_x5, 1), Size::new(icon_size, icon_size))))
                .add_field(Field::new_text("day5_name", Rectangle::new(Point::new(col_x5, icon_size as i32 + header_y), Size::new(spacing, header_height)), &FONT_4X6)
                    .styled_alignment(HorizontalAlignment::Center,VerticalAlignment::Middle)
                    .colors(Color::Cyan, None)
                    .border(1))
                .add_field(Field::new_custom("day5_data_box", Rectangle::new(Point::new(col_x5, icon_size as i32 + 12), Size::new(spacing, 22))).border(1))
                .add_field(Field::new_text("day5_temp", Rectangle::new(Point::new(col_x5 + 1, icon_size as i32 + 15), Size::new(spacing - 2, 7)), &FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
                .add_field(Field::new_text("day5_precip", Rectangle::new(Point::new(col_x5 + 1, icon_size as i32 + 22), Size::new(spacing - 2, 10)), &FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
                // Day 6 column
                .add_field(Field::new_glyph("day6_icon", Rectangle::new(Point::new(icon_x6, 1), Size::new(icon_size, icon_size))))
                .add_field(Field::new_text("day6_name", Rectangle::new(Point::new(col_x6, icon_size as i32 + header_y), Size::new(spacing, header_height)), &FONT_4X6)
                    .styled_alignment(HorizontalAlignment::Center,VerticalAlignment::Middle)
                    .colors(Color::Cyan, None)
                    .border(1))
                .add_field(Field::new_custom("day6_data_box", Rectangle::new(Point::new(col_x6, icon_size as i32 + 12), Size::new(spacing, 22))).border(1))
                .add_field(Field::new_text("day6_temp", Rectangle::new(Point::new(col_x6 + 1, icon_size as i32 + 15), Size::new(spacing - 2, 7)), &FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None))
                .add_field(Field::new_text("day6_precip", Rectangle::new(Point::new(col_x6 + 1, icon_size as i32 + 22), Size::new(spacing - 2, 10)), &FONT_4X6).styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Top).colors(Color::Cyan, None));
        }

        page
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

    /// Create the splash screen layout
    /// Shows: logo SVG (background), version, build date, and optional status message
    pub fn create_splash_page(&self) -> PageLayout {
        let width = self.layout_config.width;
        let height = self.layout_config.height;

        // Calculate positions similar to original splash
        // Version is 17px above bottom, build date at bottom - 10px
        let version_y = (height as i32) - 10 - 17;  // ~37 for 64px height
        let build_y = (height as i32) - 10;         // ~54 for 64px height

        PageLayout::new("splash")
            // Logo SVG (full screen background) - custom field for SVG rendering
            .add_field(
                Field::new_custom(
                    "logo_svg",
                    Rectangle::new(Point::new(0, 0), Size::new(width, height))
                )
            )
            // Version string (e.g., "LyMonS v0.2.3")
            .add_field(
                Field::new_text(
                    "version",
                    Rectangle::new(Point::new(0, version_y), Size::new(width, 15)),
                    &FONT_6X13_BOLD
                )
                .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                .colors(Color::White, None)
            )
            // Build date (e.g., "2026-02-04")
            .add_field(
                Field::new_text(
                    "build_date",
                    Rectangle::new(Point::new(0, build_y), Size::new(width, 10)),
                    &FONT_5X8
                )
                .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                .colors(Color::Cyan, None)
            )
            // Status/task message (optional, for showing initialization progress)
            // Positioned in middle area, above version
            .add_field(
                Field::new_text(
                    "status",
                    Rectangle::new(Point::new(0, (height / 2) as i32), Size::new(width, 8)),
                    &FONT_5X8
                )
                .styled_alignment(HorizontalAlignment::Center, VerticalAlignment::Middle)
                .colors(Color::Green, None)
            )
    }

    /// Get the layout configuration
    pub fn layout_config(&self) -> &LayoutConfig {
        &self.layout_config
    }
}
