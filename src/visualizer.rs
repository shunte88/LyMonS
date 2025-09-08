/*
 *  visualizer.rs
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
//! audio visualizations - only used if shared memory data are accessible
//!

use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration, Instant};

use log::{info, error};

use crate::spectrum::{
    SpectrumEngine,
    SPECTRUM_BANDS_COUNT,
};
use crate::vision::{
    VisReader, peak_and_rms, dbfs,
    PEAK_METER_LEVELS_MAX, 
    DECAY_STEPS_PER_FRAME,
    FLOOR_DB, CEIL_DB,
    // Timings
    POLL_ENABLED,
    POLL_IDLE

};

/// Which visualization to produce.
#[derive(Debug, Clone, Copy)]
pub enum Visualization {
    VuStereo,                 // two VU meters (L/R)
    VuMono,                   // downmix to mono VU
    PeakStereo,               // two peak meters with hold/decay
    PeakMono,                 // mono peak meter with hold/decay
    HistStereo,               // two histogram bars (L/R)
    HistMono,                 // mono histogram (downmix)
    VuStereoWithCenterPeak,   // L/R VU with a central mono peak meter
    AioVuMono,                // All In One with downmix VU
    AioHistMono,              // All In One with downmix histogram
    NoVisualization,          // no visualization
}

/// A published frame for the display to render.
#[derive(Debug, Clone)]
pub struct VizFrameOut {
    pub ts: i64,
    pub playing: bool,
    pub sample_rate: u32,
    pub kind: Visualization,
    pub payload: VizPayload,
}

/// Data payload per visualization type.
#[derive(Debug, Clone)]
pub enum VizPayload {
    VuStereo { l_db: f32, r_db: f32 },
    VuMono   { db: f32 },
    PeakStereo {
        l_level: u8, r_level: u8,
        l_hold: u8,  r_hold: u8,
    },
    PeakMono { level: u8, hold: u8 },
    HistStereo { bands_l: Vec<u8>, bands_r: Vec<u8> },
    HistMono   { bands: Vec<u8> },
    VuStereoWithCenterPeak { l_db: f32, r_db: f32, peak_level: u8, peak_hold: u8 },
    AioVuMono { db: f32 },
    AioHistMono { bands: Vec<u8> },
    NoVisualization {},
}

/// Commands sent to the background worker.
#[derive(Debug, Clone)]
pub enum VizCommand {
    Enable(bool),              // enable/disable publishing
    SetKind(Visualization),    // switch viz mode
    Shutdown,                  // stop worker
}

/// Public handle that coordinates the worker and the display consumer.
pub struct Visualizer {
    cmd_tx: Sender<VizCommand>,
    join: Option<JoinHandle<()>>,
    /// Display consumes frames from here.
    pub rx: Receiver<VizFrameOut>,
}

fn transpose_kind(kind: &str) -> Visualization {
    match kind {
        "vu_stereo" => Visualization::VuStereo,
        "vu_mono" => Visualization::VuMono,
        "peak_stereo" => Visualization::PeakStereo,
        "peak_mono" => Visualization::PeakMono,
        "hist_stereo" => Visualization::HistStereo,
        "hist_mono" => Visualization::HistMono,
        "vu_stereo_with_center_peak" | "combination" => Visualization::VuStereoWithCenterPeak,
        "aio_vu_mono" => Visualization::AioVuMono,
        "aio_hist_mono" => Visualization::AioHistMono,
        "no_viz" => Visualization::NoVisualization,
        &_ => Visualization::NoVisualization,
    }
}

impl Visualizer {
    /// Spawn the background worker. It initializes at startup but does no
    /// heavy work until you `Enable(true)` *and* a track is playing.
    pub fn spawn(kind: &str, playing_rx: watch::Receiver<bool>) -> std::io::Result<Self> {
        // small bounded queues (drop newest when full via try_send)
        let (cmd_tx, cmd_rx) = mpsc::channel::<VizCommand>(16);
        let (out_tx, out_rx) = mpsc::channel::<VizFrameOut>(128);

        let kind = transpose_kind(kind);
        // prime initial state
        let _ = cmd_tx.try_send(VizCommand::Enable(false));
        let _ = cmd_tx.try_send(VizCommand::SetKind(kind));

        // spawn async worker task
        let join = tokio::spawn(async move {
            visualizer_worker(cmd_rx, out_tx, playing_rx).await
        });

        Ok(Self { cmd_tx, join: Some(join), rx: out_rx })
    }

    // Keep caller-side simple (no .await); best-effort send.
    pub fn enable(&self, on: bool) {
        let _ = self.cmd_tx.try_send(VizCommand::Enable(on));
    }

    pub fn set_kind(&self, k: Visualization) {
        let _ = self.cmd_tx.try_send(VizCommand::SetKind(k));
    }

    /// Ask the worker to stop; the task will exit on its own.
    pub fn shutdown(mut self) {
        let _ = self.cmd_tx.try_send(VizCommand::Shutdown);
        if let Some(handle) = self.join.take() {
            handle.abort(); // fire-and-forget; worker is cooperative too
        }
    }
}

impl Drop for Visualizer {
    fn drop(&mut self) {
        let _ = self.cmd_tx.try_send(VizCommand::Shutdown);
        if let Some(handle) = self.join.take() {
            handle.abort();
        }
    }
}

async fn visualizer_worker(
    mut cmd_rx: Receiver<VizCommand>,
    mut out_tx: Sender<VizFrameOut>,
    playing_rx: watch::Receiver<bool>,
) {
    // Reader + FFT engine
    let mut reader = match VisReader::new() {
        Ok(r) => r,
        Err(e) => {
            error!("visualizer: failed to init VisReader: {e}");
            return;
        }
    };
    let mut eng: Option<SpectrumEngine> = None;

    // State
    let mut enabled = false;
    let mut kind = Visualization::VuStereo;

    // Peak-hold (for peak meters & center peak). Units: 0..=PEAK_METER_LEVELS_MAX
    let mut peak_hold_l: u8 = 0;
    let mut peak_hold_r: u8 = 0;
    let mut peak_hold_m: u8 = 0;

    info!("visualizer worker started (idle)");
    let mut last_is_playing = false;

    'outer: loop {
        // drain any pending commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                VizCommand::Enable(on) => { enabled = on; }
                VizCommand::SetKind(k) => { kind = k; }
                VizCommand::Shutdown   => { break 'outer; }
            }
        }

        let is_playing = *playing_rx.borrow();
        /*
        // send status change payload - early doors!!!
        if last_is_playing != is_playing {
            last_is_playing = is_playing;
            //debug!("Visualizer: playing is now {}", if is_playing { "on" } else { "off" });
            publish(&mut out_tx,
                0_i64,
                is_playing,
                44_100,
                Visualization::NoVisualization,
                VizPayload::NoVisualization {});
        }
        */
        
        // if not ebabled or playing - nap then continue
        if !enabled || !is_playing {
            sleep(POLL_IDLE).await;
            continue;
        }

        // Get fresh audio, when available.
        match reader.with_data_extended(|frame, left, right| {
            if !frame.running {
                return;
            }

            // Build / refresh spectrum engine lazily (for histogram modes)
            let need_bands = SPECTRUM_BANDS_COUNT as usize;
            match &mut eng {
                Some(e) => e.ensure(frame.sample_rate, left.len()),
                None => eng = Some(SpectrumEngine::new(frame.sample_rate, left.len(), need_bands)),
            }

            // Compute per chosen viz
            match kind {
                Visualization::VuStereo => {
                    let (_pk_l, rms_l) = peak_and_rms(left);
                    let (_pk_r, rms_r) = peak_and_rms(right);
                    let l_db = dbfs(rms_l);
                    let r_db = dbfs(rms_r);
                    publish(&mut out_tx, frame.timestamp, is_playing, frame.sample_rate, kind,
                        VizPayload::VuStereo { l_db, r_db });
                }

                Visualization::VuMono | Visualization::AioVuMono => {
                    let (_pk_l, rms_l) = peak_and_rms(left);
                    let (_pk_r, rms_r) = peak_and_rms(right);
                    // downmix RMS ≈ sqrt((L^2 + R^2)/2)
                    let mono_rms = (((rms_l*rms_l) + (rms_r*rms_r)) * 0.5).sqrt();
                    let db = dbfs(mono_rms);
                    publish(&mut out_tx, frame.timestamp, is_playing, frame.sample_rate, kind,
                        VizPayload::VuMono { db });
                }

                Visualization::PeakStereo => {
                    // Peaks in dBFS → level
                    let (pk_l_i16, _) = peak_and_rms(left);
                    let (pk_r_i16, _) = peak_and_rms(right);
                    let l_db = dbfs(pk_l_i16 as f32);
                    let r_db = dbfs(pk_r_i16 as f32);
                    let mut l_level = db_to_level(l_db);
                    let mut r_level = db_to_level(r_db);
                    // peak-hold with decay
                    peak_hold_l = peak_hold_l.saturating_sub(DECAY_STEPS_PER_FRAME).max(l_level);
                    peak_hold_r = peak_hold_r.saturating_sub(DECAY_STEPS_PER_FRAME).max(r_level);
                    
                    // clamp to range - REVIEW! tweak a tad as we're getting a whole bunch of clipping
                    l_level = l_level.min(PEAK_METER_LEVELS_MAX);
                    r_level = r_level.min(PEAK_METER_LEVELS_MAX);
                 
                    publish(&mut out_tx, frame.timestamp, is_playing, frame.sample_rate, kind,
                        VizPayload::PeakStereo {
                            l_level, r_level,
                            l_hold: peak_hold_l, r_hold: peak_hold_r,
                        });
                }

                Visualization::PeakMono => {
                    let (pk_l_i16, _) = peak_and_rms(left);
                    let (pk_r_i16, _) = peak_and_rms(right);
                    let mono_pk_db = dbfs(pk_l_i16.max(pk_r_i16) as f32);
                    let mut level = db_to_level(mono_pk_db);
                    peak_hold_m = peak_hold_m.saturating_sub(DECAY_STEPS_PER_FRAME).max(level);
                    level = level.min(PEAK_METER_LEVELS_MAX);

                    publish(&mut out_tx, frame.timestamp, is_playing, frame.sample_rate, kind,
                        VizPayload::PeakMono { level, hold: peak_hold_m });
                }

                Visualization::HistStereo => {
                    if let Some(e) = &mut eng {
                        let (bands_l, bands_r) = e.compute_levels(left, right);
                        publish(&mut out_tx, frame.timestamp, is_playing, frame.sample_rate, kind,
                            VizPayload::HistStereo { bands_l, bands_r });
                    }
                }

                Visualization::HistMono | Visualization::AioHistMono => {
                    if let Some(e) = &mut eng {
                        let (l, r) = e.compute_levels(left, right);
                        // downmix = max(L,R) per band (punchier than mean)
                        let bands = l.iter().zip(r.iter())
                                     .map(|(a,b)| (*a).max(*b))
                                     .collect::<Vec<u8>>();
                        publish(&mut out_tx, frame.timestamp, is_playing, frame.sample_rate, kind,
                            VizPayload::HistMono { bands });
                    }
                }

                Visualization::VuStereoWithCenterPeak => {
                    let (pk_l_i16, rms_l) = peak_and_rms(left);
                    let (pk_r_i16, rms_r) = peak_and_rms(right);
                    let l_db = dbfs(rms_l);
                    let r_db = dbfs(rms_r);

                    let peak_db = dbfs(pk_l_i16.max(pk_r_i16) as f32);
                    let mut level = db_to_level(peak_db);
                    peak_hold_m = peak_hold_m.saturating_sub(DECAY_STEPS_PER_FRAME).max(level);
                    level = level.min(PEAK_METER_LEVELS_MAX);

                    publish(&mut out_tx, frame.timestamp, is_playing, frame.sample_rate, kind,
                        VizPayload::VuStereoWithCenterPeak {
                            l_db, r_db, peak_level: level, peak_hold: peak_hold_m
                        });
                }

                Visualization::NoVisualization => {}
            }
        }) {
            Ok(true)  => { /* published */ }
            Ok(false) => {
                // nothing new; keep CPU low and try remap if stale
                sleep(POLL_ENABLED).await;
                let _ = reader.reopen_if_stale();
            }
            Err(e) => {
                error!("visualizer: snapshot error: {e}");
                sleep(POLL_ENABLED).await;
            }
        }
    }

    info!("visualizer worker stopped");
}

#[inline]
fn publish(
    tx: &mut Sender<VizFrameOut>,
    ts: i64,
    playing: bool,
    sr: u32,
    kind: Visualization,
    payload: VizPayload,
) {
    // Best-effort, non-blocking; if the queue is full, drop the frame.
    let _ = tx.try_send(VizFrameOut { ts, playing, sample_rate: sr, kind, payload });
}

/// Same mapping we use for hist levels: dBFS → 0..=PEAK_METER_LEVELS_MAX
#[inline]
fn db_to_level(db: f32) -> u8 {
    let x = ((db - FLOOR_DB) / (CEIL_DB - FLOOR_DB)).clamp(0.0, 1.0);
    (x * PEAK_METER_LEVELS_MAX as f32).round() as u8
}
