/*
 *  display/components/visualizer.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Audio visualizer component
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
use crate::visualizer::Visualizer;
use crate::visualization::Visualization;

/// Visualizer component state
#[derive(Debug, Clone)]
pub struct VisualizerState {
    /// Audio level (0-100)
    pub level: u8,

    /// Peak percentage
    pub pct: f64,

    /// Whether visualizer needs initialization clear
    pub viz_init_clear: bool,
}

impl Default for VisualizerState {
    fn default() -> Self {
        Self {
            level: 0,
            pct: 0.0,
            viz_init_clear: false,
        }
    }
}

/// Visualizer component wrapper
pub struct VisualizerComponent {
    visualizer: Option<Visualizer>,
    state: VisualizerState,
    layout: LayoutConfig,
    visualization_type: Visualization,
}

impl VisualizerComponent {
    /// Create a new visualizer component
    pub fn new(layout: LayoutConfig, visualization_type: Visualization) -> Self {
        Self {
            visualizer: None,
            state: VisualizerState::default(),
            layout,
            visualization_type,
        }
    }

    /// Initialize the visualizer with actual Visualizer instance
    pub fn set_visualizer(&mut self, visualizer: Visualizer) {
        self.visualizer = Some(visualizer);
    }

    /// Get mutable reference to visualizer
    pub fn visualizer_mut(&mut self) -> Option<&mut Visualizer> {
        self.visualizer.as_mut()
    }

    /// Get reference to visualizer
    pub fn visualizer(&self) -> Option<&Visualizer> {
        self.visualizer.as_ref()
    }

    /// Update visualizer state
    pub fn update(&mut self, level: u8, pct: f64) {
        self.state.level = level;
        self.state.pct = pct;
    }

    /// Get current state
    pub fn state(&self) -> &VisualizerState {
        &self.state
    }

    /// Get visualization type
    pub fn visualization_type(&self) -> Visualization {
        self.visualization_type
    }

    /// Set visualization type
    pub fn set_visualization_type(&mut self, viz_type: Visualization) {
        self.visualization_type = viz_type;
    }

    /// Render the visualizer
    pub fn render<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        // TODO: Implement actual visualizer rendering
        // This would draw VU meters, peak meters, histograms, etc.
        // based on the visualization_type and current audio data

        Ok(())
    }

    /// Mark that initialization clear is needed
    pub fn mark_init_clear(&mut self) {
        self.state.viz_init_clear = true;
    }

    /// Check if init clear is needed
    pub fn needs_init_clear(&self) -> bool {
        self.state.viz_init_clear
    }

    /// Clear the init flag
    pub fn clear_init_flag(&mut self) {
        self.state.viz_init_clear = false;
    }
}
