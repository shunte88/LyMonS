use chrono::Timelike;

use embedded_graphics::{
    image::{Image, ImageRaw},
    mono_font::{
        ascii::{
            FONT_5X8, 
            FONT_6X10, 
            FONT_6X13_BOLD}, 
        MonoFont, 
        MonoTextStyle, 
        MonoTextStyleBuilder
    }, 
    pixelcolor::{BinaryColor}, 
    prelude::*, 
    primitives::{PrimitiveStyleBuilder, Rectangle}, 
    text::{renderer::TextRenderer, Baseline, Text}
};
use linux_embedded_hal::I2cdev;
use ssd1306::{
    mode::BufferedGraphicsMode,
    prelude::*,
    size::DisplaySize128x64,
    I2CDisplayInterface,
    Ssd1306, 
};

use log::{info, error, debug};
use std::time::{Instant, Duration};
use std::error::Error; // Import the Error trait
use std::fmt; // Import fmt for Display trait
use std::thread::sleep;
//use binary_raster::BinaryRaster;

use display_interface::DisplayError;
use fontdue::{Font, layout::{Layout, TextStyle, CoordinateSystem}};

use crate::{imgdata}; // Import imgdata, glyphs and such
use crate::constants; // Import the new constants module
use crate::clock_font::ClockFontData; // Import the new ClockFontData struct
use crate::deutils::seconds_to_hms;

#[derive(Debug, PartialEq, Clone)]
enum ScrollState {
    Static,           // Text fits, no scrolling
    ScrollIn,         // Text is scrolling from right to left, entering the screen
    PausedAtLeft,     // Text has scrolled in and is paused at the left edge (x=0)
    ContinuousLoop,   // Text continuously scrolls left, wrapping seamlessly from right
    CylonLoop,        // Text scrolls back and forth (ping-pong)
}

#[derive(Debug, PartialEq, Clone)]
pub enum ScrollType {
    Static,
    Looping,
    Cylon,
}

/// Represents the audio bitrate mode for displaying the correct glyph.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AudioBitrate {
    HD,
    SD,
    DSD,
    None, // No specific audio bitrate glyph displayed
}

/// Represents the repeat mode for displaying the correct glyph.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RepeatMode {
    Off,
    RepeatAll,
    RepeatOne,
}

/// Represents the shuffle mode for displaying the correct glyph.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ShuffleMode {
    Off,
    ByTracks,
    ByAlbums,
}

/// NEW: Enum to define the current display mode (Scrolling text or Clock).
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum DisplayMode {
    #[allow(dead_code)]
    VUMeters,       // TBD - a stereo pair of VU meters - device scaled
    #[allow(dead_code)]
    VUDownmix,      // TBD - a VU downmix meter - single VU
    #[allow(dead_code)]
    Histograms,     // TBD - a stereo pair of histograms
    #[allow(dead_code)]
    HistoDownmix,   // TBD - a histogram downmix - single histogram
    #[allow(dead_code)]
    EasterEggs,     // TBD - our world famous easter eggs
    Scrolling,      // Done - our world famous Now Playing mode
    Clock,          // Done - our world famous Clock mode
}

#[derive(Debug, Clone)]
struct LineDisplayState {
    content: String,
    current_x_offset_float: f32,
    state: ScrollState,
    scroll_type: ScrollType,
    last_update_time: Instant,
    original_width: u32,
    last_displayed_content: String,
}

impl Default for LineDisplayState {
    fn default() -> Self {
        LineDisplayState {
            content: String::new(),
            current_x_offset_float: 0.00,
            state: ScrollState::Static,
            scroll_type: ScrollType::Static,
            last_update_time: Instant::now(),
            original_width: 0,
            last_displayed_content: String::new(),
        }
    }
}

/// Custom error type for drawing operations that implements `std::error::Error`.
#[derive(Debug)]
pub enum DisplayDrawingError {
    /// An error originating from the `display-interface` crate.
    DrawingFailed(DisplayError),
    /// A generic string error for other display-related failures.
    Other(String),
}

impl fmt::Display for DisplayDrawingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayDrawingError::DrawingFailed(e) => write!(f, "Display drawing error: {:?}", e),
            DisplayDrawingError::Other(msg) => write!(f, "Display error: {}", msg),
        }
    }
}

impl Error for DisplayDrawingError {}

// Implement `From` for `display_interface::DisplayError` to automatically convert it
impl From<DisplayError> for DisplayDrawingError {
    fn from(err: DisplayError) -> Self {
        DisplayDrawingError::DrawingFailed(err)
    }
}

pub struct OledDisplay {
    // this definition is 100% correct - DO NOT MODIFY
    display: Ssd1306<I2CInterface<I2cdev>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>,
    // this definition is 100% correct - DO NOT MODIFY
    line_states: Vec<LineDisplayState>,

    custom_font: Option<Font>,
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
    last_second_drawn: u8, // Store the last second drawn for progress bar updates
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

}

impl OledDisplay {

    /// Initializes the OLED display over I2C.
    ///
    /// `i2c_bus_path` is typically "/dev/i2c-X" where X is the bus number (e.g., "/dev/i2c-1").
    pub fn new(i2c_bus_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing OLED display on {}", i2c_bus_path);

        let i2c = I2cdev::new(i2c_bus_path)?;
        let interface = I2CDisplayInterface::new(i2c);

        let mut display = Ssd1306::new(
            interface,
            DisplaySize128x64,
            DisplayRotation::Rotate0,
        ).into_buffered_graphics_mode();

        display.init().map_err(|e| format!("Display init error: {:?}", e))?;
        display.clear_buffer();
        display.flush().map_err(|e| format!("Display flush error: {:?}", e))?;

        info!("OledDisplay initialized successfully.");

        let font_bytes = include_bytes!("../fonts/cherry-mono.ttf") as &[u8];
        let custom_font = match Font::from_bytes(font_bytes, fontdue::FontSettings::default()) {
            Ok(font) => {
                info!("Custom font loaded successfully.");
                Some(font)
            },
            Err(e) => {
                error!("Failed to load custom TrueType font (fontdue): {}. Falling back to FONT_5X8.", e);
                None
            }
        };

        let default_mono_style = MonoTextStyleBuilder::new()
            .font(&FONT_5X8)
            .text_color(BinaryColor::On)
            .build();

        Ok(OledDisplay {
            display,
            line_states: vec![LineDisplayState::default(); constants::MAX_LINES],
            custom_font,
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
            clock_font: imgdata::new_clock_font(imgdata::SEVENSEG_CLOCK_DIGITS_RAW_DATA), // Initialize with the 7-segment font
            last_second_drawn: 61, // Initialize to an invalid second to force first draw
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
            scroll_mode: "scroll".to_string(),
        })
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
    
    /// Displays a splash screen image and fades the brightness in.
    /// The splash image is the LyMonS logo, version and build date
    pub fn splash(&mut self, 
        show_splash: bool,         
        version: &str,
        build_date: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.display.clear_buffer();
        if show_splash {
   
            // 1. Set brightness to zero
            let mut contrast:u8 = 0;
            self.set_brightness(contrast)?;

            let splash_x:i32 = (constants::DISPLAY_WIDTH as i32-constants::LYMONS_LOGO_WIDTH as i32)/2;
            
            let raw_image = ImageRaw::<BinaryColor>::new(imgdata::LYMONS_IMAGE_RAW_DATA, constants::LYMONS_LOGO_WIDTH);
            Image::new(&raw_image, Point::new(splash_x, 1))
                .draw(&mut self.display)
                .map_err(|e| Box::new(DisplayDrawingError::from(e)) as Box<dyn std::error::Error>)?;

            let mut x = (constants::DISPLAY_WIDTH - (6*version.chars().count() as u32)) / 2;
            self.draw_text(version, x as i32, constants::PLAYER_TRACK_INFO_LINE_Y_POS-17,Some(&FONT_6X13_BOLD)).unwrap();
            x = (constants::DISPLAY_WIDTH - (5*build_date.chars().count() as u32)) / 2;
            self.draw_text(build_date, x as i32, constants::PLAYER_TRACK_INFO_LINE_Y_POS,Some(&FONT_5X8)).unwrap();
    
            self.flush()?; // Flush to display the black image at zero brightness

            const FADE_DURATION_MS: u64 = 3500;
            const FADE_STEPS: u8 = 60; // More steps for smoother fade
            let step_delay = Duration::from_millis(FADE_DURATION_MS / FADE_STEPS as u64);

            for i in 1..FADE_STEPS {
                contrast = (255.0 / FADE_STEPS as f32 * i as f32).round() as u8;
                self.set_brightness(contrast)?;
                sleep(step_delay);
            }

            // Ensure full brightness at the end
            self.set_brightness(255)?;

        } else {

            self.flush()?;
        
        }
        Ok(())

    }

    /// Flushes the buffer to the display, making changes visible.
    pub fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.display.flush().map_err(|e| format!("Display flush error: {:?}", e))?;
        Ok(())
    }

    pub fn set_scroll_mode(&mut self, mode: &String) {
        if mode.to_string() != self.scroll_mode {
            self.scroll_mode = mode.to_string();
        }
    }

    /// Calculates the width of the given text in pixels using either the custom font or the default.
    fn get_text_width(&self, text: &str) -> u32 {
        if let Some(ref font) = self.custom_font {
            // Use fontdue's layout for accurate text measurement
            let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
            layout.append(&[font], &TextStyle::new(text, constants::MAIN_FONT_HEIGHT as f32, 0));
            // The total width is the max x-coordinate of all glyphs + their width.
            let mut max_x:f32 = 0.0;
            for glyph in layout.glyphs() {
                max_x = max_x.max(glyph.x + glyph.width as f32);
            }
            max_x.round() as u32
        } else {
            // Fallback to default mono font's measurement
            self.default_mono_style.measure_string(text, Point::new(0, 0), Baseline::Top).bounding_box.size.width
        }
    }

    fn draw_text(&mut self, text: &str, x: i32, y: i32, font:Option<&MonoFont>) -> Result<(), Box<dyn std::error::Error>> {
        let style = if font.is_none() {
            self.default_mono_style.clone()
        } else {
            MonoTextStyleBuilder::new().font(font.unwrap()).text_color(BinaryColor::On).build()
        };
        Text::with_baseline(
            text,
            Point::new(x, y),
            style,
            Baseline::Top,

        )
        .draw(&mut self.display)
        .map_err(|e| Box::new(DisplayDrawingError::from(e)) as Box<dyn std::error::Error>)?;
        Ok(())
    }

    /// Helper function to draw text using the custom TrueType font if loaded,
    /// otherwise falls back to the default FONT_5X8.
    fn draw_text_with_custom_font(
        &mut self,
        text: &str,
        top_left: Point,
        _baseline: Baseline,
    ) -> Result<(), Box<dyn std::error::Error>> {
        
        if let Some(ref font) = self.custom_font {

            let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
            layout.append(&[font], &TextStyle::new(text, constants::MAIN_FONT_HEIGHT as f32, 0));
            
            /* 
            let mut total_width:usize = 0;
            let mut total_height:usize = 10; // fixed height, will get maxima if exceeded
            for glyph in layout.glyphs() {
                total_width = total_width.max(glyph.x as usize+glyph.width);
                total_height = total_height.max(glyph.y as usize+glyph.height);
            }

            // should adjust here for displayable bounds, avoid pixel wrapping
            let image_data = vec![0u8; total_width * total_height];
            let mut image = ImageRaw::<Rgb565>::new(&image_data, total_width);

            for glyph in layout.glyphs() {
                let (metrics, bitmap) = font.rasterize(glyph.parent, constants::MAIN_FONT_HEIGHT as f32);
                for (x, y, v) in bitmap.iter_with_coords() {
                    let px = glyph.x as usize + x;
                    let py = glyph.y as usize + y;
        
                    if px < total_width && py < total_height {
                        image(px as u32, py as u32, Rgb565([255, 255, 255, (v * 255.0) as u8]));
                    }
                }
            }
            */
        
            self.draw_text(text,top_left.x, top_left.y, None)?;
 
        } else {
            self.draw_text(text,top_left.x, top_left.y, None)?;
        }
        Ok(())

    }
    
    /// Sets the current display mode (e.g., Clock or Scrolling).
    pub fn set_display_mode(&mut self, mode: DisplayMode) {
        if self.current_mode != mode {
            info!("Changing display mode to {:?}", mode);
            self.current_mode = mode;
            // Clear the buffer when changing modes to avoid visual artifacts
            self.clear();
            let _ = self.flush(); // Attempt to flush, ignore error for mode change

            // Reset clock digits so it redraws everything when switching to clock mode
            // This ensures a clean display of the clock digits initially.
            if mode == DisplayMode::Clock {
                self.last_clock_digits = [' ', ' ', ' ', ' ', ' '];
            }
            self.last_second_drawn = 61; // Reset last second to force progress bar redraw
            self.last_date_drawn = String::new(); // Reset last drawn date to force redraw

            // Reset player display fields when switching to scrolling mode for fresh draw
            if mode == DisplayMode::Scrolling {
                self.last_track_duration_secs = 0.00; // Forces redraw
                self.last_current_track_time_secs = 0.00; // Forces redraw
                self.last_remaining_time_secs  = 0.00; }
                self.last_mode_text = String::new(); // Forces redraw
            }
        }
    
    /// Sets the clock display font.
    pub fn set_clock_font(&mut self, font_name: &str) {
        self.clock_font =
        match font_name {
            "space1999" => {
                imgdata::new_clock_font(imgdata::SPACE_1999_CLOCK_DIGIT_RAW_DATA)
                },
            "holfestus" => {
                imgdata::new_clock_font(imgdata::HOLLOW_FESTUS_CLOCK_DIGIT_RAW_DATA)
                },
            "solfestus" => {
                imgdata::new_clock_font(imgdata::SOLID_FESTUS_CLOCK_DIGIT_RAW_DATA)
                },
            "holdeco" => {
                imgdata::new_clock_font(imgdata::HOLLOW_DECO_CLOCK_DIGIT_RAW_DATA)
                },
            "soldeco" => {
                imgdata::new_clock_font(imgdata::SOLID_DECO_CLOCK_DIGIT_RAW_DATA)
                },
            "noto" => {
                imgdata::new_clock_font(imgdata::NOTO_CLOCK_DIGIT_RAW_DATA)
                },
            "roboto" => {
                imgdata::new_clock_font(imgdata::ROBOTO_CLOCK_DIGIT_RAW_DATA)
                },
            "7seg" => {
                imgdata::new_clock_font(imgdata::SEVENSEG_CLOCK_DIGITS_RAW_DATA)
            },
            _ => {
                imgdata::new_clock_font(imgdata::SEVENSEG_CLOCK_DIGITS_RAW_DATA)
            }
        }

    }

    /// Helper to draw an 8x8 glyph from raw byte data.
    fn draw_glyph(&mut self, data: &'static [u8; 8], x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
        let raw_image = ImageRaw::<BinaryColor>::new(data, constants::GLYPH_WIDTH);
        Image::new(&raw_image, Point::new(x, y))
            .draw(&mut self.display)
            .map_err(|e| Box::new(DisplayDrawingError::from(e)) as Box<dyn std::error::Error>)
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

    pub fn draw_rectangle(&mut self,top_left: Point,w:u32, h:u32,fill:BinaryColor, border_width:Option<u32>, border_color:Option<BinaryColor>) -> Result<(), DisplayDrawingError> {    
        Rectangle::new(top_left,
            Size::new(w, h))
            .into_styled(
                PrimitiveStyleBuilder::new()
                .stroke_color(if border_width.is_some() { border_color.unwrap() } else {BinaryColor::Off})
                .stroke_width(if border_width.is_some() { border_width.unwrap() } else {0})
                .fill_color(fill)
                .build(),
            )
            .draw(&mut self.display)
            .map_err(DisplayDrawingError::DrawingFailed)?;
        Ok(())
    }

    pub fn set_status_line_data(&mut self, volume_percent: u8, is_muted: bool, samplesize: String, samplerate: String, repeat_mode: RepeatMode, shuffle_mode: ShuffleMode)
    {
        let changed = self.volume_percent != volume_percent ||
                      self.is_muted != is_muted ||
                      self.repeat_mode != repeat_mode ||
                      self.shuffle_mode != shuffle_mode ||
                      self.samplerate != samplerate ||
                      self.samplesize != samplesize;
                                
        if changed {

            self.volume_percent = volume_percent;
            self.is_muted = is_muted;
            self.repeat_mode = repeat_mode;
            self.shuffle_mode = shuffle_mode;
            
            let samp_size = samplesize.parse::<u32>().unwrap_or(0);
            let samp_rate = samplerate.parse::<u32>().unwrap_or(0);

            self.bitrate_text = if samp_size == 1 { // DSD/DSF 1-bit
                format!("DSD{} ", 
                    (samp_rate / 44100 as u32))
            } else { // vanilla, e.g. 24/96 etc
                format!("{}/{} ", 
                    samplesize, 
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
        }

    }

    /// Sets the content for a specific line (excluding line 0 and line 5).
    /// If the content changes, it resets the line's scroll state to initiate scrolling.
    ///
    /// `line_num` is 0-indexed. This should be 1-4 for scrolling content.
    /// `new_content` is the string to display.
    /// `scroll_type` specifies the continuous scroll behavior for this line if it overflows.
    pub fn set_line_content(&mut self, line_num: usize, new_content: String, scroll_type: ScrollType) {
        // Ensure line_num is within the scrolling content range (1 to 4)
        if line_num == 0 || line_num >= constants::MAX_LINES -1 { // MAX_LINES - 1 is the last content line (line 5 / index 5)
            error!("Attempted to set content for line {} which is not a scrolling content line (valid lines 1-{}).", line_num, constants::MAX_LINES - 2);
            return;
        }

        let calculated_original_width = self.get_text_width(&new_content);
        let line_state = &mut self.line_states[line_num];

        if line_state.last_displayed_content != new_content {
            info!("Line {}: Content changed from '{}' to '{}'", line_num, line_state.last_displayed_content, new_content);
            line_state.content = new_content.clone();
            line_state.last_displayed_content = new_content;
            line_state.original_width = calculated_original_width;
            line_state.scroll_type = scroll_type;

            if line_state.original_width > constants::DISPLAY_REGION_WIDTH {
                line_state.state = ScrollState::ScrollIn;
                line_state.current_x_offset_float = constants::DISPLAY_REGION_WIDTH as f32; // Start off-screen right of region as f32
            } else {
                line_state.state = ScrollState::Static;
                line_state.scroll_type = ScrollType::Static; // Ensure static if it now fits
                // Center the text horizontally within the display region
                line_state.current_x_offset_float = ((constants::DISPLAY_REGION_WIDTH - line_state.original_width) / 2) as f32; // Center as f32
            }
            line_state.last_update_time = Instant::now();
        }

    }

    /// Sets the content for each line. `set_line_content` internally handles
    /// if the content has changed and resets scroll state if needed.
    // this impl. ignores the changed flags on the tags - rethink passing a
    // tag reference and utilizing the baked in functionality - though KISS works too
    pub fn set_track_details(&mut self, albumartist: String, album: String, title: String, artist: String) {
 
        let mode:ScrollType = if self.scroll_mode == "cylon" {
            ScrollType::Cylon
        } else {
            ScrollType::Looping
        };
        self.set_line_content(constants::TAG_DISPLAY_ALBUMARTIST, albumartist, mode.clone());
        self.set_line_content(constants::TAG_DISPLAY_ALBUM, album, mode.clone());
        self.set_line_content(constants::TAG_DISPLAY_TITLE, title, mode.clone());
        self.set_line_content(constants::TAG_DISPLAY_ARTIST, artist, mode.clone());
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
    pub fn update_and_draw_clock(&mut self, current_time: chrono::NaiveTime) -> Result<(), Box<dyn std::error::Error>> {
        let mut needs_flush = false;
        // No longer clear the entire buffer for each clock update to maintain persistence.

        let current_second = current_time.second(); // Renamed to avoid conflict with existing `current_seconds`

        // Determine colon state for blinking (on for even seconds, off for odd)
        let new_colon_on_state = current_second % 2 == 0;

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
                self.draw_rectangle(
                    Point::new(x_offset, y_offset),
                    self.clock_font.digit_width, self.clock_font.digit_height,
                    BinaryColor::Off,
                None, None)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                // and draw the clock character
                self.draw_custom_clock_char(current_char_for_position, x_offset, y_offset)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                
                self.last_clock_digits[i] = current_char_for_position;
                needs_flush = true;
            }
        }
        
        // Update the stored colon state *after* the loop, as it's used for comparison next iteration.
        self.colon_on = new_colon_on_state; 

        // --- Seconds Progress Bar ---
        let progress_bar_width_total = constants::DISPLAY_WIDTH as i32 - 4; // Display width minus 2px padding on each side
        let progress_bar_x = (constants::DISPLAY_WIDTH as i32 - progress_bar_width_total) / 2;

        if current_second != self.last_second_drawn as u32 {

            self.draw_rectangle(
                Point::new(progress_bar_x, progress_bar_y),
                progress_bar_width_total as u32, progress_bar_height,
                BinaryColor::Off,
                Some(border_thickness as u32),
                Some(BinaryColor::On)
            )
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            // Calculate the filled width based on seconds (0 to 59)
            // Maps 0-59 seconds to a fill ratio from 0.0 to 1.0
            let fill_ratio = current_second as f32 / 59.0; 
            let filled_width_pixel_count = (progress_bar_width_total as f32 * fill_ratio).round() as i32;

            // The actual width of the inner filled bar, considering the border.
            let inner_fill_width = (filled_width_pixel_count - (2 * border_thickness)).max(0);
            let inner_height = progress_bar_height - (2 * border_thickness as u32);

            // Draw the filled part of the progress bar if there's actual fill to show
            if inner_fill_width > 0 {
                self.draw_rectangle(
                    Point::new(progress_bar_x+ border_thickness, progress_bar_y+ border_thickness),
                    inner_fill_width as u32, inner_height,
                    BinaryColor::On,None, None
                )
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            }
            self.last_second_drawn = current_second as u8;
            needs_flush = true; // Mark for flush if progress bar updated
        }

        // --- Date Drawing ---
        let current_date_string = chrono::Local::now().format("%a %b %d").to_string(); // e.g., "Mon Jun 09"
        let date_text_width = self.get_text_width(&current_date_string);
        let date_x_pos = (constants::DISPLAY_WIDTH as i32 - date_text_width as i32) / 2;

        if current_date_string != self.last_date_drawn {
            self.draw_rectangle(
                Point::new(0, date_y),
                constants::DISPLAY_WIDTH, constants::DATE_FONT_HEIGHT,
                BinaryColor::Off,None, None
            )
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            self.draw_text(&current_date_string,date_x_pos-4, date_y, Some(&FONT_6X10))?;

            self.last_date_drawn = current_date_string;
            needs_flush = true;
        }

        if needs_flush {
            self.flush()?;
        }
        Ok(())

    }

    /// Renders a single frame of the display animation based on the current mode.
    ///
    /// This method either renders the scrolling LMS text or the large digital clock.
    pub fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.current_mode {
            DisplayMode::VUMeters => {
                // Stero VU meters - needs audio metric data - FFT
                self.clear(); // Clear for now
                debug!("VUMeters functionality TBD.");
            },
            DisplayMode::VUDownmix => {
                // single downmix (mono) VU meter - needs audio metric data - FFT
                self.clear();
                debug!("VUDownmix functionality TBD.");
            },
            DisplayMode::Histograms => {
                // stereo histograms - needs audio metric data - FFT
                self.clear();
                debug!("Histograms functionality TBD.");
            },
            DisplayMode::HistoDownmix => {
                // downmixed (mono) histogram - needs audio metric data - FFT
                self.clear();
                debug!("HistoDownmix functionality TBD.");
            },
            DisplayMode::EasterEggs => {
                // this is just some fun animations
                self.clear();
                debug!("EasterEggs functionality TBD.");
            },
            DisplayMode::Clock => {
                // When in clock mode, we pass the current local time to the clock drawing function.
                let now_chrono = chrono::Local::now().time();
                self.update_and_draw_clock(now_chrono)?;
            },
            DisplayMode::Scrolling => {
                self.clear(); // Clear the entire buffer for each frame of scrolling

                let now = Instant::now();
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
                
                self.draw_text(&vol_text, current_x, constants::DISPLAY_REGION_Y_OFFSET, None)?;

                // 2. Shuffle Glyph
                let shuffle_glyph_data = if self.shuffle_mode == ShuffleMode::ByTracks {
                    Some(&imgdata::GLYPH_SHUFFLE_TRACKS)
                } else if self.shuffle_mode == ShuffleMode::ByAlbums {
                    Some(&imgdata::GLYPH_SHUFFLE_ALBUMS)
                } else {
                    None
                };

                // 3. Repeat Glyph
                let repeat_glyph_data = if self.repeat_mode == RepeatMode::RepeatOne {
                    Some(&imgdata::GLYPH_REPEAT_ONE)
                } else if self.repeat_mode == RepeatMode::RepeatAll {
                    Some(&imgdata::GLYPH_REPEAT_ALL)
                } else {
                    None
                };

                // 4. Bitrate Text and Audio Glyph (Right justified)
                let audio_glyph_data = match self.audio_bitrate {
                    AudioBitrate::HD => Some(&imgdata::GLYPH_AUDIO_HD),
                    AudioBitrate::SD => Some(&imgdata::GLYPH_AUDIO_SD),
                    AudioBitrate::DSD => Some(&imgdata::GLYPH_AUDIO_DSD),
                    AudioBitrate::None => None,
                };
                
                let bitrate_text_width = self.get_text_width(&self.bitrate_text) as i32;
                let audio_glyph_full_width = if audio_glyph_data.is_some() { constants::GLYPH_WIDTH as i32 } else { 0 };

                // Calculate total width of right-justified elements (bitrate text + audio glyph)
                let total_right_elements_width = bitrate_text_width + audio_glyph_full_width;

                // Calculate starting X for right-justified block within the display region
                let mut right_block_x = constants::DISPLAY_REGION_X_OFFSET + constants::DISPLAY_REGION_WIDTH as i32 - total_right_elements_width;

                // Draw bitrate text
                self.draw_text(&self.bitrate_text.clone(),right_block_x, constants::DISPLAY_REGION_Y_OFFSET, None)?;

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

                // --- Render Scrolling Lines (Lines 1 to 4) ---
                // Start from line_num 1. Line 0 is status. Line 5 is track info.
                for line_num in 1..(constants::MAX_LINES - 1) { // Iterate for lines 1, 2, 3, 4
                    let line_state = &mut self.line_states[line_num].clone();
                    // Calculate Y position for each line, accounting for line 0's height and region offset.
                    let y_pos = constants::DISPLAY_REGION_Y_OFFSET + (constants::MAIN_FONT_HEIGHT as i32 + constants::MAIN_LINE_SPACING) * (line_num as i32);
                    let elapsed_time_secs = now.duration_since(line_state.last_update_time).as_secs_f32();

                    match line_state.state {
                        ScrollState::Static => {
                            // No change in offset needed. Text stays centered if it fits,
                            // or left-justified if it fills the width.
                        }
                        ScrollState::ScrollIn => {
                            let scroll_amount = constants::SCROLL_SPEED_PIXELS_PER_SEC * elapsed_time_secs;
                            line_state.current_x_offset_float = constants::DISPLAY_REGION_WIDTH as f32 - scroll_amount;

                            //info!("{:.3} e-secs {:.3}, {:.3}", elapsed_time_secs, scroll_amount, line_state.current_x_offset_float);

                            if line_state.current_x_offset_float <= 0.0 { // Compare with 0.0 for f32
                                line_state.current_x_offset_float = 0.0;
                                line_state.state = ScrollState::PausedAtLeft;
                                line_state.last_update_time = now; // Reset timer for pause duration
                            }
                        }
                        ScrollState::PausedAtLeft => {
                            if now.duration_since(line_state.last_update_time) >= Duration::from_millis(constants::PAUSE_DURATION_MS) {
                                line_state.state = match line_state.scroll_type {
                                    ScrollType::Static => ScrollState::Static,
                                    ScrollType::Looping => ScrollState::ContinuousLoop,
                                    ScrollType::Cylon => ScrollState::CylonLoop,
                                };
                                // When transitioning from PausedAtLeft, the new state starts its timer from now.
                                line_state.last_update_time = now; 
                            }
                        }
                        ScrollState::ContinuousLoop => {
                            let effective_width = line_state.original_width as f32; // Use f32
                            // For seamless looping, the total "segment" length includes the text and the gap
                            let total_segment_length = effective_width + constants::GAP_BETWEEN_LOOP_TEXT as f32; // Use f32

                            // Calculate total pixels scrolled since the ContinuousLoop state started
                            let scrolled_pixels = constants::SCROLL_SPEED_PIXELS_PER_SEC * elapsed_time_secs; // Keep as f32

                            // Calculate the current x_offset, ensuring it wraps correctly for a continuous loop
                            line_state.current_x_offset_float = -(scrolled_pixels % total_segment_length);
                        }
                        ScrollState::CylonLoop => {
                            let effective_width = line_state.original_width as f32;
                            let display_region_width_f32 = constants::DISPLAY_REGION_WIDTH as f32;

                            // If text fits or just barely overflows, it should be static.
                            if effective_width <= display_region_width_f32 {
                                line_state.state = ScrollState::Static;
                                line_state.current_x_offset_float = ((constants::DISPLAY_REGION_WIDTH - line_state.original_width) / 2) as f32;
                                continue; // Skip drawing for this iteration as state changed
                            }
                            
                            // The range of motion from left edge (0) to max left offset (negative)
                            let max_left_offset = -(effective_width - display_region_width_f32);

                            // Total distance for one full ping-pong cycle (left to right and back)
                            let total_cycle_distance = (-max_left_offset) * 2.0; // Use f32 for multiplication

                            let scroll_amount = constants::SCROLL_SPEED_PIXELS_PER_SEC * elapsed_time_secs; // Keep as f32
                            let current_scroll_progress = scroll_amount % total_cycle_distance;

                            if current_scroll_progress <= (-max_left_offset).abs() { // Scrolling left
                                line_state.current_x_offset_float = -current_scroll_progress;
                            } else { // Scrolling right
                                let progress_in_second_half = current_scroll_progress - (-max_left_offset).abs();
                                line_state.current_x_offset_float = max_left_offset + progress_in_second_half;
                            }
                        }
                    }

                    if !line_state.content.is_empty() {
                        self.draw_text_with_custom_font(
                            &line_state.content,
                            Point::new(constants::DISPLAY_REGION_X_OFFSET + line_state.current_x_offset_float.round() as i32, y_pos),
                            Baseline::Top,
                        )?;

                        if line_state.state == ScrollState::ContinuousLoop {
                            let effective_width = line_state.original_width as f32;
                            let total_segment_length = effective_width + constants::GAP_BETWEEN_LOOP_TEXT as f32;

                            self.draw_text_with_custom_font(
                                &line_state.content,
                                Point::new(constants::DISPLAY_REGION_X_OFFSET + (line_state.current_x_offset_float + total_segment_length).round() as i32, y_pos),
                                Baseline::Top,
                            )?;
                        }
                    }
                }

                // --- Player Track Progress Bar ---
                let player_progress_bar_x = constants::DISPLAY_REGION_X_OFFSET;
                let player_progress_bar_y = constants::PLAYER_PROGRESS_BAR_Y_POS;

                let progress_bar_changed = self.current_track_time_secs != self.last_current_track_time_secs ||
                                          self.track_duration_secs != self.last_track_duration_secs;

                if progress_bar_changed {
                    // draw progress bar
                    self.draw_rectangle(
                        Point::new(player_progress_bar_x, player_progress_bar_y),
                        constants::PLAYER_PROGRESS_BAR_WIDTH, constants::PLAYER_PROGRESS_BAR_HEIGHT,
                        BinaryColor::Off,
                        Some(constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS), 
                        Some(BinaryColor::On)
                    )
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        
                    // Calculate the filled width
                    let filled_width_pixel_count = if self.track_duration_secs > 0.00 {
                        (constants::PLAYER_PROGRESS_BAR_WIDTH as f32 * (self.current_track_time_secs as f32 / self.track_duration_secs as f32))
                        .round() as u32
                    } else {
                        0
                    };

                    // The actual width of the inner filled bar, considering the border.
                    let inner_fill_width = (filled_width_pixel_count as i32 - (2 * constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS as i32)).max(0);
                    let inner_height = constants::PLAYER_PROGRESS_BAR_HEIGHT - (2 * constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS);

                    // Draw the filled part if there's actual fill to show
                    if inner_fill_width > 0 {
                        self.draw_rectangle(
                            Point::new(
                                player_progress_bar_x+ constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS as i32, 
                                player_progress_bar_y+ constants::PLAYER_PROGRESS_BAR_BORDER_THICKNESS as i32),
                            inner_fill_width as u32, inner_height,
                            BinaryColor::On,
                            None, None
                        )
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                    }
                    needs_flush = true;
                }

                // --- New Track Info Line (Current Time | Mode | Remaining Time) ---
                let info_line_y = constants::PLAYER_TRACK_INFO_LINE_Y_POS;
                let current_time_str = seconds_to_hms(self.current_track_time_secs);
                let remaining_time_str = format!("-{}", seconds_to_hms(self.remaining_time_secs));
                let total_time_str = format!(" {}", seconds_to_hms(self.track_duration_secs));

                let info_line_changed = self.last_current_track_time_secs != self.current_track_time_secs ||
                                        self.last_remaining_time_secs != self.remaining_time_secs ||
                                        self.last_mode_text != self.mode_text;

                if info_line_changed {
                    // Clear the entire info line area

                    self.draw_rectangle(
                        Point::new(constants::DISPLAY_REGION_X_OFFSET, info_line_y),
                        constants::DISPLAY_REGION_WIDTH, constants::MAIN_FONT_HEIGHT,
                        BinaryColor::Off,None, None
                    )
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

                    self.draw_text(&current_time_str,constants::DISPLAY_REGION_X_OFFSET, info_line_y, None)?;

                    // Draw mode text (centered)
                    let mode_text_width = self.get_text_width(&self.mode_text) as i32;
                    let mode_text_x = constants::DISPLAY_REGION_X_OFFSET + ((constants::DISPLAY_REGION_WIDTH as i32 - mode_text_width) / 2);
                    self.draw_text(&self.mode_text.clone(),mode_text_x, info_line_y, None)?;

                    // Draw total or remaining time (right-justified)
                    let rt_time_width = self.get_text_width(&remaining_time_str) as i32;
                    let rt_time_x = constants::DISPLAY_REGION_X_OFFSET + constants::DISPLAY_REGION_WIDTH as i32 - rt_time_width;
                    if self.show_remaining {
                        self.draw_text(&remaining_time_str,rt_time_x-3, info_line_y, None)?;
                    } else {
                        self.draw_text(&total_time_str,rt_time_x-3, info_line_y, None)?;
                    }
                    self.last_current_track_time_secs = self.current_track_time_secs;
                    self.last_track_duration_secs = self.track_duration_secs;
                    self.last_remaining_time_secs = self.remaining_time_secs;
                    self.last_mode_text = self.mode_text.clone();
                    needs_flush = true;

                }
                
                // Only flush if any drawing operation in this frame necessitated it
                if needs_flush {
                    self.flush()?;
                }
            }
        }
        Ok(())
    }

}

