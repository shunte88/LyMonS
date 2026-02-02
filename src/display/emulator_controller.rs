/*
 *  display/emulator_controller.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Emulator display controller that mirrors OledDisplay interface
 *  Allows emulator to use the same main loop logic as hardware
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 */

use super::drivers::emulator::EmulatorDriver;
use super::traits::DisplayDriver;
use super::error::DisplayError;
use super::DisplayMode;
use crate::textable::{ScrollMode, TextScroller, transform_scroll_mode, GAP_BETWEEN_LOOP_TEXT_FIXED};
use crate::eggs::{Eggs, set_easter_egg};
use crate::clock_font::{ClockFontData, set_clock_font};
use crate::display_old::{RepeatMode, ShuffleMode};
use crate::deutils::seconds_to_hms;
use crate::constants;
use crate::glyphs;

use embedded_graphics::{
    mono_font::{iso_8859_13::FONT_5X8, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Rectangle, PrimitiveStyle},
    text::Text,
};
use crate::draw::{clear_region, draw_text, draw_rectangle};
use log::{info, error};
use std::time::Instant;

/// Audio bitrate categories
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioBitrate {
    None,
    SD,   // Standard Definition (lossy/low bitrate)
    HD,   // High Definition (16-24bit/44.1-192kHz)
    DSD,  // Direct Stream Digital
}

/// Emulator display controller that provides OledDisplay-compatible interface
/// This allows the emulator to use the same main loop logic as hardware
pub struct EmulatorDisplayController {
    driver: EmulatorDriver,
    scrollers: Vec<TextScroller>,

    // Status line data
    volume_percent: u8,
    is_muted: bool,
    repeat_mode: RepeatMode,
    shuffle_mode: ShuffleMode,
    audio_bitrate: AudioBitrate,
    bitrate_text: String,

    // Display mode
    pub current_mode: DisplayMode,

    // Clock state
    last_clock_digits: [char; 5],
    colon_on: bool,
    last_colon_toggle_time: Instant,
    clock_font: ClockFontData<'static>,
    last_second_drawn: f32,
    last_date_drawn: String,

    // Player progress state
    pub track_duration_secs: f32,
    pub current_track_time_secs: f32,
    pub remaining_time_secs: f32,
    pub mode_text: String,
    pub show_remaining: bool,
    last_current_track_time_secs: f32,
    last_track_duration_secs: f32,
    last_remaining_time_secs: f32,
    last_mode_text: String,

    // Easter eggs
    pub easter_egg: Eggs,
    pub show_metrics: bool,
}

impl EmulatorDisplayController {
    pub fn new(
        mut driver: EmulatorDriver,
        scroll_mode: &str,
        clock_font: &str,
        show_metrics: bool,
        egg_name: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing EmulatorDisplayController");

        DisplayDriver::init(&mut driver)?;

        let scroll_mode_enum = transform_scroll_mode(scroll_mode);
        let clock_font_data = set_clock_font(clock_font);
        let easter_egg = set_easter_egg(egg_name);

        Ok(Self {
            driver,
            scrollers: Vec::new(),
            volume_percent: 0,
            is_muted: false,
            repeat_mode: RepeatMode::Off,
            shuffle_mode: ShuffleMode::Off,
            audio_bitrate: AudioBitrate::None,
            bitrate_text: String::new(),
            current_mode: DisplayMode::Scrolling,
            last_clock_digits: ['0', '0', ':', '0', '0'],
            colon_on: true,
            last_colon_toggle_time: Instant::now(),
            clock_font: clock_font_data,
            last_second_drawn: 0.0,
            last_date_drawn: String::new(),
            track_duration_secs: 0.0,
            current_track_time_secs: 0.0,
            remaining_time_secs: 0.0,
            mode_text: String::new(),
            show_remaining: false,
            last_current_track_time_secs: 0.0,
            last_track_duration_secs: 0.0,
            last_remaining_time_secs: 0.0,
            last_mode_text: String::new(),
            easter_egg,
            show_metrics,
        })
    }

    /// Set status line data (called from main loop when has_changed())
    pub fn set_status_line_data(
        &mut self,
        volume: u8,
        is_muted: bool,
        samplesize: String,
        samplerate: String,
        repeat: RepeatMode,
        shuffle: ShuffleMode,
    ) {
        self.volume_percent = volume;
        self.is_muted = is_muted;
        self.repeat_mode = repeat;
        self.shuffle_mode = shuffle;

        // Determine audio bitrate category
        let sample_size_int: u32 = samplesize.parse().unwrap_or(0);
        let sample_rate_int: u32 = samplerate.parse().unwrap_or(0);

        self.audio_bitrate = if sample_size_int == 1 && sample_rate_int > 0 {
            AudioBitrate::DSD
        } else if sample_size_int >= 24 || sample_rate_int >= 96000 {
            AudioBitrate::HD
        } else if sample_size_int > 0 && sample_rate_int > 0 {
            AudioBitrate::SD
        } else {
            AudioBitrate::None
        };

        // Format bitrate text
        self.bitrate_text = if sample_size_int > 0 && sample_rate_int > 0 {
            format!("{}/{}", sample_size_int, sample_rate_int / 1000)
        } else {
            String::new()
        };
    }

    /// Set track details (called from main loop when has_changed())
    pub async fn set_track_details(
        &mut self,
        _album_artist: String,
        album: String,
        title: String,
        artist: String,
        scroll_mode: &str,
    ) {
        // Clear existing scrollers
        for scroller in &mut self.scrollers {
            scroller.stop();
        }
        self.scrollers.clear();

        let scroll_mode_enum = transform_scroll_mode(scroll_mode);

        // Create scrollers for album, title, artist
        // Y positions from constants.rs (assuming 128x64 display)
        let y_positions = [18, 27, 36, 45]; // album_artist, album, title, artist

        // Album scroller (Y=27)
        if !album.is_empty() {
            let scroller = TextScroller::new(
                "album".to_string(),
                Point::new(0, y_positions[1]),
                128,
                album,
                FONT_5X8,
                scroll_mode_enum,
            );
            self.scrollers.push(scroller);
        }

        // Title scroller (Y=36)
        if !title.is_empty() {
            let scroller = TextScroller::new(
                "title".to_string(),
                Point::new(0, y_positions[2]),
                128,
                title,
                FONT_5X8,
                scroll_mode_enum,
            );
            self.scrollers.push(scroller);
        }

        // Artist scroller (Y=45)
        if !artist.is_empty() {
            let scroller = TextScroller::new(
                "artist".to_string(),
                Point::new(0, y_positions[3]),
                128,
                artist,
                FONT_5X8,
                scroll_mode_enum,
            );
            self.scrollers.push(scroller);
        }
    }

    /// Set track progress data (called from main loop when has_changed())
    pub fn set_track_progress_data(
        &mut self,
        show_remaining: bool,
        duration: f32,
        elapsed: f32,
        remaining: f32,
        mode: String,
    ) {
        self.show_remaining = show_remaining;
        self.track_duration_secs = duration;
        self.current_track_time_secs = elapsed;
        self.remaining_time_secs = remaining;
        self.mode_text = mode;
    }

    /// Set display mode
    pub async fn set_display_mode(&mut self, mode: DisplayMode) {
        self.current_mode = mode;
    }

    /// Get easter egg type
    pub fn get_egg_type(&self) -> u8 {
        self.easter_egg.egg_type
    }

    /// Render frame (called from main loop)
    pub async fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.current_mode {
            DisplayMode::Clock => {
                self.render_clock()?;
            },
            DisplayMode::EasterEggs => {
                // TODO: Implement easter eggs
                self.render_scrolling().await?;
            },
            DisplayMode::Visualizer => {
                // TODO: Implement visualizer
                self.render_scrolling().await?;
            },
            DisplayMode::WeatherCurrent | DisplayMode::WeatherForecast => {
                // TODO: Implement weather
                self.render_clock()?;
            },
            DisplayMode::Scrolling => {
                self.render_scrolling().await?;
            },
        }

        DisplayDriver::flush(&mut self.driver)?;
        Ok(())
    }

    /// Render scrolling mode (main playback display)
    async fn render_scrolling(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Clear display
        DisplayDriver::clear(&mut self.driver)?;

        // === STATUS LINE (Y=0-9) ===
        let mut current_x = 0i32;

        // Volume glyph and text
        let vol_glyph = if self.is_muted || self.volume_percent == 0 {
            &glyphs::GLYPH_VOLUME_OFF
        } else {
            &glyphs::GLYPH_VOLUME_ON
        };
        self.draw_glyph(vol_glyph, current_x, 0)?;
        current_x += constants::GLYPH_WIDTH as i32;

        let vol_text = if self.is_muted || self.volume_percent == 0 {
            "mute".to_string()
        } else {
            format!("{:>3}%", self.volume_percent)
        };
        draw_text(&mut self.driver, &vol_text, current_x, 0, &FONT_5X8)?;

        // Shuffle glyph (center-left)
        let shuffle_glyph = if self.shuffle_mode == ShuffleMode::ByTracks {
            &glyphs::GLYPH_SHUFFLE_TRACKS
        } else if self.shuffle_mode == ShuffleMode::ByAlbums {
            &glyphs::GLYPH_SHUFFLE_ALBUMS
        } else {
            &glyphs::GLYPH_NONE
        };
        self.draw_glyph(shuffle_glyph, 44, 0)?;

        // Repeat glyph (center-left)
        let repeat_glyph = if self.repeat_mode == RepeatMode::RepeatOne {
            &glyphs::GLYPH_REPEAT_ONE
        } else if self.repeat_mode == RepeatMode::RepeatAll {
            &glyphs::GLYPH_REPEAT_ALL
        } else {
            &glyphs::GLYPH_NONE
        };
        self.draw_glyph(repeat_glyph, 34, 0)?;

        // Bitrate text (right side)
        if !self.bitrate_text.is_empty() {
            let bitrate_x = 128 - (self.bitrate_text.len() as i32 * 5) - 10;
            draw_text(&mut self.driver, &self.bitrate_text, bitrate_x, 0, &FONT_5X8)?;
        }

        // Audio quality glyph (far right)
        let audio_glyph = match self.audio_bitrate {
            AudioBitrate::HD => &glyphs::GLYPH_AUDIO_HD,
            AudioBitrate::SD => &glyphs::GLYPH_AUDIO_SD,
            AudioBitrate::DSD => &glyphs::GLYPH_AUDIO_DSD,
            AudioBitrate::None => &glyphs::GLYPH_NONE,
        };
        let audio_x = 128 - constants::GLYPH_WIDTH as i32;
        self.draw_glyph(audio_glyph, audio_x, 0)?;

        // === SCROLLING TEXT (Y=18, 27, 36, 45) ===
        for scroller in &mut self.scrollers {
            let mut scroller_state = scroller.state.lock().await;
            let current_text = scroller_state.text.clone();
            let text_width = scroller_state.text_width;
            let current_mode = scroller_state.scroll_mode;

            let top_left = scroller.top_left;
            let x_start = top_left.x;
            let y_start = top_left.y;

            let current_x_rounded = scroller_state.current_offset_float.round() as i32;

            // Draw main text
            let draw_x_main = x_start + current_x_rounded;
            draw_text(&mut self.driver, &current_text, draw_x_main, y_start, &FONT_5X8)?;

            // For continuous loop, draw second copy
            if current_mode == ScrollMode::ScrollLeft {
                let second_copy_x = draw_x_main + text_width as i32 + GAP_BETWEEN_LOOP_TEXT_FIXED;
                draw_text(&mut self.driver, &current_text, second_copy_x, y_start, &FONT_5X8)?;
            }

            scroller_state.last_drawn_x_rounded = current_x_rounded;
        }

        // === PROGRESS BAR (Y=51-55) ===
        let progress_bar_x = 2;
        let progress_bar_y = constants::PLAYER_PROGRESS_BAR_Y_POS;

        if self.track_duration_secs > 0.0 {
            // Draw outline
            draw_rectangle(
                &mut self.driver,
                Point::new(progress_bar_x, progress_bar_y),
                constants::PLAYER_PROGRESS_BAR_WIDTH,
                constants::PLAYER_PROGRESS_BAR_HEIGHT,
                BinaryColor::Off,
                Some(1),
                Some(BinaryColor::On),
            )?;

            // Draw fill
            let progress = (self.current_track_time_secs / self.track_duration_secs).clamp(0.0, 1.0);
            let fill_width = ((constants::PLAYER_PROGRESS_BAR_WIDTH - 2) as f32 * progress) as u32;

            if fill_width > 0 {
                draw_rectangle(
                    &mut self.driver,
                    Point::new(progress_bar_x + 1, progress_bar_y + 1),
                    fill_width,
                    constants::PLAYER_PROGRESS_BAR_HEIGHT - 2,
                    BinaryColor::On,
                    None,
                    None,
                )?;
            }
        }

        // === INFO LINE (Y=56) ===
        let info_line_y = constants::PLAYER_TRACK_INFO_LINE_Y_POS;

        // Current time (left)
        let current_time_str = seconds_to_hms(self.current_track_time_secs);
        draw_text(&mut self.driver, &current_time_str, 0, info_line_y, &FONT_5X8)?;

        // Mode text (center)
        let mode_text_width = (self.mode_text.len() * 5) as i32;
        let mode_x = (128 - mode_text_width) / 2;
        draw_text(&mut self.driver, &self.mode_text, mode_x, info_line_y, &FONT_5X8)?;

        // Remaining/total time (right)
        let time_str = if self.show_remaining {
            format!("-{}", seconds_to_hms(self.remaining_time_secs))
        } else {
            format!(" {}", seconds_to_hms(self.track_duration_secs))
        };
        let time_width = (time_str.len() * 5) as i32;
        let time_x = 128 - time_width;
        draw_text(&mut self.driver, &time_str, time_x, info_line_y, &FONT_5X8)?;

        Ok(())
    }

    /// Render clock mode
    fn render_clock(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        DisplayDriver::clear(&mut self.driver)?;

        use chrono::{Local, Timelike};
        let now = Local::now();

        // Draw time
        let time_str = format!("{:02}:{:02}", now.time().hour(), now.time().minute());

        // Use embedded_graphics Text for now (clock font rendering is complex)
        let style = MonoTextStyle::new(&embedded_graphics::mono_font::ascii::FONT_9X18_BOLD, BinaryColor::On);
        Text::new(&time_str, Point::new(30, 35), style)
            .draw(&mut self.driver)?;

        // Draw date
        let date_str = now.format("%Y-%m-%d").to_string();
        let date_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
        Text::new(&date_str, Point::new(20, 50), date_style)
            .draw(&mut self.driver)?;

        Ok(())
    }

    /// Draw a glyph at the specified position
    fn draw_glyph(&mut self, glyph: &[u8], x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
        // Glyphs are 8x8 bitmaps
        use embedded_graphics::Pixel as EgPixel;
        let mut pixels = Vec::new();

        for (row, byte) in glyph.iter().enumerate() {
            for col in 0..8 {
                if (byte >> (7 - col)) & 1 == 1 {
                    let pixel_x = x + col as i32;
                    let pixel_y = y + row as i32;
                    if pixel_x >= 0 && pixel_x < 128 && pixel_y >= 0 && pixel_y < 64 {
                        pixels.push(EgPixel(Point::new(pixel_x, pixel_y), BinaryColor::On));
                    }
                }
            }
        }

        self.driver.draw_iter(pixels.into_iter())?;
        Ok(())
    }

    // Stub methods for compatibility with OledDisplay interface
    pub fn connections(&mut self, _inet: &str, _eth0: &str, _wlan0: &str) {}
    pub async fn splash(&mut self, _show: bool, _version: &str, _build_date: &str) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    pub async fn setup_weather(&mut self, _config: &str) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    pub async fn test(&mut self, _run: bool) {}
    pub async fn setup_visualizer(&mut self, _viz_type: &str, _receiver: tokio::sync::watch::Receiver<Option<crate::visualizer::VizFrameOut>>) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    pub async fn is_weather_active(&self) -> bool { false }
}

impl Drop for EmulatorDisplayController {
    fn drop(&mut self) {
        info!("EmulatorDisplayController dropped");
        let _ = DisplayDriver::clear(&mut self.driver);
        let _ = DisplayDriver::flush(&mut self.driver);
        for scroller in &mut self.scrollers {
            scroller.stop();
        }
    }
}
