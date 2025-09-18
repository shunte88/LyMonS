/*
 *  dbfs.rs
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
pub const VU_0VU_DBFS: f32 = -8.0; // bumped from 18 to 8

/// Convert a channel’s dBFS to “VU dB” (where 0.0 is the 0 VU mark on the scale).
/// 0 VU calibration in dBFS (EBU = -18, SMPTE = -20). Make this a config knob.
#[allow(dead_code)]
#[inline]
pub fn dbfs_to_vudb(dbfs: f32) -> f32 {
    dbfs - VU_0VU_DBFS
}
