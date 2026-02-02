/*
 *  examples/ssd1306_basic.rs
 *
 *  Basic example of using SSD1306 display driver with LyMonS
 *
 *  This example demonstrates:
 *  - Initializing the SSD1306 driver
 *  - Drawing basic graphics
 *  - Using the display traits
 *
 *  Run with: cargo run --example ssd1306_basic --features driver-ssd1306
 */

// Note: Since LyMonS is a binary crate, examples would need to be part of the workspace
// or the project would need to be split into lib + bin.
// For now, this example is for documentation purposes.

// If converted to lib crate, uncomment these imports:
// use LyMonS::config::{DisplayConfig, DriverKind, BusConfig};
// use LyMonS::display::{DisplayDriverFactory, DisplayError};
// use embedded_graphics::prelude::*;
// use embedded_graphics::pixelcolor::BinaryColor;
// use embedded_graphics::primitives::{Circle, PrimitiveStyle, Rectangle, Line};
// use embedded_graphics::text::{Text, Alignment};
// use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};

// Example code (for documentation - requires lib crate to compile):
/*
fn main() -> Result<(), DisplayError> {
    println!("LyMonS SSD1306 Example");
    println!("======================\n");

    // Configure display
    let config = DisplayConfig {
        driver: Some(DriverKind::Ssd1306),
        bus: Some(BusConfig::I2c {
            bus: "/dev/i2c-1".to_string(),
            address: 0x3C,
            speed_hz: None,
        }),
        width: Some(128),
        height: Some(64),
        brightness: Some(200),
        ..Default::default()
    };

    println!("Creating SSD1306 driver...");
    let mut driver = DisplayDriverFactory::create_from_config(&config)?;

    println!("Initializing display...");
    driver.init()?;

    println!("Drawing graphics...");

    // Cast to DrawTarget for drawing
    let display = driver.as_mut();

    // Clear screen
    display.clear(BinaryColor::Off).unwrap();

    // Draw a border
    let border = Rectangle::new(Point::zero(), Size::new(128, 64))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1));
    border.draw(display).unwrap();

    // Draw a circle
    let circle = Circle::new(Point::new(64, 32), 20)
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1));
    circle.draw(display).unwrap();

    // Draw text
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    Text::with_alignment(
        "LyMonS",
        Point::new(64, 10),
        text_style,
        Alignment::Center,
    )
    .draw(display)
    .unwrap();

    // Draw a line
    let line = Line::new(Point::new(10, 50), Point::new(118, 50))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1));
    line.draw(display).unwrap();

    println!("Flushing to display...");
    driver.flush()?;

    println!("\n✓ Example complete! Check your display.");
    println!("Display should show:");
    println!("  - A border around the edge");
    println!("  - A circle in the center");
    println!("  - 'LyMonS' text at the top");
    println!("  - A horizontal line near the bottom");

    Ok(())
}
*/

fn main() {
    println!("This example is for documentation purposes.");
    println!("To use it, the project needs to be restructured with a library crate.");
    println!("See MIGRATION.md for details.");
}
