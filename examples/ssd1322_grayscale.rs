/*
 *  examples/ssd1322_grayscale.rs
 *
 *  Grayscale example for SSD1322 display driver
 *
 *  This example demonstrates:
 *  - Initializing the SSD1322 driver (256x64, 4-bit grayscale)
 *  - Drawing with different gray levels
 *  - Using SPI interface
 *
 *  Run with: cargo run --example ssd1322_grayscale --features driver-ssd1322
 */

// Note: Since LyMonS is a binary crate, examples would need to be part of the workspace
// or the project would need to be split into lib + bin.
// For now, this example is for documentation purposes.

// If converted to lib crate, uncomment these imports:
// use LyMonS::config::{DisplayConfig, DriverKind, BusConfig};
// use LyMonS::display::{DisplayDriverFactory, DisplayError};
// use embedded_graphics::prelude::*;
// use embedded_graphics::pixelcolor::Gray4;
// use embedded_graphics::primitives::{Rectangle, PrimitiveStyle, PrimitiveStyleBuilder};
// use embedded_graphics::text::{Text, Alignment};
// use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};

// Example code (for documentation - requires lib crate to compile):
/*
fn main() -> Result<(), DisplayError> {
    println!("LyMonS SSD1322 Grayscale Example");
    println!("================================\n");

    // Configure display
    let config = DisplayConfig {
        driver: Some(DriverKind::Ssd1322),
        bus: Some(BusConfig::Spi {
            bus: "/dev/spidev0.0".to_string(),
            speed_hz: Some(10_000_000), // 10 MHz
            dc_pin: 24,
            rst_pin: 25,
            cs_pin: None,
        }),
        width: Some(256),
        height: Some(64),
        brightness: Some(255),
        ..Default::default()
    };

    println!("Creating SSD1322 driver...");
    let mut driver = DisplayDriverFactory::create_from_config(&config)?;

    println!("Initializing display...");
    driver.init()?;

    println!("Drawing grayscale graphics...");

    // Cast to DrawTarget for drawing
    let display = driver.as_mut();

    // Clear screen to black
    display.clear(Gray4::new(0)).unwrap();

    // Draw 16 rectangles showing all gray levels (0-15)
    let rect_width = 256 / 16;
    for i in 0..16 {
        let gray_level = Gray4::new(i);
        let x = (i as i32) * (rect_width as i32);

        let rect = Rectangle::new(
            Point::new(x, 0),
            Size::new(rect_width, 32),
        )
        .into_styled(PrimitiveStyle::with_fill(gray_level));

        rect.draw(display).unwrap();
    }

    // Draw text with different gray levels
    let text_y = 40;

    // White text
    let text_style_white = MonoTextStyle::new(&FONT_6X10, Gray4::new(15));
    Text::with_alignment(
        "LyMonS",
        Point::new(32, text_y),
        text_style_white,
        Alignment::Left,
    )
    .draw(display)
    .unwrap();

    // Gray text
    let text_style_gray = MonoTextStyle::new(&FONT_6X10, Gray4::new(8));
    Text::with_alignment(
        "Grayscale",
        Point::new(100, text_y),
        text_style_gray,
        Alignment::Left,
    )
    .draw(display)
    .unwrap();

    // Light gray text
    let text_style_light = MonoTextStyle::new(&FONT_6X10, Gray4::new(4));
    Text::with_alignment(
        "Display",
        Point::new(180, text_y),
        text_style_light,
        Alignment::Left,
    )
    .draw(display)
    .unwrap();

    // Draw a gradient bar at the bottom
    for x in 0..256 {
        let gray_level = Gray4::new((x * 15 / 255) as u8);
        let line_style = PrimitiveStyleBuilder::new()
            .stroke_color(gray_level)
            .stroke_width(1)
            .build();

        let line = embedded_graphics::primitives::Line::new(
            Point::new(x as i32, 55),
            Point::new(x as i32, 63),
        )
        .into_styled(line_style);

        line.draw(display).unwrap();
    }

    println!("Flushing to display...");
    driver.flush()?;

    println!("\n✓ Example complete! Check your display.");
    println!("Display should show:");
    println!("  - 16 rectangles showing gray levels 0-15");
    println!("  - Text in different gray levels");
    println!("  - A smooth gradient bar at the bottom");
    println!("\nNote: SSD1322 supports 16 levels of gray (4-bit)");

    Ok(())
}
*/

fn main() {
    println!("This example is for documentation purposes.");
    println!("To use it, the project needs to be restructured with a library crate.");
    println!("See MIGRATION.md for details.");
}
