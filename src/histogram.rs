//! A 12-band stereo audio histogram widget for `embedded-graphics` on a 128x64 display.

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, Primitive, PrimitiveStyle, Rectangle},
    text::Text,
};
use rand::Rng;
use std::time::{Duration, Instant};

const NUM_BANDS: usize = 12;
const PEAK_DECAY_RATE: f32 = 0.05; // Adjusted for faster updates
const PEAK_HOLD_DURATION: Duration = Duration::from_millis(800);
const DISPLAY_WIDTH: u32 = 128;
const DISPLAY_HEIGHT: u32 = 64;

/// Represents the state of a single audio band.
#[derive(Debug, Clone, Copy)]
struct AudioBand {
    /// The current value of the band (0.0 to 1.0).
    value: f32,
    /// The peak value the band has reached.
    peak: f32,
    /// The time the last peak was set.
    peak_last_set: Instant,
}

impl Default for AudioBand {
    fn default() -> Self {
        Self {
            value: 0.0,
            peak: 0.0,
            peak_last_set: Instant::now(),
        }
    }
}

/// Holds the state for the entire histogram.
#[derive(Debug, Clone)]
pub struct HistogramState {
    left_bands: Vec<AudioBand>,
    right_bands: Vec<AudioBand>,
}

impl HistogramState {
    pub fn new() -> Self {
        Self {
            left_bands: vec![AudioBand::default(); NUM_BANDS],
            right_bands: vec![AudioBand::default(); NUM_BANDS],
        }
    }

    /// Updates the band values and handles peak decay logic.
    pub fn update(&mut self, left_data: &[f32], right_data: &[f32]) {
        let now = Instant::now();
        let update_channel = |bands: &mut [AudioBand], data: &[f32]| {
            for (i, band) in bands.iter_mut().enumerate() {
                let new_value = data.get(i).cloned().unwrap_or(0.0);
                band.value = new_value;

                if new_value >= band.peak {
                    band.peak = new_value;
                    band.peak_last_set = now;
                } else if now.duration_since(band.peak_last_set) > PEAK_HOLD_DURATION {
                    band.peak = (band.peak - PEAK_DECAY_RATE).max(band.value);
                }
            }
        };

        update_channel(&mut self.left_bands, left_data);
        update_channel(&mut self.right_bands, right_data);
    }
}

/// The audio histogram renderer.
pub struct AudioHistogram {
    state: HistogramState,
}

impl AudioHistogram {
    pub fn new() -> Self {
        Self {
            state: HistogramState::new(),
        }
    }

    /// Update the internal state with new audio data.
    /// fed from routine chomping shared memory
    /// will need to lock, clone, release to acquire data
    pub fn update(&mut self, left_data: &[f32], right_data: &[f32]) {
        self.state.update(left_data, right_data);
    }

    /// Draws the histogram onto a given DrawTarget.
    pub fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        // Define dimensions and layout for the 128x64 display
        let band_width = 4;
        let band_spacing = 1;
        let center_gap = 8;
        let total_channel_width = (NUM_BANDS as u32 * band_width) + ((NUM_BANDS - 1) as u32 * band_spacing);
        let start_x_left = (DISPLAY_WIDTH - 2 * total_channel_width - center_gap) / 2;
        let start_x_right = start_x_left + total_channel_width + center_gap;

        let bar_style = PrimitiveStyle::with_fill(BinaryColor::On);
        let peak_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

        // --- Draw Left Channel ---
        for (i, band) in self.state.left_bands.iter().enumerate() {
            let x_pos = start_x_left + (i as u32 * (band_width + band_spacing));
            self.draw_band(display, band, x_pos, band_width, bar_style, peak_style)?;
        }

        // --- Draw Right Channel ---
        for (i, band) in self.state.right_bands.iter().enumerate() {
            let x_pos = start_x_right + (i as u32 * (band_width + band_spacing));
            self.draw_band(display, band, x_pos, band_width, bar_style, peak_style)?;
        }

        Ok(())
    }

    /// Draws a single band (bar and peak cap) using embedded-graphics primitives.
    fn draw_band<D>(
        &self,
        display: &mut D,
        band: &AudioBand,
        x: u32,
        width: u32,
        bar_style: PrimitiveStyle<BinaryColor>,
        peak_style: PrimitiveStyle<BinaryColor>,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let max_bar_height = DISPLAY_HEIGHT - 1; // Leave 1px for peak cap

        // --- Draw the main bar ---
        let bar_height = (band.value * max_bar_height as f32).round() as u32;
        if bar_height > 0 {
            Rectangle::new(
                Point::new(x as i32, (DISPLAY_HEIGHT - bar_height) as i32),
                Size::new(width, bar_height),
            )
            .into_styled(bar_style)
            .draw(display)?;
        }

        // --- Draw the peak cap ---
        let peak_height = (band.peak * max_bar_height as f32).round() as u32;
        if peak_height > 0 {
            let peak_y = (DISPLAY_HEIGHT - peak_height) as i32;
            Line::new(
                Point::new(x as i32, peak_y),
                Point::new((x + width - 1) as i32, peak_y),
            )
            .into_styled(peak_style)
            .draw(display)?;
        }
        Ok(())
    }
}

/*
/// Main function for simulation purposes.
/// In your project, you would integrate `AudioHistogram` into your main loop.
fn draw_stereo_histogram(&mut display) -> Result<(), std::convert::Infallible> {

    let mut histogram = AudioHistogram::new();
    let mut rng = rand::rng();

    loop {
        display.clear(BinaryColor::Off)?;

        // --- Simulate new audio data ---
        let left_data: Vec<f32> = (0..NUM_BANDS).map(|_| rng.r#random::<f32>()).collect();
        let right_data: Vec<f32> = (0..NUM_BANDS).map(|_| rng.r#random::<f32>()).collect();
        histogram.update(&left_data, &right_data);

        // --- Draw the histogram ---
        histogram.draw(&mut display)?;

        // --- Draw some helper text (optional) ---
        let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        Text::new("L", Point::new(2, 10), text_style).draw(&mut display)?;
        Text::new("R", Point::new(120, 10), text_style).draw(&mut display)?;

        std::thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}
*/