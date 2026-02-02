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
};
use crate::textable::ScrollMode;
use crate::clock_font::{ClockFontData, set_clock_font};
use crate::eggs::{Eggs, set_easter_egg};
use crate::metrics::MachineMetrics;
use crate::vision::LastVizState;
use crate::display_old::{RepeatMode, ShuffleMode};

use log::{info, warn};
use std::time::Instant;
use arrayvec::ArrayString;
use core::fmt::Write;
use embedded_graphics::pixelcolor::BinaryColor;
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

    /// Easter egg animations
    pub easter_egg: Eggs,

    /// Whether to show system metrics
    pub show_metrics: bool,

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

    /// Performance metrics
    pub metrics: PerformanceMetrics,

    /// Pre-allocated render buffers (zero allocations in render loop!)
    render_buffers: RenderBuffers,
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
            easter_egg,
            show_metrics,
            device_metrics: MachineMetrics::default(),
            last_viz_state: LastVizState::default(),
            track_duration_secs: 0.0,
            current_track_time_secs: 0.0,
            remaining_time_secs: 0.0,
            mode_text: String::new(),
            show_remaining: false,
            metrics,
            render_buffers: RenderBuffers::default(),
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
        if let (Some(artist_field), Some(album_field), Some(title_field)) = (
            page.get_field("artist"),
            page.get_field("album"),
            page.get_field("title")
        ) {
            self.scrolling_text.update_with_fields(artist_field, album_field, title_field);
        }

        // Render each field separately to manage borrows correctly
        let fb = self.framebuffer.as_mono_mut();

        for field in page.fields() {
            match field.name.as_str() {
                "status_bar" => {
                    self.status_bar.render_field(field, fb)
                        .map_err(|_| DisplayError::DrawingError("Failed to render status bar".to_string()))?;
                }
                "artist" | "album" | "title" => {
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
                    use embedded_graphics::mono_font::{ascii::FONT_5X8, MonoTextStyle};
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
        use embedded_graphics::mono_font::{ascii::FONT_5X8, MonoTextStyle};
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
        use embedded_graphics::mono_font::{ascii::FONT_5X8, MonoTextStyle};
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
            use embedded_graphics::mono_font::{ascii::FONT_5X8, MonoTextStyle};
            use embedded_graphics::text::{Text, Baseline};
            use embedded_graphics::prelude::*;

            let fb = self.framebuffer.as_mono_mut();
            let metrics_y = 2;
            let style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);

            // Format metrics string
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

            Text::with_baseline(
                metrics_str,
                Point::new(x, metrics_y as i32),
                style,
                Baseline::Top,
            )
            .draw(fb)
            .map_err(|_| DisplayError::DrawingError("Failed to draw metrics".to_string()))?;
        }
        Ok(())
    }
    
    /// Render clock mode
    fn render_clock(&mut self) -> Result<(), DisplayError> {
        // Get the clock page layout
        let page = self.layout_manager.create_clock_page();

        // Extract current second for progress bar
        let current_second: u32 = chrono::Local::now().format("%S").to_string().parse().unwrap_or(0);

        // Render each field
        for field in page.fields() {
            match field.name.as_str() {
                "metrics" => {
                    self.render_metrics()
                        .map_err(|_| DisplayError::DrawingError("Failed to render metrics".to_string()))?;
                }
                "clock_digits" => {
                    // Get framebuffer for this field
                    let fb = self.framebuffer.as_mono_mut();
                    // Clock renders digits only (no progress bar)
                    self.clock_display.render(fb)
                        .map_err(|_| DisplayError::DrawingError("Failed to render clock".to_string()))?;
                }
                "seconds_progress" => {
                    // Render seconds progress bar in its own field
                    use embedded_graphics::primitives::{Rectangle as EgRectangle, PrimitiveStyleBuilder};
                    use embedded_graphics::prelude::*;

                    let fb = self.framebuffer.as_mono_mut();
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
                // metrics...
                "date" => {
                    // Render date text centered at bottom
                    use embedded_graphics::mono_font::MonoTextStyle;
                    use embedded_graphics::text::{Text, Baseline};
                    use embedded_graphics::prelude::*;
                    use chrono::Local;

                    let fb = self.framebuffer.as_mono_mut();
                    let field_pos = field.position();
                    let field_width = field.width();
                    let font = field.font.unwrap_or(&embedded_graphics::mono_font::ascii::FONT_6X10);
                    let style = MonoTextStyle::new(font, field.fg_binary());

                    // Format date as "Day Mon DD"
                    let date_str = Local::now().format("%a %b %d").to_string();

                    // Center the text
                    let text_width = date_str.len() * 6; // Approximate width
                    let x = field_pos.x + ((field_width as i32 - text_width as i32) / 2);

                    Text::with_baseline(
                        &date_str,
                        Point::new(x, field_pos.y),
                        style,
                        Baseline::Top,
                    )
                    .draw(fb)
                    .map_err(|_| DisplayError::DrawingError("Failed to draw date".to_string()))?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Render current weather
    fn render_weather_current(&mut self) -> Result<(), DisplayError> {
        // Get mutable reference to the underlying framebuffer
        let fb = self.framebuffer.as_mono_mut();

        // Render weather using the weather_display component
        self.weather_display.render(fb)
            .map_err(|_| DisplayError::DrawingError("Failed to render weather".to_string()))?;

        Ok(())
    }

    /// Render weather forecast
    fn render_weather_forecast(&mut self) -> Result<(), DisplayError> {
        // Get mutable reference to the underlying framebuffer
        let fb = self.framebuffer.as_mono_mut();

        // Render forecast using the weather_display component
        self.weather_display.render(fb)
            .map_err(|_| DisplayError::DrawingError("Failed to render forecast".to_string()))?;

        Ok(())
    }

    /// Render visualizer
    fn render_visualizer(&mut self) -> Result<(), DisplayError> {
        // Get mutable reference to the underlying framebuffer
        let fb = self.framebuffer.as_mono_mut();

        // Render visualizer using the visualizer component
        self.visualizer.render(fb)
            .map_err(|_| DisplayError::DrawingError("Failed to render visualizer".to_string()))?;

        Ok(())
    }

    /// Render easter eggs
    fn render_easter_eggs(&mut self) -> Result<(), DisplayError> {
        // TODO: Implement easter egg rendering with field-based system
        // Easter eggs use SVG rendering which needs special handling
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
    }

    /// Set track details (artist, album, title, album_artist)
    pub async fn set_track_details(
        &mut self,
        _album_artist: String,
        album: String,
        title: String,
        artist: String,
        _scroll_mode: &str,
    ) {
        self.scrolling_text.set_full_track_info(artist, title, album);
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
    pub async fn splash(&mut self, _show: bool, _version: &str, _build_date: &str) -> Result<(), DisplayError> { Ok(()) }
    pub async fn setup_weather(&mut self, _config: &str) -> Result<(), DisplayError> { Ok(()) }
    pub async fn test(&mut self, _run: bool) {}
    pub async fn setup_visualizer(&mut self, _viz_type: &str, _receiver: tokio::sync::watch::Receiver<Option<crate::visualizer::VizFrameOut>>) -> Result<(), DisplayError> { Ok(()) }
    pub async fn is_weather_active(&self) -> bool { false }
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
}
