/*
 *  spectrum.rs
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
// For histogram display (different from peak meter mapping)
const HIST_FLOOR_DB: f32 = -80.0;
const HIST_CEIL_DB:  f32 = -12.0;
const EPS: f32 = 1e-12;
const SPECTRUM_MIN_HZ: f32 = 20.0;              // start of spectrum
const FFT_MIN: usize = 128;
const FFT_MAX: usize = 4096;
const PEAK_METER_LEVELS_MAX: u8 = 48;
pub const SPECTRUM_BANDS_COUNT:u8 = 16;         // Number of spectrum bands

#[inline]
fn db_to_hist_level(db: f32) -> u8 {
    let x = ((db - HIST_FLOOR_DB) / (HIST_CEIL_DB - HIST_FLOOR_DB)).clamp(0.0, 1.0);
    (x * (PEAK_METER_LEVELS_MAX as f32)).round() as u8
}

pub struct SpectrumEngine {
    sr: u32,
    nfft: usize,
    fft: std::sync::Arc<dyn rustfft::Fft<f32> + Send + Sync>,
    window: Vec<f32>,
    win_sum: f32,       // hann window scratch 
    p_scale: f32,       // (2 / win_sum)^2  ~ (4/N)^2
    buf: Vec<rustfft::num_complex::Complex<f32>>,
    scratch: Vec<rustfft::num_complex::Complex<f32>>,
    magsq: Vec<f32>,                 // one-sided power spectrum (normalized)
    bands: usize,
    band_edges: Vec<(usize, usize)>,
    last_levels_l: Vec<u8>,
    last_levels_r: Vec<u8>,
}

impl SpectrumEngine {
    pub fn new(sr: u32, samples_len: usize, bands: usize) -> Self {
        let want = samples_len.max(FFT_MIN).min(FFT_MAX);
        let nfft = want.next_power_of_two().min(FFT_MAX).max(FFT_MIN);

        let mut planner = rustfft::FftPlanner::<f32>::new();
        let fft: std::sync::Arc<dyn rustfft::Fft<f32> + Send + Sync> = planner.plan_fft_forward(nfft);

        // Hann
        let window = (0..nfft)
            .map(|i| 0.5f32 * (1.0 - (2.0 * std::f32::consts::PI * (i as f32) / (nfft as f32)).cos()))
            .collect::<Vec<_>>();

        // Normalization for single-sided power (see notes above)
        let win_sum: f32 = window.iter().copied().sum();
        let p_scale = (2.0 / win_sum).powi(2); // ≈ (4/N)^2 for Hann

        let buf = vec![rustfft::num_complex::Complex::<f32>::new(0.0, 0.0); nfft];
        let scratch = vec![rustfft::num_complex::Complex::<f32>::new(0.0, 0.0); fft.get_inplace_scratch_len()];
        let magsq = vec![0.0; nfft / 2];

        let band_edges = Self::build_log_bands(sr, nfft, bands);

        Self {
            sr, nfft, fft, window, win_sum, p_scale, buf, scratch, magsq, bands, band_edges,
            last_levels_l: vec![0; bands],
            last_levels_r: vec![0; bands],
        }
    }

    pub fn build_log_bands(sr: u32, nfft: usize, bands: usize) -> Vec<(usize, usize)> {
        let nyq = sr as f32 / 2.0;
        let fmin = SPECTRUM_MIN_HZ.min(nyq - 1.0).max(1.0);
        let fmax = (nyq * 0.98).max(fmin + 1.0);
        let mut edges = Vec::with_capacity(bands + 1);
        for i in 0..=bands {
            let t = i as f32 / (bands as f32);
            // log spacing
            let f = fmin * (fmax / fmin).powf(t);
            let k = ((f * (nfft as f32) / (sr as f32)).floor() as isize)
                .clamp(1, (nfft as isize / 2) - 1) as usize;
            edges.push(k);
        }
        // turn to (start,end) per band, ensure non-empty ranges
        let mut out = Vec::with_capacity(bands);
        for i in 0..bands {
            let mut a = edges[i];
            let mut b = edges[i + 1];
            if b <= a { b = (a + 1).min(nfft / 2); }
            out.push((a, b));
        }
        out
    }

    pub fn ensure(&mut self, sr: u32, samples_len: usize) {
        if self.sr != sr || samples_len < self.nfft / 2 || self.nfft > FFT_MAX {
            *self = Self::new(sr, samples_len, self.bands);
        }
    }

    /// Power dBFS per band with proper normalization (single-sided).
    pub fn compute_db_bands(&mut self, pcm: &[i16]) -> Vec<f32> {
        let need = self.nfft.min(pcm.len());
        let start = pcm.len().saturating_sub(need);

        // windowed real signal into buf
        for i in 0..need {
            let s = (pcm[start + i] as f32) / (i16::MAX as f32);
            self.buf[i].re = s * self.window[i];
            self.buf[i].im = 0.0;
        }
        for i in need..self.nfft {
            self.buf[i].re = 0.0;
            self.buf[i].im = 0.0;
        }

        // FFT
        self.fft.process_with_scratch(&mut self.buf, &mut self.scratch);

        // One-sided, normalized power. Double bins except DC/Nyquist.
        let half = self.nfft / 2;
        for k in 0..half {
            let re = self.buf[k].re;
            let im = self.buf[k].im;
            let mut p = (re * re + im * im) * self.p_scale; // normalize
            if k != 0 && k != half { p *= 2.0; }            // single-sided correction
            self.magsq[k] = p.max(0.0);
        }

        // Sum power per band, average, convert to dBFS
        let mut out = vec![HIST_FLOOR_DB; self.bands];
        for (bi, (a, b)) in self.band_edges.iter().enumerate() {
            let mut acc = 0.0f32;
            for k in *a..*b { acc += self.magsq[k]; }
            let bins = (*b - *a) as f32;
            let avg_p = if bins > 0.0 { acc / bins } else { 0.0 };
            out[bi] = 10.0 * (avg_p.max(EPS)).log10();
        }
        out
    }

    /// Map to integer display levels with smoothing/decay you already had.
    pub fn compute_levels(&mut self, left: &[i16], right: &[i16]) -> (Vec<u8>, Vec<u8>) {
        let db_l = self.compute_db_bands(left);
        let db_r = self.compute_db_bands(right);

        let mut lv_l = vec![0u8; self.bands];
        let mut lv_r = vec![0u8; self.bands];
        for i in 0..self.bands {
            lv_l[i] = db_to_hist_level(db_l[i]);
            lv_r[i] = db_to_hist_level(db_r[i]);

            // simple visual fall from your existing code (optional here)
            lv_l[i] = lv_l[i].max(self.last_levels_l[i].saturating_sub(1));
            lv_r[i] = lv_r[i].max(self.last_levels_r[i].saturating_sub(1));
        }
        self.last_levels_l.copy_from_slice(&lv_l);
        self.last_levels_r.copy_from_slice(&lv_r);
        (lv_l, lv_r)
    }

}
