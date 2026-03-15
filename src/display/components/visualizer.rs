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

#![allow(dead_code)] // visualizer component helpers; some methods reserved

use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::mono_font::iso_8859_13::FONT_5X8;
use embedded_text::alignment::{HorizontalAlignment, VerticalAlignment};
use crate::display::color_proxy::{ColorProxy, GradientLut, HistColorScheme, Pal16};
use crate::display::layout::LayoutConfig;
use crate::visualizer::Visualizer;
use crate::visualization::{Visualization, Visual, SvgColorDepth};
use crate::vision::{POLL_ENABLED, PEAK_METER_LEVELS_MAX};
use crate::vision::ensure_band_state;
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

#[inline]
fn compute_leds(db: f64, level_brackets: &[i16]) -> Vec<bool> {
    level_brackets.iter().map(|&t| db >= t as f64).collect()
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
    /// Visualizer panel bounds for AIO modes — set from YAML layout before each render.
    aio_viz_rect: Option<Rectangle>,
    /// Pre-computed gradient LUT for Rgb565 histogram fills. Built once at construction.
    hist_lut: GradientLut,
}

impl VisualizerComponent {
    /// Create a new visualizer component
    pub fn new(layout: LayoutConfig, visualization_type: Visualization, hist_scheme: &str) -> Self {
        let mut viz_state = crate::vision::LastVizState::default();
        // Set wide flag based on layout - critical for correct SVG loading
        viz_state.wide = layout.visualizer.is_wide;
        // Set spectrum history buffer size to match display width
        viz_state.spectrum_max_cols = layout.width as usize;
        let viz = crate::visualization::get_visual(visualization_type, viz_state.wide, layout.clone());
        let scheme = match hist_scheme {
            "ocean"  => HistColorScheme::Ocean,
            "fire"   => HistColorScheme::Fire,
            "neon"   => HistColorScheme::Neon,
            _        => HistColorScheme::Classic,
        };
        let hist_lut = GradientLut::build(scheme, layout.height as u32);
        Self {
            visualizer: None,
            state: VisualizerState::default(),
            viz,
            viz_state,
            layout,
            visualization_type,
            aio_viz_rect: None,
            hist_lut,
        }
    }

    /// Initialize the visualizer with actual Visualizer instance
    pub fn set_visualizer(&mut self, visualizer: Visualizer) {
        self.visualizer = Some(visualizer);
    }

    pub fn update_visual(&mut self) {
        self.viz = crate::visualization::get_visual(self.visualization_type, self.viz_state.wide, self.layout.clone());
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
        // Reset init flag when switching visualizations
        self.viz_state.init = true;  // prime
        // Rule: vu_mono (downmix) is not supported on wide screens
        // Automatically switch to vu_stereo instead
        self.visualization_type = match (self.viz_state.wide, viz_type) {
            (true, Visualization::VuMono) => Visualization::VuStereo,
            (_, other) => other,
        };
    }

    /// Set the visualizer panel bounds for AIO modes (resolved from YAML layout).
    pub fn set_aio_viz_rect(&mut self, rect: Rectangle) {
        self.aio_viz_rect = Some(rect);
    }

    /// Render the visualizer — generic over display color depth and color proxy.
    ///
    /// Callers select the appropriate proxy at the call site:
    ///   `render::<_, MonoProxy>`, `render::<_, Gray4Proxy>`, `render::<_, Rgb565Proxy>`
    pub fn render<D, P>(&mut self, target: &mut D) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = P::Output> + OriginDimensions + 'static,
        P: ColorProxy,
        P::Output: SvgColorDepth,
    {
        let viz_mut = &mut self.viz;
        let s = &mut self.viz_state;
        match self.visualization_type {
            Visualization::PeakMono => {
                Self::draw_peak_mono(target, viz_mut, s.this.db_m, s.this.hold_m, s)
            }
            Visualization::PeakStereo => {
                Self::draw_peak_stereo(target, viz_mut, s.this.db_l, s.this.db_r, s.this.hold_l, s.this.hold_r, s)
            }
            Visualization::HistMono => {
                Self::draw_hist_mono::<D, P>(target, viz_mut, s.last_bands_m.clone(), s, &self.hist_lut)
            }
            Visualization::HistStereo => {
                Self::draw_hist_pair::<D, P>(target, viz_mut, s.last_bands_l.clone(), s.last_bands_r.clone(), s, &self.hist_lut)
            }
            Visualization::VuMono => {
                Self::draw_vu_mono(target, viz_mut, s.this.db_m, s)
            }
            Visualization::VuStereo => {
                Self::draw_vu_stereo(target, viz_mut, s.this.db_l, s.this.db_r, s)
            }
            Visualization::VuAio => {
                let (db_m, db_l, db_r) = (s.this.db_m, s.this.db_l, s.this.db_r);
                let rect = self.aio_viz_rect;
                Self::draw_aio_vu::<D, P>(target, viz_mut, rect, db_m, db_l, db_r, s)
            }
            Visualization::HistAio => {
                let (bands, bands_l, bands_r) = (s.last_bands_m.clone(), s.last_bands_l.clone(), s.last_bands_r.clone());
                let rect = self.aio_viz_rect;
                Self::draw_aio_hist::<D, P>(target, viz_mut, rect, bands, bands_l, bands_r, s, &self.hist_lut)
            }
            Visualization::WaveformSpectrum => {
                Self::draw_waveform_spectrum::<D, P>(target, s.last_waveform_l.clone(), s.last_waveform_r.clone(), Vec::new(), s, &self.layout)
            }
            Visualization::VuStereoWithCenterPeak => {
                Self::draw_vu_combi(target, viz_mut, s.this.db_l, s.this.db_r, s.this.db_m, s.this.hold_m, s)
            }
            _ => Ok(false)
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
    fn draw_peak_stereo<D>(
        display: &mut D,
        viz: &mut Visual,
        l_db: f32,
        r_db: f32,
        l_hold: u8,
        r_hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget + OriginDimensions,
        D::Color: SvgColorDepth,
    {
        let dummy: Vec<bool> = Vec::new();
        let level_brackets: [i16; 16] = [
            -30, -20, -15, -10, -7, -5, -3, -2, -1,
            0, 1, 2, 3, 5, 7, 10
        ];
        let n_db = level_brackets.len();

        ensure_band_state(state, n_db, n_db, 0, viz);
        let mut changed = state.last.db_l != l_db || state.last.db_r != r_db;
        changed |= state.last.hold_l != l_hold || state.last.hold_r != r_hold;

        state.last.db_l = l_db;
        state.last.db_r = r_db;
        state.last.hold_l = l_hold;
        state.last.hold_r = r_hold;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        viz.peak_l = compute_leds(l_db as f64, &level_brackets);
        viz.peak_r = compute_leds(r_db as f64, &level_brackets);
        let current_peak_l = viz.peak_l.iter().rposition(|&on| on).unwrap_or(0);
        let current_peak_r = viz.peak_r.iter().rposition(|&on| on).unwrap_or(0);
        viz.hold_l.fill(false);
        viz.hold_r.fill(false);
        viz.hold_l[current_peak_l] = true;
        viz.hold_r[current_peak_r] = true;

        viz.render_svg_and_draw(
            display,
            0.00, false, 0.00, false,
            dummy.clone(), dummy.clone(),
            viz.peak_l.clone(), viz.hold_l.clone(),
            viz.peak_r.clone(), viz.hold_r.clone(),
        )?;
        Ok(true)
    }

    /// Draw mono peak meter (monochrome)
    fn draw_peak_mono<D>(
        display: &mut D,
        viz: &mut Visual,
        m_db: f32,
        hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget + OriginDimensions,
        D::Color: SvgColorDepth,
    {
        let dummy: Vec<bool> = Vec::new();
        let level_brackets: [i16; 16] = [
            -30, -20, -15, -10, -7, -5, -3, -2, -1,
            0, 1, 2, 3, 5, 7, 10
        ];

        ensure_band_state(state, 0, 0, level_brackets.len(), viz);

        let mut changed = state.last.db_m != m_db;
        changed |= state.last.hold_m != hold;

        state.last.db_m = m_db;
        state.last.hold_m = hold;

        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        viz.peak_m = compute_leds(m_db as f64, &level_brackets);
        let current_peak = viz.peak_m.iter().rposition(|&on| on).unwrap_or(0);
        viz.hold_m.fill(false);
        viz.hold_m[current_peak] = true;

        viz.render_svg_and_draw(
            display,
            0.00, false, 0.00, false,
            viz.peak_m.clone(), viz.hold_m.clone(),
            dummy.clone(), dummy.clone(),
            dummy.clone(), dummy.clone(),
        )?;
        Ok(true)
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

    fn draw_hist_panel<D, P>(
        display: &mut D,
        label: &str,
        label_height: u32,
        label_pos: i32,
        origin: Point,
        panel_size: Size,
        bars: &[u8],
        caps: &[u8],
        lut: &GradientLut,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = P::Output> + OriginDimensions,
        P: ColorProxy,
    {
        use embedded_graphics::primitives::Rectangle;
        use embedded_text::{TextBox, style::TextBoxStyleBuilder};
        use embedded_graphics::mono_font::MonoTextStyle;

        let clear_rect = Rectangle::new(Point::new(origin.x - 1, origin.y - 1), Size::new(panel_size.width + 2, panel_size.height + 2));

        let text_style = MonoTextStyle::new(&FONT_5X8, P::on());
        let textbox_style = TextBoxStyleBuilder::new().alignment(HorizontalAlignment::Center).vertical_alignment(VerticalAlignment::Top).build();
        let bounds = Rectangle::new(Point::new(origin.x, label_pos + 1), Size::new(panel_size.width, label_height - 2));

        if bars.is_empty() || panel_size.width == 0 || panel_size.height == 0 { return Ok(()); }

        clear_rect.into_styled(PrimitiveStyle::with_fill(P::off())).draw(display)?;
        TextBox::with_textbox_style(label, bounds, text_style, textbox_style).draw(display)?;

        let w = panel_size.width as i32;
        let h = panel_size.height as i32;
        let n = (bars.len() as i32 - 1).max(1);
        let mut stride = num_integer::div_floor(w, n).max(1);
        if n*stride >= w {stride -= 1;} // should never happen!
        let mut bar_w = (stride - 1).max(1);
        if n <= 4 && w > n { stride = w / n; bar_w = stride-1; }

        let max_level = PEAK_METER_LEVELS_MAX as u32;
        let h_u = (panel_size.height as u32).saturating_sub(2);

        let cap_color = P::on(); // caps are always max brightness — distinct from the gradient
        let h_f = (h_u as f32 - 1.0).max(1.0);

        for (i, (&lvl, &cap)) in bars.iter().zip(caps.iter()).enumerate() {
            if i > n as usize - 1 { break; }
            let x = origin.x + (i as i32) * stride;
            let level_u = (lvl as u32).min(max_level);
            let bar_h = ((level_u * h_u) / max_level) as i32;
            let cap_level_u = (cap as u32).min(max_level);
            let cap_h = ((cap_level_u * h_u) / max_level) as i32;

            if bar_h > 0 {
                // draw_iter: single call, driver batches all pixels — colour computed per row.
                // pct = 0.0 at panel bottom (quiet), 1.0 at panel top (peak).
                let bar_top_y = origin.y + (h - bar_h);
                let bar_rect = Rectangle::new(
                    Point::new(x, bar_top_y),
                    Size::new(bar_w as u32, bar_h as u32),
                );
                display.draw_iter(bar_rect.points().map(|pt| {
                    let panel_y = (pt.y - origin.y) as usize;
                    let pct = 1.0 - panel_y as f32 / h_f;
                    Pixel(pt, P::bar_color_at_y(pct.clamp(0.0, 1.0), lut, panel_y))
                }))?;
            }

            if cap_h > 0 {
                let mut cy = origin.y + (h - cap_h) - (Self::CAP_THICKNESS_PX as i32 - 1);
                if cy < origin.y { cy = origin.y; }
                Rectangle::new(Point::new(x, cy), Size::new(bar_w as u32, Self::CAP_THICKNESS_PX))
                    .into_styled(PrimitiveStyle::with_fill(cap_color))
                    .draw(display)?;
            }
        }

        Ok(())
    }

    fn draw_hist_pair<D, P>(
        display: &mut D,
        viz: &mut Visual,
        bands_l: Vec<u8>,
        bands_r: Vec<u8>,
        state: &mut crate::vision::LastVizState,
        lut: &GradientLut,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = P::Output> + OriginDimensions,
        P: ColorProxy,
    {
        ensure_band_state(state, bands_l.len(), bands_r.len(), 0, viz);
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

        let mx = 3; 
        let my = 6; 
        let title_base = 10; 
        let gap = 2;
        let inner_w = w - mx - mx; 
        let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base; 
        let pane_w = (inner_w - gap) / 2;

        Self::draw_hist_panel::<D, P>(display, "Left",  title_base as u32, title_pos, Point::new(mx, my),              Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_l, &state.cap_l, lut)?;
        Self::draw_hist_panel::<D, P>(display, "Right", title_base as u32, title_pos, Point::new(mx + pane_w + gap, my), Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_r, &state.cap_r, lut)?;

        Ok(true)
    }

    fn draw_hist_mono<D, P>(
        display: &mut D,
        viz: &mut Visual,
        bands: Vec<u8>,
        state: &mut crate::vision::LastVizState,
        lut: &GradientLut,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = P::Output> + OriginDimensions,
        P: ColorProxy,
    {
        ensure_band_state(state, 0, 0, bands.len(), viz);
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

        let mx = 3; 
        let my = 6; 
        let title_base = 10;
        let inner_w = w - mx - mx; 
        let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base; 
        let pane_w = inner_w;

        Self::draw_hist_panel::<D, P>(display, "Downmix", title_base as u32, title_pos, Point::new(mx, my), Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_m, &state.cap_m, lut)?;

        Ok(true)
    }

    /// Draw waveform + spectrogram visualization (monochrome)
    /// Draw waveform + spectrogram visualization (generic over color depth)
    fn draw_waveform_spectrum<D, P>(
        display: &mut D,
        waveform_l: Vec<i16>,
        waveform_r: Vec<i16>,
        _spectrum_column: Vec<u8>,
        state: &mut crate::vision::LastVizState,
        _layout: &LayoutConfig,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = P::Output> + OriginDimensions,
        P: ColorProxy,
    {
        use embedded_graphics::primitives::Rectangle;

        let Size { width, height } = display.size();
        let display_width = width as usize;
        let display_height = height as usize;

        let waveform_height = display_height / 2;
        let spectrogram_height = display_height - waveform_height;

        let l_offset = waveform_height / 4;
        let r_offset = (waveform_height * 3) / 4;
        for (x, (&l, &r)) in waveform_l.iter().zip(waveform_r.iter()).enumerate() {
            if x >= display_width { break; }

            let l_y = l_offset as i32 + (l as i32 * l_offset as i32 / 32768);
            let r_y = r_offset as i32 + (r as i32 * r_offset as i32 / 32768);

            let l_point = Point::new(x as i32, l_y.clamp(0, waveform_height as i32 - 1));
            let r_point = Point::new(x as i32, r_y.clamp(0, waveform_height as i32 - 1));

            embedded_graphics::Pixel(l_point, P::proxy(Pal16::LightCyan)).draw(display)?;
            embedded_graphics::Pixel(r_point, P::on()).draw(display)?;
        }

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
                    let color = P::spectrum_pixel(intensity);

                    // Skip fully-off pixels to avoid unnecessary draws
                    if color != P::off() {
                        let rect = Rectangle::new(Point::new(x, y), Size::new(1, band_height as u32));
                        rect.into_styled(PrimitiveStyle::with_fill(color)).draw(display)?;
                    }
                }
            }
        }

        Ok(true)
    }

    /// Draw AIO VU visualization — SVG-based VU needle on the right half; left panel rendered by manager
    fn draw_aio_vu<D, P>(
        display: &mut D,
        viz: &mut Visual,
        viz_rect: Option<Rectangle>,
        db_m: f32,
        db_l: f32,
        db_r: f32,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = P::Output> + OriginDimensions,
        P: ColorProxy,
        D::Color: SvgColorDepth,
    {
        ensure_band_state(state, 0, 0, 0, viz);
        state.init = false;

        if state.wide {
            // Wide: stereo VU — position SVG at layout-provided rect (fallback: x=display.width/2)
            if let Some(rect) = viz_rect {
                viz.set_rect(rect);
            }
            state.vu_l.update(db_l as f64);
            state.vu_r.update(db_r as f64);
            let disp_l = state.vu_l.angle_degrees() as f32;
            let disp_r = state.vu_r.angle_degrees() as f32;
            state.last.db_l = db_l;
            state.last.db_r = db_r;
            state.last.disp_l = disp_l;
            state.last.disp_r = disp_r;

            let dummy: Vec<bool> = Vec::new();
            viz.render_svg_and_draw(
                display,
                disp_l as f64, state.vu_l.is_overloaded(),
                disp_r as f64, state.vu_r.is_overloaded(),
                dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(),
            )?;

        } else {

            // Narrow: mono VU — vuaio.svg covers full 128px, VU face in right portion
            state.vu_m.update(db_m as f64);
            let disp_m = state.vu_m.angle_degrees() as f32;
            state.last.db_m = db_m;
            state.last.disp_m = disp_m;

            let dummy: Vec<bool> = Vec::new();
            viz.render_svg_and_draw(
                display,
                disp_m as f64, 
                state.vu_m.is_overloaded(),
                0.0, 
                false,
                dummy.clone(), 
                dummy.clone(), 
                dummy.clone(), 
                dummy.clone(), 
                dummy.clone(), 
                dummy.clone(),
            )?;

        }

        Ok(true)
    }

    /// Draw AIO Histogram visualization — histogram right panel; left panel rendered by manager
    fn draw_aio_hist<D, P>(
        display: &mut D,
        viz: &mut Visual,
        viz_rect: Option<Rectangle>,
        bands: Vec<u8>,
        bands_l: Vec<u8>,
        bands_r: Vec<u8>,
        state: &mut crate::vision::LastVizState,
        lut: &GradientLut,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = P::Output> + OriginDimensions,
        P: ColorProxy,
    {
        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);

        let mx = 3i32;
        let my = 6i32;
        let title_base = 10i32;

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;
        state.init = false;

        // Get visualizer panel bounds from layout, or fall back to computed values.
        let (meter_x, meter_w, panel_h) = match viz_rect {
            Some(r) => (r.top_left.x, r.size.width as i32, r.size.height as i32),
            None if state.wide => { let x = w / 2 + 1; (x, w - x, h) }
            None => { let (_, _, s) = aio_text_attributes(w); (s, w - s, h) }
        };

        if state.wide {

            ensure_band_state(state, bands_l.len(), bands_r.len(), 0, viz);
            state.last_bands_l.copy_from_slice(&bands_l);
            state.last_bands_r.copy_from_slice(&bands_r);
            Self::update_body_decay(&mut state.draw_bands_l, &state.last_bands_l, elapsed);
            Self::update_body_decay(&mut state.draw_bands_r, &state.last_bands_r, elapsed);
            Self::update_caps(&mut state.cap_l, &mut state.cap_hold_until_l, &mut state.cap_last_update_l, &state.draw_bands_l, now);
            Self::update_caps(&mut state.cap_r, &mut state.cap_hold_until_r, &mut state.cap_last_update_r, &state.draw_bands_r, now);

            let gap = 2i32;
            let inner_w = meter_w - 2 * mx;
            let inner_h = panel_h - my - title_base - 1;
            let pane_w = (inner_w - gap) / 2;

            Self::draw_hist_panel::<D, P>(
                display, "L", title_base as u32, panel_h - title_base,
                Point::new(meter_x + mx, my),
                Size::new(pane_w as u32, inner_h as u32),
                &state.draw_bands_l, &state.cap_l, lut,
            )?;
            Self::draw_hist_panel::<D, P>(
                display, "R", title_base as u32, panel_h - title_base,
                Point::new(meter_x + mx + pane_w + gap, my),
                Size::new(pane_w as u32, inner_h as u32),
                &state.draw_bands_r, &state.cap_r, lut,
            )?;

        } else {

            ensure_band_state(state, 0, 0, bands.len(), viz);
            state.last_bands_m.copy_from_slice(&bands);
            Self::update_body_decay(&mut state.draw_bands_m, &state.last_bands_m, elapsed);
            Self::update_caps(&mut state.cap_m, &mut state.cap_hold_until_m, &mut state.cap_last_update_m, &state.draw_bands_m, now);

            let inner_w = meter_w - 2 * mx;
            let meter_h = panel_h - my - title_base - 1;

            Self::draw_hist_panel::<D, P>(
                display, "", // no label — scroller handles track info
                title_base as u32, panel_h - title_base,
                Point::new(meter_x + mx, my),
                Size::new(inner_w as u32, meter_h as u32),
                &state.draw_bands_m, &state.cap_m, lut,
            )?;

        }

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
        D: DrawTarget + OriginDimensions + 'static,
        D::Color: SvgColorDepth,
    {
        let dummy: Vec<bool> = Vec::new();
        ensure_band_state(state, 0, 0, 0, viz);

        state.vu_m.update(db as f64);
        let disp = state.vu_m.angle_degrees() as f32;

        let changed = state.last.db_m != db || state.last.disp_m != disp;
        state.last.db_m = db;
        state.last.disp_m = disp;

        if !changed && !state.init { return Ok(false); }
        state.init = false;

        viz.render_svg_and_draw(
            display,
            state.last.disp_m as f64, state.vu_m.is_overloaded(),
            0.00, false,
            dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(),
        )?;
        Ok(true)
    }

    fn draw_vu_stereo<D>(
        display: &mut D,
        viz: &mut Visual,
        l_db: f32,
        r_db: f32,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget + OriginDimensions + 'static,
        D::Color: SvgColorDepth,
    {
        let dummy: Vec<bool> = Vec::new();
        ensure_band_state(state, 0, 0, 0, viz);

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

        if !changed && !state.init { return Ok(false); }
        state.init = false;

        viz.render_svg_and_draw(
            display,
            state.last.disp_l as f64, state.vu_l.is_overloaded(),
            state.last.disp_r as f64, state.vu_r.is_overloaded(),
            dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(),
        )?;
        Ok(true)
    }

    fn draw_vu_combi<D>(
        display: &mut D,
        viz: &mut Visual,
        l_db: f32,
        r_db: f32,
        m_db: f32,
        _peak_hold: u8,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget + OriginDimensions + 'static,
        D::Color: SvgColorDepth,
    {
        let dummy: Vec<bool> = Vec::new();
        let level_brackets: [i16; 19] = [
            -36, -30, -20, -17, -13, -10, -8, -7, -6, -5, -4, -3, -2, -1,
            0, 2, 3, 5, 8
        ];

        ensure_band_state(state, 0, 0, 0, viz);

        state.vu_l.update(l_db as f64);
        state.vu_r.update(l_db as f64);
        let disp_l = state.vu_l.angle_degrees() as f32;
        let disp_r = state.vu_r.angle_degrees() as f32;

        let mut changed = state.last.db_l != l_db || state.last.db_r != r_db;
        changed |= state.last.db_m != m_db;
        changed |= state.last.disp_l != disp_l || state.last.disp_r != disp_r;

        state.last.db_l = l_db;
        state.last.db_r = r_db;
        state.last.db_m = m_db;
        state.last.disp_l = disp_l;
        state.last.disp_r = disp_r;
        state.last.hold_m = _peak_hold;

        if !changed && !state.init { return Ok(false); }
        state.init = false;

        viz.peak_m = compute_leds(m_db as f64, &level_brackets);
        let current_peak = viz.peak_m.iter().rposition(|&on| on).unwrap_or(0);
        viz.hold_m.fill(false);
        viz.hold_m[current_peak] = true;

        viz.render_svg_and_draw(
            display,
            state.last.disp_l as f64, state.vu_l.is_overloaded(),
            state.last.disp_r as f64, state.vu_r.is_overloaded(),
            viz.peak_m.clone(), viz.hold_m.clone(),
            dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone(),
        )?;
        Ok(true)
    }

}
