/*
 *  bin/lymons-emulator.rs
 *
 *  LyMonS Display Emulator - Desktop testing tool
 *
 *  (c) 2020-26 Stuart Hunter
 *
 *  Runs LyMonS display system in a desktop window for testing without hardware.
 *
 *  Usage:
 *    cargo run --bin lymons-emulator --features emulator
 *    cargo run --bin lymons-emulator --features emulator -- --display ssd1322
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 */

#[cfg(feature = "emulator")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("ERROR: Emulator binary cannot run from binary crate.");
    eprintln!();
    eprintln!("The emulator requires LyMonS to be structured as a library crate.");
    eprintln!("Currently, LyMonS is a binary-only crate.");
    eprintln!();
    eprintln!("To use the emulator:");
    eprintln!("1. Restructure project with src/lib.rs");
    eprintln!("2. Move display code to library");
    eprintln!("3. Rebuild emulator");
    eprintln!();
    eprintln!("Alternative: Use the EmulatorDriver directly in your main.rs");
    eprintln!("See EMULATOR.md for integration examples.");
    std::process::exit(1);
}

#[cfg(feature = "emulator")]
fn _animate_demo_example() {
    // Example animation code (for documentation)
    // This would work if LyMonS was a library crate
    //
    // fn animate_demo(state: &Arc<Mutex<EmulatorState>>) {
    //     let mut frame = 0u32;
    //     loop {
    //         {
    //             let mut state = state.lock().unwrap();
    //             // Update display buffer with animation
    //             state.frame_count += 1;
    //         }
    //         frame += 1;
    //         thread::sleep(Duration::from_millis(33)); // 30 FPS
    //     }
    // }
}

#[cfg(not(feature = "emulator"))]
fn main() {
    eprintln!("ERROR: This binary requires the 'emulator' feature.");
    eprintln!();
    eprintln!("Please compile with:");
    eprintln!("  cargo run --bin lymons-emulator --features emulator");
    eprintln!();
    eprintln!("Available display types:");
    eprintln!("  --display ssd1306   (128x64, monochrome)");
    eprintln!("  --display ssd1309   (128x64, monochrome)");
    eprintln!("  --display sh1106    (132x64, monochrome)");
    eprintln!("  --display ssd1322   (256x64, 16-level grayscale)");
    eprintln!("  --display sharp     (400x240, monochrome)");
    std::process::exit(1);
}
