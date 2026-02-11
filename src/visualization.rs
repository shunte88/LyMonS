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
#[derive(Debug, Clone, Copy)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct Visual {
    pub kind: String,
    rect: Rectangle,
    svg_data: String,
    modified_svg_data: String,
    buffer: Vec<u8>,
    low_limit: f64,
    high_limit: f64,
    low_limit_degree: f64,
    high_limit_degree: f64,
    over_support: bool,
    can_widen: bool,
}

#[allow(dead_code)]
impl Visual {

    /// Creates a new `Visual` from SVG string data and target dimensions.
    pub fn new(
        kind: String,
        path: String, 
        rect: Rectangle, 
        low_limit: f64, 
        high_limit: f64,
        low_limit_degree: f64,
        high_limit_degree: f64,
        over_support: bool,
        can_widen: bool,
    ) -> Self {

        let width = rect.size.width as usize;
        let height = rect.size.height as usize;
        let svg_data = fs::read_to_string(path.as_str()).expect("load SVG file");
        let buffer_size = height as usize * ((width + 7) / 8) as usize;

        Self {
            kind,
            rect,
            svg_data,
            modified_svg_data: String::new(),
            buffer: vec![0u8; buffer_size],
            low_limit,
            high_limit,
            low_limit_degree,
            high_limit_degree,
            over_support,
            can_widen,
        }
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

/// Loads/sets the active easter_egg
pub fn get_visual(kind: Visualization, wide: bool) -> Visual {
    let folder = if wide {"./assets/ssd1322/"}else{"./assets/ssd1309/"};
    let size = if wide { Size::new(128, 64) } else { Size::new(256, 64) };
    let viz = match kind {
        Visualization::VuStereo => {
            Visual::new(
                String::from("vu_stereo"),
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
                String::from("vu_mono"),
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
                String::from("combination"),
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
                String::from("aio_vu_mono"),
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
                String::from("peak_stereo"),
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
                String::from("peak_mono"),
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
                String::from("aio_hist_mono"),
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
                String::from("hist_stereo"),
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
                String::from("hist_mono"),
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
                String::from("waveform_spectrum"),
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
                String::from("no_viz"),
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

