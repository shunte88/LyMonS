/*
 *  astral.rs
 *
 *  LyMonS - worth the squeeze
 *	(c) 2020-26 Stuart Hunter
 *
 *  Independent astronomical calculations (sunrise, sunset, moonrise, moonset)
 *  Used for auto-brightness and display - works without weather service
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

use chrono::{DateTime, Local, NaiveDate};
use log::info;
use std::sync::{Arc, RwLock};

use crate::location::Location;
use crate::sun;

/// Astronomical data for a specific day
#[derive(Debug, Clone)]
pub struct AstralData {
    pub date: NaiveDate,
    pub sunrise: Option<DateTime<Local>>,
    pub sunset: Option<DateTime<Local>>,
    pub moonrise: Option<DateTime<Local>>,
    pub moonset: Option<DateTime<Local>>,
}

/// Astral service that caches daily calculations
pub struct AstralService {
    location: Location,
    cached_data: Arc<RwLock<Option<AstralData>>>,
}

impl AstralService {
    /// Create a new astral service for a location
    pub fn new(location: Location) -> Self {
        info!("Initializing astral service for location: {}", location);
        Self {
            location,
            cached_data: Arc::new(RwLock::new(None)),
        }
    }

    /// Get today's astral data (uses cache if valid)
    pub fn get_today(&self) -> AstralData {
        let today = Local::now().date_naive();

        // Check cache
        {
            let cache = self.cached_data.read().unwrap();
            if let Some(data) = cache.as_ref() {
                if data.date == today {
                    return data.clone();
                }
            }
        }

        // Calculate new data
        info!("Calculating astral data for {}", today);
        let data = self.calculate_for_date(today);

        // Update cache
        {
            let mut cache = self.cached_data.write().unwrap();
            *cache = Some(data.clone());
        }

        data
    }

    /// Calculate astral data for a specific date
    pub fn calculate_for_date(&self, date: NaiveDate) -> AstralData {
        let sun_times = sun::sun_times_for_date(
            self.location.latitude,
            self.location.longitude,
            date,
        );

        // Convert UTC to local time
        let sunrise = sun_times.sunrise_utc.map(|dt| dt.with_timezone(&Local));
        let sunset = sun_times.sunset_utc.map(|dt| dt.with_timezone(&Local));

        // TODO: Calculate moonrise/moonset
        // For now, return None - will implement moon calculations later
        let moonrise = None;
        let moonset = None;

        AstralData {
            date,
            sunrise,
            sunset,
            moonrise,
            moonset,
        }
    }

    /// Check if it's currently daytime
    pub fn is_daytime(&self) -> bool {
        let data = self.get_today();
        let now = Local::now();

        if let (Some(sunrise), Some(sunset)) = (data.sunrise, data.sunset) {
            now >= sunrise && now < sunset
        } else {
            // Default to daytime if we can't determine
            true
        }
    }

    /// Get minutes until next sunrise or sunset (useful for auto-brightness scheduling)
    pub fn minutes_until_next_event(&self) -> Option<i64> {
        let data = self.get_today();
        let now = Local::now();

        let mut next_events = Vec::new();

        if let Some(sunrise) = data.sunrise {
            if sunrise > now {
                next_events.push(sunrise);
            }
        }

        if let Some(sunset) = data.sunset {
            if sunset > now {
                next_events.push(sunset);
            }
        }

        next_events.sort();
        next_events.first().map(|dt| (*dt - now).num_minutes())
    }

    /// Update location and invalidate cache
    pub fn update_location(&mut self, location: Location) {
        info!("Updating astral service location to: {}", location);
        self.location = location;
        // Invalidate cache
        let mut cache = self.cached_data.write().unwrap();
        *cache = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::LocationSource;

    #[test]
    fn test_astral_calculation() {
        let location = Location {
            latitude: 40.7128,
            longitude: -74.0060,
            city: Some("New York".to_string()),
            region: Some("NY".to_string()),
            source: LocationSource::UserConfig,
        };

        let service = AstralService::new(location);
        let data = service.get_today();

        // Should have sunrise and sunset for NYC
        assert!(data.sunrise.is_some());
        assert!(data.sunset.is_some());
    }

    #[test]
    fn test_cache() {
        let location = Location {
            latitude: 40.7128,
            longitude: -74.0060,
            city: None,
            region: None,
            source: LocationSource::UserConfig,
        };

        let service = AstralService::new(location);

        // First call calculates
        let data1 = service.get_today();
        // Second call uses cache
        let data2 = service.get_today();

        assert_eq!(data1.date, data2.date);
        assert_eq!(data1.sunrise, data2.sunrise);
    }
}
