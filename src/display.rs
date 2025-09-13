/*
 *  display.rs
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

#[allow(unused_imports)]
#[allow(dead_code)]
use chrono::{Timelike, DateTime, Local};

use embedded_graphics::{
    image::{Image, ImageRaw},
    mono_font::{
        ascii::FONT_7X13, iso_8859_13::{
            FONT_4X6, FONT_5X7, FONT_5X8, FONT_6X10, FONT_6X13_BOLD, FONT_6X9, FONT_7X14}, MonoFont, MonoTextStyle, MonoTextStyleBuilder
    }, 
    pixelcolor::BinaryColor, 
    prelude::*, 
    primitives::{Arc as CircleArc, Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle}, 
    text::{self, 
        renderer::TextRenderer, 
        Baseline, 
        Text, 
    }
};

use embedded_text::{
    alignment::{HorizontalAlignment, VerticalAlignment},
    style::TextBoxStyleBuilder,
    TextBox,
};

use libc::_CS_POSIX_V6_ILP32_OFF32_CFLAGS;
use linux_embedded_hal::{I2cdev, I2CError as LinuxI2CError};

use ssd1306::{
    mode::{self, BufferedGraphicsMode},
    prelude::*,
    size::{self, DisplaySize128x64},
    I2CDisplayInterface,
    Ssd1306, 
};

use log::{debug, info, error};
use std::{time::{Duration, Instant}};
use std::error::Error; // Import the Error trait
use std::fmt; // Import fmt for Display trait
use std::thread::sleep;
use tokio::sync::Mutex as TokMutex;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::fs;
use display_interface::DisplayError;

use crate::imgdata;   // imgdata, glyphs and such
use crate::vu2up_ssd1309::{
    draw_vu_face as draw_vu_face_1309,
    vu_db_to_meter_angle as vu_db_to_meter_angle_1309 
};
use crate::vuphysics::{
    VuNeedleNew,
    VuNeedle,
    VU_FLOOR_DB, VU_CEIL_DB, VU_GAMMA, 
    db_to_drive,
    needle_tip
};   // VU physics
use crate::constants; // constants
use crate::climacell; // weather glyphs - need to move to SVG impl.
use crate::clock_font::{ClockFontData, set_clock_font}; // ClockFontData struct
use crate::deutils::seconds_to_hms;
use crate::weather::{Weather, WeatherData};
use crate::textable::{ScrollMode, TextScroller, transform_scroll_mode, GAP_BETWEEN_LOOP_TEXT_FIXED};
use crate::svgimage::{SvgImageRenderer, SvgImageError};
use crate::metrics::{MachineMetrics};
use crate::eggs::{Eggs, set_easter_egg};
use crate::visualizer::{VizPayload, Visualizer, VizFrameOut};
use crate::vision::{POLL_ENABLED, PEAK_METER_LEVELS_MAX, LastVizState, VuArcDigits};
use crate::draw::{
    clear_region,
    draw_line,
    draw_text,
    draw_text_region_align,
    draw_rectangle,
    draw_rect_with_style,
    draw_circle_from_center,
    draw_circle,
    draw_arc,
};
use crate::trig::{cosf, sinf, DEG_TO_RAD};

/// Custom error type for drawing operations that implements `std::error::Error`.
#[derive(Debug)]
pub enum DisplayDrawingError {
    /// An error originating from the `display-interface` crate.
    InterfaceI2CError(String),
    /// An error originating from the `display-interface` crate.
    InitializationError(DisplayError),
    /// An error originating from the `display-interface` crate.
    DrawingFailed(DisplayError),
    /// Error from SVG rendering.
    SvgError(crate::svgimage::SvgImageError),
    /// Error from Easter Egg rendering.
    EggsError(crate::eggs::EggsError),
    /// Error reading SVG file.
    IoError(std::io::Error),
    /// An error originating from the `display-interface` crate.
    VisualizerError(DisplayError),
    /// A generic string error for other display-related failures.
    Other(String),
}

impl fmt::Display for DisplayDrawingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayDrawingError::InterfaceI2CError(e) => write!(f, "Initialization {:?}", e),
            DisplayDrawingError::InitializationError(e) => write!(f, "Initialization failed {:?}", e),
            DisplayDrawingError::DrawingFailed(e) => write!(f, "Display drawing error: {:?}", e),
            DisplayDrawingError::VisualizerError(e) => write!(f, "Visualization error: {:?}", e),
            DisplayDrawingError::SvgError(e) => write!(f, "SVG rendering error: {}", e),
            DisplayDrawingError::EggsError(e) => write!(f, "Easter Egg error: {}", e),
            DisplayDrawingError::IoError(e) => write!(f, "IO error reading SVG file: {}", e),
            DisplayDrawingError::Other(e) => write!(f, "Display error: {}", e),
        }
    }
}

impl Error for DisplayDrawingError {}

// need to map SPI equivalent too
impl From<LinuxI2CError> for DisplayDrawingError {
    fn from(err: LinuxI2CError) -> Self {
        DisplayDrawingError::InterfaceI2CError(format!("{:?}", err))
    }
}

// Implement `From` for `display_interface::DisplayError` to automatically convert it
impl From<DisplayError> for DisplayDrawingError {
    fn from(err: DisplayError) -> Self {
        DisplayDrawingError::DrawingFailed(err)
    }
}
impl From<crate::svgimage::SvgImageError> for DisplayDrawingError {
    fn from(err: crate::svgimage::SvgImageError) -> Self {
        DisplayDrawingError::SvgError(err)
    }
}

impl From<crate::eggs::EggsError> for DisplayDrawingError {
    fn from(err: crate::eggs::EggsError) -> Self {
        DisplayDrawingError::EggsError(err)
    }
}
impl From<std::io::Error> for DisplayDrawingError {
    fn from(err: std::io::Error) -> Self {
        DisplayDrawingError::IoError(err)
    }
}

const HIST_DECAY_PER_TICK: u8 = 1;
const HIST_DECAY_TICK: Duration = POLL_ENABLED;
const CAP_HOLD: Duration = Duration::from_millis(1000);
const CAP_DECAY_LPS: f32 = 64.0;
const CAP_THICKNESS_PX: u32 = 1;

fn ensure_band_state(state: &mut LastVizState, n_l: usize, n_r: usize, n_m: usize, vu_init: bool) {

    let now = Instant::now();
    let mut ensure = |buf: &mut Vec<u8>, n: usize| { if buf.len() != n { *buf = vec![0; n]; }};
    let mut ensure_t = |buf: &mut Vec<Instant>, n: usize| { if buf.len() != n { *buf = vec![now; n]; }};

    ensure(&mut state.draw_bands_m, n_m);
    ensure(&mut state.draw_bands_l, n_l);
    ensure(&mut state.draw_bands_r, n_r);

    ensure(&mut state.last_bands_m, n_m);
    ensure(&mut state.last_bands_l, n_l);
    ensure(&mut state.last_bands_r, n_r);

    ensure(&mut state.cap_m, n_m);
    ensure(&mut state.cap_l, n_l);
    ensure(&mut state.cap_r, n_r);

    ensure_t(&mut state.cap_hold_until_m, n_m);
    ensure_t(&mut state.cap_hold_until_l, n_l);
    ensure_t(&mut state.cap_hold_until_r, n_r);

    ensure_t(&mut state.cap_last_update_m, n_m);
    ensure_t(&mut state.cap_last_update_l, n_l);
    ensure_t(&mut state.cap_last_update_r, n_r);
}

fn update_body_decay(dst: &mut [u8], src: &[u8], elapsed: Duration) -> bool {
    let ticks = (elapsed.as_millis() / (HIST_DECAY_TICK.as_millis().max(1))) as u32;
    if ticks == 0 { return false; }
    let step = (ticks as u8).saturating_mul(HIST_DECAY_PER_TICK);
    let mut changed = false;
    for (d, &s) in dst.iter_mut().zip(src.iter()) {
        let new = if s >= *d { s } else { d.saturating_sub(step).max(s) };
        if new != *d { *d = new; changed = true; }
    }
    changed
}

fn update_caps(
    caps: &mut [u8],
    hold_until: &mut [Instant],
    last_upd: &mut [Instant],
    bars: &[u8],
    now: Instant,
) -> bool {
    let mut changed = false;
    for i in 0..bars.len() {
        let bar = bars[i];
        let cap = &mut caps[i];
        let hu  = &mut hold_until[i];
        let lu  = &mut last_upd[i];

        if bar >= *cap {
            // jump cap up and hold
            if *cap != bar { *cap = bar; changed = true; }
            *hu = now + CAP_HOLD;
            *lu = now;
            continue;
        }

        // below cap
        if now < *hu {
            // still holding; nothing to do
            continue;
        }

        // decay after hold
        let dt = now.saturating_duration_since(*lu).as_secs_f32();
        if dt <= 0.0 { continue; }
        let drop = (dt * CAP_DECAY_LPS).floor() as u8;
        if drop == 0 { continue; }

        let after = cap.saturating_sub(drop).max(bar);
        if after != *cap { *cap = after; changed = true; }
        *lu = now; // advance decay clock
    }
    changed
}

fn draw_hist_panel_with_caps<D>(
    display: &mut D,
    label: &str,
    label_height: u32,
    label_pos: i32,
    origin: Point,
    panel_size: Size,
    bars: &[u8],   // 0..=PEAK_METER_LEVELS_MAX
    caps: &[u8],   // 0..=PEAK_METER_LEVELS_MAX
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    // Clear panel area (avoid ghosting on shrink)
    // this should be done once then use erase/replace pattern for the barsc
    let nu_origin = Point::new(origin.x - 1, origin.y - 1);
    let nu_panel_size = Size::new(panel_size.width + 2, panel_size.height + 2);

    draw_rectangle(
        display,
        nu_origin,
        nu_panel_size.width as u32,
        nu_panel_size.height as u32,
        BinaryColor::Off,
        None, None)
        .map_err(|e| D::Error::from(e))?;

    let _ = draw_line (
        display,
        Point::new(origin.x, origin.y),
        Point::new(origin.x + panel_size.width as i32, origin.y),
        BinaryColor::On,
        2
    );

    draw_text_region_align(
        display,
        label, 
        Point::new(origin.x, label_pos+1), 
        Size::new(panel_size.width, label_height-2), 
        HorizontalAlignment::Center, 
        VerticalAlignment::Middle, 
        &FONT_5X8)
        .map_err(|e| D::Error::from(e))?;

    if bars.is_empty() || panel_size.width == 0 || panel_size.height == 0 {
        return Ok(());
    }

    let w = panel_size.width as i32;
    let h = panel_size.height as i32;
    let n = bars.len() as i32 - 2;  // eat the last 2 bars - always empty
    let mut stride = (w / n).max(1);
    let mut bar_w = (stride - 1).max(1);
    if n <= 4 && w > n {
        stride = w / n;
        bar_w = stride;
    }

    let max_level = PEAK_METER_LEVELS_MAX as u32;
    let h_u = panel_size.height as u32 - 2;

    // bars
    for (i, &lvl) in bars.iter().enumerate() {
        if i > n as usize { break; }  
        let level_u = (lvl as u32).min(max_level);
        let bar_h = ((level_u * h_u) / max_level) as i32;
        if bar_h <= 0 { continue; }

        let x = origin.x + (i as i32) * stride;
        let y = origin.y + (h - bar_h);
        Rectangle::new(Point::new(x, y), Size::new(bar_w as u32, bar_h as u32))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
    }

    // caps (thin markers)
    for (i, &lvl) in caps.iter().enumerate() {
        if i > n as usize { break; }  
        let level_u = (lvl as u32).min(max_level);
        let cap_h = ((level_u * h_u) / max_level) as i32;
        if cap_h <= 0 { continue; }

        let x = origin.x + (i as i32) * stride;
        // place cap top just above bar height (clamp to panel)
        let mut y = origin.y + (h - cap_h) - (CAP_THICKNESS_PX as i32 - 1);
        if y < origin.y { y = origin.y; }

        Rectangle::new(Point::new(x, y), Size::new(bar_w as u32, CAP_THICKNESS_PX))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
    }

    Ok(())
}


// Reuseable helper: draw a single histogram panel
fn draw_hist_panel<D>(
    display: &mut D,
    origin: Point,
    panel_size: Size,
    bands: &[u8], // 0..=PEAK_METER_LEVELS_MAX
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    // Clear panel area (avoid ghosting on shrink)
    // this should be done once then use erase/replace pattern for the bars
    draw_rectangle(
        display,
        origin,
        panel_size.width as u32,
        panel_size.height as u32,
        BinaryColor::Off,
        Some(1), Some(BinaryColor::On))
        .map_err(|e| D::Error::from(e))?;

    if bands.is_empty() || panel_size.width == 0 || panel_size.height == 0 {
        return Ok(());
    }

    let w = panel_size.width as i32;
    let h = panel_size.height as i32;

    let n = bands.len() as i32;
    let mut stride = (w / n).max(1);
    let mut bar_w = (stride - 1).max(1);

    // For very few bands, fill the width (no gap)
    if n <= 4 && w > n {
        stride = w / n;
        bar_w = stride;
    }

    let max_level = PEAK_METER_LEVELS_MAX as u32;
    let h_u = panel_size.height as u32;

    for (i, &lvl) in bands.iter().enumerate() {
        let level_u = (lvl as u32).min(max_level);
        let bar_h = ((level_u * h_u) / max_level) as i32;
        if bar_h <= 0 { continue; }

        //debug_str = format!("{}{:>4}->{:>4}", debug_str, lvl, bar_h);

        let x = origin.x + (i as i32) * stride;
        let y = origin.y + (h - bar_h);

        Rectangle::new(Point::new(x, y), Size::new(bar_w as u32, bar_h as u32))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
        
    }

    Ok(())

}

/// Errors that can happen while placing an SVG on a DrawTarget.
#[derive(Debug)]
pub enum PutSvgError<DE> {
    Io(std::io::Error),
    Svg(Box<dyn std::error::Error + Send + Sync>),
    Draw(DE),
}
impl<DE: fmt::Debug> fmt::Display for PutSvgError<DE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PutSvgError::Io(e)   => write!(f, "I/O error: {e}"),
            PutSvgError::Svg(e)  => write!(f, "SVG error: {e}"),
            PutSvgError::Draw(e) => write!(f, "draw error: {e:?}"),
        }
    }
}

impl<DE: fmt::Debug> std::error::Error for PutSvgError<DE> {}

/// Direct SVG rendering with scale (no SVG dynamics).
pub async fn put_svg<D>(
    target: &mut D,
    path: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), PutSvgError<D::Error>>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{

    let data = fs::read_to_string(path).await.map_err(PutSvgError::Io)?;

    let bytes_per_row = ((width + 7) / 8) as usize;
    let buffer_size = height as usize * bytes_per_row;
    let mut buffer = vec![0u8; buffer_size];

    let svg_renderer = SvgImageRenderer::new(&data, width, height)
        .map_err(|e| PutSvgError::Svg(Box::new(e)))?;
    svg_renderer
        .render_to_buffer(&mut buffer)
        .map_err(|e| PutSvgError::Svg(Box::new(e)))?;

    // Blit to target
    let raw = ImageRaw::<BinaryColor>::new(&buffer, width);
    Image::new(&raw, Point::new(x, y))
        .draw(target)
        .map_err(PutSvgError::Draw)?;

    Ok(())
}

// Reuseable helper: draw a single vu meter panel
fn draw_vu_panel<D>(
    display: &mut D,
    label: &str,
    label_height: u32,
    label_pos: i32,
    origin: Point,
    panel_size: Size,
    sweep_min: i32, // = -48;
    sweep_max: i32, // = 48;
    displacement: f32,
    overload: bool,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{

    // Clear panel area (avoid ghosting on shrink)
    // this should be done once then use erase/replace pattern for the meter (needle)
    let panel = Rectangle::new(origin, panel_size);
    let panel_style = 
        PrimitiveStyleBuilder::new()
            .stroke_color(BinaryColor::On)
            .stroke_width(1)
            .fill_color(BinaryColor::Off)
            .build();
    draw_rect_with_style (
        display,
        panel,
        panel_style)
        .map_err(|e| D::Error::from(e))?;

    let w = panel_size.width as i32;
    let h = panel_size.height as i32;
    // handle downmix and combi - as well as display driver variants here
    let center = if display.size().width == 128 {
        let c = draw_vu_face_1309 (
            display, 
            panel,
            sweep_min,
            sweep_max,
        )
        .map_err(|e| D::Error::from(e))?;
        c
    } else {
        Point::new(origin.x as i32 + w/2,label_pos - 1 + label_height as i32 / 2)
    };

    //let center_y = label_pos - 1 + label_height as i32 / 2;
    let mut legend_y = center.y + 8;
    //let needle_len = 8 + panel_size.height as i32 / 2;
    let (tip, _base) = needle_tip(displacement, center.x, w, center.y);

    draw_text_region_align(
        display,
        "-", 
        Point::new(origin.x + 4, legend_y), 
        Size::new(w as u32 - 8, 6), 
        HorizontalAlignment::Left, 
        VerticalAlignment::Middle, 
        &FONT_4X6)
        .map_err(|e| D::Error::from(e))?;
    draw_text_region_align(
        display,
        "+", 
        Point::new(origin.x + 4, legend_y),
        Size::new(w as u32 - 8, 6), 
        HorizontalAlignment::Right, 
        VerticalAlignment::Middle, 
        &FONT_4X6)
        .map_err(|e| D::Error::from(e))?;

    // LED
    legend_y += 14;
    let led_fill = if overload { BinaryColor::On} else {BinaryColor::Off}; 
    draw_circle(
        display, 
        Point::new(origin.x + panel_size.width as i32 - 9, legend_y), 
        8, 
        BinaryColor::On, 
        1, 
        led_fill)
        .map_err(|e| D::Error::from(e))?;

    // Create a styled line
    let needle_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let _needle = Line::new(center, tip)
        .into_styled(needle_style)
        .draw(display)
        .map_err(|e| D::Error::from(e))?;

    draw_rectangle(
        display,
        Point::new(origin.x, label_pos-1), 
        panel_size.width as u32,
        label_height as u32,
        BinaryColor::Off,
        Some(2), Some(BinaryColor::On))
        .map_err(|e| D::Error::from(e))?;

    draw_text_region_align(
        display,
        label, 
        Point::new(origin.x+2, label_pos+1), 
        Size::new(panel_size.width, label_height-2), 
        HorizontalAlignment::Left, 
        VerticalAlignment::Middle, 
        &FONT_4X6)
        .map_err(|e| D::Error::from(e))?;

    draw_circle_from_center(
        display, 
        center, 
        (w / 4) as i32, 
        PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .map_err(|e| D::Error::from(e))?;

    Ok(())

}

/// Represents the audio bitrate mode for displaying the correct glyph.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AudioBitrate {
    SD = 1,
    HD = 2,
    DSD = 3,
    None, // No specific audio bitrate glyph displayed
}

/// Represents the repeat mode for displaying the correct glyph.
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum RepeatMode {
    Off,
    RepeatAll,
    RepeatOne,
}

fn map_repeat_mode(mode: u8) -> RepeatMode {
    match mode {
        0 => RepeatMode::Off,
        1 => RepeatMode::RepeatAll,
        2 => RepeatMode::RepeatOne,
        _ => RepeatMode::Off,
    }
}

/// Represents the shuffle mode for displaying the correct glyph.
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum ShuffleMode {
    Off,
    ByTracks,
    ByAlbums,
}

fn map_shuffle_mode(mode: u8) -> ShuffleMode {
    match mode {
        0 => ShuffleMode::Off,
        1 => ShuffleMode::ByTracks,
        2 => ShuffleMode::ByAlbums,
        _ => ShuffleMode::Off,
    }
}

/// NEW: Enum to define the current display mode (Scrolling text or Clock).
#[derive(Debug, PartialEq, Clone, Copy)]
#[allow(dead_code)]
pub enum DisplayMode {
    #[allow(dead_code)]
    Visualizer,     // WIP  - visualizations - meters, meters, meters
    EasterEggs,     // Done - easter eggs
    Scrolling,      // Done - Now Playing mode
    Clock,          // Done - Clock mode
    WeatherCurrent, // Done - Current Weather mode
    WeatherForecast,// Done - Weather Forecast mode
}

#[allow(dead_code)]
pub struct OledDisplay {

    // this definition is 100% correct - DO NOT MODIFY
    display: Ssd1306<I2CInterface<I2cdev>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>,
    // this definition is 100% correct - DO NOT MODIFY

    scrollers: Vec<TextScroller>,

    default_mono_style: MonoTextStyle<'static, BinaryColor>,
    
    // Status line (Line 0) specific data
    volume_percent: u8, // 0-100
    is_muted: bool,
    repeat_mode: RepeatMode,
    shuffle_mode: ShuffleMode,
    audio_bitrate: AudioBitrate,
    samplerate: String, // not displayed - logic only
    samplesize: String, // not displayed - logic only
    bitrate_text: String, // e.g., "24/192"

    pub current_mode: DisplayMode,

    // Clock display state
    last_clock_digits: [char; 5], // Store 'H', 'H', ':', 'M', 'M' for comparison
    colon_on: bool, // State of the colon for blinking
    #[allow(dead_code)]
    last_colon_toggle_time: Instant, // When the colon last toggled
    clock_font: ClockFontData<'static>, // Instance of the currently active clock font
    last_second_drawn: f32, // Store the last second drawn for progress bar updates
    last_date_drawn: String, // Store the last drawn date string to avoid constant redraws

    // Player display state (for track progress bar and info line)
    show_remaining: bool,
    pub track_duration_secs: f32,
    pub current_track_time_secs: f32,
    pub remaining_time_secs: f32,
    pub mode_text: String,
    last_track_duration_secs: f32,
    last_current_track_time_secs: f32,
    last_remaining_time_secs: f32,
    last_mode_text: String,
    scroll_mode: String,
    weather_data_arc: Option<Arc<TokMutex<Weather>>>, // Reference to the shared weather client
    weather_display_switch_timer: Option<Instant>,
    last_weather_draw_data: Vec<WeatherData>, // To track if weather data has changed for redraw
    artist: String,
    title: String,
    level: u8,
    pct: f64,
    viz: Option<Visualizer>,
    easter_egg: Eggs,
    show_metrics: bool,
    device_metrics: MachineMetrics,
    viz_init_clear: bool,
    last_viz_state: LastVizState,
}

#[allow(dead_code)]
impl OledDisplay {

    /// Initializes the OLED display over I2C.
    ///
    /// `i2c_bus_path` is typically "/dev/i2c-X" where X is the bus number (e.g., "/dev/i2c-1").
    /// NEED  support for i2c and spi, argument should drive the logic for the 
    /// interface to be instantiated
    pub fn new(
        i2c_bus_path: &str, 
        scroll_mode: &str, 
        clock_font: &str, 
        show_metrics: bool,
        egg_name: &str) -> Result<Self, DisplayDrawingError> {
        info!("Initializing Display on {}", i2c_bus_path);

        let i2c = I2cdev::new(i2c_bus_path)
            .map_err(|e| DisplayDrawingError::InterfaceI2CError(e.to_string()))?;
        let interface = I2CDisplayInterface::new(i2c);

        /*
        let interface = if bus_path.lower contains 'spi' {
        remove spi and split the string 
         // GPIOs for DC and RESET
        let dc = Pin::new(24); // GPIO24
        dc.export()?;
        dc.set_direction(linux_embedded_hal::Direction::Out)?;
        let rst = Pin::new(25); // GPIO25
        rst.export()?;
        rst.set_direction(linux_embedded_hal::Direction::Out)?;

        let interface = SPIInterface::new(spi, dc, rst);
        let mut display: Ssd1306<_, _, BufferedGraphicsMode<_>> =
        Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode();
        */

        let mut display = Ssd1306::new(
            interface,
            DisplaySize128x64,
            DisplayRotation::Rotate0,
        ).into_buffered_graphics_mode();

        display.init().map_err(|e| DisplayDrawingError::InitializationError(e))?;
        display.clear_buffer();
        display.flush().map_err(|e| DisplayDrawingError::DrawingFailed(e))?;

        info!("Display initialized successfully.");

        let default_mono_style = MonoTextStyleBuilder::new()
            .font(&FONT_5X8)
            .text_color(BinaryColor::On)
            .build();

        // --- Initialize TextScrollers for scrolling display mode ---
        let mut scrollers: Vec<TextScroller> = Vec::with_capacity(constants::MAX_LINES);
        let main_font = &FONT_5X8; // Use FONT_5X8 as the default for scrolling lines
        let real_scroll_mode = transform_scroll_mode(scroll_mode);

        let wide = display.size().width > 128;
        let mut state = LastVizState::default();
        state.wide = wide;
        // initialize vu scale - minimize heavy lifting at vizualization stage - should be nsec but hey...
        ensure_band_state(&mut state, 0, 0, 0, true);

        // Create TextScrollers for lines 1 to 4 (index 1 to 4 in a 0-indexed array)
        // Line 0 is status, Line 5 is player info.
        for i in 1..(constants::MAX_LINES - 1) { // Lines 1, 2, 3, 4
            let y_pos = constants::DISPLAY_REGION_Y_OFFSET + (constants::MAIN_FONT_HEIGHT as i32 + constants::MAIN_LINE_SPACING) * (i as i32);
            scrollers.push(TextScroller::new(
                String::from(format!("Scroller0{}", i)),
                Point::new(constants::DISPLAY_REGION_X_OFFSET, y_pos),
                constants::DISPLAY_REGION_WIDTH,
                String::from(""), // Initial empty text
                *main_font, // Pass the actual MonoFont
                real_scroll_mode, // Initial mode
            ));
        }

        Ok(OledDisplay {
            display,
            scrollers, // Store the created scrollers
            default_mono_style,
            // Status line (Line 0) specific data
            volume_percent: 0,
            is_muted: false,
            repeat_mode: RepeatMode::Off, // Default
            shuffle_mode: ShuffleMode::Off, // Default
            audio_bitrate: AudioBitrate::None,
            bitrate_text: String::new(),
            samplerate: String::new(),
            samplesize: String::new(),
            current_mode: DisplayMode::Scrolling, // Default to scrolling mode
            last_clock_digits: [' ', ' ', ' ', ' ', ' '], // Initialize with spaces
            colon_on: false, // Colon starts off
            last_colon_toggle_time: Instant::now(),
            clock_font: set_clock_font(clock_font),
            last_second_drawn: 61.0000, // Initialize to an invalid second to force first draw
            last_date_drawn: String::new(), // Initialize last drawn date
            // Initialize new player fields
            show_remaining: false,
            track_duration_secs: 0.00,
            current_track_time_secs: 0.00,
            remaining_time_secs: 0.00,
            mode_text: String::new(),
            last_track_duration_secs: 0.00,
            last_current_track_time_secs: 0.00,
            last_remaining_time_secs: 0.00,
            last_mode_text: String::new(),
            scroll_mode: scroll_mode.to_string(),
            // Weather fields
            weather_data_arc: None,
            last_weather_draw_data: vec![WeatherData::default();4],
            weather_display_switch_timer: None,
            artist: String::new(),
            title: String::new(),
            level: 1,
            pct: 0.00,
            viz: None,
            easter_egg: set_easter_egg(egg_name),
            show_metrics,
            device_metrics: MachineMetrics::default(),
            viz_init_clear: true,
            last_viz_state: state,
        })

    }

    /// Sets the `Arc<TokMutex<LMSWeather>>` for the display to access weather data.
    pub fn set_weather_client(&mut self, weather_client: Arc<TokMutex<Weather>>) {
        self.weather_data_arc = Some(weather_client);
    }

    pub fn clear_flushable_buffer(&mut self) {
        self.display.clear_buffer();
        self.display.flush().unwrap();
    
    }
    /// Clears the display buffer.
    pub fn clear(&mut self) {
        self.display.clear_buffer();
    }

    /// Sets the contrast (brightness) of the OLED display.
    /// `contrast` should be a value between 0 and 255.
    pub fn set_brightness(&mut self, contrast: u8) -> Result<(), Box<dyn std::error::Error>> {

        self.display.set_brightness(Brightness::custom(1, contrast))
            .map_err(|e| format!("Failed to set contrast: {:?}", e))?;
        Ok(())

    }

    fn calc_progress_angle(&mut self, angle0:f32, angle100:f32, progress_percent: f32) -> f32 {
        let clamped_percent = progress_percent.clamp(0.0, 100.0);
        let angle_range = angle100 - angle0;
        let factor = clamped_percent / 100.0;
        angle0 + (angle_range * factor)
    }

    pub fn get_egg_type(&mut self) -> u8 {
        self.easter_egg.egg_type
    }

    /// direct SVG rendering with scale and eggy SVG dynamics
    pub async fn put_eggy_svg(&mut self, artist: &str, title: &str, level: u8, pct: f64, tsec:f32, x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
        
        let mut eggy = self.easter_egg.clone();
        let raw_image = eggy.update_and_render( 
            artist,
            title,
            level, 
            pct,
            tsec
        )
        .await
        .map_err(DisplayDrawingError::EggsError)?;

        Image::new(&raw_image, Point::new(x as i32, y as i32))
            .draw(&mut self.display)
            .map_err(DisplayDrawingError::DrawingFailed)?;

        let combined = eggy.is_combined();
        let character_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);

        let textbox_style = if !combined {
            TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Middle)
            .build()
        } else {
            TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Left)
            .vertical_alignment(VerticalAlignment::Top)
            .build()
        };

        let trect = eggy.get_time_rect();
        let arect = eggy.get_artist_rect();
        let atext = eggy.get_artist();
        let text_box = TextBox::with_textbox_style(
            atext, 
            arect, 
            character_style, 
            textbox_style);
        text_box.draw(&mut self.display)
            .map_err(DisplayDrawingError::DrawingFailed)?;

        if !combined {
            let trect = eggy.get_title_rect();
            let ttext = eggy.get_title();
            let text_box = TextBox::with_textbox_style(
                ttext, 
                trect, 
                character_style, 
                textbox_style);
            text_box.draw(&mut self.display)
                .map_err(DisplayDrawingError::DrawingFailed)?;
        }
        if !trect.is_zero_sized() {
            let time_character_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
            let time_textbox_style = TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Middle)
                .build();
            let time_str = seconds_to_hms(eggy.get_track_time());
            let text_box = TextBox::with_textbox_style(
                time_str.as_str(), 
                trect, 
                time_character_style, 
                time_textbox_style);
            text_box.draw(&mut self.display)
                .map_err(DisplayDrawingError::DrawingFailed)?;
        }
        Ok(())
    }

    pub async fn test(&mut self, test: bool) 
    {
        if test {

            let save_egg = self.easter_egg.clone();

            // svg animation test
            for egg in [
                "cassette",
                "technics",
                "reel2reel",
                "vcr",
                "tubeamp",
                "radio40",
                "radio50",
                "tvtime",
                "ibmpc"] {

                self.easter_egg = set_easter_egg(egg);
                self.display.clear_buffer();

                for i in 0..100 { 
                    let pct = i as f64 / 100.0; 
                    self.put_eggy_svg("Bonnie Barrow", "My Dingo, My Love",2, pct, i as f32, 0, 0)
                    .await
                    .unwrap();
                    self.display.flush().unwrap();
                    sleep(Duration::from_millis(50));
                }
            }
            self.easter_egg = save_egg;

            self.clear();
            self.display.flush().unwrap();

        }
    }

    /// Displays IP and MAC address.
    pub fn connections(&mut self, inet:&str, eth0_mac_addr:&str, wlan0_mac_addr:&str) {

        info!("This IP ....: {}", inet);
        info!("This MAC ...: {}, eth0", eth0_mac_addr);
        info!("This MAC ...: {}, wlan0", wlan0_mac_addr);

        self.set_brightness(255).unwrap();
        self.display.clear_buffer();
        let mut y = 6;
        draw_line(&mut self.display,Point::new(2, y), Point::new(124, y), BinaryColor::On, 1).unwrap();
        y += 3;
        for text in [inet, eth0_mac_addr, wlan0_mac_addr] {            
            draw_text_region_align(
                &mut self.display,
                text, 
                Point::new(2, y), Size::new(124,10), 
                HorizontalAlignment::Center, VerticalAlignment::Middle, 
                &FONT_6X10).unwrap();
            y += 13;
        }
        draw_line(&mut self.display,Point::new(2, y), Point::new(124, y), BinaryColor::On, 2).unwrap();
        self.display.flush().unwrap();

        sleep(Duration::from_millis(2500));

    }

    /// Displays a splash screen image and fades the brightness in.
    /// The splash image is the LyMonS logo, version and build date
    pub async fn splash(&mut self, 
        show_splash: bool,         
        version: &str,
        build_date: &str
    ) -> Result<(), DisplayDrawingError> {
        self.display.clear_buffer();
        if show_splash {
   
            // 1. Set brightness to zero
            let mut contrast:u8 = 0;
            let _ = self.set_brightness(contrast);

            put_svg(
                &mut self.display,
                "./assets/lymonslogo.svg", 
                0, 0, 
                constants::DISPLAY_WIDTH as u32, constants::DISPLAY_HEIGHT as u32)
                .await.unwrap();
            self.display.flush().unwrap();

            let mut x = (constants::DISPLAY_WIDTH - (6*version.chars().count() as u32)) / 2;
            draw_text(&mut self.display, version, x as i32, constants::PLAYER_TRACK_INFO_LINE_Y_POS-17,&FONT_6X13_BOLD).unwrap();
            x = (constants::DISPLAY_WIDTH - (5*build_date.chars().count() as u32)) / 2;
            draw_text(&mut self.display, build_date, x as i32, constants::PLAYER_TRACK_INFO_LINE_Y_POS,&FONT_5X8).unwrap();
    
            self.display.flush().unwrap(); // Flush to display - yes at zero brightness

            const FADE_DURATION_MS: u64 = 3500;
            const FADE_STEPS: u8 = 60; // More steps for smoother fade
            let step_delay = Duration::from_millis(FADE_DURATION_MS / FADE_STEPS as u64);

            for i in 1..FADE_STEPS {
                contrast = (255.0 / FADE_STEPS as f32 * i as f32).round() as u8;
                let _ = self.set_brightness(contrast);
                sleep(step_delay);
            }

            // Ensure full brightness at the end
            // make this dusk-dawn compatible
            let _ = self.set_brightness(255);

        } else {

            self.display.flush().unwrap();
        
        }
        Ok(())

    }

    /// Calculates the width of the given text in pixels using the provided font.
    // This is a static/associated function, not a method, so it doesn't borrow self.
    fn get_text_width_specific_font(text: &str, font: &MonoFont) -> u32 {
        MonoTextStyleBuilder::new().font(font).text_color(BinaryColor::On).build()
            .measure_string(text, Point::zero(), Baseline::Top).bounding_box.size.width
    }

    /// Calculates the width of the given text in pixels using either the custom font or the default.
    fn get_text_width(&self, text: &str) -> u32 {
        self.default_mono_style.measure_string(text, Point::zero(), Baseline::Top).bounding_box.size.width
    }

    pub async fn setup_visualizer(&mut self, viz_type: &str, rx: watch::Receiver<bool>) -> Result<(), Box<dyn std::error::Error>> {
        self.viz = 
            if viz_type != "no_viz" {
                Some(Visualizer::spawn(viz_type, rx)?)
            } else {
                None
            };
        Ok(())
    }

    pub async fn setup_weather(&mut self, weather_config: &str) -> Result<(), Box<dyn std::error::Error>> {

        self.weather_data_arc = None;
        if weather_config != "" {
    
            match Weather::new(weather_config).await {
                Ok(w) => {
                    let w_arc = Arc::new(TokMutex::new(w));
                    // Initial fetch
                    match w_arc.lock().await.fetch_weather_data().await {
                        Ok(_) => debug!("Initial weather data fetched."),
                        Err(e) => error!("Failed initial weather data fetch: {}", e),
                    }
                    // Set the weather client in the display
                    self.set_weather_client(Arc::clone(&w_arc));
                    // Start polling in background
                    match Weather::start_polling(Arc::clone(&w_arc)).await {
                        Ok(_) => debug!("Weather polling started."),
                        Err(e) => error!("Failed to start weather polling: {}", e),
                    }
                    self.weather_display_switch_timer = Some(Instant::now()); // Start timer for weather display
                },
                Err(e) => error!("Failed to initialize Weather: {}", e),
            }

        }
        Ok(())
    
    }
 
    fn enable_vizualization(&mut self, on:bool) {
        match self.viz.as_mut() {
            Some(viz) => {
                viz.enable(on);
            },
            None => {}
        }
    }

    /// Sets the current display mode (e.g., Clock or Scrolling).
    pub async fn set_display_mode(&mut self, mode: DisplayMode) {
        if self.current_mode != mode {
            info!("Changing display mode to {:?}", mode);
            self.current_mode = mode;
            // Clear the buffer when changing modes to avoid visual artifacts
            self.display.clear_buffer();
            self.display.flush().unwrap(); // Attempt to flush, ignore error for mode change

            // If switching to Clock or Weather, stop all text scrollers
            if mode == DisplayMode::Clock || mode == DisplayMode::EasterEggs || mode == DisplayMode::WeatherCurrent || mode == DisplayMode::WeatherForecast {
                for scroller in &mut self.scrollers {
                    scroller.stop().await;
                }
            }

            if mode == DisplayMode::Visualizer {
                self.enable_vizualization(true);
                self.viz_init_clear = true;
                let _ = self.drain_frame_queue().await.unwrap();
                self.last_viz_state = LastVizState::default();

            } else { 
                self.enable_vizualization(false);   
            }

            // Reset clock digits so it redraws everything when switching to clock mode
            // This ensures a clean display of the clock digits initially.
            if mode == DisplayMode::Clock {
                self.last_clock_digits = [' ', ' ', ' ', ' ', ' '];
                self.last_second_drawn = 61.000; // Reset last second to force progress bar redraw
                self.last_date_drawn = String::new(); // Reset last drawn date to force redraw
            //} else if mode == DisplayMode::WeatherCurrent {
            //} else if mode == DisplayMode::WeatherForecast {
            } else if mode == DisplayMode::Scrolling {
                // When switching back to scrolling, the track details will be re-set
                // by the main loop, which will then trigger scroller starts as needed.
                self.last_track_duration_secs = 0.00; // Forces redraw
                self.last_current_track_time_secs = 0.00; // Forces redraw
                self.last_remaining_time_secs  = 0.00;
                self.last_mode_text = String::new(); // Forces redraw
            }

        }
    }

    /// Helper to draw an 8x8 glyph from raw byte data.
    fn draw_glyph(&mut self, data: &'static [u8; 8], x: i32, y: i32) -> Result<(), DisplayDrawingError> {
        let raw_image = ImageRaw::<BinaryColor>::new(data, constants::GLYPH_WIDTH);
        Image::new(&raw_image, Point::new(x, y))
            .draw(&mut self.display)
            .map_err(|e| DisplayDrawingError::from(e))
    }

    /// Helper to draw a custom clock character using the currently loaded font.
    fn draw_custom_clock_char(&mut self, char_to_draw: char, x: i32, y: i32) -> Result<(), DisplayDrawingError> {
        let char_image_raw = self.clock_font.get_char_image_raw(char_to_draw)
            .ok_or_else(|| DisplayDrawingError::Other(format!("Character '{}' not found in current clock font.", char_to_draw)))?;

        Image::new(char_image_raw, Point::new(x, y))
            .draw(&mut self.display)
            .map_err(DisplayDrawingError::DrawingFailed)?;
        Ok(())
    }

    pub fn set_status_line_data(&mut self, volume_percent: u8, is_muted: bool, samplesize: String, samplerate: String, repeat_mode: u8, shuffle_mode: u8)
    {
        let this_repeat = map_repeat_mode(repeat_mode);
        let this_shuffle = map_shuffle_mode(shuffle_mode);

        let changed = self.volume_percent != volume_percent ||
                      self.is_muted != is_muted ||
                      self.repeat_mode != this_repeat ||
                      self.shuffle_mode != this_shuffle ||
                      self.samplerate != samplerate ||
                      self.samplesize != samplesize;
                                
        if changed {

            self.volume_percent = volume_percent;
            self.is_muted = is_muted;
            self.repeat_mode = this_repeat;
            self.shuffle_mode = this_shuffle;
            self.samplerate = samplerate.clone();
            self.samplesize = samplesize.clone();
            
            let samp_size = samplesize.clone().parse::<u32>().unwrap_or(0);
            let samp_rate = samplerate.clone().parse::<u32>().unwrap_or(0);

            self.bitrate_text = if samp_size == 1 { // DSD/DSF 1-bit
                format!("DSD{} ", 
                    (samp_rate / 44100 as u32))
            } else { // vanilla, e.g. 24/96 etc
                format!("{}/{} ", 
                    samp_size, 
                    (samp_rate / 1000 as u32))
            }
            .to_string();

            self.audio_bitrate = if self.bitrate_text.to_uppercase().contains("DSD") {
                AudioBitrate::DSD
            } else if samp_size >= 24 || samp_rate > 44100 {
                AudioBitrate::HD
            } else if !self.bitrate_text.is_empty() { // Default to SD
                AudioBitrate::SD
            } else {
                AudioBitrate::None
            };

            self.level = self.audio_bitrate as u8;

        }

    }

    /// Sets the content for each scrolling line.
    pub async fn set_track_details(&mut self, albumartist: String, album: String, title: String, artist: String, scroll_mode_str:&str) {

        self.artist = artist.clone();
        self.title = title.clone();

        // Prepare data for each scroller
        let scroller_data = [
            (constants::TAG_DISPLAY_ALBUMARTIST, albumartist),  // @ 1 
            (constants::TAG_DISPLAY_ALBUM, album),              // @ 2
            (constants::TAG_DISPLAY_TITLE, title),              // @ 3
            (constants::TAG_DISPLAY_ARTIST, artist),            // @ 4
        ];

        let real_scroll_mode = transform_scroll_mode(scroll_mode_str);

        // First, collect all necessary information without mutable borrows of `self.scrollers`
        let mut prepared_updates: Vec<(usize, String, ScrollMode, u32)> = Vec::new();

        for (idx, text) in scroller_data.into_iter() {
            let scroller_font;
            { // Scoped to ensure the lock is released
                let scroller_state = self.scrollers[idx].state.lock().await;
                scroller_font = scroller_state.font.clone();
            }
            // Measure text width using the static helper function
            let measured_width = Self::get_text_width_specific_font(&text, &scroller_font); // Call static method
            debug!("Scroller{}. {} '{}'", idx, measured_width, text);
            prepared_updates.push((idx, text, real_scroll_mode, measured_width));
        }

        // Now, iterate and perform mutable operations - here idx is pre-adjusted
        for (idx, text, mode, text_width) in prepared_updates.into_iter() {
            let scroller = &mut self.scrollers[idx]; // Mutable borrow of specific scroller

            let display_region_width = scroller.width;
            scroller.update_content(text.clone(), mode, text_width).await;

            if mode != ScrollMode::Static && text_width > display_region_width {
                // If text needs to scroll, update content and start the scroller's internal timer
                debug!("{} triggering...", scroller.name);
                scroller.start().await;
            } else {
                // If text is static or fits, stop the scroller's timer and update content
                scroller.stop().await; // Ensures task is stopped and offset is reset
            }
        }
    }

    /// Sets the track duration, current time, and mode text for the player display.
    /// This method updates internal state and will trigger a re-draw on render_frame
    /// if any of the track info elements have changed.
    pub fn set_track_progress_data(
        &mut self,
        show_remaining: bool, 
        duration: f32, 
        current_time: f32, 
        remaining_time: f32, 
        mode: String) {
        if self.show_remaining != show_remaining { self.show_remaining = show_remaining; }
        if self.track_duration_secs != duration { self.track_duration_secs = duration; }
        if self.current_track_time_secs != current_time { self.current_track_time_secs = current_time; }
        if self.remaining_time_secs != remaining_time { self.remaining_time_secs = remaining_time; }
        if self.mode_text != mode { self.mode_text = mode; } // going to be rare as only playing will have us here
    }

    /// Updates and draws the clock on the display. Only flushes if changes occurred.
    /// This method is intended to be called frequently (e.g., every frame or second).
    pub fn update_and_draw_clock(&mut self, current_time: DateTime<Local>) -> Result<(), DisplayDrawingError> {

        let mut needs_flush = false; // No longer clear the entire buffer for each clock update to maintain persistence.

        // get fractional seconds - there has to be a cleaner way to get this
        let current_second_fidelity:f32 = current_time.format("%S.%f").to_string().parse::<f32>().unwrap_or(0.00);
    
        // Determine colon state for blinking (on for even seconds, off for odd)
        let new_colon_on_state = current_second_fidelity as u8 % 2 == 0;

        // Format time into HH:MM string
        let hours_str = format!("{:02}", current_time.hour());
        let minutes_str = format!("{:02}", current_time.minute());
        let time_chars: [char; 5] = [
            hours_str.chars().nth(0).unwrap_or(' '),
            hours_str.chars().nth(1).unwrap_or(' '),
            if new_colon_on_state { ':' } else { ' ' }, // Use new_colon_on_state directly
            minutes_str.chars().nth(0).unwrap_or(' '),
            minutes_str.chars().nth(1).unwrap_or(' '),
        ];

        let digit_width = self.clock_font.digit_width as i32;
        let digit_height = self.clock_font.digit_height as i32;

        // Calculate the total width of the 5 clock digits with custom spacing.
        let total_clock_visual_width: i32 = (digit_width * 5) +
                                             constants::CLOCK_DIGIT_GAP_HORIZONTAL * 2 + // H-H and H-Colon gaps
                                             constants::CLOCK_COLON_MINUTE_GAP +     // Colon-M1 gap
                                             constants::CLOCK_DIGIT_GAP_HORIZONTAL;  // M1-M2 gap

        // --- Calculate Y positions for the entire block (Clock + Progress Bar + Date) ---
        let progress_bar_height: u32 = 6;
        let border_thickness: i32 = 1;
        let date_font_height: u32 = constants::DATE_FONT_HEIGHT;

        // Total height of elements below the clock digits
        let total_lower_elements_height = progress_bar_height as i32 +
                                          constants::CLOCK_PROGRESS_BAR_GAP +
                                          constants::PROGRESS_BAR_DATE_GAP +
                                          date_font_height as i32;

        // Total height of the entire block (clock + progress bar + date)
        let total_block_height = digit_height + total_lower_elements_height;

        // Calculate starting Y for the entire block to center it vertically
        let clock_y_start = (constants::DISPLAY_HEIGHT as i32 - total_block_height) / 2;

        // Now set individual Y positions relative to clock_y_start
        let progress_bar_y = clock_y_start + digit_height + constants::CLOCK_PROGRESS_BAR_GAP;
        let date_y = progress_bar_y + progress_bar_height as i32 + constants::PROGRESS_BAR_DATE_GAP;

        // Calculate X positions for clock digits (horizontal centering remains the same as before)
        let clock_x_start: i32 = (constants::DISPLAY_WIDTH as i32 - total_clock_visual_width) / 2;

        let x_positions: [i32; 5] = [
            clock_x_start, // H1
            clock_x_start + digit_width + constants::CLOCK_DIGIT_GAP_HORIZONTAL, // H2
            clock_x_start + (digit_width * 2) + (constants::CLOCK_DIGIT_GAP_HORIZONTAL * 2), // Colon
            clock_x_start + (digit_width * 3) + (constants::CLOCK_DIGIT_GAP_HORIZONTAL * 2) + constants::CLOCK_COLON_MINUTE_GAP, // M1
            clock_x_start + (digit_width * 4) + (constants::CLOCK_DIGIT_GAP_HORIZONTAL * 3) + constants::CLOCK_COLON_MINUTE_GAP, // M2
        ];

        for i in 0..5 {
            let current_char_for_position = time_chars[i];
            let x_offset = x_positions[i];
            let y_offset = clock_y_start; // Use the new clock_y_start for drawing clock digits

            // Check for change: character itself or, specifically for the colon, its blink state
            let char_changed = current_char_for_position != self.last_clock_digits[i];
            let colon_state_changed = (i == 2) && (new_colon_on_state != self.colon_on); // Only check if it's the colon and its state truly changed

            if char_changed || colon_state_changed {
                // blanking rectangle
                draw_rectangle(
                    &mut self.display,
                    Point::new(x_offset, y_offset),
                    self.clock_font.digit_width, self.clock_font.digit_height,
                    BinaryColor::Off,
                    None, None)
                    .map_err(|e| DisplayDrawingError::from(e))?;
                // and draw the clock character
                self.draw_custom_clock_char(current_char_for_position, x_offset, y_offset)
                    .map_err(|e| DisplayDrawingError::from(e))?;
                
                self.last_clock_digits[i] = current_char_for_position;
                needs_flush = true;
            }
        }
        
        // Update the stored colon state *after* the loop, as it's used for comparison next iteration.
        self.colon_on = new_colon_on_state; 

        // --- Seconds Progress Bar ---
        let progress_bar_width_total = constants::DISPLAY_WIDTH as i32 - 4; // Display width minus 2px padding on each side
        let progress_bar_x = (constants::DISPLAY_WIDTH as i32 - progress_bar_width_total) / 2;

        if current_second_fidelity != self.last_second_drawn {

            draw_rectangle(
                &mut self.display,
                Point::new(progress_bar_x, progress_bar_y),
                progress_bar_width_total as u32, progress_bar_height,
                BinaryColor::Off,
                Some(border_thickness as u32),
                Some(BinaryColor::On)
            )
            .map_err(|e| DisplayDrawingError::from(e))?;

            // Calculate the filled width based on seconds (0.0000 to 59.99999)
            // Maps to a fill ratio from 0.0 to 1.0
            let fill_ratio = current_second_fidelity / 59.99999; 
            let fill_width_pixels = (progress_bar_width_total as f32 * fill_ratio).round() as i32;

            // The actual width of the inner filled bar, considering the border.
            let inner_fill_width = (fill_width_pixels - (2 * border_thickness)).max(0);
            let inner_height = progress_bar_height - (2 * border_thickness as u32);

            // Draw the filled part of the progress bar if there's actual fill to show
            if inner_fill_width > 0 {
                draw_rectangle(
                    &mut self.display,
                    Point::new(progress_bar_x+ border_thickness, progress_bar_y+ border_thickness),
                    inner_fill_width as u32, inner_height,
                    BinaryColor::On,None, None
                )
                .map_err(|e| DisplayDrawingError::from(e))?;
            }
            self.last_second_drawn = current_second_fidelity;
            needs_flush = true; // Mark for flush if progress bar updated
        }

        // --- Date Drawing ---
        let current_date_string = chrono::Local::now().format("%a %b %d").to_string(); // e.g., "Mon Jun 09"
        let date_text_width = self.get_text_width(&current_date_string);
        let date_x_pos = (constants::DISPLAY_WIDTH as i32 - date_text_width as i32) / 2;

        if current_date_string != self.last_date_drawn {

            // rtool to region
            draw_rectangle(
                &mut self.display,
                Point::new(0, date_y),
                constants::DISPLAY_WIDTH, constants::DATE_FONT_HEIGHT,
                BinaryColor::Off,None, None
            )
            .map_err(|e| DisplayDrawingError::from(e))?;

            draw_text(&mut self.display, &current_date_string,date_x_pos-4, date_y, &FONT_6X10).unwrap();

            self.last_date_drawn = current_date_string;
            needs_flush = true;
        }

        if self.show_metrics {
            let metrics_y = 39;
            let metrics = self.device_metrics.check();
            if metrics != self.device_metrics {
                self.device_metrics.update(metrics);
                let buff = format!("{:>3}% {:>2.1}C", 
                    metrics.cpu_load as u8, 
                    metrics.cpu_temp);
                draw_rectangle(
                    &mut self.display,
                    Point::new(0, metrics_y),
                    constants::DISPLAY_WIDTH, 6,
                    BinaryColor::Off,None, None
                )
                .map_err(|e| DisplayDrawingError::from(e))?;
                draw_text(&mut self.display, &buff,2, metrics_y, &FONT_4X6).unwrap();
                needs_flush = true;
            }

        }

        if needs_flush {
            self.display.flush().unwrap();
        }
        Ok(())

    }

    pub async fn is_weather_active(&self) -> bool {
        // Acquire a lock on the weather data
        let weather_data = if let Some(ref weather_arc) = self.weather_data_arc {
            weather_arc.lock().await
        } else {
            error!("Weather client not setup.");
            return false; // Nothing to draw if no weather clientfalse
        };
        return weather_data.active;
    }

    fn draw_vu_pair<D>(
        display: &mut D, 
        l_db: f32, 
        r_db: f32, 
        h: bool,
        state: &mut LastVizState
    ) -> Result<bool, D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {

        // initialize vu scale attributes
        ensure_band_state(state, 0, 0, 0, true);

        let changed = 
            state.last_metric[0] != l_db as i32 ||
            state.last_metric[1] != r_db as i32;
        // last metric is not taking into account time based decay
        // need to review before adding early doors

        // save the latest inputs
        state.last_db_l = l_db;
        state.last_db_r = r_db;
        let (disp_l, over_l) = state.vu_l.update_drive(l_db);
        let (disp_r, over_r) = state.vu_r.update_drive(r_db);

        // if nothing would change on screen, skip work (non-blocking) early doors
        //if !changed && !state.init {
        //    return Ok(false);
        //}
        state.init = false;

        // layout horizontal (h=true)
        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 { return Ok(false); }

        let mx = 3;
        let my = 6;
        let title_base = 10;
        let gap = 2;
        let inner_w = w - 2 * mx;
        let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base;
        let pane_w = (inner_w - gap) / 2;

        // ssd1309 - make dynamic!!!
        let sweep_min: i32 = -48;
        let sweep_max: i32 = 48;
        let disp_l = vu_db_to_meter_angle_1309(l_db, sweep_min, sweep_max);
        let disp_r = vu_db_to_meter_angle_1309(l_db, sweep_min, sweep_max);

        draw_vu_panel(
            display,
            "VU:L", title_base as u32, title_pos,
            Point::new(mx, my),
            Size::new(pane_w as u32, inner_h as u32),
            sweep_min,
            sweep_max,
            disp_l, over_l,
        ).map_err(|e| D::Error::from(e))?;

        draw_vu_panel(
            display,
            "VU:R", title_base as u32, title_pos,
            Point::new(mx + pane_w + gap, my),
            Size::new(pane_w as u32, inner_h as u32),
            sweep_min,
            sweep_max,
            disp_r, over_r,
        ).map_err(|e| D::Error::from(e))?;

        Ok(true)

    }

    fn draw_viz_combi<D>(
        display: &mut D,
        l_db: f32, 
        r_db: f32, 
        peak_level: u8, 
        peak_hold: u8,
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        let ret = Self::draw_vu_pair(display, l_db, r_db, false, state)?;
        Ok(ret)
    }

    fn draw_vu_mono<D>(
        display: &mut D, 
        db: f32,
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        Ok(false)
    }

    fn draw_aio_vu<D>(
        display: &mut D, 
        db: f32,
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        Ok(false)
    }

    fn draw_peak_pair<D>(
        display: &mut D, 
        l_level: u8, 
        r_level: u8, 
        l_hold: u8, 
        r_hold: u8, 
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {

        // we implement darw and erase, only initialize on first call
        if state.init {
            let raw_image = ImageRaw::<BinaryColor>::new(imgdata::PEAK_RMS_RAW_DATA, 128);
            Image::new(&raw_image, Point::new(0, 0))
                .draw(display)
                .map_err(|e| D::Error::from(e))?;
            state.init = false;

        }

        // 0..PEAK_METER_LEVELS_MAX
        let level_brackets: [i16; 19] = [
            -36, -30, -20, -17, -13, -10, -8, -7, -6, -5,
            -4,  -3,  -2,  -1,  0,   2,   3,  5,  8];

        let hbar = 17;
        let mut xpos = 15;
        let ypos:[u8;2] = [7, 40];

        if state.last_metric[0] == l_level as i32 &&
            state.last_metric[1] == r_level as i32 &&
            state.hold[0] == l_hold &&
            state.hold[1] == r_hold {
            return Ok(false);
        }

        state.last_metric[0] = l_level as i32; 
        state.last_metric[1] = r_level as i32; 
        state.hold[0] = l_hold;
        state.hold[1] = r_hold;

        for l in level_brackets {
            let nodeo = if l < 0 {5} else {7};
            let nodew = if l < 0 {2} else {4};
            for c in 0..2 {
                // levels are 0..48 - adjust to fit the display scaling
                // PEAK_METER_LEVELS_MAX
                //println!("{}", -PEAK_METER_LEVELS_MAX + state.last_metric[c] / PEAK_METER_LEVELS_MAX as i32 )
                let mv = level_brackets[0] as i32 + state.last_metric[c];
                let color = if mv >= l as i32 {
                    BinaryColor::On
                } else {
                    BinaryColor::Off};
                draw_rectangle(
                    display,
                    Point::new(xpos, ypos[c] as i32),
                    nodew, hbar,
                    color,
                    Some(0), Some(BinaryColor::Off))
                    .map_err(|e| D::Error::from(e))?;
            }
            xpos += nodeo;
        }
        Ok(true)
    }

    fn draw_peak_mono<D>(
        display: &mut D, 
        level: u8, hold: u8, 
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        // we implement draw and erase, only initialize on first call
        if state.init {
            let raw_image = ImageRaw::<BinaryColor>::new(imgdata::PEAK_MONO_RMS_RAW_DATA, 128);
            Image::new(&raw_image, Point::new(0, 0))
                .draw(display)
                .map_err(|e| D::Error::from(e))?;
            state.init = false;

        }

        let level_brackets: [i16; 19] = [
            -36, -30, -20, -17, -13, -10, -8, -7, -6, -5,
            -4,  -3,  -2,  -1,  0,   2,   3,  5,  8];

        let hbar = 34;
        let mut xpos = 15;
        let ypos = 23;

        if state.last_metric[0] == level as i32 && 
            state.hold[0] == hold {
            return Ok(false);
        }

        state.last_metric[0] = level as i32; 
        state.hold[0] = hold;

        for l in level_brackets {
            let nodeo = if l < 0 {5} else {7};
            let nodew = if l < 0 {2} else {4};
            // levels are 0..48 - adjust to fit the display scaling
            let mv = level_brackets[0] as i32 + state.last_metric[0];
            let color = if mv >= l as i32 {
                BinaryColor::On
            } else {
                BinaryColor::Off
            };
            draw_rectangle(
                display,
                Point::new(xpos, ypos),
                nodew, hbar,
                color,
                Some(0), Some(BinaryColor::Off))
                .map_err(|e| D::Error::from(e))?;
            xpos += nodeo;
        }
        Ok(true)
    }

    // need these interfaces to support Drawable
    pub fn draw_hist_pair<D>(
        display: &mut D, 
        bands_l: Vec<u8>, bands_r: Vec<u8>,
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {

        // resize state buffers if band count changed
        ensure_band_state(state, bands_l.len(), bands_r.len(), 0, false);

        // store latest inputs
        state.last_bands_l.copy_from_slice(&bands_l);
        state.last_bands_r.copy_from_slice(&bands_r);

        // compute body decay (rise fast, fall slow)
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let mut changed = false;
        changed |= update_body_decay(&mut state.draw_bands_l, &state.last_bands_l, elapsed);
        changed |= update_body_decay(&mut state.draw_bands_r, &state.last_bands_r, elapsed);

        // update peak caps (time-based hold then decay)
        changed |= update_caps(
            &mut state.cap_l,
            &mut state.cap_hold_until_l,
            &mut state.cap_last_update_l,
            &state.draw_bands_l,
            now,
        );
        changed |= update_caps(
            &mut state.cap_r,
            &mut state.cap_hold_until_r,
            &mut state.cap_last_update_r,
            &state.draw_bands_r,
            now,
        );

        // If nothing would change visually and we already drew once, skip
        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // layout
        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 { return Ok(false); }

        let mx = 3;
        let my = 6;
        let title_base = 10;
        let gap = 2;
        let inner_w = w - 2 * mx;
        let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base;
        let pane_w = (inner_w - gap) / 2;

        // left panel
        draw_hist_panel_with_caps(
            display,
            "Left", title_base as u32, title_pos,
            Point::new(mx, my),
            Size::new(pane_w as u32, inner_h as u32),
            &state.draw_bands_l,
            &state.cap_l,
        ).map_err(|e| D::Error::from(e))?;

        // right panel
        draw_hist_panel_with_caps(
            display,
            "Right", title_base as u32, title_pos,
            Point::new(mx + pane_w + gap, my),
            Size::new(pane_w as u32, inner_h as u32),
            &state.draw_bands_r,
            &state.cap_r,
        ).map_err(|e| D::Error::from(e))?;

        Ok(true)

    }

    pub fn draw_hist_pair_no_caps<D>(
        display: &mut D, 
        bands_l: Vec<u8>, bands_r: Vec<u8>,
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        ensure_band_state(state, bands_l.len(), bands_r.len(), 0, false);

        // 2) Save the latest inputs (debug/inspection)
        state.last_bands_l.copy_from_slice(&bands_l);
        state.last_bands_r.copy_from_slice(&bands_r);

        // 3) Compute decay steps based on elapsed time since last draw
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let ticks = (elapsed.as_millis() / (HIST_DECAY_TICK.as_millis().max(1))) as u32;
        let decay_steps: u8 = (ticks as u8).saturating_mul(HIST_DECAY_PER_TICK).max(1);
        // for *strictly time-based* decay (no redraws = no decay), remove `.max(1)`.

        // 4) Update draw bands with "rise fast, fall slow" rule; track if anything changed
        let mut changed = false;

        for (i, &target) in bands_l.iter().enumerate() {
            let curl = state.draw_bands_l[i];
            let newl = if target >= curl { target } else { curl.saturating_sub(decay_steps).max(target) };
            if newl != curl { changed = true; }
            state.draw_bands_l[i] = newl;
            let curr = state.draw_bands_r[i];
            let newr = if target >= curr { target } else { curr.saturating_sub(decay_steps).max(target) };
            if newr != curr { changed = true; }
            state.draw_bands_r[i] = newr;
        }

        // 5) If nothing would change on screen, skip work (non-blocking)
        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // 6) Layout and draw the panels
        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);

        if w <= 6 || h <= 4 {
            // too small to draw meaningfully
            return Ok(false);
        }

        let mx = 3;               // outer margins
        let my = 3;
        let gap = 3;              // gap between L/R histos
        let inner_w = w - 2 * mx;
        let inner_h = h - 2 * my;
        let pane_w = (inner_w - gap) / 2;

        // Left
        draw_hist_panel(
            display,
            Point::new(mx, my),
            Size::new(pane_w as u32, inner_h as u32),
            &state.draw_bands_l,
        ).map_err(|e| D::Error::from(e))?;

        // Right
        draw_hist_panel(
            display,
            Point::new(mx + pane_w + gap, my),
            Size::new(pane_w as u32, inner_h as u32),
            &state.draw_bands_r,
        ).map_err(|e| D::Error::from(e))?;

        Ok(true) // we dun did drew; caller should flush
    }

    fn draw_hist_mono<D>(
        display: &mut D, 
        bands: Vec<u8>,
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {

        // resize state buffers if band count changed
        ensure_band_state(state, 0, 0, bands.len(), false);

        // store latest inputs
        state.last_bands_m.copy_from_slice(&bands);

        // compute body decay (rise fast, fall slow)
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;

        let mut changed = false;
        changed |= update_body_decay(&mut state.draw_bands_m, &state.last_bands_m, elapsed);

        // update peak caps (time-based hold then decay)
        changed |= update_caps(
            &mut state.cap_m,
            &mut state.cap_hold_until_m,
            &mut state.cap_last_update_m,
            &state.draw_bands_m,
            now,
        );

        // If nothing would change visually and we already drew once, skip
        if !changed && !state.init {
            return Ok(false);
        }
        state.init = false;

        // layout
        let Size { width, height } = display.size();
        let (w, h) = (width as i32, height as i32);
        if w <= 6 || h <= 4 { return Ok(false); }

        let mx = 3;
        let my = 6;
        let title_base = 10;
        let inner_w = w - 2 * mx;
        let inner_h = h - my - title_base - 1;
        let title_pos = h - title_base;
        let pane_w = inner_w;

        // downmix (mono) panel
        draw_hist_panel_with_caps(
            display,
            "Downmix", title_base as u32, title_pos,
            Point::new(mx, my),
            Size::new(pane_w as u32, inner_h as u32),
            &state.draw_bands_m,
            &state.cap_m,
        ).map_err(|e| D::Error::from(e))?;

        Ok(true)

    }

    fn draw_aio_hist<D>(
        display: &mut D, 
        bands: Vec<u8>,
        state: &mut LastVizState
    ) -> Result<bool, D::Error> 
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        Ok(false)
    }

    pub async fn drain_frame_queue(
        &mut self
    ) -> Result<Option<VizFrameOut>, DisplayDrawingError> {
        let mut latest: Option<VizFrameOut> = None;
        if let Some(viz) = self.viz.as_mut() {
            loop {
                match viz.rx.try_recv() {
                    Ok(f) => latest = Some(f),  // keep latest
                    Err(TryRecvError::Empty) => break,       // nothing waiting (non-blocking)
                    Err(TryRecvError::Disconnected) => {
                        // upstream ended; return None or surface an error
                        break;
                    }
                }
            }
        }
        Ok(latest)
    }

    /// Updates and draws the visualization
    pub async fn update_and_draw_visualizer(&mut self) -> Result<(), DisplayDrawingError> {

        if self.viz_init_clear {
            self.display.clear_buffer();
            self.viz_init_clear = false;
        }
        let _ = self.drain_frame_queue().await.unwrap();
        // draw the active meter/visualization
        let mut need_flush = false;
        //let mut burst = 0;
        //loop {
            let frame = self.viz.as_mut().unwrap().rx.recv().await;
            //let frame = self.drain_frame_queue().await.unwrap();
            //burst += 1;

            //if burst > 120 || frame.is_none() {
            if !frame.is_none() {
        let mut state = self.last_viz_state.clone();
                let frame = frame.unwrap();
                // we can remove if viz is performative
                //if !frame.playing { // should we hold on pause??? would need decay peaks impl.
                //    return Ok(());
                //} else {
                    match frame.payload {
                        VizPayload::VuStereo { l_db, r_db } => 
                            need_flush = Self::draw_vu_pair(&mut self.display, l_db, r_db, true, &mut state)?,
                        VizPayload::VuStereoWithCenterPeak { l_db, r_db, peak_level, peak_hold } => 
                            need_flush = Self::draw_viz_combi(&mut self.display, l_db, r_db, peak_level, peak_hold, &mut state)?,
                        VizPayload::VuMono { db } => 
                            need_flush = Self::draw_vu_mono(&mut self.display, db, &mut state)?,
                        VizPayload::AioVuMono { db } => 
                            need_flush = Self::draw_aio_vu(&mut self.display, db, &mut state)?,
                        VizPayload::PeakStereo { l_level, r_level, l_hold, r_hold } => 
                            need_flush = Self::draw_peak_pair(&mut self.display, l_level, r_level, l_hold, r_hold, &mut state)?,
                        VizPayload::PeakMono { level, hold } => 
                            need_flush = Self::draw_peak_mono(&mut self.display, level, hold, &mut state)?,
                        VizPayload::HistStereo { bands_l, bands_r } => 
                            need_flush = Self::draw_hist_pair(&mut self.display, bands_l, bands_r, &mut state)?,
                        VizPayload::HistMono { bands } => 
                            need_flush = Self::draw_hist_mono(&mut self.display, bands, &mut state)?,
                        VizPayload::AioHistMono { bands } => 
                            need_flush = Self::draw_aio_hist(&mut self.display, bands, &mut state)?,
                        _ => {}
                    }
                    self.last_viz_state = state;
                //}
                if need_flush {
                    self.display.flush().unwrap();
                }
            }
//        }
        Ok(())

    }

    /// Updates and draws the weather data to display. Only flushes if changes occurred.
    pub async fn update_and_draw_weather(&mut self, show_current_weather: bool) -> Result<(), DisplayDrawingError> {

        let mut needs_flush = false;
        // Acquire a lock on the weather data
        let weather_data = if let Some(ref weather_arc) = self.weather_data_arc {
            weather_arc.lock().await
        } else {
            error!("Weather client not setup.");
            return Ok(()); // Nothing to draw if no weather client
        };

        let icon_w = 34;
        let temp_units = &weather_data.weather_data.temperature_units.clone();
        let wind_speed_units = &weather_data.weather_data.windspeed_units.clone();

        // Display current or forecast based on the flag
        if show_current_weather {

            // Display current weather
            let current = &weather_data.weather_data.current.clone();

            if *current != self.last_weather_draw_data[0] {

                self.display.clear(BinaryColor::Off).unwrap(); // Clear the screen completely for a new weather display
                self.last_weather_draw_data[0] = current.clone();

            let conditions = current.weather_code.description.clone();

            let curr_feels_temp = format!(
                "{:.0}({:.0}) {}",
                current.temperature_avg, current.temperature_apparent_avg, temp_units
            );
            let humidity = format!("{:.0}%", current.humidity_avg);
            let wind_dir = current.wind_direction.clone();
            let wind_speed = format!("{:.0} {} {}", current.wind_speed_avg, wind_speed_units, wind_dir);
            let pop =  format!("{}%", current.precipitation_probability_avg);
            let _icon_idx = current.weather_code.icon;
            let svg = current.weather_code.svg.clone();

            put_svg(
                &mut self.display,
                svg.as_str(), 12, 10, icon_w, icon_w)
                .await
                .unwrap();

            // Draw weather details
            let glyph_w = 12;
            let glyph_x = 52;
            let text_x = glyph_x as i32 + 2 + glyph_w;
            let mut text_y = 2;

            for (label, glyph_idx) in [(curr_feels_temp, 0), (humidity, 2), (wind_speed, 1), (pop, 3)] {
                let (height, font, ty) = if glyph_idx == 0 {
                    (glyph_w+2, &FONT_6X13_BOLD, text_y)} else {(glyph_w, &FONT_5X8, text_y-1)};
                let _glyph = ImageRaw::<BinaryColor>::new(
                    imgdata::get_glyph_slice(
                        climacell::THERMO_RAW_DATA, 
                        glyph_idx as usize, glyph_w as u32, glyph_w as u32),glyph_w as u32);
                Image::new(&_glyph, Point::new(glyph_x, ty))
                    .draw(&mut self.display).unwrap();
                draw_text_region_align(
                    &mut self.display,
                    label.as_str(), 
                    Point::new(text_x, text_y), Size::new(124-text_x as u32,height as u32),
                    HorizontalAlignment::Left, VerticalAlignment::Middle, 
                    font
                ).unwrap();
                text_y += if glyph_idx == 0 { 13 } else { 10 };
            }

            text_y += 1;
            draw_text_region_align(
                &mut self.display,
                conditions.as_str(), 
                Point::new(2, text_y), Size::new(constants::DISPLAY_WIDTH as u32 - 4, 14),
                HorizontalAlignment::Center, VerticalAlignment::Middle, 
                &FONT_7X14).unwrap();

            needs_flush = true;
            }

        } else {

            // Display 3-day forecast
            let forecasts = &weather_data.weather_data.forecast;
            if forecasts.len() > 0 {

                let fore0 = forecasts[0].clone();
                let fore1 = forecasts[1].clone();
                let fore2 = forecasts[2].clone();

            if fore0 != self.last_weather_draw_data[1] ||
                fore1 != self.last_weather_draw_data[2] ||
                fore2 != self.last_weather_draw_data[3]
            {

                self.last_weather_draw_data[1] = fore0;
                self.last_weather_draw_data[2] = fore1;
                self.last_weather_draw_data[3] = fore2;

                self.display.clear(BinaryColor::Off).unwrap(); // Clear the screen completely for a new weather display

                let mut icon_x = 7;
                for (_i, forecast) in forecasts.iter().enumerate() {

                    let mut day_y = 1;
                    let day_of_week = forecast.sunrise_time
                        .map_or("".to_string(), |dt| dt.with_timezone(&Local).format("~ %a ~").to_string());
                    let min_max_temp = format!(
                        "{:.0}{2}|{:.0}{2}",
                        forecast.temperature_min,
                        forecast.temperature_max,
                        temp_units
                    );
                    let pop =  format!("{}%", forecast.precipitation_probability_avg);

                    let svg = forecast.weather_code.svg.clone();
                    put_svg(
                        &mut self.display,
                        svg.as_str(), icon_x, day_y, icon_w-4, icon_w-4)
                        .await
                        .unwrap();

                    day_y += icon_w as i32 + 1;

                    draw_rectangle(
                        &mut self.display,
                        Point::new(icon_x-4, day_y-2),
                        icon_w + 6, 9,
                        BinaryColor::Off,
                        Some(1), Some(BinaryColor::On))
                    .map_err(|e| DisplayDrawingError::from(e))?;

                    // Draw Day of Week (left-aligned)
                    let day_width = Self::get_text_width_specific_font(&day_of_week, &FONT_4X6);
                    let day_x = icon_x + ((icon_w as i32 - day_width as i32) / 2);
                    draw_text(&mut self.display,&day_of_week, day_x, day_y, &FONT_4X6)?;

                    day_y += 9;
                    draw_rectangle(
                        &mut self.display,
                        Point::new(icon_x-4, day_y-3),
                        icon_w + 6, 18,
                        BinaryColor::Off,
                        Some(1), Some(BinaryColor::On))
                    .map_err(|e| DisplayDrawingError::from(e))?;

                    // Draw Min/Max Temp (right-aligned)
                    let temp_width = Self::get_text_width_specific_font(&min_max_temp, &FONT_4X6);
                    let temp_x = icon_x + ((icon_w as i32 - temp_width as i32) / 2);
                    draw_text(&mut self.display,&min_max_temp, temp_x, day_y, &FONT_4X6)?;

                    // and POP
                    day_y += 7;
                    let pop_width = Self::get_text_width_specific_font(&pop, &FONT_4X6);
                    let pop_x = icon_x + ((icon_w as i32 - pop_width as i32) / 2);
                    draw_text(&mut self.display,&pop, pop_x, day_y, &FONT_4X6)?;

                    icon_x += icon_w as i32 + 6; // next day forecast position

                }
                needs_flush = true;
            }
            }
        }

        if needs_flush {
            self.display.flush().unwrap();
        }
        Ok(())

    }

    /// Draws the configured Egg inclusive animation and track progress.
    async fn draw_egg(&mut self, current_track_time_secs: f32, remaining_time_secs: f32, duration_secs: f32, show_remaining: bool) -> Result<(), Box<dyn std::error::Error>> {
        let pct = current_track_time_secs/duration_secs;
        let tl = self.easter_egg.get_top_left();
        self.put_eggy_svg(
            self.artist.clone().as_str(),
            self.title.clone().as_str(),
            self.level,
            pct as f64,
            if show_remaining { remaining_time_secs } else { current_track_time_secs },
            tl.x,
            tl.y)
        .await?;
        // hand the text output here
        self.display.flush().unwrap();
        Ok(())
    }

    /// Renders a single frame of the display animation based on the current mode.
    ///
    /// This method either renders the scrolling LMS text or the large digital clock.
    pub async fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.current_mode {
            DisplayMode::Clock => {
                // When in clock mode, we pass the current local time to the clock drawing function.
                self.update_and_draw_clock(chrono::Local::now())?;
            },
            DisplayMode::EasterEggs => {
                self.draw_egg(
                    self.current_track_time_secs, 
                    self.remaining_time_secs, 
                    self.track_duration_secs,
                    self.show_remaining
                )
                .await?;
            },
            DisplayMode::WeatherCurrent => {
                // When in weather mode, drawing is self contained
                self.update_and_draw_weather(true).await?;
            },
            DisplayMode::Visualizer => {
                // When in weather mode, drawing is self contained
                self.update_and_draw_visualizer().await?;
            },
            DisplayMode::WeatherForecast => {
                // When in weather mode, drawing is self contained
                self.update_and_draw_weather(false).await?;
            },
            DisplayMode::Scrolling => {
                //self.clear(); // Clear the entire buffer for each frame of scrolling

                let mut needs_flush = false; // Track if anything changed to warrant a flush

                // --- Render Line 0 (Status Line) ---
                let mut current_x: i32 = constants::DISPLAY_REGION_X_OFFSET; // X position for drawing elements on line 0, offset by region start

                // 1. Volume Glyph and Text (Left justified)
                let vol_glyph = if self.is_muted || self.volume_percent == 0 {
                    &imgdata::GLYPH_VOLUME_OFF
                } else {
                    &imgdata::GLYPH_VOLUME_ON
                };
                self.draw_glyph(vol_glyph, current_x, constants::DISPLAY_REGION_Y_OFFSET)?;
                current_x += constants::GLYPH_WIDTH as i32; // Move X past the glyph

                let vol_text = if self.is_muted || self.volume_percent == 0 {
                    current_x += 3;
                    "mute".to_string() // Use spaces to clear previous volume % if muted
                } else {
                    format!("{:>3}%", self.volume_percent) // Right justified 3-digit number + '%'
                };
                
                draw_text(&mut self.display, &vol_text, current_x, constants::DISPLAY_REGION_Y_OFFSET, &FONT_5X8).unwrap();

                // 2. Shuffle Glyph
                let shuffle_glyph_data = if self.shuffle_mode == ShuffleMode::ByTracks {
                    Some(&imgdata::GLYPH_SHUFFLE_TRACKS)
                } else if self.shuffle_mode == ShuffleMode::ByAlbums {
                    Some(&imgdata::GLYPH_SHUFFLE_ALBUMS)
                } else {
                    Some(&imgdata::GLYPH_NONE)
                };

                // 3. Repeat Glyph
                let repeat_glyph_data = if self.repeat_mode == RepeatMode::RepeatOne {
                    Some(&imgdata::GLYPH_REPEAT_ONE)
                } else if self.repeat_mode == RepeatMode::RepeatAll {
                    Some(&imgdata::GLYPH_REPEAT_ALL)
                } else {
                    Some(&imgdata::GLYPH_NONE)
                };

                // 4. Bitrate Text and Audio Glyph (Right justified)
                let audio_glyph_data = match self.audio_bitrate {
                    AudioBitrate::HD => Some(&imgdata::GLYPH_AUDIO_HD),
                    AudioBitrate::SD => Some(&imgdata::GLYPH_AUDIO_SD),
                    AudioBitrate::DSD => Some(&imgdata::GLYPH_AUDIO_DSD),
                    AudioBitrate::None => Some(&imgdata::GLYPH_NONE),
                };
                
                let bitrate_text_width = self.get_text_width(&self.bitrate_text.clone()) as i32;
                let audio_glyph_full_width = if audio_glyph_data.is_some() { constants::GLYPH_WIDTH as i32 } else { 0 };

                // Calculate total width of right-justified elements (bitrate text + audio glyph)
                let total_right_elements_width = bitrate_text_width + audio_glyph_full_width;

                // Calculate starting X for right-justified block within the display region
                let mut right_block_x = constants::DISPLAY_REGION_X_OFFSET + constants::DISPLAY_REGION_WIDTH as i32 - total_right_elements_width;

                // Draw bitrate text
                draw_text(&mut self.display, &self.bitrate_text.clone(),right_block_x, constants::DISPLAY_REGION_Y_OFFSET, &FONT_5X8).unwrap();

                right_block_x += bitrate_text_width;

                // Draw audio glyph
                if let Some(glyph_data) = audio_glyph_data {
                    self.draw_glyph(glyph_data, right_block_x, constants::DISPLAY_REGION_Y_OFFSET)?;
                }

                if let Some(glyph_data) = shuffle_glyph_data {
                    let shuffle_x = 44; // left_most_occupied_x;
                    self.draw_glyph(glyph_data, shuffle_x, constants::DISPLAY_REGION_Y_OFFSET)?;
                }

                if let Some(glyph_data) = repeat_glyph_data {
                    let repeat_x = 34; // left_most_occupied_x + constants::GLYPH_WIDTH as i32 + 4;
                    self.draw_glyph(glyph_data, repeat_x, constants::DISPLAY_REGION_Y_OFFSET)?;
                }

                // --- Player Track Progress Bar ---
                let player_progress_bar_x = constants::DISPLAY_REGION_X_OFFSET;
                let player_progress_bar_y = constants::PLAYER_PROGRESS_BAR_Y_POS;

                let progress_bar_changed = self.current_track_time_secs != self.last_current_track_time_secs ||
                                          self.track_duration_secs != self.last_track_duration_secs;

                if progress_bar_changed {
                    // draw progress bar
                    draw_rectangle(
                        &mut self.display,
                        Point::new(player_progress_bar_x, player_progress_bar_y),
                        constants::PLAYER_PROGRESS_BAR_WIDTH, constants::PLAYER_PROGRESS_BAR_HEIGHT,
                        BinaryColor::Off,
                        Some(constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS), 
                        Some(BinaryColor::On)
                    )
                    .map_err(|e| DisplayDrawingError::from(e))?;
        
                    // Calculate the filled width
                    let fill_width_pixels = if self.track_duration_secs > 0.00 {
                        (constants::PLAYER_PROGRESS_BAR_WIDTH as f32 * (self.current_track_time_secs as f32 / self.track_duration_secs as f32))
                        .round() as u32
                    } else {
                        0
                    };

                    // The actual width of the inner filled bar, considering the border.
                    let inner_fill_width = (fill_width_pixels as i32 - (2 * constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS as i32)).max(0);
                    let inner_height = constants::PLAYER_PROGRESS_BAR_HEIGHT - (2 * constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS);

                    // Draw the filled part if there's actual fill to show
                    if inner_fill_width > 0 {
                        draw_rectangle(
                            &mut self.display,
                            Point::new(
                                player_progress_bar_x+ constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS as i32, 
                                player_progress_bar_y+ constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS as i32),
                            inner_fill_width as u32, inner_height,
                            BinaryColor::On,
                            None, None
                        )
                        .map_err(|e| DisplayDrawingError::from(e))?;
                    }
                    needs_flush = true;
                }

                // Iterate through scrollers and draw their content if changed
                for scroller in &mut self.scrollers { // Use `&mut self.scrollers` to get mutable access
                    let mut scroller_state = scroller.state.lock().await; // Lock scroller's internal state
                    let current_text = scroller_state.text.clone();
                    let current_font = scroller_state.font.clone();
                    let current_mode = scroller_state.scroll_mode;
                    let text_width = scroller_state.text_width; // Get text_width from scroller's state
                    let text_height = current_font.character_size.height;

                    let top_left = scroller.top_left; 
                    let x_start = top_left.x;
                    let y_start = top_left.y;

                    // Define a rectangle representing the region you want to draw inside
                    let region = Rectangle::new(top_left, Size::new(constants::DISPLAY_WIDTH, text_height as u32)); // (x, y), (width, height)

                    let current_x_rounded_from_scroller = (scroller_state.current_offset_float).round() as i32;
        
                    // Clear the entire region for this scroller before redrawing
                    clear_region(&mut self.display, region).unwrap();
                    // Draw main text
                    let draw_x_main = x_start + current_x_rounded_from_scroller;
                    draw_text(&mut self.display, &current_text, draw_x_main, y_start, &FONT_5X8).unwrap();

                    // For continuous loop, draw a second copy if needed
                    if current_mode == ScrollMode::ScrollLeft {
                        let second_copy_x = draw_x_main + text_width as i32 + GAP_BETWEEN_LOOP_TEXT_FIXED;
                        draw_text(&mut self.display, &current_text, second_copy_x, y_start, &FONT_5X8).unwrap();
                    }

                    // Update OledDisplay's record of what was last drawn
                    scroller_state.last_drawn_x_rounded = current_x_rounded_from_scroller;
                    needs_flush = true;

                }

                // --- New Track Info Line (Current Time | Mode | Remaining Time) ---
                let info_line_y = constants::PLAYER_TRACK_INFO_LINE_Y_POS;
                let current_time_str = seconds_to_hms(self.current_track_time_secs);
                let remaining_time_str = format!("-{}", seconds_to_hms(self.remaining_time_secs));
                let total_time_str = format!(" {}", seconds_to_hms(self.track_duration_secs));
                let mode_text = self.mode_text.clone();

                let info_line_changed = self.last_current_track_time_secs != self.current_track_time_secs ||
                                        self.last_remaining_time_secs != self.remaining_time_secs ||
                                        self.last_mode_text != self.mode_text;

                if info_line_changed {

                    // Clear the entire info line area
                    draw_rectangle(
                        &mut self.display,
                        Point::new(constants::DISPLAY_REGION_X_OFFSET, info_line_y),
                        constants::DISPLAY_WIDTH, constants::MAIN_FONT_HEIGHT,
                        BinaryColor::Off,None, None
                    )
                    .map_err(|e| DisplayDrawingError::from(e))?;

                    draw_text(&mut self.display, &current_time_str,constants::DISPLAY_REGION_X_OFFSET, info_line_y, &FONT_5X8).unwrap();

                    // Draw mode text (centered)
                    let mode_text_width = self.get_text_width(&mode_text) as i32;
                    let mode_text_x = constants::DISPLAY_REGION_X_OFFSET + ((constants::DISPLAY_REGION_WIDTH as i32 - mode_text_width) / 2);
                    draw_text(&mut self.display, &self.mode_text.clone(),mode_text_x, info_line_y, &FONT_5X8).unwrap();

                    // Draw total or remaining time (right-justified)
                    let rt_time_width = self.get_text_width(&remaining_time_str) as i32;
                    let rt_time_x = constants::DISPLAY_REGION_X_OFFSET + constants::DISPLAY_REGION_WIDTH as i32 - rt_time_width;
                    if self.show_remaining {
                        draw_text(&mut self.display, &remaining_time_str,rt_time_x-3, info_line_y, &FONT_5X8).unwrap();
                    } else {
                        draw_text(&mut self.display, &total_time_str,rt_time_x-3, info_line_y, &FONT_5X8).unwrap();
                    }
                    self.last_current_track_time_secs = self.current_track_time_secs;
                    self.last_track_duration_secs = self.track_duration_secs;
                    self.last_remaining_time_secs = self.remaining_time_secs;
                    self.last_mode_text = self.mode_text.clone();
                    needs_flush = true;

                }
                
                // Only flush if any drawing operation in this frame necessitated it
                if needs_flush {
                    self.display.flush().unwrap();
                }
                // drain chatter from vizualizer
                let _ = self.drain_frame_queue().await.unwrap();
            }
        }
        Ok(())

    }

}



// Implement Drop trait to stop the background thread when OledDisplay goes out of scope
impl Drop for OledDisplay {
    fn drop(&mut self) {
        self.clear();
        self.display.flush().unwrap();
        info!("OledDisplay dropped. Attempting to stop scrolling thread...");
        for scroller in &mut self.scrollers {
            scroller.stop();
        }
        // Note: Joining the thread here (self.poll_handle.take().unwrap().await)
        // would require this Drop impl to be async or to block on a runtime,
        // which is generally discouraged in Drop implementations.
        // For graceful shutdown, a dedicated async `shutdown` method is usually preferred.
        // Here, we just send the signal and let the runtime clean up the detached task.
    }
}

