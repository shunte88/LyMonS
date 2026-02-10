/*
 *  display/manager.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Display manager - orchestrates all display components
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

use crate::config::DisplayConfig;
use crate::display::{
    BoxedDriver,
    DisplayCapabilities,
    DisplayDriverFactory,
    DisplayError,
    DisplayMode,
    FrameBuffer,
    LayoutConfig,
    LayoutManager,
    PageLayout,
};
use crate::display::components::{
    StatusBar,
    ScrollingText,
    ClockDisplay,
    WeatherDisplay as WeatherComponent,
    VisualizerComponent,
    EasterEggsComponent,
};
use crate::textable::ScrollMode;
use crate::clock_font::{ClockFontData, set_clock_font};
use crate::eggs::{Eggs, set_easter_egg};
use crate::metrics::MachineMetrics;
use crate::vision::LastVizState;
use crate::display_old::{RepeatMode, ShuffleMode};

use embedded_graphics::mono_font::ascii::FONT_4X6;
use log::{info, warn};
use std::time::Instant;
use arrayvec::ArrayString;
use core::fmt::Write;
use embedded_graphics::pixelcolor::{BinaryColor, Gray4};
use embedded_graphics::prelude::*;

/// Pre-allocated render buffers to avoid heap allocations in hot paths
#[derive(Debug)]
pub struct RenderBuffers {
    /// Buffer for time strings (e.g., "3:45")
    pub time_buffer: ArrayString<16>,

    /// Buffer for status text
    pub status_buffer: ArrayString<32>,

    /// Buffer for track info
    pub track_buffer: ArrayString<128>,

    /// Buffer for temp calculations
    pub temp_buffer: ArrayString<64>,
}

impl Default for RenderBuffers {
    fn default() -> Self {
        Self {
            time_buffer: ArrayString::new(),
            status_buffer: ArrayString::new(),
            track_buffer: ArrayString::new(),
            temp_buffer: ArrayString::new(),
        }
    }
}

impl RenderBuffers {
    /// Format time as MM:SS (no allocations!)
    pub fn format_time(&mut self, seconds: f32) -> &str {
        self.time_buffer.clear();
        let mins = (seconds as u32) / 60;
        let secs = (seconds as u32) % 60;
        let _ = write!(&mut self.time_buffer, "{}:{:02}", mins, secs);
        &self.time_buffer
    }

    /// Format HMS time (no allocations!)
    pub fn format_hms(&mut self, seconds: f32) -> &str {
        self.time_buffer.clear();
        let hours = (seconds as u32) / 3600;
        let mins = ((seconds as u32) % 3600) / 60;
        let secs = (seconds as u32) % 60;
        if hours > 0 {
            let _ = write!(&mut self.time_buffer, "{}:{:02}:{:02}", hours, mins, secs);
        } else {
            let _ = write!(&mut self.time_buffer, "{}:{:02}", mins, secs);
        }
        &self.time_buffer
    }
}

/// Performance metrics for display rendering
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total frame time (render + transfer)
    pub frame_time_us: u64,

    /// Time spent rendering to framebuffer
    pub render_time_us: u64,

    /// Time spent transferring to hardware
    pub transfer_time_us: u64,

    /// Frame counter for averaging
    pub frame_count: u64,

    /// Average frame time over last N frames
    pub avg_frame_time_us: u64,

    /// Target frame time based on display capabilities
    pub target_frame_time_us: u64,
}

impl PerformanceMetrics {
    pub fn new(target_fps: u32) -> Self {
        let target_frame_time_us = 1_000_000 / target_fps as u64;
        Self {
            frame_time_us: 0,
            render_time_us: 0,
            transfer_time_us: 0,
            frame_count: 0,
            avg_frame_time_us: 0,
            target_frame_time_us,
        }
    }

    pub fn record_frame(&mut self, render_time_us: u64, transfer_time_us: u64) {
        self.render_time_us = render_time_us;
        self.transfer_time_us = transfer_time_us;
        self.frame_time_us = render_time_us + transfer_time_us;
        self.frame_count += 1;

        // Simple moving average (last frame + current) / 2
        if self.avg_frame_time_us == 0 {
            self.avg_frame_time_us = self.frame_time_us;
        } else {
            self.avg_frame_time_us = (self.avg_frame_time_us + self.frame_time_us) / 2;
        }

        // Warn if exceeding target by >20%
        if self.frame_time_us > self.target_frame_time_us * 12 / 10 {
            warn!("Frame time {}μs exceeds target {}μs (render: {}μs, transfer: {}μs)",
                  self.frame_time_us, self.target_frame_time_us,
                  render_time_us, transfer_time_us);
        }
    }

    pub fn fps(&self) -> f32 {
        if self.avg_frame_time_us == 0 {
            0.0
        } else {
            1_000_000.0 / self.avg_frame_time_us as f32
        }
    }
}

/// Display manager that orchestrates all display operations
///
/// `DisplayManager` is the main entry point for the LyMonS display system. It replaces
/// the legacy `OledDisplay` with a modular, trait-based architecture that supports
/// multiple display types and resolutions.
///
/// # Architecture
///
/// The display manager coordinates several subsystems:
///
/// - **Driver abstraction**: Hardware-specific operations via `DisplayDriver` trait
/// - **Component system**: Modular UI components (status bar, clock, weather, etc.)
/// - **Layout system**: Adaptive layouts for different display resolutions
/// - **Performance monitoring**: Real-time frame timing and metrics
///
/// # Features
///
/// - ✅ **Multiple display support**: Works with SSD1306, SSD1309, SSD1322, SH1106
/// - ✅ **Runtime driver selection**: No recompilation needed to switch displays
/// - ✅ **Zero allocations**: Pre-allocated buffers for rendering
/// - ✅ **Performance tracking**: Automatic timing and FPS monitoring
/// - ✅ **Adaptive layouts**: UI automatically adjusts to display resolution
///
/// # Example
///
/// ```no_run
/// use LyMonS::config::DisplayConfig;
/// use LyMonS::display::DisplayManager;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = DisplayConfig::default();
///
/// let mut display = DisplayManager::new(
///     &config,
///     "once_wait",      // scroll mode
///     "lcd17x44",       // clock font
///     false,            // show metrics
///     "none",           // easter egg
/// )?;
///
/// // Render frame
/// display.render()?;
///
/// // Check performance
/// let metrics = display.performance_metrics();
/// println!("FPS: {:.1}", metrics.fps());
/// # Ok(())
/// # }
/// ```
///
/// # Performance
///
/// The display manager is optimized for real-time rendering:
///
/// - **Render path**: Synchronous, no awaits, zero allocations
/// - **Update path**: Asynchronous data fetching (separate from rendering)
/// - **Frame timing**: Microsecond precision with automatic warnings
/// - **Target**: <16.7ms for 60 FPS (SPI), <33ms for 30 FPS (I2C)
///
/// # Thread Safety
///
/// `DisplayManager` is not `Sync` because it contains a mutable driver. It should
/// be owned by a single render thread. Data updates can happen on other threads
/// via channels (e.g., weather updates via watch channel).
pub struct DisplayManager {
    /// Display driver (trait object)
    driver: BoxedDriver,

    /// Framebuffer for rendering
    framebuffer: FrameBuffer,

    /// Display capabilities
    capabilities: DisplayCapabilities,

    /// Layout configuration
    layout: LayoutConfig,

    /// Layout manager - owns all page definitions
    layout_manager: LayoutManager,

    /// Current display mode
    pub current_mode: DisplayMode,

    /// Status bar component
    status_bar: StatusBar,

    /// Scrolling text component
    scrolling_text: ScrollingText,

    /// Clock display component
    clock_display: ClockDisplay,

    /// Weather display component
    weather_display: WeatherComponent,

    /// Visualizer component
    visualizer: VisualizerComponent,

    /// Easter eggs component
    easter_eggs_component: EasterEggsComponent,

    /// Easter egg animations
    pub easter_egg: Eggs,

    /// Whether to show system metrics
    pub show_metrics: bool,

    /// Emulator state (for keyboard shortcuts)
    #[cfg(feature = "emulator")]
    emulator_state: Option<std::sync::Arc<std::sync::Mutex<crate::display::drivers::emulator::EmulatorState>>>,

    /// Device metrics
    pub device_metrics: MachineMetrics,

    /// Last visualizer state
    pub last_viz_state: LastVizState,

    /// Track duration in seconds
    pub track_duration_secs: f32,

    /// Current track time in seconds
    pub current_track_time_secs: f32,

    /// Remaining time in seconds
    pub remaining_time_secs: f32,

    /// Mode text (e.g., "Paused", "Playing")
    pub mode_text: String,

    /// Whether to show remaining time
    pub show_remaining: bool,

    /// Audio quality level (SD=1, HD=2, DSD=3, None=0) for easter egg animations
    pub audio_level: u8,

    /// Current artist for easter eggs (stored separately from scrolling_text)
    pub artist: String,

    /// Current title for easter eggs (stored separately from scrolling_text)
    pub title: String,

    /// Performance metrics
    pub metrics: PerformanceMetrics,

    /// Pre-allocated render buffers (zero allocations in render loop!)
    render_buffers: RenderBuffers,

    /// Weather temperature units ("C" or "F")
    pub weather_temp_units: String,

    /// Weather wind speed units ("mph" or "km/h")
    pub weather_wind_speed_units: String,

    /// Weather location name
    pub weather_location_name: String,

    /// Weather data receiver (watch channel for lock-free updates)
    weather_rx: Option<tokio::sync::watch::Receiver<crate::weather::WeatherConditions>>,

    /// Splash screen state
    splash_active: bool,
    splash_version: String,
    splash_build_date: String,
}

impl DisplayManager {
    /// Create a new display manager
    ///
    /// # Arguments
    ///
    /// * `config` - Display configuration
    /// * `scroll_mode` - Text scrolling mode
    /// * `clock_font` - Clock font name
    /// * `show_metrics` - Whether to show system metrics
    /// * `egg_name` - Easter egg name
    ///
    /// # Returns
    ///
    /// A configured DisplayManager or an error
    pub fn new(
        config: &DisplayConfig,
        scroll_mode: &str,
        clock_font: &str,
        show_metrics: bool,
        egg_name: &str,
    ) -> Result<Self, DisplayError> {
        info!("Initializing DisplayManager with new modular architecture");

        // Create driver from configuration
        let mut driver = DisplayDriverFactory::create_from_config(config)?;
        driver.init()?;

        Self::new_with_driver(driver, scroll_mode, clock_font, show_metrics, egg_name)
    }

    /// Create a new display manager with an existing driver
    ///
    /// This is useful when you need to extract state from the driver before
    /// wrapping it in DisplayManager (e.g., for emulator window state).
    ///
    /// # Arguments
    ///
    /// * `driver` - Pre-initialized display driver
    /// * `scroll_mode` - Text scrolling mode
    /// * `clock_font` - Clock font name
    /// * `show_metrics` - Whether to show system metrics
    /// * `egg_name` - Easter egg name
    ///
    /// # Returns
    ///
    /// A configured DisplayManager or an error
    pub fn new_with_driver(
        driver: BoxedDriver,
        scroll_mode: &str,
        clock_font: &str,
        show_metrics: bool,
        egg_name: &str,
    ) -> Result<Self, DisplayError> {
        // Get capabilities and create layout
        let capabilities = driver.capabilities().clone();
        let layout = LayoutConfig::for_display(&capabilities);

        info!("Display: {}x{}, Layout: {:?}, Assets: {}",
              capabilities.width,
              capabilities.height,
              layout.category,
              layout.asset_path);

        // Create layout manager with page definitions
        let layout_manager = LayoutManager::new(layout.clone());

        // Create framebuffer
        let framebuffer = FrameBuffer::new(&capabilities);

        // Initialize components
        let status_bar = StatusBar::new(layout.clone());

        let scroll_mode_enum = crate::textable::transform_scroll_mode(scroll_mode);
        let scrolling_text = ScrollingText::new(layout.clone(), scroll_mode_enum);

        let clock_font_data = set_clock_font(clock_font);
        let clock_display = ClockDisplay::new(layout.clone(), clock_font_data, show_metrics);

        let weather_display = WeatherComponent::new(layout.clone());

        let visualizer = VisualizerComponent::new(
            layout.clone(),
            crate::visualization::Visualization::NoVisualization,
        );

        let easter_eggs_component = EasterEggsComponent::new(layout.clone());

        let easter_egg = set_easter_egg(egg_name);

        // Create performance metrics with target based on display capabilities
        let metrics = PerformanceMetrics::new(capabilities.max_fps);

        info!("DisplayManager initialized successfully (target: {} FPS)", capabilities.max_fps);

        Ok(Self {
            driver,
            framebuffer,
            capabilities,
            layout,
            layout_manager,
            current_mode: DisplayMode::Scrolling,
            status_bar,
            scrolling_text,
            clock_display,
            weather_display,
            visualizer,
            easter_eggs_component,
            easter_egg,
            show_metrics,
            device_metrics: MachineMetrics::default(),
            last_viz_state: LastVizState::default(),
            track_duration_secs: 0.0,
            current_track_time_secs: 0.0,
            remaining_time_secs: 0.0,
            mode_text: String::new(),
            show_remaining: false,
            audio_level: 0,
            artist: String::new(),
            title: String::new(),
            metrics,
            render_buffers: RenderBuffers::default(),
            weather_temp_units: String::from("C"),
            weather_wind_speed_units: String::from("km/h"),
            weather_location_name: String::new(),
            weather_rx: None,
            splash_active: false,
            splash_version: String::new(),
            splash_build_date: String::new(),
            #[cfg(feature = "emulator")]
            emulator_state: None,
        })
    }

    /// Get display capabilities
    pub fn capabilities(&self) -> &DisplayCapabilities {
        &self.capabilities
    }

    /// Get layout configuration
    pub fn layout(&self) -> &LayoutConfig {
        &self.layout
    }

    /// Get mutable reference to status bar
    pub fn status_bar_mut(&mut self) -> &mut StatusBar {
        &mut self.status_bar
    }

    /// Get mutable reference to scrolling text
    pub fn scrolling_text_mut(&mut self) -> &mut ScrollingText {
        &mut self.scrolling_text
    }

    /// Get mutable reference to clock display
    pub fn clock_display_mut(&mut self) -> &mut ClockDisplay {
        &mut self.clock_display
    }

    /// Get mutable reference to weather display
    pub fn weather_display_mut(&mut self) -> &mut WeatherComponent {
        &mut self.weather_display
    }

    /// Get mutable reference to visualizer
    pub fn visualizer_mut(&mut self) -> &mut VisualizerComponent {
        &mut self.visualizer
    }

    /// Clear the display
    pub fn clear(&mut self) -> Result<(), DisplayError> {
        self.framebuffer.clear();
        self.driver.flush()
    }

    /// Render the current display mode (fast, sync-only path)
    pub fn render(&mut self) -> Result<(), DisplayError> {
        let frame_start = Instant::now();

        // Clear framebuffer
        self.framebuffer.clear();

        // Render based on current mode
        match self.current_mode {
            DisplayMode::Scrolling => self.render_scrolling(),
            DisplayMode::Clock => self.render_clock(),
            DisplayMode::WeatherCurrent => self.render_weather_current(),
            DisplayMode::WeatherForecast => self.render_weather_forecast(),
            DisplayMode::Visualizer => self.render_visualizer(),
            DisplayMode::EasterEggs => self.render_easter_eggs(),
        }?;

        let render_time = frame_start.elapsed().as_micros() as u64;

        // Transfer framebuffer to driver and flush to hardware
        let transfer_start = Instant::now();

        // Pack framebuffer into bytes for driver
        let buffer_data = self.framebuffer.to_packed_bytes();

        // Write buffer to driver
        self.driver.write_buffer(&buffer_data)?;

        // Flush to hardware
        self.driver.flush()?;
        let transfer_time = transfer_start.elapsed().as_micros() as u64;

        // Record performance metrics
        self.metrics.record_frame(render_time, transfer_time);

        Ok(())
    }

    /// Render scrolling text mode
    fn render_scrolling(&mut self) -> Result<(), DisplayError> {
        // Get the scrolling page layout
        let page = self.layout_manager.create_scrolling_page();

        // Update scroll positions using field widths
        if let (
            Some(album_artist_field), 
            Some(album_field), 
            Some(title_field), 
            Some(artist_field)) = (
            page.get_field("album_artist"),
            page.get_field("album"),
            page.get_field("title"),
            page.get_field("artist"),
        ) {
            self.scrolling_text.update_with_fields(
                album_artist_field, 
                album_field, 
                title_field, 
                artist_field);
        }

        // Render each field - dispatch based on framebuffer type
        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                for field in page.fields() {
                    match field.name.as_str() {
                        "status_bar" => {
                            self.status_bar.render_field(field, fb)
                                .map_err(|_| DisplayError::DrawingError("Failed to render status bar".to_string()))?;
                        }
                        "album_artist" | "album" | "title" | "artist" => {
                            self.scrolling_text.render_field(field, fb)
                                .map_err(|_| DisplayError::DrawingError(format!("Failed to render {}", field.name)))?;
                        }
                "progress_bar" => {
                    if self.track_duration_secs > 0.0 {
                        // Extract data inline to avoid borrow conflicts
                        use embedded_graphics::primitives::{Rectangle, PrimitiveStyleBuilder};
                        use embedded_graphics::prelude::*;
                        let field_pos = field.position();
                        let field_width = field.width();
                        let field_height = field.height();
                        let track_duration = self.track_duration_secs;
                        let current_time = self.current_track_time_secs;

                        // Draw outline (inset by 2 pixels on sides)
                        Rectangle::new(
                            Point::new(field_pos.x + 2, field_pos.y),
                            Size::new(field_width - 4, field_height),
                        )
                        .into_styled(PrimitiveStyleBuilder::new()
                            .stroke_color(BinaryColor::On)
                            .stroke_width(1)
                            .build())
                        .draw(fb)
                        .map_err(|_| DisplayError::DrawingError("Failed to draw progress bar".to_string()))?;

                        // Draw fill
                        let progress = (current_time / track_duration).clamp(0.0, 1.0);
                        let fill_width = ((field_width - 6) as f32 * progress) as u32;

                        if fill_width > 0 {
                            Rectangle::new(
                                Point::new(field_pos.x + 3, field_pos.y + 1),
                                Size::new(fill_width, field_height.saturating_sub(2)),
                            )
                            .into_styled(PrimitiveStyleBuilder::new()
                                .fill_color(BinaryColor::On)
                                .build())
                            .draw(fb)
                            .map_err(|_| DisplayError::DrawingError("Failed to draw progress fill".to_string()))?;
                        }
                    }
                }
                "info_line" => {
                    // Inline the info line rendering to avoid borrow conflicts
                    use embedded_graphics::mono_font::{iso_8859_13::FONT_5X8, MonoTextStyle};
                    use embedded_graphics::text::{Text, Baseline};
                    use embedded_graphics::prelude::*;
                    let field_pos = field.position();
                    let field_width = field.width();
                    let info_y = field_pos.y;
                    let font = field.font.unwrap_or(&FONT_5X8);
                    let style = MonoTextStyle::new(font, field.fg_binary());

                    // Current time (left)
                    let current_time_str = self.render_buffers.format_time(self.current_track_time_secs);
                    Text::with_baseline(
                        current_time_str,
                        Point::new(field_pos.x + 2, info_y),
                        style,
                        Baseline::Top,
                    )
                    .draw(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw current time".to_string()))?;

                    // Mode text (center)
                    let mode_x = field_pos.x + (field_width as i32 - (self.mode_text.len() * 5) as i32) / 2;
                    Text::with_baseline(
                        &self.mode_text,
                        Point::new(mode_x, info_y),
                        style,
                        Baseline::Top,
                    )
                    .draw(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw mode text".to_string()))?;

                    // Remaining/total time (right)
                    self.render_buffers.temp_buffer.clear();
                    let time_secs = if self.show_remaining {
                        self.remaining_time_secs
                    } else {
                        self.track_duration_secs
                    };
                    let mins = (time_secs as u32) / 60;
                    let secs = (time_secs as u32) % 60;
                    if self.show_remaining {
                        let _ = write!(&mut self.render_buffers.temp_buffer, "-{}:{:02}", mins, secs);
                    } else {
                        let _ = write!(&mut self.render_buffers.temp_buffer, "{}:{:02}", mins, secs);
                    }
                    let time_str = self.render_buffers.temp_buffer.as_str();

                    let time_x = field_pos.x + field_width as i32 - (time_str.len() * 5) as i32 - 2;
                    Text::with_baseline(
                        time_str,
                        Point::new(time_x, info_y),
                        style,
                        Baseline::Top,
                    )
                    .draw(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw time".to_string()))?;
                }
                        _ => {}
                    }
                }
            }
            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                for field in page.fields() {
                    match field.name.as_str() {
                        "status_bar" => {
                            self.status_bar.render_field(field, fb)
                                .map_err(|_| DisplayError::DrawingError("Failed to render status bar".to_string()))?;
                        }
                        "album_artist" | "album" | "title" | "artist" => {
                            self.scrolling_text.render_field(field, fb)
                                .map_err(|_| DisplayError::DrawingError(format!("Failed to render {}", field.name)))?;
                        }
                        "progress_bar" => {
                            if self.track_duration_secs > 0.0 {
                                // Progress bar rendering for Gray4 displays
                                use embedded_graphics::primitives::{Rectangle, PrimitiveStyleBuilder};
                                use embedded_graphics::prelude::*;
                                let field_pos = field.position();
                                let field_width = field.width();
                                let field_height = field.height();
                                let track_duration = self.track_duration_secs;
                                let current_time = self.current_track_time_secs;

                                // Draw outline (inset by 2 pixels on sides) - use field color or white
                                use crate::display::color_proxy::ConvertColor;
                                let outline_color = field.fg_color.to_color();
                                Rectangle::new(
                                    Point::new(field_pos.x + 2, field_pos.y),
                                    Size::new(field_width - 4, field_height),
                                )
                                .into_styled(PrimitiveStyleBuilder::new()
                                    .stroke_color(outline_color)
                                    .stroke_width(1)
                                    .build())
                                .draw(fb)
                                .map_err(|_| DisplayError::DrawingError("Failed to draw progress bar".to_string()))?;

                                // Draw fill
                                let progress = (current_time / track_duration).clamp(0.0, 1.0);
                                let fill_width = ((field_width - 6) as f32 * progress) as u32;

                                if fill_width > 0 {
                                    Rectangle::new(
                                        Point::new(field_pos.x + 3, field_pos.y + 1),
                                        Size::new(fill_width, field_height.saturating_sub(2)),
                                    )
                                    .into_styled(PrimitiveStyleBuilder::new()
                                        .fill_color(outline_color)
                                        .build())
                                    .draw(fb)
                                    .map_err(|_| DisplayError::DrawingError("Failed to draw progress fill".to_string()))?;
                                }
                            }
                        }
                        "info_line" => {
                            // Info line rendering for Gray4 displays
                            use embedded_graphics::mono_font::{iso_8859_13::FONT_5X8, MonoTextStyle};
                            use embedded_graphics::text::{Text, Baseline};
                            use embedded_graphics::prelude::*;
                            use crate::display::color_proxy::ConvertColor;

                            let field_pos = field.position();
                            let field_width = field.width();
                            let info_y = field_pos.y;
                            let font = field.font.unwrap_or(&FONT_5X8);
                            let text_color = field.fg_color.to_color();
                            let style = MonoTextStyle::new(font, text_color);

                            // Current time (left)
                            let current_time_str = self.render_buffers.format_time(self.current_track_time_secs);
                            Text::with_baseline(
                                current_time_str,
                                Point::new(field_pos.x + 2, info_y),
                                style,
                                Baseline::Top,
                            )
                            .draw(fb)
                            .map_err(|_| DisplayError::DrawingError("Failed to draw current time".to_string()))?;

                            // Mode text (center)
                            let mode_x = field_pos.x + (field_width as i32 - (self.mode_text.len() * 5) as i32) / 2;
                            Text::with_baseline(
                                &self.mode_text,
                                Point::new(mode_x, info_y),
                                style,
                                Baseline::Top,
                            )
                            .draw(fb)
                            .map_err(|_| DisplayError::DrawingError("Failed to draw mode text".to_string()))?;

                            // Remaining/total time (right)
                            self.render_buffers.temp_buffer.clear();
                            let time_secs = if self.show_remaining {
                                self.remaining_time_secs
                            } else {
                                self.track_duration_secs
                            };
                            let mins = (time_secs as u32) / 60;
                            let secs = (time_secs as u32) % 60;
                            if self.show_remaining {
                                let _ = write!(&mut self.render_buffers.temp_buffer, "-{}:{:02}", mins, secs);
                            } else {
                                let _ = write!(&mut self.render_buffers.temp_buffer, "{}:{:02}", mins, secs);
                            }
                            let time_str = self.render_buffers.temp_buffer.as_str();

                            let time_x = field_pos.x + field_width as i32 - (time_str.len() * 5) as i32 - 2;
                            Text::with_baseline(
                                time_str,
                                Point::new(time_x, info_y),
                                style,
                                Baseline::Top,
                            )
                            .draw(fb)
                            .map_err(|_| DisplayError::DrawingError("Failed to draw time".to_string()))?;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    /// Render progress bar to a field with explicit data (to avoid borrow conflicts)
    fn render_progress_bar_to_field_data<D>(
        &mut self,
        field: &crate::display::field::Field,
        fb: &mut D,
        track_duration: f32,
        current_time: f32,
    ) -> Result<(), DisplayError>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        use embedded_graphics::primitives::{Rectangle, PrimitiveStyleBuilder};
        use embedded_graphics::prelude::*;
        let field_pos = field.position();
        let field_width = field.width();
        let field_height = field.height();

        // Draw outline (inset by 2 pixels on sides)
        Rectangle::new(
            Point::new(field_pos.x + 2, field_pos.y),
            Size::new(field_width - 4, field_height),
        )
        .into_styled(PrimitiveStyleBuilder::new()
            .stroke_color(BinaryColor::On)
            .stroke_width(1)
            .build())
        .draw(fb)
        .map_err(|_| DisplayError::DrawingError("Failed to draw progress bar".to_string()))?;

        // Draw fill
        if track_duration > 0.0 {
            let progress = (current_time / track_duration).clamp(0.0, 1.0);
            let fill_width = ((field_width - 6) as f32 * progress) as u32;

            if fill_width > 0 {
                Rectangle::new(
                    Point::new(field_pos.x + 3, field_pos.y + 1),
                    Size::new(fill_width, field_height.saturating_sub(2)),
                )
                .into_styled(PrimitiveStyleBuilder::new()
                    .fill_color(BinaryColor::On)
                    .build())
                .draw(fb)
                .map_err(|_| DisplayError::DrawingError("Failed to draw progress fill".to_string()))?;
            }
        }

        Ok(())
    }

    /// Render progress bar to a field (legacy wrapper)
    fn render_progress_bar_to_field<D>(&mut self, field: &crate::display::field::Field, fb: &mut D) -> Result<(), DisplayError>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        self.render_progress_bar_to_field_data(field, fb, self.track_duration_secs, self.current_track_time_secs)
    }

    /// Render progress bar (legacy method - calls field-based version)
    fn render_progress_bar(&mut self) -> Result<(), DisplayError> {
        use embedded_graphics::primitives::{Rectangle, PrimitiveStyleBuilder};
        use embedded_graphics::prelude::*;

        let fb = self.framebuffer.as_mono_mut();
        let progress_y = self.layout.content_area.y + self.layout.content_area.height - 13;
        let progress_width = self.layout.width - 4;
        let progress_height = 4;

        // Draw outline
        Rectangle::new(
            Point::new(2, progress_y as i32),
            Size::new(progress_width, progress_height),
        )
        .into_styled(PrimitiveStyleBuilder::new()
            .stroke_color(BinaryColor::On)
            .stroke_width(1)
            .build())
        .draw(fb)
        .map_err(|_| DisplayError::DrawingError("Failed to draw progress bar".to_string()))?;

        // Draw fill
        if self.track_duration_secs > 0.0 {
            let progress = (self.current_track_time_secs / self.track_duration_secs).clamp(0.0, 1.0);
            let fill_width = ((progress_width - 2) as f32 * progress) as u32;

            if fill_width > 0 {
                Rectangle::new(
                    Point::new(3, progress_y as i32 + 1),
                    Size::new(fill_width, progress_height - 2),
                )
                .into_styled(PrimitiveStyleBuilder::new()
                    .fill_color(BinaryColor::On)
                    .build())
                .draw(fb)
                .map_err(|_| DisplayError::DrawingError("Failed to draw progress fill".to_string()))?;
            }
        }

        Ok(())
    }

    /// Render info line to a field with explicit data (to avoid borrow conflicts)
    fn render_info_line_to_field_data<D>(
        &mut self,
        field: &crate::display::field::Field,
        fb: &mut D,
        current_time: f32,
        mode_text: &str,
        remaining_time: f32,
        track_duration: f32,
        show_remaining: bool,
    ) -> Result<(), DisplayError>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        use embedded_graphics::mono_font::{iso_8859_13::FONT_5X8, MonoTextStyle};
        use embedded_graphics::text::{Text, Baseline};
        use embedded_graphics::prelude::*;
        let field_pos = field.position();
        let field_width = field.width();
        let info_y = field_pos.y;

        // Use field's font and colors if specified, otherwise default
        let font = field.font.unwrap_or(&FONT_5X8);
        let style = MonoTextStyle::new(font, field.fg_binary());

        // Current time (left)
        let current_time_str = self.render_buffers.format_time(current_time);
        Text::with_baseline(
            current_time_str,
            Point::new(field_pos.x + 2, info_y),
            style,
            Baseline::Top,
        )
        .draw(fb)
        .map_err(|_| DisplayError::DrawingError("Failed to draw current time".to_string()))?;

        // Mode text (center)
        let mode_x = field_pos.x + (field_width as i32 - (mode_text.len() * 5) as i32) / 2;
        Text::with_baseline(
            mode_text,
            Point::new(mode_x, info_y),
            style,
            Baseline::Top,
        )
        .draw(fb)
        .map_err(|_| DisplayError::DrawingError("Failed to draw mode text".to_string()))?;

        // Remaining/total time (right)
        self.render_buffers.temp_buffer.clear();
        let time_secs = if show_remaining {
            remaining_time
        } else {
            track_duration
        };
        let mins = (time_secs as u32) / 60;
        let secs = (time_secs as u32) % 60;
        if show_remaining {
            let _ = write!(&mut self.render_buffers.temp_buffer, "-{}:{:02}", mins, secs);
        } else {
            let _ = write!(&mut self.render_buffers.temp_buffer, "{}:{:02}", mins, secs);
        }
        let time_str = self.render_buffers.temp_buffer.as_str();

        let time_x = field_pos.x + field_width as i32 - (time_str.len() * 5) as i32 - 2;
        Text::with_baseline(
            time_str,
            Point::new(time_x, info_y),
            style,
            Baseline::Top,
        )
        .draw(fb)
        .map_err(|_| DisplayError::DrawingError("Failed to draw time".to_string()))?;

        Ok(())
    }

    /// Render info line to a field (legacy wrapper)
    fn render_info_line_to_field<D>(&mut self, field: &crate::display::field::Field, fb: &mut D) -> Result<(), DisplayError>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        self.render_info_line_to_field_data(
            field,
            fb,
            self.current_track_time_secs,
            &self.mode_text.clone(),
            self.remaining_time_secs,
            self.track_duration_secs,
            self.show_remaining,
        )
    }

    /// Render info line (current time | mode | remaining time) - legacy method
    fn render_info_line(&mut self) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::{iso_8859_13::FONT_5X8, MonoTextStyle};
        use embedded_graphics::text::{Text, Baseline};
        use embedded_graphics::prelude::*;

        let fb = self.framebuffer.as_mono_mut();
        let info_y = self.layout.height - 8;
        let style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);

        // Current time (left)
        let current_time = self.render_buffers.format_time(self.current_track_time_secs);
        Text::with_baseline(
            current_time,
            Point::new(2, info_y as i32),
            style,
            Baseline::Top,
        )
        .draw(fb)
        .map_err(|_| DisplayError::DrawingError("Failed to draw current time".to_string()))?;

        // Mode text (center)
        let mode_x = (self.layout.width as i32 - (self.mode_text.len() * 5) as i32) / 2;
        Text::with_baseline(
            &self.mode_text,
            Point::new(mode_x, info_y as i32),
            style,
            Baseline::Top,
        )
        .draw(fb)
        .map_err(|_| DisplayError::DrawingError("Failed to draw mode text".to_string()))?;

        // Remaining/total time (right)
        // Format time directly in temp_buffer to avoid double borrow
        self.render_buffers.temp_buffer.clear();
        let time_secs = if self.show_remaining {
            self.remaining_time_secs
        } else {
            self.track_duration_secs
        };
        let mins = (time_secs as u32) / 60;
        let secs = (time_secs as u32) % 60;
        if self.show_remaining {
            let _ = write!(&mut self.render_buffers.temp_buffer, "-{}:{:02}", mins, secs);
        } else {
            let _ = write!(&mut self.render_buffers.temp_buffer, "{}:{:02}", mins, secs);
        }
        let time_str = self.render_buffers.temp_buffer.as_str();

        let time_x = self.layout.width as i32 - (time_str.len() * 5) as i32 - 2;
        Text::with_baseline(
            time_str,
            Point::new(time_x, info_y as i32),
            style,
            Baseline::Top,
        )
        .draw(fb)
        .map_err(|_| DisplayError::DrawingError("Failed to draw time".to_string()))?;

        Ok(())
    }

    /// Render metrics data line
    fn render_metrics(&mut self) -> Result<(), DisplayError> {
        if self.show_metrics {
            use embedded_graphics::mono_font::{iso_8859_13::FONT_5X8, MonoTextStyle};
            use embedded_graphics::text::{Text, Baseline};
            use embedded_graphics::prelude::*;

            let metrics_y = 2;

            // Format metrics string (only once)
            self.render_buffers.status_buffer.clear();
            let _ = write!(
                &mut self.render_buffers.status_buffer,
                "CPU:{:.1}% CPUt:{:.1}C MEM:{:.1}% FPS:{:.1}",
                self.device_metrics.cpu_load,
                self.device_metrics.cpu_temp,
                100.00 - self.device_metrics.mem_avail_pct,
                self.metrics.fps(),
            );
            let metrics_str = self.render_buffers.status_buffer.as_str();

            // Center the metrics text
            let text_width = metrics_str.len() * 6; // Approximate width
            let x = (self.layout.width as i32 - text_width as i32) / 2;

            // Render based on framebuffer type
            match &mut self.framebuffer {
                crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                    let style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
                    Text::with_baseline(
                        metrics_str,
                        Point::new(x, metrics_y as i32),
                        style,
                        Baseline::Top,
                    )
                    .draw(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw metrics".to_string()))?;
                }
                crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                    let style = MonoTextStyle::new(&FONT_5X8, Gray4::WHITE);
                    Text::with_baseline(
                        metrics_str,
                        Point::new(x, metrics_y as i32),
                        style,
                        Baseline::Top,
                    )
                    .draw(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw metrics".to_string()))?;
                }
            }
        }
        Ok(())
    }
    
    /// Render clock mode
    fn render_clock(&mut self) -> Result<(), DisplayError> {
        // Get the clock page layout
        let page = self.layout_manager.create_clock_page();

        // Extract current second for progress bar
        let current_second: u32 = chrono::Local::now().format("%S").to_string().parse().unwrap_or(0);

        // Render metrics first (if present) before borrowing framebuffer
        if page.fields().iter().any(|f| f.name == "metrics") {
            self.render_metrics()
                .map_err(|_| DisplayError::DrawingError("Failed to render metrics".to_string()))?;
        }

        // Clock is monochrome-only, but must work on both display types
        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                // Render each field for monochrome displays
                for field in page.fields() {
                    match field.name.as_str() {
                        "metrics" => {
                            // Already rendered above
                        }
                        "clock_digits" => {
                            // Clock renders digits with field color (e.g., green)
                            self.clock_display.render(fb, field.fg_binary())
                                .map_err(|_| DisplayError::DrawingError("Failed to render clock".to_string()))?;
                        }
                        "seconds_progress" => {
                            // Render seconds progress bar in its own field
                            use embedded_graphics::primitives::{Rectangle as EgRectangle, PrimitiveStyleBuilder};
                            use embedded_graphics::prelude::*;

                            let field_pos = field.position();
                            let field_width = field.width();
                            let field_height = field.height();

                            // Draw progress bar outline
                            EgRectangle::new(
                                Point::new(field_pos.x, field_pos.y),
                                Size::new(field_width, field_height),
                            )
                            .into_styled(PrimitiveStyleBuilder::new()
                                .stroke_color(BinaryColor::On)
                                .stroke_width(1)
                                .build())
                            .draw(fb)
                            .map_err(|_| DisplayError::DrawingError("Failed to draw progress bar outline".to_string()))?;

                            // Draw progress bar fill (seconds)
                            let progress = (current_second as f32) / 60.0;
                            let fill_width = (((field_width - 2) as f32) * progress) as u32;

                            if fill_width > 0 {
                                EgRectangle::new(
                                    Point::new(field_pos.x + 1, field_pos.y + 1),
                                    Size::new(fill_width, field_height - 2),
                                )
                                .into_styled(PrimitiveStyleBuilder::new()
                                    .fill_color(BinaryColor::On)
                                    .build())
                                .draw(fb)
                                .map_err(|_| DisplayError::DrawingError("Failed to draw progress fill".to_string()))?;
                            }
                        }
                        "date" => {
                            // Render date text using field color (e.g., cyan)
                            use embedded_graphics::mono_font::MonoTextStyle;
                            let font = field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_6X10);
                            let style = MonoTextStyle::new(font, field.fg_binary());
                            let date_str = chrono::Local::now().format("%a %b %d").to_string();

                            // Use DRY helper for text rendering
                            Self::draw_field_text_mono(fb, field, &date_str, style)
                                .map_err(|_| DisplayError::DrawingError("Failed to draw date".to_string()))?;
                        }
                        _ => {}
                    }
                }
            }
            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                // Clock is monochrome-only, render in white on grayscale displays
                for field in page.fields() {
                    match field.name.as_str() {
                        "metrics" => {
                            // Already rendered above
                        }
                        "clock_digits" => {
                            // Clock renders digits with field color (e.g., green → gray4 value 8)
                            self.clock_display.render_gray4(fb, field.fg_color.to_gray4())
                                .map_err(|_| DisplayError::DrawingError("Failed to render clock".to_string()))?;
                        }
                        "seconds_progress" => {
                            // Render seconds progress bar in white
                            use embedded_graphics::primitives::{Rectangle as EgRectangle, PrimitiveStyleBuilder};
                            use embedded_graphics::prelude::*;

                            let field_pos = field.position();
                            let field_width = field.width();
                            let field_height = field.height();

                            // Draw progress bar outline (white)
                            EgRectangle::new(
                                Point::new(field_pos.x, field_pos.y),
                                Size::new(field_width, field_height),
                            )
                            .into_styled(PrimitiveStyleBuilder::new()
                                .stroke_color(Gray4::WHITE)
                                .stroke_width(1)
                                .build())
                            .draw(fb)
                            .map_err(|_| DisplayError::DrawingError("Failed to draw progress bar outline".to_string()))?;

                            // Draw progress bar fill (seconds)
                            let progress = (current_second as f32) / 60.0;
                            let fill_width = (((field_width - 2) as f32) * progress) as u32;

                            if fill_width > 0 {
                                EgRectangle::new(
                                    Point::new(field_pos.x + 1, field_pos.y + 1),
                                    Size::new(fill_width, field_height - 2),
                                )
                                .into_styled(PrimitiveStyleBuilder::new()
                                    .fill_color(Gray4::WHITE)
                                    .build())
                                .draw(fb)
                                .map_err(|_| DisplayError::DrawingError("Failed to draw progress fill".to_string()))?;
                            }
                        }
                        "date" => {
                            // Render date text using field color (e.g., cyan → gray4 value 11)
                            use embedded_graphics::mono_font::MonoTextStyle;
                            let font = field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_6X10);
                            let style = MonoTextStyle::new(font, field.fg_color.to_gray4());
                            let date_str = chrono::Local::now().format("%a %b %d").to_string();

                            // Use DRY helper for text rendering
                            Self::draw_field_text_gray4(fb, field, &date_str, style)
                                .map_err(|_| DisplayError::DrawingError("Failed to draw date".to_string()))?;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    /// Render current weather
    fn render_weather_current(&mut self) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::MonoTextStyle;
        use embedded_graphics::text::{Text, Baseline};
        use embedded_graphics::prelude::*;
        use embedded_graphics::mono_font::iso_8859_13::FONT_6X10;

        // Get weather page layout
        let page = self.layout_manager.create_weather_current_page();

        // Extract all weather data needed to avoid borrow conflicts
        let weather_data = if let Some(current) = self.weather_display.weather_data().first() {
            current.clone()
        } else {
            // No weather data available - show message
            let msg = "No Weather Data\n\nConfigure -W option\nwith API key";
            match &mut self.framebuffer {
                crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
                    Text::with_baseline(msg, Point::new(10, 20), style, Baseline::Top)
                        .draw(fb)
                        .map_err(|_| DisplayError::DrawingError("Failed to draw message".to_string()))?;
                }
                crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                    let style = MonoTextStyle::new(&FONT_6X10, Gray4::WHITE);
                    Text::with_baseline(msg, Point::new(10, 20), style, Baseline::Top)
                        .draw(fb)
                        .map_err(|_| DisplayError::DrawingError("Failed to draw message".to_string()))?;
                }
            }
            return Ok(());
        };

        // Get weather icon SVG path
        let svg_path = weather_data.weather_code.svg.clone();

        // Format text strings
        let conditions_text = weather_data.weather_code.description.clone();
        let temp_text = format!("{}({}) °{}",
            weather_data.temperature_avg.round() as i32,
            weather_data.temperature_apparent_avg.round() as i32,
            weather_data.temperature_units.clone()
        );
        let humidity_text = format!("{}%", weather_data.humidity_avg);
        let wind_text = format!("{} {} {}",
            weather_data.wind_speed_avg.round() as i32,
            weather_data.wind_speed_units.clone(),
            weather_data.wind_direction
        );
        let precip_text = format!("{}%", weather_data.precipitation_probability_avg.round() as i32);

        // Wide display fields (sunrise, sunset, moon phase)
        let sunrise_text = if let Some(sunrise) = weather_data.sunrise_time {
            format!("{}", sunrise.format("%l:%M %p"))
        } else {
            "--:--".to_string()
        };
        let sunset_text = if let Some(sunset) = weather_data.sunset_time {
            format!("{}", sunset.format("%l:%M %p"))
        } else {
            "--:--".to_string()
        };
        let moonrise_text = if let Some(moonrise) = weather_data.moonrise_time {
            format!("{}", moonrise.format("%l:%M %p"))
        } else {
            "--:--".to_string()
        };
        let moonset_text = if let Some(moonset) = weather_data.moonrise_time {
            format!("{}", moonset.format("%l:%M %p"))
        } else {
            "--:--".to_string()
        };

        // Dispatch rendering based on framebuffer type
        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                Self::render_weather_fields_mono(
                    fb, 
                    &page, 
                    &svg_path, 
                    &conditions_text, 
                    &temp_text, 
                    &humidity_text, 
                    &wind_text, 
                    &precip_text, 
                    &sunrise_text, 
                    &sunset_text, 
                    &moonrise_text,
                    &moonset_text,
                )?;
            }
            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                Self::render_weather_fields_gray4(
                    fb, 
                    &page, 
                    &svg_path, 
                    &conditions_text, 
                    &temp_text, 
                    &humidity_text, 
                    &wind_text, 
                    &precip_text, 
                    &sunrise_text, 
                    &sunset_text, 
                    &moonrise_text,
                    &moonset_text,
                )?;
            }
        }

        Ok(())
    }

    /// Render weather fields to monochrome display (static method to avoid borrow issues)
    fn render_weather_fields_mono(
        target: &mut impl DrawTarget<Color = BinaryColor>,
        page: &crate::display::PageLayout,
        svg_path: &str,
        conditions_text: &str,
        temp_text: &str,
        humidity_text: &str,
        wind_text: &str,
        precip_text: &str,
        sunrise_text: &str,
        sunset_text: &str,
        moonrise_text: &str,
        moonset_text: &str, // and moon phase idx
    ) -> Result<(), DisplayError> {
        use embedded_graphics::prelude::*;
        use embedded_graphics::Pixel;
        use embedded_graphics::mono_font::MonoTextStyle;
        use embedded_graphics::text::{Text, Baseline};
        use embedded_graphics::mono_font::iso_8859_13::{FONT_5X8, FONT_6X13_BOLD, FONT_7X14};

        // Render all fields
        for field in page.fields() {
            let pos = field.position();

            match field.name.as_str() {
                "weather_icon" => {
                    // Render SVG weather icon
                    if !svg_path.is_empty() && svg_path.contains(".svg") {
                        let full_path = format!("./assets/mono/{}", svg_path);
                        let icon_width = field.bounds.size.width;
                        let icon_height = field.bounds.size.height;

                        // Render SVG to buffer
                        let mut svg_buffer = Vec::new();
                        if let Ok(_) = crate::drawsvg::get_svg(&full_path, icon_width, icon_height, &mut svg_buffer) {
                            // Draw the SVG buffer as ImageRaw
                            use embedded_graphics::image::{Image, ImageRaw};
                            let raw_image = ImageRaw::<BinaryColor>::new(&svg_buffer, icon_width);
                            Image::new(&raw_image, Point::new(pos.x, pos.y))
                                .draw(target)
                                .map_err(|_| DisplayError::DrawingError("Failed to draw weather icon".to_string()))?;
                        }
                    }
                }
                "temp_glyph" => {
                    use crate::weather_glyph::GLYPH_TEMPERATURE;
                    Self::draw_weather_glyph(target, GLYPH_TEMPERATURE, pos.x, pos.y, field.fg_color.to_binary())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw temp glyph".to_string()))?;
                }
                "humidity_glyph" => {
                    use crate::weather_glyph::GLYPH_HUMIDITY;
                    Self::draw_weather_glyph(target, GLYPH_HUMIDITY, pos.x, pos.y, field.fg_color.to_binary())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw humidity glyph".to_string()))?;
                }
                "wind_glyph" => {
                    use crate::weather_glyph::GLYPH_WIND;
                    Self::draw_weather_glyph(target, GLYPH_WIND, pos.x, pos.y, field.fg_color.to_binary())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw wind glyph".to_string()))?;
                }
                "precip_glyph" => {
                    use crate::weather_glyph::GLYPH_PRECIPITATION;
                    Self::draw_weather_glyph(target, GLYPH_PRECIPITATION, pos.x, pos.y, field.fg_color.to_binary())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw precip glyph".to_string()))?;
                }
                "temperature" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_6X13_BOLD), field.fg_color.to_binary());
                    Text::with_baseline(temp_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw temperature".to_string()))?;
                }
                "humidity" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_binary());
                    Text::with_baseline(humidity_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw humidity".to_string()))?;
                }
                "wind" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_binary());
                    Text::with_baseline(wind_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw wind".to_string()))?;
                }
                "precipitation" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_binary());
                    Text::with_baseline(precip_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw precipitation".to_string()))?;
                }
                "conditions" => {
                    let font = field.font.unwrap_or(&FONT_7X14);
                    let style = MonoTextStyle::new(font, BinaryColor::On);
                    // Use DRY helper for text rendering
                    Self::draw_field_text_mono(target, field, conditions_text, style)
                        .map_err(|_| DisplayError::DrawingError("Failed to draw conditions".to_string()))?;
                }
                "sunrise_glyph" => {
                    use crate::weather_glyph::GLYPH_SUNRISE;
                    Self::draw_weather_glyph(target, GLYPH_SUNRISE, pos.x, pos.y, field.fg_color.to_binary())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw sunrise glyph".to_string()))?;
                }
                "sunrise_text" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_binary());
                    Text::with_baseline(sunrise_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw sunrise".to_string()))?;
                }
                "sunset_glyph" => {
                    use crate::weather_glyph::GLYPH_SUNSET;
                    Self::draw_weather_glyph(target, GLYPH_SUNSET, pos.x, pos.y, field.fg_color.to_binary())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw sunset glyph".to_string()))?;
                }
                "sunset_text" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_binary());
                    Text::with_baseline(sunset_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw sunset".to_string()))?;
                }
                "moonrise_glyph" => {
                    use crate::weather_glyph::GLYPH_MOONRISE;
                    Self::draw_weather_glyph(target, GLYPH_MOONRISE, pos.x, pos.y, field.fg_color.to_binary())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw moonrise glyph".to_string()))?;
                }
                "moonrise_text" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_binary());
                    Text::with_baseline(moonrise_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw moon".to_string()))?;
                }
                "moonset_glyph" => {
                    use crate::weather_glyph::GLYPH_MOONSET;
                    Self::draw_weather_glyph(target, GLYPH_MOONSET, pos.x, pos.y, field.fg_color.to_binary())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw moonset glyph".to_string()))?;
                }
                "moonset_text" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_binary());
                    Text::with_baseline(moonset_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw moon".to_string()))?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Render weather fields to Gray4 display (static method to avoid borrow issues)
    fn render_weather_fields_gray4(
        target: &mut impl DrawTarget<Color = Gray4>,
        page: &crate::display::PageLayout,
        svg_path: &str,
        conditions_text: &str,
        temp_text: &str,
        humidity_text: &str,
        wind_text: &str,
        precip_text: &str,
        sunrise_text: &str,
        sunset_text: &str,
        moonrise_text: &str,
        moonset_text: &str, // and moon phase idx
    ) -> Result<(), DisplayError> {
        use embedded_graphics::prelude::*;
        use embedded_graphics::Pixel;
        use embedded_graphics::mono_font::MonoTextStyle;
        use embedded_graphics::text::{Text, Baseline};
        use embedded_graphics::mono_font::iso_8859_13::{FONT_5X8, FONT_6X13_BOLD, FONT_7X14};

        // Render all fields
        for field in page.fields() {
            let pos = field.position();

            match field.name.as_str() {
                "weather_icon" => {
                    // Render SVG weather icon for Gray4
                    if !svg_path.is_empty() && svg_path.contains(".svg") {
                        let full_path = format!("./assets/color/{}", svg_path);
                        let icon_width = field.bounds.size.width;
                        let icon_height = field.bounds.size.height;

                        // Render SVG to Gray4 buffer with color support
                        let mut svg_buffer = Vec::new();
                        if let Ok(_) = crate::drawsvg::get_svg_gray4(&full_path, icon_width, icon_height, &mut svg_buffer) {
                            // Draw the SVG buffer as ImageRaw
                            use embedded_graphics::image::{Image, ImageRaw};
                            let raw_image = ImageRaw::<Gray4>::new(&svg_buffer, icon_width);
                            Image::new(&raw_image, Point::new(pos.x, pos.y))
                                .draw(target)
                                .map_err(|_| DisplayError::DrawingError("Failed to draw weather icon".to_string()))?;
                        }
                    }
                }
                "temp_glyph" => {
                    use crate::weather_glyph::GLYPH_TEMPERATURE;
                    Self::draw_weather_glyph(target, GLYPH_TEMPERATURE, pos.x, pos.y, field.fg_color.to_gray4())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw temp glyph".to_string()))?;
                }
                "humidity_glyph" => {
                    use crate::weather_glyph::GLYPH_HUMIDITY;
                    Self::draw_weather_glyph(target, GLYPH_HUMIDITY, pos.x, pos.y, field.fg_color.to_gray4())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw humidity glyph".to_string()))?;
                }
                "wind_glyph" => {
                    use crate::weather_glyph::GLYPH_WIND;
                    Self::draw_weather_glyph(target, GLYPH_WIND, pos.x, pos.y, field.fg_color.to_gray4())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw wind glyph".to_string()))?;
                }
                "precip_glyph" => {
                    use crate::weather_glyph::GLYPH_PRECIPITATION;
                    Self::draw_weather_glyph(target, GLYPH_PRECIPITATION, pos.x, pos.y, field.fg_color.to_gray4())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw precip glyph".to_string()))?;
                }
                "temperature" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_6X13_BOLD), field.fg_color.to_gray4());
                    Text::with_baseline(temp_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw temperature".to_string()))?;
                }
                "humidity" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_gray4());
                    Text::with_baseline(humidity_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw humidity".to_string()))?;
                }
                "wind" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_gray4());
                    Text::with_baseline(wind_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw wind".to_string()))?;
                }
                "precipitation" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_gray4());
                    Text::with_baseline(precip_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw precipitation".to_string()))?;
                }
                "conditions" => {
                    let font = field.font.unwrap_or(&FONT_7X14);
                    let style = MonoTextStyle::new(font, Gray4::WHITE);
                    // Use DRY helper for text rendering
                    Self::draw_field_text_gray4(target, field, conditions_text, style)
                        .map_err(|_| DisplayError::DrawingError("Failed to draw conditions".to_string()))?;
                }
                "sunrise_glyph" => {
                    use crate::weather_glyph::GLYPH_SUNRISE;
                    Self::draw_weather_glyph(target, GLYPH_SUNRISE, pos.x, pos.y, field.fg_color.to_gray4())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw sunrise glyph".to_string()))?;
                }
                "sunrise_text" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_gray4());
                    Text::with_baseline(sunrise_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw sunrise".to_string()))?;
                }
                "sunset_glyph" => {
                    use crate::weather_glyph::GLYPH_SUNSET;
                    Self::draw_weather_glyph(target, GLYPH_SUNSET, pos.x, pos.y, field.fg_color.to_gray4())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw sunset glyph".to_string()))?;
                }
                "sunset_text" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_gray4());
                    Text::with_baseline(sunset_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw sunset".to_string()))?;
                }
                "moonrise_glyph" => {
                    use crate::weather_glyph::GLYPH_MOONRISE;
                    Self::draw_weather_glyph(target, GLYPH_MOONRISE, pos.x, pos.y, field.fg_color.to_gray4())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw moonrise glyph".to_string()))?;
                }
                "moonrise_text" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_gray4());
                    Text::with_baseline(moonrise_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw moon".to_string()))?;
                }
                "moonset_glyph" => {
                    use crate::weather_glyph::GLYPH_MOONSET;
                    Self::draw_weather_glyph(target, GLYPH_MOONSET, pos.x, pos.y, field.fg_color.to_gray4())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw moonset glyph".to_string()))?;
                }
                "moonset_text" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&FONT_5X8), field.fg_color.to_gray4());
                    Text::with_baseline(moonset_text, Point::new(pos.x, pos.y), style, Baseline::Top)
                        .draw(target)
                        .map(|_| ())
                        .map_err(|_| DisplayError::DrawingError("Failed to draw moon".to_string()))?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Draw a weather glyph (12x12 from THERMO_RAW_DATA)
    /// glyph_index: 0=temp, 1=wind, 2=humidity, 3=precip
    fn draw_weather_glyph<D, C>(target: &mut D, glyph_index: usize, x: i32, y: i32, color: C) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
        C: PixelColor,
    {
        use embedded_graphics::prelude::*;
        use embedded_graphics::Pixel;
        use crate::weather_glyph::THERMO_RAW_DATA;
        use crate::glyphs::get_glyph_slice;

        // Each glyph is 12x12, stored as 24 bytes (12 rows * 2 bytes per row)
        let glyph_data = get_glyph_slice(THERMO_RAW_DATA, glyph_index, 12, 12);

        // Iterate over 12x12 glyph bitmap (2 bytes per row)
        for row in 0..12 {
            let byte_idx = row * 2;
            let word = ((glyph_data[byte_idx] as u16) << 8) | (glyph_data[byte_idx + 1] as u16);

            for col in 0..12 {
                if (word & (1 << (15 - col))) != 0 {
                    let pixel = Pixel(Point::new(x + col, y + row as i32), color);
                    target.draw_iter(core::iter::once(pixel))?;
                }
            }
        }

        Ok(())
    }


    /// Render weather forecast
    fn render_weather_forecast(&mut self) -> Result<(), DisplayError> {
        use embedded_graphics::prelude::*;
        use embedded_graphics::mono_font::MonoTextStyle;
        use embedded_graphics::text::{Text, Baseline};
        use embedded_graphics::mono_font::iso_8859_13::FONT_6X10;
        use embedded_graphics::pixelcolor::{BinaryColor, Gray4};
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle as EgRect};

        // Get the weather forecast page layout
        let page = self.layout_manager.create_weather_forecast_page();

        // Get forecast data
        let forecast_data = self.weather_display.weather_data();

        if forecast_data.len() < 4 {
            // Not enough forecast data (need current + 3 days)
            let msg = "Loading Forecast...";
            match &mut self.framebuffer {
                crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
                    Text::with_baseline(msg, Point::new(10, 20), style, Baseline::Top)
                        .draw(fb)
                        .map_err(|_| DisplayError::DrawingError("Failed to draw message".to_string()))?;
                }
                crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                    let style = MonoTextStyle::new(&FONT_6X10, Gray4::WHITE);
                    Text::with_baseline(msg, Point::new(10, 20), style, Baseline::Top)
                        .draw(fb)
                        .map_err(|_| DisplayError::DrawingError("Failed to draw message".to_string()))?;
                }
            }
            return Ok(());
        }

        // Get forecast days (skip index 0 which is current)
        let day1 = &forecast_data[1];
        let day2 = &forecast_data[2];
        let day3 = &forecast_data[3];

        // Format data for each day
        let day1_name = day1.day.format("%a").to_string();
        let day1_temp = format!("{}°|{}°", day1.temperature_min.round() as i32, day1.temperature_max.round() as i32);
        let day1_precip = format!("{}%", day1.precipitation_probability_avg.round() as i32);
        let day1_svg = day1.weather_code.svg.clone();

        let day2_name = day2.day.format("%a").to_string();
        let day2_temp = format!("{}°|{}°", day2.temperature_min.round() as i32, day2.temperature_max.round() as i32);
        let day2_precip = format!("{}%", day2.precipitation_probability_avg.round() as i32);
        let day2_svg = day2.weather_code.svg.clone();

        let day3_name = day3.day.format("%a").to_string();
        let day3_temp = format!("{}°|{}°", day3.temperature_min.round() as i32, day3.temperature_max.round() as i32);
        let day3_precip = format!("{}%", day3.precipitation_probability_avg.round() as i32);
        let day3_svg = day3.weather_code.svg.clone();

        // Days 4-6 for wide displays (conditional)
        let (day4_name, day4_temp, day4_precip, day4_svg) = if forecast_data.len() > 4 {
            let day4 = &forecast_data[4];
            (day4.day.format("%a").to_string(),
             format!("{}°|{}°", day4.temperature_min.round() as i32, day4.temperature_max.round() as i32),
             format!("{}%", day4.precipitation_probability_avg.round() as i32),
             day4.weather_code.svg.clone())
        } else {
            ("".to_string(), "".to_string(), "".to_string(), "".to_string())
        };

        let (day5_name, day5_temp, day5_precip, day5_svg) = if forecast_data.len() > 5 {
            let day5 = &forecast_data[5];
            (day5.day.format("%a").to_string(),
             format!("{}°|{}°", day5.temperature_min.round() as i32, day5.temperature_max.round() as i32),
             format!("{}%", day5.precipitation_probability_avg.round() as i32),
             day5.weather_code.svg.clone())
        } else {
            ("".to_string(), "".to_string(), "".to_string(), "".to_string())
        };

        let (day6_name, day6_temp, day6_precip, day6_svg) = if forecast_data.len() > 6 {
            let day6 = &forecast_data[6];
            (day6.day.format("%a").to_string(),
             format!("{}°|{}°", day6.temperature_min.round() as i32, day6.temperature_max.round() as i32),
             format!("{}%", day6.precipitation_probability_avg.round() as i32),
             day6.weather_code.svg.clone())
        } else {
            ("".to_string(), "".to_string(), "".to_string(), "".to_string())
        };

        // Dispatch rendering
        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                Self::render_forecast_fields_mono(
                    fb, &page,
                    &day1_name, &day1_temp, &day1_precip, &day1_svg,
                    &day2_name, &day2_temp, &day2_precip, &day2_svg,
                    &day3_name, &day3_temp, &day3_precip, &day3_svg,
                    &day4_name, &day4_temp, &day4_precip, &day4_svg,
                    &day5_name, &day5_temp, &day5_precip, &day5_svg,
                    &day6_name, &day6_temp, &day6_precip, &day6_svg,
                )?;
            }
            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                Self::render_forecast_fields_gray4(
                    fb, &page,
                    &day1_name, &day1_temp, &day1_precip, &day1_svg,
                    &day2_name, &day2_temp, &day2_precip, &day2_svg,
                    &day3_name, &day3_temp, &day3_precip, &day3_svg,
                    &day4_name, &day4_temp, &day4_precip, &day4_svg,
                    &day5_name, &day5_temp, &day5_precip, &day5_svg,
                    &day6_name, &day6_temp, &day6_precip, &day6_svg,
                )?;
            }
        }

        Ok(())
    }

    /// Render forecast fields to monochrome display
    fn render_forecast_fields_mono(
        target: &mut impl DrawTarget<Color = BinaryColor>,
        page: &crate::display::PageLayout,
        day1_name: &str, day1_temp: &str, day1_precip: &str, day1_svg: &str,
        day2_name: &str, day2_temp: &str, day2_precip: &str, day2_svg: &str,
        day3_name: &str, day3_temp: &str, day3_precip: &str, day3_svg: &str,
        day4_name: &str, day4_temp: &str, day4_precip: &str, day4_svg: &str,
        day5_name: &str, day5_temp: &str, day5_precip: &str, day5_svg: &str,
        day6_name: &str, day6_temp: &str, day6_precip: &str, day6_svg: &str,
    ) -> Result<(), DisplayError> {

        for field in page.fields() {

            match field.name.as_str() {
                // Day 1
                "day1_icon" => Self::render_forecast_icon_mono(target, field, day1_svg)?,
                "day1_name" => Self::render_centered_text_mono(target, field, day1_name)?,
                "day1_data_box" => Self::render_box_mono(target, field)?,
                "day1_temp" => Self::render_centered_text_mono(target, field, day1_temp)?,
                "day1_precip" => Self::render_centered_text_mono(target, field, day1_precip)?,

                // Day 2
                "day2_icon" => Self::render_forecast_icon_mono(target, field, day2_svg)?,
                "day2_name" => Self::render_centered_text_mono(target, field, day2_name)?,
                "day2_data_box" => Self::render_box_mono(target, field)?,
                "day2_temp" => Self::render_centered_text_mono(target, field, day2_temp)?,
                "day2_precip" => Self::render_centered_text_mono(target, field, day2_precip)?,

                // Day 3
                "day3_icon" => Self::render_forecast_icon_mono(target, field, day3_svg)?,
                "day3_name" => Self::render_centered_text_mono(target, field, day3_name)?,
                "day3_data_box" => Self::render_box_mono(target, field)?,
                "day3_temp" => Self::render_centered_text_mono(target, field, day3_temp)?,
                "day3_precip" => Self::render_centered_text_mono(target, field, day3_precip)?,

                // Day 4 (wide display)
                "day4_icon" => Self::render_forecast_icon_mono(target, field, day4_svg)?,
                "day4_name" => Self::render_centered_text_mono(target, field, day4_name)?,
                "day4_data_box" => Self::render_box_mono(target, field)?,
                "day4_temp" => Self::render_centered_text_mono(target, field, day4_temp)?,
                "day4_precip" => Self::render_centered_text_mono(target, field, day4_precip)?,

                // Day 5 (wide display)
                "day5_icon" => Self::render_forecast_icon_mono(target, field, day5_svg)?,
                "day5_name" => Self::render_centered_text_mono(target, field, day5_name)?,
                "day5_data_box" => Self::render_box_mono(target, field)?,
                "day5_temp" => Self::render_centered_text_mono(target, field, day5_temp)?,
                "day5_precip" => Self::render_centered_text_mono(target, field, day5_precip)?,

                // Day 6 (wide display)
                "day6_icon" => Self::render_forecast_icon_mono(target, field, day6_svg)?,
                "day6_name" => Self::render_centered_text_mono(target, field, day6_name)?,
                "day6_data_box" => Self::render_box_mono(target, field)?,
                "day6_temp" => Self::render_centered_text_mono(target, field, day6_temp)?,
                "day6_precip" => Self::render_centered_text_mono(target, field, day6_precip)?,

                _ => {}
            }
        }

        Ok(())
    }

    /// Render forecast fields to Gray4 display
    fn render_forecast_fields_gray4(
        target: &mut impl DrawTarget<Color = Gray4>,
        page: &crate::display::PageLayout,
        day1_name: &str, day1_temp: &str, day1_precip: &str, day1_svg: &str,
        day2_name: &str, day2_temp: &str, day2_precip: &str, day2_svg: &str,
        day3_name: &str, day3_temp: &str, day3_precip: &str, day3_svg: &str,
        day4_name: &str, day4_temp: &str, day4_precip: &str, day4_svg: &str,
        day5_name: &str, day5_temp: &str, day5_precip: &str, day5_svg: &str,
        day6_name: &str, day6_temp: &str, day6_precip: &str, day6_svg: &str,
    ) -> Result<(), DisplayError> {

        for field in page.fields() {

            match field.name.as_str() {
                // Day 1
                "day1_icon" => Self::render_forecast_icon_gray4(target, field, day1_svg)?,
                "day1_name" => Self::render_centered_text_gray4(target, field, day1_name)?,
                "day1_data_box" => Self::render_box_gray4(target, field)?,
                "day1_temp" => Self::render_centered_text_gray4(target, field, day1_temp)?,
                "day1_precip" => Self::render_centered_text_gray4(target, field, day1_precip)?,

                // Day 2
                "day2_icon" => Self::render_forecast_icon_gray4(target, field, day2_svg)?,
                "day2_name" => Self::render_centered_text_gray4(target, field, day2_name)?,
                "day2_data_box" => Self::render_box_gray4(target, field)?,
                "day2_temp" => Self::render_centered_text_gray4(target, field, day2_temp)?,
                "day2_precip" => Self::render_centered_text_gray4(target, field, day2_precip)?,

                // Day 3
                "day3_icon" => Self::render_forecast_icon_gray4(target, field, day3_svg)?,
                "day3_name" => Self::render_centered_text_gray4(target, field, day3_name)?,
                "day3_data_box" => Self::render_box_gray4(target, field)?,
                "day3_temp" => Self::render_centered_text_gray4(target, field, day3_temp)?,
                "day3_precip" => Self::render_centered_text_gray4(target, field, day3_precip)?,

                // Day 4 (wide display)
                "day4_icon" => Self::render_forecast_icon_gray4(target, field, day4_svg)?,
                "day4_name" => Self::render_centered_text_gray4(target, field, day4_name)?,
                "day4_data_box" => Self::render_box_gray4(target, field)?,
                "day4_temp" => Self::render_centered_text_gray4(target, field, day4_temp)?,
                "day4_precip" => Self::render_centered_text_gray4(target, field, day4_precip)?,

                // Day 5 (wide display)
                "day5_icon" => Self::render_forecast_icon_gray4(target, field, day5_svg)?,
                "day5_name" => Self::render_centered_text_gray4(target, field, day5_name)?,
                "day5_data_box" => Self::render_box_gray4(target, field)?,
                "day5_temp" => Self::render_centered_text_gray4(target, field, day5_temp)?,
                "day5_precip" => Self::render_centered_text_gray4(target, field, day5_precip)?,

                // Day 6 (wide display)
                "day6_icon" => Self::render_forecast_icon_gray4(target, field, day6_svg)?,
                "day6_name" => Self::render_centered_text_gray4(target, field, day6_name)?,
                "day6_data_box" => Self::render_box_gray4(target, field)?,
                "day6_temp" => Self::render_centered_text_gray4(target, field, day6_temp)?,
                "day6_precip" => Self::render_centered_text_gray4(target, field, day6_precip)?,

                _ => {}
            }
        }

        Ok(())
    }

    // Helper methods for forecast rendering
    fn render_forecast_icon_mono(
        target: &mut impl DrawTarget<Color = BinaryColor>,
        field: &crate::display::Field,
        svg_path: &str,
    ) -> Result<(), DisplayError> {
        if !svg_path.is_empty() && svg_path.contains(".svg") {
            let full_path = format!("./assets/mono/{}", svg_path);
            let icon_width = field.bounds.size.width;
            let icon_height = field.bounds.size.height;

            let mut svg_buffer = Vec::new();
            if let Ok(_) = crate::drawsvg::get_svg(&full_path, icon_width, icon_height, &mut svg_buffer) {
                use embedded_graphics::image::{Image, ImageRaw};
                let raw_image = ImageRaw::<BinaryColor>::new(&svg_buffer, icon_width);
                Image::new(&raw_image, field.position())
                    .draw(target)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw forecast icon".to_string()))?;
            }
        }
        Ok(())
    }

    fn render_forecast_icon_gray4(
        target: &mut impl DrawTarget<Color = Gray4>,
        field: &crate::display::Field,
        svg_path: &str,
    ) -> Result<(), DisplayError> {
        if !svg_path.is_empty() && svg_path.contains(".svg") {
            let full_path = format!("./assets/color/{}", svg_path);
            let icon_width = field.bounds.size.width;
            let icon_height = field.bounds.size.height;

            let mut svg_buffer = Vec::new();
            if let Ok(_) = crate::drawsvg::get_svg_gray4(&full_path, icon_width, icon_height, &mut svg_buffer) {
                use embedded_graphics::image::{Image, ImageRaw};
                let raw_image = ImageRaw::<Gray4>::new(&svg_buffer, icon_width);
                Image::new(&raw_image, field.position())
                    .draw(target)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw forecast icon".to_string()))?;
            }
        }
        Ok(())
    }

    /// Text rendering helper for BinaryColor using TextBoxStyleBuilder
    ///
    /// Uses embedded_text's TextBoxStyleBuilder for proper text alignment and layout.
    #[inline]
    fn draw_field_text_mono(
        target: &mut impl DrawTarget<Color = BinaryColor>,
        field: &crate::display::Field,
        text: &str,
        style: embedded_graphics::mono_font::MonoTextStyle<'static, BinaryColor>,
    ) -> Result<(), DisplayError>
    {
        use embedded_text::{
            style::TextBoxStyleBuilder,
            TextBox,
        };

        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(field.horizontal_alignment)
            .vertical_alignment(field.vertical_alignment)
            .build();

        TextBox::with_textbox_style(
            text,
            field.bounds,
            style,
            textbox_style,
        )
        .draw(target)
        .map(|_| ()) // TextBox::draw returns &str, map to ()
        .map_err(|_| DisplayError::DrawingError("Failed to draw text".to_string()))
    }

    /// Text rendering helper for Gray4 using TextBoxStyleBuilder
    ///
    /// Uses embedded_text's TextBoxStyleBuilder for proper text alignment and layout.
    #[inline]
    fn draw_field_text_gray4(
        target: &mut impl DrawTarget<Color = Gray4>,
        field: &crate::display::Field,
        text: &str,
        style: embedded_graphics::mono_font::MonoTextStyle<'static, Gray4>,
    ) -> Result<(), DisplayError>
    {
        use embedded_text::{
            style::TextBoxStyleBuilder,
            TextBox,
        };

        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(field.horizontal_alignment)
            .vertical_alignment(field.vertical_alignment)
            .build();

        TextBox::with_textbox_style(
            text,
            field.bounds,
            style,
            textbox_style,
        )
        .draw(target)
        .map(|_| ()) // TextBox::draw returns &str, map to ()
        .map_err(|_| DisplayError::DrawingError("Failed to draw text".to_string()))
    }

    fn render_centered_text_mono(
        target: &mut impl DrawTarget<Color = BinaryColor>,
        field: &crate::display::Field,
        text: &str,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::MonoTextStyle;
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle as EgRect};

        // Draw border if specified
        if field.border > 0 {
            let rect = EgRect::new(field.position(), field.bounds.size);
            rect.into_styled(PrimitiveStyle::with_stroke(field.fg_color.to_binary(), field.border as u32))
                .draw(target)
                .map_err(|_| DisplayError::DrawingError("Failed to draw border".to_string()))?;
        }

        let font = field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_4X6);
        let style = MonoTextStyle::new(font, field.fg_color.to_binary());

        // Use DRY helper for text rendering
        Self::draw_field_text_mono(target, field, text, style)
    }

    fn render_centered_text_gray4(
        target: &mut impl DrawTarget<Color = Gray4>,
        field: &crate::display::Field,
        text: &str,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::MonoTextStyle;
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle as EgRect};

        // Draw border if specified
        if field.border > 0 {
            let rect = EgRect::new(field.position(), field.bounds.size);
            rect.into_styled(PrimitiveStyle::with_stroke(field.fg_color.to_gray4(), field.border as u32))
                .draw(target)
                .map_err(|_| DisplayError::DrawingError("Failed to draw border".to_string()))?;
        }

        let font = field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_4X6);
        let style = MonoTextStyle::new(font, field.fg_color.to_gray4());

        // Use DRY helper for text rendering
        Self::draw_field_text_gray4(target, field, text, style)
    }

    fn render_box_mono(
        target: &mut impl DrawTarget<Color = BinaryColor>,
        field: &crate::display::Field,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle as EgRect};

        if field.border > 0 {
            let rect = EgRect::new(
                field.position(),
                field.bounds.size
            );
            rect.into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, field.border as u32))
                .draw(target)
                .map_err(|_| DisplayError::DrawingError("Failed to draw box".to_string()))?;
        }
        Ok(())
    }

    fn render_box_gray4(
        target: &mut impl DrawTarget<Color = Gray4>,
        field: &crate::display::Field,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle as EgRect};

        if field.border > 0 {
            let rect = EgRect::new(
                field.position(),
                field.bounds.size
            );
            rect.into_styled(PrimitiveStyle::with_stroke(Gray4::WHITE, field.border as u32))
                .draw(target)
                .map_err(|_| DisplayError::DrawingError("Failed to draw box".to_string()))?;
        }
        Ok(())
    }


    /// Render splash screen for monochrome displays
    fn render_splash_mono(
        target: &mut impl DrawTarget<Color = BinaryColor>,
        page: &crate::display::PageLayout,
        version: &str,
        build_date: &str,
        status: Option<&str>,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::MonoTextStyle;

        for field in page.fields() {
            match field.name.as_str() {
                "logo_svg" => {
                    // Render logo SVG
                    let svg_path = "./assets/lymonslogo.svg";
                    let mut svg_buffer = Vec::new();
                    if let Ok(_) = crate::drawsvg::get_svg(
                        svg_path,
                        field.bounds.size.width,
                        field.bounds.size.height,
                        &mut svg_buffer
                    ) {
                        use embedded_graphics::image::{Image, ImageRaw};
                        let raw_image = ImageRaw::<BinaryColor>::new(&svg_buffer, field.bounds.size.width);
                        Image::new(&raw_image, field.position())
                            .draw(target)
                            .map_err(|_| DisplayError::DrawingError("Failed to draw logo".to_string()))?;
                    }
                }
                "version" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_6X13_BOLD), field.fg_color.to_binary());
                    Self::draw_field_text_mono(target, field, version, style)?;
                }
                "build_date" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_5X8), field.fg_color.to_binary());
                    Self::draw_field_text_mono(target, field, build_date, style)?;
                }
                "status" => {
                    // Optional status field - render if provided
                    if let Some(status_text) = status {
                        let style = MonoTextStyle::new(field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_5X8), field.fg_color.to_binary());
                        Self::draw_field_text_mono(target, field, status_text, style)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Render splash screen for grayscale displays
    fn render_splash_gray4(
        target: &mut impl DrawTarget<Color = Gray4>,
        page: &crate::display::PageLayout,
        version: &str,
        build_date: &str,
        status: Option<&str>,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::MonoTextStyle;

        for field in page.fields() {
            match field.name.as_str() {
                "logo_svg" => {
                    // Render logo SVG with color support
                    let svg_path = "./assets/lymonslogo.svg";
                    let mut svg_buffer = Vec::new();
                    if let Ok(_) = crate::drawsvg::get_svg_gray4(
                        svg_path,
                        field.bounds.size.width,
                        field.bounds.size.height,
                        &mut svg_buffer
                    ) {
                        use embedded_graphics::image::{Image, ImageRaw};
                        let raw_image = ImageRaw::<Gray4>::new(&svg_buffer, field.bounds.size.width);
                        Image::new(&raw_image, field.position())
                            .draw(target)
                            .map_err(|_| DisplayError::DrawingError("Failed to draw logo".to_string()))?;
                    }
                }
                "version" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_6X13_BOLD), field.fg_color.to_gray4());
                    Self::draw_field_text_gray4(target, field, version, style)?;
                }
                "build_date" => {
                    let style = MonoTextStyle::new(field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_5X8), field.fg_color.to_gray4());
                    Self::draw_field_text_gray4(target, field, build_date, style)?;
                }
                "status" => {
                    // Optional status field - render if provided
                    if let Some(status_text) = status {
                        let style = MonoTextStyle::new(field.font.unwrap_or(&embedded_graphics::mono_font::iso_8859_13::FONT_5X8), field.fg_color.to_gray4());
                        Self::draw_field_text_gray4(target, field, status_text, style)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Render visualizer
    fn render_visualizer(&mut self) -> Result<(), DisplayError> {
        use crate::visualizer::VizPayload;

        // Try to consume latest frame from visualizer channel (non-blocking)
        if let Some(visualizer) = self.visualizer.visualizer_mut() {
            // Drain all pending frames, keeping only the latest
            let mut latest_frame = None;
            while let Ok(frame) = visualizer.rx.try_recv() {
                latest_frame = Some(frame);
            }

            // If we got a frame, update component state based on payload
            if let Some(frame) = latest_frame {
                match frame.payload {
                    VizPayload::PeakStereo { l_level, r_level, l_hold, r_hold } => {
                        // Update viz state directly
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_peak_l = l_level;
                        viz_state.last_peak_r = r_level;
                        viz_state.last_hold_l = l_hold;
                        viz_state.last_hold_r = r_hold;
                    }
                    VizPayload::PeakMono { level, hold } => {
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_peak_m = level;
                        viz_state.last_hold_m = hold;
                    }
                    VizPayload::HistMono { bands } => {
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_bands_m = bands;
                    }
                    VizPayload::HistStereo { bands_l, bands_r } => {
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_bands_l = bands_l;
                        viz_state.last_bands_r = bands_r;
                    }
                    VizPayload::VuMono { db } => {
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_db_m = db;
                    }
                    VizPayload::VuStereo { l_db, r_db } => {
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_db_l = l_db;
                        viz_state.last_db_r = r_db;
                    }
                    VizPayload::AioVuMono { db } => {
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_db_m = db;
                        // Update track info for AIO display
                        let track_info = if !self.artist.is_empty() && !self.title.is_empty() {
                            format!("{} - {}", self.artist, self.title)
                        } else if !self.artist.is_empty() {
                            self.artist.clone()
                        } else if !self.title.is_empty() {
                            self.title.clone()
                        } else {
                            String::from("No track playing")
                        };
                        viz_state.last_artist = track_info;
                    }
                    VizPayload::AioHistMono { bands } => {
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_bands_m = bands;
                        // Update track info for AIO display
                        let track_info = if !self.artist.is_empty() && !self.title.is_empty() {
                            format!("{} - {}", self.artist, self.title)
                        } else if !self.artist.is_empty() {
                            self.artist.clone()
                        } else if !self.title.is_empty() {
                            self.title.clone()
                        } else {
                            String::from("No track playing")
                        };
                        viz_state.last_artist = track_info;
                    }
                    VizPayload::WaveformSpectrum { waveform_l, waveform_r, spectrum_column } => {
                        let viz_state = self.visualizer.viz_state_mut();
                        viz_state.last_waveform_l = waveform_l;
                        viz_state.last_waveform_r = waveform_r;
                        // spectrum_column is handled inside the drawing function via spectrum_history
                        viz_state.spectrum_history.push_back(spectrum_column);
                        if viz_state.spectrum_history.len() > viz_state.spectrum_max_cols {
                            viz_state.spectrum_history.pop_front();
                        }
                    }
                    _ => {
                        // TODO: Handle other visualization types (combi modes, AIO, etc.)
                    }
                }
            }
        }

        // Dispatch to the appropriate render method based on framebuffer type
        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                self.visualizer.render_mono(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to render visualizer (mono)".to_string()))?;
            }
            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                self.visualizer.render_gray4(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to render visualizer (gray4)".to_string()))?;
            }
        }

        Ok(())
    }

    /// Render easter eggs
    fn render_easter_eggs(&mut self) -> Result<(), DisplayError> {
        // Calculate track progress percentage
        let track_percent = if self.track_duration_secs > 0.0 {
            (self.current_track_time_secs / self.track_duration_secs).clamp(0.0, 1.0) as f64
        } else {
            0.0
        };

        // Check if display is wide (256+ pixels)
        let is_wide = self.capabilities.width >= 256;
        let can_widen = self.easter_egg.can_widen();

        // Get position and rectangles
        let position = self.easter_egg.get_top_left();
        let mut artist_rect = self.easter_egg.get_artist_rect();
        let mut title_rect = self.easter_egg.get_title_rect();
        let mut time_rect = self.easter_egg.get_time_rect();
        let is_combined = self.easter_egg.is_combined();

        // For wide displays, extend text block widths by 128 pixels
        // can_widen attribute refines the rule - cassette for example 
        // uses text overlay for artist and track so widen is not applicable
        if is_wide {
            if can_widen {
                if artist_rect.size.width != 0 {
                    artist_rect.size.width += 128;
                    if artist_rect.top_left.x+artist_rect.size.width as i32 > self.capabilities.width  as i32{
                        artist_rect.size.width = self.capabilities.width - (artist_rect.top_left.x - 4) as u32 
                    }
                }
                if title_rect.size.width != 0 {
                    title_rect.size.width += 128;
                    if title_rect.top_left.x+title_rect.size.width as i32 > self.capabilities.width  as i32{
                        title_rect.size.width = self.capabilities.width - (title_rect.top_left.x - 4) as u32 
                    }
                }
            }
            // does not fall umder can_widen rule
            time_rect.size.width += 128;
            if time_rect.top_left.x+time_rect.size.width as i32 > self.capabilities.width  as i32{
                time_rect.size.width = self.capabilities.width - (time_rect.top_left.x - 4) as u32 
            }
        }

        // Match on framebuffer type and render accordingly
        // We handle the render and text extraction separately for each type to avoid borrow issues
        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                // Render the easter egg SVG
                let raw_image = self.easter_egg
                    .update_and_render_blocking(
                        &self.artist,
                        &self.title,
                        self.audio_level,
                        track_percent,
                        self.current_track_time_secs,
                    )
                    .map_err(|e| DisplayError::DrawingError(format!("Easter egg render failed: {}", e)))?;

                // Draw SVG image
                embedded_graphics::image::Image::new(&raw_image, position)
                    .draw(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw easter egg image".to_string()))?;
            } // raw_image dropped here, borrow of self.easter_egg ends

            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                // Render the easter egg SVG with full Gray4 color support
                let raw_image = self.easter_egg
                    .update_and_render_blocking_gray4(
                        &self.artist,
                        &self.title,
                        self.audio_level,
                        track_percent,
                        self.current_track_time_secs,
                    )
                    .map_err(|e| DisplayError::DrawingError(format!("Easter egg render failed: {}", e)))?;

                // Draw Gray4 SVG image directly
                embedded_graphics::image::Image::new(&raw_image, position)
                    .draw(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw easter egg image".to_string()))?;
            } // raw_image dropped here, borrow of self.easter_egg ends
        }

        // Now draw text overlays (borrow of self.easter_egg is free)
        let artist_text = self.easter_egg.get_artist().to_string();
        let title_text = self.easter_egg.get_title().to_string();
        let track_time = self.easter_egg.get_track_time();
        let show_remaining = self.show_remaining;
        let remaining_time = self.remaining_time_secs;

        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                if !artist_rect.is_zero_sized() {
                    Self::draw_egg_artist_text_mono(fb, &artist_rect, &artist_text, is_combined, is_wide, can_widen)?;
                }
                if !is_combined && !title_rect.is_zero_sized() {
                    Self::draw_egg_title_text_mono(fb, &title_rect, &title_text, is_wide, can_widen)?;
                }
                if !time_rect.is_zero_sized() {
                    Self::draw_egg_time_text_mono(fb, &time_rect, track_time, show_remaining, remaining_time)?;
                }
            }
            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                if !artist_rect.is_zero_sized() {
                    Self::draw_egg_artist_text_gray4(fb, &artist_rect, &artist_text, is_combined, is_wide, can_widen)?;
                }
                if !is_combined && !title_rect.is_zero_sized() {
                    Self::draw_egg_title_text_gray4(fb, &title_rect, &title_text, is_wide, can_widen)?;
                }
                if !time_rect.is_zero_sized() {
                    Self::draw_egg_time_text_gray4(fb, &time_rect, track_time, show_remaining, remaining_time)?;
                }
            }
        }

        Ok(())
    }

    /// Draw artist text for easter egg (monochrome)
    fn draw_egg_artist_text_mono(
        target: &mut impl embedded_graphics::prelude::DrawTarget<Color = BinaryColor>,
        rect: &embedded_graphics::primitives::Rectangle,
        artist_text: &str,
        is_combined: bool,
        is_wide: bool,
        can_widen: bool,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::{iso_8859_13::{FONT_4X6, FONT_7X13_BOLD}, MonoTextStyle};
        use embedded_text::{
            alignment::{HorizontalAlignment, VerticalAlignment},
            style::TextBoxStyleBuilder,
            TextBox,
        };

        if artist_text.is_empty() {
            return Ok(());
        }

        // Use larger font for wide displays (double size: 4x6 -> 8x13)
        let font = if is_wide && can_widen{ &FONT_7X13_BOLD } else { &FONT_4X6 };
        let character_style = MonoTextStyle::new(font, BinaryColor::On);

        // Use TextBox for automatic text wrapping (like original implementation)
        let textbox_style = if is_combined {
            // Combined mode: Left-aligned, Top
            TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Top)
                .build()
        } else {
            // Non-combined: Centered, Middle
            TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Middle)
                .build()
        };

        TextBox::with_textbox_style(
            artist_text,
            *rect,
            character_style,
            textbox_style,
        )
        .draw(target)
        .map_err(|_| DisplayError::DrawingError("Failed to draw artist text".to_string()))?;

        Ok(())
    }

    /// Draw title text for easter egg (static method)
    fn draw_egg_title_text_mono(
        target: &mut impl embedded_graphics::prelude::DrawTarget<Color = BinaryColor>,
        rect: &embedded_graphics::primitives::Rectangle,
        title_text: &str,
        is_wide: bool,
        can_widen: bool,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::{iso_8859_13::{FONT_4X6, FONT_7X13_BOLD}, MonoTextStyle};
        use embedded_text::{
            alignment::{HorizontalAlignment, VerticalAlignment},
            style::TextBoxStyleBuilder,
            TextBox,
        };

        if title_text.is_empty() {
            return Ok(());
        }

        // Use larger font for wide displays (double size: 4x6 -> 8x13)
        let font = if is_wide && can_widen{ &FONT_7X13_BOLD } else { &FONT_4X6 };
        let character_style = MonoTextStyle::new(font, BinaryColor::On);

        // Center alignment, Middle vertical alignment (with wrapping)
        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Middle)
            .build();

        TextBox::with_textbox_style(
            title_text,
            *rect,
            character_style,
            textbox_style,
        )
        .draw(target)
        .map_err(|_| DisplayError::DrawingError("Failed to draw title text".to_string()))?;

        Ok(())
    }

    /// Draw time text for easter egg (monochrome)
    fn draw_egg_time_text_mono(
        target: &mut impl embedded_graphics::prelude::DrawTarget<Color = BinaryColor>,
        rect: &embedded_graphics::primitives::Rectangle,
        track_time: f32,
        show_remaining: bool,
        remaining_time: f32,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::{iso_8859_13::FONT_6X10, MonoTextStyle};
        use embedded_text::{
            alignment::{HorizontalAlignment, VerticalAlignment},
            style::TextBoxStyleBuilder,
            TextBox,
        };
        use crate::deutils::seconds_to_hms;

        let time_str = if show_remaining {
            format!("-{}", seconds_to_hms(remaining_time))
        } else {
            seconds_to_hms(track_time)
        };

        let character_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

        // Right alignment, Middle vertical alignment
        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Middle)
            .build();

        TextBox::with_textbox_style(
            &time_str,
            *rect,
            character_style,
            textbox_style,
        )
        .draw(target)
        .map_err(|_| DisplayError::DrawingError("Failed to draw time text".to_string()))?;

        Ok(())
    }

    /// Draw artist text for easter egg (grayscale)
    fn draw_egg_artist_text_gray4(
        target: &mut impl embedded_graphics::prelude::DrawTarget<Color = embedded_graphics::pixelcolor::Gray4>,
        rect: &embedded_graphics::primitives::Rectangle,
        artist_text: &str,
        is_combined: bool,
        is_wide: bool,
        can_widen: bool,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::{iso_8859_13::{FONT_4X6, FONT_7X13_BOLD}, MonoTextStyle};
        use embedded_graphics::pixelcolor::Gray4;
        use embedded_text::{
            alignment::{HorizontalAlignment, VerticalAlignment},
            style::TextBoxStyleBuilder,
            TextBox,
        };

        if artist_text.is_empty() {
            return Ok(());
        }

        // Use larger font for wide displays (double size: 4x6 -> 8x13)
        let font = if is_wide && can_widen { &FONT_7X13_BOLD } else { &FONT_4X6 };
        let character_style = MonoTextStyle::new(font, Gray4::WHITE);

        // Center alignment, Top/Middle based on is_combined
        let textbox_style = if is_combined {
            TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Top)
                .build()
        } else {
            TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Middle)
                .build()
        };

        TextBox::with_textbox_style(
            artist_text,
            *rect,
            character_style,
            textbox_style,
        )
        .draw(target)
        .map_err(|_| DisplayError::DrawingError("Failed to draw artist text".to_string()))?;

        Ok(())
    }

    fn draw_egg_title_text_gray4(
        target: &mut impl embedded_graphics::prelude::DrawTarget<Color = embedded_graphics::pixelcolor::Gray4>,
        rect: &embedded_graphics::primitives::Rectangle,
        title_text: &str,
        is_wide: bool,
        can_widen: bool,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::{iso_8859_13::{FONT_4X6, FONT_7X13_BOLD}, MonoTextStyle};
        use embedded_graphics::pixelcolor::Gray4;
        use embedded_text::{
            alignment::{HorizontalAlignment, VerticalAlignment},
            style::TextBoxStyleBuilder,
            TextBox,
        };

        if title_text.is_empty() {
            return Ok(());
        }

        // Use larger font for wide displays (double size: 4x6 -> 8x13)
        let font = if is_wide && can_widen { &FONT_7X13_BOLD } else { &FONT_4X6 };
        let character_style = MonoTextStyle::new(font, Gray4::WHITE);

        // Center alignment, Middle vertical alignment (with wrapping)
        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Middle)
            .build();

        TextBox::with_textbox_style(
            title_text,
            *rect,
            character_style,
            textbox_style,
        )
        .draw(target)
        .map_err(|_| DisplayError::DrawingError("Failed to draw title text".to_string()))?;

        Ok(())
    }

    /// Draw time text for easter egg (grayscale)
    fn draw_egg_time_text_gray4(
        target: &mut impl embedded_graphics::prelude::DrawTarget<Color = embedded_graphics::pixelcolor::Gray4>,
        rect: &embedded_graphics::primitives::Rectangle,
        track_time: f32,
        show_remaining: bool,
        remaining_time: f32,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::{iso_8859_13::FONT_6X10, MonoTextStyle};
        use embedded_graphics::pixelcolor::Gray4;
        use embedded_text::{
            alignment::{HorizontalAlignment, VerticalAlignment},
            style::TextBoxStyleBuilder,
            TextBox,
        };
        use crate::deutils::seconds_to_hms;

        let time_str = if show_remaining {
            format!("-{}", seconds_to_hms(remaining_time))
        } else {
            seconds_to_hms(track_time)
        };

        let character_style = MonoTextStyle::new(&FONT_6X10, Gray4::WHITE);

        // Right alignment, Middle vertical alignment
        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Middle)
            .build();

        TextBox::with_textbox_style(
            &time_str,
            *rect,
            character_style,
            textbox_style,
        )
        .draw(target)
        .map_err(|_| DisplayError::DrawingError("Failed to draw time text".to_string()))?;

        Ok(())
    }

    /// Set brightness
    pub fn set_brightness(&mut self, brightness: u8) -> Result<(), DisplayError> {
        self.driver.set_brightness(brightness)
    }

    /// Get current display mode
    pub fn display_mode(&self) -> DisplayMode {
        self.current_mode
    }

    /// Set display mode
    pub fn set_display_mode(&mut self, mode: DisplayMode) {
        self.current_mode = mode;
    }

    /// Set emulator state (for keyboard shortcuts)
    #[cfg(feature = "emulator")]
    pub fn set_emulator_state(&mut self, state: std::sync::Arc<std::sync::Mutex<crate::display::drivers::emulator::EmulatorState>>) {
        self.emulator_state = Some(state);
    }

    /// Check if manual mode override is active (keyboard locked mode)
    #[cfg(feature = "emulator")]
    pub fn is_manual_mode_override(&self) -> bool {
        if let Some(ref state) = self.emulator_state {
            let guard = state.lock().unwrap();
            return guard.manual_mode_override;
        }
        false
    }

    #[cfg(not(feature = "emulator"))]
    pub fn is_manual_mode_override(&self) -> bool {
        false
    }

    /// Check for emulator mode requests (keyboard shortcuts)
    #[cfg(feature = "emulator")]
    pub fn check_emulator_mode_request(&mut self) -> Option<DisplayMode> {
        // Check emulator state if available
        if let Some(ref state) = self.emulator_state {
            let mut guard = state.lock().unwrap();
            return guard.requested_mode.take();
        }
        None
    }

    #[cfg(not(feature = "emulator"))]
    pub fn check_emulator_mode_request(&mut self) -> Option<DisplayMode> {
        None
    }

    /// Update current display mode in emulator state (for keyboard toggle tracking)
    #[cfg(feature = "emulator")]
    pub fn update_emulator_current_mode(&mut self, mode: DisplayMode) {
        if let Some(ref state) = self.emulator_state {
            let mut guard = state.lock().unwrap();
            guard.current_display_mode = mode;
        }
    }

    #[cfg(not(feature = "emulator"))]
    pub fn update_emulator_current_mode(&mut self, _mode: DisplayMode) {
        // No-op for non-emulator builds
    }

    /// Async update path - fetch data from external sources
    ///
    /// This method handles all async operations (LMS polling, weather updates, etc.)
    /// and should be called separately from the sync render() method.
    ///
    /// Separating async updates from sync rendering ensures:
    /// - Consistent frame timing (no await in render loop)
    /// - Updates can run concurrently with rendering
    /// - Easy to add rate limiting per data source
    pub async fn update(&mut self) -> Result<(), DisplayError> {
        // TODO: Implement async data updates
        // This would poll:
        // - LMS player status
        // - Audio visualizer data
        // - System metrics (if enabled)
        //
        // Weather updates would come via watch channel (already lock-free!)

        Ok(())
    }

    /// Get reference to render buffers for external use
    pub fn render_buffers_mut(&mut self) -> &mut RenderBuffers {
        &mut self.render_buffers
    }

    /// Get performance metrics
    pub fn performance_metrics(&self) -> &PerformanceMetrics {
        &self.metrics
    }

    // === OledDisplay-compatible interface for main loop ===

    /// Set status line data (volume, bitrate, repeat, shuffle)
    pub fn set_status_line_data(
        &mut self,
        volume: u8,
        is_muted: bool,
        samplesize: String,
        samplerate: String,
        repeat: RepeatMode,
        shuffle: ShuffleMode,
    ) {
        use crate::display::components::status_bar::{
            RepeatMode as SBRepeatMode,
            ShuffleMode as SBShuffleMode,
        };

        self.status_bar.set_volume(volume);
        self.status_bar.set_muted(is_muted);

        // Convert RepeatMode from display_old to status_bar
        let sb_repeat = match repeat {
            RepeatMode::Off => SBRepeatMode::Off,
            RepeatMode::RepeatAll => SBRepeatMode::All,
            RepeatMode::RepeatOne => SBRepeatMode::One,
        };
        self.status_bar.set_repeat_mode(sb_repeat);

        // Convert ShuffleMode from display_old to status_bar
        let sb_shuffle = match shuffle {
            ShuffleMode::Off => SBShuffleMode::Off,
            ShuffleMode::ByTracks => SBShuffleMode::ByTracks,
            ShuffleMode::ByAlbums => SBShuffleMode::ByAlbums,
        };
        self.status_bar.set_shuffle_mode(sb_shuffle);

        self.status_bar.set_bitrate(&samplerate, &samplesize);

        // Determine audio level for easter eggs (SD=1, HD=2, DSD=3, None=0)
        let samp_size: u32 = samplesize.parse().unwrap_or(0);
        let samp_rate: u32 = samplerate.parse().unwrap_or(0);

        self.audio_level = if samplesize.to_uppercase().contains("DSD") || samplerate.to_uppercase().contains("DSD") {
            3 // DSD
        } else if samp_size >= 24 || samp_rate > 44100 {
            2 // HD
        } else if samp_size > 0 && samp_rate > 0 {
            1 // SD
        } else {
            0 // None
        };
    }

    /// Set track details (artist, album, title, album_artist)
    pub async fn set_track_details(
        &mut self,
        album_artist: String,
        album: String,
        title: String,
        artist: String,
        _scroll_mode: &str,
    ) {
        // Store for easter eggs
        self.artist = artist.clone();
        self.title = title.clone();

        // Update scrolling text component
        self.scrolling_text.set_full_track_info(
            album_artist, 
            album, 
            title, 
            artist
        );
        // Note: update() is called in render_scrolling() on each frame
    }

    /// Set track progress data (duration, elapsed, remaining, mode)
    pub fn set_track_progress_data(
        &mut self,
        show_remaining: bool,
        duration: f32,
        elapsed: f32,
        remaining: f32,
        mode: String,
    ) {
        self.show_remaining = show_remaining;
        self.track_duration_secs = duration;
        self.current_track_time_secs = elapsed;
        self.remaining_time_secs = remaining;
        self.mode_text = mode;
    }

    /// Stub methods for OledDisplay compatibility
    pub fn connections(&mut self, _inet: &str, _eth0: &str, _wlan0: &str) {}

    /// Display splash screen with logo, version, and build date
    pub async fn splash(&mut self, show: bool, version: &str, build_date: &str) -> Result<(), DisplayError> {
        if !show {
            self.splash_active = false;
            return Ok(());
        }

        // Store splash state for status updates
        self.splash_active = true;
        self.splash_version = version.to_string();
        self.splash_build_date = build_date.to_string();

        info!("Rendering splash screen...");

        // Get splash page layout
        let splash_page = self.layout_manager.create_splash_page();

        // Clear display
        let _ = self.clear();

        // Render based on framebuffer type
        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                Self::render_splash_mono(fb, &splash_page, version, build_date, None)?;
            }
            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                Self::render_splash_gray4(fb, &splash_page, version, build_date, None)?;
            }
        }

        // Flush to display
        self.driver.flush()?;

        // Hold splash for a moment
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

        Ok(())
    }

    /// Update splash screen status message (during initialization)
    pub fn update_splash_status(&mut self, status: &str) -> Result<(), DisplayError> {
        if !self.splash_active {
            return Ok(());
        }

        // Get splash page layout
        let splash_page = self.layout_manager.create_splash_page();

        // Clear display
        let _ = self.clear();

        // Render with status message
        match &mut self.framebuffer {
            crate::display::framebuffer::FrameBuffer::Mono(fb) => {
                Self::render_splash_mono(
                    fb,
                    &splash_page,
                    &self.splash_version,
                    &self.splash_build_date,
                    Some(status)
                )?;
            }
            crate::display::framebuffer::FrameBuffer::Gray4(fb) => {
                Self::render_splash_gray4(
                    fb,
                    &splash_page,
                    &self.splash_version,
                    &self.splash_build_date,
                    Some(status)
                )?;
            }
        }

        // Flush to display
        self.driver.flush()?;

        Ok(())
    }
    /// Setup weather service with background polling
    pub async fn setup_weather(&mut self, config: &str) -> Result<(), DisplayError> {
        use crate::weather::Weather;
        use log::{info, error};

        if config.is_empty() {
            info!("Weather config is empty, skipping weather setup");
            return Ok(());
        }

        info!("Setting up weather with config: {}", config);

        // Create Weather instance
        let mut weather = Weather::new(config).await
            .map_err(|e| DisplayError::InitializationFailed(format!("Failed to create Weather: {}", e)))?;

        // Initial fetch
        match weather.fetch_weather_data().await {
            Ok(_) => info!("Initial weather data fetched successfully"),
            Err(e) => error!("Failed initial weather data fetch: {}", e),
        }

        // Extract initial data to populate display manager fields
        let weather_display = weather.weather_data.get_weather_display();
        self.weather_temp_units = weather_display.temp_units.clone();
        // Location name could be fetched from coordinates if needed
        // For now, just set a placeholder
        self.weather_location_name = "Local".to_string();

        // Prepare initial weather data for component
        let mut weather_vec = vec![weather_display.current.clone()];
        weather_vec.extend(weather_display.forecasts.clone());
        self.weather_display.update(weather_vec);

        // Start polling with watch channel (lock-free!)
        let (poll_handle, weather_rx) = weather.start_polling_with_watch().await
            .map_err(|e| DisplayError::InitializationFailed(format!("Failed to start weather polling: {}", e)))?;

        // Store the receiver for updates
        self.weather_rx = Some(weather_rx);

        info!("Weather setup complete, background polling started");
        Ok(())
    }

    pub async fn test(&mut self, _run: bool) {}
    /// Setup visualizer with playing state receiver
    pub async fn setup_visualizer(
        &mut self,
        viz_type: &str,
        playing_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), DisplayError> {
        use crate::visualization::transpose_kind;

        if viz_type == "no_viz" {
            return Ok(());
        }

        // Parse visualization type
        let mut viz_kind = transpose_kind(viz_type);

        // Display-specific visualization mapping
        // On large displays (256x64+), don't stretch smaller downmix SVGs
        // Instead use all-in-one views which are designed for wider displays
        use crate::visualization::Visualization;
        use crate::display::layout::LayoutCategory;
        if matches!(self.layout.category, LayoutCategory::Large | LayoutCategory::ExtraLarge) {
            viz_kind = match viz_kind {
                Visualization::VuMono => Visualization::AioVuMono,
                _ => viz_kind,
            };
        }

        // Spawn the visualizer worker
        let visualizer = crate::visualizer::Visualizer::spawn(viz_type, playing_rx)
            .map_err(|e| DisplayError::InitializationFailed(format!("Failed to spawn visualizer: {}", e)))?;

        // Set the visualizer in the component
        self.visualizer.set_visualizer(visualizer);

        // Set the visualization type on the component
        self.visualizer.set_visualization_type(viz_kind);

        // Enable the visualizer
        if let Some(viz) = self.visualizer.visualizer() {
            viz.enable(true);
        }

        info!("Visualizer setup complete: {} ({:?})", viz_type, viz_kind);
        Ok(())
    }

    /// Check if weather is active (configured and data available)
    pub async fn is_weather_active(&self) -> bool {
        self.weather_rx.is_some()
    }
    pub fn get_egg_type(&self) -> u8 { self.easter_egg.egg_type }

    /// Render frame (called from main loop)
    pub async fn render_frame(&mut self) -> Result<(), DisplayError> {
        self.render()
    }

    /// Get emulator state for window (only available with emulator feature)
    #[cfg(feature = "emulator")]
    pub fn emulator_state(&self) -> Option<std::sync::Arc<tokio::sync::Mutex<crate::display::drivers::emulator::EmulatorState>>> {
        // Try to get the driver's inner emulator driver
        // This is a hack - ideally we'd have a better way to access driver internals
        // For now, return None and we'll handle this differently
        None
    }

    /// Cycle to next easter egg animation
    pub fn cycle_easter_egg(&mut self) {
        use crate::eggs::set_easter_egg;

        let egg_names = [
            "bass", "cassette", "ibmpc", "moog", "pipboy", "reel2reel",
            "radio40", "radio50", "scope", "technics", "tubeamp",
            "tvtime", "vcr", "none"
        ];

        // Find current egg name
        let current_type = self.easter_egg.egg_type;
        let current_idx = egg_names.iter().position(|&name| {
            let egg = set_easter_egg(name);
            egg.egg_type == current_type
        }).unwrap_or(0);

        // Get next egg
        let next_idx = (current_idx + 1) % egg_names.len();
        let next_egg_name = egg_names[next_idx];

        info!("Cycling easter egg: {} -> {}", egg_names[current_idx], next_egg_name);
        self.easter_egg = set_easter_egg(next_egg_name);

        // Switch to easter eggs mode
        self.current_mode = crate::display::DisplayMode::EasterEggs;
    }

    /// Cycle to next visualization type
    pub fn cycle_visualization(&mut self) {
        use crate::visualization::Visualization;

        let viz_types = [
            Visualization::VuStereo,
            Visualization::VuMono,
            Visualization::PeakStereo,
            Visualization::PeakMono,
            Visualization::HistStereo,
            Visualization::HistMono,
            Visualization::VuStereoWithCenterPeak,
            Visualization::AioVuMono,
            Visualization::AioHistMono,
        ];

        // Find current viz index
        let current_type = self.visualizer.visualization_type();
        let current_idx = viz_types.iter().position(|&t| {
            // Compare as debug strings since Visualization doesn't derive PartialEq
            format!("{:?}", t) == format!("{:?}", current_type)
        }).unwrap_or(0);

        // Get next viz
        let next_idx = (current_idx + 1) % viz_types.len();
        let next_viz = viz_types[next_idx];

        info!("Cycling visualization: {:?} -> {:?}", current_type, next_viz);
        self.visualizer.set_visualization_type(next_viz);

        // Switch to visualizer mode
        self.current_mode = crate::display::DisplayMode::Visualizer;
    }

    /// Check and clear easter egg cycle request from emulator
    #[cfg(feature = "emulator")]
    pub fn check_and_clear_cycle_easter_egg(&mut self) -> bool {
        if let Some(emu_state) = &self.emulator_state {
            let mut state = emu_state.lock().unwrap();
            if state.cycle_easter_egg {
                state.cycle_easter_egg = false;
                return true;
            }
        }
        false
    }

    /// Check and clear visualization cycle request from emulator
    #[cfg(feature = "emulator")]
    pub fn check_and_clear_cycle_visualization(&mut self) -> bool {
        if let Some(emu_state) = &self.emulator_state {
            let mut state = emu_state.lock().unwrap();
            if state.cycle_visualization {
                state.cycle_visualization = false;
                return true;
            }
        }
        false
    }
}
