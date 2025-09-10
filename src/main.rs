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

#[allow(dead_code)]
#[allow(unused_imports)]
use std::{time::Duration};
use log::{info, error};
use env_logger::Env;
//use clap::Parser;
use clap::{Arg, ArgAction, Command};
use chrono::{Timelike, Local};
use local_ip_address::{local_ip};

#[cfg(unix)] // Only compile this block on Unix-like systems
use tokio::signal::unix::{signal, SignalKind}; // Import specific Unix signals

// move these to mod.rs
//mod singles;
mod draw;
mod display;
mod mac_addr;
mod metrics;
mod constants;
mod imgdata;
mod clock_font;
mod deutils;
mod httprpc;
mod sliminfo;
mod weather;
mod textable;
mod climacell;
mod geoloc;
mod translate;
mod eggs;
mod spectrum;
mod vision;
mod visualizer;
mod vuphysics;
mod svgimage;
mod shm_path;
mod func_timer;

use sliminfo::LMSServer;
use mac_addr::{get_mac_addr,get_mac_addr_for};
//use singles::SingleInstance;
include!(concat!(env!("OUT_DIR"), "/build_info.rs"));

/// Asynchronously waits for a SIGINT, SIGTERM, or SIGHUP signal.
/// always unix so forget the cfg 
/// This function sets up signal handlers for common Unix termination signals
/// and waits for any of them to be received. Once a signal is caught, it logs
/// the event and returns, allowing for graceful shutdown.
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

fn check_half_hour(test:&String, active: bool) -> u8 {

    if test == "" || !active {
        return 0;
    }

    let now = Local::now();
    let minute = now.minute();
    let second = now.second();
    if minute == 5 || minute == 25 || minute == 45 {
        if second < 30 {
            1
        } else {
            2
        }
    } else {
        0
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
        .help("Display Remaining Time rather than Totsl Time")
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
        .arg(Arg::new("splash")
        .short('S')
        .long("splash")
        .help("Display splash screen") 
        .action(ArgAction::SetTrue)
        .required(false))
        .arg(Arg::new("metrics")
        .short('k')
        .long("metrics")
        .help("Display device metrics") 
        .action(ArgAction::SetTrue)
        .default_value("")
        .required(false))
        .arg(Arg::new("config")
        .short('c')
        .long("config")
        .default_value("config.toml")
        .help("monitor config file")
        .required(false)) // false as defaulted
        .arg(Arg::new("i2c-bus")
        .long("i2c-bus")
        .default_value("/dev/i2c-1") // Default I2C bus path for Raspberry Pi
        .help("I2C bus device path for OLED display (e.g., /dev/i2c-1)")
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
            "hist_stereo",   // two histogram bars (L/R)
            "hist_mono",     // mono histogram (downmix)
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

    let show_splash = matches.get_flag("splash");
    let show_metrics = matches.get_flag("metrics");
    let show_remaining = matches.get_flag("remain");
    let debug_enabled = matches.get_flag("debug");
    let _config_file = matches.get_one::<String>("config").unwrap();
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

    let mut oled_display = display::OledDisplay::new(
        i2c_bus_path, 
        scroll_mode, 
        clock_font,
        show_metrics, 
        easter_egg)?;

    let inet =  local_ip().unwrap();
    let mac_addr = get_mac_addr();
    let eth0_mac_addr = get_mac_addr_for("eth0").unwrap();
    let wlan0_mac_addr = get_mac_addr_for("wlan0").unwrap();

    oled_display.connections(
        inet.to_string().as_str(),
        eth0_mac_addr.clone().as_str(),
        wlan0_mac_addr.clone().as_str()
    );
 
    oled_display.splash(
        show_splash,
        &format!("v{}",env!("CARGO_PKG_VERSION")).as_str(),
        BUILD_DATE
    ).await?;

    if weather_config != "" {
        oled_display.setup_weather(weather_config).await?;
    }
    
    oled_display.test(false).await;

    // Initialize the LMS server, discover it, fetch players, init tags, and start polling
    // init_server now returns Arc<TokMutex<LMSServer>>
    let lms_arc = match LMSServer::init_server(name_filter, mac_addr.as_str()).await {
        Ok(server_arc) => server_arc,
        Err(e) => {
            error!("LMS Server initialization failed: {}", e);
            return Err(e);
        }
    };

    // sleep duration for playing, visualizer, and easter eggs
    let scrolling_poll_duration = Duration::from_millis(50);
    // clock display sleep duration
    let clock_poll_duration = Duration::from_millis(100);
    let viz_poll_duration = Duration::from_millis(32); // > 30Hz (16=60Hz)

    // The polling thread is now running in the background if init_server was successful.
    info!("LMS Server communication initialized.");

    let lms = lms_arc.lock().await;
    oled_display.setup_visualizer(viz_type, lms.subscribe_playing()).await?;
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
            //oled_display.clear_flushable_buffer();

            let egg_type = oled_display.get_egg_type(); // static

            loop {

                let is_weather_active = oled_display.is_weather_active().await;
                let wc_chk = check_half_hour(weather_config, is_weather_active);

                // Acquire a lock on the LMSServer instance to access its methods and data
                let mut lms_guard = lms_arc.lock().await;

                if lms_guard.is_playing() {

                    let mut this_mode = "scrolling";

                    if egg_type == eggs::EGGS_TYPE_UNKNOWN {
                        if viz_type == "no_viz" {
                            oled_display.set_display_mode(display::DisplayMode::Scrolling).await; // Set mode
                        } else {
                            oled_display.set_display_mode(display::DisplayMode::Visualizer).await; // Set mode
                            this_mode = "vizzy";
                        }
                    } else {
                        oled_display.set_display_mode(display::DisplayMode::EasterEggs).await; // Set mode
                        this_mode = "eggy"
                    }
                    if oled_display.current_mode == display::DisplayMode::Visualizer {
                        oled_display.render_frame().await.unwrap_or_else(|e| error!("Failed to render display frame in {} mode (no change): {}", this_mode, e));
                    } else if lms_guard.has_changed() { // Only update display data if LMS tags have changed

                        // --- Line 0: Volume, Repeat/Shuffle, Bitrate/Audio Glyphs ---
                        let current_volume_percent = lms_guard.sliminfo.volume.clone();
                        let current_is_muted = current_volume_percent == 0;

                        oled_display.set_status_line_data(
                            current_volume_percent,
                            current_is_muted,
                            lms_guard.sliminfo.samplesize.clone().to_string(),
                            lms_guard.sliminfo.samplerate.clone().to_string(),
                            lms_guard.sliminfo.repeat.clone(),
                            lms_guard.sliminfo.shuffle.clone(),
                        );

                        // Lines 1-4: Track Details with Scrolling
                        oled_display.set_track_details(
                            lms_guard.sliminfo.albumartist.clone(), 
                            lms_guard.sliminfo.album.clone(), 
                            lms_guard.sliminfo.title.clone(), 
                            lms_guard.sliminfo.artist.clone(),
                            scroll_mode
                        ).await;

                        // use raw_data - higher fidelity
                        oled_display.set_track_progress_data(
                            show_remaining,
                            lms_guard.sliminfo.duration.raw.clone() as f32,
                            lms_guard.sliminfo.tracktime.raw.clone() as f32,
                            lms_guard.sliminfo.remaining.raw.clone() as f32,
                            lms_guard.sliminfo.mode.clone(),
                        );

                        // Render the frame, which includes updating scroll positions and drawing
                        oled_display.render_frame().await.unwrap_or_else(|e| error!("Failed to render display frame in {} mode: {}", this_mode, e));

                        // Request a refresh for the next LMS polling cycle
                        lms_guard.reset_changed(); // Reset changed flags after display update
                    } else {
                        // If not changed, but playing, just render the current animation frame.
                        // This allows ongoing scrolling animations to continue without new data.
                        oled_display.render_frame().await.unwrap_or_else(|e| error!("Failed to render display frame in {} mode (no change): {}", this_mode, e));
                    }
                } else {

                    // When not playing, display the digital clock and if configured
                    // weather at intervals
                    if wc_chk == 0 {
                        oled_display.set_display_mode(display::DisplayMode::Clock).await; // Set clock mode

                    } else if wc_chk == 1 {
                        oled_display.set_display_mode(display::DisplayMode::WeatherCurrent).await; // Set current weather mod

                    } else {
                        oled_display.set_display_mode(display::DisplayMode::WeatherForecast).await; // Set 3 day forecast mode
                    }
                    // Render the frame. 
                    // The required update and draw method will be called
                    oled_display.render_frame().await.unwrap_or_else(|e| error!("Failed to render display frame in clock/weather mode: {}", e));

                }
                
                // Determine sleep duration based on the current display mode
                let current_poll_duration = if oled_display.current_mode == display::DisplayMode::Clock {
                    clock_poll_duration
                } else if oled_display.current_mode == display::DisplayMode::Visualizer {
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
    oled_display.clear_flushable_buffer();

    // When `lms_arc` goes out of scope here (at the end of main),
    // its `Drop` implementation will be called, which will attempt to stop the background polling thread.
    // We can also explicitly drop it here for clarity, though it's not strictly necessary.
    drop(lms_arc);

    Ok(())

}

