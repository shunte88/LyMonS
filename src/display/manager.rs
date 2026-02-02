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

use log::{info, warn};
use std::time::Instant;
use arrayvec::ArrayString;
use core::fmt::Write;

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

        // Get capabilities and create layout
        let capabilities = driver.capabilities().clone();
        let layout = LayoutConfig::for_display(&capabilities);

        info!("Display: {}x{}, Layout: {:?}, Assets: {}",
              capabilities.width,
              capabilities.height,
              layout.category,
              layout.asset_path);

        // Create framebuffer
        let framebuffer = FrameBuffer::new(&capabilities);

        // Initialize components
        let status_bar = StatusBar::new(layout.clone());

        let scroll_mode_enum = crate::textable::transform_scroll_mode(scroll_mode);
        let scrolling_text = ScrollingText::new(layout.clone(), scroll_mode_enum);

        let clock_font_data = set_clock_font(clock_font);
        let clock_display = ClockDisplay::new(layout.clone(), clock_font_data);

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

        // Flush to hardware
        let transfer_start = Instant::now();
        self.driver.flush()?;
        let transfer_time = transfer_start.elapsed().as_micros() as u64;

        // Record performance metrics
        self.metrics.record_frame(render_time, transfer_time);

        Ok(())
    }

    /// Render scrolling text mode
    fn render_scrolling(&mut self) -> Result<(), DisplayError> {
        // TODO: Implement scrolling text rendering
        // - Draw status bar
        // - Draw scrolling artist/title
        // - Draw progress bar
        Ok(())
    }

    /// Render clock mode
    fn render_clock(&mut self) -> Result<(), DisplayError> {
        // TODO: Implement clock rendering
        // - Draw status bar
        // - Draw large clock digits
        // - Draw date
        Ok(())
    }

    /// Render current weather
    fn render_weather_current(&mut self) -> Result<(), DisplayError> {
        // TODO: Implement weather rendering
        // - Draw status bar
        // - Draw weather icon
        // - Draw temperature and conditions
        Ok(())
    }

    /// Render weather forecast
    fn render_weather_forecast(&mut self) -> Result<(), DisplayError> {
        // TODO: Implement forecast rendering
        // - Draw status bar
        // - Draw multi-day forecast
        Ok(())
    }

    /// Render visualizer
    fn render_visualizer(&mut self) -> Result<(), DisplayError> {
        // TODO: Implement visualizer rendering
        // - Draw VU meters or other visualizations
        Ok(())
    }

    /// Render easter eggs
    fn render_easter_eggs(&mut self) -> Result<(), DisplayError> {
        // TODO: Implement easter egg rendering
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
}
