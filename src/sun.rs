/*
 *  sun.rs
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

#![allow(dead_code)] // astronomical calculation helpers; some trig fns written for moon calc

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

#[derive(Debug, Clone)]
pub struct MoonTimes {
    pub moonrise_utc: Option<DateTime<Utc>>,
    pub moonset_utc:  Option<DateTime<Utc>>,
}

/// Moon rise/set times for a specific date using a simplified Meeus algorithm.
/// Accuracy is typically within ±10 minutes — adequate for a display.
pub fn moon_times_for_date(lat_deg: f64, lon_deg: f64, date: NaiveDate) -> MoonTimes {
    // Days from J2000.0 (noon UTC 2000-01-01) to noon on the target date.
    let j2000 = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let d = (date - j2000).num_days() as f64;

    // d0: days from J2000.0 to 0h UT on the target date.
    let d0 = d - 0.5;

    // Moon orbital elements (Meeus Chapter 47, simplified).
    let l_prime = norm360(218.3164591 + 13.17639648 * d); // mean longitude
    let m_moon  = norm360(134.9634114 + 13.06499295 * d); // mean anomaly
    let f_moon  = norm360(93.2720993  + 13.22935024 * d); // argument of latitude

    // Geocentric ecliptic coordinates.
    let lambda = norm360(l_prime + 6.289 * sin_deg(m_moon));
    let beta   = 5.128 * sin_deg(f_moon);

    // Mean obliquity of the ecliptic (degrees).
    let eps = 23.4393 - 0.0000003563 * d;

    // Equatorial coordinates.
    let sin_dec = sin_deg(beta) * cos_deg(eps)
        + cos_deg(beta) * sin_deg(eps) * sin_deg(lambda);
    let cos_dec = (1.0 - sin_dec * sin_dec).sqrt();

    let ra_deg = norm360(
        f64::atan2(
            sin_deg(lambda) * cos_deg(eps) - tan_deg(beta) * sin_deg(eps),
            cos_deg(lambda),
        ) * RAD_TO_DEG,
    );
    let ra_h = ra_deg / 15.0;

    // Hour angle at rise/set.  0.7° horizon correction (refraction + semi-diameter).
    let cos_h0 = (cos_deg(90.7) - sin_dec * sin_deg(lat_deg))
        / (cos_dec * cos_deg(lat_deg));

    if cos_h0 > 1.0 || cos_h0 < -1.0 {
        // Moon perpetually above or below horizon on this date.
        return MoonTimes { moonrise_utc: None, moonset_utc: None };
    }

    let h0_h = acos_deg(cos_h0) / 15.0; // hours of half-arc

    // GMST at 0h UT (degrees → hours).
    let gmst_h = norm360(280.46061837 + 360.98564736629 * d0) / 15.0;

    // Local sidereal time at 0h UT at the observer's longitude.
    let lst0_h = (gmst_h + lon_deg / 15.0).rem_euclid(24.0);

    // UT of meridian transit.
    let transit_h = (ra_h - lst0_h).rem_euclid(24.0);

    MoonTimes {
        moonrise_utc: Some(ut_hours_to_utc(date, (transit_h - h0_h).rem_euclid(24.0))),
        moonset_utc:  Some(ut_hours_to_utc(date, (transit_h + h0_h).rem_euclid(24.0))),
    }
}

/// Today's moon rise/set times.
pub fn moon_times_today(lat_deg: f64, lon_deg: f64) -> MoonTimes {
    let today = Utc::now().date_naive();
    moon_times_for_date(lat_deg, lon_deg, today)
}

/// Moon phase index [0..=7] for a given date.
/// 0=New, 1=WaxingCrescent, 2=FirstQuarter, 3=WaxingGibbous,
/// 4=Full, 5=WaningGibbous, 6=ThirdQuarter, 7=WaningCrescent.
pub fn moon_phase_index(date: NaiveDate) -> usize {
    let j2000 = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let d = (date - j2000).num_days() as f64;

    let m_moon = norm360(134.9634114 + 13.06499295 * d);
    let m_sun  = norm360(357.529     + 0.9856003   * d);
    let l_moon = norm360(218.3164591 + 13.17639648 * d);
    let l_sun  = norm360(280.4665    + 0.9856474   * d);

    let i = norm360(l_moon - l_sun + 6.289 * sin_deg(m_moon) - 2.1 * sin_deg(m_sun));
    ((i + 22.5) / 45.0) as usize % 8
}

/// Moon illumination fraction [0.0, 1.0] for a given date.
/// 0.0 = new moon, 1.0 = full moon.
pub fn moon_phase_fraction(date: NaiveDate) -> f64 {
    let j2000 = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let d = (date - j2000).num_days() as f64;

    let m_moon = norm360(134.9634114 + 13.06499295 * d); // moon mean anomaly
    let m_sun  = norm360(357.529     + 0.9856003   * d); // sun  mean anomaly

    // Elongation of the moon from the sun.
    let l_moon = norm360(218.3164591 + 13.17639648 * d);
    let l_sun  = norm360(280.4665    + 0.9856474   * d);
    let i = norm360(l_moon - l_sun + 6.289 * sin_deg(m_moon) - 2.1 * sin_deg(m_sun));

    // Illuminated fraction.
    (1.0 - cos_deg(i)) * 0.5
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
