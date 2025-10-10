/*
 *  draw.rs
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

use embedded_graphics::{
    mono_font::{
        MonoFont, MonoTextStyle, MonoTextStyleBuilder,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Arc, Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use embedded_graphics::pixelcolor::PixelColor;

use embedded_text::{
    alignment::{HorizontalAlignment, VerticalAlignment}, style::{TextBoxStyle, TextBoxStyleBuilder}, TextBox
};

// when we get to gray4 follow this pattern
#[allow(dead_code)]
pub fn draw_text_c<C, D>(
    target: &mut D,
    text: &str,
    x: i32,
    y: i32,
    font: &MonoFont,
    color: C,
) -> Result<(), D::Error>
where
    C: PixelColor,
    D: DrawTarget<Color = C> + OriginDimensions,
{
    Text::with_baseline(
        text,
        Point::new(x, y),
        MonoTextStyleBuilder::new().font(font).text_color(color).build(),
        Baseline::Top,
    )
    .draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_line<D>(
    target: &mut D,
    start: Point,
    end: Point,
    color: BinaryColor,
    width: u32
) -> Result<(), D::Error> 
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions
{
    let _ = Line::new(start, end)
        .into_styled(PrimitiveStyleBuilder::new().stroke_width(width).stroke_color(color).build())
        .draw(target)?;
    Ok(())
}

/// Clears a rectangular region of the target buffer to background color (BinaryColor::Off).
#[allow(dead_code)]
pub fn clear_region<D>(
    target: &mut D, 
    region: Rectangle
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    region
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(BinaryColor::Off)
                .build(),
        )
        .draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_text<D>(
    target: &mut D,
    text: &str,
    x: i32,
    y: i32,
    font: &MonoFont,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    Text::with_baseline(
        text,
        Point::new(x, y),
        MonoTextStyleBuilder::new()
            .font(font)
            .text_color(BinaryColor::On)
            .build(),
        Baseline::Top,
    )
    .draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_text_align<D>(
    target: &mut D,
    text: &str,
    top_left: Point,
    length: u32,
    align: HorizontalAlignment,
    font: &MonoFont,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    let h = font.character_size.height;
    let size = Size::new(length, h);
    let character_style = MonoTextStyle::new(font, BinaryColor::On);
    let textbox_style = TextBoxStyleBuilder::new()
        .alignment(align)
        .vertical_alignment(VerticalAlignment::Middle)
        .build();
    let label_rect = Rectangle::new(top_left, size);
    let label_box = TextBox::with_textbox_style(text, label_rect, character_style, textbox_style);
    label_box.draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_text_align_style<D>(
    target: &mut D,
    text: &str,
    top_left: Point,
    length: u32,
    style: TextBoxStyle,
    font: &MonoFont,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    let h = font.character_size.height;
    let size = Size::new(length, h);
    let character_style = MonoTextStyle::new(font, BinaryColor::On);
    let label_rect = Rectangle::new(top_left, size);
    let label_box = TextBox::with_textbox_style(text, label_rect, character_style, style);
    label_box.draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_text_region_align<D>(
    target: &mut D,
    text: &str,
    top_left: Point,
    size: Size,
    halign: HorizontalAlignment,
    valign: VerticalAlignment,
    font: &MonoFont,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    let character_style = MonoTextStyle::new(font, BinaryColor::On);
    let textbox_style = TextBoxStyleBuilder::new()
        .alignment(halign)
        .vertical_alignment(valign)
        .build();
    let label_rect = Rectangle::new(top_left, size);
    let label_box = TextBox::with_textbox_style(text, label_rect, character_style, textbox_style);
    label_box.draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_circle_from_center<D, C>(
    target: &mut D,
    center: Point,
    diameter: i32,
    style: PrimitiveStyle<C>,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = C> + OriginDimensions,
    C: PixelColor,
{
    assert!(diameter >= 0, "diameter must be non-negative");
    let r = diameter / 2;
    // Convert (center, diameter) -> (top_left, diameter)
    let top_left = Point::new(center.x - r, center.y - r);
    Circle::new(top_left, diameter as u32)
        .into_styled(style)
        .draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_circle<D>(
    target: &mut D,
    origin: Point,
    diameter: u32,
    color: BinaryColor,
    stroke_width: u32,
    fill_color: BinaryColor,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    // Draw the circle
    Circle::new(origin, diameter / 2)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .stroke_color(color)
                .stroke_width(stroke_width)
                .fill_color(fill_color)
                .build(),
        )
        .draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_arc<D>(
    target: &mut D,
    origin: Point,
    diameter: u32,
    angle_start: f32,
    angle_sweep: f32,
    color: BinaryColor,
    stroke_width: u32,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    Arc::new(origin, diameter, angle_start.deg(), angle_sweep.deg())
        .into_styled(PrimitiveStyle::with_stroke(color, stroke_width))
        .draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_rectangle<D>(
    target: &mut D,
    top_left: Point,
    w: u32,
    h: u32,
    fill: BinaryColor,
    border_width: Option<u32>,
    border_color: Option<BinaryColor>,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
{
    Rectangle::new(top_left, Size::new(w, h))
        .into_styled(
            PrimitiveStyleBuilder::new()
                .stroke_color(if border_color.is_some() {
                    border_color.unwrap()
                } else {
                    BinaryColor::Off
                })
                .stroke_width(if border_width.is_some() {
                    border_width.unwrap()
                } else {
                    0
                })
                .fill_color(fill)
                .build(),
        )
        .draw(target)?;
    Ok(())
}

#[allow(dead_code)]
pub fn draw_rect_with_style<D, C>(
    target: &mut D,
    rect: Rectangle,
    style: PrimitiveStyle<C>,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = C> + OriginDimensions,
    C: PixelColor,
{
    rect
        .into_styled(style)
        .draw(target)?;
    Ok(())
}