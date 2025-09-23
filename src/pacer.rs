/*
 *  pacer.rs
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
use std::time::{Duration, Instant};

pub struct Pacer {
    next_deadline: Instant,
    frame: Duration,
}

// I²C we should manage 30fps
// SPI we can manage 60fps
// set the target accordingly
impl Pacer {
    pub fn new(target_fps: u32) -> Self {
        let frame = Duration::from_micros((1_000_000u32 / target_fps.max(1)) as u64);
        Self { next_deadline: Instant::now(), frame }
    }

    #[inline]
    pub fn set_fps(&mut self, fps: u32) {
        self.frame = Duration::from_micros((1_000_000u32 / fps.max(1)) as u64);
    }

    /// Returns true if we should flush now; if true, it also schedules the next deadline.
    #[inline]
    pub fn should_flush(&mut self) -> bool {
        let now = Instant::now();
        if now >= self.next_deadline {
            self.next_deadline = now + self.frame;
            true
        } else {
            false
        }
    }
}

pub struct AutoPacer {
    pacer: Pacer,
    ema_ms: f32,     // moving avg of flush time
    alpha: f32,      // smoothing (0.1 ~ 0.3)
    headroom: f32,   // >1.0 to avoid saturation (e.g. 1.25)
    max_fps: u32,    // user cap (e.g. 60 for SPI)
    min_fps: u32,    // floor (e.g. 10 for I²C)
}

impl AutoPacer {
    pub fn new(initial_fps: u32, max_fps: u32, min_fps: u32) -> Self {
        Self {
            pacer: Pacer::new(initial_fps),
            ema_ms: 0.0,
            alpha: 0.2,
            headroom: 1.25,
            max_fps,
            min_fps,
        }
    }
    pub fn should_flush(&mut self) -> bool { self.pacer.should_flush() }

    /// Call immediately after a successful display.flush().
    pub fn record_flush_ms(&mut self, flush_ms: f32) {
        self.ema_ms = if self.ema_ms == 0.0 {
            flush_ms
        } else {
            self.alpha * flush_ms + (1.0 - self.alpha) * self.ema_ms
        };
        if self.ema_ms > 0.0 {
            let safe_fps = (1000.0 / (self.ema_ms * self.headroom)).clamp(self.min_fps as f32, self.max_fps as f32) as u32;
            self.pacer.set_fps(safe_fps);
        }
    }
}

/*

// in LastVizState
pub vu_face_drawn: bool;

// when switching into a VU mode:
if !state.vu_face_drawn {
    draw_vu_face_1309(display, panel_left)?;
    draw_vu_face_1309(display, panel_right)?;
    state.vu_face_drawn = true;
}

*/