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
    kind: Visualization,
    svg_supported: bool,
    rect: Rectangle,
    svg_data: String,
    modified_svg_data: String,
    buffer: Vec<u8>,
    scale_min: f64,
    scale_max: f64,
    sweep_min: f64,
    sweep_max: f64,
    over_support: bool,
    can_widen: bool,
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
    ) -> Self {

        let width = rect.size.width as usize;
        let height = rect.size.height as usize;
        let svg_supported = visualizer_svg_supported(kind);
        let svg_data = if svg_supported {fs::read_to_string(path.as_str()).expect("load SVG file")} else {String::from("")};
        let buffer_size = height as usize * ((width + 7) / 8) as usize;

        Self {
            kind,
            svg_supported,
            rect,
            svg_data,
            modified_svg_data: String::new(),
            buffer: vec![0u8; buffer_size],
            scale_min,
            scale_max,
            sweep_min,
            sweep_max,
            over_support,
            can_widen,
        }
    }

    pub fn update (
        &mut self, 
        metric: f64,
        over: bool,
    ) -> Result<(), VizError> {

        if !self.svg_supported {
            return Ok(());
        }

        let mut data = self.svg_data.clone();

        // overage beacon
        if over {
            data = data.replace("{{overflow}}", "1");
        }

        // metric - clamp within bounds
        let metric = metric.clamp(self.scale_min, self.scale_max);
        let normalized = (metric - self.scale_min) / (self.scale_max - self.scale_min);
        let arc_angle = self.sweep_min + normalized * (self.sweep_max - self.sweep_min);

        data = data.replace("{{needle-arc}}", arc_angle.to_string().as_str());
        self.modified_svg_data = data;
        Ok(())

    }

    pub async fn update_and_render (
        &mut self,
        metric: f64,
        over: bool,
    ) -> Result<ImageRaw<BinaryColor>, VizError> {
        // Delegate to blocking version (no actual async ops here)
        self.update_and_render_blocking(metric, over)
    }

    pub fn update_and_render_blocking (
        &mut self,
        metric: f64,
        over: bool,
    ) -> Result<ImageRaw<BinaryColor>, VizError> {

        let width = self.rect.size.width as u32;
        let height = self.rect.size.height as u32;
        if self.svg_supported {
            self.update(metric, over)?;
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
        metric: f64,
        over: bool,
    ) -> Result<ImageRaw<Gray4>, VizError> {

        let width = self.rect.size.width as u32;
        let height = self.rect.size.height as u32;
        if self.svg_supported {
            self.update(metric, over)?;
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
        "vu_stereo_with_center_peak" | "combination" => Visualization::VuStereoWithCenterPeak,
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
        Visualization::VuMono  |
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
    let size = if wide { Size::new(128, 64) } else { Size::new(256, 64) };
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
            )
        },
        Visualization::VuMono  => {
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
            )
        },
        Visualization::VuStereoWithCenterPeak => {
            Visual::new(
                kind,
                String::from(format!("{folder}vucombi.svg")),
                Rectangle::new(Point::zero(), Size::new(size.width, size.height)),
                -25.0,
                5.0,
                -44.01,
                44.01,
                true,
                false,
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
            )
        },
    };
    viz
}

