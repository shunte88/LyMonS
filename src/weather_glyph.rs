/*
 *  weather_glyph.rs
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

#![allow(dead_code)] // weather icon bitmaps and helper fns; some glyphs reserved for future use

 #[allow(dead_code)]
pub enum MoonPhase {
    New = 0,
    WaxingCrescent = 1,
    FirstQuarter = 2,
    WaxingGibbous = 3,
    Full = 4,
    WaningGibbous = 5,
    ThirdQuarter = 6,
    WaningCrescent = 7,
}

/// Weather glyph dimensions (8 glyphs in vertical strip)
pub const THERMO_GLYPH_WIDTH: u32 = 12;
pub const THERMO_GLYPH_HEIGHT: u32 = 12;

/// Load weather glyphs from binary file
/// 'thermo', 12x96px (8 glyphs: 0=temperature, 1=wind, 2=humidity, 3=precipitation, 4=sunrise, 5=sunset, 6=moonset, 7=moonrise)
pub const THERMO_RAW_DATA: &[u8] = include_bytes!("../data/thermo_12x96.bin");

/// Weather glyph indices
pub const GLYPH_TEMPERATURE: usize = 0;
pub const GLYPH_WIND: usize = 1;
pub const GLYPH_HUMIDITY: usize = 2;
pub const GLYPH_PRECIPITATION: usize = 3;
pub const GLYPH_SUNRISE: usize = 4;
pub const GLYPH_SUNSET: usize = 5;
pub const GLYPH_MOONSET: usize = 6;
pub const GLYPH_MOONRISE: usize = 7;

/// Get a slice for a specific weather glyph
///
/// # Arguments
/// * `glyph_index` - 0=temperature, 1=wind, 2=humidity, 3=precipitation, 4=sunrise, 5=sunset, 6=moonset, 7=moonrise
///
/// # Returns
/// Byte slice containing the 12x12 monochrome bitmap for the requested glyph
pub fn get_weather_glyph_slice(glyph_index: usize) -> &'static [u8] {
    crate::glyphs::get_glyph_slice(
        THERMO_RAW_DATA,
        glyph_index,
        THERMO_GLYPH_WIDTH,
        THERMO_GLYPH_HEIGHT
    )
}

/// Moon phase glyph dimensions
pub const MOON_PHASE_WIDTH: u32 = 30;
pub const MOON_PHASE_HEIGHT: u32 = 30;

/// Load moon phase glyphs from binary file
/// 8 phases as described by the MoonPhase enum, 30x30 pixels each
pub const MOON_PHASE_RAW_DATA: &[u8] = include_bytes!("../data/moonphase_30x30.bin");

/// Get a slice for a specific moon phase glyph
///
/// # Arguments
/// * `phase` - Moon phase enum value (0-7)
///
/// # Returns
/// Byte slice containing the 30x30 monochrome bitmap for the requested phase
pub fn get_moon_phase_slice(phase: MoonPhase) -> &'static [u8] {
    crate::glyphs::get_glyph_slice(
        MOON_PHASE_RAW_DATA,
        phase as usize,
        MOON_PHASE_WIDTH,
        MOON_PHASE_HEIGHT
    )
}

/// Get a description of the moon phase
///
/// # Arguments
/// * `phase` - Moon phase enum value (0-7)
///
/// # Returns
/// string description of the requested phase
/// Map a `*_glyph` field name to its 1bpp bitmap index in `THERMO_RAW_DATA`.
/// Returns `usize::MAX` for unrecognised names (caller should skip).
pub fn glyph_index_for_field(name: &str) -> usize {
    match name {
        "temp_glyph"     => GLYPH_TEMPERATURE,
        "humidity_glyph" => GLYPH_HUMIDITY,
        "wind_glyph"     => GLYPH_WIND,
        "precip_glyph"   => GLYPH_PRECIPITATION,
        "sunrise_glyph"  => GLYPH_SUNRISE,
        "sunset_glyph"   => GLYPH_SUNSET,
        "moonset_glyph"  => GLYPH_MOONSET,
        "moonrise_glyph" => GLYPH_MOONRISE,
        _                => usize::MAX,
    }
}

/// SVG glyph data loaded from `data/weather_glyphs.zip` at startup.
///
/// Each field holds the raw SVG bytes for one glyph.  Missing entries (file
/// not found in the zip) are represented as empty `Vec`s; the renderer falls
/// back to the 1bpp bitmap in that case.
#[derive(Default)]
pub struct WeatherGlyphSet {
    pub temperature:   Vec<u8>,
    pub humidity:      Vec<u8>,
    pub wind:          Vec<u8>,
    pub precipitation: Vec<u8>,
    pub sunrise:       Vec<u8>,
    pub sunset:        Vec<u8>,
    pub moonrise:      Vec<u8>,
    pub moonset:       Vec<u8>,
    pub pressure:      Vec<u8>,
}

impl WeatherGlyphSet {
    /// Load all matching entries from a zip archive.
    ///
    /// Entries are matched by filename keywords (e.g. `weather_temperature.svg`).
    /// Returns `None` only if the zip cannot be opened at all; a partially-populated
    /// set is returned when individual entries are missing.
    pub fn load_from_zip(zip_path: &str) -> Option<Self> {
        use std::io::Read;
        let file = std::fs::File::open(zip_path)
            .map_err(|e| log::warn!("weather_glyphs: cannot open {}: {}", zip_path, e))
            .ok()?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| log::warn!("weather_glyphs: cannot read zip {}: {}", zip_path, e))
            .ok()?;

        let mut set = WeatherGlyphSet::default();

        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(e)  => e,
                Err(_) => continue,
            };
            let name = entry.name().to_lowercase();

            let dest: &mut Vec<u8> =
                if      name.contains("temperature")   { &mut set.temperature   }
                else if name.contains("humidity")      { &mut set.humidity      }
                else if name.contains("wind")          { &mut set.wind          }
                else if name.contains("precipitation") { &mut set.precipitation }
                else if name.contains("sunrise")       { &mut set.sunrise       }
                else if name.contains("sunset")        { &mut set.sunset        }
                else if name.contains("moonrise")      { &mut set.moonrise      }
                else if name.contains("moonset")       { &mut set.moonset       }
                else if name.contains("pressure")      { &mut set.pressure      }
                else { continue };

            if entry.read_to_end(dest).is_err() {
                log::warn!("weather_glyphs: failed to read {} from {}", entry.name(), zip_path);
                dest.clear();
            }
        }

        log::info!(
            "weather_glyphs: loaded from {} (temp={}, humidity={}, wind={}, precip={}, \
             sunrise={}, sunset={}, moonrise={}, moonset={})",
            zip_path,
            !set.temperature.is_empty(), !set.humidity.is_empty(),
            !set.wind.is_empty(), !set.precipitation.is_empty(),
            !set.sunrise.is_empty(), !set.sunset.is_empty(),
            !set.moonrise.is_empty(), !set.moonset.is_empty(),
        );

        Some(set)
    }

    /// Look up SVG bytes by the YAML field name (`"temp_glyph"` etc.).
    /// Returns `None` for unknown names or glyphs that failed to load.
    pub fn get(&self, field_name: &str) -> Option<&[u8]> {
        let v = match field_name {
            "temp_glyph"     => &self.temperature,
            "humidity_glyph" => &self.humidity,
            "wind_glyph"     => &self.wind,
            "precip_glyph"   => &self.precipitation,
            "sunrise_glyph"  => &self.sunrise,
            "sunset_glyph"   => &self.sunset,
            "moonrise_glyph" => &self.moonrise,
            "moonset_glyph"  => &self.moonset,
            "pressure_glyph" => &self.pressure,
            _                => return None,
        };
        if v.is_empty() { None } else { Some(v.as_slice()) }
    }
}

/// SVG moon phase glyphs loaded from `data/moonphase.zip` at startup.
///
/// Indexed 0–7 matching the `MoonPhase` enum (New=0 … WaningCrescent=7).
/// Empty `Vec` means that phase file was not found in the zip; the renderer
/// falls back to the 1bpp bitmap in that case.
pub struct MoonPhaseGlyphSet {
    phases: [Vec<u8>; 8],
}

impl MoonPhaseGlyphSet {
    /// Load all 8 phase SVGs from a zip archive.
    ///
    /// Files are matched by unique keyword in the filename:
    ///   `_new`, `waxing_crescent`, `first_quarter`, `waxing_gibbous`,
    ///   `_full`, `waning_gibbous`, `third_quarter`, `waning_crescent`.
    /// Returns `None` only if the zip cannot be opened.
    pub fn load_from_zip(zip_path: &str) -> Option<Self> {
        use std::io::Read;
        let file = std::fs::File::open(zip_path)
            .map_err(|e| log::warn!("moonphase: cannot open {}: {}", zip_path, e))
            .ok()?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| log::warn!("moonphase: cannot read zip {}: {}", zip_path, e))
            .ok()?;

        let mut phases: [Vec<u8>; 8] = Default::default();

        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(e)  => e,
                Err(_) => continue,
            };
            let name = entry.name().to_lowercase();

            let idx: usize =
                if      name.contains("waxing_crescent") { 1 }
                else if name.contains("first_quarter")   { 2 }
                else if name.contains("waxing_gibbous")  { 3 }
                else if name.contains("waning_gibbous")  { 5 }
                else if name.contains("third_quarter")||name.contains("final_quarter")   { 6 }
                else if name.contains("waning_crescent") { 7 }
                else if name.contains("_full")           { 4 }
                else if name.contains("_new")            { 0 }
                else { continue };

            let mut raw = Vec::new();
            if entry.read_to_end(&mut raw).is_err() {
                log::warn!("moonphase: failed to read {} from {}", entry.name(), zip_path);
                continue;
            }
            phases[idx] = raw;
        }

        let loaded = phases.iter().filter(|v| !v.is_empty()).count();
        log::info!("moonphase: loaded {}/8 phases from {}", loaded, zip_path);

        Some(Self { phases })
    }

    /// Return SVG bytes for `phase_index` (0–7), or `None` if not loaded.
    pub fn get(&self, phase_index: usize) -> Option<&[u8]> {
        self.phases.get(phase_index)
            .filter(|v| !v.is_empty())
            .map(|v| v.as_slice())
    }
}

// moon phase strings - these need to be translatable
pub fn get_moon_phase_description(phase: MoonPhase) -> &'static str {
    match phase {
        MoonPhase::New => "New Moon",
        MoonPhase::WaxingCrescent => "Waxing Crescent",
        MoonPhase::FirstQuarter => "First Quarter",
        MoonPhase::WaxingGibbous => "Waxing Gibbous",
        MoonPhase::Full => "Full Moon",
        MoonPhase::WaningGibbous => "Waning Gibbous",
        MoonPhase::ThirdQuarter => "Third Quarter",
        MoonPhase::WaningCrescent => "Waning Crescent",
    }
}