/*
 *  visualization.rs
 *
 *  LyMonS - worth the squeeze
 *	(c) 2020-25 Stuart Hunter
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
