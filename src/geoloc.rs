use serde::{Deserialize};
use std::time::Duration;
use reqwest::{Client, header, Error};

#[derive(Debug, Deserialize)]
pub struct GeoLocation {
    #[allow(dead_code)]
    pub city: String,
    #[allow(dead_code)]
    pub region_code: String,
    #[allow(dead_code)]
    country_code: String,
    #[allow(dead_code)]
    utc_offset: String,
    #[allow(dead_code)]
    pub latitude: f64,
    #[allow(dead_code)]
    pub longitude: f64,
}

pub async fn fetch_location() -> Result<GeoLocation, Error> {
    const VERSION: &'static str = concat!("LyMonS ",env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));
    let mut headers = header::HeaderMap::new();
    headers.insert("User-Agent", header::HeaderValue::from_static(VERSION));
    headers.insert("Accept", header::HeaderValue::from_static("application/json"));
    headers.insert("Connection", header::HeaderValue::from_static("close"));

    let client = Client::builder()
        .connect_timeout(Duration::from_millis(200))
        .default_headers(headers)
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();

    let geo = client
        .get("https://ipapi.co/json/")
        .send()
        .await?
        .error_for_status()? // none 2xx raise
        .json::<GeoLocation>()
        .await?;

    Ok(geo)
}

