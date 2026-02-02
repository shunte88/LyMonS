/*
 *  display/components/status_bar.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Status bar component - displays volume, playback mode, bitrate, etc.
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *  Public License.
 *
 */

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::BinaryColor;
use crate::display::layout::LayoutConfig;
use arrayvec::ArrayString;
use core::fmt::Write;

/// Repeat mode for playback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

/// Shuffle mode for playback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShuffleMode {
    Off,
    ByTracks,
    ByAlbums,
}

/// Audio bitrate information (stack-allocated for zero heap allocations)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioBitrate {
    Unknown,
    Bitrate(ArrayString<16>), // e.g., "24/192" - max 16 chars on stack
}

/// Status bar state
#[derive(Debug, Clone)]
pub struct StatusBarState {
    /// Volume level (0-100)
    pub volume_percent: u8,

    /// Whether audio is muted
    pub is_muted: bool,

    /// Repeat mode
    pub repeat_mode: RepeatMode,

    /// Shuffle mode
    pub shuffle_mode: ShuffleMode,

    /// Audio bitrate information
    pub audio_bitrate: AudioBitrate,

    /// Sample rate (for logic, not always displayed) - stack allocated
    pub samplerate: ArrayString<8>,

    /// Sample size (for logic, not always displayed) - stack allocated
    pub samplesize: ArrayString<8>,

    /// Formatted bitrate text for display - stack allocated
    pub bitrate_text: ArrayString<16>,
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self {
            volume_percent: 50,
            is_muted: false,
            repeat_mode: RepeatMode::Off,
            shuffle_mode: ShuffleMode::Off,
            audio_bitrate: AudioBitrate::Unknown,
            samplerate: ArrayString::new(),
            samplesize: ArrayString::new(),
            bitrate_text: ArrayString::new(),
        }
    }
}

/// Status bar component
pub struct StatusBar {
    state: StatusBarState,
    layout: LayoutConfig,
}

impl StatusBar {
    /// Create a new status bar component
    pub fn new(layout: LayoutConfig) -> Self {
        Self {
            state: StatusBarState::default(),
            layout,
        }
    }

    /// Update status bar state
    pub fn update(&mut self, state: StatusBarState) {
        self.state = state;
    }

    /// Get current state
    pub fn state(&self) -> &StatusBarState {
        &self.state
    }

    /// Render the status bar
    pub fn render<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        // TODO: Implement actual rendering
        // This is a placeholder that would draw:
        // - Volume indicator (muted if applicable)
        // - Repeat mode icon
        // - Shuffle mode icon
        // - Bitrate text

        Ok(())
    }

    /// Update volume
    pub fn set_volume(&mut self, volume: u8) {
        self.state.volume_percent = volume.min(100);
    }

    /// Update mute state
    pub fn set_muted(&mut self, muted: bool) {
        self.state.is_muted = muted;
    }

    /// Update repeat mode
    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.state.repeat_mode = mode;
    }

    /// Update shuffle mode
    pub fn set_shuffle_mode(&mut self, mode: ShuffleMode) {
        self.state.shuffle_mode = mode;
    }

    /// Update bitrate information (zero heap allocations!)
    pub fn set_bitrate(&mut self, samplerate: &str, samplesize: &str) {
        // Clear and populate stack-allocated strings
        self.state.samplerate.clear();
        self.state.samplesize.clear();
        self.state.bitrate_text.clear();

        // Truncate if needed (shouldn't happen with normal audio specs)
        let _ = self.state.samplerate.try_push_str(samplerate);
        let _ = self.state.samplesize.try_push_str(samplesize);

        // Format bitrate text (e.g., "24/192") - no heap allocation!
        let _ = write!(&mut self.state.bitrate_text, "{}/{}",
            self.state.samplesize, self.state.samplerate);

        self.state.audio_bitrate = AudioBitrate::Bitrate(self.state.bitrate_text);
    }

    /// Format volume text to stack-allocated string (zero allocations!)
    pub fn format_volume(&self) -> ArrayString<8> {
        let mut buf = ArrayString::new();
        if self.state.is_muted {
            let _ = write!(&mut buf, "MUTE");
        } else {
            let _ = write!(&mut buf, "{:>3}%", self.state.volume_percent);
        }
        buf
    }
}

/// Helper function to transpose repeat mode from integer
pub fn transpose_repeat_mode(mode: i32) -> RepeatMode {
    match mode {
        0 => RepeatMode::Off,
        1 => RepeatMode::All,
        2 => RepeatMode::One,
        _ => RepeatMode::Off,
    }
}

/// Helper function to transpose shuffle mode from integer
pub fn transpose_shuffle_mode(mode: i32) -> ShuffleMode {
    match mode {
        0 => ShuffleMode::Off,
        1 => ShuffleMode::ByTracks,
        2 => ShuffleMode::ByAlbums,
        _ => ShuffleMode::Off,
    }
}
