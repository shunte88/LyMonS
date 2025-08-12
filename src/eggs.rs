
//! Module for Easter Egg Support.
//!
use chrono::{Timelike, Local};
use embedded_graphics::{
    image::{ImageRaw},
    pixelcolor::BinaryColor,
    prelude::*, 
    primitives::{Rectangle}, 
};

use log::{info};
use std::error::Error;
use std::fmt;
use std::fs;

use crate::svgimage::SvgImageRenderer;

pub const EGGS_TYPE_CASSETTE: u8 = 10;
pub const EGGS_TYPE_MOOG: u8 = 20;
pub const EGGS_TYPE_TECHNICS: u8 = 30;
pub const EGGS_TYPE_REEL2REEL: u8 = 40;
pub const EGGS_TYPE_VCR: u8 = 50;
pub const EGGS_TYPE_TUBEAMP: u8 = 60;
pub const EGGS_TYPE_RADIO40: u8 = 70;
pub const EGGS_TYPE_RADIO50: u8 = 80;
pub const EGGS_TYPE_TVTIME: u8 = 90;
pub const EGGS_TYPE_IBMPC: u8 = 100;
pub const EGGS_TYPE_BASS: u8 = 110;
pub const EGGS_TYPE_UNKNOWN: u8 = 255;

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
}

/// Sets the clock display font
pub fn set_easter_egg(egg_name: &str) -> Eggs {
    info!("Load egg: {}",egg_name);
    match egg_name {
        "cassette" => {
            Eggs::new(
                EGGS_TYPE_CASSETTE,
                "./assets/compactcassettef.svg",
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(18,6), Size::new(90,6)), 
                Rectangle::new(Point::new(18,11), Size::new(90,6)), 
                // 13.5=0%, 0=100%
                // reversed these
                0.0, 
                13.5, 
                false,
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
            )
        },
        "moog" => {
            Eggs::new(
                EGGS_TYPE_MOOG,
                "./assets/moogf.svg",
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(83,3), Size::new(41,58)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)), 
                -10.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(83,52), Size::new(41,12)),
            )
        },
        "technics" => {
            Eggs::new(
                EGGS_TYPE_TECHNICS,
                "./assets/sl1200.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(85,3), Size::new(39,58)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
                -10.0, 
                12.0, 
                true,
                Rectangle::new(Point::new(85,52), Size::new(39,12)),
            )
            },
        "reel2reel" => {
            Eggs::new(
                EGGS_TYPE_REEL2REEL,
                "./assets/reel2reels.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(72,3), Size::new(52,58)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
                0.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(72,52), Size::new(52,12)),
            )
            },
        "vcr" => {
            Eggs::new(
                EGGS_TYPE_VCR,
                "./assets/vcr2000z.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(4,2), Size::new(120,6)), 
                Rectangle::new(Point::new(4,10), Size::new(120,6)), 
                0.0, 
                0.0, 
                false, // should replace clock
                Rectangle::new(Point::new(30,16), Size::new(48,12)),
            )
            },
        "tubeamp" => {        
            Eggs::new(
                EGGS_TYPE_TUBEAMP,
                "./assets/tubeampd.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(87,3), Size::new(37,58)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
                0.0, 
                100.0, 
                true,
                Rectangle::new(Point::new(87,52), Size::new(37,12)),
            )
            },
        "radio40" => {
            Eggs::new(
                EGGS_TYPE_RADIO40,
                "./assets/radio40s.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(64,3), Size::new(60,58)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
                -5.0, 
                5.0, 
                true,
                Rectangle::new(Point::new(64,52), Size::new(60,12)),
            )
            },
        "radio50" => {
            Eggs::new(
                EGGS_TYPE_RADIO50,
                "./assets/radio502.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(74,3), Size::new(46,58)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
                0.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(74,52), Size::new(46,12)),
            )
            },
        "tvtime" => {
            Eggs::new(
                EGGS_TYPE_TVTIME,
                "./assets/tvtime2.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(89,3), Size::new(35,58)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
                0.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(89,52), Size::new(35,12)),
            )
            },
        "ibmpc" => {
            Eggs::new(
                EGGS_TYPE_IBMPC,
                "./assets/ibmpc.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(70,3), Size::new(64,58)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
                0.0, 
                0.0, 
                true,
                Rectangle::new(Point::new(70,52), Size::new(54,12)),
            )
        },
        "bass" => {
            Eggs::new(
                EGGS_TYPE_BASS,
                "./assets/bass.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(4,4), Size::new(120,6)), 
                Rectangle::new(Point::new(4,12), Size::new(120,6)), 
                48.0, 
                88.0, 
                false,
                Rectangle::new(Point::new(64,26), Size::new(48,12)),
            )
        },
        _ => {
            Eggs::new(
                EGGS_TYPE_UNKNOWN,
                "./assets/none.svg", 
                Rectangle::new(Point::new(0,0), Size::new(128,64)),
                Rectangle::new(Point::new(0,0), Size::new(0,0)), 
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
                0.0, 
                0.0, 
                false,
                Rectangle::new(Point::new(0,0), Size::new(0,0)),
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
        time_rect: Rectangle
    ) -> Self {

        let width = rect.size.width as usize;
        let height = rect.size.height as usize;
        let svg_data = fs::read_to_string(path).expect("load SVG file");
        let buffer_size = height as usize * ((width + 7) / 8) as usize;
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
        }
    }

    pub fn update (&mut self, 
        artist: &str, 
        title: &str, 
        level: u8, 
        track_percent: f64,
        track_time: f32,
    ) -> Result<(), EggsError> {
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
        if level == 3 {
            data = data.replace("{{level-switch-03}}", "1.0");
            data = data.replace("{{level-onoff-02}}", "1.0");
            data = data.replace("{{level-onoff-03}}", "1.0");
        } else {
            data = data.replace("{{level-switch-03}}", "0.0");
            data = data.replace("{{level-onoff-02}}", "0.0");
            data = data.replace("{{level-onoff-03}}", "0.0");
        }
        if level == 2 {
            data = data.replace("{{level-switch-02}}", "1.0");
            data = data.replace("{{level-onoff-02}}", "1.0");
        } else {
            data = data.replace("{{level-switch-02}}", "0.0");
            data = data.replace("{{level-onoff-02}}", "0.0");
        }
        if level == 1 {
            data = data.replace("{{level1-switch-0}}", "1.0");
        } else {
            data = data.replace("{{level-switch-01}}", "0.0");
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
        data = data.replace("{{seconds-angle}}", seconds_angle.to_string().as_str());
        if seconds%2.0 == 0.0 {
            data = data.replace("{{blink-even}}", "1.0");
            data = data.replace("{{ripple-even}}", "-1.0");
            data = data.replace("{{ripple-odd}}", "0.0");
        } else {
            data = data.replace("{{blink-even}}", "0.0");
            data = data.replace("{{ripple-even}}", "0.0");
            data = data.replace("{{ripple-odd}}", "1.0");
        }

        // note that right->left (-ve) and left->right (+ve) is defined via -ve , -{{progress}}, in the SVG
        let linear_pct = self.calc_progress_angle_linear(self.low_limit as f32, self.high_limit as f32, track_percent as f32);
        data = data.replace("{{track-progress}}", linear_pct.to_string().as_str());

        // progress-arc - note using _pct flavor as we're called with precalculated percentile
        let arc_angle = self.calc_progress_angle_pct(self.low_limit as f32, self.high_limit as f32, track_percent as f32);
        data = data.replace("{{progress-arc}}", arc_angle.to_string().as_str());
        self.modified_svg_data = data;
        Ok(())

    }

    pub async fn update_and_render (
        &mut self, 
        artist: &str, 
        title: &str, 
        level: u8, 
        track_percent: f64,
        track_time: f32,
    ) -> Result<ImageRaw<BinaryColor>, EggsError> {

        let width = self.rect.size.width as u32;
        let height = self.rect.size.height as u32;
        if self.egg_type != EGGS_TYPE_UNKNOWN{
            self.update(artist, title, level, track_percent, track_time)?;
            let data = self.modified_svg_data.clone();
            let svg_renderer = SvgImageRenderer::new(&data, width, height)
                .map_err(|e| EggsError::EggRenderError(e.to_string()))?;
            svg_renderer.render_to_buffer(&mut self.buffer)
                .map_err(|e| EggsError::EggBufferError(e.to_string()))?;
        }
        let raw_image = ImageRaw::<BinaryColor>::new(&self.buffer, width);
        Ok(raw_image)
      
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

