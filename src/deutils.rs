/*
 *  deutils.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-25 Stuart Hunter
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
// src/deutils.rs
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::convert::TryInto;
use chrono::{Utc, TimeZone};
//use chrono::DateTime;

pub fn default_zero_i16() -> i16 { 0 }
pub fn default_false() -> bool { false }

// Note: value_to_i16 and flatten_json are now in sliminfo.rs if they are only used there.
// If they are truly general utilities, they could stay in deutils or be in their own `utils.rs` module.
// For now, assuming they are LMS-specific helpers and moving them with LMSServer.
// If you need them outside of LMSServer in main.rs, you'd re-add them here or import from deutils.

pub fn deserialize_bool_from_anything<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    let s = v.to_string().trim_matches('"').trim().to_lowercase();
    match s.as_str() {
        "1" | "true" | "yes" | "y" | "t" => Ok(true),
        "0" | "false" | "no"  | "n" | "f" => Ok(false),
        _ => Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Str(&s.as_str()),
            &"expected boolean representation",
        )),
    }
}

pub fn deserialize_numeric_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let v = Value::deserialize(deserializer)?;
    // debug!("in i32 {:#?}",v); // Remove or adjust log based on deutils context
    let n = v
        .as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| D::Error::custom("non-integer"))?
        .try_into()
        .map_err(|_| D::Error::custom("overflow"))?;
    Ok(n)
}

pub fn deserialize_numeric_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let v = Value::deserialize(deserializer)?;
    // debug!("in i32 {:#?}",v); // Remove or adjust log based on deutils context
    let n = v
        .as_f64()
        .or_else(|| v.as_str().and_then(|s| s.replace('"', "").parse().ok()))
        .ok_or_else(|| D::Error::custom("non-floating-point"))?
        .try_into()
        .map_err(|_| D::Error::custom("overflow?"))?;
    Ok(n)
}

pub fn deserialize_numeric_i16<'de, D>(deserializer: D) -> Result<i16, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let v = Value::deserialize(deserializer)?;
    // debug!("in i16 {:#?}",v); // Remove or adjust log based on deutils context
    let n = v
        .as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| D::Error::custom("non-integer"))?
        .try_into()
        .map_err(|_| D::Error::custom("overflow"))?;
    Ok(n)
}

pub fn deserialize_numeric_u8<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let v = Value::deserialize(deserializer)?;
    // debug!("in u8 {:#?}",v); // Remove or adjust log based on deutils context
    let n = v
        .as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| D::Error::custom("non-integer"))?
        .try_into()
        .map_err(|_| D::Error::custom("overflow"))?;
    Ok(n)
}

/// Deserializes a float epoch timestamp into a "YYYY-MM-DD" date string.
pub fn deserialize_epoch_to_date_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let float_epoch = f64::deserialize(deserializer)?;
    
    let secs = float_epoch.trunc() as i64;
    let nanos = (float_epoch.fract() * 1_000_000_000.0) as u32;

    // Use Utc.timestamp_opt for safe conversion
    Utc.timestamp_opt(secs, nanos)
        .earliest() // Get the earliest valid DateTime if multiple exist (due to DST transitions)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .ok_or_else(|| D::Error::custom(format!("invalid epoch timestamp: {}", float_epoch)))
}

/// Converts total seconds (f32) into a "HH:MM:SS" or "MM:SS" duration string.
/// If hours is zero, only MM:SS is surfaced.
///
/// # Arguments
/// * `total_seconds` - The duration in seconds as a float.
///
/// # Returns
/// A `String` representing the formatted duration.
pub fn seconds_to_hms(total_seconds: f32) -> String {
    let total_seconds_u32 = total_seconds as u32;
    let hours = total_seconds_u32 / 3600;
    let minutes = (total_seconds_u32 % 3600) / 60;
    let seconds = total_seconds_u32 % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

/// Deserializes a float (seconds) into a "HH:MM:SS" or "MM:SS" duration string using `seconds_to_hms`.
pub fn deserialize_seconds_to_hms<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let total_seconds = f32::deserialize(deserializer)?;
    Ok(seconds_to_hms(total_seconds))
}

#[allow(dead_code)]
/// Deserializes aand transpose weather units
pub fn deserialize_weather_uom<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let mut units = String::deserialize(deserializer)?;
    // yes we could just sl;ice off /hrs but they may be more to add here
    if units == "in/hr" {
        units = "in".to_string();      
    } else if units == "cm/hr" {
        units = "cm".to_string();
    }   
    Ok(units)

}

#[allow(dead_code)]
/// Deserializes and transpose compass direction
pub fn deserialize_compass_direction<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let deg = f32::deserialize(deserializer)?;
    let mut d16 = ((deg / 22.5) + 0.5) as u8;
    d16 %= 16;
    let compass_points = [
        "N",  "NNE", "NE", "ENE", "E",  "ESE",
        "SE", "SSE", "S",  "SSW", "SW", "WSW",
        "W",  "WNW", "NW", "NNW"];
    Ok(compass_points[d16 as usize].to_string())
}
