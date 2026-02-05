/*
 *  geoloc.rs
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

