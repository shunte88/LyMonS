use anyhow::{bail};
use serde::{Deserialize, Serialize};
use reqwest::{Url, Client, header};
use std::fmt::{self, Display};
use std::time::{Duration, Instant}; // Added Instant for tracking last update
use log::{info, error, debug};
use tokio::sync::{mpsc, Mutex as TokMutex};
use tokio::task::JoinHandle;
use std::sync::Arc;
use chrono::{DateTime, Utc}; // Import Duration for adding to DateTime

use flate2::read::GzDecoder;
use std::io::Read;

use crate::geoloc::{fetch_location};
use crate::translate::Translation;

// Custom error type for weather API operations.
#[allow(dead_code)]
#[derive(Debug)]
pub enum WeatherApiError {
    HttpRequestError(reqwest::Error),
    SerializationError(serde_json::Error),
    DeserializationError(serde_json::Error),
    ApiKeyError(String), // For specific API error messages
    GeolocationError(String),
    InvalidInput(String),
    PollingError(String),
    ApiError(String), // For specific API error messages
    MissingData(String),
    TranslationError(String),
}

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

impl From<serde_json::Error> for WeatherApiError {
    fn from(err: serde_json::Error) -> Self {
        WeatherApiError::SerializationError(err) // Can be refined to DeserializationError later if context allows
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct WeatherCode {
	pub description: String,
	pub icon: u8,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct WeatherData {
    pub day: DateTime<Utc>,
    pub humidity_avg: i64,
    pub moonrise_time: Option<DateTime<Utc>>,
    pub moonset_time: Option<DateTime<Utc>>,
    pub precipitation_probability_avg: f64,
    pub pressure_sea_level_avg: f64,
    pub sunrise_time: Option<DateTime<Utc>>,
    pub sunset_time: Option<DateTime<Utc>>,
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
    pub location_name: String,
    pub temperature_units: String, // "C" or "F"
    pub windspeed_units: String, // "km/h" or "mph"
    pub current: WeatherData,
    pub forecast: Vec<WeatherData>,
    pub last_updated: DateTime<Utc>,
}

// Main Weather client
#[derive(Debug)]
pub struct Weather {
    base_url: String,
    api_key: String,
    lat: f64,
    lng: f64,
    units: String, // "metric" or "imperial"
    translate: String,
    client: Client,
    pub weather_data: WeatherConditions, // Public so OledDisplay can read it
    stop_sender: Option<mpsc::Sender<()>>,
    poll_handle: Option<JoinHandle<()>>,
    pub last_fetch_time: Option<Instant>, // To track when data was last fetched
}

impl WeatherConditions {
    pub fn new(location_name: String, units: String) -> Self {
        Self {
            location_name,
            temperature_units: if units == "imperial" { "F" } else { "C" }.to_string(),
            windspeed_units: if units == "imperial" { "mph" } else { "km/h" }.to_string(),
            current: WeatherData::default(),
            forecast: vec![WeatherData::default(); 3],
            last_updated: Utc::now(),
        }
    }
}

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

        for part in parts.iter().skip(1) {
            if part.starts_with("lat=") {
                lat_str = Some(part.trim_start_matches("lat=").to_string());
            } else if part.starts_with("lng=") {
                lng_str = Some(part.trim_start_matches("lng=").to_string());
            } else if part.starts_with("lang=") {
                transl = part.trim_start_matches("lang=").to_string();
            } else if part.starts_with("units=") {
                units = part.trim_start_matches("units=").to_string();
            }
        }
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

        Ok(Weather {
            base_url: "https://api.tomorrow.io/v4/weather/forecast".to_string(),
            api_key: api_key.to_string(),
            lat: final_lat,
            lng: final_lng,
            units,
            translate: transl.to_string(),
            client,
            weather_data: WeatherConditions::new(location_name, conditions_units),
            stop_sender: None,
            poll_handle: None,
            last_fetch_time: None,
        })
    }

    /// Fetches current weather and 3-day forecast from Tomorrow.io.
    pub async fn fetch_weather_data(&mut self) -> Result<(), WeatherApiError> {
        info!("Fetching weather data for {}...", self.weather_data.location_name);

        // fields do not appear to work!
        let fields: Vec<&'static str> = vec![
            "temperature", "weatherCode", "humidity", "windSpeed", "precipitationType",
            "temperatureMax", "temperatureMin", "sunriseTime", "sunsetTime", "precipitationProbability",
            "windSpeed","windDirection","windGust",];
        let params = [
            ("location", format!("{},{}", self.lat, self.lng)),
            ("fields", fields.join(",")), // this is NOT working???
            ("units", self.units.clone()),
            ("timesteps", "1d".to_string()),
            ("startTime", "now".to_string()),
            ("endTime", "nowPlus4d".to_string()),
            ("dailyStartTime", "6".to_string()),
            ("apikey", self.api_key.clone()),
        ];

        let response = self.client.get(&self.base_url.clone())
        .query(&params)
        .send()
        .await?;
       
        // take control - ensure we're on the green path
        let raw = response.bytes().await?;
        let mut decoder = GzDecoder::new(&raw[..]);
        let mut plain = String::new();
        decoder.read_to_string(&mut plain).unwrap();
        
        let the_json: serde_json::Value = serde_json::from_str(&plain.as_str())
            .map_err(|e| WeatherApiError::DeserializationError(e))?;

        let mut idx = 0;
        if let Some(days) = the_json["timelines"]["daily"].as_array() {
            for day in days {

                let values = day["values"].clone();
                let deg = values["windDirectionAvg"].as_f64().unwrap();
                let mut d16 = ((deg / 22.5) + 0.5) as u8;
                d16 %= 16;
                let compass_points = [
                    "N",  "NNE", "NE", "ENE", "E",  "ESE",
                    "SE", "SSE", "S",  "SSW", "SW", "WSW",
                    "W",  "WNW", "NW", "NNW"];
                let wind_dir = compass_points[d16 as usize].to_string();
                let wc: WeatherCode = self.parse_weather_code(values["weatherCodeMax"].as_i64().unwrap()).await;

                let moonrise_time = if "" != values["moonriseTime"].as_str().unwrap_or("") {
                    let date_str = values["moonriseTime"].as_str().unwrap();
                    Some(DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Utc))
                } else {
                    None
                };
                let moonset_time = if "" != values["moonsetTime"].as_str().unwrap_or("") {
                    let date_str = values["moonsetTime"].as_str().unwrap();
                    Some(DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Utc))
                } else {
                    None
                };
                let sunrise_time = if "" != values["sunriseTime"].as_str().unwrap_or("") {
                    let date_str = values["sunriseTime"].as_str().unwrap();
                    Some(DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Utc))
                } else {
                    None
                };
                let sunset_time = if "" != values["sunsetTime"].as_str().unwrap_or("") {
                    let date_str = values["sunsetTime"].as_str().unwrap();
                    Some(DateTime::parse_from_rfc3339(date_str).unwrap().with_timezone(&Utc))
                } else {
                    None
                };
                let day_time = day["time"].as_str().unwrap();

                let wd = WeatherData {
                    day: DateTime::parse_from_rfc3339(day_time).unwrap().with_timezone(&Utc),
                    humidity_avg: values["humidityAvg"].as_i64().unwrap_or(0),
                    moonrise_time: moonrise_time,
                    moonset_time: moonset_time,
                    precipitation_probability_avg: values["precipitationProbabilityAvg"].as_f64().unwrap_or(0.0),
                    pressure_sea_level_avg: values["pressureSeaLevelAvg"].as_f64().unwrap_or(0.0),
                    sunrise_time: sunrise_time,
                    sunset_time: sunset_time,
                    temperature_apparent_avg: values["temperatureApparentAvg"].as_f64().unwrap_or(0.0),
                    temperature_avg: values["temperatureAvg"].as_f64().unwrap_or(0.0),
                    temperature_max: values["temperatureMax"].as_f64().unwrap_or(0.0),
                    temperature_min: values["temperatureMin"].as_f64().unwrap_or(0.0),
                    weather_code: wc,
                    wind_direction: wind_dir,
                    wind_speed_avg: values["windSpeedAvg"].as_f64().unwrap_or(0.0),
                };

                if idx == 0 {
                    self.weather_data.current = wd;
                } else {
                    self.weather_data.forecast[idx-1] = wd;
                }
                idx += 1;
                if idx > 3 {
                    break;
                }
            }
        }

        debug!("{:#?}", self.weather_data);
        self.weather_data.last_updated = Utc::now();
        info!("Weather data fetched successfully.");
        self.last_fetch_time = Some(Instant::now()); // Record fetch time
        Ok(())

    }

    /// Starts a background polling task to fetch weather data periodically.
    pub async fn start_polling(instance: Arc<TokMutex<Self>>) -> Result<(), WeatherApiError> {
        let (tx, rx) = mpsc::channel(1);
        {
            let mut locked_instance = instance.lock().await;
            if locked_instance.poll_handle.is_some() {
                return Err(WeatherApiError::PollingError("Polling already running".to_string()));
            }
            locked_instance.stop_sender = Some(tx);
        }

        // Clone `instance` here to move a copy into the spawned task,
        // leaving the original `instance` available in `start_polling`'s scope.
        let instance_for_poll_task = Arc::clone(&instance);

        let poll_handle = tokio::spawn(async move {
            let mut rx = rx;
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(35 * 60)) => { // Poll every 35 minutes
                        let mut locked_self = instance_for_poll_task.lock().await; // Use the cloned instance
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

        // The original `instance` is still valid here.
        instance.lock().await.poll_handle = Some(poll_handle);
        Ok(())
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
    /// A &'static str describing the condition.
    async fn parse_weather_code(&self, weather_code: i64) -> WeatherCode {
        let mut wcd = match weather_code {
            1000 | 10000 => WeatherCode {
                description: "Clear, Sunny".to_string(),
                icon: 0,
            },
            10001 => WeatherCode {
                description: "Clear".to_string(),
                icon: 1,
            }, // night
            1001 | 10010 => WeatherCode {
                description: "Cloudy".to_string(),
                icon: 2,
            },
            10011 => WeatherCode {
                description: "Cloudy".to_string(),
                icon: 2,
            }, // night
            1100 | 11000 => WeatherCode {
                description: "Mostly Clear".to_string(),
                icon: 14,
            },
            11001 => WeatherCode {
                description: "Mostly Clear".to_string(),
                icon: 15,
            }, // night
            1101 | 11010 => WeatherCode {
                description: "Partly Cloudy".to_string(),
                icon: 17,
            },
            11011 => WeatherCode {
                description: "Partly Cloudy".to_string(),
                icon: 18,
            }, // night
            1102 | 11020 => WeatherCode {
                description: "Mostly Cloudy".to_string(),
                icon: 16,
            },
            11021 => WeatherCode {
                description: "Mostly Cloudy".to_string(),
                icon: 18,
            }, // night
            1103 | 11030 => WeatherCode {
                description: "Partly Cloudy and Mostly Clear".to_string(),
                icon: 17,
            },
            11031 => WeatherCode {
                description: "Partly Cloudy and Mostly Clear".to_string(),
                icon: 18,
            }, // night
            // Fog
            2000 | 20001 => WeatherCode {
                description: "Fog".to_string(),
                icon: 5,
            },
            2100 | 21000 | 21001 => WeatherCode {
                description: "Light Fog".to_string(),
                icon: 6,
            },
            2101 | 21010 | 21011 => WeatherCode {
                description: "Mostly Clear and Light Fog".to_string(),
                icon: 5,
            },
            2102 | 21020 | 21021 => WeatherCode {
                description: "Partly Cloudy and Light Fog".to_string(),
                icon: 6,
            },
            2103 | 21030 | 21031 => WeatherCode {
                description: "Mostly Cloudy and Light Fog".to_string(),
                icon: 5,
            },
            2106 | 21060 | 21061 => WeatherCode {
                description: "Mostly Clear and Fog".to_string(),
                icon: 6,
            },
            2107 | 21070 | 21071 => WeatherCode {
                description: "Partly Cloudy and Fog".to_string(),
                icon: 5,
            },
            2108 | 21080 | 21081 => WeatherCode {
                description: "Mostly Cloudy and Fog".to_string(),
                icon: 6,
            },
            // Drizzle
            4000 | 40000 | 40001 => WeatherCode {
                description: "Drizzle".to_string(),
                icon: 3,
            },
            4203 | 42030 | 42031 => WeatherCode {
                description: "Mostly Clear and Drizzle".to_string(),
                icon: 3,
            },
            4204 | 42040 | 42041 => WeatherCode {
                description: "Partly Cloudy and Drizzle".to_string(),
                icon: 3,
            },
            4205 | 42050 | 42051 => WeatherCode {
                description: "Mostly Cloudy and Drizzle".to_string(),
                icon: 3,
            },
            // Rain
            4001 | 40010 | 40011 => WeatherCode {
                description: "Rain".to_string(),
                icon: 21,
            },
            4200 | 42000 => WeatherCode {
                description: "Light Rain".to_string(),
                icon: 21,
            },
            4201 | 42010 => WeatherCode {
                description: "Heavy Rain".to_string(),
                icon: 20,
            },
            4213 | 42130 | 42131 => WeatherCode {
                description: "Mostly Clear and Light Rain".to_string(),
                icon: 21,
            },
            4214 | 42140 | 42141 => WeatherCode {
                description: "Partly Cloudy and Light Rain".to_string(),
                icon: 21,
            },
            4215 | 42150 | 42151 => WeatherCode {
                description: "Mostly Cloudy and Light Rain".to_string(),
                icon: 21,
            },
            4209 | 42090 | 42091 => WeatherCode {
                description: "Mostly Clear and Rain".to_string(),
                icon: 21,
            },
            4208 | 42080 | 42081 => WeatherCode {
                description: "Partly Cloudy and Rain".to_string(),
                icon: 21,
            },
            4210 | 42100 | 42101 => WeatherCode {
                description: "Mostly Cloudy and Rain".to_string(),
                icon: 21,
            },
            4211 | 42110 | 42111 => WeatherCode {
                description: "Mostly Clear and Heavy Rain".to_string(),
                icon: 20,
            },
            4202 | 42020 | 42021 => WeatherCode {
                description: "Partly Cloudy and Heavy Rain".to_string(),
                icon: 20,
            },
            4212 | 42120 | 42121 => WeatherCode {
                description: "Mostly Cloudy and Heavy Rain".to_string(),
                icon: 20,
            },
            6220 | 62200 => WeatherCode {
                description: "Light Rain and Freezing Rain".to_string(),
                icon: 7,
            },
            6222 | 62220 => WeatherCode {
                description: "Rain and Freezing Rain".to_string(),
                icon: 8,
            },
            // Snow
            5000 | 50000 | 50001 => WeatherCode {
                description: "Snow".to_string(),
                icon: 22,
            },
            5001 | 50010 | 50011 => WeatherCode {
                description: "Flurries".to_string(),
                icon: 4,
            },
            5100 | 51000 | 51001 => WeatherCode {
                description: "Light Snow".to_string(),
                icon: 24,
            },
            5101 | 51010 | 51011 => WeatherCode {
                description: "Heavy Snow".to_string(),
                icon: 22,
            },
            5102 | 51020 | 51021 => WeatherCode {
                description: "Mostly Clear and Light Snow".to_string(),
                icon: 24,
            },
            5103 | 51030 | 51031 => WeatherCode {
                description: "Partly Cloudy and Light Snow".to_string(),
                icon: 24,
            },
            5104 | 51040 | 51041 => WeatherCode {
                description: "Mostly Cloudy and Light Snow".to_string(),
                icon: 24,
            },
            5105 | 51050 | 51051 => WeatherCode {
                description: "Mostly Clear and Snow".to_string(),
                icon: 24,
            },
            5106 | 51060 | 51061 => WeatherCode {
                description: "Partly Cloudy and Snow".to_string(),
                icon: 24,
            },
            5107 | 51070 | 51071 => WeatherCode {
                description: "Mostly Cloudy and Snow".to_string(),
                icon: 24,
            },
            5119 | 51190 | 51191 => WeatherCode {
                description: "Mostly Clear and Heavy Snow".to_string(),
                icon: 22,
            },
            5120 | 51200 | 51201 => WeatherCode {
                description: "Partly Cloudy and Heavy Snow".to_string(),
                icon: 22,
            },
            5121 | 51210 | 51211 => WeatherCode {
                description: "Mostly Cloudy and Heavy Snow".to_string(),
                icon: 22,
            },
            5115 | 51150 | 51151 => WeatherCode {
                description: "Mostly Clear and Flurries".to_string(),
                icon: 7,
            },
            5116 | 51160 | 51161 => WeatherCode {
                description: "Partly Cloudy and Flurries".to_string(),
                icon: 4,
            },
            5117 | 51170 | 51171 => WeatherCode {
                description: "Mostly Cloudy and Flurries".to_string(),
                icon: 4,
            },
            5110 | 51100 | 51101 => WeatherCode {
                description: "Drizzle and Snow".to_string(),
                icon: 7,
            },
            5108 | 51080 => WeatherCode {
                description: "Rain and Snow".to_string(),
                icon: 4,
            },
            5122 | 51220 | 51221 => WeatherCode {
                description: "Drizzle and Light Snow".to_string(),
                icon: 4,
            },
            // Freezing Drizzle / Rain
            6000 | 60000 | 60001 => WeatherCode {
                description: "Freezing Drizzle".to_string(),
                icon: 8,
            },
            6001 | 60010 | 60011 => WeatherCode {
                description: "Freezing Rain".to_string(),
                icon: 8,
            },
            6200 | 62000 | 62001 => WeatherCode {
                description: "Light Freezing Rain".to_string(),
                icon: 10,
            },
            6201 | 62010 | 62011 => WeatherCode {
                description: "Heavy Freezing Rain".to_string(),
                icon: 9,
            },
            6003 | 60030 | 60031 => WeatherCode {
                description: "Mostly Clear and Freezing Drizzle".to_string(),
                icon: 8,
            },
            6002 | 60020 | 60021 => WeatherCode {
                description: "Partly Cloudy and Freezing Drizzle".to_string(),
                icon: 8,
            },
            6004 | 60040 | 60041 => WeatherCode {
                description: "Mostly Cloudy and Freezing Drizzle".to_string(),
                icon: 8,
            },
            6204 | 62040 | 62041 => WeatherCode {
                description: "Drizzle and Freezing Drizzle".to_string(),
                icon: 8,
            },
            6206 | 62060 | 62061 => WeatherCode {
                description: "Light Rain and Freezing Drizzle".to_string(),
                icon: 8,
            },
            6205 | 62050 | 62051 => WeatherCode {
                description: "Mostly Clear and Light Freezing Rain".to_string(),
                icon: 10,
            },
            6203 | 62030 | 62031 => WeatherCode {
                description: "Partly Cloudy and Light Freezing Rain".to_string(),
                icon: 10,
            },
            6209 | 62090 | 62091 => WeatherCode {
                description: "Mostly Cloudy and Light Freezing Rain".to_string(),
                icon: 10,
            },
            6213 | 62130 | 62131 => WeatherCode {
                description: "Mostly Clear and Freezing Rain".to_string(),
                icon: 10,
            },
            6214 | 62140 | 62141 => WeatherCode {
                description: "Partly Cloudy and Freezing Rain".to_string(),
                icon: 10,
            },
            6215 | 62150 | 62151 => WeatherCode {
                description: "Mostly Cloudy and Freezing Rain".to_string(),
                icon: 10,
            },
            6212 | 62120 | 62121 => WeatherCode {
                description: "Drizzle and Freezing Rain".to_string(),
                icon: 10,
            },
            // Ice Pellets
            7000 | 70001 => WeatherCode {
                description: "Ice Pellets".to_string(),
                icon: 11,
            },
            7101 | 71010 => WeatherCode {
                description: "Heavy Ice Pellets".to_string(),
                icon: 12,
            },
            7102 | 71020 | 71021 => WeatherCode {
                description: "Light Ice Pellets".to_string(),
                icon: 13,
            },
            7105 | 71050 | 71051 => WeatherCode {
                description: "Drizzle and Ice Pellets".to_string(),
                icon: 13,
            },
            7106 | 71060 | 71061 => WeatherCode {
                description: "Freezing Rain and Ice Pellets".to_string(),
                icon: 13,
            },
            7115 | 71150 | 71151 => WeatherCode {
                description: "Light Rain and Ice Pellets".to_string(),
                icon: 13,
            },
            7117 | 71170 | 71171 => WeatherCode {
                description: "Rain and Ice Pellets".to_string(),
                icon: 13,
            },
            7103 | 71030 | 71031 => WeatherCode {
                description: "Freezing Rain and Heavy Ice Pellets".to_string(),
                icon: 12,
            },
            7113 | 71130 | 71131 => WeatherCode {
                description: "Mostly Clear and Heavy Ice Pellets".to_string(),
                icon: 12,
            },
            7114 | 71140 | 71141 => WeatherCode {
                description: "Partly Cloudy and Heavy Ice Pellets".to_string(),
                icon: 12,
            },
            7116 | 71160 | 71161 => WeatherCode {
                description: "Mostly Cloudy and Heavy Ice Pellets".to_string(),
                icon: 12,
            },
            7108 | 71080 | 71081 => WeatherCode {
                description: "Mostly Clear and Ice Pellets".to_string(),
                icon: 11,
            },
            7107 | 71070 | 71071 => WeatherCode {
                description: "Partly Cloudy and Ice Pellets".to_string(),
                icon: 11,
            },
            7109 | 71090 | 71091 => WeatherCode {
                description: "Mostly Cloudy and Ice Pellets".to_string(),
                icon: 11,
            },
            7110 | 71100 | 71101 => WeatherCode {
                description: "Mostly Clear and Light Ice Pellets".to_string(),
                icon: 11,
            },
            7111 | 71110 | 71111 => WeatherCode {
                description: "Partly Cloudy and Light Ice Pellets".to_string(),
                icon: 11,
            },
            7112 | 71120 | 71121 => WeatherCode {
                description: "Mostly Cloudy and Light Ice Pellets".to_string(),
                icon: 11,
            },
            // Thunderstorm
            8000 | 80000 => WeatherCode {
                description: "Thunderstorm".to_string(),
                icon: 25,
            },
            8001 | 80010 | 80011 => WeatherCode {
                description: "Mostly Clear and Thunderstorm".to_string(),
                icon: 25,
            },
            8002 | 80020 | 80021 => WeatherCode {
                description: "Mostly Cloudy and Thunderstorm".to_string(),
                icon: 25,
            },
            8003 | 80030 | 80031 => WeatherCode {
                description: "Partly Cloudy and Thunderstorm".to_string(),
                icon: 25,
            },
            _ => WeatherCode {
                description: "Unknown".to_string(),
                icon: 26,
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

/*

        (wiconmap_t){"clearD", "Clear", 0, true},
        (wiconmap_t){"clearN", "Clear", 1, true},
        (wiconmap_t){"clear", "Clear", 0, true},
        (wiconmap_t){"cloudy", "Cloudy", 2, true},
        (wiconmap_t){"drizzle", "Drizzle", 3, true},
        (wiconmap_t){"flurries", "Snow Flurries", 4, true},
        (wiconmap_t){"fog", "Fog", 5, true},
        (wiconmap_t){"fog_light", "Mist", 6, true},
        (wiconmap_t){"freezing_drizzle", "Freezing Drizzle", 7, true},
        (wiconmap_t){"freezing_rain", "Freezing Rain", 8, true},
        (wiconmap_t){"freezing_rain_heavy", "Frezing Rain", 9, true},
        (wiconmap_t){"freezing_rain_light", "Freezing Rain", 10, true},
        (wiconmap_t){"ice_pellets", "Hail", 11, true},
        (wiconmap_t){"ice_pellets_heavy", "Hail", 12, true},
        (wiconmap_t){"ice_pellets_light", "Hail", 13, true},
        (wiconmap_t){"mostly_clearD", "Mostly Clear", 14, true},
        (wiconmap_t){"mostly_clearN", "Mostly Clear", 15, true},
        (wiconmap_t){"mostly_clear", "Mostly Clear", 14, true},
        (wiconmap_t){"partly_cloudyD", "Partly Cloudy", 17, true},
        (wiconmap_t){"partly_cloudyN", "Partly Cloudy", 18, true},
        (wiconmap_t){"partly_cloudy", "Partly Cloudy", 17, true},
        (wiconmap_t){"mostly_cloudyD", "Mostly Cloudy", 16, true},
        (wiconmap_t){"mostly_cloudyN", "Mostly Cloudy", 18, true},
        (wiconmap_t){"mostly_cloudy", "Mostly Cloudy", 16, true},
        (wiconmap_t){"rain", "Rain", 19, true},
        (wiconmap_t){"rain_heavy", "Heavy Rain", 20, true},
        (wiconmap_t){"rain_light", "Showers", 21, true},
        (wiconmap_t){"snow", "Snow", 22, true},
        (wiconmap_t){"snow_heavy", "Heavy Snow", 23, true},
        (wiconmap_t){"snow_light", "Snow Showers", 24, true},
        (wiconmap_t){"tstorm", "Thunderstorm", 25, true},
        (wiconmap_t){"???", "Unknown", 26, true},

*/