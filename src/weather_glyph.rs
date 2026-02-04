/*
 *  weather_glyph.rs
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

/// Weather glyph dimensions (4 glyphs in vertical strip)
pub const THERMO_GLYPH_WIDTH: u32 = 12;
pub const THERMO_GLYPH_HEIGHT: u32 = 12;

/// Load weather glyphs from binary file
/// 'thermo', 12x48px (4 glyphs: 0=temperature, 1=wind, 2=humidity, 3=precipitation)
pub const THERMO_RAW_DATA: &[u8] = include_bytes!("../data/thermo_12x48.bin");

/// Get a slice for a specific weather glyph
///
/// # Arguments
/// * `glyph_index` - 0=temperature, 1=wind, 2=humidity, 3=precipitation
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
const MOON_PHASE_RAW_DATA: &[u8] = include_bytes!("../data/moonphase_30x30.bin");

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
