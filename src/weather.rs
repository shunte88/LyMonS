use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::Client;
use std::fmt::{self, Display};
use std::time::{Duration, Instant}; // Added Instant for tracking last update
use log::{info, error, debug};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use std::sync::Arc;
use chrono::{DateTime, Utc, Timelike, Duration as ChronoDuration}; // Import Duration for adding to DateTime

// Custom error type for weather API operations.
#[derive(Debug)]
pub enum WeatherApiError {
    HttpRequestError(reqwest::Error),
    SerializationError(serde_json::Error),
    DeserializationError(serde_json::Error),
    ApiError(String), // For specific API error messages
    GeolocationError(String),
    MissingData(String),
    InvalidInput(String),
    PollingError(String),
}

impl Display for WeatherApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WeatherApiError::HttpRequestError(e) => write!(f, "HTTP request error: {}", e),
            WeatherApiError::SerializationError(e) => write!(f, "JSON serialization error: {}", e),
            WeatherApiError::DeserializationError(e) => write!(f, "JSON deserialization error: {}", e),
            WeatherApiError::ApiError(msg) => write!(f, "Tomorrow.io API error: {}", msg),
            WeatherApiError::GeolocationError(msg) => write!(f, "Geolocation error: {}", msg),
            WeatherApiError::MissingData(msg) => write!(f, "Missing weather data: {}", msg),
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

// Structs for parsing Tomorrow.io API responses
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Added PartialEq for easier comparison in display logic
#[serde(rename_all = "camelCase")]
pub struct CurrentWeatherFields {
    pub temperature: Option<f32>,
    pub weather_code: Option<i32>,
    pub humidity: Option<f32>,
    pub wind_speed: Option<String>,
    pub precipitation_type: Option<i32>,
    pub feels_like: Option<f32>,
    pub baro_pressure: Option<f32>,
    pub visibility: Option<f32>,
    #[serde(deserialize_with="deserialize_compass_direction")]
    pub wind_direction: Option<f32>,
    pub precipitation_probability: Option<f32>,
    pub precipitation: Option<String>,
    pub precipitation_min: Option<f32>,
    pub precipitation_max: Option<f32>,
    #[serde(with = "chrono::serde::ts_seconds_option")] // Deserialize Unix timestamp to DateTime<Utc>
    pub sunrise: Option<DateTime<Utc>>,
    #[serde(with = "chrono::serde::ts_seconds_option")] // Deserialize Unix timestamp to DateTime<Utc>
    pub sunset: Option<DateTime<Utc>>,
    #[serde(with = "chrono::serde::ts_seconds_option")] // Deserialize Unix timestamp to DateTime<Utc>
    pub observation_time: Option<DateTime<Utc>>, // data date or forecast period
    }

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Added PartialEq
#[serde(rename_all = "camelCase")]
pub struct DailyForecastFields {
    pub temperature_max: Option<f32>,
    pub temperature_min: Option<f32>,
    pub weather_code: Option<i32>,
    #[serde(with = "chrono::serde::ts_seconds_option")] // Deserialize Unix timestamp to DateTime<Utc>
    pub sunrise_time: Option<DateTime<Utc>>,
    pub precipitation_probability: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Interval {
    #[serde(rename = "startTime")]
    pub start_time: DateTime<Utc>,
    pub values: Value, // Use Value to dynamically parse based on timestep
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelinesData {
    pub intervals: Vec<Interval>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TomorrowIOResponse {
    pub data: TimelinesData,
    pub warning: Option<Value>,
    pub error: Option<Value>,
}

// Struct for IP Geolocation API response
#[derive(Debug, Serialize, Deserialize)]
pub struct GeolocationResponse {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub city: Option<String>,
    pub region_code: Option<String>,
    pub utc_offset: Option<String>,
}

// Main weather data struct
#[derive(Debug, Clone, PartialEq)] // Added PartialEq
pub struct WeatherData {
    pub current: Option<CurrentWeatherFields>,
    pub forecast: Vec<DailyForecastFields>,
    pub last_updated: Option<DateTime<Utc>>,
    pub location_name: String, // e.g., "Cambridge, MA, USA"
}

impl Default for WeatherData {
    fn default() -> Self {
        WeatherData {
            current: None,
            forecast: Vec::new(),
            last_updated: None,
            location_name: "Unknown Location".to_string(),
        }
    }
}

// Main Weather client
#[derive(Debug)]
pub struct Weather {
    api_key: String,
    lat: f64,
    lng: f64,
    units: String, // "metric" or "imperial"
    client: Client,
    pub weather_data: WeatherData, // Public so OledDisplay can read it
    stop_sender: Option<mpsc::Sender<()>>,
    poll_handle: Option<JoinHandle<()>>,
    pub last_fetch_time: Option<Instant>, // To track when data was last fetched
}

impl Weather {
    /// Creates a new `Weather` instance. Performs IP lookup if lat/lng are not provided.
    pub async fn new(
        api_key: String,
        lat_str: Option<String>,
        lng_str: Option<String>,
        units: String,
    ) -> Result<Self, WeatherApiError> {
        let client = Client::new();
        let mut lat: Option<f64> = None;
        let mut lng: Option<f64> = None;
        let mut location_name = "Unknown Location".to_string(); // Initialize location name

        if let (Some(l), Some(g)) = (lat_str, lng_str) {
            lat = Some(l.parse().map_err(|_| WeatherApiError::InvalidInput("Invalid latitude format".to_string()))?);
            lng = Some(g.parse().map_err(|_| WeatherApiError::InvalidInput("Invalid longitude format".to_string()))?);
            location_name = format!("{:.4}, {:.4}", lat.unwrap(), lng.unwrap()); // Default name if manually provided
        }
/*
    "city": "Cambridge",
    "region_code": "MA",
    "latitude": 42.3649,
    "longitude": -71.0987,
    "utc_offset": "-0400",
*/
        // If lat/lng are still None, perform IP lookup
        if lat.is_none() || lng.is_none() {
            info!("Latitude or longitude not provided. Attempting IP-based geolocation...");
            let geo_url = "https://ipapi.co/json/";
            let geo_response: GeolocationResponse = client.get(geo_url)
                .send().await?
                .error_for_status()?
                .json().await
                .map_err(|e| WeatherApiError::GeolocationError(format!("Failed to parse geolocation response: {}", e)))?;

            lat = geo_response.latitude;
            lng = geo_response.longitude;
            
            // Populate location_name from geolocation data
            location_name = if let (Some(city), Some(region_code)) = (geo_response.city, geo_response.region_code) {
                format!("{} {}", city, region_code)
            } else {
                "Unknown Location (GeoIP Failed)".to_string()
            };
            info!("Geolocation successful: {}", location_name);
        }

        let final_lat = lat.ok_or(WeatherApiError::GeolocationError("Could not determine latitude".to_string()))?;
        let final_lng = lng.ok_or(WeatherApiError::GeolocationError("Could not determine longitude".to_string()))?;

        info!("Using coordinates: ({}, {}) with units: {}", final_lat, final_lng, units);

        Ok(Weather {
            api_key,
            lat: final_lat,
            lng: final_lng,
            units,
            client,
            weather_data: WeatherData { location_name, ..Default::default() },
            stop_sender: None,
            poll_handle: None,
            last_fetch_time: None,
        })
    }

    /// Fetches current weather and 3-day forecast from Tomorrow.io.
    pub async fn fetch_weather_data(&mut self) -> Result<(), WeatherApiError> {
        info!("Fetching weather data for {}...", self.weather_data.location_name);

        let fields: Vec<&'static str> = vec![
            "temperature", "weatherCode", "humidity", "windSpeed", "precipitationType",
            "temperatureMax", "temperatureMin", "sunriseTime", "precipitationProbability",
            "temp","feels_like","baro_pressure","visibility","precipitation_type","precipitation",
            "wind_speed","wind_direction","wind_gust",
            "sunrise","sunset","weather_code"];

        let timesteps = vec!["1min", "1day"]; // 1min for current, 1day for forecast
        let start_time = Utc::now();
        // Request 4 daily intervals: today, tomorrow, day after tomorrow, and the day after that.
        // The first 1day interval from Tomorrow.io often represents the current day's summary
        // if startTime is now().
        let end_time = start_time + ChronoDuration::days(4); // Request 4 days to get 3 full forecast days (excluding current)

        let url = format!(
            "https://api.tomorrow.io/v4/timelines?location={},{}&fields={}&units={}&timesteps={}&startTime={}&endTime={}&apikey={}",
            self.lat,
            self.lng,
            fields.join(","),
            self.units,
            timesteps.join(","),
            start_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            end_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            self.api_key
        );

        debug!("Tomorrow.io API URL: {}", url);

        let response: TomorrowIOResponse = self.client.get(&url)
            .send().await?
            .error_for_status()? // Check for HTTP status errors (4xx, 5xx)
            .json().await
            .map_err(|e| WeatherApiError::DeserializationError(e))?;

        if let Some(api_error) = response.error {
            return Err(WeatherApiError::ApiError(format!("{:?}", api_error)));
        }

        let mut current_weather: Option<CurrentWeatherFields> = None;
        let mut daily_forecast: Vec<DailyForecastFields> = Vec::new();

        for interval in response.data.intervals {
            // Check if interval is for current (1min timestep) or daily (1day timestep)
            if interval.values.get("humidity").is_some() {
                // This is a 1min interval (current conditions)
                current_weather = Some(serde_json::from_value(interval.values)
                    .map_err(|e| WeatherApiError::DeserializationError(format!("Failed to parse current weather fields: {}", e)))?);
            } else if interval.values.get("temperatureMax").is_some() {
                // This is a 1day interval (forecast)
                // Filter out past intervals or ensure only future daily forecasts are taken
                // Assuming the API returns daily intervals in chronological order starting from today.
                // We typically want "today's forecast" (which starts now), and then the next 2-3 days.
                daily_forecast.push(serde_json::from_value(interval.values)
                    .map_err(|e| WeatherApiError::DeserializationError(format!("Failed to parse daily forecast fields: {}", e)))?);
            }
        }
        
        // Tomorrow.io's "1day" intervals can include a partial current day.
        // We typically want the *next* 3 full days for forecast.
        // If the first daily forecast entry matches today's date (or has already passed significantly),
        // we might want to skip it for "3-day forecast" display and take the next three.
        let now_local_date = chrono::Local::now().date_naive();
        if daily_forecast.len() > 0 {
            // Filter out any daily forecast that is for a date strictly before today (unlikely but safe)
            // Or if it's the first daily forecast and its start_time is significantly in the past
            // of the current day, it's likely "today's summary" which we might skip if we want "tomorrow, +2, +3"
            // For now, let's just ensure we have at least 3 forecast entries by taking a slice.
            // A more robust check might compare `interval.startTime` with `start_time` for 1day intervals.
            daily_forecast.retain(|f| f.sunrise_time.map_or(true, |st| st.date_naive() >= now_local_date));
        }

        self.weather_data.current = current_weather;
        // Take up to 3 daily forecasts (e.g., from tomorrow onwards)
        // If the first daily_forecast is 'today', we might want to skip it for '3-day forecast'.
        // For simplicity, let's just take the first 3 valid daily forecasts for now.
        // If tomorrow.io includes current day in '1day' intervals, we'd need to adjust `skip(1)`.
        // Let's assume the first entry in daily_forecast is *today's* summary and we want *tomorrow* and next two.
        self.weather_data.forecast = daily_forecast.into_iter().take(3).collect();
        self.weather_data.last_updated = Some(Utc::now());
        // location_name already set during new() based on IP or provided lat/lng.

        info!("Weather data fetched successfully.");
        debug!("Current: {:?}, Forecast: {:?}", self.weather_data.current, self.weather_data.forecast);
        self.last_fetch_time = Some(Instant::now()); // Record fetch time
        Ok(())
    }

    /// Starts a background polling task to fetch weather data periodically.
    pub async fn start_polling(instance: Arc<Mutex<Self>>) -> Result<(), WeatherApiError> {
        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut locked_instance = instance.lock().await;
            if locked_instance.poll_handle.is_some() {
                return Err(WeatherApiError::PollingError("Polling already running".to_string()));
            }
            locked_instance.stop_sender = Some(tx);
        }

        let poll_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(35 * 60)) => { // Poll every 35 minutes
                        let mut locked_self = instance.lock().await;
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
        info!("Weather polling started.");
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


async fn translate_str(lang: &str, input: &str) -> Result<String> {
    let url = Url::parse_with_params(
        "https://translate.googleapis.com/translate_a/single?client=gtx&ie=UTF-8&oe=UTF-8&dt=t&sl=en_US",
        &[("tl", lang), ("q", input)],
    )?;

    let res = reqwest::get(url)
        .await?
        .json::<Vec<Value>>()
        .await
        .with_context(|| "Translation request failed.")?;

    let output = res.first().map_or_else(String::new, |i| {
        i.as_array()
            .unwrap()
            .iter()
            .map(|s| s[0].as_str().unwrap())
            .collect::<Vec<&str>>()
            .join("")
    });

    Ok(output)
}

pub struct WeatherCode {
	pub description: String,
	pub icon: u8,
}

impl WeatherCode {
	pub fn resolve(weather_code: &str, is_night: bool, t: &Translation) -> Result<Self> {
		let res = match weather_code {
			0 => (&t.clear_sky, if is_night { 19 } else { 1 }),
			1 => (&t.mostly_clear, if is_night { '' } else { '' }),
			2 => (&t.partly_cloudy, if is_night { '' } else { '' }),
			3 => (&t.overcast, ''),
			45 => (&t.fog, if is_night { '' } else { '' }),
			48 => (&t.depositing_rime_fog, ''),
			51 => (&t.light_drizzle, if is_night { '' } else { '' }),
			53 => (&t.moderate_drizzle, if is_night { '' } else { '' }),
			55 => (&t.dense_drizzle, if is_night { '' } else { '' }),
			56 => (&t.light_freezing_drizzle, if is_night { '' } else { '󰼵' }),
			57 => (&t.dense_freezing_drizzle, if is_night { '' } else { '󰙿' }),
			61 => (&t.slight_rain, if is_night { '' } else { '' }),
			63 => (&t.moderate_rain, if is_night { '' } else { '' }),
			65 => (&t.heavy_rain, if is_night { '' } else { '' }),
			66 => (&t.light_freezing_rain, if is_night { '' } else { '' }),
			67 => (&t.heavy_freezing_rain, if is_night { '' } else { '' }),
			71 => (&t.slight_snow_fall, if is_night { '' } else { '' }),
			73 => (&t.moderate_snow_fall, if is_night { '' } else { '' }),
			75 => (&t.heavy_snow_fall, if is_night { '' } else { '' }),
			77 => (&t.snow_grains, ''),
			80 => (&t.slight_rain_showers, if is_night { '' } else { '' }),
			81 => (&t.moderate_rain_showers, if is_night { '' } else { '' }),
			82 => (&t.violent_rain_showers, if is_night { '' } else { '' }),
			85 => (&t.slight_snow_showers, if is_night { '' } else { '' }),
			86 => (&t.heavy_snow_showers, if is_night { '' } else { '' }),
			95 => (&t.thunderstorm, if is_night { '' } else { '' }),
			96 => (&t.thunderstorm_slight_hail, if is_night { '' } else { '' }),
			99 => (&t.thunderstorm_heavy_hail, if is_night { '' } else { '' }),
			_ => bail!("Unknown weather code"),
		};

		Ok(Self {
			description: res.0.to_string(),
			icon: res.1,
		})
	}
}

/*

"weatherCode": {
      "0": "Unknown",
      "1000": "Clear, Sunny",
      "1100": "Mostly Clear",
      "1101": "Partly Cloudy",
      "1102": "Mostly Cloudy",
      "1001": "Cloudy",
      "2000": "Fog",
      "2100": "Light Fog",
      "4000": "Drizzle",
      "4001": "Rain",
      "4200": "Light Rain",
      "4201": "Heavy Rain",
      "5000": "Snow",
      "5001": "Flurries",
      "5100": "Light Snow",
      "5101": "Heavy Snow",
      "6000": "Freezing Drizzle",
      "6001": "Freezing Rain",
      "6200": "Light Freezing Rain",
      "6201": "Heavy Freezing Rain",
      "7000": "Ice Pellets",
      "7101": "Heavy Ice Pellets",
      "7102": "Light Ice Pellets",
      "8000": "Thunderstorm"
    },

    "weatherCodeFullDay": {
      "0": "Unknown",
      "1000": "Clear, Sunny",
      "1100": "Mostly Clear",
      "1101": "Partly Cloudy",
      "1102": "Mostly Cloudy",
      "1001": "Cloudy",
      "1103": "Partly Cloudy and Mostly Clear",
      "2100": "Light Fog",
      "2101": "Mostly Clear and Light Fog",
      "2102": "Partly Cloudy and Light Fog",
      "2103": "Mostly Cloudy and Light Fog",
      "2106": "Mostly Clear and Fog",
      "2107": "Partly Cloudy and Fog",
      "2108": "Mostly Cloudy and Fog",
      "2000": "Fog",
      "4204": "Partly Cloudy and Drizzle",
      "4203": "Mostly Clear and Drizzle",
      "4205": "Mostly Cloudy and Drizzle",
      "4000": "Drizzle",
      "4200": "Light Rain",
      "4213": "Mostly Clear and Light Rain",
      "4214": "Partly Cloudy and Light Rain",
      "4215": "Mostly Cloudy and Light Rain",
      "4209": "Mostly Clear and Rain",
      "4208": "Partly Cloudy and Rain",
      "4210": "Mostly Cloudy and Rain",
      "4001": "Rain",
      "4211": "Mostly Clear and Heavy Rain",
      "4202": "Partly Cloudy and Heavy Rain",
      "4212": "Mostly Cloudy and Heavy Rain",
      "4201": "Heavy Rain",
      "5115": "Mostly Clear and Flurries",
      "5116": "Partly Cloudy and Flurries",
      "5117": "Mostly Cloudy and Flurries",
      "5001": "Flurries",
      "5100": "Light Snow",
      "5102": "Mostly Clear and Light Snow",
      "5103": "Partly Cloudy and Light Snow",
      "5104": "Mostly Cloudy and Light Snow",
      "5122": "Drizzle and Light Snow",
      "5105": "Mostly Clear and Snow",
      "5106": "Partly Cloudy and Snow",
      "5107": "Mostly Cloudy and Snow",
      "5000": "Snow",
      "5101": "Heavy Snow",
      "5119": "Mostly Clear and Heavy Snow",
      "5120": "Partly Cloudy and Heavy Snow",
      "5121": "Mostly Cloudy and Heavy Snow",
      "5110": "Drizzle and Snow",
      "5108": "Rain and Snow",
      "5114": "Snow and Freezing Rain",
      "5112": "Snow and Ice Pellets",
      "6000": "Freezing Drizzle",
      "6003": "Mostly Clear and Freezing drizzle",
      "6002": "Partly Cloudy and Freezing drizzle",
      "6004": "Mostly Cloudy and Freezing drizzle",
      "6204": "Drizzle and Freezing Drizzle",
      "6206": "Light Rain and Freezing Drizzle",
      "6205": "Mostly Clear and Light Freezing Rain",
      "6203": "Partly Cloudy and Light Freezing Rain",
      "6209": "Mostly Cloudy and Light Freezing Rain",
      "6200": "Light Freezing Rain",
      "6213": "Mostly Clear and Freezing Rain",
      "6214": "Partly Cloudy and Freezing Rain",
      "6215": "Mostly Cloudy and Freezing Rain",
      "6001": "Freezing Rain",
      "6212": "Drizzle and Freezing Rain",
      "6220": "Light Rain and Freezing Rain",
      "6222": "Rain and Freezing Rain",
      "6207": "Mostly Clear and Heavy Freezing Rain",
      "6202": "Partly Cloudy and Heavy Freezing Rain",
      "6208": "Mostly Cloudy and Heavy Freezing Rain",
      "6201": "Heavy Freezing Rain",
      "7110": "Mostly Clear and Light Ice Pellets",
      "7111": "Partly Cloudy and Light Ice Pellets",
      "7112": "Mostly Cloudy and Light Ice Pellets",
      "7102": "Light Ice Pellets",
      "7108": "Mostly Clear and Ice Pellets",
      "7107": "Partly Cloudy and Ice Pellets",
      "7109": "Mostly Cloudy and Ice Pellets",
      "7000": "Ice Pellets",
      "7105": "Drizzle and Ice Pellets",
      "7106": "Freezing Rain and Ice Pellets",
      "7115": "Light Rain and Ice Pellets",
      "7117": "Rain and Ice Pellets",
      "7103": "Freezing Rain and Heavy Ice Pellets",
      "7113": "Mostly Clear and Heavy Ice Pellets",
      "7114": "Partly Cloudy and Heavy Ice Pellets",
      "7116": "Mostly Cloudy and Heavy Ice Pellets",
      "7101": "Heavy Ice Pellets",
      "8001": "Mostly Clear and Thunderstorm",
      "8003": "Partly Cloudy and Thunderstorm",
      "8002": "Mostly Cloudy and Thunderstorm",
      "8000": "Thunderstorm"
    },

    "weatherCodeDay":{
      "0": "Unknown",
      "10000": "Clear, Sunny",
      "11000": "Mostly Clear",
      "11010": "Partly Cloudy",
      "11020": "Mostly Cloudy",
      "10010": "Cloudy",
      "11030": "Partly Cloudy and Mostly Clear",
      "21000": "Light Fog",
      "21010": "Mostly Clear and Light Fog",
      "21020": "Partly Cloudy and Light Fog",
      "21030": "Mostly Cloudy and Light Fog",
      "21060": "Mostly Clear and Fog",
      "21070": "Partly Cloudy and Fog",
      "21080": "Mostly Cloudy and Fog",
      "20000": "Fog",
      "42040": "Partly Cloudy and Drizzle",
      "42030": "Mostly Clear and Drizzle",
      "42050": "Mostly Cloudy and Drizzle",
      "40000": "Drizzle",
      "42000": "Light Rain",
      "42130": "Mostly Clear and Light Rain",
      "42140": "Partly Cloudy and Light Rain",
      "42150": "Mostly Cloudy and Light Rain",
      "42090": "Mostly Clear and Rain",
      "42080": "Partly Cloudy and Rain",
      "42100": "Mostly Cloudy and Rain",
      "40010": "Rain",
      "42110": "Mostly Clear and Heavy Rain",
      "42020": "Partly Cloudy and Heavy Rain",
      "42120": "Mostly Cloudy and Heavy Rain",
      "42010": "Heavy Rain",
      "51150": "Mostly Clear and Flurries",
      "51160": "Partly Cloudy and Flurries",
      "51170": "Mostly Cloudy and Flurries",
      "50010": "Flurries",
      "51000": "Light Snow",
      "51020": "Mostly Clear and Light Snow",
      "51030": "Partly Cloudy and Light Snow",
      "51040": "Mostly Cloudy and Light Snow",
      "51220": "Drizzle and Light Snow",
      "51050": "Mostly Clear and Snow",
      "51060": "Partly Cloudy and Snow",
      "51070": "Mostly Cloudy and Snow",
      "50000": "Snow",
      "51010": "Heavy Snow",
      "51190": "Mostly Clear and Heavy Snow",
      "51200": "Partly Cloudy and Heavy Snow",
      "51210": "Mostly Cloudy and Heavy Snow",
      "51100": "Drizzle and Snow",
      "51080": "Rain and Snow",
      "51140": "Snow and Freezing Rain",
      "51120": "Snow and Ice Pellets",
      "60000": "Freezing Drizzle",
      "60030": "Mostly Clear and Freezing drizzle",
      "60020": "Partly Cloudy and Freezing drizzle",
      "60040": "Mostly Cloudy and Freezing drizzle",
      "62040": "Drizzle and Freezing Drizzle",
      "62060": "Light Rain and Freezing Drizzle",
      "62050": "Mostly Clear and Light Freezing Rain",
      "62030": "Partly Cloudy and Light Freezing Rain",
      "62090": "Mostly Cloudy and Light Freezing Rain",
      "62000": "Light Freezing Rain",
      "62130": "Mostly Clear and Freezing Rain",
      "62140": "Partly Cloudy and Freezing Rain",
      "62150": "Mostly Cloudy and Freezing Rain",
      "60010": "Freezing Rain",
      "62120": "Drizzle and Freezing Rain",
      "62200": "Light Rain and Freezing Rain",
      "62220": "Rain and Freezing Rain",
      "62070": "Mostly Clear and Heavy Freezing Rain",
      "62020": "Partly Cloudy and Heavy Freezing Rain",
      "62080": "Mostly Cloudy and Heavy Freezing Rain",
      "62010": "Heavy Freezing Rain",
      "71100": "Mostly Clear and Light Ice Pellets",
      "71110": "Partly Cloudy and Light Ice Pellets",
      "71120": "Mostly Cloudy and Light Ice Pellets",
      "71020": "Light Ice Pellets",
      "71080": "Mostly Clear and Ice Pellets",
      "71070": "Partly Cloudy and Ice Pellets",
      "71090": "Mostly Cloudy and Ice Pellets",
      "70000": "Ice Pellets",
      "71050": "Drizzle and Ice Pellets",
      "71060": "Freezing Rain and Ice Pellets",
      "71150": "Light Rain and Ice Pellets",
      "71170": "Rain and Ice Pellets",
      "71030": "Freezing Rain and Heavy Ice Pellets",
      "71130": "Mostly Clear and Heavy Ice Pellets",
      "71140": "Partly Cloudy and Heavy Ice Pellets",
      "71160": "Mostly Cloudy and Heavy Ice Pellets",
      "71010": "Heavy Ice Pellets",
      "80010": "Mostly Clear and Thunderstorm",
      "80030": "Partly Cloudy and Thunderstorm",
      "80020": "Mostly Cloudy and Thunderstorm",
      "80000": "Thunderstorm"
    },

    "weatherCodeNight": {
      "0": "Unknown",
      "10001": "Clear",
      "11001": "Mostly Clear",
      "11011": "Partly Cloudy",
      "11021": "Mostly Cloudy",
      "10011": "Cloudy",
      "11031": "Partly Cloudy and Mostly Clear",
      "21001": "Light Fog",
      "21011": "Mostly Clear and Light Fog",
      "21021": "Partly Cloudy and Light Fog",
      "21031": "Mostly Cloudy and Light Fog",
      "21061": "Mostly Clear and Fog",
      "21071": "Partly Cloudy and Fog",
      "21081": "Mostly Cloudy and Fog",
      "20001": "Fog",
      "42041": "Partly Cloudy and Drizzle",
      "42031": "Mostly Clear and Drizzle",
      "42051": "Mostly Cloudy and Drizzle",
      "40001": "Drizzle",
      "42001": "Light Rain",
      "42131": "Mostly Clear and Light Rain",
      "42141": "Partly Cloudy and Light Rain",
      "42151": "Mostly Cloudy and Light Rain",
      "42091": "Mostly Clear and Rain",
      "42081": "Partly Cloudy and Rain",
      "42101": "Mostly Cloudy and Rain",
      "40011": "Rain",
      "42111": "Mostly Clear and Heavy Rain",
      "42021": "Partly Cloudy and Heavy Rain",
      "42121": "Mostly Cloudy and Heavy Rain",
      "42011": "Heavy Rain",
      "51151": "Mostly Clear and Flurries",
      "51161": "Partly Cloudy and Flurries",
      "51171": "Mostly Cloudy and Flurries",
      "50011": "Flurries",
      "51001": "Light Snow",
      "51021": "Mostly Clear and Light Snow",
      "51031": "Partly Cloudy and Light Snow",
      "51041": "Mostly Cloudy and Light Snow",
      "51221": "Drizzle and Light Snow",
      "51051": "Mostly Clear and Snow",
      "51061": "Partly Cloudy and Snow",
      "51071": "Mostly Cloudy and Snow",
      "50001": "Snow",
      "51011": "Heavy Snow",
      "51191": "Mostly Clear and Heavy Snow",
      "51201": "Partly Cloudy and Heavy Snow",
      "51211": "Mostly Cloudy and Heavy Snow",
      "51101": "Drizzle and Snow",
      "51081": "Rain and Snow",
      "51141": "Snow and Freezing Rain",
      "51121": "Snow and Ice Pellets",
      "60001": "Freezing Drizzle",
      "60031": "Mostly Clear and Freezing drizzle",
      "60021": "Partly Cloudy and Freezing drizzle",
      "60041": "Mostly Cloudy and Freezing drizzle",
      "62041": "Drizzle and Freezing Drizzle",
      "62061": "Light Rain and Freezing Drizzle",
      "62051": "Mostly Clear and Light Freezing Rain",
      "62031": "Partly cloudy and Light Freezing Rain",
      "62091": "Mostly Cloudy and Light Freezing Rain",
      "62001": "Light Freezing Rain",
      "62131": "Mostly Clear and Freezing Rain",
      "62141": "Partly Cloudy and Freezing Rain",
      "62151": "Mostly Cloudy and Freezing Rain",
      "60011": "Freezing Rain",
      "62121": "Drizzle and Freezing Rain",
      "62201": "Light Rain and Freezing Rain",
      "62221": "Rain and Freezing Rain",
      "62071": "Mostly Clear and Heavy Freezing Rain",
      "62021": "Partly Cloudy and Heavy Freezing Rain",
      "62081": "Mostly Cloudy and Heavy Freezing Rain",
      "62011": "Heavy Freezing Rain",
      "71101": "Mostly Clear and Light Ice Pellets",
      "71111": "Partly Cloudy and Light Ice Pellets",
      "71121": "Mostly Cloudy and Light Ice Pellets",
      "71021": "Light Ice Pellets",
      "71081": "Mostly Clear and Ice Pellets",
      "71071": "Partly Cloudy and Ice Pellets",
      "71091": "Mostly Cloudy and Ice Pellets",
      "70001": "Ice Pellets",
      "71051": "Drizzle and Ice Pellets",
      "71061": "Freezing Rain and Ice Pellets",
      "71151": "Light Rain and Ice Pellets",
      "71171": "Rain and Ice Pellets",
      "71031": "Freezing Rain and Heavy Ice Pellets",
      "71131": "Mostly Clear and Heavy Ice Pellets",
      "71141": "Partly Cloudy and Heavy Ice Pellets",
      "71161": "Mostly Cloudy and Heavy Ice Pellets",
      "71011": "Heavy Ice Pellets",
      "80011": "Mostly Clear and Thunderstorm",
      "80031": "Partly Cloudy and Thunderstorm",
      "80021": "Mostly Cloudy and Thunderstorm",
      "80001": "Thunderstorm"
    

*/