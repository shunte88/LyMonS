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

