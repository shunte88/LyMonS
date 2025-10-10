/*
 *  glyphs.rs
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
/// Helper function to get a slice for a specific glyph from a binary set
pub fn get_glyph_slice(raw_glyph: &'static [u8], index: usize, w: u32, h: u32) -> &'static [u8] {
    let byte_size = ((w as usize + 7) / 8) * h as usize;
    let start_idx = index * byte_size;
    let end_idx = start_idx + byte_size;
    &raw_glyph[start_idx..end_idx]
}

pub const GLYPH_VOLUME_OFF: [u8; 8] = [0x10, 0x30, 0xe5, 0xe2, 0xe2, 0xe5, 0x30, 0x10,];
pub const GLYPH_VOLUME_ON: [u8; 8] = [0x10, 0x31, 0xe5, 0xe5, 0xe5, 0xe5, 0x31, 0x10,];

pub const GLYPH_NONE: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,];
pub const GLYPH_REPEAT_ONE: [u8; 8] = [0x02, 0x3f, 0x42, 0x58, 0x1a, 0x42, 0xfc, 0x40,];
pub const GLYPH_REPEAT_ALL: [u8; 8] = [0x02, 0x7f, 0x82, 0x80, 0x82, 0x82, 0x7c, 0x00,];
pub const GLYPH_SHUFFLE_TRACKS: [u8; 8] = [0x7e, 0x00, 0xfc, 0x00, 0x7e, 0x00, 0xfc, 0x00,];
pub const GLYPH_SHUFFLE_ALBUMS: [u8; 8] = [0xfc, 0x00, 0x7e, 0x00, 0xfe, 0x12, 0x72, 0x1e,];

pub const GLYPH_AUDIO_HD: [u8; 8] = [0x00, 0x66, 0x66, 0x7e, 0x7e, 0x66, 0x66, 0x00,];
pub const GLYPH_AUDIO_SD: [u8; 8] = [0x00, 0x3c, 0x66, 0x60, 0x1c, 0x46, 0x66, 0x3c,];
pub const GLYPH_AUDIO_DSD: [u8; 8] = [0x00, 0x78, 0x6c, 0x66, 0x66, 0x6c, 0x78, 0x00,];
