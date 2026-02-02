use serde::{Deserialize, Serialize};
use clap::{ArgAction, Parser, ValueHint};
use dirs_next::home_dir;
use std::{fs, path::{Path, PathBuf}};
use thiserror::Error;

/// Error type for config loading/validation.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

/// Top-level app configuration. Extend to mirror your current fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Config {
    /// General options
    pub log_level: Option<String>,     // e.g., "info" | "debug"
    pub sample_rate_hz: Option<u32>,   // audio sample rate, etc.
    /// display-specific geometry & behavior
    pub display: Option<DisplayConfig>,

    // Any other groups you already have can go here
    // pub network: Option<NetConfig>,
    // pub theme: Option<ThemeConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DisplayConfig {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub rotate_deg: Option<u16>,
    pub invert: Option<bool>,
    pub brightness: Option<u8>,     // 0-255
    pub driver: Option<DriverKind>, // <- strongly-typed driver selection
    pub bus: Option<BusConfig>,     // <- i2c or spi wiring
}

/// CLI overrides. All fields are Options so we can layer them over YAML.
#[derive(Debug, Parser, Clone)]
#[command(name = "LyMonS", about = "LyMonS Monitor", disable_help_flag = false)]
pub struct Cli {
    /// Path to a YAML config file (overrides search)
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub log_level: Option<String>,
    #[arg(long)]
    pub sample_rate_hz: Option<u32>,
    #[arg(long)]
    pub device_name: Option<String>,
    #[arg(long)]
    pub display_width: Option<u32>,
    #[arg(long)]
    pub display_height: Option<u32>,
    #[arg(long)]
    pub display_rotate_deg: Option<u16>,
    #[arg(long, action = ArgAction::Set)]
    pub display_invert: Option<bool>,
    /// dump fully merged config (after overrides) and exit
    #[arg(long, action = ArgAction::SetTrue)]
    pub dump_config: bool,
}

/// Public entry point: parse CLI, read YAML, merge, validate.
pub fn load() -> Result<Config, ConfigError> {
    let cli = Cli::parse();

    // 1) defaults (from `Default` impl)
    let mut cfg = Config::default();

    // 2) YAML file (explicit path or search)
    if let Some(p) = cli.config.as_ref() {
        if p.exists() {
            let y = read_yaml(p)?;
            merge(&mut cfg, y);
        } else {
            return Err(ConfigError::Validation(format!(
                "Config file not found: {}",
                p.display()
            )));
        }
    } else if let Some(p) = find_config_file() {
        let y = read_yaml(&p)?;
        merge(&mut cfg, y);
    }

    // 3) CLI overrides (highest precedence)
    apply_cli_overrides(&mut cfg, &cli);

    // 4) Validate
    validate(&cfg)?;

    if cli.dump_config {
        // Pretty YAML of effective config (nice for debugging)
        let s = serde_yaml::to_string(&cfg)?;
        println!("{s}");
        std::process::exit(0);
    }

    Ok(cfg)
}

/// Try common locations in order (first hit wins).
fn find_config_file() -> Option<PathBuf> {
    // XDG-style: ~/.config/lymons/config.yaml
    if let Some(home) = home_dir() {
        let p = home.join(".config/lymons/config.yaml");
        if p.exists() { return Some(p) }
        let p = home.join(".config/lymons.yaml");
        if p.exists() { return Some(p) }
    }
    // project local
    for candidate in &["lymons.yaml", "config.yaml", "config/lymons.yaml"] {
        let p = PathBuf::from(candidate);
        if p.exists() { return Some(p) }
    }
    None
}

fn read_yaml(path: &Path) -> Result<Config, ConfigError> {
    let s = fs::read_to_string(path)?;
    let cfg: Config = serde_yaml::from_str(&s)?;
    Ok(cfg)
}

/// Shallow merge `src` into `dst`, Option-by-Option.
fn merge(dst: &mut Config, src: Config) {
    // top-level
    if src.log_level.is_some()      { dst.log_level = src.log_level; }
    // display
    match (&mut dst.display, src.display) {
        (None, Some(c)) => dst.display = Some(c),
        (Some(d), Some(s)) => merge_display(d, s),
        _ => {}
    }
}

fn merge_display(dst: &mut DisplayConfig, src: DisplayConfig) {
    if src.width.is_some()       { dst.width = src.width; }
    if src.height.is_some()      { dst.height = src.height; }
    if src.rotate_deg.is_some()  { dst.rotate_deg = src.rotate_deg; }
    if src.invert.is_some()      { dst.invert = src.invert; }
    if src.brightness.is_some()  { dst.brightness = src.brightness; }
    if src.driver.is_some()      { dst.driver = src.driver; }
    if src.bus.is_some()         { dst.bus = src.bus; }
}

fn apply_cli_overrides(cfg: &mut Config, cli: &Cli) {
    if cli.log_level.is_some()       { cfg.log_level = cli.log_level.clone(); }
    let any_case = cli.display_width.is_some()
        || cli.display_height.is_some()
        || cli.display_rotate_deg.is_some()
        || cli.display_invert.is_some();

    if any_case && cfg.display.is_none() {
        cfg.display = Some(DisplayConfig::default());
    }
    if let Some(display) = cfg.display.as_mut() {
        if cli.display_width.is_some()       { display.width = cli.display_width; }
        if cli.display_height.is_some()      { display.height = cli.display_height; }
        if cli.display_rotate_deg.is_some()  { display.rotate_deg = cli.display_rotate_deg; }
        if cli.display_invert.is_some()      { display.invert = cli.display_invert; }
    }
}

/// Put any invariants here (required fields, ranges, etc.)
fn validate(cfg: &Config) -> Result<(), ConfigError> {
    if let Some(display) = cfg.display.as_ref() {
        if let (Some(w), Some(h)) = (display.width, display.height) {
            if w == 0 || h == 0 {
                return Err(ConfigError::Validation("display width/height must be > 0".into()));
            }
        }
        if let Some(rot) = display.rotate_deg {
            match rot {
                0 | 90 | 180 | 270 => {},
                _ => return Err(ConfigError::Validation("display rotate_deg must be 0|90|180|270".into()))
            }
        }
        if let Some(b) = display.brightness {
            if b > 255 {
                return Err(ConfigError::Validation("display brightness must be 0..=255".into()));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BusConfig {
    I2c {
        bus: String,        // e.g. "/dev/i2c-1"
        address: u8,        // e.g. 0x3C (I2C addresses are 7-bit, stored in u8)
        speed_hz: Option<u32>,
    },
    Spi {
        bus: String,        // e.g. "/dev/spidev0.0"
        speed_hz: Option<u32>,
        dc_pin: u32,        // BCM numbering (or however your HAL maps)
        rst_pin: Option<u32>,
        cs_pin: Option<u32>, // optional if part of /dev/spidevX.Y
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriverKind {
    Ssd1306,
    Ssd1309,
    Ssd1322,
    Sh1106,
    SharpMemory, // Future implementation - 400x240 memory LCD
}
