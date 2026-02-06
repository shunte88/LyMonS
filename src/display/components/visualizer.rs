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
use embedded_graphics::mono_font::iso_8859_13::FONT_5X8;
use embedded_text::alignment::{HorizontalAlignment, VerticalAlignment};
use crate::display::color_proxy::{ConvertColor};
use crate::display::layout::LayoutConfig;
use crate::visualizer::Visualizer;
use crate::visualization::Visualization;
use crate::vision::{POLL_ENABLED, PEAK_METER_LEVELS_MAX};
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

/// Visualizer component wrapper
pub struct VisualizerComponent {
    visualizer: Option<Visualizer>,
    state: VisualizerState,
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

        Self {
            visualizer: None,
            state: VisualizerState::default(),
            viz_state,
            layout,
            visualization_type,
        }
    }

    /// Initialize the visualizer with actual Visualizer instance
    pub fn set_visualizer(&mut self, visualizer: Visualizer) {
        self.visualizer = Some(visualizer);
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
    pub fn set_visualization_type(&mut self, viz_type: Visualization) {
        self.visualization_type = viz_type;
    }

    /// Render the visualizer (monochrome version)
    pub fn render_mono<D>(&mut self, target: &mut D) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions + 'static,
    {
        // Dispatch based on visualization type
        match self.visualization_type {
            Visualization::PeakMono => {
                Self::draw_peak_mono(target, self.viz_state.last_peak_m, self.viz_state.last_hold_m, self.visualization_type, &mut self.viz_state)
            }
            Visualization::PeakStereo => {
                Self::draw_peak_pair(target, self.viz_state.last_peak_l, self.viz_state.last_peak_r, self.viz_state.last_hold_l, self.viz_state.last_hold_r, self.visualization_type, &mut self.viz_state)
            }
            Visualization::HistMono => {
                Self::draw_hist_mono(target, self.viz_state.last_bands_m.clone(), self.visualization_type, &mut self.viz_state)
            }
            Visualization::HistStereo => {
                Self::draw_hist_pair(target, self.viz_state.last_bands_l.clone(), self.viz_state.last_bands_r.clone(), self.visualization_type, &mut self.viz_state)
            }
            Visualization::VuMono => {
                let _db = self.viz_state.last_db_m;
                Ok(false) // TODO: Implement VU meters
            }
            Visualization::VuStereo => {
                let _l_db = self.viz_state.last_db_l;
                let _r_db = self.viz_state.last_db_r;
                Ok(false) // TODO: Implement VU meters
            }
            _ => Ok(false) // Other types not yet implemented
        }
    }

    /// Render the visualizer (grayscale version)
    pub fn render_gray4<D>(&mut self, target: &mut D) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = Gray4> + OriginDimensions + 'static,
    {
        // Dispatch based on visualization type
        match self.visualization_type {
            Visualization::PeakMono => {
                Self::draw_peak_mono_gray4(target, self.viz_state.last_peak_m, self.viz_state.last_hold_m, self.visualization_type, &mut self.viz_state)
            }
            Visualization::PeakStereo => {
                Self::draw_peak_pair_gray4(target, self.viz_state.last_peak_l, self.viz_state.last_peak_r, self.viz_state.last_hold_l, self.viz_state.last_hold_r, self.visualization_type, &mut self.viz_state)
            }
            Visualization::HistMono => {
                Self::draw_hist_mono_gray4(target, self.viz_state.last_bands_m.clone(), self.visualization_type, &mut self.viz_state)
            }
            Visualization::HistStereo => {
                Self::draw_hist_pair_gray4(target, self.viz_state.last_bands_l.clone(), self.viz_state.last_bands_r.clone(), self.visualization_type, &mut self.viz_state)
            }
            Visualization::VuMono => {
                let _db = self.viz_state.last_db_m;
                Ok(false) // TODO: Implement VU meters
            }
            Visualization::VuStereo => {
                let _l_db = self.viz_state.last_db_l;
                let _r_db = self.viz_state.last_db_r;
                Ok(false) // TODO: Implement VU meters
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
        l_level: u8,
        r_level: u8,
        l_hold: u8,
        r_hold: u8,
        vk: Visualization,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        use embedded_graphics::image::{Image, ImageRaw};
        use embedded_graphics::primitives::Rectangle;
        use embedded_graphics::Drawable;

        ensure_band_state(state, 0, 0, 0, vk, true, 0.0, 0.0, 0.0, 0.0, 0, 0);
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

        if !state.init && state.last_peak_l == l_level && state.last_peak_r == r_level && state.last_hold_l == l_hold && state.last_hold_r == r_hold {
            return Ok(need_flush);
        }

        state.last_peak_l = l_level;
        state.last_peak_r = r_level;
        state.last_hold_l = l_hold;
        state.last_hold_r = r_hold;
        state.init = false;

        for l in level_brackets {
            let nodeo = if l < 0 { 5 } else { 7 };
            let nodew = if l < 0 { 2 } else { 4 };

            for c in 0..2 {
                let mv = level_brackets[0] + if c == 0 { state.last_peak_l as i16 } else { state.last_peak_r as i16 };
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
        l_level: u8,
        r_level: u8,
        l_hold: u8,
        r_hold: u8,
        vk: Visualization,
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

        ensure_band_state(state, 0, 0, 0, vk, true, 0.0, 0.0, 0.0, 0.0, 0, 0);
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

        if !state.init && state.last_peak_l == l_level && state.last_peak_r == r_level && state.last_hold_l == l_hold && state.last_hold_r == r_hold {
            return Ok(need_flush);
        }

        state.last_peak_l = l_level;
        state.last_peak_r = r_level;
        state.last_hold_l = l_hold;
        state.last_hold_r = r_hold;
        state.init = false;

        for l in level_brackets {
            let nodeo = if l < 0 { 5 } else { 7 };
            let nodew = if l < 0 { 2 } else { 4 };

            for c in 0..2 {
                let mv = level_brackets[0] + if c == 0 { state.last_peak_l as i16 } else { state.last_peak_r as i16 };
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
        level: u8,
        hold: u8,
        vk: Visualization,
        state: &mut crate::vision::LastVizState,
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        use embedded_graphics::image::{Image, ImageRaw};
        use embedded_graphics::primitives::Rectangle;
        use embedded_graphics::Drawable;

        ensure_band_state(state, 0, 0, 0, vk, true, 0.0, 0.0, 0.0, 0.0, 0, 0);
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

        if !state.init && state.last_peak_m == level && state.last_hold_m == hold {
            return Ok(need_flush);
        }

        state.last_peak_m = level;
        state.last_hold_m = hold;
        state.init = false;

        for l in level_brackets {
            let nodeo = if l < 0 { 5 } else { 7 };
            let nodew = if l < 0 { 2 } else { 4 };
            let mv = level_brackets[0] + state.last_peak_m as i16;
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
        level: u8,
        hold: u8,
        vk: Visualization,
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

        ensure_band_state(state, 0, 0, 0, vk, true, 0.0, 0.0, 0.0, 0.0, 0, 0);
        let mut need_flush = false;

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

        if !state.init && state.last_peak_m == level && state.last_hold_m == hold {
            return Ok(need_flush);
        }

        state.last_peak_m = level;
        state.last_hold_m = hold;
        state.init = false;

        for l in level_brackets {
            let nodeo = if l < 0 { 5 } else { 7 };
            let nodew = if l < 0 { 2 } else { 4 };
            let mv = level_brackets[0] + state.last_peak_m as i16;
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

    fn draw_hist_panel_mono<D>(display: &mut D, label: &str, label_height: u32, label_pos: i32, origin: Point, panel_size: Size, bars: &[u8], caps: &[u8]) -> Result<(), D::Error>
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

    fn draw_hist_panel_gray4<D>(display: &mut D, label: &str, label_height: u32, label_pos: i32, origin: Point, panel_size: Size, bars: &[u8], caps: &[u8]) -> Result<(), D::Error>
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

    fn draw_hist_pair<D>(display: &mut D, bands_l: Vec<u8>, bands_r: Vec<u8>, vk: Visualization, state: &mut crate::vision::LastVizState) -> Result<bool, D::Error>
    where D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        ensure_band_state(state, bands_l.len(), bands_r.len(), 0, vk, false, 0.0, 0.0, 0.0, 0.0, 0, 0);
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

        Self::draw_hist_panel_mono(display, "Left", title_base as u32, title_pos, Point::new(mx, my), Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_l, &state.cap_l)?;
        Self::draw_hist_panel_mono(display, "Right", title_base as u32, title_pos, Point::new(mx + pane_w + gap, my), Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_r, &state.cap_r)?;

        Ok(true)
    }

    fn draw_hist_pair_gray4<D>(display: &mut D, bands_l: Vec<u8>, bands_r: Vec<u8>, vk: Visualization, state: &mut crate::vision::LastVizState) -> Result<bool, D::Error>
    where D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        ensure_band_state(state, bands_l.len(), bands_r.len(), 0, vk, false, 0.0, 0.0, 0.0, 0.0, 0, 0);
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

        Self::draw_hist_panel_gray4(display, "Left", title_base as u32, title_pos, Point::new(mx, my), Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_l, &state.cap_l)?;
        Self::draw_hist_panel_gray4(display, "Right", title_base as u32, title_pos, Point::new(mx + pane_w + gap, my), Size::new(pane_w as u32, inner_h as u32), &state.draw_bands_r, &state.cap_r)?;

        Ok(true)
    }

    fn draw_hist_mono<D>(display: &mut D, bands: Vec<u8>, vk: Visualization, state: &mut crate::vision::LastVizState) -> Result<bool, D::Error>
    where D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        ensure_band_state(state, 0, 0, bands.len(), vk, false, 0.0, 0.0, 0.0, 0.0, 0, 0);
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

    fn draw_hist_mono_gray4<D>(display: &mut D, bands: Vec<u8>, vk: Visualization, state: &mut crate::vision::LastVizState) -> Result<bool, D::Error>
    where D: DrawTarget<Color = Gray4> + OriginDimensions,
    {
        use crate::vision::ensure_band_state;
        ensure_band_state(state, 0, 0, bands.len(), vk, false, 0.0, 0.0, 0.0, 0.0, 0, 0);
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
}
