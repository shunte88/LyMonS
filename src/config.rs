/*
 *  config.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Single source of truth for all CLI arguments and config-file settings.
 *  Priority (highest wins): CLI flags → config file → defaults
 *
 */

use clap::{ArgAction, Parser, ValueHint};
use dirs_next::home_dir;
use std::{fs, path::{Path, PathBuf}};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct WeatherConfig {
    pub api:       Option<String>,  // Tomorrow.io API key
    pub units:     Option<String>,  // "metric" | "imperial"
    pub translate: Option<String>,  // language/translation code
    pub latitude:  Option<f64>,
    pub longitude: Option<f64>,
}

impl WeatherConfig {
    /// True when an API key is present and non-empty.
    pub fn is_active(&self) -> bool {
        self.api.as_deref().map(|k| !k.is_empty()).unwrap_or(false)
    }

    /// Normalise units to the string Tomorrow.io expects.
    pub fn normalised_units(&self) -> String {
        match self.units.as_deref().unwrap_or("metric").to_lowercase().as_str() {
            "f" | "fahrenheit" | "imperial" => "imperial".to_string(),
            _ => "metric".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DisplayConfig {
    pub width:      Option<u32>,
    pub height:     Option<u32>,
    pub rotate_deg: Option<u16>,
    pub invert:     Option<bool>,
    pub brightness: Option<u8>,
    pub driver:     Option<DriverKind>,
    pub bus:        Option<BusConfig>,
    pub emulated:   Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BusConfig {
    I2c {
        bus:       String,
        address:   u8,
        speed_hz:  Option<u32>,
    },
    Spi {
        bus:      String,
        dc_pin:   u32,
        rst_pin:  Option<u32>,
        cs_pin:   Option<u32>,
        speed_hz: Option<u32>,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriverKind {
    Ssd1306,
    Ssd1309,
    Ssd1322,
    Sh1106,
    Sh1122,
    SharpMemory,
    St7789,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Config {
    pub log_level:      Option<String>,  // "info" | "debug"
    pub player:         Option<String>,  // LMS player name to monitor
    pub text_font:      Option<String>,  // TTF font name (zip in ./data/)
    pub text_font_size: Option<f32>,     // TTF font size in points - defaults to 9.0
    pub scroll_mode:    Option<String>,  // "cylon" | "loop" | "loopleft"
    pub show_remaining: Option<bool>,
    pub clock_font:     Option<String>,
    pub easter_egg:     Option<String>,
    pub visualizer:     Option<String>,
    pub hist_scheme:    Option<String>,  // "classic" | "ocean" | "fire" | "neon"
    pub show_metrics:   Option<bool>,
    pub show_splash:    Option<bool>,
    pub i2c_bus:        Option<String>,
    /// Standalone lat/lon — fallback for astral when weather is not configured.
    pub latitude:       Option<f64>,
    pub longitude:      Option<f64>,
    pub display:        Option<DisplayConfig>,
    pub weather:        Option<WeatherConfig>,
}

impl Config {
    /// Resolve lat/lon: weather config first, standalone fallback second.
    /// Returns `(None, None)` when neither is set — callers should GeoIP.
    pub fn effective_lat_lng(&self) -> (Option<f64>, Option<f64>) {
        let wlat = self.weather.as_ref().and_then(|w| w.latitude);
        let wlon = self.weather.as_ref().and_then(|w| w.longitude);
        (wlat.or(self.latitude), wlon.or(self.longitude))
    }

    /// Return a WeatherConfig suitable for Weather::new(), with lat/lon
    /// back-populated from the standalone fields if the weather block omits them.
    /// Returns None if no API key is configured.
    pub fn effective_weather(&self) -> Option<WeatherConfig> {
        let mut wc = self.weather.clone()?;
        if !wc.is_active() {
            return None;
        }
        if wc.latitude.is_none() || wc.longitude.is_none() {
            let (lat, lon) = self.effective_lat_lng();
            if wc.latitude.is_none()  { wc.latitude  = lat; }
            if wc.longitude.is_none() { wc.longitude = lon; }
        }
        Some(wc)
    }
}

#[derive(Debug, Parser, Clone)]
#[command(
    name    = "LyMonS",
    about   = "LMS monitor — worth the squeeze",
    version,
    author,
    after_help = ""
)]
pub struct Cli {
    /// Path to YAML config file (overrides default search)
    #[arg(short = 'c', long="config", value_hint = ValueHint::FilePath)]
    pub config: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short = 'v', long, alias = "verbose", action = ArgAction::SetTrue)]
    pub debug: bool,

    /// LMS player name to monitor (required unless set in config file)
    #[arg(short = 'N', long)]
    pub name: Option<String>,

    /// Weather: API key,units,lang,latitude,longitude (comma-separated)
    #[arg(short = 'W', long = "weather", value_name = "WEATHER")]
    pub weather: Option<String>,

    /// Tomorrow.io API key (overrides --weather key field)
    #[arg(long = "weather-api")]
    pub weather_api: Option<String>,

    /// Weather units: metric (default) or imperial (overrides --weather units field)
    #[arg(long = "weather-units")]
    pub weather_units: Option<String>,

    /// Weather language/translation code (overrides --weather lang field)
    #[arg(long = "weather-lang")]
    pub weather_lang: Option<String>,

    /// Latitude — overrides config file and GeoIP
    #[arg(long)]
    pub lat: Option<f64>,

    /// Longitude — overrides config file and GeoIP
    #[arg(long)]
    pub lon: Option<f64>,

    /// Text scroll mode
    #[arg(short = 'z', long, value_parser = ["loop", "loopleft", "cylon"])]
    pub scroll: Option<String>,

    /// Show remaining time instead of total duration
    #[arg(short = 'r', long, action = ArgAction::SetTrue)]
    pub remain: bool,

    /// TTF text font name (must have ./data/{name}-text.zip)
    #[arg(short = 'F', long = "text_font")]
    pub text_font: Option<String>,

    /// TTF text font size (must have ./data/{name}-text.zip)
    #[arg(short = 'f', long = "text_font_size")]
    pub text_font_size: Option<f32>,

    /// Clock font
    #[arg(short = 'C', long = "clock_font",
          value_parser = ["7seg","dejavu","dotty","gawker","ledreal","mackintosh","marvel","moomy","noto","poppins","roboto"])]
    pub clock_font: Option<String>,

    /// Easter egg animation
    #[arg(short = 'E', long,
          value_parser = ["bass","blackfly","cassette","ibmpc","moog","pipboy","radio40","radio50","reel2reel","scope","technics","tubeamp","tvtime","vcr","none"])]
    pub eggs: Option<String>,

    /// Skip splash screen
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_splash: bool,

    /// Show device metrics overlay
    #[arg(short = 'k', long, action = ArgAction::SetTrue)]
    pub metrics: bool,

    /// I2C bus device path
    #[arg(long, default_value = "/dev/i2c-1")]
    pub i2c_bus: Option<String>,

    /// [Internal] emulation mode
    #[arg(long, hide = true, action = ArgAction::SetTrue)]
    pub emulated: bool,

    /// Display driver (emulator / config override)
    #[arg(short = 'd', long = "driver",
          value_parser = ["ssd1306","ssd1309","ssd1322","sh1106","sh1122","sharpmemory","st7789"])]
    pub driver: Option<String>,

    /// Visualizer type
    #[arg(short = 'a', long = "viz",
          value_parser = ["combination","hist_aio","hist_mono","hist_stereo","peak_mono","peak_stereo","vu_aio","vu_mono","vu_stereo","waveform_spectrum","no_viz"])]
    pub viz: Option<String>,

    /// Histogram colour scheme
    #[arg(long = "hist-scheme", value_parser = ["classic","ocean","fire","neon"])]
    pub hist_scheme: Option<String>,

    /// Print fully merged config and exit
    #[arg(long, action = ArgAction::SetTrue)]
    pub dump_config: bool,
}

/// Parse CLI, read YAML config file, merge (CLI wins), validate.
/// Returns the fully resolved `Config`.
pub fn load() -> Result<Config, ConfigError> {
    let cli = Cli::parse();

    // 1. Defaults
    let mut cfg = Config::default();

    // 2. YAML file
    let config_path = cli.config.clone()
        .or_else(find_config_file);

    if let Some(ref p) = config_path {
        if p.exists() {
            let y = read_yaml(p)?;
            merge(&mut cfg, y);
        } else if cli.config.is_some() {
            // Explicit path was given but doesn't exist — that's an error
            return Err(ConfigError::Validation(format!(
                "Config file not found: {}", p.display()
            )));
        }
    }

    // 3. CLI overrides (highest precedence)
    apply_cli_overrides(&mut cfg, &cli);

    // 4. Validate
    validate(&cfg)?;

    if cli.dump_config {
        let s = serde_yaml::to_string(&cfg)?;
        println!("{s}");
        std::process::exit(0);
    }

    Ok(cfg)
}

fn find_config_file() -> Option<PathBuf> {
    if let Some(home) = home_dir() {
        for name in &["config.yaml", "config.yml"] {
            let p = home.join(".config/lymons").join(name);
            if p.exists() { return Some(p); }
        }
        let p = home.join(".config/lymons.yaml");
        if p.exists() { return Some(p); }
    }
    for candidate in &["lymons.yaml", "config.yaml", "config.yml", "config/lymons.yaml"] {
        let p = PathBuf::from(candidate);
        if p.exists() { return Some(p); }
    }
    None
}

fn read_yaml(path: &Path) -> Result<Config, ConfigError> {
    let s = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&s)?)
}

fn merge(dst: &mut Config, src: Config) {
    macro_rules! take {
        ($field:ident) => { if src.$field.is_some() { dst.$field = src.$field; } };
    }
    take!(log_level);
    take!(player);
    take!(text_font);
    take!(text_font_size);
    take!(scroll_mode);
    take!(show_remaining);
    take!(clock_font);
    take!(easter_egg);
    take!(visualizer);
    take!(hist_scheme);
    take!(show_metrics);
    take!(show_splash);
    take!(i2c_bus);     // need to retire this and fold any code under display.bus.bus
    take!(latitude);
    take!(longitude);

    match (&mut dst.display, src.display) {
        (None, Some(c)) => dst.display = Some(c),
        (Some(d), Some(s)) => merge_display(d, s),
        _ => {}
    }
    match (&mut dst.weather, src.weather) {
        (None, Some(c)) => dst.weather = Some(c),
        (Some(d), Some(s)) => merge_weather(d, s),
        _ => {}
    }
}

fn merge_display(dst: &mut DisplayConfig, src: DisplayConfig) {
    macro_rules! take {
        ($field:ident) => { if src.$field.is_some() { dst.$field = src.$field; } };
    }
    take!(width); take!(height); take!(rotate_deg); take!(invert);
    take!(brightness); take!(driver); take!(bus); take!(emulated);
}

fn merge_weather(dst: &mut WeatherConfig, src: WeatherConfig) {
    macro_rules! take {
        ($field:ident) => { if src.$field.is_some() { dst.$field = src.$field; } };
    }
    take!(api); take!(units); take!(translate); take!(latitude); take!(longitude);
}

fn apply_cli_overrides(cfg: &mut Config, cli: &Cli) {
    if cli.debug        { cfg.log_level = Some("debug".to_string()); }
    if cli.remain       { cfg.show_remaining = Some(true); }
    if cli.no_splash    { cfg.show_splash = Some(false); }
    if cli.metrics      { cfg.show_metrics = Some(true); }
    if cli.emulated {
        cfg.display.get_or_insert_with(DisplayConfig::default).emulated = Some(true);
    }

    macro_rules! take_opt {
        ($src:expr => $dst:expr) => { if $src.is_some() { $dst = $src.clone(); } };
    }
    take_opt!(cli.name           => cfg.player);
    take_opt!(cli.text_font      => cfg.text_font);
    take_opt!(cli.text_font_size => cfg.text_font_size);
    take_opt!(cli.scroll         => cfg.scroll_mode);
    take_opt!(cli.clock_font     => cfg.clock_font);
    take_opt!(cli.eggs           => cfg.easter_egg);
    take_opt!(cli.viz            => cfg.visualizer);
    take_opt!(cli.hist_scheme    => cfg.hist_scheme);
    take_opt!(cli.i2c_bus        => cfg.i2c_bus);
    take_opt!(cli.lat            => cfg.latitude);
    take_opt!(cli.lon            => cfg.longitude);

    // Weather overrides — comma-separated -W/--weather first, then discrete flags win
    if let Some(w_str) = &cli.weather {
        // Format: key,units,lang,latitude,longitude  (any trailing fields may be omitted)
        let parts: Vec<&str> = w_str.splitn(5, ',').collect();
        let w = cfg.weather.get_or_insert_with(WeatherConfig::default);
        if parts.len() >= 1 && !parts[0].is_empty() { w.api       = Some(parts[0].to_string()); }
        if parts.len() >= 2 && !parts[1].is_empty() { w.units     = Some(parts[1].to_string()); }
        if parts.len() >= 3 && !parts[2].is_empty() { w.translate = Some(parts[2].to_string()); }
        if parts.len() >= 4 && !parts[3].is_empty() {
            if let Ok(lat) = parts[3].parse::<f64>() { w.latitude  = Some(lat); }
        }
        if parts.len() >= 5 && !parts[4].is_empty() {
            if let Ok(lon) = parts[4].parse::<f64>() { w.longitude = Some(lon); }
        }
    }
    if cli.weather_api.is_some() || cli.weather_units.is_some() || cli.weather_lang.is_some() {
        let w = cfg.weather.get_or_insert_with(WeatherConfig::default);
        if cli.weather_api.is_some()   { w.api       = cli.weather_api.clone(); }
        if cli.weather_units.is_some() { w.units     = cli.weather_units.clone(); }
        if cli.weather_lang.is_some()  { w.translate = cli.weather_lang.clone(); }
    }
    // lat/lon also back-fill into weather if not already set there
    if let (Some(lat), Some(lon)) = (cli.lat, cli.lon) {
        let w = cfg.weather.get_or_insert_with(WeatherConfig::default);
        if w.latitude.is_none()  { w.latitude  = Some(lat); }
        if w.longitude.is_none() { w.longitude = Some(lon); }
    }

    // Driver override
    if let Some(d) = &cli.driver {
        let driver = match d.as_str() {
            "ssd1306"     => DriverKind::Ssd1306,
            "ssd1309"     => DriverKind::Ssd1309,
            "ssd1322"     => DriverKind::Ssd1322,
            "sh1106"      => DriverKind::Sh1106,
            "sh1122"      => DriverKind::Sh1122,
            "sharpmemory" => DriverKind::SharpMemory,
            "st7789"      => DriverKind::St7789,
            _             => unreachable!(),
        };
        cfg.display.get_or_insert_with(DisplayConfig::default).driver = Some(driver);
    }
}

fn validate(cfg: &Config) -> Result<(), ConfigError> {
    if cfg.player.as_ref().map(|p| p.is_empty()).unwrap_or(true) {
        return Err(ConfigError::Validation(
            "Player name is required: set 'player' in config file or use -N / --name".into()
        ));
    }
    if let Some(display) = cfg.display.as_ref() {
        if let (Some(w), Some(h)) = (display.width, display.height) {
            if w == 0 || h == 0 {
                return Err(ConfigError::Validation("display width/height must be > 0".into()));
            }
        }
        if let Some(rot) = display.rotate_deg {
            if !matches!(rot, 0 | 90 | 180 | 270) {
                return Err(ConfigError::Validation(
                    "display rotate_deg must be 0|90|180|270".into()
                ));
            }
        }
    }
    Ok(())
}

