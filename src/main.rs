use clap::{Arg, ArgAction, Command};
use log::{debug, info, error};
use env_logger::Env;
use std::time::{Duration};
#[cfg(unix)] // Only compile this block on Unix-like systems
use tokio::signal::unix::{signal, SignalKind}; // Import specific Unix signals

include!(concat!(env!("OUT_DIR"), "/build_info.rs"));

mod clock_font; // clock fonts impl. and usage
mod imgdata;    // imgdata module, glyphs and graphics
mod constants;  // application constants
mod deutils;    // deserialize helpers
mod sliminfo;   // sliminfo - LMSServer and support
mod httprpc;    // http and [json]rpc helpers
mod display;    // display setup and primitives

// Import LMSServer directly, as init_server will return the instance
use sliminfo::{LMSServer, TagID}; // Also import TagID for easy access to tags
// Import DisplayMode along with other enums
use display::{OledDisplay};

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

#[tokio::main] // Requires the `tokio` runtime with `macros` and `rt-multi-thread` features
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    debug!("{} vers. {}",env!("CARGO_PKG_NAME"),env!("CARGO_PKG_VERSION"));
    // Parse command line arguments
    let matches = Command::new(env!("CARGO_PKG_NAME")) // Use Cargo.toml name
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION")) // also want build date
        .arg(Arg::new("debug")
        .action(ArgAction::SetTrue)
        .long("debug")
        .short('v')
        .alias("verbose") // Use alias for verbose
        .help("Enable debug log level")
        .required(false))
        .arg(Arg::new("name")
        .short('N')
        .long("name")
        .help("LMS player name to monitor")
        .required(true))
        .arg(Arg::new("scroll")
        .short('z')
        .long("scroll")
        .help("Text display scroll mode")
        .value_parser(["loop", "cylon"])
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
            ["space1999",
            "holfestus",
            "solfestus",
            "holdeco",
            "soldeco",
            "noto",
            "roboto",
            "7seg"]
            )
        .default_value("7seg")
        .required(false))
        .arg(Arg::new("splash")
        .short('S')
        .long("splash")
        .help("Display splash screen") 
        .action(ArgAction::SetTrue)
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
        .after_help("LyMonR:\
            \nLMS monitor\
            \n\n\tDisplay LMS details and animations\
            \n\tClock, Weather and more\
            \n\n\
            CONTROLS:\
            \n\ttodo.")
        .get_matches();

    let show_splash = matches.get_flag("splash");
    let show_remaining = matches.get_flag("remain");
    let debug_enabled = matches.get_flag("debug");
    let _config_file = matches.get_one::<String>("config").unwrap();
    let scroll_mode = matches.get_one::<String>("scroll").unwrap();
    let name_filter = matches.get_one::<String>("name").unwrap();
    let clock_font = matches.get_one::<String>("font").unwrap();
    let i2c_bus_path = matches.get_one::<String>("i2c-bus").unwrap();
    
    // Initialize the logger with the appropriate level based on debug flag
    env_logger::Builder::from_env(Env::default().default_filter_or(if debug_enabled {"debug"}else{"info"}))
        .format_timestamp_secs()
        .init();
    
    info!("{} v{} built {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), BUILD_DATE);

    // Initialize the OLED display
    let mut oled_display = match OledDisplay::new(i2c_bus_path) {
        Ok(display) => {
            info!("OLED display initialized.");
            display
        },
        Err(e) => {
            error!("Failed to initialize OLED display: {}", e);
            return Err(e);
        }
    };

    oled_display.splash(
        show_splash,
        &format!("v{}",env!("CARGO_PKG_VERSION")).as_str(),
        BUILD_DATE
    ).unwrap();

    if clock_font != "7seg" {
        oled_display.set_clock_font(clock_font);
    }
    if scroll_mode != "standard" {
        oled_display.set_scroll_mode(scroll_mode);
    }

    // Initialize the LMS server, discover it, fetch players, init tags, and start polling
    // init_server now returns Arc<Mutex<LMSServer>>
    let lms_arc = match LMSServer::init_server(name_filter).await {
        Ok(server_arc) => server_arc,
        Err(e) => {
            error!("LMS Server initialization failed: {}", e);
            return Err(e);
        }
    };

    // sleep duration for playing, visualizer, and easter eggs
    let scrolling_poll_duration = Duration::from_millis(50);
    // clock display sleep duration
    let clock_poll_duration = Duration::from_millis(500);

    // The polling thread is now running in the background if init_server was successful.
    info!("LMS Server communication initialized.");

    // Main application loop
    tokio::select! {
        // Handle Unix signals for graceful shutdown
        _ = signal_handler() => {
            // The signal_handler function logs the received signal.
            // Execution will proceed to the end of main, where lms_arc is dropped.
        }
        
        // Main logic loop
        _ = async {
            // clear the display before we dip into
            // the specifiuc displayods
            oled_display.clear();
            oled_display.flush().unwrap();
            loop {
                // Acquire a lock on the LMSServer instance to access its methods and data
                let mut lms_guard = lms_arc.lock().await;

                if lms_guard.is_playing() {
                    oled_display.set_display_mode(display::DisplayMode::Scrolling); // Set mode
                    
                    // Only update display data if LMS tags have changed
                    if lms_guard.has_changed() {

                        // --- Line 0: Volume, Repeat/Shuffle, Bitrate/Audio Glyphs ---
                        let current_volume_raw = lms_guard.tags[TagID::VOLUME as usize].raw_value.clone();
                        let current_volume_percent = if current_volume_raw == "-999" {
                            0 // Muted, internally use 0
                        } else {
                            current_volume_raw.parse::<u8>().unwrap_or(0) // Parse volume percent
                        };
                        let current_is_muted = current_volume_raw == "-999" || current_volume_raw == "0";

                        let current_repeat = {
                            let repeat_val = lms_guard.tags[TagID::REPEAT as usize].raw_value.parse::<i16>().unwrap_or(0);
                            if repeat_val == 2 { display::RepeatMode::RepeatOne }
                            else if repeat_val == 1 { display::RepeatMode::RepeatAll }
                            else { display::RepeatMode::Off }
                        };

                        let current_shuffle = {
                            let shuffle_val = lms_guard.tags[TagID::SHUFFLE as usize].raw_value.parse::<i16>().unwrap_or(0);                            
                            if shuffle_val == 2 { display::ShuffleMode::ByAlbums }
                            else if shuffle_val == 1 { display::ShuffleMode::ByTracks }
                            else { display::ShuffleMode::Off }
                        };

                        oled_display.set_status_line_data(
                            current_volume_percent,
                            current_is_muted,
                            lms_guard.tags[TagID::SAMPLESIZE as usize].display_value.clone(),
                            lms_guard.tags[TagID::SAMPLERATE as usize].display_value.clone(),
                            current_repeat,
                            current_shuffle,
                        );

                        // Lines 1-4: Track Details with Scrolling
                        oled_display.set_track_details(
                            lms_guard.tags[TagID::ALBUMARTIST as usize].display_value.clone(), 
                            lms_guard.tags[TagID::ALBUM as usize].display_value.clone(), 
                            lms_guard.tags[TagID::TITLE as usize].display_value.clone(), 
                            lms_guard.tags[TagID::ARTIST as usize].display_value.clone(), 
                        );

                        oled_display.set_track_progress_data(
                            show_remaining,
                            lms_guard.tags[TagID::DURATION as usize].raw_value.parse::<f32>().unwrap_or(0.00),
                            lms_guard.tags[TagID::TIME as usize].raw_value.parse::<f32>().unwrap_or(0.00),
                            lms_guard.tags[TagID::REMAINING as usize].raw_value.parse::<f32>().unwrap_or(0.00),
                            lms_guard.tags[TagID::MODE as usize].display_value.clone(),
                        );

                        // Render the frame, which includes updating scroll positions and drawing
                        oled_display.render_frame().unwrap_or_else(|e| error!("Failed to render display frame in scrolling mode: {}", e));

                        // Request a refresh for the next LMS polling cycle
                        lms_guard.reset_changed(); // Reset changed flags after display update
                    } else {
                        // If not changed, but playing, just render the current animation frame.
                        // This allows ongoing scrolling animations to continue without new data.
                        oled_display.render_frame().unwrap_or_else(|e| error!("Failed to render display frame in scrolling mode (no change): {}", e));
                    }
                } else {
                    // When not playing, display the digital clock
                    oled_display.set_display_mode(display::DisplayMode::Clock); // Set mode

                    // Render the clock frame. The clock's `render_frame` will call `update_and_draw_clock`
                    // internally, which handles its own partial updates and flushing.
                    oled_display.render_frame().unwrap_or_else(|e| error!("Failed to render display frame in clock mode: {}", e));

                }

                // ew cki soeop                
                // Determine sleep duration based on the current display mode
                let current_poll_duration = if oled_display.current_mode == display::DisplayMode::Clock {
                    clock_poll_duration
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
    oled_display.clear();
    oled_display.flush()?;

    // When `lms_arc` goes out of scope here (at the end of main),
    // its `Drop` implementation will be called, which will attempt to stop the background polling thread.
    // We can also explicitly drop it here for clarity, though it's not strictly necessary.
    drop(lms_arc);

    Ok(())
}

