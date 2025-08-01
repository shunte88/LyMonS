//! Module for rendering simple SVG images to a monochrome pixel buffer.
//!
//! This module uses `usvg` for SVG parsing and `resvg` for rendering.
//! The output is a 1-bit per pixel monochrome bitmap, suitable for OLED displays
//! using `embedded-graphics::image::ImageRaw`.

use resvg::{
    render, 
    usvg::{
        Tree as ResvgTree, 
        Options as ResvgUsvgOptions, 
        Transform,
    }
}; // Use resvg's re-exports for usvg types

//use roxmltree::Document;
use tiny_skia::Pixmap;
use log::{debug, error};
use std::{error::Error};
use std::fmt;

/// Custom error type for SVG rendering operations.
#[derive(Debug)]
pub enum SvgImageError {
    /// Error parsing the SVG data.
    SvgParseError(String),
    /// Error creating a pixmap for rendering.
    PixmapCreationError(String),
    /// The provided buffer is too small for the target image size.
    BufferTooSmall,
    /// Generic rendering error.
    RenderingError(String),
    /// Node with specified ID not found.
    _NodeNotFound(String),
    /// Attempted to apply an animation type to an incompatible SVG node.
    _IncompatibleNodeType(String),
}

impl fmt::Display for SvgImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SvgImageError::SvgParseError(msg) => write!(f, "SVG parse error: {}", msg),
            SvgImageError::PixmapCreationError(msg) => write!(f, "Pixmap creation error: {}", msg),
            SvgImageError::BufferTooSmall => write!(f, "Provided buffer is too small for SVG rendering."),
            SvgImageError::RenderingError(msg) => write!(f, "SVG rendering error: {}", msg),
            SvgImageError::_NodeNotFound(id) => write!(f, "SVG node with ID '{}' not found.", id),
            SvgImageError::_IncompatibleNodeType(msg) => write!(f, "Incompatible SVG node type: {}", msg),
        }
    }
}

impl Error for SvgImageError {}

/// Renders simple SVG data to a monochrome pixel buffer.
#[derive(Debug)]
pub struct SvgImageRenderer {
    tree: ResvgTree,
    target_width: u32,
    target_height: u32,
}

#[allow(dead_code)]
impl SvgImageRenderer {
    /// Creates a new `SvgImageRenderer` from SVG string data and target dimensions.
    ///
    /// The SVG will be scaled to fit `target_width` and `target_height`.
    pub fn new(svg_data: &str, target_width: u32, target_height: u32) -> Result<Self, SvgImageError> {
        let usvg_options = ResvgUsvgOptions::default(); // Use resvg's re-exported Options   
        let tree = ResvgTree::from_str(svg_data, &usvg_options) // Use resvg's re-exported Tree
            .map_err(|e| SvgImageError::SvgParseError(format!("Failed to parse SVG: {:?}", e)))?;
        Ok(SvgImageRenderer {
            tree,
            target_width,
            target_height,
        })
    }

    /// Renders the SVG to a mutable byte slice, converting it to a 1-bit monochrome format.
    /// The `buffer` must be large enough to hold `(target_width * target_height / 8)` bytes.
    /// Each bit in the buffer represents a pixel (1 for BinaryColor::On, 0 for BinaryColor::Off).
    /// The format is row-major, LSB-first within each byte.
    pub fn render_to_buffer(&self, buffer: &mut [u8]) -> Result<(), SvgImageError> {

        let padded_width = (self.target_width + 7) / 8;
        let buffer_len_needed = self.target_height as usize * padded_width as usize;
        if buffer.len() < buffer_len_needed {
            error!(
                "Buffer too small. Needed: {} bytes, Got: {} bytes",
                buffer_len_needed,
                buffer.len()
            );
            return Err(SvgImageError::BufferTooSmall);
        }

        // Clear the buffer to ensure all bits are initially off
        buffer.fill(0);

        // Create a Pixmap for rendering the SVG (RGBA format)
        let mut pixmap = Pixmap::new(self.target_width, self.target_height)
            .ok_or_else(|| SvgImageError::PixmapCreationError("Failed to create pixmap".to_string()))?;

        // scaling transform
        // For simple scaling from (0,0), a direct scale transform is sufficient.
        // If the SVG has a viewBox with a non-zero origin, more complex translation might be needed.
        let svg_size = self.tree.size();
        let scale_x = self.target_width as f32 / svg_size.width();
        let scale_y = self.target_height as f32 / svg_size.height();        
        let transform = Transform::from_scale(scale_x, scale_y);

        // Render the SVG to the pixmap - majorly hit and miss!!!
        let threshold = 128;
        render(&self.tree, transform, &mut pixmap.as_mut());

        pixmap
            .pixels()
            .chunks(self.target_width as usize)
            .take(self.target_height as usize)
            .enumerate()
            .for_each(|(y, row)| {
                row.iter().enumerate().for_each(|(x, p)| {
                    let luminance = 0.299 * p.red() as f32 + 0.597 * p.green() as f32 + 0.114 * p.blue() as f32;
                    if luminance > threshold as f32 && p.alpha() > threshold {
                        let byte_idx = y * padded_width as usize + (x / 8); // Corrected byte index
                        let bit_idx = x % 8; // Bit index within the byte
                        buffer[byte_idx] |= 1 << (7 - bit_idx);
                    }
                });
            });
    
        debug!("SVG rendered to buffer successfully.");
        Ok(())
    }

    /// Renders the SVG to a mutable byte slice, converting it to a 1-bit monochrome format.
    ///
    /// The `buffer` must be large enough to hold `(target_width * target_height / 8)` bytes.
    /// Each bit in the buffer represents a pixel (1 for BinaryColor::On, 0 for BinaryColor::Off).
    /// The format is row-major, LSB-first within each byte.
    pub fn render_to_buffer_dither(&self, buffer: &mut [u8]) -> Result<(), SvgImageError> {

        let padded_width = (self.target_width + 7) / 8;
        let buffer_len_needed = self.target_height as usize * padded_width as usize;
        if buffer.len() < buffer_len_needed {
            error!(
                "Buffer too small. Needed: {} bytes, Got: {} bytes",
                buffer_len_needed,
                buffer.len()
            );
            return Err(SvgImageError::BufferTooSmall);
        }

        // Clear the buffer to ensure all bits are initially off
        buffer.fill(0);

        // Create a Pixmap for rendering the SVG (RGBA format)
        let mut pixmap = Pixmap::new(self.target_width, self.target_height)
            .ok_or_else(|| SvgImageError::PixmapCreationError("Failed to create pixmap".to_string()))?;

        // Calculate scaling transformation using the SVG's intrinsic size
        let svg_size = self.tree.size();
        let scale_x = self.target_width as f32 / svg_size.width();
        let scale_y = self.target_height as f32 / svg_size.height();
        
        // For simple scaling from (0,0), a direct scale transform is sufficient.
        // If the SVG has a viewBox with a non-zero origin, more complex translation might be needed.
        let transform = Transform::from_scale(scale_x, scale_y);

        // Render the SVG to the pixmap
        render(&self.tree, transform, &mut pixmap.as_mut());

        // --- Apply Floyd-Steinberg Dithering ---
        let width = self.target_width as usize;
        let height = self.target_height as usize;
        let threshold = 127; // Grayscale threshold for 1-bit output

        // Pixmap data is RGBA, 4 bytes per pixel
        let pixmap_data = pixmap.data_mut(); // Get mutable access to raw pixel data

        for y in 0..height {
            for x in 0..width {
                let current_pixel_idx = (y * width + x) * 4; // RGBA, 4 bytes per pixel

                // Get original RGBA values
                let r = pixmap_data[current_pixel_idx] as f32;
                let g = pixmap_data[current_pixel_idx + 1] as f32;
                let b = pixmap_data[current_pixel_idx + 2] as f32;
                let a = pixmap_data[current_pixel_idx + 3] as f32;

                // Convert to grayscale for dithering calculation
                let old_grayscale = 0.299 * r + 0.587 * g + 0.114 * b; // Using standard luminance weights

                // Determine the quantized (1-bit) value
                let new_grayscale_val = if old_grayscale > threshold as f32 { 255.0 } else { 0.0 };

                // Calculate error
                let error = old_grayscale - new_grayscale_val;

                // Update the current pixel in the pixmap to its quantized value
                // We set R, G, B to the new grayscale value, keeping original alpha or setting to 255 if it was 0.
                let new_r = new_grayscale_val;
                let new_g = new_grayscale_val;
                let new_b = new_grayscale_val;

                pixmap_data[current_pixel_idx] = new_r.round() as u8;
                pixmap_data[current_pixel_idx + 1] = new_g.round() as u8;
                pixmap_data[current_pixel_idx + 2] = new_b.round() as u8;
                // Keep original alpha or make fully opaque if it was a colored pixel
                if a > 0.0 {
                    pixmap_data[current_pixel_idx + 3] = 255;
                }


                // Distribute error to neighbors
                // (x + 1, y)   * 7 / 16
                if x + 1 < width {
                    let neighbor_idx = (y * width + (x + 1)) * 4;
                    let current_val = 0.299 * pixmap_data[neighbor_idx] as f32 + 0.587 * pixmap_data[neighbor_idx + 1] as f32 + 0.114 * pixmap_data[neighbor_idx + 2] as f32;
                    let new_val = (current_val + error * 7.0 / 16.0).clamp(0.0, 255.0);
                    pixmap_data[neighbor_idx] = new_val.round() as u8;
                    pixmap_data[neighbor_idx + 1] = new_val.round() as u8;
                    pixmap_data[neighbor_idx + 2] = new_val.round() as u8;
                }
                // (x - 1, y + 1) * 3 / 16
                if x > 0 && y + 1 < height {
                    let neighbor_idx = ((y + 1) * width + (x - 1)) * 4;
                    let current_val = 0.299 * pixmap_data[neighbor_idx] as f32 + 0.587 * pixmap_data[neighbor_idx + 1] as f32 + 0.114 * pixmap_data[neighbor_idx + 2] as f32;
                    let new_val = (current_val + error * 3.0 / 16.0).clamp(0.0, 255.0);
                    pixmap_data[neighbor_idx] = new_val.round() as u8;
                    pixmap_data[neighbor_idx + 1] = new_val.round() as u8;
                    pixmap_data[neighbor_idx + 2] = new_val.round() as u8;
                }
                // (x, y + 1)   * 5 / 16
                if y + 1 < height {
                    let neighbor_idx = ((y + 1) * width + x) * 4;
                    let current_val = 0.299 * pixmap_data[neighbor_idx] as f32 + 0.587 * pixmap_data[neighbor_idx + 1] as f32 + 0.114 * pixmap_data[neighbor_idx + 2] as f32;
                    let new_val = (current_val + error * 5.0 / 16.0).clamp(0.0, 255.0);
                    pixmap_data[neighbor_idx] = new_val.round() as u8;
                    pixmap_data[neighbor_idx + 1] = new_val.round() as u8;
                    pixmap_data[neighbor_idx + 2] = new_val.round() as u8;
                }
                // (x + 1, y + 1) * 1 / 16
                if x + 1 < width && y + 1 < height {
                    let neighbor_idx = ((y + 1) * width + (x + 1)) * 4;
                    let current_val = 0.299 * pixmap_data[neighbor_idx] as f32 + 0.587 * pixmap_data[neighbor_idx + 1] as f32 + 0.114 * pixmap_data[neighbor_idx + 2] as f32;
                    let new_val = (current_val + error * 1.0 / 16.0).clamp(0.0, 255.0);
                    pixmap_data[neighbor_idx] = new_val.round() as u8;
                    pixmap_data[neighbor_idx + 1] = new_val.round() as u8;
                    pixmap_data[neighbor_idx + 2] = new_val.round() as u8;
                }
            }
        }

        // Convert Dithered RGBA pixmap to 1-bit monochrome buffer
        let pixmap_pixels = pixmap.pixels();
        for y in 0..self.target_height as usize {
            for x in 0..self.target_width as usize {
                let pixel_idx = y * self.target_width as usize + x;
                let p = pixmap_pixels[pixel_idx];
                // After dithering, we can simply check if the dithered grayscale value is "on" (non-zero)
                // as the error diffusion has already spread the intensity.
                let is_on = p.red() > 0; // Since R=G=B after dithering to grayscale

                if is_on {
                    let byte_idx = y * padded_width as usize + (x / 8); // Corrected byte index
                    let bit_idx = x % 8; // Bit index within the byte
                    buffer[byte_idx] |= 1 << (7 - bit_idx); // Set the pixel (MSB-first)
                }
            }
        }
        debug!("SVG rendered to buffer successfully.");
        Ok(())
    }


}
