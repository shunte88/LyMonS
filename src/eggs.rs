/*
 *  eggs.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-25 Stuart Hunter
 *
 *	TODO:
 *
 *	This program is free software: you can redistribute it and/or modify
 *	it under the terms of the GNU General Public License as published by
 *	the Free Software Foundation, either version 3 of the License, or
 *	(at your option) any later version.
 *
 *	This program is distributed in the hope that it will be useful,
 *	but WITHOUT ANY WARRANTY; without even the implied warranty of
 *	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *	GNU General Public License for more details.
 *
 *	See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *	Public License.
 *
 */

//! Module for Easter Egg Support.
//!
use chrono::{Timelike, Local};
use embedded_graphics::{
    prelude::*,
    primitives::Rectangle,
};

use log::{info};
use std::error::Error;
use std::fmt;
use std::fs;

use crate::svgimage::SvgImageRenderer;
use crate::visualization::SvgColorDepth;

pub const EGGS_TYPE_BASS: u8 = 10;
pub const EGGS_TYPE_CASSETTE: u8 = 20;
pub const EGGS_TYPE_IBMPC: u8 = 30;
pub const EGGS_TYPE_MOOG: u8 = 40;
pub const EGGS_TYPE_PIPBOY: u8 = 45;
pub const EGGS_TYPE_RADIO40: u8 = 50;
pub const EGGS_TYPE_RADIO50: u8 = 60;
pub const EGGS_TYPE_REEL2REEL: u8 = 70;
pub const EGGS_TYPE_SCOPE: u8 = 80;
pub const EGGS_TYPE_TECHNICS: u8 = 90;
pub const EGGS_TYPE_TUBEAMP: u8 = 100;
pub const EGGS_TYPE_TVTIME: u8 = 110;
pub const EGGS_TYPE_VCR: u8 = 120;
pub const EGGS_TYPE_UNKNOWN: u8 = 255;

pub const NO_WIDE_ASSETS: [u8; 1] = [EGGS_TYPE_CASSETTE];

/// Custom error type for Eggs rendering operations.
#[derive(Debug)]
pub enum EggsError {
    /// Error parsing egg configuration.
    _EggParseError(String),
    EggRenderError(String),
    EggBufferError(String),
}

impl fmt::Display for EggsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EggsError::_EggParseError(msg) => write!(f, "Egg parse error: {}", msg),
            EggsError::EggRenderError(msg) => write!(f, "Egg SVG render error: {}", msg),
            EggsError::EggBufferError(msg) => write!(f, "Egg buffer render error: {}", msg),
        }
    }
}

impl Error for EggsError {}

#[derive(Clone, Debug, PartialEq)]
struct AnimState {
    tick: u32,
    frames_per_cycle: u32,
}

impl AnimState {

    fn new() -> Self
    {
        Self {
            tick: 0,
            frames_per_cycle: 20,
        }
    }

    fn advance(&mut self) {
        self.tick = (self.tick + 1) % self.frames_per_cycle;
    }

    fn inchworm(&self) -> (f64, f64) {
        let t = self.tick as f64 / self.frames_per_cycle as f64;
        if t < 0.5 {
            // Phase 1: stretch - front moves, back stays
            let p = t * 2.0;
            let stretch = 1.0 + 0.3 * p;  // widen
            let offset = 5.0 * p;          // shift forward
            (stretch, offset)
        } else {
            // Phase 2: pull - body compresses, back catches up
            let p = (t - 0.5) * 2.0;
            let stretch = 1.3 - 0.3 * p;  // back to normal
            let offset = 5.0 + 5.0 * p;   // continue forward
            (stretch, offset)
        }
    }

    fn bounce_offset(&self) -> f64 {
        let t = self.tick as f64 / self.frames_per_cycle as f64;
        let height = 20.0;
        // Simple parabolic bounce
        let phase = (t * std::f64::consts::PI).sin();
        height * phase
    }
}

/// Renders Easter Egg [animation] from SVG data.
#[derive(Clone, Debug, PartialEq)]
pub struct Eggs {
    pub egg_type: u8,
    rect: Rectangle,
    svg_data: String,
    modified_svg_data: String,
    buffer: Vec<u8>,
    artist_rect: Rectangle,
    artist: String,
    title_rect: Rectangle,
    title: String,
    combine: bool,
    low_limit: f64,
    high_limit: f64,
    time_rect: Rectangle,
    track_pcnt: f64,
    track_time_secs: f32,
    can_widen: bool,
    groovy: AnimState,
    re: String,
}

/// Loads/sets the active easter_egg
pub fn set_easter_egg(egg_name: &str) -> Eggs {
    info!("Load egg: {}",egg_name);
    match egg_name {
        "bass" => {
            Eggs::new(
                EGGS_TYPE_BASS,
                "./assets/bass.svg",
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(4,4), Size::new(120,6)),
                Rectangle::new(Point::new(4,12), Size::new(120,6)),
                48.0,
                88.0,
                false,
                Rectangle::new(Point::new(48,26), Size::new(48,12)),
                false,
            )
        },
        "cassette" => {
            Eggs::new(
                EGGS_TYPE_CASSETTE,
                "./assets/compactcassette.svg",
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(18,6), Size::new(90,6)), 
                Rectangle::new(Point::new(18,12), Size::new(90,6)), 
                // 13.5=0%, 0=100%
                // scale in play here for tape reels
                25.000,  // 25.000 small right
                48.578, // 48.578 large left
                false,
                // empty time rect
                Rectangle::new(Point::zero(), Size::new(0,0)),
                false,
            )
        },
        "ibmpc" => {
            Eggs::new(
                EGGS_TYPE_IBMPC,
                "./assets/ibmpc.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(70,3), Size::new(64,58)), 
                Rectangle::new(Point::zero(), Size::new(0,0)),
                0.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(70,52), Size::new(54,12)),
                true,
            )
        },
        "moog" => {
            Eggs::new(
                EGGS_TYPE_MOOG,
                "./assets/moog.svg",
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(83,3), Size::new(41,58)), 
                Rectangle::new(Point::zero(), Size::new(0,0)), 
                -10.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(83,52), Size::new(41,12)),
                true,
            )
        },
        "pipboy" => {
            Eggs::new(
                EGGS_TYPE_PIPBOY,
                "./assets/pipboy.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::zero(), Size::new(0,0)), 
                Rectangle::new(Point::zero(), Size::new(0,0)),
                0.0, 
                0.0, 
                false,
                Rectangle::new(Point::new(72,52), Size::new(52,12)),
                false,
            )
        },
        "reel2reel" => {
            Eggs::new(
                EGGS_TYPE_REEL2REEL,
                "./assets/reel2reels.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(72,3), Size::new(52,58)), 
                Rectangle::new(Point::zero(), Size::new(0,0)),
                0.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(72,52), Size::new(52,12)),
                true,
            )
            },
        "radio40" => {
            Eggs::new(
                EGGS_TYPE_RADIO40,
                "./assets/radio40s.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(64,3), Size::new(60,58)), 
                Rectangle::new(Point::zero(), Size::new(0,0)),
                -5.0, 
                5.0, 
                true,
                Rectangle::new(Point::new(64,52), Size::new(60,12)),
                true,
            )
            },
        "radio50" => {
            Eggs::new(
                EGGS_TYPE_RADIO50,
                "./assets/radio50s.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(74,3), Size::new(46,58)), 
                Rectangle::new(Point::zero(), Size::new(0,0)),
                0.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(74,52), Size::new(46,12)),
                true,
            )
            },
        "scope" => {
            Eggs::new(
                EGGS_TYPE_SCOPE,
                "./assets/scope.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(65,3), Size::new(59,58)), 
                Rectangle::new(Point::zero(), Size::new(0,0)),
                0.0, 
                10.0, 
                true,
                Rectangle::new(Point::new(65,52), Size::new(59,12)),
                true,
            )
        },
        "technics" => {
            Eggs::new(
                EGGS_TYPE_TECHNICS,
                "./assets/sl1200.svg",
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(85,5), Size::new(39,56)),
                Rectangle::new(Point::zero(), Size::new(0,0)),
                -10.0,
                12.0,
                true,
                Rectangle::new(Point::new(85,52), Size::new(39,12)),
                true,
            )
            },
        "tubeamp" => {        
            Eggs::new(
                EGGS_TYPE_TUBEAMP,
                "./assets/tubeampd.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(87,3), Size::new(37,58)), 
                Rectangle::new(Point::zero(), Size::new(0,0)),
                0.0, 
                100.0, 
                true,
                Rectangle::new(Point::new(87,52), Size::new(37,12)),
                true,
            )
            },
        "tvtime" => {
            Eggs::new(
                EGGS_TYPE_TVTIME,
                "./assets/tvtime.svg",
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(85,3), Size::new(43,58)),
                Rectangle::new(Point::zero(), Size::new(0,0)),
                0.0,
                0.0,
                true,
                Rectangle::new(Point::new(85,52), Size::new(43,12)),
                true,
            )
            },
        "vcr" => {
            Eggs::new(
                EGGS_TYPE_VCR,
                "./assets/vcr2000.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::new(4,2), Size::new(120,6)), 
                Rectangle::new(Point::new(4,10), Size::new(120,6)), 
                0.0, 
                0.0, 
                false, // should replace clock
                Rectangle::new(Point::new(30,16), Size::new(48,12)),
                false,
            )
            },
        _ => {
            Eggs::new(
                EGGS_TYPE_UNKNOWN,
                "./assets/none.svg", 
                Rectangle::new(Point::zero(), Size::new(128,64)),
                Rectangle::new(Point::zero(), Size::new(0,0)), 
                Rectangle::new(Point::zero(), Size::new(0,0)),
                0.0, 
                0.0, 
                false,
                Rectangle::new(Point::zero(), Size::new(0,0)),
                false,
            )
        }
    }

}

#[allow(dead_code)]
impl Eggs {

    /// Creates a new `Egg` from SVG string data and target dimensions.
    pub fn new(
        egg_type: u8,
        path: &str, 
        rect: Rectangle, 
        artist_rect: Rectangle, 
        title_rect: Rectangle, 
        low_limit: f64, 
        high_limit: f64,
        combine: bool,
        time_rect: Rectangle,
        can_widen: bool,
    ) -> Self {

        let re = r"\{\{.*?\}\}".to_string();
        let width = rect.size.width as usize;
        let height = rect.size.height as usize;
        let svg_data = fs::read_to_string(path).expect("load SVG file");
        let buffer_size = height as usize * ((width + 7) / 8) as usize;
        let groovy = AnimState::new();

        Self {
            egg_type,
            rect,
            svg_data,
            modified_svg_data: String::new(),
            buffer: vec![0u8; buffer_size],
            artist_rect,
            artist: String::new(),
            title_rect,
            title: String::new(),
            combine,
            low_limit,
            high_limit,
            time_rect,
            track_pcnt: 0.00,
            track_time_secs: 0.00,
            can_widen,
            groovy,
            re,
        }
    }

    pub fn update (&mut self, 
        artist: &str, 
        title: &str, 
        level: u8, 
        track_percent: f64,
        track_time: f32,
    ) -> Result<(), EggsError> {
        
        use regex::Regex;

        if self.egg_type == EGGS_TYPE_UNKNOWN {
            return Ok(());
        }
        let mut data = self.svg_data.clone();
        self.artist = artist.to_string();
        self.title = title.to_string(); 
        if self.combine {
            self.artist = format!("{}\n{}", self.artist, self.title);
        }
        if track_time != self.track_time_secs {
            self.track_time_secs = track_time;
        }

        // level - supports switch and on-off (cumulative) modes
        if data.contains("{{level") {
            if level == 3 {
                data = data
                    .replace("{{level-switch-03}}", "1.0")
                    .replace("{{level-onoff-02}}", "1.0")
                    .replace("{{level-onoff-03}}", "1.0");
            } else {
                data = data
                    .replace("{{level-switch-03}}", "0.0")
                    .replace("{{level-onoff-02}}", "0.0")
                    .replace("{{level-onoff-03}}", "0.0");
            }
            if level == 2 {
                data = data
                    .replace("{{level-switch-02}}", "1.0")
                    .replace("{{level-onoff-02}}", "1.0");
            } else {
                data = data
                    .replace("{{level-switch-02}}", "0.0")
                    .replace("{{level-onoff-02}}", "0.0");
            }
            if level == 1 {
                data = data.replace("{{level1-switch-0}}", "1.0");
            } else {
                data = data.replace("{{level-switch-01}}", "0.0");
            }
        }

        // artist
        data = data.replace("{{artist}}", self.artist.clone().as_str());
        // title
        if !self.combine {
            data = data.replace("{{title}}", self.title.clone().as_str());
        }
            
        // time based rotation animation
        let now = Local::now();
        let seconds = now.second() as f64;
        let seconds_angle =  (
            now.second() as f64 + now.timestamp_subsec_nanos() as f64 / 1_000_000_000.0) * 12.0;
        if data.contains("-angle}}") {
            data = data
                .replace("{{seconds-angle}}", seconds_angle.to_string().as_str())
                .replace("{{anticlockwise-seconds-angle}}", format!("-{seconds_angle}").as_str());
        }
        if seconds%2.0 == 0.0 {
            data = data
                .replace("{{flip}}", "1")
                .replace("{{flip-odd}}", "-1")
            	.replace("{{flip-even}}", "1")
            	.replace("{{blink-even}}", "1.0")
            	.replace("{{blink-odd}}", "0.0")
            	.replace("{{ripple-even}}", "1.0")
            	.replace("{{ripple-odd}}", "0.0");
        } else {
            data = data
                .replace("{{flip}}", "-1")
            	.replace("{{flip-odd}}", "1")
            	.replace("{{flip-even}}", "-1")
            	.replace("{{blink-even}}", "0.0")
            	.replace("{{blink-odd}}", "1.0")
            	.replace("{{ripple-even}}", "0.0")
            	.replace("{{ripple-odd}}", "1.0");
        }

        data = data.replace("{{track-percent}}", track_percent.to_string().as_str());
        let track_percent_whole = (track_percent * 100.0).floor() as u8;
        data = data.replace("{{track-percent-whole}}", track_percent_whole.to_string().as_str());

        // scaler (drink me) logic - grow and shrink SVG objects
        let grow = 1.0 + track_percent;
        data = data.replace("{{scale-grow-progress}}", grow.to_string().as_str());
        let shrink = 1.0 - track_percent;
        data = data.replace("{{scale-shrink-progress}}", shrink.to_string().as_str());
        
        // note that right->left (-ve) and left->right (+ve) is defined via -ve , -{{progress}}, in the SVG
        let linear_pct = self.calc_progress_angle_linear(self.low_limit as f32, self.high_limit as f32, track_percent as f32);
        data = data.replace("{{track-progress}}", linear_pct.to_string().as_str());

        // progress-arc - note using _pct flavor as we're called with precalculated percentile
        let arc_angle = self.calc_progress_angle_pct(self.low_limit as f32, self.high_limit as f32, track_percent as f32);
        data = data.replace("{{progress-arc}}", arc_angle.to_string().as_str());

        if data.contains("{{worm-") {
            /*
            <g transform="translate({{worm-x}}, 0) scale({{worm-stretch}}, 1)">
            <!-- character paths -->
            </g>
            */
            self.groovy.advance();
            let inchworm_offset = self.groovy.bounce_offset();
             // simple 2-frame stretch animation for fun
            let inchworm_stretch = if self.groovy.tick < self.groovy.frames_per_cycle / 2 {
                1.0 + 0.3 * (self.groovy.tick as f64 / (self.groovy.frames_per_cycle as f64 / 2.0))
            } else {
                1.3 - 0.3 * ((self.groovy.tick - self.groovy.frames_per_cycle / 2) as f64 / (self.groovy.frames_per_cycle as f64 / 2.0))
            };
            data = data
                .replace("{{worm-stretch}}", inchworm_stretch.to_string().as_str())
                .replace("{{worm-x}}", inchworm_offset.to_string().as_str());
        }

        // patch any missed replacement tags
        let re = Regex::new(self.re.as_str()).unwrap();
        let replace = "0";
        data = re.replace_all(data.clone().as_str(), replace).to_string();

        self.modified_svg_data = data;
        Ok(())

    }

    /// Render the egg SVG and draw it directly to `display`.
    ///
    /// Replaces the paired `update_and_render_blocking` / `update_and_render_blocking_gray4`
    /// methods. Color depth is inferred from the display's pixel type via `SvgColorDepth`.
    pub fn render_and_draw<D>(
        &mut self,
        display: &mut D,
        artist: &str,
        title: &str,
        level: u8,
        track_percent: f64,
        track_time: f32,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget,
        D::Color: SvgColorDepth,
    {
        let width = self.rect.size.width;
        let height = self.rect.size.height;
        if self.egg_type != EGGS_TYPE_UNKNOWN {
            if self.update(artist, title, level, track_percent, track_time).is_ok() {
                let data = self.modified_svg_data.clone();
                if let Ok(svg_renderer) = SvgImageRenderer::new(&data, width, height) {
                    let buffer_size = D::Color::required_buffer_size(width, height);
                    self.buffer.resize(buffer_size, 0);
                    if D::Color::render_to_buffer(&svg_renderer, &mut self.buffer).is_ok() {
                        D::Color::draw_buffer_to_display(&self.buffer, width, Point::zero(), display)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_svg_data(&self) -> &str {
        &self.modified_svg_data
    }

    pub fn get_artist(&self) -> &str {
        &self.artist
    }

    pub fn get_track_time(&self) -> f32 {
        self.track_time_secs
    }

    pub fn get_title(&self) -> &str {
        &self.title
    }

    pub fn get_width(&self) -> u32 {
        self.rect.size.width
    }

    pub fn get_top_left(&self) -> Point {
        self.rect.top_left
    }

    pub fn is_combined(&self) -> bool {
        self.combine
    }

    pub fn get_title_rect(&self) -> Rectangle {
        self.title_rect
    }

    pub fn get_artist_rect(&self) -> Rectangle {
        self.artist_rect
    }

    pub fn get_time_rect(&self) -> Rectangle {
        self.time_rect
    }

    pub fn can_widen(&self) -> bool {
        self.can_widen
    }

    fn calc_progress_angle(&mut self, angle0:f32, angle100:f32, progress_percent: f32) -> f32 {
        let clamped_percent = progress_percent.clamp(0.0, 100.0);
        let angle_range = angle100 - angle0;
        let factor = clamped_percent / 100.0;
        angle0 + (angle_range * factor)
    }

    fn calc_progress_angle_linear(&mut self, angle0:f32, angle100:f32, progress_percent: f32) -> f32 {
        let angle_range = angle100 - angle0;
        let factor = angle_range / 100.0;
        angle0 + (progress_percent * factor)
    }

    fn calc_progress_angle_pct(&mut self, angle0:f32, angle100:f32, progress_percent: f32) -> f32 {
        let factor = progress_percent.clamp(0.0, 1.0);
        let angle_range = angle100 - angle0;
        angle0 + (angle_range * factor)
    }

}
