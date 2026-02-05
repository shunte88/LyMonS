/*
 *  weather.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-26 Stuart Hunter
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
use serde::{Deserialize, Serialize};
use serde_json::{Value, Error as JsonError};
use reqwest::{Client, header};
use std::fmt::{self, Display};
use std::time::{Duration, Instant};
use log::{info, error};
use tokio::sync::{mpsc, watch, Mutex as TokMutex};
use tokio::task::JoinHandle;
use std::sync::Arc;
use chrono::{DateTime, Local, FixedOffset, Utc};

use flate2::read::GzDecoder;
use std::io::Read;
use std::thread;

use crate::geoloc::{fetch_location};
use crate::translate::Translation;

//use embedded_graphics::prelude::*;
use crate::sun;

/// Represents the audio bitrate mode for displaying the correct glyph.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum IconSet {
    Mono = 1,
    Basic = 2,
}

#[derive(Debug)]
pub enum WeatherCondition {
    Current = 0,
    ForecastDay1 = 1,
    ForecastDay2 = 2,
    ForecastDay3 = 3,
}

#[derive(Debug)]
pub struct WeatherDisplay{
    pub temp_units: String,
    pub wind_speed_units: String,
    pub sun: sun::SunTimes,
    pub current: WeatherData,
    pub forecasts: Vec<WeatherData>,
    pub svg: String,
    pub fsvg: Vec<String>,
}

// Custom error type for weather API operations.
#[allow(dead_code)]
#[derive(Debug)]
pub enum WeatherApiError {
    HttpRequestError(reqwest::Error),
    SerializationError(JsonError),
    DeserializationError(JsonError),
    ApiKeyError(String), // For specific API error messages
    GeolocationError(String),
    InvalidInput(String),
    PollingError(String),
    ApiError(String), // For specific API error messages
    MissingData(String),
    TranslationError(String),
}

#[allow(dead_code)]
impl Display for WeatherApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WeatherApiError::HttpRequestError(e) => write!(f, "HTTP request error: {}", e),
            WeatherApiError::SerializationError(e) => write!(f, "JSON serialization error: {}", e),
            WeatherApiError::DeserializationError(e) => write!(f, "JSON deserialization error: {}", e),
            WeatherApiError::ApiError(msg) => write!(f, "Tomorrow.io API error: {}", msg),
            WeatherApiError::ApiKeyError(msg) => write!(f, "Tomorrow.io API key required: {}", msg),
            WeatherApiError::GeolocationError(msg) => write!(f, "Geolocation error: {}", msg),
            WeatherApiError::MissingData(msg) => write!(f, "Missing weather data: {}", msg),
            WeatherApiError::TranslationError(msg) => write!(f, "Google Translate error: {}", msg),
            WeatherApiError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            WeatherApiError::PollingError(msg) => write!(f, "Polling error: {}", msg),
        }
    }
}

impl std::error::Error for WeatherApiError {}

impl From<reqwest::Error> for WeatherApiError {
    fn from(err: reqwest::Error) -> Self {
        WeatherApiError::HttpRequestError(err)
    }
}

impl From<JsonError> for WeatherApiError {
    fn from(err: JsonError) -> Self {
        WeatherApiError::SerializationError(err) // Can be refined to DeserializationError later if context allows
    }
}

#[allow(dead_code)]
#[derive(Default, Debug, Clone, PartialEq)]
pub struct WeatherCode {
	pub description: String,
	pub icon: u8, // old imgdata
    pub svg: String // new svg render
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct WeatherData {
    pub day: DateTime<Local>,
    pub humidity_avg: i64,
    pub moonrise_time: Option<DateTime<Local>>,
    pub moonset_time: Option<DateTime<Local>>,
    pub precipitation_probability_avg: f64,
    pub pressure_sea_level_avg: f64,
    pub sunrise_time: Option<DateTime<Local>>,
    pub sunset_time: Option<DateTime<Local>>,
    pub temperature_apparent_avg: f64,
    pub temperature_avg: f64,
    pub temperature_max: f64,
    pub temperature_min: f64,
    pub weather_code: WeatherCode,
    pub wind_direction: String,
    pub wind_speed_avg: f64,
}

// Main weather data struct
#[derive(Debug, Clone, PartialEq)] // Added PartialEq
pub struct WeatherConditions {
    location_name: String,
    pub base_folder: String,
    pub temperature_units: String, // "C" or "F"
    pub windspeed_units: String, // "km/h" or "mph"
    pub lat: f64,
    pub lng: f64,
    pub current: WeatherData,
    pub forecast: Vec<WeatherData>,
    pub last_updated: DateTime<Local>,
}

// Main Weather client
#[derive(Debug)]
pub struct Weather {
    pub active: bool,
    base_url: String,
    api_key: String,
    lat: f64,
    lng: f64,
    units: String, // "metric" or "imperial"
    translate: String,
    client: Client,
    icons: i32,
    pub weather_data: WeatherConditions,
    weather_tx: Option<watch::Sender<WeatherConditions>>,
    stop_sender: Option<mpsc::Sender<()>>,
    poll_handle: Option<JoinHandle<()>>,
    pub last_fetch_time: Option<Instant>, // track last fetched
}

impl WeatherConditions {
    pub fn new(location_name: String, units: String, base_folder: String, lat: f64, lng: f64) -> Self {
        Self {
            location_name,
            base_folder,
            temperature_units: if units == "imperial" { "F" } else { "C" }.to_string(),
            windspeed_units: if units == "imperial" { "mph" } else { "km/h" }.to_string(),
            lat,
            lng,
            current: WeatherData::default(),
            forecast: vec![WeatherData::default(); 7],
            last_updated: Local::now(),
        }
    }
    fn get_svg_path(&self, wc: WeatherCondition) -> String {
        let svg = match wc {
            WeatherCondition::Current => &self.current.weather_code.svg.clone(),
            WeatherCondition::ForecastDay1 => &self.forecast[0].weather_code.svg.clone(),
            WeatherCondition::ForecastDay2 => &self.forecast[1].weather_code.svg.clone(),
            WeatherCondition::ForecastDay3 => &self.forecast[2].weather_code.svg.clone(),
        };
        let ret = format!("{}{}", &self.base_folder.clone(), 
            if svg.contains(".svg") {
                svg.clone()
            }else{
                "no_data.svg".to_string()
            }
        );
        ret
    }

    pub fn get_weather_display(&self) -> WeatherDisplay {
        let temp_units = self.temperature_units.clone();
        let wind_speed_units = self.windspeed_units.clone();
        let current = self.current.clone();
        let forecasts = self.forecast.clone();
        let svg = self.get_svg_path(WeatherCondition::Current).clone();
        let mut fsvg: Vec<String> = Vec::new();
        for _i in 0..3 {
            let wc = match _i {
                0 => WeatherCondition::ForecastDay1,
                1 => WeatherCondition::ForecastDay2,
                2 => WeatherCondition::ForecastDay3,
                _ => WeatherCondition::ForecastDay1,
            };
            fsvg.push(self.get_svg_path(wc).clone());
        }
        let sun = sun::sun_times_today(self.lat, self.lng);
        WeatherDisplay {
            temp_units,
            wind_speed_units,
            sun,
            current,
            forecasts,
            svg,
            fsvg,
        }
    }

}

#[allow(dead_code)]
#[allow(irrefutable_let_patterns)]
impl Weather {
    /// Creates a new `Weather` instance. Performs IP lookup if lat/lng are not provided.
    pub async fn new(weather_config:&str) -> Result<Self, WeatherApiError> {

        const VERSION: &'static str = concat!("LyMonS ", env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));

        let parts: Vec<&str> = weather_config.split(',').collect();
        let api_key = parts.get(0).ok_or(WeatherApiError::ApiKeyError("no key specified".to_string()))?;
        let mut lat_str: Option<String> = None;
        let mut lng_str: Option<String> = None;
        let mut transl: String = "".to_string();
        // if US - imperial else metric
        let mut units: String = "imperial".to_string(); // Default units
        let mut icons: i32 = 1;

        for part in parts.iter().skip(1) {
            if part.starts_with("lat=") {
                lat_str = Some(part.trim_start_matches("lat=").to_string());
            } else if part.starts_with("lng=") {
                lng_str = Some(part.trim_start_matches("lng=").to_string());
            } else if part.starts_with("lang=") {
                transl = part.trim_start_matches("lang=").to_string();
            } else if part.starts_with("units=") {
                units = part.trim_start_matches("units=").to_string();
            } else if part.starts_with("icons=") {
                icons = part.trim_start_matches("icons=").to_string().parse().map_err(|_| WeatherApiError::InvalidInput("Invalid icons format".to_string()))?;
            }
        }

        // Normalize units to API format (Tomorrow.io expects "metric" or "imperial")
        units = match units.to_lowercase().as_str() {
            "f" | "fahrenheit" | "imperial" => "imperial".to_string(),
            "c" | "celsius" | "metric" => "metric".to_string(),
            _ => units, // Keep as-is if already correct or unknown
        };

        let conditions_units = units.clone();

        let mut headers = header::HeaderMap::new();
        headers.insert("User-Agent", header::HeaderValue::from_static(VERSION));
        headers.insert("Accept", header::HeaderValue::from_static("application/json"));
        headers.insert("Accept-Encoding", header::HeaderValue::from_static("deflate, gzip, br"));
        headers.insert("Connection", header::HeaderValue::from_static("close"));

        let client = Client::builder()
            .connect_timeout(Duration::from_millis(500))
            .default_headers(headers)
            .timeout(Duration::from_millis(800))
            .build()
            .unwrap();
     
        let mut lat: Option<f64> = None;
        let mut lng: Option<f64> = None;
        let mut location_name = "Unknown Location".to_string(); // Initialize location name

        if let (Some(l), Some(g)) = (lat_str, lng_str) {
            lat = Some(l.parse().map_err(|_| WeatherApiError::InvalidInput("Invalid latitude format".to_string()))?);
            lng = Some(g.parse().map_err(|_| WeatherApiError::InvalidInput("Invalid longitude format".to_string()))?);
            location_name = format!("{:.4}, {:.4}", lat.unwrap(), lng.unwrap()); // Default name if manually provided
        }

        // If lat/lng are still None, perform IP lookup
        if lat.is_none() || lng.is_none() {
            info!("Latitude or longitude not provided. Attempting IP-based geolocation...");
            let geo_data = fetch_location().await?;

            lat = Some(geo_data.latitude);
            lng = Some(geo_data.longitude);
            
            // Populate location_name from geolocation data
            location_name = if let (city, region_code) = (geo_data.city, geo_data.region_code) {
                format!("{} {}", city, region_code)
            } else {
                "Unknown Location (GeoIP Failed)".to_string()
            };
            info!("Geolocation successful: {}", location_name);
        }

        let final_lat = lat.ok_or(WeatherApiError::GeolocationError("Could not determine latitude".to_string()))?;
        let final_lng = lng.ok_or(WeatherApiError::GeolocationError("Could not determine longitude".to_string()))?;

        let base_folder = match icons {
            1 => "./assets/mono/".to_string(),
            2 => "./assets/basic/".to_string(),
            _ => "./assets/basic/".to_string()
        };
        Ok(Weather {
            active: false,
            base_url: "https://api.tomorrow.io/v4/weather/forecast".to_string(),
            api_key: api_key.to_string(),
            lat: final_lat,
            lng: final_lng,
            units,
            translate: transl.to_string(),
            client,
            icons,
            //base_folder,
            weather_data: WeatherConditions::new(location_name, conditions_units, base_folder, final_lat, final_lng),
            weather_tx: None,
            stop_sender: None,
            poll_handle: None,
            last_fetch_time: None,
        })
    }

    pub async fn get_forecast_data(&mut self, day: &Value) -> Result<WeatherData, JsonError> {

        let values = day["values"].clone();
        let mut deg = values["windDirectionAvg"].as_f64().unwrap_or(-999.0);
        if deg == -999.0 {
            deg = values["windDirection"].as_f64().unwrap_or(0.0);
        }
        
        let mut d16 = ((deg / 22.5) + 0.5) as u8;
        d16 %= 16;
        let compass_points = [
            "N",  "NNE", "NE", "ENE", "E",  "ESE",
            "SE", "SSE", "S",  "SSW", "SW", "WSW",
            "W",  "WNW", "NW", "NNW"];
        let wind_dir = compass_points[d16 as usize].to_string();
        let mut weather_code = values["weatherCodeMax"].as_i64().unwrap_or(99999);
        if weather_code == 99999 {
            weather_code = values["weatherCode"].as_i64().unwrap_or(0);
        }
        let wc: WeatherCode = self.parse_weather_code(weather_code).await;

        let moonrise_time = if "" != values["moonriseTime"].as_str().unwrap_or("") {
            let date_str = values["moonriseTime"].as_str().unwrap();
            Some(DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Local))
        } else {
            None
        };
        let moonset_time = if "" != values["moonsetTime"].as_str().unwrap_or("") {
            let date_str = values["moonsetTime"].as_str().unwrap();
            Some(DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Local))
        } else {
            None
        };
        let sunrise_time = if "" != values["sunriseTime"].as_str().unwrap_or("") {
            let date_str = values["sunriseTime"].as_str().unwrap();
            Some(DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Local))
        } else {
            None
        };
        let sunset_time = if "" != values["sunsetTime"].as_str().unwrap_or("") {
            let date_str = values["sunsetTime"].as_str().unwrap();
            Some(DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Local))
        } else {
            None
        };
        let day_time = day["time"].as_str().unwrap();

        let mut temp_apparent_avg = values["temperatureApparentAvg"].as_f64().unwrap_or(-999.0);
        if temp_apparent_avg == -999.0 {
            temp_apparent_avg = values["temperatureApparent"].as_f64().unwrap_or(0.0);
        }
        let mut temp_avg = values["temperatureAvg"].as_f64().unwrap_or(-999.0);
        if temp_avg == -999.0 {
            temp_avg = values["temperature"].as_f64().unwrap_or(0.0);
        }
        let mut temp_max = values["temperatureMax"].as_f64().unwrap_or(-999.0);
        if temp_max == -999.0 {
            temp_max = values["temperature"].as_f64().unwrap_or(0.0);
        }
        let mut temp_min = values["temperatureMin"].as_f64().unwrap_or(-999.0);
        if temp_min == -999.0 {
            temp_min = temp_max;
        }
        let mut pop = values["precipitationProbabilityAvg"].as_f64().unwrap_or(-999.0);
        if pop == -999.0 {
            pop = values["precipitationProbability"].as_f64().unwrap_or(0.0);
        }
        let mut pressure = values["pressureSurfaceLevelAvg"].as_f64().unwrap_or(-999.0);
        if pressure == -999.0 {
            pressure = values["pressureSurfaceLevel"].as_f64().unwrap_or(0.0);
        }
        let mut wind_speed = values["windSpeedAvg"].as_f64().unwrap_or(-999.0);
        if wind_speed == -999.0 {
            wind_speed = values["windSpeed"].as_f64().unwrap_or(0.0);
        }
        let mut humidity = values["humidityAvg"].as_i64().unwrap_or(999);
        if humidity == 999 {
            humidity = values["humidity"].as_i64().unwrap_or(0);
        }

        let wd = WeatherData {
            day: DateTime::parse_from_rfc3339(day_time).unwrap().with_timezone(&Local),
            humidity_avg: humidity,
            moonrise_time: moonrise_time,
            moonset_time: moonset_time,
            precipitation_probability_avg: pop,
            pressure_sea_level_avg: pressure,
            sunrise_time: sunrise_time,
            sunset_time: sunset_time,
            temperature_apparent_avg: temp_apparent_avg,
            temperature_avg: temp_avg,
            temperature_max: temp_max,
            temperature_min: temp_min,
            weather_code: wc,
            wind_direction: wind_dir,
            wind_speed_avg: wind_speed,
        };
        Ok(wd)

    }

    async fn send_with_retries<T: Serialize + ?Sized>(&mut self, params: &T, max_retries: u8) -> Result<String, reqwest::Error> {
        let mut retries = 0;
        loop {
            match self.client.get(&self.base_url.clone())
                .query(params).send().await {
                Ok(response) => {
                    let raw = response.bytes().await?;

                    // Try to decode as gzip first, fall back to plain text if it fails
                    let plain = {
                        let mut decoder = GzDecoder::new(&raw[..]);
                        let mut decoded = String::new();
                        match decoder.read_to_string(&mut decoded) {
                            Ok(_) => decoded,
                            Err(_) => {
                                // Not gzipped, treat as plain text
                                String::from_utf8_lossy(&raw).to_string()
                            }
                        }
                    };
                    return Ok(plain);
                }
                Err(e) => {
                    retries += 1;
                    if retries >= max_retries {
                        self.active = false;
                        return Err(e); // max retries reached
                    }
                    thread::sleep(Duration::from_secs(1)); // Wait before retrying
                }
            }
        }
    }

    /// Fetches current weather and 3-day forecast from Tomorrow.io.
    pub async fn fetch_weather_data(&mut self) -> Result<(), WeatherApiError> {
        info!("Fetching weather data for {}...", self.weather_data.location_name);

        // fields do not appear to work!
        let fields= [
            "temperature",
            "temperatureAvg", 
            "temperatureApparentAvg", 
            "temperatureMax", 
            "temperatureMin", 
            "temperature", 
            "humidity", 
            "precipitationType",
            "precipitationProbabilityAvg",
            "pressureSeaLevelAvg",
            "moonriseTime", 
            "moonsetTime", 
            "sunriseTime", 
            "sunsetTime", 
            "windSpeed",
            "windDirection",
            "windGust",
            "weatherCode",
            ]
            .join(",");

        let params = [
            ("location", format!("{},{}", self.lat, self.lng)),
            ("fields", fields), // this is NOT working???
            ("units", self.units.clone()),
            ("timesteps", "1h,1d".to_string()),
            ("startTime", "now".to_string()),
            ("endTime", "nowPlus8d".to_string()),
            ("dailyStartTime", "0".to_string()),
            ("apikey", self.api_key.clone()),
        ];

        let plain =
            self.send_with_retries(&params, 3)
                .await
                .map_err(|e| WeatherApiError::HttpRequestError(e))?;

        let the_json: Value = serde_json::from_str(&plain.as_str())
            .map_err(|e| WeatherApiError::DeserializationError(e))?;

        let now = Local::now();
        if let Some(timelines) = the_json.get("timelines") {
            // Current weather (next hour)
            if let Some(hours) = timelines.get("hourly").and_then(|i| i.as_array()) {
                for hour in hours.iter() {
                    let date_str = hour["time"].as_str().unwrap();
                    if DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Local) > now {
                        self.weather_data.current = self.get_forecast_data(hour)
                            .await
                            .map_err(|e| WeatherApiError::DeserializationError(e))?;
                        break;
                    }
                }
            }
            // 7-day daily forecast
            let mut idx: usize = 0;
            let now_day = now.date_naive();
            if let Some(daily) = timelines.get("daily").and_then(|i| i.as_array()) {
                for day in daily.iter().take(8) {
                    //println!("{idx}");
                    if idx > 6 { // safe! 7 days (0-6)
                        break;
                    }
                    let date_str = day["time"].as_str().unwrap();
                    if DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Local).date_naive() <= now_day {
                        continue;
                    }
                    //println!("Forecast {} for {} > {}", idx, date_str, now_day);
                    self.weather_data.forecast[idx] = self.get_forecast_data(day)
                        .await
                        .map_err(|e| WeatherApiError::DeserializationError(e))?;
                    idx += 1;
                }
            }
        } else {
            println!("No forecast data found in the response.");
        }
        self.weather_data.last_updated = Local::now();
        info!("Weather data fetched successfully.");
        self.last_fetch_time = Some(Instant::now()); // Record fetch time
        self.active = true;

        // Send update via watch channel if available (no lock!)
        if let Some(tx) = &self.weather_tx {
            let _ = tx.send(self.weather_data.clone());
        }

        Ok(())

    }

    /// Starts a background polling task to fetch weather data periodically (legacy API).
    ///
    /// This is the legacy Arc<Mutex<Weather>> version for backwards compatibility.
    /// New code should use start_polling_with_watch() instead for lock-free updates.
    pub async fn start_polling(instance: Arc<TokMutex<Self>>) -> Result<(), WeatherApiError> {
        let (tx, rx) = mpsc::channel(1);
        {
            let mut locked_instance = instance.lock().await;
            if locked_instance.poll_handle.is_some() {
                return Err(WeatherApiError::PollingError("Polling already running".to_string()));
            }
            locked_instance.stop_sender = Some(tx);
        }

        let instance_for_poll_task = Arc::clone(&instance);

        let poll_handle = tokio::spawn(async move {
            let mut rx = rx;
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(35 * 60)) => {
                        let mut locked_self = instance_for_poll_task.lock().await;
                        match locked_self.fetch_weather_data().await {
                            Ok(_) => info!("Weather polling successful."),
                            Err(e) => error!("Weather polling failed: {}", e),
                        }
                    }
                    _ = rx.recv() => {
                        info!("Weather polling thread received stop signal. Exiting.");
                        break;
                    }
                }
            }
        });

        instance.lock().await.poll_handle = Some(poll_handle);
        Ok(())
    }

    /// Starts a background polling task with lock-free updates via watch channel (new API).
    ///
    /// This takes ownership of the Weather instance and returns a watch::Receiver
    /// for lock-free access to weather updates. Preferred for new code.
    pub async fn start_polling_with_watch(mut self) -> Result<(JoinHandle<()>, watch::Receiver<WeatherConditions>), WeatherApiError> {
        if self.poll_handle.is_some() {
            return Err(WeatherApiError::PollingError("Polling already running".to_string()));
        }

        // Create watch channel for weather updates (lock-free!)
        let (weather_tx, weather_rx) = watch::channel(self.weather_data.clone());
        self.weather_tx = Some(weather_tx);

        // Create stop channel
        let (stop_tx, mut stop_rx) = mpsc::channel(1);
        self.stop_sender = Some(stop_tx);

        let poll_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(35 * 60)) => { // Poll every 35 minutes
                        match self.fetch_weather_data().await {
                            Ok(_) => info!("Weather polling successful."),
                            Err(e) => error!("Weather polling failed: {}", e),
                        }
                    }
                    _ = stop_rx.recv() => {
                        info!("Weather polling thread received stop signal. Exiting.");
                        break;
                    }
                }
            }
        });

        Ok((poll_handle, weather_rx))
    }


    /// Stops the background polling task.
    pub async fn stop_polling(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            if let Err(e) = sender.send(()).await {
                error!("Failed to send stop signal to weather polling thread: {}", e);
            }
        }
        if let Some(handle) = self.poll_handle.take() {
            // It's generally not good practice to block in Drop, but for async shutdown
            // from a sync context (like main's cleanup or explicit shutdown), awaiting is fine.
            handle.await.unwrap_or_else(|e| error!("Weather polling thread failed to join: {}", e));
        }
        info!("Weather polling stopped.");
    }
    /// Returns the weather condition description for a given weather code.
    ///
    /// # Arguments
    /// * `weather_code` - The numeric weather code as i64.
    ///
    /// # Returns
    /// A &'static str describing the condition, an icon index, and an svg filename.

    async fn parse_weather_code(&self, weather_code: i64) -> WeatherCode {
        let mut wcd = match weather_code {
            1000 | 10000 => WeatherCode {
                description: "Clear, Sunny".to_string(),
                icon: 0,
                svg: "clear_day.svg".to_string()
            },
            10001 => WeatherCode {
                description: "Clear".to_string(),
                icon: 1,
                svg: "clear_night.svg".to_string()
            }, // night
            1001 | 10010 => WeatherCode {
                description: "Cloudy".to_string(),
                icon: 2,
                svg: "mostly_cloudy_day.svg".to_string()
            },
            10011 => WeatherCode {
                description: "Cloudy".to_string(),
                icon: 2,
                svg: if self.icons == 2 {"mostly_cloudy.svg".to_string()} else {"mostly_cloudy_night.svg".to_string()}
            }, // night
            1100 | 11000 => WeatherCode {
                description: "Mostly Clear".to_string(),
                icon: 14,
                svg: "mostly_clear_day.svg".to_string()
            },
            11001 => WeatherCode {
                description: "Mostly Clear".to_string(),
                icon: 15,
                svg: "mostly_clear_night.svg".to_string()
            }, // night
            1101 | 11010 => WeatherCode {
                description: "Partly Cloudy".to_string(),
                icon: 17,
                svg: "partly_cloudy_day.svg".to_string()
            },
            11011 => WeatherCode {
                description: "Partly Cloudy".to_string(),
                icon: 18,
                svg: "partly_cloudy_night.svg".to_string()
            }, // night
            1102 | 11020 => WeatherCode {
                description: "Mostly Cloudy".to_string(),
                icon: 17,
                svg: "mostly_cloudy_day.svg".to_string()
            },
            11021 => WeatherCode {
                description: "Mostly Cloudy".to_string(),
                icon: 18,
                svg: "mostly_cloudy_night.svg".to_string()
            }, // night
            1103 | 11030 => WeatherCode {
                description: "Partly Cloudy and Mostly Clear".to_string(),
                icon: 17,
                svg: "mostly_clear_day.svg".to_string()
            },
            11031 => WeatherCode {
                description: "Partly Cloudy and Mostly Clear".to_string(),
                icon: 18,
                svg: "mostly_clear_night.svg".to_string()
            }, // night
            // Fog
            2000 | 20001 => WeatherCode {
                description: "Fog".to_string(),
                icon: 5,
                svg: if self.icons == 2 {"fog.svg".to_string()} else {"haze_fog_dust_smoke.svg".to_string()}
            },
            2100 | 21000 | 21001 => WeatherCode {
                description: "Light Fog".to_string(),
                icon: 6,
                svg: if self.icons == 2 {"fog.svg".to_string()} else {"haze_fog_dust_smoke.svg".to_string()}
            },
            2101 | 21010 | 21011 => WeatherCode {
                description: "Mostly Clear and Light Fog".to_string(),
                icon: 5,
                svg: if self.icons == 2 {"fog.svg".to_string()} else {"haze_fog_dust_smoke.svg".to_string()}
            },
            2102 | 21020 | 21021 => WeatherCode {
                description: "Partly Cloudy and Light Fog".to_string(),
                icon: 6,
                svg: if self.icons == 2 {"fog.svg".to_string()} else {"haze_fog_dust_smoke.svg".to_string()}
            },
            2103 | 21030 | 21031 => WeatherCode {
                description: "Mostly Cloudy and Light Fog".to_string(),
                icon: 5,
                svg: if self.icons == 2 {"fog.svg".to_string()} else {"haze_fog_dust_smoke.svg".to_string()}
            },
            2106 | 21060 | 21061 => WeatherCode {
                description: "Mostly Clear and Fog".to_string(),
                icon: 6,
                svg: if self.icons == 2 {"fog.svg".to_string()} else {"haze_fog_dust_smoke.svg".to_string()}
            },
            2107 | 21070 | 21071 => WeatherCode {
                description: "Partly Cloudy and Fog".to_string(),
                icon: 5,
                svg: if self.icons == 2 {"fog.svg".to_string()} else {"haze_fog_dust_smoke.svg".to_string()}
            },
            2108 | 21080 | 21081 => WeatherCode {
                description: "Mostly Cloudy and Fog".to_string(),
                icon: 6,
                svg: if self.icons == 2 {"fog.svg".to_string()} else {"haze_fog_dust_smoke.svg".to_string()}
            },
            // Drizzle
            4000 | 40000 | 40001 => WeatherCode {
                description: "Drizzle".to_string(),
                icon: 3,
                svg: "drizzle.svg".to_string()
            },
            4203 | 42030 | 42031 => WeatherCode {
                description: "Mostly Clear and Drizzle".to_string(),
                icon: 3,
                svg: "drizzle.svg".to_string()
            },
            4204 | 42040 | 42041 => WeatherCode {
                description: "Partly Cloudy and Drizzle".to_string(),
                icon: 3,
                svg: "drizzle.svg".to_string(),
            },
            4205 | 42050 | 42051 => WeatherCode {
                description: "Mostly Cloudy and Drizzle".to_string(),
                icon: 3,
                svg: "drizzle.svg".to_string()
            },
            // Rain
            4001 | 40010 | 40011 => WeatherCode {
                description: "Rain".to_string(),
                icon: 21,
                svg: if self.icons == 2 {"rain.svg".to_string()}else{"drizzle.svg".to_string()}
            },
            4200 | 42000 => WeatherCode {
                description: "Light Rain".to_string(),
                icon: 21,
                svg: if self.icons == 2 {"rain_light.svg".to_string()}else{"cloudy_with_rain_light.svg".to_string()}
            },
            4201 | 42010 => WeatherCode {
                description: "Heavy Rain".to_string(),
                icon: 20,
                svg: if self.icons == 2 {"rain_heavy.svg".to_string()}else{"heavy_rain.svg".to_string()}
            },
            4213 | 42130 | 42131 => WeatherCode {
                description: "Mostly Clear and Light Rain".to_string(),
                icon: 21,
                svg: if self.icons == 2 {"rain_light.svg".to_string()}else{"cloudy_with_rain_light.svg".to_string()}
            },
            4214 | 42140 | 42141 => WeatherCode {
                description: "Partly Cloudy and Light Rain".to_string(),
                icon: 21,
                svg: if self.icons == 2 {"rain_light.svg".to_string()}else{"cloudy_with_rain_light.svg".to_string()}
            },
            4215 | 42150 | 42151 => WeatherCode {
                description: "Mostly Cloudy and Light Rain".to_string(),
                icon: 21,
                svg: if self.icons == 2 {"rain_light.svg".to_string()}else{"cloudy_with_rain_light.svg".to_string()}
            },
            4209 | 42090 | 42091 => WeatherCode {
                description: "Mostly Clear and Rain".to_string(),
                icon: 21,
                svg: if self.icons == 2 {"rain_light.svg".to_string()}else{"cloudy_with_rain_light.svg".to_string()}
            },
            4208 | 42080 | 42081 => WeatherCode {
                description: "Partly Cloudy and Rain".to_string(),
                icon: 21,
                svg: if self.icons == 2 {"rain_light.svg".to_string()}else{"showers_rain.svg".to_string()}
            },
            4210 | 42100 | 42101 => WeatherCode {
                description: "Mostly Cloudy and Rain".to_string(),
                icon: 21,
                svg: if self.icons == 2 {"rain_light.svg".to_string()}else{"showers_rain.svg".to_string()}
            },
            4211 | 42110 | 42111 => WeatherCode {
                description: "Mostly Clear and Heavy Rain".to_string(),
                icon: 20,
                svg: if self.icons == 2 {"rain_heavy.svg".to_string()}else{"heavy_rain.svg".to_string()}
            },
            4202 | 42020 | 42021 => WeatherCode {
                description: "Partly Cloudy and Heavy Rain".to_string(),
                icon: 20,
                svg: if self.icons == 2 {"rain_heavy.svg".to_string()}else{"heavy_rain.svg".to_string()}
            },
            4212 | 42120 | 42121 => WeatherCode {
                description: "Mostly Cloudy and Heavy Rain".to_string(),
                icon: 20,
                svg: if self.icons == 2 {"rain_heavy.svg".to_string()}else{"heavy_rain.svg".to_string()}
            },
            6220 | 62200 => WeatherCode {
                description: "Light Rain and Freezing Rain".to_string(),
                icon: 7,
                svg: if self.icons == 2 {"freezing_rain.svg".to_string()}else{"icy.svg".to_string()}
            },
            6222 | 62220 => WeatherCode {
                description: "Rain and Freezing Rain".to_string(),
                icon: 8,
                svg: if self.icons == 2 {"freezing_rain.svg".to_string()}else{"icy.svg".to_string()}
            },
            // Snow
            5000 | 50000 | 50001 => WeatherCode {
                description: "Snow".to_string(),
                icon: 22,
                svg: if self.icons == 2 {"snow.svg".to_string()}else{"scattered_snow_showers_day.svg".to_string()}
            },
            5001 | 50010 | 50011 => WeatherCode {
                description: "Flurries".to_string(),
                icon: 4,
                svg: "flurries.svg".to_string(),
            },
            5100 | 51000 | 51001 => WeatherCode {
                description: "Light Snow".to_string(),
                icon: 24,
                svg: if self.icons == 2 {"snow_light.svg".to_string()}else{"light_snow.svg".to_string()}
            },
            5101 | 51010 | 51011 => WeatherCode {
                description: "Heavy Snow".to_string(),
                icon: 22,
                svg: if self.icons == 2 {"snow_heavy.svg".to_string()}else{"heavy_snow.svg".to_string()}
            },
            5102 | 51020 | 51021 => WeatherCode {
                description: "Mostly Clear and Light Snow".to_string(),
                icon: 24,
                svg: if self.icons == 2 {"snow_light.svg".to_string()}else{"cloudy_with_snow_light.svg".to_string()}
            },
            5103 | 51030 | 51031 => WeatherCode {
                description: "Partly Cloudy and Light Snow".to_string(),
                icon: 24,
                svg: if self.icons == 2 {"snow_light.svg".to_string()}else{"cloudy_with_snow_light.svg".to_string()}
            },
            5104 | 51040 | 51041 => WeatherCode {
                description: "Mostly Cloudy and Light Snow".to_string(),
                icon: 24,
                svg: if self.icons == 2 {"snow_light.svg".to_string()}else{"cloudy_with_snow_light.svg".to_string()}
            },
            5105 | 51050 | 51051 => WeatherCode {
                description: "Mostly Clear and Snow".to_string(),
                icon: 24,
                svg: if self.icons == 2 {"snow_light.svg".to_string()}else{"cloudy_with_snow_light.svg".to_string()}
            },
            5106 | 51060 | 51061 => WeatherCode {
                description: "Partly Cloudy and Snow".to_string(),
                icon: 24,
                svg: if self.icons == 2 {"snow_light.svg".to_string()}else{"cloudy_with_snow_light.svg".to_string()}
            },
            5107 | 51070 | 51071 => WeatherCode {
                description: "Mostly Cloudy and Snow".to_string(),
                icon: 24,
                svg: if self.icons == 2 {"snow_light.svg".to_string()}else{"cloudy_with_snow_light.svg".to_string()}
            },
            5119 | 51190 | 51191 => WeatherCode {
                description: "Mostly Clear and Heavy Snow".to_string(),
                icon: 22,
                svg: if self.icons == 2 {"snow_heavy.svg".to_string()}else{"heavy_snow.svg".to_string()}
            },
            5120 | 51200 | 51201 => WeatherCode {
                description: "Partly Cloudy and Heavy Snow".to_string(),
                icon: 22,
                svg: if self.icons == 2 {"snow_heavy.svg".to_string()}else{"heavy_snow.svg".to_string()}
            },
            5121 | 51210 | 51211 => WeatherCode {
                description: "Mostly Cloudy and Heavy Snow".to_string(),
                icon: 22,
                svg: if self.icons == 2 {"snow_heavy.svg".to_string()}else{"heavy_snow.svg".to_string()}
            },
            5115 | 51150 | 51151 => WeatherCode {
                description: "Mostly Clear and Flurries".to_string(),
                icon: 7,
                svg: "flurries.svg".to_string(),
            },
            5116 | 51160 | 51161 => WeatherCode {
                description: "Partly Cloudy and Flurries".to_string(),
                icon: 4,
                svg: "flurries.svg".to_string(),
            },
            5117 | 51170 | 51171 => WeatherCode {
                description: "Mostly Cloudy and Flurries".to_string(),
                icon: 4,
                svg: "flurries.svg".to_string(),
            },
            5110 | 51100 | 51101 => WeatherCode {
                description: "Drizzle and Snow".to_string(),
                icon: 7,
                svg: if self.icons == 2 {"drizzle.svg".to_string()}else{"sleet_rain.svg".to_string()}
            },
            5108 | 51080 => WeatherCode {
                description: "Rain and Snow".to_string(),
                icon: 4,
                svg: if self.icons == 2 {"snow.svg".to_string()}else{"showers_snow.svg".to_string()}
            },
            5122 | 51220 | 51221 => WeatherCode {
                description: "Drizzle and Light Snow".to_string(),
                icon: 4,
                svg: if self.icons == 2 {"snow.svg".to_string()}else{"showers_snow.svg".to_string()}
            },
            // Freezing Drizzle / Rain
            6000 | 60000 | 60001 => WeatherCode {
                description: "Freezing Drizzle".to_string(),
                icon: 8,
                svg: if self.icons == 2 {"freezing_drizzle.svg".to_string()}else{"showers_snow.svg".to_string()}
            },
            6001 | 60010 | 60011 => WeatherCode {
                description: "Freezing Rain".to_string(),
                icon: 8, 
                svg: if self.icons == 2 {"freezing_rain.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            6200 | 62000 | 62001 => WeatherCode {
                description: "Light Freezing Rain".to_string(),
                icon: 10,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            6201 | 62010 | 62011 => WeatherCode {
                description: "Heavy Freezing Rain".to_string(),
                icon: 9,
                svg: if self.icons == 2 {"freezing_rain_heavy.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            6003 | 60030 | 60031 => WeatherCode {
                description: "Mostly Clear and Freezing Drizzle".to_string(),
                icon: 8,
                svg: if self.icons == 2 {"freezing_drizzle.svg".to_string()}else{"icy.svg".to_string()}
            },
            6002 | 60020 | 60021 => WeatherCode {
                description: "Partly Cloudy and Freezing Drizzle".to_string(),
                icon: 8,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            6004 | 60040 | 60041 => WeatherCode {
                description: "Mostly Cloudy and Freezing Drizzle".to_string(),
                icon: 8,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            6204 | 62040 | 62041 => WeatherCode {
                description: "Drizzle and Freezing Drizzle".to_string(),
                icon: 8,
                svg: if self.icons == 2 {"freezing_drizzle.svg".to_string()}else{"icy.svg".to_string()}
            },
            6206 | 62060 | 62061 => WeatherCode {
                description: "Light Rain and Freezing Drizzle".to_string(),
                icon: 8,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            6205 | 62050 | 62051 => WeatherCode {
                description: "Mostly Clear and Light Freezing Rain".to_string(),
                icon: 10,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            6203 | 62030 | 62031 => WeatherCode {
                description: "Partly Cloudy and Light Freezing Rain".to_string(),
                icon: 10,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            6209 | 62090 | 62091 => WeatherCode {
                description: "Mostly Cloudy and Light Freezing Rain".to_string(),
                icon: 10,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            6213 | 62130 | 62131 => WeatherCode {
                description: "Mostly Clear and Freezing Rain".to_string(),
                icon: 10,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            6214 | 62140 | 62141 => WeatherCode {
                description: "Partly Cloudy and Freezing Rain".to_string(),
                icon: 10,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            6215 | 62150 | 62151 => WeatherCode {
                description: "Mostly Cloudy and Freezing Rain".to_string(),
                icon: 10,
                svg: if self.icons == 2 {"freezing_rain_light.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            6212 | 62120 | 62121 => WeatherCode {
                description: "Drizzle and Freezing Rain".to_string(),
                icon: 10,
                svg: if self.icons == 2 {"freezing_drizzle.svg".to_string()}else{"icy.svg".to_string()}
            },
            // Ice Pellets
            7000 | 70001 => WeatherCode {
                description: "Ice Pellets".to_string(),
                icon: 11,
                svg: if self.icons == 2 {"ice_pellets.svg".to_string()}else{"icy.svg".to_string()}
            },
            7101 | 71010 => WeatherCode {
                description: "Heavy Ice Pellets".to_string(),
                icon: 12,
                svg: if self.icons == 2 {"ice_pellets_heavy.svg".to_string()}else{"icy.svg".to_string()}
            },
            7102 | 71020 | 71021 => WeatherCode {
                description: "Light Ice Pellets".to_string(),
                icon: 13,
                svg: if self.icons == 2 {"ice_pellets_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            7105 | 71050 | 71051 => WeatherCode {
                description: "Drizzle and Ice Pellets".to_string(),
                icon: 13,
                svg: if self.icons == 2 {"ice_pellets_light.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            7106 | 71060 | 71061 => WeatherCode {
                description: "Freezing Rain and Ice Pellets".to_string(),
                icon: 13,
                svg: if self.icons == 2 {"ice_pellets_light.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            7115 | 71150 | 71151 => WeatherCode {
                description: "Light Rain and Ice Pellets".to_string(),
                icon: 13,
                svg: if self.icons == 2 {"ice_pellets_light.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            7117 | 71170 | 71171 => WeatherCode {
                description: "Rain and Ice Pellets".to_string(),
                icon: 13,
                svg: if self.icons == 2 {"ice_pellets_light.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            7103 | 71030 | 71031 => WeatherCode {
                description: "Freezing Rain and Heavy Ice Pellets".to_string(),
                icon: 12,
                svg: if self.icons == 2 {"ice_pellets_heavy.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            7113 | 71130 | 71131 => WeatherCode {
                description: "Mostly Clear and Heavy Ice Pellets".to_string(),
                icon: 12,
                svg: if self.icons == 2 {"ice_pellets_heavy.svg".to_string()}else{"mixed_rain_hail_sleet.svg".to_string()}
            },
            7114 | 71140 | 71141 => WeatherCode {
                description: "Partly Cloudy and Heavy Ice Pellets".to_string(),
                icon: 12,
                svg: if self.icons == 2 {"ice_pellets_heavy.svg".to_string()}else{"icy.svg".to_string()}
            },
            7116 | 71160 | 71161 => WeatherCode {
                description: "Mostly Cloudy and Heavy Ice Pellets".to_string(),
                icon: 12,
                svg: if self.icons == 2 {"ice_pellets_heavy.svg".to_string()}else{"icy.svg".to_string()}
            },
            7108 | 71080 | 71081 => WeatherCode {
                description: "Mostly Clear and Ice Pellets".to_string(),
                icon: 11,
                svg: if self.icons == 2 {"ice_pellets.svg".to_string()}else{"icy.svg".to_string()}
            },
            7107 | 71070 | 71071 => WeatherCode {
                description: "Partly Cloudy and Ice Pellets".to_string(),
                icon: 11,
                svg: if self.icons == 2 {"ice_pellets.svg".to_string()}else{"icy.svg".to_string()}
            },
            7109 | 71090 | 71091 => WeatherCode {
                description: "Mostly Cloudy and Ice Pellets".to_string(),
                icon: 11,
                svg: if self.icons == 2 {"ice_pellets.svg".to_string()}else{"icy.svg".to_string()}
            },
            7110 | 71100 | 71101 => WeatherCode {
                description: "Mostly Clear and Light Ice Pellets".to_string(),
                icon: 11,
                svg: if self.icons == 2 {"ice_pellets.svg".to_string()}else{"icy.svg".to_string()}
            },
            7111 | 71110 | 71111 => WeatherCode {
                description: "Partly Cloudy and Light Ice Pellets".to_string(),
                icon: 11,
                svg: if self.icons == 2 {"ice_pellets_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            7112 | 71120 | 71121 => WeatherCode {
                description: "Mostly Cloudy and Light Ice Pellets".to_string(),
                icon: 11,
                svg: if self.icons == 2 {"ice_pellets_light.svg".to_string()}else{"icy.svg".to_string()}
            },
            // Thunderstorm
            8000 | 80000 => WeatherCode {
                description: "Thunderstorm".to_string(),
                icon: 25,
                svg: if self.icons == 2 {"tstorm.svg".to_string()}else{"strong_thunderstorms.svg".to_string()}
            },
            8001 | 80010 | 80011 => WeatherCode {
                description: "Mostly Clear and Thunderstorm".to_string(),
                icon: 25,
                svg: if self.icons == 2 {"tstorm.svg".to_string()}else{"isolated_thunderstorms.svg".to_string()}
            },
            8002 | 80020 | 80021 => WeatherCode {
                description: "Mostly Cloudy and Thunderstorm".to_string(),
                icon: 25,
                svg: if self.icons == 2 {"tstorm.svg".to_string()}else{"isolated_thunderstorms.svg".to_string()}
            },
            8003 | 80030 | 80031 => WeatherCode {
                description: "Partly Cloudy and Thunderstorm".to_string(),
                icon: 25,
                svg: "isolated_thunderstorms.svg".to_string(),
            },
            _ => WeatherCode {
                description: "Unknown".to_string(),
                icon: 26,
                svg: "no_data.svg".to_string(),
            },
        };
        if self.translate.len() > 0 {
            if self.translate != "en" {
                let text = wcd.description.clone();
                let mut tl8 = Translation::new(self.translate.as_str()).unwrap();
                wcd.description = tl8.translate_phrase(text.as_str())
                    .await
                    .unwrap()
                    .to_string();
            }
        }
        wcd
    }


}

// Implement Drop trait to stop the background thread when Weather goes out of scope
impl Drop for Weather {
    fn drop(&mut self) {
        info!("Weather dropped. Attempting to stop polling thread (if running)...");
        // Due to Drop not being async, we can only try_send.
        // The actual await for joining the handle would need a dedicated async shutdown.
        if let Some(sender) = self.stop_sender.take() {
            if let Err(e) = sender.try_send(()) {
                error!("Failed to send stop signal to weather polling thread on drop: {}", e);
            }
        }
    }
}

