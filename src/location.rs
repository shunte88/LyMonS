/*
 *  location.rs
 *
 *  LyMonS - worth the squeeze
 *	(c) 2020-26 Stuart Hunter
 *
 *  Location service - provides lat/lng from config or geolocation lookup
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

use log::{info, warn};
use std::fmt;

/// Location information with coordinates
#[derive(Debug, Clone)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub city: Option<String>,
    pub region: Option<String>,
    pub source: LocationSource,
}

/// Source of location data
#[derive(Debug, Clone, PartialEq)]
pub enum LocationSource {
    UserConfig,
    GeoIP,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(city), Some(region)) = (&self.city, &self.region) {
            write!(f, "{}, {} ({:.4}, {:.4}) [{}]",
                city, region, self.latitude, self.longitude,
                match self.source {
                    LocationSource::UserConfig => "config",
                    LocationSource::GeoIP => "geoip",
                })
        } else {
            write!(f, "({:.4}, {:.4}) [{}]",
                self.latitude, self.longitude,
                match self.source {
                    LocationSource::UserConfig => "config",
                    LocationSource::GeoIP => "geoip",
                })
        }
    }
}

#[derive(Debug)]
pub enum LocationError {
    ConfigMissing,
    GeoIPFailed(String),
    InvalidCoordinates,
}

impl fmt::Display for LocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocationError::ConfigMissing => write!(f, "No location in config"),
            LocationError::GeoIPFailed(e) => write!(f, "GeoIP lookup failed: {}", e),
            LocationError::InvalidCoordinates => write!(f, "Invalid coordinates"),
        }
    }
}

impl std::error::Error for LocationError {}

/// Get location from config or fallback to GeoIP lookup
pub async fn get_location(
    config_lat: Option<f64>,
    config_lng: Option<f64>,
) -> Result<Location, LocationError> {
    // Try user-specified location from config first
    if let (Some(lat), Some(lng)) = (config_lat, config_lng) {
        // Validate coordinates
        if lat >= -90.0 && lat <= 90.0 && lng >= -180.0 && lng <= 180.0 {
            info!("Using location from config: {:.4}, {:.4}", lat, lng);
            return Ok(Location {
                latitude: lat,
                longitude: lng,
                city: None,
                region: None,
                source: LocationSource::UserConfig,
            });
        } else {
            warn!("Invalid coordinates in config: {}, {}", lat, lng);
            return Err(LocationError::InvalidCoordinates);
        }
    }

    // Fall back to GeoIP lookup
    info!("No location in config, attempting GeoIP lookup...");
    match crate::geoloc::fetch_location().await {
        Ok(geo) => {
            info!("GeoIP lookup successful: {}, {} ({:.4}, {:.4})",
                geo.city, geo.region_code, geo.latitude, geo.longitude);
            Ok(Location {
                latitude: geo.latitude,
                longitude: geo.longitude,
                city: Some(geo.city),
                region: Some(geo.region_code),
                source: LocationSource::GeoIP,
            })
        }
        Err(e) => {
            warn!("GeoIP lookup failed: {}", e);
            Err(LocationError::GeoIPFailed(e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_location() {
        let loc = get_location(Some(40.7128), Some(-74.0060)).await.unwrap();
        assert_eq!(loc.latitude, 40.7128);
        assert_eq!(loc.longitude, -74.0060);
        assert_eq!(loc.source, LocationSource::UserConfig);
    }

    #[tokio::test]
    async fn test_invalid_coordinates() {
        let result = get_location(Some(100.0), Some(-74.0)).await;
        assert!(result.is_err());
    }
}
