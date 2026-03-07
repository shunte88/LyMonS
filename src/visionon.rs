/*
 *  visionon.rs
 *
 *  LyMonS - worth the squeeze
 *	(c) 2020-26 Stuart Hunter
 *
 *	Parse visionon SSE JSON events and map them to VizPayload values that the
 *	existing visualizer rendering pipeline can consume.
 *
 *	visionon JSON (alternating VU / SA events, ~2 ms apart):
 *	  {"type":"VU","channel":[
 *	    {"name":"L","accumulated":<i64>,"scaled":<i32>,
 *	     "dBfs":<i64>,"dB":<i64>,"linear":<i64>,"FFT":[…],"numFFT":<i32>},
 *	    {"name":"R",…}]}
 *
 *	dBfs field:  integer dBFS value, range -96..0; -1000 = silence sentinel.
 *	FFT field:   raw FFT power per frequency sub-band (SA events only).
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

#![allow(dead_code)] // SSE VU/SA JSON parser; wired to hardware path pending visualizer type fix

use serde::Deserialize;

use crate::dbfs;
use crate::spectrum::SPECTRUM_BANDS_COUNT;
use crate::visualization::Visualization;
use crate::visualizer::VizPayload;

// Histogram dB range — mirrors spectrum.rs constants.
const HIST_FLOOR_DB: f32 = -80.0;
const HIST_CEIL_DB: f32 = -12.0;
const PEAK_METER_LEVELS_MAX: u8 = 48;

// ─── JSON structures ─────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct VisionOnFrame {
    #[serde(rename = "type")]
    pub event_type: String,
    pub channel: Vec<ChannelData>,
}

#[allow(non_snake_case)]
#[derive(Deserialize, Debug)]
pub struct ChannelData {
    pub name: String,
    /// Integer dBFS, -96..0.  -1000 = silence sentinel.
    pub dBfs: i64,
    /// FFT sub-band power values (SA events).
    pub FFT: Vec<i32>,
    /// Number of valid entries in FFT[].
    pub numFFT: i32,
}

// ─── Conversion helpers ───────────────────────────────────────────────────────

/// Convert a visionon integer dBFS to the VU display dB expected by rendering.
#[inline]
fn sse_dbfs_to_vudb(dbfs_i: i64) -> f32 {
    // -1000 is the "silence" sentinel; anything ≤ floor also maps to floor.
    let db = if dbfs_i <= -96 { -96.0_f32 } else { dbfs_i as f32 };
    dbfs::dbfs_to_vudb(db)
}

/// Simple downmix: average of two VU-adjusted channel levels.
#[inline]
fn downmix(l: f32, r: f32) -> f32 {
    (l + r) * 0.5
}

/// Reduce raw FFT bins to `SPECTRUM_BANDS_COUNT` histogram bars scaled 0..=48.
///
/// Groups consecutive bins linearly, takes the max per group, then maps to dB
/// relative to 16-bit full-scale (32768) and converts to a display level.
fn fft_bins_to_levels(bins: &[i32]) -> Vec<u8> {
    let n = SPECTRUM_BANDS_COUNT as usize;
    if bins.is_empty() {
        return vec![0; n];
    }
    let len = bins.len();

    (0..n)
        .map(|i| {
            let start = (i * len) / n;
            let end = (((i + 1) * len) / n).min(len);
            let max_bin = bins[start..end].iter().copied().max().unwrap_or(0);
            if max_bin <= 0 {
                return 0;
            }
            // Convert raw FFT bin to dBFS relative to 16-bit full-scale.
            let db = (20.0 * (max_bin as f32 / 32768.0_f32).log10()).max(HIST_FLOOR_DB);
            let x = ((db - HIST_FLOOR_DB) / (HIST_CEIL_DB - HIST_FLOOR_DB)).clamp(0.0, 1.0);
            (x * PEAK_METER_LEVELS_MAX as f32).round() as u8
        })
        .collect()
}

/// Clamp FFT slice to the number of valid entries reported by `numFFT`.
#[inline]
fn valid_fft(ch: &ChannelData) -> &[i32] {
    let n = (ch.numFFT.max(0) as usize).min(ch.FFT.len());
    &ch.FFT[..n]
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Parse a raw visionon JSON string and produce a `VizPayload` for the given
/// visualization `kind`.  Returns `None` when the event type does not match
/// `kind` (VU events drive VU/Peak payloads; SA events drive Hist payloads).
pub fn parse_event(json: &str, kind: Visualization) -> Option<VizPayload> {
    let frame: VisionOnFrame = serde_json::from_str(json).ok()?;

    let is_vu = frame.event_type == "VU";
    let is_sa = frame.event_type == "SA";

    // visionon always sends L before R.
    let ch_l = frame.channel.iter().find(|c| c.name == "L")?;
    let ch_r = frame.channel.iter().find(|c| c.name == "R")?;

    let l_db = sse_dbfs_to_vudb(ch_l.dBfs);
    let r_db = sse_dbfs_to_vudb(ch_r.dBfs);
    let m_db = downmix(l_db, r_db);

    match kind {
        Visualization::VuStereo if is_vu =>
            Some(VizPayload::VuStereo { l_db, r_db }),

        Visualization::VuMono if is_vu =>
            Some(VizPayload::VuMono { m_db }),

        Visualization::VuAio if is_vu =>
            Some(VizPayload::VuAio { m_db, l_db, r_db }),

        Visualization::VuStereoWithCenterPeak if is_vu =>
            Some(VizPayload::VuStereoWithCenterPeak { l_db, r_db, m_db, peak_hold: 0 }),

        Visualization::PeakStereo if is_vu =>
            Some(VizPayload::PeakStereo { l_db, r_db, l_hold: 0, r_hold: 0 }),

        Visualization::PeakMono if is_vu =>
            Some(VizPayload::PeakMono { m_db, hold: 0 }),

        Visualization::HistStereo if is_sa => {
            let bands_l = fft_bins_to_levels(valid_fft(ch_l));
            let bands_r = fft_bins_to_levels(valid_fft(ch_r));
            Some(VizPayload::HistStereo { bands_l, bands_r })
        }

        Visualization::HistMono if is_sa => {
            let bl = fft_bins_to_levels(valid_fft(ch_l));
            let br = fft_bins_to_levels(valid_fft(ch_r));
            let bands = bl.iter().zip(br.iter()).map(|(a, b)| (*a).max(*b)).collect();
            Some(VizPayload::HistMono { bands })
        }

        Visualization::HistAio if is_sa => {
            let bands_l = fft_bins_to_levels(valid_fft(ch_l));
            let bands_r = fft_bins_to_levels(valid_fft(ch_r));
            let bands = bands_l.iter().zip(bands_r.iter())
                .map(|(a, b)| (*a).max(*b))
                .collect();
            Some(VizPayload::HistAio { bands, bands_l, bands_r })
        }

        // WaveformSpectrum requires raw PCM — not available via SSE.
        // All other type/event mismatches are silently skipped.
        _ => None,
    }
}
