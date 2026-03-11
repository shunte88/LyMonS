/*
 *  visualization.rs
 *
 *  LyMonS - worth the squeeze
 *	(c) 2020-26 Stuart Hunter
 *
 *  Visualization selection and asset path management
 *  Updated to use the adaptive layout system
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

#![allow(dead_code)] // SVG visualization infrastructure; traits and helpers for future viz types

use crate::display::layout::LayoutConfig;
use embedded_graphics::{
    image::{Image, ImageRaw},
    pixelcolor::{BinaryColor, Gray4, Rgb565},
    prelude::*,
    primitives::Rectangle,
};
use std::error::Error;
use std::fmt;
use std::fs;

use crate::svgimage::SvgImageRenderer;

/// Trait that abstracts SVG-to-buffer rendering for different color depths.
///
/// Implementing this for `BinaryColor` and `Gray4` allows `Visual::render_svg_and_draw<D>()`
/// to work generically, eliminating paired `_mono`/`_gray4` draw functions.
pub trait SvgColorDepth: PixelColor + From<<Self as PixelColor>::Raw> {
    /// Buffer size in bytes required for a `width × height` frame.
    fn required_buffer_size(width: u32, height: u32) -> usize;

    /// Rasterize `renderer` into `buffer` using this color depth's format.
    fn render_to_buffer(renderer: &SvgImageRenderer, buffer: &mut Vec<u8>) -> Result<(), VizError>;

    /// Draw a rendered buffer to `display` at `position`.
    ///
    /// Implemented concretely per color depth so the compiler sees the exact
    /// `ImageRaw<BinaryColor>` / `ImageRaw<Gray4>` type and can satisfy the
    /// `ImageDrawable` trait bound without additional HRTB constraints on callers.
    fn draw_buffer_to_display<D>(buffer: &[u8], width: u32, position: Point, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Self>;

    /// Asset sub-folder for weather SVG icons (path differs by color depth).
    fn weather_asset_folder() -> &'static str;

    /// Maximum brightness / "on" color for this depth.
    fn on() -> Self;

    /// Whether this color depth should render weather glyphs from SVG rather
    /// than the compiled-in 1bpp bitmaps.
    ///
    /// Returns `false` for `BinaryColor` (mono OLEDs: pixel-precise hand-crafted
    /// bitmaps look better at 12×12) and `true` for `Gray4` / `Rgb565` (smooth
    /// anti-aliased SVG rendering at any size).
    fn use_svg_glyphs() -> bool { true }
}

impl SvgColorDepth for BinaryColor {
    fn required_buffer_size(width: u32, height: u32) -> usize {
        // Ceiling division: each row needs (width+7)/8 bytes
        height as usize * ((width + 7) / 8) as usize
    }
    fn render_to_buffer(renderer: &SvgImageRenderer, buffer: &mut Vec<u8>) -> Result<(), VizError> {
        renderer.render_to_buffer(buffer)
            .map_err(|e| VizError::VizBufferError(e.to_string()))
    }
    fn draw_buffer_to_display<D>(buffer: &[u8], width: u32, position: Point, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let raw = ImageRaw::<BinaryColor>::new(buffer, width);
        Image::new(&raw, position).draw(display)
    }
    fn weather_asset_folder() -> &'static str { "./assets/mono" }
    fn on() -> Self { BinaryColor::On }
    fn use_svg_glyphs() -> bool { false }
}

impl SvgColorDepth for Gray4 {
    fn required_buffer_size(width: u32, height: u32) -> usize {
        ((width * height + 1) / 2) as usize
    }
    fn render_to_buffer(renderer: &SvgImageRenderer, buffer: &mut Vec<u8>) -> Result<(), VizError> {
        renderer.render_to_buffer_gray4(buffer)
            .map_err(|e| VizError::VizBufferError(e.to_string()))
    }
    fn draw_buffer_to_display<D>(buffer: &[u8], width: u32, position: Point, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        let raw = ImageRaw::<Gray4>::new(buffer, width);
        Image::new(&raw, position).draw(display)
    }
    fn weather_asset_folder() -> &'static str { "./assets/color" }
    fn on() -> Self { Gray4::WHITE }
}

impl SvgColorDepth for Rgb565 {
    fn required_buffer_size(width: u32, height: u32) -> usize {
        (width * height * 2) as usize
    }
    fn render_to_buffer(renderer: &SvgImageRenderer, buffer: &mut Vec<u8>) -> Result<(), VizError> {
        renderer.render_to_buffer_rgb565(buffer)
            .map_err(|e| VizError::VizBufferError(e.to_string()))
    }
    fn draw_buffer_to_display<D>(buffer: &[u8], width: u32, position: Point, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let raw = ImageRaw::<Rgb565>::new(buffer, width);
        Image::new(&raw, position).draw(display)
    }
    fn weather_asset_folder() -> &'static str { "./assets/color" }
    fn on() -> Self { Rgb565::WHITE }
}

/// Which visualization to produce.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visualization {
    VuStereo,                 // two VU meters (L/R)
    VuMono,                   // downmix to mono VU
    PeakStereo,               // two peak meters with hold/decay
    PeakMono,                 // mono peak meter with hold/decay
    HistStereo,               // two histogram bars (L/R)
    HistMono,                 // mono histogram (downmix)
    VuStereoWithCenterPeak,   // L/R VU with a central mono peak meter
    VuAio,                    // All In One with downmix VU
    HistAio,                  // All In One with downmix histogram
    WaveformSpectrum,         // Waveform + Spectrogram (oscilloscope + waterfall)
    NoVisualization,          // no visualization
}

/// Custom error type for SVG visualization rendering operations.
#[derive(Debug)]
pub enum VizError {
    /// Error parsing egg configuration.
    _VizParseError(String),
    VizRenderError(String),
    VizBufferError(String),
}

impl fmt::Display for VizError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VizError::_VizParseError(msg) => write!(f, "Visualization parse error: {}", msg),
            VizError::VizRenderError(msg) => write!(f, "Visualization SVG render error: {}", msg),
            VizError::VizBufferError(msg) => write!(f, "Visualization buffer render error: {}", msg),
        }
    }
}

impl Error for VizError {}

#[derive(Clone, Debug, PartialEq)]
pub struct Visual {
    pub kind: Visualization,
    pub svg_supported: bool,
    rect: Rectangle,
    svg_name: String,
    svg_data: String,
    modified_svg_data: String,
    buffer: Vec<u8>,
    pub scale_min: f64,
    pub scale_max: f64,
    pub sweep_min: f64,
    pub sweep_max: f64,
    pub over_support: bool,
    pub can_widen: bool,
    pub peak_m: Vec<bool>,
    pub hold_m: Vec<bool>,
    pub peak_l: Vec<bool>,
    pub hold_l: Vec<bool>,
    pub peak_r: Vec<bool>,
    pub hold_r: Vec<bool>,
    re: String,
}

#[allow(dead_code)]
impl Visual {

    /// Creates a new `Visual` from SVG string data and target dimensions.
    pub fn new(
        kind: Visualization,
        path: String, 
        rect: Rectangle, 
        scale_min: f64, 
        scale_max: f64,
        sweep_min: f64,
        sweep_max: f64,
        over_support: bool,
        can_widen: bool,
        bools_idx: usize,
    ) -> Self {

        let re = r"\{\{.*?\}\}".to_string();
        let width = rect.size.width as usize;
        let height = rect.size.height as usize;
        let svg_supported = visualizer_svg_supported(kind);
        let svg_name = path.clone();
        let svg_data = if svg_supported {fs::read_to_string(path.as_str()).expect("load SVG file")} else {String::from("")};
        let buffer_size = height as usize * ((width + 7) / 8) as usize;

        let init_vec = vec![false; bools_idx];

        Self {
            kind,
            svg_supported,
            rect,
            svg_name,
            svg_data,
            modified_svg_data: String::new(),
            buffer: vec![0u8; buffer_size],
            scale_min,
            scale_max,
            sweep_min,
            sweep_max,
            over_support,
            can_widen,
            peak_m: init_vec.clone(),
            hold_m: init_vec.clone(),
            peak_l: init_vec.clone(),
            hold_l: init_vec.clone(),
            peak_r: init_vec.clone(),
            hold_r: init_vec.clone(),
            re,
        }
    }

    pub fn apply_template(
        &mut self,
        template: &str, 
        peak: &[bool], 
        hold: &[bool],
        seed: &str,
    ) -> String {
        use std::collections::HashMap;
        // Build lookup
        let values: HashMap<String, &str> = peak.iter()
            .zip(hold.iter())
            .enumerate()
            .map(|(i, (&peak, &hold))| {
                let tag = format!("{}_{:02}", seed, i);
                // using 0.4 as equates to off on mono OLED
                let value = if peak || hold { "1" } else { "0.4" };
                (tag, value)
            })
            .collect();

        let mut result = String::with_capacity(template.len());
        let mut chars = template.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' && chars.peek() == Some(&'{') {
                chars.next(); // consume second '{'
                let key: String = chars.by_ref()
                    .take_while(|&c| c != '}')
                    .collect();
                chars.next(); // consume second '}'

                if let Some(value) = values.get(&key) {
                    result.push_str(value);
                } else {
                    // Unknown tag, preserve it
                    result.push_str(&format!("{{{{{}}}}}", key));
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    pub fn update (
        &mut self,
        metric_left: f64,
        over_left: bool,
        metric_right: f64,
        over_right: bool,
        peak_m: Vec<bool>,
        hold_m: Vec<bool>,
        peak_l: Vec<bool>,
        hold_l: Vec<bool>,
        peak_r: Vec<bool>,
        hold_r: Vec<bool>,
    ) -> Result<(), VizError> {

        if !self.svg_supported {
            return Ok(());
        }

        use regex::Regex;

        let mut data = self.svg_data.clone();

        // overage beacon
        let over = if over_left { "1" } else { "0" };
        data = data
            .replace("{{overflow}}", over) // downmix
            .replace("{{overflow-left}}", over)
            .replace("{{overflow_left}}", over);
        let over = if over_right { "1" } else { "0" };
        data = data
            .replace("{{overflow-right}}", over)
            .replace("{{overflow_right}}", over);

        data = data
            .replace("{{needle}}", metric_left.to_string().as_str())
            .replace("{{needle_left}}", metric_left.to_string().as_str())
            .replace("{{needle-left}}", metric_left.to_string().as_str())
            .replace("{{needle_right}}", metric_right.to_string().as_str())
            .replace("{{needle-right}}", metric_right.to_string().as_str());

        data = self.apply_template(&data, &peak_m, &hold_m, "peak");
        data = self.apply_template(&data, &peak_l, &hold_l, "peak_left");
        data = self.apply_template(&data, &peak_r, &hold_r, "peak_right");

        // patch any missed replacement tags
        let re = Regex::new(self.re.as_str()).unwrap();
        let replace = "0";
        data = re.replace_all(data.clone().as_str(), replace).to_string();

        self.modified_svg_data = data.clone();
        Ok(())

    }

    /// Render SVG and draw directly to `display`.
    ///
    /// Avoids returning `ImageRaw<C>` to the caller (which would require
    /// `ImageDrawable` HRTB bounds), instead delegating the draw step to the
    /// concrete `SvgColorDepth::draw_buffer_to_display` impl.
    pub fn render_svg_and_draw<D>(
        &mut self,
        display: &mut D,
        metric_left: f64,
        over_left: bool,
        metric_right: f64,
        over_right: bool,
        peak_m: Vec<bool>,
        hold_m: Vec<bool>,
        peak_l: Vec<bool>,
        hold_l: Vec<bool>,
        peak_r: Vec<bool>,
        hold_r: Vec<bool>,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget,
        D::Color: SvgColorDepth,
    {
        let width = self.rect.size.width;
        let height = self.rect.size.height;

        if self.svg_supported {
            self.update(
                metric_left, over_left, metric_right, over_right,
                peak_m, hold_m, peak_l, hold_l, peak_r, hold_r,
            ).map_err(|e| {
                // VizError can't be converted to D::Error generically;
                // log and treat as a no-op draw rather than crashing.
                eprintln!("SVG update error: {}", e);
            }).ok();
            let data = self.modified_svg_data.clone();
            if let Ok(svg_renderer) = crate::svgimage::SvgImageRenderer::new(&data, width, height) {
                let buffer_size = D::Color::required_buffer_size(width, height);
                self.buffer.resize(buffer_size, 0);
                if D::Color::render_to_buffer(&svg_renderer, &mut self.buffer).is_ok() {
                    D::Color::draw_buffer_to_display(&self.buffer, width, self.rect.top_left, display)?;
                }
            }
        }
        Ok(())
    }

    pub fn get_svg_filename(&self) -> &str {
        &self.svg_name.as_str()
    }

    pub fn get_svg_data(&self) -> &str {
        &self.modified_svg_data
    }

}

pub fn transpose_kind(kind: &str) -> Visualization {
    match kind {
        "vu_stereo" => Visualization::VuStereo,
        "vu_mono" => Visualization::VuMono,
        "peak_stereo" => Visualization::PeakStereo,
        "peak_mono" => Visualization::PeakMono,
        "hist_stereo" => Visualization::HistStereo,
        "hist_mono" => Visualization::HistMono,
        "vu_stereo_with_center_peak" | "combination" | "vu_combi" 
            => Visualization::VuStereoWithCenterPeak,
        "vu_aio" => Visualization::VuAio,
        "hist_aio" => Visualization::HistAio,
        "waveform_spectrum" => Visualization::WaveformSpectrum,
        "no_viz" => Visualization::NoVisualization,
        &_ => Visualization::NoVisualization,
    }
}

/// Get visualizer panel path using layout configuration (preferred)
///
/// This function uses the layout system to select the appropriate
/// asset path based on display resolution and capabilities.
pub fn get_visualizer_panel_with_layout(kind: Visualization, layout: &LayoutConfig) -> String {
    let folder = &layout.asset_path;
    //let vu_aio_svg = if layout.width > 132 { "vuaio.svg" } else { "vudownmix.svg" };
    let vu_aio_svg = "vuaio.svg";
    let panel = match kind {
        Visualization::VuStereo => format!("{}vu2up.svg", folder),
        Visualization::VuMono  => format!("{}vudownmix.svg", folder),
        Visualization::VuStereoWithCenterPeak => format!("{}vucombi.svg", folder),
        Visualization::VuAio => format!("{}{}", folder, vu_aio_svg),
        Visualization::PeakStereo => format!("{}peak.svg", folder),
        Visualization::PeakMono  => format!("{}peakmono.svg", folder),
        Visualization::HistAio => format!("{}histaio.svg", folder),
        Visualization::HistStereo |
        Visualization::HistMono |
        Visualization::WaveformSpectrum |
        Visualization::NoVisualization => "".to_string(),
    };
    panel
}

pub fn visualizer_svg_supported(kind: Visualization) -> bool {
    let supported = match kind {
        Visualization::VuStereo |
        Visualization::VuMono |
        Visualization::VuStereoWithCenterPeak |
        Visualization::VuAio |
        Visualization::PeakStereo |
        Visualization::PeakMono  |
        Visualization::HistAio => true,
        Visualization::HistStereo |
        Visualization::HistMono |
        Visualization::WaveformSpectrum |
        Visualization::NoVisualization => false,
    };
    supported
}

/// Get visualizer panel path (legacy function for backwards compatibility)
///
/// This function maintains backwards compatibility with existing code.
/// New code should use `get_visualizer_panel_with_layout` instead.
/// THIS NEEDS TO COME FROM Visual configurator
pub fn get_visualizer_panel(kind: Visualization, wide: bool) -> String {
    let folder = if wide {"./assets/ssd1322/"}else{"./assets/ssd1309/"};
    let panel = match kind {
        Visualization::VuStereo => format!("{folder}vu2up.svg"),
        Visualization::VuMono  => format!("{folder}vudownmix.svg"),
        Visualization::VuStereoWithCenterPeak => format!("{folder}vucombi.svg"),
        Visualization::VuAio => format!("{folder}vuaio.svg"),
        Visualization::PeakStereo => format!("{folder}peak.svg"),
        Visualization::PeakMono  => format!("{folder}peakmono.svg"),
        Visualization::HistAio => format!("{folder}histaio.svg"),
        Visualization::HistStereo |
        Visualization::HistMono |
        Visualization::WaveformSpectrum |
        Visualization::NoVisualization => "".to_string(),
    };
    panel.clone()
}

/// Loads/sets the visualization
pub fn get_visual(kind: Visualization, wide: bool) -> Visual {
    let folder = if wide {"./assets/ssd1322/"}else{"./assets/ssd1309/"};
    let size = if wide { Size::new(256, 64) } else { Size::new(128, 64) };
    let viz = match kind {
        Visualization::VuStereo => {
            let sweep: f64 = if size.width > 128 {44.01} else {41.12};
            Visual::new(
                kind,
                String::from(format!("{folder}vu2up.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                -25.0,
                5.0,
                -sweep,
                sweep,
                true,
                false,
                0,
            )
        },
        Visualization::VuMono  => {
            Visual::new(
                kind,
                String::from(format!("{folder}vudownmix.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                -25.0,
                5.0,
                -44.01,
                44.01,
                true,
                false,
                0,
            )
        },
        Visualization::VuStereoWithCenterPeak => {
            let sweep: f64 = if size.width > 128 {44.34} else {45.00};
            Visual::new(
                kind,
                String::from(format!("{folder}vucombi.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                -23.0,
                3.0,
                -sweep,
                sweep,
                false,
                false,
                19,
            )},
        Visualization::VuAio  => {
            // Wide: stereo VU using ssd1309/vu2up.svg drawn at x=128 (right half of 256px display)
            // Narrow: mono VU using vuaio.svg covering full 128px (VU face in right portion)
            let (svg_file, rect) = if wide {
                (String::from("./assets/ssd1309/vu2up.svg"),
                 Rectangle::new(Point::new(128, 0), Size::new(128, 64)))
            } else {
                (String::from(format!("{folder}vuaio.svg")),
                 Rectangle::new(Point::zero(), Size::new(128, 64)))
            };
            Visual::new(
                kind,
                svg_file,
                rect,
                -25.0,
                5.0,
                -44.01,
                44.01,
                true,
                false,
                0,
            )
        },
        Visualization::PeakStereo  => {
            Visual::new(
                kind,
                String::from(format!("{folder}peakstereo.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
                16,
            )
        },
        Visualization::PeakMono   => {
            Visual::new(
                kind,
                String::from(format!("{folder}peakmono.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
                16,
            )
        },
        Visualization::HistAio  => {
            Visual::new(
                kind,
                String::from(format!("{folder}none.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
                0,
            )
        },
        Visualization::HistStereo   => {
            Visual::new(
                kind,
                String::from(format!("{folder}none.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
                0,
            )
        },
        Visualization::HistMono  => {
            Visual::new(
                kind,
                String::from(format!("{folder}none.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
                0,
            )
        },
        Visualization::WaveformSpectrum  => {
            Visual::new(
                kind,
                String::from(format!("{folder}none.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
                0,
            )
        },
        Visualization::NoVisualization  => {
            Visual::new(
                kind,
                String::from(format!("{folder}none.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                0.0,
                0.0,
                0.0,
                0.0,
                false,
                false,
                0,
            )
        },
    };
    viz
}

