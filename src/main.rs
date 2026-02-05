/*
 *  main.rs
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

// Currently requires driver-ssd1306 until DisplayManager is implemented (Phase 5)
#[cfg(not(feature = "driver-ssd1306"))]
compile_error!("LyMonS currently requires the 'driver-ssd1306' feature. Use --features driver-ssd1306");

#[allow(dead_code)]
#[allow(unused_imports)]
use std::{time::Duration};
use log::{info, error, warn};
use env_logger::Env;
use clap::{Arg, ArgAction, Command};
use chrono::{Timelike, Local};
use local_ip_address::{local_ip};

#[cfg(unix)] // Only compile this block on Unix-like systems
use tokio::signal::unix::{signal, SignalKind}; // Import specific Unix signals

// move these to mod.rs
//mod singles;
mod config;
mod trig;
//mod pacer;
mod dbfs;
mod draw;
mod drawsvg;
// Legacy display module (requires driver-ssd1306 feature)
#[cfg(feature = "driver-ssd1306")]
#[path = "display_old.rs"]
mod display_old;
// New modular display system
mod display;
mod mac_addr;
mod metrics;
mod const_oled;
mod constants;
mod glyphs;
mod clock_font;
mod deutils;
mod httprpc;
mod sliminfo;
mod weather;
mod textable;
mod weather_glyph;
mod geoloc;
mod location;
mod astral;
mod translate;
mod eggs;
mod spectrum;
mod vframebuf;
mod vision;
mod visualization;
mod visualizer;
//mod vuneedle;
mod vuphysics;
mod svgimage;
mod shm_path;
mod func_timer;
mod sun;

use sliminfo::LMSServer;
use mac_addr::{get_mac_addr,get_mac_addr_for};
//use singles::SingleInstance;
include!(concat!(env!("OUT_DIR"), "/build_info.rs"));

/// Asynchronously waits for a SIGINT, SIGTERM, or SIGHUP signal.
/// always unix so forget the cfg 
/// This function sets up signal handlers for common Unix termination signals
/// and waits for any of them to be received. Once a signal is caught, it logs
/// the event and returns, allowing for graceful shutdown.
/// Unified display loop that works with DisplayManager
/// Uses same has_changed() logic as hardware path
async fn unified_display_loop(
    display: std::sync::Arc<tokio::sync::Mutex<display::DisplayManager>>,
    player_name: &str,
    show_remaining: bool,
    weather_config: &str,
    viz_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::sliminfo::LMSServer;
    use std::time::Duration;

    info!("Starting unified display loop with DisplayManager");

    // Get MAC address for LMS discovery
    let mac_addr = get_mac_addr();

    // Initialize LMS server
    let lms_arc = match LMSServer::init_server(player_name, mac_addr.as_str()).await {
        Ok(server_arc) => server_arc,
        Err(e) => {
            error!("LMS Server initialization failed: {}", e);
            return Err(e);
        }
    };

    info!("LMS Server communication initialized");

    // Setup weather if configured
    if !weather_config.is_empty() {
        info!("Setting up weather with config...");
        let mut display_lock = display.lock().await;
        display_lock.setup_weather(weather_config).await?;
        drop(display_lock);
        info!("Weather setup complete");
    }

    // Setup visualizer if requested
    if viz_type != "no_viz" {
        let lms = lms_arc.lock().await;
        let mut display_lock = display.lock().await;
        display_lock.setup_visualizer(viz_type, lms.subscribe_playing()).await?;
        drop(lms);
        drop(display_lock);
    }

    info!("Setting up polling intervals and display mode");

    // Polling intervals
    let scrolling_poll_duration = Duration::from_millis(50);
    let clock_poll_duration = Duration::from_millis(100);
    let viz_poll_duration = Duration::from_millis(36);

    info!("Getting easter egg type");
    // Get easter egg type and create mode controller
    let egg_type = {
        let display_lock = display.lock().await;
        display_lock.get_egg_type()
    };

    info!("Egg type: {}", egg_type);

    // Create display mode controller
    let mode_config = display::ModeControllerConfig {
        weather_config: weather_config.to_string(),
        visualizer_type: viz_type.to_string(),
        egg_type,
        weather_interval_mins: 20,
        weather_current_duration_secs: 30,
        weather_forecast_duration_secs: 30,
    };
    let mut mode_controller = display::DisplayModeController::new(mode_config);

    info!("Entering main display loop");
    // Main loop - SAME PATTERN AS HARDWARE
    loop {
        let mut display_lock = display.lock().await;

        // Update weather active state in mode controller
        let is_weather_active = display_lock.is_weather_active().await;
        mode_controller.set_weather_active(is_weather_active);

        let mut lms_guard = lms_arc.lock().await;

        // Determine and set display mode
        let mut mode = display_lock.display_mode();
        let is_playing = lms_guard.is_playing();

        // Check if manual mode override is active (keyboard locked)
        let manual_override = display_lock.is_manual_mode_override();

        if manual_override {
            // Manual override active - check for mode requests but don't run controller
            if let Some(requested_mode) = display_lock.check_emulator_mode_request() {
                mode = requested_mode;
                info!("Emulator mode locked: {:?}", mode);
            }
        } else {
            // Automatic mode - run controller normally
            mode_controller.update_mode(is_playing);
            mode = mode_controller.current_mode();

            // Check for emulator mode requests that enable manual override
            if let Some(requested_mode) = display_lock.check_emulator_mode_request() {
                mode = requested_mode;
                info!("Emulator mode override: {:?} (manual lock enabled)", mode);
            }
        }

        // Check for emulator easter egg cycling (requires track playing)
        #[cfg(feature = "emulator")]
        if display_lock.check_and_clear_cycle_easter_egg() {
            if is_playing {
                display_lock.cycle_easter_egg();
                mode = display::DisplayMode::EasterEggs;
                info!("Easter egg cycled (track playing)");
            } else {
                info!("Easter egg cycle requested but no track playing - ignored");
            }
        }

        // Check for emulator visualization cycling (requires track playing)
        #[cfg(feature = "emulator")]
        if display_lock.check_and_clear_cycle_visualization() {
            if is_playing {
                display_lock.cycle_visualization();
                mode = display::DisplayMode::Visualizer;
                info!("Visualization cycled (track playing)");
            } else {
                info!("Visualization cycle requested but no track playing - ignored");
            }
        }

        display_lock.set_display_mode(mode);
        display_lock.update_emulator_current_mode(mode);

        // Get mode name for logging
        let this_mode = match mode {
            display::DisplayMode::Visualizer => "vizzy",
            display::DisplayMode::EasterEggs => "eggy",
            display::DisplayMode::Scrolling => "scrolling",
            display::DisplayMode::Clock => "clock",
            display::DisplayMode::WeatherCurrent => "weather_current",
            display::DisplayMode::WeatherForecast => "weather_forecast",
        };

        if is_playing {

            if display_lock.display_mode() == display::DisplayMode::Visualizer {
                display_lock.render_frame().await.unwrap_or_else(|e|
                    error!("Failed to render display frame in {} mode (no change): {}", this_mode, e));
            } else if lms_guard.has_changed() { // DIRTY FLAG CHECK - key difference!

                // --- Update display data when LMS tags have changed ---
                let current_volume_percent = lms_guard.sliminfo.volume.clone();
                let current_is_muted = current_volume_percent == 0;

                // Convert u8 repeat/shuffle to enums
                let repeat_mode = match lms_guard.sliminfo.repeat {
                    0 => crate::display_old::RepeatMode::Off,
                    1 => crate::display_old::RepeatMode::RepeatAll,
                    2 => crate::display_old::RepeatMode::RepeatOne,
                    _ => crate::display_old::RepeatMode::Off,
                };
                let shuffle_mode = match lms_guard.sliminfo.shuffle {
                    0 => crate::display_old::ShuffleMode::Off,
                    1 => crate::display_old::ShuffleMode::ByTracks,
                    2 => crate::display_old::ShuffleMode::ByAlbums,
                    _ => crate::display_old::ShuffleMode::Off,
                };

                display_lock.set_status_line_data(
                    current_volume_percent,
                    current_is_muted,
                    lms_guard.sliminfo.samplesize.clone().to_string(),
                    lms_guard.sliminfo.samplerate.clone().to_string(),
                    repeat_mode,
                    shuffle_mode,
                );

                display_lock.set_track_details(
                    lms_guard.sliminfo.albumartist.clone(),
                    lms_guard.sliminfo.album.clone(),
                    lms_guard.sliminfo.title.clone(),
                    lms_guard.sliminfo.artist.clone(),
                    "scroll_mode",
                ).await;

                display_lock.set_track_progress_data(
                    show_remaining,
                    lms_guard.sliminfo.duration.raw.clone() as f32,
                    lms_guard.sliminfo.tracktime.raw.clone() as f32,
                    lms_guard.sliminfo.remaining.raw.clone() as f32,
                    lms_guard.sliminfo.mode.clone(),
                );

                // Render the frame
                display_lock.render_frame().await.unwrap_or_else(|e|
                    error!("Failed to render display frame in {} mode: {}", this_mode, e));

                lms_guard.reset_changed(); // Reset dirty flag
            } else {
                // Not changed but playing - continue animation
                display_lock.render_frame().await.unwrap_or_else(|e|
                    error!("Failed to render display frame in {} mode (no change): {}", this_mode, e));
            }
        } else {
            // Not playing - mode controller has already set Clock or Weather mode
            display_lock.render_frame().await.unwrap_or_else(|e|
                error!("Failed to render display frame in {} mode: {}", this_mode, e));
        }

        // Determine sleep duration based on display mode
        let current_poll_duration = if display_lock.display_mode() == display::DisplayMode::Clock {
            clock_poll_duration
        } else if display_lock.display_mode() == display::DisplayMode::Visualizer {
            viz_poll_duration
        } else {
            scrolling_poll_duration
        };

        lms_guard.ask_refresh();
        drop(lms_guard);
        drop(display_lock);

        tokio::time::sleep(current_poll_duration).await;
    }
}

async fn signal_handler() -> Result<(), Box<dyn std::error::Error>> {
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sighup = signal(SignalKind::hangup())?;

    tokio::select! {
        _ = sigint.recv() => {
            info!("SIGINT received. Initiating graceful shutdown.");
        }
        _ = sigterm.recv() => {
            info!("SIGTERM received. Initiating graceful shutdown.");
        }
        _ = sighup.recv() => {
            info!("SIGHUP received. Initiating graceful shutdown.");
        }
    }
    Ok(())
}


/// Demo mode render loop (when LMS is not available) - shows clock
#[cfg(feature = "emulator")]
async fn emulator_demo_loop(
    driver: std::sync::Arc<tokio::sync::Mutex<display::drivers::emulator::EmulatorDriver>>,
    clock_font: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use embedded_graphics::prelude::*;
    use embedded_graphics::pixelcolor::BinaryColor;
    use embedded_graphics::mono_font::{MonoTextStyle, ascii::{FONT_5X8, FONT_6X10, FONT_9X18_BOLD}};
    use embedded_graphics::text::Text;
    use embedded_graphics::primitives::{Line, PrimitiveStyle};
    use display::traits::DisplayDriver;
    use chrono::Local;

    loop {
        let now = Local::now();
        let time_str = now.format("%H:%M:%S").to_string();
        let date_str = now.format("%a %b %d").to_string();

        let mut driver_lock = driver.lock().await;

        // Clear display
        <display::drivers::emulator::EmulatorDriver as DrawTarget>::clear(
            &mut *driver_lock,
            BinaryColor::Off
        )?;

        let title_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let time_style = MonoTextStyle::new(&FONT_9X18_BOLD, BinaryColor::On);

        // Title
        Text::new("LyMonS Demo Mode", Point::new(10, 8), title_style)
            .draw(&mut *driver_lock)?;

        // Separator line
        Line::new(Point::new(0, 12), Point::new(127, 12))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut *driver_lock)?;

        // Clock - centered
        Text::new(&time_str, Point::new(20, 35), time_style)
            .draw(&mut *driver_lock)?;

        // Date
        Text::new(&date_str, Point::new(20, 52), title_style)
            .draw(&mut *driver_lock)?;

        // No LMS connection indicator
        Text::new("No LMS", Point::new(2, 63), title_style)
            .draw(&mut *driver_lock)?;

        <display::drivers::emulator::EmulatorDriver as DisplayDriver>::flush(&mut *driver_lock)?;
        drop(driver_lock);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Emulator render loop - connects to LMS and renders content
#[cfg(feature = "emulator")]
async fn emulator_render_loop(
    driver: std::sync::Arc<tokio::sync::Mutex<display::drivers::emulator::EmulatorDriver>>,
    player_name: &str,
    _scroll_mode: &str,
    clock_font: &str,
    show_metrics: bool,
    _easter_egg: &str,
    _weather: &str,
    _viz_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {

    // Show remaining time by default (matches hardware default)
    let show_remaining = true;
    use embedded_graphics::prelude::*;
    use embedded_graphics::pixelcolor::BinaryColor;
    use embedded_graphics::primitives::{PrimitiveStyle, Line};
    use embedded_graphics::mono_font::{MonoTextStyle, ascii::{FONT_5X8, FONT_6X10}};
    use embedded_graphics::text::Text;
    use embedded_graphics::geometry::Size;
    use display::traits::DisplayDriver;

    info!("Emulator render loop starting...");

    // Give LMS server time to be ready
    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("Connecting to LMS server...");

    // Connect to LMS - use MAC address "" to get all players
    // Convert error to String immediately to ensure Send trait
    let lms_result = LMSServer::init_server(player_name, "").await
        .map_err(|e| format!("{}", e));

    if lms_result.is_err() {
        error!("Failed to connect to LMS. Running in demo mode.");
        info!("Continuing with demo content...");
        // Run demo mode in a separate function (this never returns)
        return emulator_demo_loop(driver, clock_font).await;
    }

    let lms_arc = lms_result.unwrap();
    info!("Connected to LMS server");
    info!("LMS server initialized, getting player info...");

    // Main render loop with player data
    let mut frame_count = 0u64;
    let mut scroll_offset_title = 0i32;
    let mut scroll_offset_artist = 0i32;
    let mut scroll_offset_album = 0i32;
    let mut last_title = String::new();
    let mut last_artist = String::new();
    let mut last_album = String::new();

    // Hybrid time tracking: Use sliminfo as source of truth, interpolate for smoothness
    let mut base_elapsed = 0.0f64;
    let mut last_update = std::time::Instant::now();
    let mut was_playing = false;

    info!("Starting real-time display update loop (live sliminfo + interpolation)...");

    loop {
        frame_count += 1;

        // Lock LMS to read status - HYBRID: sliminfo + interpolation
        let player_info = {
            let lms = lms_arc.lock().await;

            let player_name = if lms.active_player < lms.players.len() {
                lms.players[lms.active_player].player_name.clone()
            } else {
                "No Player".to_string()
            };

            // Read bitrate info
            let bitrate = lms.sliminfo.bitrate.clone();
            let samplerate = lms.sliminfo.samplerate;
            let samplesize = lms.sliminfo.samplesize;

            // Format bitrate display (e.g., "24/192" or "320k")
            let bitrate_display = if samplesize > 0 && samplerate > 0 {
                format!("{}/{}", samplesize, samplerate / 1000)
            } else if !bitrate.is_empty() {
                bitrate.clone()
            } else {
                String::new()
            };

            // Get live sliminfo values
            let lms_elapsed = lms.sliminfo.tracktime.raw;
            let duration = lms.sliminfo.duration.raw;
            let is_playing = lms.sliminfo.mode == "play";

            // HYBRID TIME CALCULATION:
            // - When sliminfo updates (value changes), use it as new base
            // - Between updates, interpolate based on elapsed time (if playing)
            // - This gives smooth updates while respecting sliminfo as source of truth

            if (lms_elapsed - base_elapsed).abs() > 0.1 || is_playing != was_playing {
                // Sliminfo updated or play state changed - reset base
                base_elapsed = lms_elapsed;
                last_update = std::time::Instant::now();
                was_playing = is_playing;
            }

            // Calculate display time with interpolation
            let display_elapsed = if is_playing {
                base_elapsed + last_update.elapsed().as_secs_f64()
            } else {
                base_elapsed
            };

            let display_remaining = (duration - display_elapsed).max(0.0);

            (
                player_name,
                lms.sliminfo.mode.clone(),
                lms.sliminfo.title.clone(),
                lms.sliminfo.artist.clone(),
                lms.sliminfo.album.clone(),
                duration,
                display_elapsed,    // Interpolated from sliminfo base
                display_remaining,  // Calculated from interpolated elapsed
                lms.sliminfo.volume,
                lms.sliminfo.repeat,
                lms.sliminfo.shuffle,
                bitrate_display,
            )
        };

        // Reset scroll if track changed AND LOG IT
        if player_info.2 != last_title {
            scroll_offset_title = 0;
            last_title = player_info.2.clone();
            info!("TRACK CHANGED: '{}' by '{}'", player_info.2, player_info.3);
        }
        if player_info.3 != last_artist {
            scroll_offset_artist = 0;
            last_artist = player_info.3.clone();
        }
        if player_info.4 != last_album {
            scroll_offset_album = 0;
            last_album = player_info.4.clone();
        }

        // Render to display
        {
            let mut driver_lock = driver.lock().await;

            // Clear display
            <display::drivers::emulator::EmulatorDriver as DrawTarget>::clear(
                &mut *driver_lock,
                BinaryColor::Off
            )?;

            let small_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
            let tiny_style = MonoTextStyle::new(&embedded_graphics::mono_font::iso_8859_13::FONT_5X8, BinaryColor::On);

            // Check if playing
            let is_playing = player_info.1 == "play" || player_info.1 == "pause";

            if is_playing {
                // ═══════════════════════════════════════════════════
                // EXACT HARDWARE DISPLAY EMULATION
                // Replicates OledDisplay::render_frame() Scrolling mode
                // ═══════════════════════════════════════════════════

                // === STATUS LINE (Y=0-9) ===
                // Volume glyph (left): Use simple text representations
                let vol_glyph = if player_info.8 == 0 { "M" } else { "\u{266A}" }; // M for mute, note for sound
                Text::new(vol_glyph, Point::new(1, 7), tiny_style)
                    .draw(&mut *driver_lock)?;

                // Volume text
                let vol_text = if player_info.8 == 0 {
                    "mute".to_string()
                } else {
                    format!("{:>3}%", player_info.8)
                };
                Text::new(&vol_text, Point::new(9, 7), tiny_style)
                    .draw(&mut *driver_lock)?;

                // Bitrate (center) - e.g., "24/192" or "320"
                if !player_info.11.is_empty() {
                    Text::new(&player_info.11, Point::new(42, 7), tiny_style)
                        .draw(&mut *driver_lock)?;
                }

                // Repeat glyph (right side)
                if player_info.9 == 1 {
                    Text::new("R", Point::new(95, 7), tiny_style)
                        .draw(&mut *driver_lock)?;
                } else if player_info.9 == 2 {
                    Text::new("R1", Point::new(92, 7), tiny_style)
                        .draw(&mut *driver_lock)?;
                }

                // Shuffle glyph
                if player_info.10 > 0 {
                    Text::new("S", Point::new(110, 7), tiny_style)
                        .draw(&mut *driver_lock)?;
                }

                // Separator line
                Line::new(Point::new(0, 9), Point::new(127, 9))
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                    .draw(&mut *driver_lock)?;

                // === TRACK DETAILS (Y=10-50) - 4 SCROLLING LINES ===
                // Hardware uses X=0 (left-justified), width=126 chars
                let title_width_chars = 25; // 126 pixels / 5 pixels per char (FONT_5X8)

                // Line 1: Album Artist (Y=18)
                // (Not in current player_info - placeholder for now)

                // Line 2: Album (Y=27)
                let album = &player_info.4;
                if album.len() > title_width_chars {
                    let extended = format!("{}   {}", album, album);
                    let text: String = extended.chars()
                        .skip(scroll_offset_album as usize)
                        .take(title_width_chars)
                        .collect();
                    Text::new(&text, Point::new(0, 27), small_style)
                        .draw(&mut *driver_lock)?;
                    if frame_count % 3 == 0 {
                        scroll_offset_album = (scroll_offset_album + 1) % (album.len() as i32 + 3);
                    }
                } else {
                    Text::new(album, Point::new(0, 27), small_style)
                        .draw(&mut *driver_lock)?;
                }

                // Line 3: Title (Y=36)
                let title = &player_info.2;
                if title.len() > title_width_chars {
                    let extended = format!("{}   {}", title, title);
                    let text: String = extended.chars()
                        .skip(scroll_offset_title as usize)
                        .take(title_width_chars)
                        .collect();
                    Text::new(&text, Point::new(0, 36), small_style)
                        .draw(&mut *driver_lock)?;
                    if frame_count % 3 == 0 {
                        scroll_offset_title = (scroll_offset_title + 1) % (title.len() as i32 + 3);
                    }
                } else {
                    Text::new(title, Point::new(0, 36), small_style)
                        .draw(&mut *driver_lock)?;
                }

                // Line 4: Artist (Y=45)
                let artist = &player_info.3;
                if artist.len() > title_width_chars {
                    let extended = format!("{}   {}", artist, artist);
                    let text: String = extended.chars()
                        .skip(scroll_offset_artist as usize)
                        .take(title_width_chars)
                        .collect();
                    Text::new(&text, Point::new(0, 45), small_style)
                        .draw(&mut *driver_lock)?;
                    if frame_count % 3 == 0 {
                        scroll_offset_artist = (scroll_offset_artist + 1) % (artist.len() as i32 + 3);
                    }
                } else {
                    Text::new(artist, Point::new(0, 45), small_style)
                        .draw(&mut *driver_lock)?;
                }

                // === PROGRESS BAR (Y=51-55, 124px wide, 4px high) ===
                let duration = player_info.5;
                let elapsed = player_info.6;
                let remaining = player_info.7;

                if duration > 0.0 {
                    // Progress bar outline (124px wide, 2px padding on each side)
                    embedded_graphics::primitives::Rectangle::new(
                        Point::new(2, 51),
                        embedded_graphics::geometry::Size::new(124, 4)
                    )
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                    .draw(&mut *driver_lock)?;

                    // Fill (interior is 122x2)
                    let progress = (elapsed / duration).clamp(0.0, 1.0);
                    let fill_width = (122.0 * progress) as u32;
                    if fill_width > 0 {
                        embedded_graphics::primitives::Rectangle::new(
                            Point::new(3, 52),
                            embedded_graphics::geometry::Size::new(fill_width, 2)
                        )
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(&mut *driver_lock)?;
                    }
                }

                // === INFO LINE (Y=56-63, BOTTOM) ===
                // Current time (left)
                let current_time_str = format!("{}:{:02}",
                    (elapsed as u32) / 60,
                    (elapsed as u32) % 60
                );
                Text::new(&current_time_str, Point::new(2, 61), tiny_style)
                    .draw(&mut *driver_lock)?;

                // Mode text (center)
                let mode_text = match player_info.1.as_str() {
                    "play" => "playing",
                    "pause" => "paused",
                    "stop" => "stopped",
                    _ => &player_info.1,
                };
                let mode_x = 50; // Approximate center
                Text::new(mode_text, Point::new(mode_x, 61), tiny_style)
                    .draw(&mut *driver_lock)?;

                // Remaining OR duration (right) - based on show_remaining flag
                let time_str = if show_remaining {
                    format!("-{}:{:02}", (remaining as u32) / 60, (remaining as u32) % 60)
                } else {
                    format!("{}:{:02}", (duration as u32) / 60, (duration as u32) % 60)
                };
                Text::new(&time_str, Point::new(100, 61), tiny_style)
                    .draw(&mut *driver_lock)?;

                // DEBUG: Log every 30 frames (~1 second) to verify updates
                if frame_count % 30 == 0 {
                    info!("Frame {}: elapsed={:.1}s, remaining={:.1}s, duration={:.1}s",
                        frame_count, elapsed, remaining, duration);
                }

            } else {
                // === IDLE MODE - Show Clock ===
                use chrono::Local;
                let now = Local::now();
                let bold_style = MonoTextStyle::new(&embedded_graphics::mono_font::iso_8859_13::FONT_9X18_BOLD, BinaryColor::On);

                // Status bar (empty when stopped)
                Text::new("\u{23F9}", Point::new(2, 7), tiny_style) // Stop glyph
                    .draw(&mut *driver_lock)?;

                // Separator
                Line::new(Point::new(0, 9), Point::new(127, 9))
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                    .draw(&mut *driver_lock)?;

                // Clock (centered, large)
                let time_str = now.format("%H:%M:%S").to_string();
                Text::new(&time_str, Point::new(20, 32), bold_style)
                    .draw(&mut *driver_lock)?;

                // Date
                let date_str = now.format("%a %b %d").to_string();
                Text::new(&date_str, Point::new(28, 50), small_style)
                    .draw(&mut *driver_lock)?;

                // Status
                Text::new("No Playback", Point::new(30, 62), tiny_style)
                    .draw(&mut *driver_lock)?;
            }

            // Flush to display
            <display::drivers::emulator::EmulatorDriver as DisplayDriver>::flush(&mut *driver_lock)?;

            drop(driver_lock);
        }

        // Sleep for frame time (30 FPS)
        tokio::time::sleep(Duration::from_millis(33)).await;
    }
}

#[tokio::main] // Requires the `tokio` runtime with `macros` and `rt-multi-thread` features
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
//    let singleton = match SingleInstance::new(env!("CARGO_PKG_NAME"))?
//    {
//        Ok(s) => s, 
//        Err(e) => ,
//    }

    // Parse command line arguments
    let matches = Command::new(env!("CARGO_PKG_NAME")) // Use Cargo.toml name
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION")) // also want build date
        .arg(Arg::new("debug")
        .action(ArgAction::SetTrue)
        .long("debug")
        .short('v')
        .alias("verbose") 
        .help("Enable debug log level")
        .required(false))
        .arg(Arg::new("name")
        .short('N')
        .long("name")
        .help("LMS player name to monitor")
        .required(true))
        .arg(Arg::new("weather")
        .short('W')
        .long("weather")
        .help("Weather API key,units,transl,latitude,longitude")
        .default_value("")
        .required(false))
        .arg(Arg::new("scroll")
        .short('z')
        .long("scroll")
        .help("Text display scroll mode")
        .value_parser(["loop", "loopleft", "cylon"])
        .default_value("cylon")
        .required(false))
        .arg(Arg::new("remain")
        .action(ArgAction::SetTrue)
        .short('r')
        .long("remain")
        .help("Display Remaining Time rather than Total Time")
        .required(false))
        .arg(Arg::new("font")
        .short('F')
        .long("font")
        .help("Clock font to use")
        .value_parser(
            ["7seg",
            "holdeco",
            "holfestus",
            "noto",
            "roboto",
            "soldeco",
            "solfestus",
            "space1999"]
            )
        .default_value("7seg")
        .required(false))
        .arg(Arg::new("eggs")
        .short('E')
        .long("eggs")
        .help("Easter Egg Animation")
        .value_parser(
            ["bass",
            "cassette",
            "ibmpc",
            "moog",
            "radio40",
            "radio50",
            "reel2reel",
            "scope",
            "technics",
            "tubeamp",
            "tvtime",
            "vcr",
            "none"]
            )
        .default_value("none")
        .required(false))
        .arg(Arg::new("no-splash")
        .long("no-splash")
        .help("Skip splash screen (shown by default)")
        .action(ArgAction::SetTrue)
        .required(false))
        .arg(Arg::new("metrics")
        .short('k')
        .long("metrics")
        .help("Display device metrics")
        .action(ArgAction::SetTrue)
        .required(false))
        .arg(Arg::new("config")
        .short('c')
        .long("config")
        .default_value("config.toml")
        .help("monitor config file")
        .required(false)) // false as defaulted
        // change this to interface/oled_interface
        .arg(Arg::new("i2c-bus")
        .long("i2c-bus")
        .default_value("/dev/i2c-1") // Default I2C bus path for Raspberry Pi
        .help("I2C bus device path for OLED display (e.g., /dev/i2c-1)")
        .required(false))
        .arg(Arg::new("emulated")
        .long("emulated")
        .help("[Internal] Emulation mode for development/testing")
        .action(ArgAction::SetTrue)
        .hide(true)
        .required(false))
        .arg(Arg::new("viz")
        .short('a')
        .long("viz")
        .help("Visualization, meters, VU, Peak, Histograms, and more")
        .value_parser(
            ["vu_stereo",    // two VU meters (L/R)
            "vu_mono",       // downmix to mono VU
            "peak_stereo",   // two peak meters with hold/decay
            "peak_mono",     // mono peak meter with hold/decay
            "hist_stereo",   // two freq. histogram "bars" (L/R)
            "hist_mono",     // mono freq. histogram "bars" (downmix)
            "combination",   // L/R VU with a central mono peak meter
            "aio_vu_mono",   // All In One with downmix VU
            "aio_hist_mono", // All In One with downmix histogram,
            "no_viz"]
            )
        .default_value("no_viz")
        .required(false))
        .after_help("LyMonS:\
            \nLMS monitor\
            \n\n\tDisplay LMS details and animations\
            \n\tClock, Weather, Meters, and more\
            \n\n\
            CONTROLS:\
            \n\ttodo.")
        .get_matches();

    let skip_splash = matches.get_flag("no-splash");
    let show_splash = !skip_splash; // Show splash by default unless --no-splash is provided
    let show_metrics = matches.get_flag("metrics");
    let show_remaining = matches.get_flag("remain");
    let debug_enabled = matches.get_flag("debug");
    let mut emulated = matches.get_flag("emulated");
    let _config_file = matches.get_one::<String>("config").unwrap();

    // Also check config file for emulated setting
    if !emulated {
        if let Ok(config_content) = std::fs::read_to_string(_config_file) {
            if let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&config_content) {
                if let Some(display) = config.get("display") {
                    if let Some(emulated_setting) = display.get("emulated") {
                        emulated = emulated_setting.as_bool().unwrap_or(false);
                    }
                }
            }
        }
    }
    let scroll_mode = matches.get_one::<String>("scroll").unwrap();
    let weather_config = matches.get_one::<String>("weather").unwrap();
    let name_filter = matches.get_one::<String>("name").unwrap();
    let clock_font = matches.get_one::<String>("font").unwrap();
    let i2c_bus_path = matches.get_one::<String>("i2c-bus").unwrap();
    let easter_egg = matches.get_one::<String>("eggs").unwrap();
    let viz_type = matches.get_one::<String>("viz").unwrap();
    
    /*
	let args = Cli::parse();
	let config = Config::get();
	let params = Params::merge(&config, &args).await?;

	run(&params).await?.render(&params)?;
	params.handle_next(args, &config)?;
    */

    // Initialize the logger with the appropriate level based on debug flag
    env_logger::Builder::from_env(Env::default().default_filter_or(if debug_enabled {"debug"}else{"info"}))
        .format_timestamp_secs()
        .init();
    
    info!("This {} worth the Squeeze", env!("CARGO_PKG_NAME"));
    info!("v.{} built {}", env!("CARGO_PKG_VERSION"), BUILD_DATE);

    // Check if emulation mode is requested
    #[cfg(feature = "emulator")]
    if emulated {
        info!("Emulation mode enabled - using DisplayManager with EmulatorDriver");

        use display::drivers::emulator::EmulatorDriver;
        use display::emulator_window::{EmulatorWindow, EmulatorWindowConfig};
        use display::traits::DisplayDriver;
        use display::DisplayManager;
        use crate::config::DisplayConfig;

        // Load config to get display specifications
        let mut display_config = DisplayConfig::default();

        // Try to load from config file
        if let Ok(config_content) = std::fs::read_to_string(_config_file) {
            if let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&config_content) {
                if let Some(display) = config.get("display") {
                    display_config = serde_yaml::from_value(display.clone())
                        .unwrap_or_default();
                }
            }
        }

        // For emulator: create EmulatorDriver with specs from config
        info!("Creating emulator with DisplayManager (unified approach)");

        // Determine display specs from config (for emulation)
        // TEMP: Testing VU meters on different displays
        let (width, height, is_grayscale, display_name) = match display_config.driver {
            Some(crate::config::DriverKind::Ssd1306) => (128, 64, false, "SSD1306"),
            Some(crate::config::DriverKind::Ssd1309) => (128, 64, false, "SSD1309"),
            Some(crate::config::DriverKind::Sh1106) => (132, 64, false, "SH1106"),
            Some(crate::config::DriverKind::Ssd1322) => (256, 64, true, "SSD1322"), // Grayscale (Gray4)
            Some(crate::config::DriverKind::SharpMemory) => (400, 240, false, "SharpMemory"),
            None => (256, 64, true, "SSD1322"), // TEMP: Testing ssd1322 grayscale (Tests 3-4)
        };

        // Create EmulatorDriver (not the actual hardware driver!)
        let mut emulator_driver: display::BoxedDriver = if is_grayscale {
            Box::new(EmulatorDriver::new_grayscale(width, height, display_name)?)
        } else {
            Box::new(EmulatorDriver::new_monochrome(width, height, display_name)?)
        };
        emulator_driver.init()?;

        // Extract capabilities, state, and then move driver into DisplayManager
        let (caps, emulator_state) = {
            // Get capabilities (clone to avoid borrow issues)
            let caps = emulator_driver.capabilities().clone();

            // Extract emulator state for window (must do before moving driver)
            // Downcast BoxedDriver to EmulatorDriver using as_any()
            let state = if let Some(emu) = emulator_driver.as_any().downcast_ref::<EmulatorDriver>() {
                emu.state()
            } else {
                return Err("Failed to downcast driver to EmulatorDriver".into());
            };

            (caps, state)
        };

        info!("Emulator: {}x{} {:?}", caps.width, caps.height, caps.color_depth);
        info!("Emulator state extracted for window");

        // Create DisplayManager with the emulator driver
        let mut display_manager = display::DisplayManager::new_with_driver(
            emulator_driver,
            scroll_mode,
            clock_font,
            show_metrics,
            easter_egg,
        )?;

        // Set emulator state for keyboard shortcuts
        display_manager.set_emulator_state(emulator_state.clone());

        info!("DisplayManager created - using unified display loop");

        // === INITIALIZATION SEQUENCE WITH SPLASH SCREEN ===
        // Load full config for location settings
        let full_config = if let Ok(config_content) = std::fs::read_to_string(_config_file) {
            serde_yaml::from_str::<config::Config>(&config_content).ok()
        } else {
            None
        };

        // Show splash screen during initialization (unless user opted out)
        display_manager.splash(
            show_splash,
            &format!("v{}", env!("CARGO_PKG_VERSION")).as_str(),
            BUILD_DATE
        ).await?;

        // Initialize location service
        let location = {
            let lat = full_config.as_ref().and_then(|c| c.latitude);
            let lng = full_config.as_ref().and_then(|c| c.longitude);

            if show_splash {
                display_manager.update_splash_status("Determining location...")?;
            }

            match location::get_location(lat, lng).await {
                Ok(loc) => {
                    info!("Location determined: {}", loc);
                    Some(loc)
                }
                Err(e) => {
                    warn!("Failed to determine location: {}", e);
                    info!("Astronomical calculations will be unavailable");
                    None
                }
            }
        };

        // Initialize astral service (if location is available)
        let _astral_service = if let Some(loc) = location.clone() {
            if show_splash {
                display_manager.update_splash_status("Calculating astronomical data...")?;
            }

            let astral = astral::AstralService::new(loc);
            let astral_data = astral.get_today();

            info!("Astronomical data calculated:");
            if let Some(sunrise) = astral_data.sunrise {
                info!("  Sunrise: {}", sunrise.format("%H:%M"));
            }
            if let Some(sunset) = astral_data.sunset {
                info!("  Sunset: {}", sunset.format("%H:%M"));
            }

            // TODO: Pass astral_service to DisplayManager for auto-brightness
            // TODO: Use astral_data for sunrise/sunset display in weather/clock pages
            Some(astral)
        } else {
            None
        };

        if show_splash {
            display_manager.update_splash_status("Initialization complete")?;
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }

        // Wrap DisplayManager in Arc<Mutex> for sharing with unified loop
        let display_arc = std::sync::Arc::new(tokio::sync::Mutex::new(display_manager));
        let display_clone = display_arc.clone();

        // Clone parameters for the unified loop
        let name_filter_clone = name_filter.to_string();
        let weather_clone = weather_config.to_string();
        let viz_clone = viz_type.to_string();

        // Spawn unified display loop in background (SAME AS HARDWARE!)
        tokio::spawn(async move {
            if let Err(e) = unified_display_loop(
                display_clone,
                &name_filter_clone,
                show_remaining,
                &weather_clone,
                &viz_clone,
            ).await {
                error!("Unified display loop error: {}", e);
            }
        });

        info!("═══════════════════════════════════════════════════");
        info!("  LyMonS Emulator - DisplayManager Mode");
        info!("═══════════════════════════════════════════════════");
        info!("  Using: Unified display loop (same as hardware)");
        info!("  Driver: {} ({}x{})", caps.width, caps.height,
            if caps.color_depth == display::ColorDepth::Monochrome { "mono" } else { "gray" });
        info!("  Layout: Adaptive based on display capabilities");
        info!("  Connecting to LMS server...");
        info!("  Close window or press Ctrl+C to exit");
        info!("═══════════════════════════════════════════════════");

        // Run window on main thread (required by winit)
        let window = EmulatorWindow::new(emulator_state, EmulatorWindowConfig::default());
        return window.run().map_err(|e| e.into());
    }

    #[cfg(not(feature = "emulator"))]
    if emulated {
        error!("Emulation mode requested but not compiled with --features emulator");
        return Err("Build with --features emulator to use emulation mode".into());
    }

    // Create DisplayManager (works with any driver from config)
    info!("Creating DisplayManager with dynamic driver loading");

    // Load display config
    let mut display_config = crate::config::DisplayConfig::default();
    if let Ok(config_content) = std::fs::read_to_string(_config_file) {
        if let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&config_content) {
            if let Some(display) = config.get("display") {
                display_config = serde_yaml::from_value(display.clone())
                    .unwrap_or_default();
            }
        }
    }

    let mut display_manager = display::DisplayManager::new(
        &display_config,
        scroll_mode,
        clock_font,
        show_metrics,
        easter_egg,
    )?;

    let inet =  local_ip().unwrap();
    let mac_addr = get_mac_addr();
    let eth0_mac_addr = get_mac_addr_for("eth0").unwrap_or_else(|_| "00:00:00:00:00:00".to_string());
    let wlan0_mac_addr = get_mac_addr_for("wlan0").unwrap_or_else(|_| "00:00:00:00:00:00".to_string());

    display_manager.connections(
        inet.to_string().as_str(),
        eth0_mac_addr.clone().as_str(),
        wlan0_mac_addr.clone().as_str()
    );

    // === INITIALIZATION SEQUENCE (HARDWARE PATH) ===
    // Load full config for location settings
    let full_config = if let Ok(config_content) = std::fs::read_to_string(_config_file) {
        serde_yaml::from_str::<config::Config>(&config_content).ok()
    } else {
        None
    };

    // Show splash screen during initialization (unless user opted out)
    display_manager.splash(
        show_splash,
        &format!("v{}",env!("CARGO_PKG_VERSION")).as_str(),
        BUILD_DATE
    ).await?;

    // Initialize location service
    let location = {
        let lat = full_config.as_ref().and_then(|c| c.latitude);
        let lng = full_config.as_ref().and_then(|c| c.longitude);

        if show_splash {
            display_manager.update_splash_status("Determining location...")?;
        }

        match location::get_location(lat, lng).await {
            Ok(loc) => {
                info!("Location determined: {}", loc);
                Some(loc)
            }
            Err(e) => {
                warn!("Failed to determine location: {}", e);
                info!("Astronomical calculations will be unavailable");
                None
            }
        }
    };

    // Initialize astral service (if location is available)
    let _astral_service = if let Some(loc) = location.clone() {
        if show_splash {
            display_manager.update_splash_status("Calculating astronomical data...")?;
        }

        let astral = astral::AstralService::new(loc);
        let astral_data = astral.get_today();

        info!("Astronomical data calculated:");
        if let Some(sunrise) = astral_data.sunrise {
            info!("  Sunrise: {}", sunrise.format("%H:%M"));
        }
        if let Some(sunset) = astral_data.sunset {
            info!("  Sunset: {}", sunset.format("%H:%M"));
        }

        // TODO: Pass astral_service to DisplayManager for auto-brightness
        // TODO: Use astral_data for sunrise/sunset display
        Some(astral)
    } else {
        None
    };

    if show_splash {
        display_manager.update_splash_status("Initialization complete")?;
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }

    if weather_config != "" {
        display_manager.setup_weather(weather_config).await?;
    }

    display_manager.test(false).await;

    // sleep duration for playing, visualizer, and easter eggs
    let scrolling_poll_duration = Duration::from_millis(50);
    // clock display sleep duration
    let clock_poll_duration = Duration::from_millis(100);
    let viz_poll_duration = Duration::from_millis(36); // ~30Hz balance I2C refresh (16=60Hz)

    // Initialize the LMS server, discover it, fetch players, init tags, and start polling
    // init_server now returns Arc<TokMutex<LMSServer>>
    let lms_arc = match LMSServer::init_server(name_filter, mac_addr.as_str()).await {
        Ok(server_arc) => server_arc,
        Err(e) => {
            error!("LMS Server initialization failed: {}", e);
            return Err(e);
        }
    };

    // The polling thread is now running in the background if init_server was successful.
    info!("LMS Server communication initialized.");

    let lms = lms_arc.lock().await;
    // TODO: Fix visualizer receiver type mismatch
    // display_manager.setup_visualizer(viz_type, lms.subscribe_playing()).await?;
    drop(lms);

    // Main application loop
    tokio::select! {
        // Handle Unix signals for graceful shutdown
        _ = signal_handler() => {
            // The signal_handler function logs the received signal.
            // Execution will proceed to the end of main, where lms_arc is dropped.
        }
        
        // Main logic loop
        _ = async {

            // clear the display before we dip into the specific display modes
            //display_manager.clear_flushable_buffer();

            let egg_type = display_manager.get_egg_type(); // static

            // Create display mode controller for hardware
            let mode_config = display::ModeControllerConfig {
                weather_config: weather_config.to_string(),
                visualizer_type: viz_type.to_string(),
                egg_type,
                weather_interval_mins: 20,
                weather_current_duration_secs: 30,
                weather_forecast_duration_secs: 30,
            };
            let mut mode_controller = display::DisplayModeController::new(mode_config);

            loop {

                let is_weather_active = display_manager.is_weather_active().await;
                mode_controller.set_weather_active(is_weather_active);

                // Acquire a lock on the LMSServer instance to access its methods and data
                let mut lms_guard = lms_arc.lock().await;

                // Determine and set display mode using controller
                let is_playing = lms_guard.is_playing();
                mode_controller.update_mode(is_playing);
                let mode = mode_controller.current_mode();
                display_manager.set_display_mode(mode);

                // Get mode name for logging
                let this_mode = match mode {
                    display::DisplayMode::Visualizer => "vizzy",
                    display::DisplayMode::EasterEggs => "eggy",
                    display::DisplayMode::Scrolling => "scrolling",
                    display::DisplayMode::Clock => "clock",
                    display::DisplayMode::WeatherCurrent => "weather_current",
                    display::DisplayMode::WeatherForecast => "weather_forecast",
                };

                if is_playing {
                    if display_manager.current_mode == display::DisplayMode::Visualizer {
                        display_manager.render_frame().await.unwrap_or_else(|e| error!("Failed to render display frame in {} mode (no change): {}", this_mode, e));
                    } else if lms_guard.has_changed() { // Only update display data if LMS tags have changed

                        // --- Line 0: Volume, Repeat/Shuffle, Bitrate/Audio Glyphs ---
                        let current_volume_percent = lms_guard.sliminfo.volume.clone();
                        let current_is_muted = current_volume_percent == 0;

                        // Convert u8 repeat/shuffle to enums
                        let repeat_mode = match lms_guard.sliminfo.repeat {
                            0 => crate::display_old::RepeatMode::Off,
                            1 => crate::display_old::RepeatMode::RepeatAll,
                            2 => crate::display_old::RepeatMode::RepeatOne,
                            _ => crate::display_old::RepeatMode::Off,
                        };
                        let shuffle_mode = match lms_guard.sliminfo.shuffle {
                            0 => crate::display_old::ShuffleMode::Off,
                            1 => crate::display_old::ShuffleMode::ByTracks,
                            2 => crate::display_old::ShuffleMode::ByAlbums,
                            _ => crate::display_old::ShuffleMode::Off,
                        };

                        display_manager.set_status_line_data(
                            current_volume_percent,
                            current_is_muted,
                            lms_guard.sliminfo.samplesize.clone().to_string(),
                            lms_guard.sliminfo.samplerate.clone().to_string(),
                            repeat_mode,
                            shuffle_mode,
                        );

                        // Lines 1-4: Track Details with Scrolling
                        display_manager.set_track_details(
                            lms_guard.sliminfo.albumartist.clone(), 
                            lms_guard.sliminfo.album.clone(), 
                            lms_guard.sliminfo.title.clone(), 
                            lms_guard.sliminfo.artist.clone(),
                            scroll_mode
                        ).await;

                        // use raw_data - higher fidelity
                        display_manager.set_track_progress_data(
                            show_remaining,
                            lms_guard.sliminfo.duration.raw.clone() as f32,
                            lms_guard.sliminfo.tracktime.raw.clone() as f32,
                            lms_guard.sliminfo.remaining.raw.clone() as f32,
                            lms_guard.sliminfo.mode.clone(),
                        );

                        // Render the frame, which includes updating scroll positions and drawing
                        display_manager.render_frame().await.unwrap_or_else(|e| error!("Failed to render display frame in {} mode: {}", this_mode, e));

                        // Request a refresh for the next LMS polling cycle
                        lms_guard.reset_changed(); // Reset changed flags after display update
                    } else {
                        // If not changed, but playing, just render the current animation frame.
                        // This allows ongoing scrolling animations to continue without new data.
                        display_manager.render_frame().await.unwrap_or_else(|e| error!("Failed to render display frame in {} mode (no change): {}", this_mode, e));
                    }
                } else {

                    // When not playing - mode controller has already set Clock or Weather mode
                    // Render the frame. The required update and draw method will be called
                    display_manager.render_frame().await.unwrap_or_else(|e| error!("Failed to render display frame in {} mode: {}", this_mode, e));

                }
                
                // Determine sleep duration based on the current display mode
                let current_poll_duration = if display_manager.current_mode == display::DisplayMode::Clock {
                    clock_poll_duration
                } else if display_manager.current_mode == display::DisplayMode::Visualizer {
                    viz_poll_duration
                } else {
                    scrolling_poll_duration
                };

                // Ensure LMS server data is refreshed
                lms_guard.ask_refresh();
                // Release the lock before yielding to the Tokio runtime
                drop(lms_guard); 
                tokio::time::sleep(current_poll_duration).await; // Wait for appropriate period
            }
        } => {
            // This branch executes if the internal loop breaks (e.g., due to timeout)
            info!("Closed Application Loop.");
        }
    }

    info!("Main application exiting. Clearing display and stopping polling thread.");

    // Clear the display on shutdown
    // display_manager.clear_flushable_buffer(); // Not needed with DisplayManager

    // When `lms_arc` goes out of scope here (at the end of main),
    // its `Drop` implementation will be called, which will attempt to stop the background polling thread.
    // We can also explicitly drop it here for clarity, though it's not strictly necessary.
    drop(lms_arc);

    Ok(())

}

