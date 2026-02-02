/*
 *  display/error.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Unified error types for display subsystem
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

use std::fmt;
use std::error::Error;

/// Unified error type for all display operations
#[derive(Debug)]
pub enum DisplayError {
    /// Hardware initialization failed
    InitializationFailed(String),

    /// I2C communication error
    I2cError(String),

    /// SPI communication error
    SpiError(String),

    /// GPIO pin error
    GpioError(String),

    /// Invalid configuration
    InvalidConfiguration(String),

    /// Unsupported operation for this display
    UnsupportedOperation,

    /// Invalid rotation angle
    InvalidRotation(u16),

    /// Framebuffer size mismatch
    BufferSizeMismatch { expected: usize, actual: usize },

    /// Drawing operation failed
    DrawingError(String),

    /// Display interface error
    InterfaceError(display_interface::DisplayError),

    /// Generic error with message
    Other(String),
}

impl fmt::Display for DisplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayError::InitializationFailed(msg) =>
                write!(f, "Display initialization failed: {}", msg),
            DisplayError::I2cError(msg) =>
                write!(f, "I2C communication error: {}", msg),
            DisplayError::SpiError(msg) =>
                write!(f, "SPI communication error: {}", msg),
            DisplayError::GpioError(msg) =>
                write!(f, "GPIO error: {}", msg),
            DisplayError::InvalidConfiguration(msg) =>
                write!(f, "Invalid configuration: {}", msg),
            DisplayError::UnsupportedOperation =>
                write!(f, "Operation not supported by this display"),
            DisplayError::InvalidRotation(degrees) =>
                write!(f, "Invalid rotation angle: {} (must be 0, 90, 180, or 270)", degrees),
            DisplayError::BufferSizeMismatch { expected, actual } =>
                write!(f, "Buffer size mismatch: expected {} bytes, got {}", expected, actual),
            DisplayError::DrawingError(msg) =>
                write!(f, "Drawing error: {}", msg),
            DisplayError::InterfaceError(err) =>
                write!(f, "Display interface error: {:?}", err),
            DisplayError::Other(msg) =>
                write!(f, "{}", msg),
        }
    }
}

impl Error for DisplayError {
    // display_interface::DisplayError doesn't implement std::error::Error
    // so we can't provide it as a source
}

// Conversion from display_interface::DisplayError
impl From<display_interface::DisplayError> for DisplayError {
    fn from(err: display_interface::DisplayError) -> Self {
        DisplayError::InterfaceError(err)
    }
}

// Conversion from Linux I2C errors
impl From<linux_embedded_hal::I2CError> for DisplayError {
    fn from(err: linux_embedded_hal::I2CError) -> Self {
        DisplayError::I2cError(format!("{:?}", err))
    }
}

/// Factory error types
#[derive(Debug)]
pub enum DisplayFactoryError {
    /// No driver specified in configuration
    NoDriverSpecified,

    /// No bus configuration specified
    NoBusConfiguration,

    /// Unsupported driver/bus combination
    UnsupportedCombination,

    /// Display driver initialization failed
    DriverInitFailed(DisplayError),

    /// Configuration validation error
    ConfigError(String),
}

impl fmt::Display for DisplayFactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayFactoryError::NoDriverSpecified =>
                write!(f, "No display driver specified in configuration"),
            DisplayFactoryError::NoBusConfiguration =>
                write!(f, "No bus configuration specified"),
            DisplayFactoryError::UnsupportedCombination =>
                write!(f, "Unsupported driver/bus combination"),
            DisplayFactoryError::DriverInitFailed(err) =>
                write!(f, "Driver initialization failed: {}", err),
            DisplayFactoryError::ConfigError(msg) =>
                write!(f, "Configuration error: {}", msg),
        }
    }
}

impl Error for DisplayFactoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DisplayFactoryError::DriverInitFailed(err) => Some(err),
            _ => None,
        }
    }
}

impl From<DisplayError> for DisplayFactoryError {
    fn from(err: DisplayError) -> Self {
        DisplayFactoryError::DriverInitFailed(err)
    }
}

// Conversion from DisplayFactoryError to DisplayError
impl From<DisplayFactoryError> for DisplayError {
    fn from(err: DisplayFactoryError) -> Self {
        match err {
            DisplayFactoryError::DriverInitFailed(e) => e,
            DisplayFactoryError::NoDriverSpecified =>
                DisplayError::InvalidConfiguration("No driver specified".to_string()),
            DisplayFactoryError::NoBusConfiguration =>
                DisplayError::InvalidConfiguration("No bus configuration".to_string()),
            DisplayFactoryError::UnsupportedCombination =>
                DisplayError::InvalidConfiguration("Unsupported driver/bus combination".to_string()),
            DisplayFactoryError::ConfigError(msg) =>
                DisplayError::InvalidConfiguration(msg),
        }
    }
}
