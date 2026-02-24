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

use crate::display::layout::LayoutConfig;
use embedded_graphics::{
    image::{ImageRaw},
    pixelcolor::{BinaryColor, Gray4},
    prelude::*,
    primitives::{Rectangle},
};
use std::error::Error;
use std::fmt;
use std::fs;

use crate::svgimage::SvgImageRenderer;

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
    AioVuMono,                // All In One with downmix VU
    AioHistMono,              // All In One with downmix histogram
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
            re,
        }
    }

    pub fn update (
        &mut self,
        metric_left: f64,
        over_left: bool,
        metric_right: f64,
        over_right: bool,
        peak_m: Vec<bool>,
        hold_m: Vec<bool>,
    ) -> Result<(), VizError> {

        if !self.svg_supported {
            return Ok(());
        }

        use regex::Regex;

        let mut data = self.svg_data.clone();

        // overage beacon
        if over_left {
            data = data.replace("{{overflow}}", "1"); // downmix
            data = data.replace("{{overflow_left}}", "1");
        } else {
            data = data.replace("{{overflow}}", "0"); // downmix
            data = data.replace("{{overflow_left}}", "0");
        }
        if over_right {
            data = data.replace("{{overflow_right}}", "1");
        } else {
            data = data.replace("{{overflow_right}}", "0");
        }

        data = data.replace("{{needle}}", metric_left.to_string().as_str());
        data = data.replace("{{needle_left}}", metric_left.to_string().as_str());
        data = data.replace("{{needle_right}}", metric_right.to_string().as_str());

        if peak_m.len()>0 && hold_m.len()>0 { 
            for (i, &value) in peak_m.iter().enumerate() {
                let tag = format!("{{{{peak_{:0width$}}}}}", i, width = 2);
                let mut replacement = if value { "1" } else { "0.5" };
                if hold_m[i] {
                    replacement = "1"
                }
                data = data.replace(tag.as_str(), replacement);
            }
        }

        // patch any missed replacement tags
        let re = Regex::new(self.re.as_str()).unwrap();
        let replace = "0";
        data = re.replace_all(data.clone().as_str(), replace).to_string();

        self.modified_svg_data = data.clone();
        Ok(())

    }

    pub async fn update_and_render (
        &mut self,
        metric_left: f64,
        over_left: bool,
        metric_right: f64,
        over_right: bool,
        peak_m: Vec<bool>,
        hold_m: Vec<bool>,
    ) -> Result<ImageRaw<BinaryColor>, VizError> {
        // Delegate to blocking version (no actual async ops here)
        self.update_and_render_blocking(        
            metric_left,
            over_left,
            metric_right,
            over_right,
            peak_m,
            hold_m,
        )
    }

    pub fn update_and_render_blocking (
        &mut self,
        metric_left: f64,
        over_left: bool,
        metric_right: f64,
        over_right: bool,
        peak_m: Vec<bool>,
        hold_m: Vec<bool>,
    ) -> Result<ImageRaw<BinaryColor>, VizError> {

        let width = self.rect.size.width as u32;
        let height = self.rect.size.height as u32;

        if self.svg_supported {
            self.update(
                metric_left,
                over_left,
                metric_right,
                over_right,
                peak_m,
                hold_m,
            )?;
            let data = self.modified_svg_data.clone();
            let svg_renderer = SvgImageRenderer::new(&data, width, height)
                .map_err(|e| VizError::VizRenderError(e.to_string()))?;
            svg_renderer.render_to_buffer(&mut self.buffer)
                .map_err(|e| VizError::VizBufferError(e.to_string()))?;
        }
        let raw_image = ImageRaw::<BinaryColor>::new(&self.buffer, width);
        Ok(raw_image)

    }

    /// Render to Gray4 format with full 16-level grayscale support for colorized SVGs
    pub fn update_and_render_blocking_gray4 (
        &mut self,
        metric_left: f64,
        over_left: bool,
        metric_right: f64,
        over_right: bool,
        peak_m: Vec<bool>,
        hold_m: Vec<bool>,
    ) -> Result<ImageRaw<Gray4>, VizError> {

        let width = self.rect.size.width as u32;
        let height = self.rect.size.height as u32;

        if self.svg_supported {
            self.update(
                metric_left,
                over_left,
                metric_right,
                over_right,
                peak_m,
                hold_m,
            )?;
            let data = self.modified_svg_data.clone();
            
            let svg_renderer = SvgImageRenderer::new(&data, width, height)
                .map_err(|e| VizError::VizRenderError(e.to_string()))?;

            // Resize buffer for Gray4 format (2 pixels per byte)
            let buffer_size = (height as usize * width as usize + 1) / 2;
            self.buffer.resize(buffer_size, 0);

            svg_renderer.render_to_buffer_gray4(&mut self.buffer)
                .map_err(|e| VizError::VizBufferError(e.to_string()))?;
        }
        let raw_image = ImageRaw::<Gray4>::new(&self.buffer, width);
        Ok(raw_image)

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
        "aio_vu_mono" => Visualization::AioVuMono,
        "aio_hist_mono" => Visualization::AioHistMono,
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
    let panel = match kind {
        Visualization::VuStereo => format!("{}vu2up.svg", folder),
        Visualization::VuMono  => format!("{}vudownmix.svg", folder),
        Visualization::VuStereoWithCenterPeak => format!("{}vucombi.svg", folder),
        Visualization::AioVuMono => format!("{}vuaio.svg", folder),
        Visualization::PeakStereo => format!("{}peak.svg", folder),
        Visualization::PeakMono  => format!("{}peakmono.svg", folder),
        Visualization::AioHistMono => format!("{}histaio.svg", folder),
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
        Visualization::AioVuMono => true,
        Visualization::PeakStereo |
        Visualization::PeakMono  |
        Visualization::AioHistMono |
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
        Visualization::AioVuMono => format!("{folder}vuaio.svg"),
        Visualization::PeakStereo => format!("{folder}peak.svg"),
        Visualization::PeakMono  => format!("{folder}peakmono.svg"),
        Visualization::AioHistMono => format!("{folder}histaio.svg"),
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
            Visual::new(
                kind,
                String::from(format!("{folder}vu2up.svg")),
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
        Visualization::AioVuMono  => {
            Visual::new(
                kind,
                String::from(format!("{folder}vuaio.svg")),
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
        Visualization::PeakStereo  => {
            Visual::new(
                kind,
                String::from(format!("{folder}peak.svg")),
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
                0,
            )
        },
        Visualization::AioHistMono  => {
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

