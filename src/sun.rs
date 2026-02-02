/*
 *  sun.rs
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
//! Sunrise/Sunset for a given lat/lon and date (NOAA algorithm, zenith 90.833°).
//! Returns UTC times; helper provided to shift to a fixed offset.

use chrono::{prelude::*, Duration};

const ZENITH_DEG: f64 = 90.833_f64; // "official" sunrise/sunset (refraction accounted)
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
const RAD_TO_DEG: f64 = 180.0 / std::f64::consts::PI;

#[derive(Debug, Clone)]
pub struct SunTimes {
    pub sunrise_utc: Option<DateTime<Utc>>,
    pub sunset_utc:  Option<DateTime<Utc>>,
}

#[inline]
fn sin_deg(x: f64) -> f64 { (x * DEG_TO_RAD).sin() }
#[inline]
fn cos_deg(x: f64) -> f64 { (x * DEG_TO_RAD).cos() }
#[inline]
fn tan_deg(x: f64) -> f64 { (x * DEG_TO_RAD).tan() }
#[inline]
fn asin_deg(x: f64) -> f64 { (x).asin() * RAD_TO_DEG }
#[inline]
fn acos_deg(x: f64) -> f64 { (x).acos() * RAD_TO_DEG }
#[inline]
fn atan_deg(x: f64) -> f64 { (x).atan() * RAD_TO_DEG }

/// Day-of-year (1..=366)
fn day_of_year(date: NaiveDate) -> u32 {
    date.ordinal()
}

/// Normalize angle to [0,360)
fn norm360(x: f64) -> f64 {
    let mut a = x % 360.0;
    if a < 0.0 { a += 360.0; }
    a
}

/// Compute sunrise/sunset UT in hours for a given day-of-year using NOAA method.
/// Returns (Option<UT_rise_hours>, Option<UT_set_hours>).
fn sunrise_sunset_ut_hours(lat_deg: f64, lon_deg: f64, doy: u32) -> (Option<f64>, Option<f64>) {
    // Longitude hour
    let lng_hour = lon_deg / 15.0;

    // Two passes: sunrise uses 6h local solar, sunset 18h.
    let (ut_rise, ut_set) = (true, false);

    let rise = compute_ut(lat_deg, lng_hour, doy as f64, ut_rise);
    let set  = compute_ut(lat_deg, lng_hour, doy as f64, ut_set);

    (rise, set)
}

fn compute_ut(lat_deg: f64, lng_hour: f64, n: f64, is_rise: bool) -> Option<f64> {
    // Approximate time
    let t = if is_rise {
        n + (6.0 - lng_hour) / 24.0
    } else {
        n + (18.0 - lng_hour) / 24.0
    };

    // Sun's mean anomaly
    let m = 0.9856 * t - 3.289;
    // Sun's true longitude (L), normalized
    let mut l = m + 1.916 * sin_deg(m) + 0.020 * sin_deg(2.0 * m) + 282.634;
    l = norm360(l);
    // Sun's right ascension (RA)
    let mut ra = atan_deg(0.91764 * tan_deg(l));
    ra = norm360(ra);
    // Quadrant adjust RA to be in same quadrant as L
    let l_quadrant  = (l / 90.0).floor() * 90.0;
    let ra_quadrant = (ra / 90.0).floor() * 90.0;
    ra = ra + (l_quadrant - ra_quadrant);
    // RA to hours
    ra /= 15.0;
    // Sun declination
    let sin_dec = 0.39782 * sin_deg(l);
    let cos_dec = (1.0 - sin_dec * sin_dec).sqrt();
    // Sun local hour angle
    let cos_h = (cos_deg(ZENITH_DEG) - sin_dec * sin_deg(lat_deg)) / (cos_dec * cos_deg(lat_deg));
    if cos_h > 1.0 {
        // Sun never rises on this location (on the specified date)
        return None;
    } else if cos_h < -1.0 {
        // Sun never sets on this location (on the specified date)
        return None;
    }

    let h = if is_rise {
        // H_rise = 360 - acos
        360.0 - acos_deg(cos_h)
    } else {
        // H_set = acos
        acos_deg(cos_h)
    };

    // H to hours
    let h = h / 15.0;
    // Local mean time of rising/setting
    let t_local = h + ra - (0.06571 * t) - 6.622;
    // UT
    let mut ut = t_local - lng_hour;
    // normalize into [0,24)
    ut = ((ut % 24.0) + 24.0) % 24.0;
    Some(ut)

}

/// Convert UT hours (0..24) to a UTC DateTime on the given civil date.
/// Handles wrap-around if UT < 0 or >= 24 (shouldn't happen after normalize, but safe).
fn ut_hours_to_utc(date: NaiveDate, ut_hours: f64) -> DateTime<Utc> {
    let hour = ut_hours.floor() as i64;
    let mins_f = (ut_hours - hour as f64) * 60.0;
    let minute = mins_f.floor() as i64;
    let secs_f = (mins_f - minute as f64) * 60.0;
    let second = secs_f.round() as i64;

    // Start of day in UTC
    let base = NaiveDateTime::new(date, NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let dt = base + Duration::hours(hour) + Duration::minutes(minute) + Duration::seconds(second);
    DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
}

/// sunrise/sunset for a specific date (UTC civil date).
pub fn sun_times_for_date(lat_deg: f64, lon_deg: f64, date: NaiveDate) -> SunTimes {
    let doy = day_of_year(date) as u32;
    let (rise_ut_h, set_ut_h) = sunrise_sunset_ut_hours(lat_deg, lon_deg, doy);

    let sunrise_utc = rise_ut_h.map(|h| ut_hours_to_utc(date, h));
    let sunset_utc  = set_ut_h.map(|h| ut_hours_to_utc(date, h));

    SunTimes { sunrise_utc, sunset_utc }
}

/// sunrise/sunset for "today" (UTC civil date).
pub fn sun_times_today(lat_deg: f64, lon_deg: f64) -> SunTimes {
    let today = Utc::now().date_naive();
    sun_times_for_date(lat_deg, lon_deg, today)
}

/// convert UTC time to a fixed-offset local time (e.g., minutes = -240 for EDT).
pub fn to_fixed_offset(
    dt: Option<DateTime<Utc>>,
    offset_minutes: i32,
) -> Option<DateTime<FixedOffset>> {
    dt.map(|t| {
        let offset = FixedOffset::east_opt(offset_minutes * 60).unwrap();
        t.with_timezone(&offset)
    })
}
