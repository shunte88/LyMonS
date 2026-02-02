/*
 *  display/factory.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Factory pattern for dynamic display driver loading
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

use crate::config::{DisplayConfig, DriverKind, BusConfig};
use crate::display::error::DisplayFactoryError;
use crate::display::traits::DisplayDriver;
use log::{info, debug};

#[cfg(feature = "driver-ssd1306")]
use crate::display::drivers::ssd1306::Ssd1306Driver;

#[cfg(feature = "driver-ssd1309")]
use crate::display::drivers::ssd1309::Ssd1309Driver;

#[cfg(feature = "driver-ssd1322")]
use crate::display::drivers::ssd1322::Ssd1322Driver;

#[cfg(feature = "driver-sh1106")]
use crate::display::drivers::sh1106::Sh1106Driver;

#[cfg(feature = "plugin-system")]
use crate::display::plugin::{PluginLoader, PluginDriverAdapter};

/// Type alias for boxed display driver trait objects
pub type BoxedDriver = Box<dyn DisplayDriver>;

/// Factory for creating display drivers from configuration
pub struct DisplayDriverFactory;

impl DisplayDriverFactory {
    /// Create a display driver from configuration
    ///
    /// This method examines the configuration and creates the appropriate
    /// driver implementation based on the driver kind and bus configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Display configuration containing driver and bus settings
    ///
    /// # Returns
    ///
    /// A boxed trait object implementing DisplayDriver, or an error if the
    /// configuration is invalid or the driver/bus combination is unsupported.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = DisplayConfig {
    ///     driver: Some(DriverKind::Ssd1306),
    ///     bus: Some(BusConfig::I2c {
    ///         bus: "/dev/i2c-1".to_string(),
    ///         address: 0x3C,
    ///         speed_hz: None,
    ///     }),
    ///     ..Default::default()
    /// };
    ///
    /// let driver = DisplayDriverFactory::create_from_config(&config)?;
    /// ```
    pub fn create_from_config(
        config: &DisplayConfig
    ) -> Result<BoxedDriver, DisplayFactoryError> {
        let driver_kind = config.driver.as_ref()
            .ok_or(DisplayFactoryError::NoDriverSpecified)?;

        // Check if emulation mode is requested
        #[cfg(feature = "emulator")]
        if config.emulated.unwrap_or(false) {
            info!("Emulation mode enabled - creating emulator driver");
            return Self::create_emulator_driver(config, driver_kind);
        }

        let bus_config = config.bus.as_ref()
            .ok_or(DisplayFactoryError::NoBusConfiguration)?;

        // Try plugin loading first if plugin system is enabled
        #[cfg(feature = "plugin-system")]
        {
            if let Some(plugin_driver) = Self::try_load_plugin(config, driver_kind) {
                info!("Using plugin driver for {:?}", driver_kind);
                return Ok(plugin_driver);
            }
            debug!("Plugin not found, falling back to built-in driver");
        }

        // Fall back to built-in static drivers
        match (driver_kind, bus_config) {
            #[cfg(feature = "driver-ssd1306")]
            (DriverKind::Ssd1306, BusConfig::I2c { bus, address, .. }) => {
                Ok(Box::new(Ssd1306Driver::new_i2c(bus, *address, config)?))
            }

            #[cfg(feature = "driver-ssd1309")]
            (DriverKind::Ssd1309, BusConfig::I2c { bus, address, .. }) => {
                Ok(Box::new(Ssd1309Driver::new_i2c(bus, *address, config)?))
            }

            #[cfg(feature = "driver-ssd1322")]
            (DriverKind::Ssd1322, BusConfig::Spi { bus, dc_pin, rst_pin, .. }) => {
                Ok(Box::new(Ssd1322Driver::new_spi(
                    bus,
                    *dc_pin,
                    rst_pin.ok_or_else(|| DisplayFactoryError::ConfigError(
                        "SSD1322 requires rst_pin".to_string()
                    ))?,
                    config
                )?))
            }

            #[cfg(feature = "driver-sh1106")]
            (DriverKind::Sh1106, BusConfig::I2c { bus, address, .. }) => {
                Ok(Box::new(Sh1106Driver::new_i2c(bus, *address, config)?))
            }

            // Catch-all for unsupported combinations or disabled features
            _ => {
                #[cfg(not(feature = "driver-ssd1306"))]
                if matches!(driver_kind, DriverKind::Ssd1306) {
                    return Err(DisplayFactoryError::ConfigError(
                        "SSD1306 driver not enabled. Enable with --features driver-ssd1306".to_string()
                    ));
                }

                #[cfg(not(feature = "driver-ssd1309"))]
                if matches!(driver_kind, DriverKind::Ssd1309) {
                    return Err(DisplayFactoryError::ConfigError(
                        "SSD1309 driver not enabled. Enable with --features driver-ssd1309".to_string()
                    ));
                }

                #[cfg(not(feature = "driver-ssd1322"))]
                if matches!(driver_kind, DriverKind::Ssd1322) {
                    return Err(DisplayFactoryError::ConfigError(
                        "SSD1322 driver not enabled. Enable with --features driver-ssd1322".to_string()
                    ));
                }

                #[cfg(not(feature = "driver-sh1106"))]
                if matches!(driver_kind, DriverKind::Sh1106) {
                    return Err(DisplayFactoryError::ConfigError(
                        "SH1106 driver not enabled. Enable with --features driver-sh1106".to_string()
                    ));
                }

                Err(DisplayFactoryError::UnsupportedCombination)
            }
        }
    }

    /// Try to load a plugin for the specified driver kind
    ///
    /// Returns Some(driver) if a plugin was successfully loaded,
    /// or None if no plugin was found or loading failed.
    #[cfg(feature = "plugin-system")]
    fn try_load_plugin(
        config: &DisplayConfig,
        driver_kind: &DriverKind
    ) -> Option<BoxedDriver> {
        // Map DriverKind to plugin name
        let plugin_name = match driver_kind {
            DriverKind::Ssd1306 => "ssd1306",
            DriverKind::Ssd1309 => "ssd1309",
            DriverKind::Ssd1322 => "ssd1322",
            DriverKind::Sh1106 => "sh1106",
            DriverKind::SharpMemory => "sharpmemory",
        };

        debug!("Searching for plugin: {}", plugin_name);

        // Try to load the plugin
        match PluginLoader::load_by_driver_type(plugin_name) {
            Ok(plugin) => {
                info!("Loaded plugin: {} v{}",
                    plugin.metadata().name,
                    plugin.metadata().version
                );

                // Create the adapter
                match PluginDriverAdapter::new(plugin, config) {
                    Ok(adapter) => {
                        info!("Successfully created plugin driver adapter");
                        Some(Box::new(adapter))
                    }
                    Err(e) => {
                        info!("Failed to create plugin driver: {:?}", e);
                        None
                    }
                }
            }
            Err(e) => {
                debug!("Failed to load plugin: {}", e);
                None
            }
        }
    }

    /// Create an emulator driver based on display kind
    ///
    /// This creates a desktop window emulator instead of trying to access hardware.
    #[cfg(feature = "emulator")]
    fn create_emulator_driver(
        config: &DisplayConfig,
        driver_kind: &DriverKind,
    ) -> Result<BoxedDriver, DisplayFactoryError> {
        use crate::display::drivers::emulator::EmulatorDriver;

        // Determine dimensions and color depth based on driver kind
        let (width, height, is_grayscale, name) = match driver_kind {
            DriverKind::Ssd1306 => (128, 64, false, "SSD1306"),
            DriverKind::Ssd1309 => (128, 64, false, "SSD1309"),
            DriverKind::Sh1106 => (132, 64, false, "SH1106"),
            DriverKind::Ssd1322 => (256, 64, true, "SSD1322"),
            DriverKind::SharpMemory => (400, 240, false, "SharpMemory"),
        };

        // Override with config if specified
        let final_width = config.width.unwrap_or(width);
        let final_height = config.height.unwrap_or(height);

        info!("Creating emulator driver: {} ({}x{}, {})",
            name, final_width, final_height,
            if is_grayscale { "grayscale" } else { "monochrome" }
        );

        let driver = if is_grayscale {
            EmulatorDriver::new_grayscale(final_width, final_height, name)?
        } else {
            EmulatorDriver::new_monochrome(final_width, final_height, name)?
        };

        Ok(Box::new(driver))
    }

    /// Validate a configuration without creating a driver
    ///
    /// This is useful for checking configuration at startup before attempting
    /// to initialize hardware.
    pub fn validate_config(config: &DisplayConfig) -> Result<(), DisplayFactoryError> {
        let _driver_kind = config.driver.as_ref()
            .ok_or(DisplayFactoryError::NoDriverSpecified)?;

        let _bus_config = config.bus.as_ref()
            .ok_or(DisplayFactoryError::NoBusConfiguration)?;

        // Add additional validation here as needed
        // For example, check that rotation angle is valid
        if let Some(rotation) = config.rotate_deg {
            if rotation != 0 && rotation != 90 && rotation != 180 && rotation != 270 {
                return Err(DisplayFactoryError::ConfigError(
                    format!("Invalid rotation angle: {} (must be 0, 90, 180, or 270)", rotation)
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_config_no_driver() {
        let config = DisplayConfig {
            driver: None,
            bus: Some(BusConfig::I2c {
                bus: "/dev/i2c-1".to_string(),
                address: 0x3C,
                speed_hz: None,
            }),
            ..Default::default()
        };

        assert!(DisplayDriverFactory::validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_no_bus() {
        let config = DisplayConfig {
            driver: Some(DriverKind::Ssd1306),
            bus: None,
            ..Default::default()
        };

        assert!(DisplayDriverFactory::validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_invalid_rotation() {
        let config = DisplayConfig {
            driver: Some(DriverKind::Ssd1306),
            bus: Some(BusConfig::I2c {
                bus: "/dev/i2c-1".to_string(),
                address: 0x3C,
                speed_hz: None,
            }),
            rotate_deg: Some(45), // Invalid!
            ..Default::default()
        };

        assert!(DisplayDriverFactory::validate_config(&config).is_err());
    }
}
