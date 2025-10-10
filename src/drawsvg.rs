/*
 *  drawsvg.rs
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
use log::{warn};
use std::fmt;
use tokio::fs;
use std::fs as fs_std;
use embedded_graphics::{
    image::{Image, ImageRaw},
    pixelcolor::BinaryColor, 
    prelude::*, 
};

use crate::svgimage::SvgImageRenderer;

/// Errors that can happen while placing an SVG on a DrawTarget.
#[derive(Debug)]
pub enum PutSvgError<DE> {
    Io(std::io::Error),
    Svg(Box<dyn std::error::Error + Send + Sync>),
    Draw(DE),
}
impl<DE: fmt::Debug> fmt::Display for PutSvgError<DE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PutSvgError::Io(e)   => write!(f, "I/O error: {e}"),
            PutSvgError::Svg(e)  => write!(f, "SVG error: {e}"),
            PutSvgError::Draw(e) => write!(f, "draw error: {e:?}"),
        }
    }
}

impl<DE: fmt::Debug> std::error::Error for PutSvgError<DE> {}

/// Direct SVG rendering with scale (no SVG dynamics) -- getter.
pub async fn get_svg_async (
    path: &str,
    width: u32,
    height: u32,
    buffer: &mut Vec<u8>,
) -> Result<(), PutSvgError<std::io::Error>>
{
    if fs::metadata(path).await.is_ok() {

        let data = fs::read_to_string(path).await.map_err(PutSvgError::Io)?;
        println!("{data}");
        let buffer_size = height as usize * ((width + 7) / 8) as usize;
        *buffer = vec![0u8; buffer_size];

        let svg_renderer = SvgImageRenderer::new(&data, width, height)
            .map_err(|e| PutSvgError::Svg(Box::new(e)))?;
        svg_renderer
            .render_to_buffer(buffer)
            .map_err(|e| PutSvgError::Svg(Box::new(e)))?;

    }else{
        warn!("{path} doesn't exist!");
    }
    Ok(())

}

pub fn get_svg (
    path: &str,
    width: u32,
    height: u32,
    buffer: &mut Vec<u8>,
) -> Result<(), PutSvgError<std::io::Error>>
{
    if fs_std::metadata(path).is_ok() {

        let data = fs_std::read_to_string(path).map_err(PutSvgError::Io)?;
        let buffer_size = height as usize * ((width + 7) / 8) as usize;
        *buffer = vec![0u8; buffer_size];

        let svg_renderer = SvgImageRenderer::new(&data, width, height)
            .map_err(|e| PutSvgError::Svg(Box::new(e)))?;
        svg_renderer
            .render_to_buffer(buffer)
            .map_err(|e| PutSvgError::Svg(Box::new(e)))?;

    }else{
        warn!("{path} doesn't exist!");
    }
    Ok(())

}

/// Direct SVG rendering with scale (no SVG dynamics).
pub fn put_svg<D>(
    target: &mut D,
    path: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), PutSvgError<D::Error>>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    if fs_std::metadata(path).is_ok() {

        let data = fs_std::read_to_string(path).map_err(PutSvgError::Io)?;
        let buffer_size = height as usize * ((width + 7) / 8) as usize;
        let mut buffer = vec![0u8; buffer_size];

        let svg_renderer = SvgImageRenderer::new(&data, width, height)
            .map_err(|e| PutSvgError::Svg(Box::new(e)))?;
        svg_renderer
            .render_to_buffer(&mut buffer)
            .map_err(|e| PutSvgError::Svg(Box::new(e)))?;

        // Blit to target
        let raw = ImageRaw::<BinaryColor>::new(&buffer, width);
        Image::new(&raw, Point::new(x, y))
            .draw(target)
            .map_err(PutSvgError::Draw)?;
    }else{
        warn!("{path} doesn't exist!");
    }
    Ok(())

}

/// Direct SVG rendering with scale (no SVG dynamics) async.
pub async fn put_svg_async<D>(
    target: &mut D,
    path: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), PutSvgError<D::Error>>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    if fs::metadata(path).await.is_ok() {

        let data = fs::read_to_string(path).await.map_err(PutSvgError::Io)?;
        let buffer_size = height as usize * ((width + 7) / 8) as usize;
        let mut buffer = vec![0u8; buffer_size];

        let svg_renderer = SvgImageRenderer::new(&data, width, height)
            .map_err(|e| PutSvgError::Svg(Box::new(e)))?;
        svg_renderer
            .render_to_buffer(&mut buffer)
            .map_err(|e| PutSvgError::Svg(Box::new(e)))?;

        // Blit to target
        let raw = ImageRaw::<BinaryColor>::new(&buffer, width);
        Image::new(&raw, Point::new(x, y))
            .draw(target)
            .map_err(PutSvgError::Draw)?;
    }else{
        warn!("{path} doesn't exist!");
    }
    Ok(())
}
