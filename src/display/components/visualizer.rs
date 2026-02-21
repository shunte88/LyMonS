/*
 *  display/components/visualizer.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Audio visualizer component
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

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::{BinaryColor, Gray4};
use embedded_graphics::primitives::{PrimitiveStyle, PrimitiveStyleBuilder};
use embedded_graphics::mono_font::iso_8859_13::{FONT_4X6, FONT_5X8};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_text::alignment::{HorizontalAlignment, VerticalAlignment};
use embedded_text::{TextBox, style::TextBoxStyleBuilder};
use crate::display::color_proxy::{ConvertColor};
use crate::display::layout::LayoutConfig;
use crate::visualizer::Visualizer;
use crate::visualization::{Visualization, Visual};
use crate::vision::{POLL_ENABLED, PEAK_METER_LEVELS_MAX};
use crate::draw::draw_circle;
use std::time::{Duration, Instant};

/// Visualizer component state
#[derive(Debug, Clone)]
pub struct VisualizerState {
    /// Audio level (0-100)
    pub level: u8,

    /// Peak percentage
    pub pct: f64,

    /// Whether visualizer needs initialization clear
    pub viz_init_clear: bool,
}

impl Default for VisualizerState {
    fn default() -> Self {
        Self {
            level: 0,
            pct: 0.0,
            viz_init_clear: false,
        }
    }
}

/// AIO Layout helper: right side for visualization
#[inline]
pub fn aio_meter_attributes(meter_area_start: i32, w: i32, h: i32) -> (i32, i32, i32, i32)
{
    // Layout: left side, half width for text, right side for visualization
        let mx = 3;
        let my = 6;
        let meter_width = w - meter_area_start - (2 * mx);
        let meter_height = h - (2 * my);
    (mx, my, meter_width, meter_height)
}

/// AIO Layout helper: left side, half width for text, right side for visualization
#[inline]
pub fn aio_text_attributes(w: i32) -> (i32, i32, i32)
{
    // Layout: left side, half width for text, right side for visualization
    let text_area_width = w/2;
    let text_margin = 2;
    let text_usable_width = text_area_width - (2 * text_margin);
    let meter_area_start = text_area_width+1;
    (text_margin, text_usable_width, meter_area_start)
}

/// Visualizer component wrapper
pub struct VisualizerComponent {
    visualizer: Option<Visualizer>,
    state: VisualizerState,
    viz: Visual,
    viz_state: crate::vision::LastVizState,
    layout: LayoutConfig,
    visualization_type: Visualization,
}

impl VisualizerComponent {
    /// Create a new visualizer component
    pub fn new(layout: LayoutConfig, visualization_type: Visualization) -> Self {
        let mut viz_state = crate::vision::LastVizState::default();
        // Set wide flag based on layout - critical for correct SVG loading
        viz_state.wide = layout.visualizer.is_wide;
        // Set spectrum history buffer size to match display width
        viz_state.spectrum_max_cols = layout.width as usize;
        let viz = crate::visualization::get_visual(visualization_type, viz_state.wide);
        Self {
            visualizer: None,
            state: VisualizerState::default(),
            viz,
            viz_state,
            layout,
            visualization_type,
        }
    }

    /// Initialize the visualizer with actual Visualizer instance
    pub fn set_visualizer(&mut self, visualizer: Visualizer) {
        self.visualizer = Some(visualizer);
    }

    pub fn update_visual(&mut self) {
        self.viz = crate::visualization::get_visual(self.visualization_type, self.viz_state.wide);
    }

    /// Get mutable reference to visualizer
    pub fn visualizer_mut(&mut self) -> Option<&mut Visualizer> {
        self.visualizer.as_mut()
    }

    /// Get reference to visualizer
    pub fn visualizer(&self) -> Option<&Visualizer> {
        self.visualizer.as_ref()
    }

    /// Update visualizer state
    pub fn update(&mut self, level: u8, pct: f64) {
        self.state.level = level;
        self.state.pct = pct;
    }

    /// Get current state
    pub fn state(&self) -> &VisualizerState {
        &self.state
    }

    /// Get visualization type
    pub fn visualization_type(&self) -> Visualization {
        self.visualization_type
    }

    /// Set visualization type
    /// Note: On wide displays, VuMono (downmix) is automatically shifted to VuStereo
    pub fn set_visualization_type(&mut self, viz_type: Visualization) {
        // Rule: vu_mono (downmix) is not supported on wide screens
        // Automatically switch to vu_stereo instead
        self.visualization_type = match (self.viz_state.wide, viz_type) {
            (true, Visualization::VuMono) => Visualization::VuStereo,
            (_, other) => other,
        };
        // Reset init flag when switching visualizations
        self.viz_state.init = true;  // prime

    }

    /// Render the visualizer (monochrome version)
    pub fn render_mono<D>(&mut self, target: &mut D) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions + 'static,
    {
        let viz_mut = &mut self.viz;

        // Dispatch based on visualization type
        match self.visualization_type {
            Visualization::PeakMono => {
                Self::draw_peak_mono(
                    target,  
                    viz_mut,
                    self.viz_state.this.peak_m, 
                    self.viz_state.this.hold_m, 
                    &mut self.viz_state
                )
            }
            Visualization::PeakStereo => {
                Self::draw_peak_pair(
                    target,  
                    viz_mut, 
                    self.viz_state.this.peak_l, 
                    self.viz_state.this.peak_r, 
                    self.viz_state.this.hold_l, 
                    self.viz_state.this.hold_r, 
                    &mut self.viz_state
                )
            }
            Visualization::HistMono => {
                Self::draw_hist_mono(
                    target,  
                    viz_mut,
                    self.viz_state.last_bands_m.clone(), 
                    &mut self.viz_state
                )
            }
            Visualization::HistStereo => {
                Self::draw_hist_pair(
                    target,  
                    viz_mut,
                    self.viz_state.last_bands_l.clone(), 
                    self.viz_state.last_bands_r.clone(), 
                    &mut self.viz_state
                )
            }
            Visualization::VuMono => {
                Self::draw_vu_mono(
                    target,
                    viz_mut,
                    self.viz_state.this.db_m,
                    &mut self.viz_state,
                )
            }
            Visualization::VuStereo => {
                Self::draw_vu_stereo(
                    target,
                    viz_mut,
                    self.viz_state.this.db_l,
                    self.viz_state.this.db_r,
                    &mut self.viz_state,
                )
            }
            Visualization::AioVuMono => {
                let track_info = self.viz_state.last_artist.clone();
                Self::draw_aio_vu_mono(
                    target,
                    viz_mut,
                    self.viz_state.this.db_m,
                    &track_info,
                    &mut self.viz_state,
                )
            }
            Visualization::AioHistMono => {
                let track_info = self.viz_state.last_artist.clone();
                Self::draw_aio_hist_mono(
                    target,
                    viz_mut,
                    self.viz_state.last_bands_m.clone(),
                    &track_info,
                    &mut self.viz_state,
                )
            }
            Visualization::WaveformSpectrum => {
                Self::draw_waveform_spectrum_mono(
                    target,
                    self.viz_state.last_waveform_l.clone(),
                    self.viz_state.last_waveform_r.clone(),
                    Vec::new(), // spectrum_column already in history
                    &mut self.viz_state,
                    &self.layout,
                )
            }
            Visualization::VuStereoWithCenterPeak => {
                Self::draw_vu_combi(
                    target,
                    viz_mut,
                    self.viz_state.this.db_l,
                    self.viz_state.this.db_r,
                    self.viz_state.this.peak_m,
                    self.viz_state.this.hold_m,
                    &mut self.viz_state,
                )
            }
            _ => Ok(false) // Other types not yet implemented
        }
    }

    /// Render the visualizer (grayscale version)
    pub fn render_gray4<D>(&mut self, target: &mut D) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions + 'static,
    {
        let viz_mut = &mut self.viz;
        // Dispatch based on visualization type
        match self.visualization_type {
            Visualization::PeakMono => {
                Self::draw_peak_mono_gray4(
                    target,
                    viz_mut,
                    self.viz_state.this.peak_m, 
                    self.viz_state.this.hold_m, 
                    &mut self.viz_state
                )
            }
            Visualization::PeakStereo => {
                Self::draw_peak_pair_gray4(
                    target,  
                    viz_mut, 
                    self.viz_state.this.peak_l, 
                    self.viz_state.this.peak_r, 
                    self.viz_state.this.hold_l, 
                    self.viz_state.this.hold_r, 
                    &mut self.viz_state
                )
            }
            Visualization::HistMono => {
                Self::draw_hist_mono_gray4(
                    target,  
                    viz_mut,
                    self.viz_state.last_bands_m.clone(),  
                    &mut self.viz_state
                )
            }
            Visualization::HistStereo => {
                Self::draw_hist_pair_gray4(
                    target,  
                    viz_mut, 
                    self.viz_state.last_bands_l.clone(), 
                    self.viz_state.last_bands_r.clone(), 
                    &mut self.viz_state
                )
            }
            Visualization::VuMono => {
                Self::draw_vu_mono_gray4(
                    target,
                    viz_mut,
                    self.viz_state.this.db_m,
                    &mut self.viz_state,
                )
            }
            Visualization::VuStereo => {
                Self::draw_vu_stereo_gray4(
                    target,
                    viz_mut,
                    self.viz_state.this.db_l,
                    self.viz_state.this.db_r,
                    &mut self.viz_state,
                )
            }
            Visualization::AioVuMono => {
                let track_info = self.viz_state.last_artist.clone();
                Self::draw_aio_vu_gray4(
                    target,
                    viz_mut,
                    self.viz_state.this.db_m,
                    &track_info,
                    &mut self.viz_state,
                )
            }
            Visualization::AioHistMono => {
                let track_info = self.viz_state.last_artist.clone();
                Self::draw_aio_hist_gray4(
                    target,
                    viz_mut,
                    self.viz_state.last_bands_m.clone(),
                    &track_info,
                    &mut self.viz_state,
                )
            }
            Visualization::WaveformSpectrum => {
                Self::draw_waveform_spectrum_gray4(
                    target, 
                    self.viz_state.last_waveform_l.clone(),
                    self.viz_state.last_waveform_r.clone(),
                    Vec::new(), // spectrum_column already in history
                    &mut self.viz_state,
                    &self.layout,
                )
            }
            Visualization::VuStereoWithCenterPeak => {
                Self::draw_vu_combi_gray4(
                    target,
                    viz_mut,
                    self.viz_state.this.db_l,
                    self.viz_state.this.db_r,
                    self.viz_state.this.peak_m,
                    self.viz_state.this.hold_m,
                    &mut self.viz_state,
                )
            }
            _ => Ok(false) // Other types not yet implemented
        }
    }

    /// Mark that initialization clear is needed
    pub fn mark_init_clear(&mut self) {
        self.state.viz_init_clear = true;
    }

    /// Check if init clear is needed
    pub fn needs_init_clear(&self) -> bool {
        self.state.viz_init_clear
    }

    /// Clear the init flag
    pub fn clear_init_flag(&mut self) {
        self.state.viz_init_clear = false;
    }

    /// Get mutable reference to visualization state
    pub fn viz_state_mut(&mut self) -> &mut crate::vision::LastVizState {
        &mut self.viz_state
    }

    /// Get reference to visualization state
    pub fn viz_state(&self) -> &crate::vision::LastVizState {
        &self.viz_state
    }

    //
    // Peak Meter Drawing Functions
    //

    /// Draw stereo peak meters (monochrome)
    fn draw_peak_pair<D>(
        display: &mut D,
        viz: &mut Visual,
        l_level: u8,
        r_level: u8,
        l_hold: u8,
        r_hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        use embedded_graphics::image::{Image, ImageRaw};
        use embedded_graphics::primitives::Rectangle;
        use embedded_graphics::Drawable;

        ensure_band_state(state, 0, 0, 0, viz, true);
        let mut need_flush = false;

        if state.init {
            let raw = ImageRaw::<BinaryColor>::new(&state.buffer, display.size().width);
            Image::new(&raw, Point::new(0, 0)).draw(display)?;
            need_flush = true;
        }

        let level_brackets: [i16; 19] = [-36, -30, -20, -17, -13, -10, -8, -7, -6, -5, -4, -3, -2, -1, 0, 2, 3, 5, 8];
        let hbar = 17;
        let mut xpos = 15;
        let ypos: [u8; 2] = [7, 40];

        if !state.init && state.last.peak_l == l_level && state.last.peak_r == r_level && state.last.hold_l == l_hold && state.last.hold_r == r_hold {
            return Ok(need_flush);
        }

        state.last.peak_l = l_level;
        state.last.peak_r = r_level;
        state.last.hold_l = l_hold;
        state.last.hold_r = r_hold;
        state.init = false;

        for l in level_brackets {
            let nodeo = if l < 0 { 5 } else { 7 };
            let nodew = if l < 0 { 2 } else { 4 };

            for c in 0..2 {
                let mv = level_brackets[0] + if c == 0 { state.last.peak_l as i16 } else { state.last.peak_r as i16 };
                let color = if mv >= l { BinaryColor::On } else { BinaryColor::Off };

                let rect = Rectangle::new(Point::new(xpos, ypos[c] as i32), Size::new(nodew, hbar));
                let style = PrimitiveStyleBuilder::new().fill_color(color).build();
                rect.into_styled(style).draw(display)?;
            }
            xpos += nodeo;
            need_flush = true;
        }
        Ok(need_flush)
    }

    /// Draw stereo peak meters (Gray4)
    fn draw_peak_pair_gray4<D>(
        display: &mut D,
        viz: &mut Visual,
        l_level: u8,
        r_level: u8,
        l_hold: u8,
        r_hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        use crate::drawsvg::get_svg_gray4;
        use embedded_graphics::image::{Image, ImageRaw};
        use embedded_graphics::primitives::Rectangle;
        use embedded_graphics::Drawable;

        ensure_band_state(state, 0, 0, 0, viz, true);
        let mut need_flush = false;

        if state.init {
            let width = display.size().width;
            let _ = get_svg_gray4(&state.svg_file, width, 64, &mut state.buffer);
            let raw = ImageRaw::<Gray4>::new(&state.buffer, width);
            Image::new(&raw, Point::new(0, 0)).draw(display)?;
            need_flush = true;
        }

        let level_brackets: [i16; 19] = [-36, -30, -20, -17, -13, -10, -8, -7, -6, -5, -4, -3, -2, -1, 0, 2, 3, 5, 8];
        let hbar = 17;
        let mut xpos = 15;
        let ypos: [u8; 2] = [7, 40];

        if !state.init && state.last.peak_l == l_level && state.last.peak_r == r_level && state.last.hold_l == l_hold && state.last.hold_r == r_hold {
            return Ok(need_flush);
        }

        state.last.peak_l = l_level;
        state.last.peak_r = r_level;
        state.last.hold_l = l_hold;
        state.last.hold_r = r_hold;
        state.init = false;

        for l in level_brackets {
            let nodeo = if l < 0 { 5 } else { 7 };
            let nodew = if l < 0 { 2 } else { 4 };

            for c in 0..2 {
                let mv = level_brackets[0] + if c == 0 { state.last.peak_l as i16 } else { state.last.peak_r as i16 };
                let color = if mv >= l { Gray4::WHITE } else { Gray4::BLACK };

                let rect = Rectangle::new(Point::new(xpos, ypos[c] as i32), Size::new(nodew, hbar));
                let style = PrimitiveStyleBuilder::new().fill_color(color).build();
                rect.into_styled(style).draw(display)?;
            }
            xpos += nodeo;
            need_flush = true;
        }
        Ok(need_flush)
    }

    /// Draw mono peak meter (monochrome)
    fn draw_peak_mono<D>(
        display: &mut D,
        viz: &mut Visual,
        level: u8,
        hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        use embedded_graphics::image::{Image, ImageRaw};
        use embedded_graphics::primitives::Rectangle;
        use embedded_graphics::Drawable;

        ensure_band_state(state, 0, 0, 0, viz, true);
        let mut need_flush = false;

        if state.init {
            let raw = ImageRaw::<BinaryColor>::new(&state.buffer, display.size().width);
            Image::new(&raw, Point::new(0, 0)).draw(display)?;
            need_flush = true;
        }

        let level_brackets: [i16; 19] = [-36, -30, -20, -17, -13, -10, -8, -7, -6, -5, -4, -3, -2, -1, 0, 2, 3, 5, 8];
        let hbar = 36;
        let mut xpos = 15;
        let ypos = 20;

        if !state.init && state.last.peak_m == level && state.last.hold_m == hold {
            return Ok(need_flush);
        }

        state.last.peak_m = level;
        state.last.hold_m = hold;
        state.init = false;

        for l in level_brackets {
            let nodeo = if l < 0 { 5 } else { 7 };
            let nodew = if l < 0 { 2 } else { 4 };
            let mv = level_brackets[0] + state.last.peak_m as i16;
            let color = if mv >= l { BinaryColor::On } else { BinaryColor::Off };

            let rect = Rectangle::new(Point::new(xpos, ypos), Size::new(nodew, hbar));
            let style = PrimitiveStyleBuilder::new().fill_color(color).build();
            rect.into_styled(style).draw(display)?;
            xpos += nodeo;
        }
        Ok(need_flush)
    }

    /// Draw mono peak meter (Gray4)
    fn draw_peak_mono_gray4<D>(
        display: &mut D,
        viz: &mut Visual,
        level: u8,
        hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        use crate::drawsvg::get_svg_gray4;
        use embedded_graphics::image::{Image, ImageRaw};
        use embedded_graphics::primitives::Rectangle;
        use embedded_graphics::Drawable;

        ensure_band_state(state, 0, 0, 0, viz, true);
        let mut need_flush = false;

        // REVIEW IF WE GO THE SVG ANIMATION ROUTE
        if state.init {
            let width = display.size().width;
            let _ = get_svg_gray4(&state.svg_file, width, 64, &mut state.buffer);
            let raw = ImageRaw::<Gray4>::new(&state.buffer, width);
            Image::new(&raw, Point::new(0, 0)).draw(display)?;
            need_flush = true;
        }

        let level_brackets: [i16; 19] = [-36, -30, -20, -17, -13, -10, -8, -7, -6, -5, -4, -3, -2, -1, 0, 2, 3, 5, 8];
        let hbar = 36;
        let mut xpos = 15;
        let ypos = 20;

        if !state.init && state.last.peak_m == level && state.last.hold_m == hold {
            return Ok(need_flush);
        }

        state.last.peak_m = level;
        state.last.hold_m = hold;
        state.init = false;

        for l in level_brackets {
            let nodeo = if l < 0 { 5 } else { 7 };
            let nodew = if l < 0 { 2 } else { 4 };
            let mv = level_brackets[0] + state.last.peak_m as i16;
            let color = if mv >= l { Gray4::WHITE } else { Gray4::BLACK };

            let rect = Rectangle::new(Point::new(xpos, ypos), Size::new(nodew, hbar));
            let style = PrimitiveStyleBuilder::new().fill_color(color).build();
            rect.into_styled(style).draw(display)?;
            xpos += nodeo;
        }
        Ok(need_flush)
    }

    //
    // Histogram Drawing Functions
    //

    const HIST_DECAY_PER_TICK: u8 = 1;
    const CAP_HOLD: Duration = Duration::from_millis(500);
    const CAP_DECAY_LPS: f32 = 8.0;
    const CAP_THICKNESS_PX: u32 = 1;

    fn update_body_decay(dst: &mut [u8], src: &[u8], elapsed: Duration) -> bool {
        let ticks = (elapsed.as_millis() / (POLL_ENABLED.as_millis().max(1))) as u32;
        if ticks == 0 { return false; }
        let step = (ticks as u8).saturating_mul(Self::HIST_DECAY_PER_TICK);
        let mut changed = false;
        for (d, &s) in dst.iter_mut().zip(src.iter()) {
            let new = if s >= *d { s } else { d.saturating_sub(step).max(s) };
            if new != *d { *d = new; changed = true; }
        }
        changed
    }

    fn update_caps(caps: &mut [u8], hold_until: &mut [Instant], last_upd: &mut [Instant], bars: &[u8], now: Instant) -> bool {
        let mut changed = false;
        for i in 0..bars.len() {
            let bar = bars[i];
            let cap = &mut caps[i];
            let hu = &mut hold_until[i];
            let lu = &mut last_upd[i];

            if bar >= *cap {
                if *cap != bar { *cap = bar; changed = true; }
                *hu = now + Self::CAP_HOLD;
                *lu = now;
                continue;
            }

            if now < *hu { continue; }

            let dt = now.saturating_duration_since(*lu).as_secs_f32();
            if dt <= 0.0 { continue; }
            let drop = (dt * Self::CAP_DECAY_LPS).floor() as u8;
            if drop == 0 { continue; }

            let after = cap.saturating_sub(drop).max(bar);
            if after != *cap { *cap = after; changed = true; }
            *lu = now;
        }
        changed
    }

    fn draw_hist_panel_mono<D>(
        display: &mut D, 
        label: &str, 
        label_height: u32, 
        label_pos: i32, 
        origin: Point, 
        panel_size: Size, 
        bars: &[u8], 
        caps: &[u8]
    ) -> Result<(), D::Error>
    where D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;
        use embedded_text::{TextBox, style::TextBoxStyleBuilder};
        use embedded_graphics::mono_font::MonoTextStyle;

        let clear_rect = Rectangle::new(Point::new(origin.x - 1, origin.y - 1), Size::new(panel_size.width + 2, panel_size.height + 2));
        clear_rect.into_styled(PrimitiveStyle::with_fill(BinaryColor::Off)).draw(display)?;

        let text_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
        let textbox_style = TextBoxStyleBuilder::new().alignment(HorizontalAlignment::Center).vertical_alignment(VerticalAlignment::Middle).build();
        let bounds = Rectangle::new(Point::new(origin.x, label_pos + 1), Size::new(panel_size.width, label_height - 2));
        TextBox::with_textbox_style(label, bounds, text_style, textbox_style).draw(display)?;

        if bars.is_empty() || panel_size.width == 0 || panel_size.height == 0 { return Ok(()); }

        let w = panel_size.width as i32;
        let h = panel_size.height as i32;
        let n = (bars.len() as i32 - 2).max(1);
        let mut stride = (w / n).max(1);
        let mut bar_w = (stride - 1).max(1);
        if n <= 4 && w > n { stride = w / n; bar_w = stride; }

        let max_level = PEAK_METER_LEVELS_MAX as u32;
        let h_u = (panel_size.height as u32).saturating_sub(2);

        for (i, &lvl) in bars.iter().enumerate() {
            if i > n as usize { break; }
            let level_u = (lvl as u32).min(max_level);
            let bar_h = ((level_u * h_u) / max_level) as i32;
            if bar_h <= 0 { continue; }
            let x = origin.x + (i as i32) * stride;
            let y = origin.y + (h - bar_h);
            Rectangle::new(Point::new(x, y), Size::new(bar_w as u32, bar_h as u32)).into_styled(PrimitiveStyle::with_fill(BinaryColor::On)).draw(display)?;
        }

        for (i, &lvl) in caps.iter().enumerate() {
            if i > n as usize { break; }
            let level_u = (lvl as u32).min(max_level);
            let cap_h = ((level_u * h_u) / max_level) as i32;
            if cap_h <= 0 { continue; }
            let x = origin.x + (i as i32) * stride;
            let mut y = origin.y + (h - cap_h) - (Self::CAP_THICKNESS_PX as i32 - 1);
            if y < origin.y { y = origin.y; }
            Rectangle::new(Point::new(x, y), Size::new(bar_w as u32, Self::CAP_THICKNESS_PX)).into_styled(PrimitiveStyle::with_fill(BinaryColor::On)).draw(display)?;
        }

        Ok(())
    }

    fn draw_hist_panel_gray4<D>(
        display: &mut D,
        label: &str, 
        label_height: u32, 
        label_pos: i32, 
        origin: Point, 
        panel_size: Size, 
        bars: &[u8], 
        caps: &[u8]
    ) -> Result<(), D::Error>
    where D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;
        use embedded_text::{TextBox, style::TextBoxStyleBuilder};
        use embedded_graphics::mono_font::MonoTextStyle;

        let clear_rect = Rectangle::new(Point::new(origin.x - 1, origin.y - 1), Size::new(panel_size.width + 2, panel_size.height + 2));
        clear_rect.into_styled(PrimitiveStyle::with_fill(Gray4::BLACK)).draw(display)?;

        let text_style = MonoTextStyle::new(&FONT_5X8, Gray4::WHITE);
        let textbox_style = TextBoxStyleBuilder::new().alignment(HorizontalAlignment::Center).vertical_alignment(VerticalAlignment::Middle).build();
        let bounds = Rectangle::new(Point::new(origin.x, label_pos + 1), Size::new(panel_size.width, label_height - 2));
        TextBox::with_textbox_style(label, bounds, text_style, textbox_style).draw(display)?;

        if bars.is_empty() || panel_size.width == 0 || panel_size.height == 0 { return Ok(()); }

        let w = panel_size.width as i32;
        let h = panel_size.height as i32;
        let n = (bars.len() as i32 - 2).max(1);
        let mut stride = (w / n).max(1);
        let mut bar_w = (stride - 1).max(1);
        if n <= 4 && w > n { stride = w / n; bar_w = stride; }

        let max_level = PEAK_METER_LEVELS_MAX as u32;
        let h_u = (panel_size.height as u32).saturating_sub(2);

        // Light cyan for bars (Gray4 value 11)
        let bar_color = Gray4::new(11);
        // Yellow/bright for caps (Gray4::WHITE)
        let cap_color = Gray4::WHITE;

        for (i, &lvl) in bars.iter().enumerate() {
            if i > n as usize { break; }
            let level_u = (lvl as u32).min(max_level);
            let bar_h = ((level_u * h_u) / max_level) as i32;
            if bar_h <= 0 { continue; }
            let x = origin.x + (i as i32) * stride;
            let y = origin.y + (h - bar_h);
            Rectangle::new(Point::new(x, y), Size::new(bar_w as u32, bar_h as u32)).into_styled(PrimitiveStyle::with_fill(bar_color)).draw(display)?;
        }

        for (i, &lvl) in caps.iter().enumerate() {
            if i > n as usize { break; }
            let level_u = (lvl as u32).min(max_level);
            let cap_h = ((level_u * h_u) / max_level) as i32;
            if cap_h <= 0 { continue; }
            let x = origin.x + (i as i32) * stride;
            let mut y = origin.y + (h - cap_h) - (Self::CAP_THICKNESS_PX as i32 - 1);
            if y < origin.y { y = origin.y; }
            Rectangle::new(Point::new(x, y), Size::new(bar_w as u32, Self::CAP_THICKNESS_PX)).into_styled(PrimitiveStyle::with_fill(cap_color)).draw(display)?;
        }

        Ok(())
    }

    fn draw_hist_pair<D>(
        display: &mut D, 
        viz: &mut Visual, 
        bands_l: Vec<u8>, 
        bands_r: Vec<u8>, 
        state: &mut crate::vision::LastVizState
    ) -> Result<bool, D::Error>
    where D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        ensure_band_state(state, bands_l.len(), bands_r.len(), 0, viz, true);
        state.last_bands_l.copy_from_slice(&bands_l);
        state.last_bands_r.copy_from_slice(&bands_r);

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let mut changed = false;
        changed |= Self::update_body_decay(&mut state.draw_bands_l, &state.last_bands_l, elapsed);
        changed |= Self::update_body_decay(&mut state.draw_bands_r, &state.last_bands_r, elapsed);
        changed |= Self::update_caps(&mut state.cap_l, &mut state.cap_hold_until_l, &mut state.cap_last_update_l, &state.draw_bands_l, now);
        changed |= Self::update_caps(&mut state.cap_r, &mut state.cap_hold_until_r, &mut state.cap_last_update_r, &state.draw_bands_r, now);

        if !changed && !state.init { return Ok(false); }
        state.init = false;

        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 { return Ok(false); }

        let mx = 3; let my = 6; let title_base = 10; let gap = 2;
        let inner_w = w - 2 * mx; let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base; let pane_w = (inner_w - gap) / 2;

        Self::draw_hist_panel_mono(
            display,
            "Left", 
            title_base as u32, 
            title_pos, 
            Point::new(mx, my), 
            Size::new(pane_w as u32, inner_h as u32), 
            &state.draw_bands_l, 
            &state.cap_l
        )?;
        Self::draw_hist_panel_mono(
            display, 
            "Right", 
            title_base as u32, 
            title_pos, 
            Point::new(mx + pane_w + gap, my), 
            Size::new(pane_w as u32, 
            inner_h as u32), 
            &state.draw_bands_r, 
            &state.cap_r
            )?;

        Ok(true)
    }

    fn draw_hist_pair_gray4<D>(
        display: &mut D, 
        viz: &mut Visual, 
        bands_l: Vec<u8>, 
        bands_r: Vec<u8>, 
        state: &mut crate::vision::LastVizState
    ) -> Result<bool, D::Error>
    where D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        ensure_band_state(state, bands_l.len(), bands_r.len(), 0, viz, true);
        state.last_bands_l.copy_from_slice(&bands_l);
        state.last_bands_r.copy_from_slice(&bands_r);

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let mut changed = false;
        changed |= Self::update_body_decay(&mut state.draw_bands_l, &state.last_bands_l, elapsed);
        changed |= Self::update_body_decay(&mut state.draw_bands_r, &state.last_bands_r, elapsed);
        changed |= Self::update_caps(&mut state.cap_l, &mut state.cap_hold_until_l, &mut state.cap_last_update_l, &state.draw_bands_l, now);
        changed |= Self::update_caps(&mut state.cap_r, &mut state.cap_hold_until_r, &mut state.cap_last_update_r, &state.draw_bands_r, now);

        if !changed && !state.init { return Ok(false); }
        state.init = false;

        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 { return Ok(false); }

        let mx = 3; let my = 6; let title_base = 10; let gap = 2;
        let inner_w = w - 2 * mx; let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base; let pane_w = (inner_w - gap) / 2;

        Self::draw_hist_panel_gray4(
            display,
            "Left", 
            title_base as u32, 
            title_pos, 
            Point::new(mx, my), 
            Size::new(pane_w as u32, inner_h as u32), 
            &state.draw_bands_l, 
            &state.cap_l
        )?;
        Self::draw_hist_panel_gray4(
            display,
            "Right", 
            title_base as u32, 
            title_pos, 
            Point::new(mx + pane_w + gap, my), 
            Size::new(pane_w as u32, inner_h as u32), 
            &state.draw_bands_r, 
            &state.cap_r
        )?;

        Ok(true)
    }

    fn draw_hist_mono<D>(
        display: &mut D, 
        viz: &mut Visual, 
        bands: Vec<u8>, 
        state: &mut crate::vision::LastVizState
    ) -> Result<bool, D::Error>
    where D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        ensure_band_state(state, 0, 0, bands.len(), viz, true);
        state.last_bands_m.copy_from_slice(&bands);

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let mut changed = false;
        changed |= Self::update_body_decay(&mut state.draw_bands_m, &state.last_bands_m, elapsed);
        changed |= Self::update_caps(&mut state.cap_m, &mut state.cap_hold_until_m, &mut state.cap_last_update_m, &state.draw_bands_m, now);

        if !changed && !state.init { return Ok(false); }
        state.init = false;

        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 { return Ok(false); }

        let mx = 3; let my = 6; let title_base = 10;
        let inner_w = w - 2 * mx; let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base; let pane_w = inner_w;

        Self::draw_hist_panel_mono(display, "Downmix", title_base as u32, title_pos, Point::new(mx, my), Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_m, &state.cap_m)?;

        Ok(true)
    }

    fn draw_hist_mono_gray4<D>(
        display: &mut D, 
        viz: &mut Visual, 
        bands: Vec<u8>, 
        state: &mut crate::vision::LastVizState
    ) -> Result<bool, D::Error>
    where D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        ensure_band_state(state, 0, 0, bands.len(), viz, true);
        state.last_bands_m.copy_from_slice(&bands);

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let mut changed = false;
        changed |= Self::update_body_decay(&mut state.draw_bands_m, &state.last_bands_m, elapsed);
        changed |= Self::update_caps(&mut state.cap_m, &mut state.cap_hold_until_m, &mut state.cap_last_update_m, &state.draw_bands_m, now);

        if !changed && !state.init { return Ok(false); }
        state.init = false;

        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 { return Ok(false); }

        let mx = 3; let my = 6; let title_base = 10;
        let inner_w = w - 2 * mx; let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base; let pane_w = inner_w;

        Self::draw_hist_panel_gray4(display, "Downmix", title_base as u32, title_pos, Point::new(mx, my), Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_m, &state.cap_m)?;

        Ok(true)
    }

    /// Draw waveform + spectrogram visualization (monochrome)
    fn draw_waveform_spectrum_mono<D>(
        display: &mut D,
        waveform_l: Vec<i16>,
        waveform_r: Vec<i16>,
        _spectrum_column: Vec<u8>,
        state: &mut crate::vision::LastVizState,
        _layout: &LayoutConfig,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;

        let Size { width, height } = display.size();
        let display_width = width as usize;
        let display_height = height as usize;

        // Split screen: top half for waveform, bottom half for spectrogram
        let waveform_height = display_height / 2;
        let spectrogram_height = display_height - waveform_height;

        // Draw waveforms (oscilloscope style)
        let l_offset = waveform_height / 4;
        let r_offset = (waveform_height * 3) / 4;

        for (x, (&l, &r)) in waveform_l.iter().zip(waveform_r.iter()).enumerate() {
            if x >= display_width { break; }

            // Map i16 (-32768..32767) to screen coordinates
            let l_y = l_offset as i32 + (l as i32 * l_offset as i32 / 32768);
            let r_y = r_offset as i32 + (r as i32 * r_offset as i32 / 32768);

            // Draw sample points
            let l_point = Point::new(x as i32, l_y.clamp(0, waveform_height as i32 - 1));
            let r_point = Point::new(x as i32, r_y.clamp(0, waveform_height as i32 - 1));

            embedded_graphics::Pixel(l_point, BinaryColor::On).draw(display)?;
            embedded_graphics::Pixel(r_point, BinaryColor::On).draw(display)?;
        }

        // Draw spectrogram (waterfall) - history is managed by the display manager
        let spec_y_offset = waveform_height as i32;

        if let Some(first_col) = state.spectrum_history.front() {
            let band_height = if first_col.is_empty() { 1 } else {
                (spectrogram_height / first_col.len()).max(1)
            };

            for (col_idx, column) in state.spectrum_history.iter().enumerate() {
                let x = col_idx as i32;
                if x >= display_width as i32 { break; }

                for (band_idx, &intensity) in column.iter().enumerate() {
                    let y = spec_y_offset + (band_idx * band_height) as i32;

                    // Threshold for monochrome display
                    if intensity > 128 {
                        let rect = Rectangle::new(
                            Point::new(x, y),
                            Size::new(1, band_height as u32)
                        );
                        rect.into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                            .draw(display)?;
                    }
                }
            }
        }

        Ok(true)
    }

    /// Draw waveform + spectrogram visualization (Gray4)
    /// a thing of beauty to behold
    fn draw_waveform_spectrum_gray4<D>(
        display: &mut D,
        waveform_l: Vec<i16>,
        waveform_r: Vec<i16>,
        _spectrum_column: Vec<u8>,
        state: &mut crate::vision::LastVizState,
        _layout: &LayoutConfig,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;

        let Size { width, height } = display.size();
        let display_width = width as usize;
        let display_height = height as usize;

        // Split screen: top half for waveform, bottom half for spectrogram
        let waveform_height = display_height / 2;
        let spectrogram_height = display_height - waveform_height;

        // Draw waveforms (oscilloscope style) - L in cyan, R in yellow
        let l_offset = waveform_height / 4;
        let r_offset = (waveform_height * 3) / 4;

        for (x, (&l, &r)) in waveform_l.iter().zip(waveform_r.iter()).enumerate() {
            if x >= display_width { break; }

            // Map i16 (-32768..32767) to screen coordinates
            let l_y = l_offset as i32 + (l as i32 * l_offset as i32 / 32768);
            let r_y = r_offset as i32 + (r as i32 * r_offset as i32 / 32768);

            // Draw sample points with color differentiation
            let l_point = Point::new(x as i32, l_y.clamp(0, waveform_height as i32 - 1));
            let r_point = Point::new(x as i32, r_y.clamp(0, waveform_height as i32 - 1));

            embedded_graphics::Pixel(l_point, Gray4::new(11)).draw(display)?; // cyan
            embedded_graphics::Pixel(r_point, Gray4::WHITE).draw(display)?;     // yellow
        }

        // Draw spectrogram (waterfall) - history is managed by the display manager
        let spec_y_offset = waveform_height as i32;

        if let Some(first_col) = state.spectrum_history.front() {
            let band_height = if first_col.is_empty() { 1 } else {
                (spectrogram_height / first_col.len()).max(1)
            };

            for (col_idx, column) in state.spectrum_history.iter().enumerate() {
                let x = col_idx as i32;
                if x >= display_width as i32 { break; }

                for (band_idx, &intensity) in column.iter().enumerate() {
                    let y = spec_y_offset + (band_idx * band_height) as i32;

                    // Map intensity (0-255) to Gray4 (0-15)
                    let gray_value = (intensity as u32 * 15 / 255) as u8;

                    if gray_value > 0 {
                        let rect = Rectangle::new(
                            Point::new(x, y),
                            Size::new(1, band_height as u32)
                        );
                        rect.into_styled(PrimitiveStyle::with_fill(Gray4::new(gray_value)))
                            .draw(display)?;
                    }
                }
            }
        }

        Ok(true)
    }

    /// Draw AIO VU visualization (monochrome) - combines track info with meter
    /// TODO: Replace simple meter with VU needle once VU color support is added
    fn draw_aio_vu_mono<D>(
        display: &mut D,
        _viz: &mut Visual,
        db: f32,
        track_info: &str,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;
        use crate::vision::{LEVEL_FLOOR_DB, LEVEL_CEIL_DB};

        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);

        // Layout: left side (64px) for text, right side for simple meter
        let (text_margin, text_usable_width, meter_area_start) = aio_text_attributes(w);        

        // Convert dB to level for simple vertical meter
        let x = ((db - LEVEL_FLOOR_DB) / (LEVEL_CEIL_DB - LEVEL_FLOOR_DB)).clamp(0.0, 1.0);
        let level = (x * PEAK_METER_LEVELS_MAX as f32).round() as u8;
        let mut changed = state.last.peak_m != level;
        changed |= state.last_artist != track_info;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;
        state.last.peak_m = level;

        // Draw track info text on left side if changed
        if state.last_artist != track_info {
            state.last_artist = track_info.to_string();

            let character_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
            let textbox_style = TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Top)
                .build();

            let text_rect = Rectangle::new(
                Point::new(text_margin, 3),
                Size::new(text_usable_width as u32, (h - 6) as u32)
            );

            let text_box = TextBox::with_textbox_style(
                track_info,
                text_rect,
                character_style,
                textbox_style
            );
            text_box.draw(display)?;
        }

        // Draw simple vertical meter on right side
        let (mx, my, meter_width, meter_height) = aio_meter_attributes(meter_area_start, w, h);

        // Calculate fill height based on level
        let fill_height = (level as i32 * meter_height) / PEAK_METER_LEVELS_MAX as i32;

        // Draw meter bar
        if fill_height > 0 {
            Rectangle::new(
                Point::new(meter_area_start + mx, my + (meter_height - fill_height)),
                Size::new(meter_width as u32, fill_height as u32)
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
        }

        Ok(true)
    }

    /// Draw AIO VU visualization (Gray4) - combines track info with meter
    /// TODO: Replace simple meter with VU needle once VU color support is added
    fn draw_aio_vu_gray4<D>(
        display: &mut D,
        viz: &mut Visual,
        db: f32,
        track_info: &str,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;
        use crate::vision::{LEVEL_FLOOR_DB, LEVEL_CEIL_DB};

        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);

        // Layout: left side (64px) for text, right side for simple meter
        let (text_margin, text_usable_width, meter_area_start) = aio_text_attributes(w);        

        // Convert dB to level for simple vertical meter
        let x = ((db - LEVEL_FLOOR_DB) / (LEVEL_CEIL_DB - LEVEL_FLOOR_DB)).clamp(0.0, 1.0);
        let level = (x * PEAK_METER_LEVELS_MAX as f32).round() as u8;
        let mut changed = state.last.peak_m != level;
        changed |= state.last_artist != track_info;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;
        state.last.peak_m = level;

        // Draw track info text on left side if changed
        if state.last_artist != track_info {
            state.last_artist = track_info.to_string();

            let character_style = MonoTextStyle::new(&FONT_4X6, Gray4::WHITE);
            let textbox_style = TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Top)
                .build();

            let text_rect = Rectangle::new(
                Point::new(text_margin, 3),
                Size::new(text_usable_width as u32, (h - 6) as u32)
            );

            let text_box = TextBox::with_textbox_style(
                track_info,
                text_rect,
                character_style,
                textbox_style
            );
            text_box.draw(display)?;
        }

        // Draw simple vertical meter on right side - cyan colored
        let (mx, my, meter_width, meter_height) = aio_meter_attributes(meter_area_start, w, h);

        // Calculate fill height based on level
        let fill_height = (level as i32 * meter_height) / PEAK_METER_LEVELS_MAX as i32;

        // Draw meter bar in cyan
        if fill_height > 0 {
            Rectangle::new(
                Point::new(meter_area_start + mx, my + (meter_height - fill_height)),
                Size::new(meter_width as u32, fill_height as u32)
            )
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(11))) // Cyan
            .draw(display)?;
        }

        Ok(true)
    }

    /// Draw AIO Histogram visualization (monochrome) - combines track info with histogram
    fn draw_aio_hist_mono<D>(
        display: &mut D,
        viz: &mut Visual,
        bands: Vec<u8>,
        track_info: &str,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;
        use crate::vision::ensure_band_state;

        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);

        // Ensure state buffers match band count
        ensure_band_state(state, 0, 0, bands.len(), viz, true);
        state.last_bands_m.copy_from_slice(&bands);

        // Compute body decay and peak caps
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let mut changed = Self::update_body_decay(&mut state.draw_bands_m, &state.last_bands_m, elapsed);
        changed |= Self::update_caps(&mut state.cap_m, &mut state.cap_hold_until_m, &mut state.cap_last_update_m, &state.draw_bands_m, now);
        changed |= state.last_artist != track_info;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // Layout: left side (64px) for text, right side for histogram
        let (text_margin, text_usable_width, meter_area_start) = aio_text_attributes(w);        

        // Draw track info text on left side if changed
        if state.last_artist != track_info {
            state.last_artist = track_info.to_string();

            let character_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
            let textbox_style = TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Top)
                .build();

            let text_rect = Rectangle::new(
                Point::new(text_margin, 3),
                Size::new(text_usable_width as u32, (h - 6) as u32)
            );

            let text_box = TextBox::with_textbox_style(
                track_info,
                text_rect,
                character_style,
                textbox_style
            );
            text_box.draw(display)?;
        }

        // Draw histogram on right side
        let (mx, my, meter_width, meter_height) = aio_meter_attributes(meter_area_start, w, h);

        Self::draw_hist_panel_mono(
            display,
            "Downmix",
            10,
            h - 10,
            Point::new(meter_area_start + mx, my),
            Size::new(meter_width as u32, meter_height as u32),
            &state.draw_bands_m,
            &state.cap_m
        )?;

        Ok(true)
    }

    /// Draw AIO Histogram visualization (Gray4) - combines track info with histogram
    fn draw_aio_hist_gray4<D>(
        display: &mut D,
        viz: &mut Visual,
        bands: Vec<u8>,
        track_info: &str,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;
        use crate::vision::ensure_band_state;

        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);

        // Ensure state buffers match band count
        ensure_band_state(state, 0, 0, bands.len(), viz, true);
        state.last_bands_m.copy_from_slice(&bands);

        // Compute body decay and peak caps
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let mut changed = Self::update_body_decay(&mut state.draw_bands_m, &state.last_bands_m, elapsed);
        changed |= Self::update_caps(&mut state.cap_m, &mut state.cap_hold_until_m, &mut state.cap_last_update_m, &state.draw_bands_m, now);
        changed |= state.last_artist != track_info;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // Layout: left side, half width for text, right side for visualization
        let (text_margin, text_usable_width, meter_area_start) = aio_text_attributes(w);        

        // Draw track info text on left side if changed
        if state.last_artist != track_info {
            state.last_artist = track_info.to_string();

            let character_style = MonoTextStyle::new(&FONT_4X6, Gray4::WHITE);
            let textbox_style = TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Top)
                .build();

            let text_rect = Rectangle::new(
                Point::new(text_margin, 3),
                Size::new(text_usable_width as u32, (h - 6) as u32)
            );

            let text_box = TextBox::with_textbox_style(
                track_info,
                text_rect,
                character_style,
                textbox_style
            );
            text_box.draw(display)?;
        }

        // Draw histogram on right side with cyan bars and yellow caps
        let (mx, my, meter_width, meter_height) = aio_meter_attributes(meter_area_start, w, h);

        Self::draw_hist_panel_gray4(
            display,
            "Downmix",
            10,
            h - 10,
            Point::new(meter_area_start + mx, my),
            Size::new(meter_width as u32, meter_height as u32),
            &state.draw_bands_m,
            &state.cap_m
        )?;

        Ok(true)
    }

    /// Draw VU mono visualization (monochrome) - single VU meter with needle
    fn draw_vu_mono<D>(
        display: &mut D,
        viz: &mut Visual,
        db: f32,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions + 'static,
    {
        use crate::vision::ensure_band_state;
        // Ensure state is initialized with VU physics parameters
        ensure_band_state(state, 0, 0, 0, viz, true);

        // Update VU physics
        state.vu_m.update(db as f64);
        let disp = state.vu_m.angle_degrees() as f32;

        let changed = state.last.db_m != db || state.last.disp_m != disp;

        state.last.db_m = db;
        state.last.disp_m = disp;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // this is the only place we reference color depth - easily conditionalalized to DRY the code
        // repeat the metrics - keeps it simple
        let raw_image = viz.update_and_render_blocking(
            state.last.disp_m as f64, 
            state.vu_m.is_overloaded(),
            0.00, 
            false,
        )
            .map_err(|e| format!("Visualizer render failed: {}", e)).unwrap();

        // Draw SVG image
        embedded_graphics::image::Image::new(&raw_image, Point::new(0, 0))
            .draw(display)?;

        Ok(true)
    }

    /// Draw VU mono visualization (Gray4) - single VU meter with red needle
    fn draw_vu_mono_gray4<D>(
        display: &mut D,
        viz: &mut Visual,
        db: f32,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;

        // Ensure state is initialized
        ensure_band_state(state, 0, 0, 0, viz, true);

        // Update VU physics
        state.vu_m.update(db as f64);
        let disp = state.vu_m.angle_degrees() as f32;

        let mut changed = state.last.db_m != db;
        changed |= state.last.disp_m != disp;

        state.last.db_m = db;
        state.last.disp_m = disp;

        if !changed && !state.init {
            return Ok(false)
        }
        state.init = false;

        // this is the only place we referce color depth - easily conditionalalized to DRY the code
        // repeat the metrics - keeps it simple
        let raw_image = viz.update_and_render_blocking_gray4(
            state.last.disp_m as f64, 
            state.vu_m.is_overloaded(),
            0.00, 
            false,
        )
            .map_err(|e| format!("Visualizer render failed: {}", e)).unwrap();

        // Draw SVG image
        embedded_graphics::image::Image::new(&raw_image, Point::new(0, 0))
            .draw(display)?;

        Ok(true)
    }

    /// Draw VU stereo visualization (monochrome) - dual VU meters with needles
    fn draw_vu_stereo<D>(
        display: &mut D,
        viz: &mut Visual,
        l_db: f32,
        r_db: f32,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions + 'static,
    {

        use crate::vision::ensure_band_state;

        // Ensure state is initialized with VU physics parameters (different from mono)
        ensure_band_state(state, 0, 0, 0, viz, true);

        // Update VU physics for both channels
        state.vu_l.update(l_db as f64);
        state.vu_r.update(r_db as f64);
        let disp_l = state.vu_l.angle_degrees() as f32;
        let disp_r = state.vu_r.angle_degrees() as f32;

        let mut changed = state.last.db_l != l_db || state.last.db_r != r_db;
        changed |= state.last.disp_l != disp_l || state.last.disp_r != disp_r;

        state.last.db_l = l_db;
        state.last.db_r = r_db;
        state.last.disp_l = disp_l;
        state.last.disp_r = disp_r;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // this is the only place we reference color depth - easily conditionalalized to DRY the code
        // repeat the metrics - keeps it simple
        let raw_image = viz.update_and_render_blocking(
            state.last.disp_l as f64, 
            state.vu_l.is_overloaded(),
            state.last.disp_r as f64, 
            state.vu_r.is_overloaded(),
        )
            .map_err(|e| format!("Visualizer render failed: {}", e)).unwrap();

        // Draw SVG image
        embedded_graphics::image::Image::new(&raw_image, Point::new(0, 0))
            .draw(display)?;

        Ok(true)
    }

    /// Draw VU stereo visualization (Gray4) - dual VU meters with red needles
    fn draw_vu_stereo_gray4<D>(
        display: &mut D,
        viz: &mut Visual, 
        l_db: f32,
        r_db: f32,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;

        // Ensure state is initialized
        ensure_band_state(state, 0, 0, 0, viz, false);

        // Update VU physics for both channels
        state.vu_l.update(l_db as f64);
        state.vu_r.update(l_db as f64);
        let disp_l = state.vu_l.angle_degrees() as f32;
        let disp_r = state.vu_r.angle_degrees() as f32;

        let mut changed = state.last.db_l != l_db || state.last.db_r != r_db;
        changed |= state.last.disp_l != disp_l || state.last.disp_r != disp_r;

        state.last.db_l = l_db;
        state.last.db_r = r_db;
        state.last.disp_l = disp_l;
        state.last.disp_r = disp_r;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        println!("{:#?}", state.vu_l);

        // this is the only place we referce color depth - easily conditionalalized to DRY the code
        // repeat the metrics - keeps it simple
        let raw_image = viz.update_and_render_blocking_gray4(
            state.last.disp_l as f64,
            state.vu_l.is_overloaded(),
            state.last.disp_r as f64, 
            state.vu_r.is_overloaded(),
        )
            .map_err(|e| format!("Visualizer render failed: {}", e)).unwrap();

        // Draw SVG image
        embedded_graphics::image::Image::new(&raw_image, Point::new(0, 0))
            .draw(display)?;

        Ok(true)

    }

    /// Draw VU stereo with center peak visualization (monochrome) - combination mode
    fn draw_vu_combi<D>(
        display: &mut D,
        viz: &mut Visual,
        l_db: f32,
        r_db: f32,
        peak_level: u8,
        peak_hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions + 'static,
    {

        use crate::vision::ensure_band_state;
        use crate::draw::draw_rectangle;

        // Ensure state is initialized with combination VU physics parameters
        ensure_band_state(state, 0, 0, 0, viz, true);

        // Update VU physics for both channels
        state.vu_l.update(l_db  as f64);
        state.vu_r.update(r_db  as f64);
        let disp_l = state.vu_l.angle_degrees() as f32;
        let disp_r = state.vu_r.angle_degrees() as f32;

        
        let mut changed = state.last.db_l != l_db || state.last.db_r != r_db;
        changed |= state.last.disp_l != disp_l || state.last.disp_r != disp_r;
        changed |= state.last.peak_m != peak_level || state.last.hold_m != peak_hold;

        state.last.db_l = l_db;
        state.last.db_r = r_db;
        state.last.disp_l = disp_l;
        state.last.disp_r = disp_r;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // this is the only place we reference color depth - easily conditionalalized to DRY the code
        // repeat the metrics - keeps it simple
        let raw_image = viz.update_and_render_blocking(
            state.last.disp_l as f64, 
            state.vu_l.is_overloaded(),
            state.last.disp_r as f64, 
            state.vu_r.is_overloaded(),
        )
            .map_err(|e| format!("Visualizer render failed: {}", e)).unwrap();

        // Draw SVG image
        embedded_graphics::image::Image::new(&raw_image, Point::new(0, 0))
            .draw(display)?;

            // Layout
        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 {
            return Ok(false);
        }

        let my = 8;
        let gap = 6;

        // Draw center peak meter
        if state.last.peak_m != peak_level || state.last.hold_m != peak_hold {
            let level_brackets: [i16; 19] = [
                -36, -30, -20, -17, -13, -10, -8, -7, -6, -5,
                -4, -3, -2, -1, 0, 2, 3, 5, 8
            ];
            state.last.peak_m = peak_level;
            state.last.hold_m = peak_hold;

            let top_meter = my + 1;
            let bottom_meter = h - 2 * my - 1;
            let nodeh = (bottom_meter as u32 - top_meter as u32) / (level_brackets.len() as u32 + 1);
            let nodew = 2 * gap as u32;
            let xpos = w / 2 - gap;
            let mut ypos = bottom_meter + nodeh as i32;

            for l in level_brackets {
                let mv = level_brackets[0] + state.last.peak_m as i16;
                let color = if mv >= l {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                };
                draw_rectangle(
                    display,
                    Point::new(xpos, ypos),
                    nodew,
                    nodeh,
                    color,
                    Some(0),
                    Some(BinaryColor::Off)
                )?;
                ypos -= nodeh as i32;
            }
        }

        Ok(true)
    }

    /// Draw VU stereo with center peak visualization (Gray4) - combination mode
    fn draw_vu_combi_gray4<D>(
        display: &mut D,
        viz: &mut Visual,
        l_db: f32,
        r_db: f32,
        peak_level: u8,
        peak_hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use embedded_graphics::image::{Image, ImageRaw};
        use crate::vision::ensure_band_state;
        use crate::draw::draw_rectangle;

        // Ensure state is initialized
        ensure_band_state(state, 0, 0, 0, viz, true);

        // Always redraw Gray4 SVG background
        // THIS IS NOW REDUNDANT
        let svg_path = crate::visualization::get_visualizer_panel(viz.kind, state.wide);
        if !svg_path.is_empty() {
            let width = display.size().width;
            let height = display.size().height;
            let _ = crate::drawsvg::get_svg_gray4(&svg_path, width, height, &mut state.buffer);
            let raw = ImageRaw::<Gray4>::new(&state.buffer, width);
            Image::new(&raw, Point::new(0, 0)).draw(display)?;
        }

        // Update VU physics for both channels
        state.vu_l.update(l_db as f64);
        state.vu_r.update(r_db as f64);
        let disp_l = state.vu_l.angle_degrees() as f32;
        let disp_r = state.vu_r.angle_degrees() as f32;

        let mut changed = state.last.db_l != l_db || state.last.db_r != r_db;
        changed |= state.last.disp_l != disp_l || state.last.disp_r != disp_r;
        changed |= state.last.peak_m != peak_level || state.last.hold_m != peak_hold;

        state.last.db_l = l_db;
        state.last.db_r = r_db;
        state.last.disp_l = disp_l;
        state.last.disp_r = disp_r;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // this is the only place we referce color depth - easily conditionalalized to DRY the code
        // repeat the metrics - keeps it simple
        let raw_image = viz.update_and_render_blocking_gray4(
            state.last.disp_l as f64, 
            state.vu_l.is_overloaded(),
            state.last.disp_r as f64, 
            state.vu_r.is_overloaded(),
        )
            .map_err(|e| format!("Visualizer render failed: {}", e)).unwrap();

        // Draw SVG image
        embedded_graphics::image::Image::new(&raw_image, Point::new(0, 0))
            .draw(display)?;

        // Layout
        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 {
            return Ok(false);
        }

        let my = 8;
        let gap = 6;

        // Draw center peak meter with cyan bars
        if state.last.peak_m != peak_level || state.last.hold_m != peak_hold {
            let level_brackets: [i16; 19] = [
                -36, -30, -20, -17, -13, -10, -8, -7, -6, -5,
                -4, -3, -2, -1, 0, 2, 3, 5, 8
            ];
            state.last.peak_m = peak_level;
            state.last.hold_m = peak_hold;

            let top_meter = my + 1;
            let bottom_meter = h - 2 * my - 1;
            let nodeh = (bottom_meter as u32 - top_meter as u32) / (level_brackets.len() as u32 + 1);
            let nodew = 2 * gap as u32;
            let xpos = w / 2 - gap;
            let mut ypos = bottom_meter + nodeh as i32;

            for l in level_brackets {
                let mv = level_brackets[0] + state.last.peak_m as i16;
                let color = if mv >= l {
                    Gray4::new(11)  // Cyan
                } else {
                    Gray4::new(0)   // Off
                };
                draw_rectangle(
                    display,
                    Point::new(xpos, ypos),
                    nodew,
                    nodeh,
                    color,
                    Some(0),
                    Some(Gray4::new(0))
                )?;
                ypos -= nodeh as i32;
            }
        }

        Ok(true)
    }

}
