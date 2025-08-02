//! audio vizualizations - only used if shared memory data are accessible
//! 
//! 

#[allow(unused_imports)]
#[allow(dead_code)]
use embedded_graphics::{
    image::{ImageRaw},
    pixelcolor::BinaryColor,
    prelude::*, 
    primitives::{Rectangle}, 
};

use log::{info};
use std::error::Error;
use std::fmt;
use std::fs;

// supported audio visualizations
use crate::histogram;


pub const VIZ_TYPE_STEREO_HISTOGRAM: u8 = 10;
pub const VIZ_TYPE_DOWNMIX_HISTOGRAM: u8 = 20;
pub const VIZ_TYPE_STEREO_VUMETER: u8 = 30;
pub const VIZ_TYPE_PEAK_METER: u8 = 40;
pub const VIZ_TYPE_ALL_IN_ONE: u8 = 50;
pub const VIZ_TYPE_UNKNOWN: u8 = 255;

/// Custom error type for Visualization rendering operations.
#[derive(Debug)]
pub enum VisualizationError {
    /// Error parsing Viz configuration.
    _VizParseError(String),
    VizRenderError(String),
    VizBufferError(String),
}

impl fmt::Display for VisualizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VisualizationError::_VizParseError(msg) => write!(f, "Visualization parse error: {}", msg),
            VisualizationError::VizRenderError(msg) => write!(f, "Visualization SVG render error: {}", msg),
            VisualizationError::VizBufferError(msg) => write!(f, "Visualization buffer render error: {}", msg),
        }
    }
}

impl Error for VisualizationError {}

/// Renders Easter Viz [animation] from SVG data.
#[derive(Clone, Debug, PartialEq)]
pub struct Visualizer {
    pub viz_type: u8,
    rect: Rectangle,
}


#[allow(dead_code)]
impl Visualizer {

    /// Creates a new Visualization.
    pub fn new(
        viz_type: u8,
        rect: Rectangle, 
    ) -> Self {

        Self {
            viz_type,
            rect,
        }
    }

}