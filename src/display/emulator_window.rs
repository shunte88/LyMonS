/*
 *  display/emulator_window.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Emulator window management
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

#[cfg(feature = "emulator")]
use pixels::{Pixels, SurfaceTexture};
#[cfg(feature = "emulator")]
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
#[cfg(feature = "emulator")]
use winit_input_helper::WinitInputHelper;

#[cfg(feature = "emulator")]
use crate::display::drivers::emulator::{EmulatorState, EmulatorColor};
#[cfg(feature = "emulator")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "emulator")]
use std::time::Instant;

/// Emulator window configuration
#[cfg(feature = "emulator")]
#[derive(Debug, Clone)]
pub struct EmulatorWindowConfig {
    /// Pixel scale factor (display pixel → screen pixels)
    pub scale: u32,

    /// Whether to show pixel grid
    pub show_grid: bool,

    /// Whether to show FPS counter
    pub show_fps: bool,

    /// Whether to show keyboard shortcuts
    pub show_help: bool,

    /// Background color [R, G, B, A]
    pub bg_color: [u8; 4],
}

#[cfg(feature = "emulator")]
impl Default for EmulatorWindowConfig {
    fn default() -> Self {
        Self {
            scale: 4,
            show_grid: false,
            show_fps: true,
            show_help: true,
            bg_color: [20, 20, 20, 255],
        }
    }
}

/// Emulator window manager
#[cfg(feature = "emulator")]
pub struct EmulatorWindow {
    state: Arc<Mutex<EmulatorState>>,
    config: EmulatorWindowConfig,
    fps_counter: FpsCounter,
}

#[cfg(feature = "emulator")]
struct FpsCounter {
    last_update: Instant,
    frame_count: u32,
    current_fps: f32,
}

#[cfg(feature = "emulator")]
impl FpsCounter {
    fn new() -> Self {
        Self {
            last_update: Instant::now(),
            frame_count: 0,
            current_fps: 0.0,
        }
    }

    fn tick(&mut self) -> f32 {
        self.frame_count += 1;
        let elapsed = self.last_update.elapsed();

        if elapsed.as_secs_f32() >= 1.0 {
            self.current_fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.frame_count = 0;
            self.last_update = Instant::now();
        }

        self.current_fps
    }
}

#[cfg(feature = "emulator")]
impl EmulatorWindow {
    /// Create a new emulator window
    pub fn new(state: Arc<Mutex<EmulatorState>>, config: EmulatorWindowConfig) -> Self {
        Self {
            state,
            config,
            fps_counter: FpsCounter::new(),
        }
    }

    /// Run the emulator window event loop
    pub fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let state_lock = self.state.lock().unwrap();
        let width = state_lock.width;
        let height = state_lock.height;
        let display_type = state_lock.display_type.clone();
        drop(state_lock);

        let event_loop = EventLoop::new();
        let mut input = WinitInputHelper::new();

        let window_width = width * self.config.scale;
        let window_height = height * self.config.scale;

        // Use PhysicalSize to avoid Wayland DPI scaling issues
        use winit::dpi::PhysicalSize;
        let window = WindowBuilder::new()
            .with_title(format!("LyMonS Emulator - {}", display_type))
            .with_inner_size(PhysicalSize::new(window_width, window_height))
            .with_resizable(false)
            .with_decorations(false)  // Disable decorations for Wayland compatibility
            // Note: with_always_on_top not available in winit 0.28
            // User can set always-on-top via window manager if needed
            .build(&event_loop)?;

        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let mut pixels = Pixels::new(width, height, surface_texture)?;

        println!("═══════════════════════════════════════════════════");
        println!("  LyMonS Display Emulator");
        println!("═══════════════════════════════════════════════════");
        println!("  Display: {} ({}x{})", display_type, width, height);
        println!("  Scale: {}x", self.config.scale);
        println!();
        println!("  Keyboard Shortcuts:");
        println!("  ─────────────────────────────────────────────────");
        println!("    ESC / Q   - Quit");
        println!("    W         - Lock to weather mode");
        println!("    C         - Lock to clock mode");
        println!("    A         - Return to automatic mode");
        println!("    E         - Cycle easter egg animations");
        println!("    V         - Cycle visualizations");
        println!("    G         - Toggle pixel grid");
        println!("    F         - Toggle FPS counter");
        println!("    H         - Toggle help overlay");
        println!("    S         - Save screenshot");
        println!("    B         - Cycle brightness");
        println!("    R         - Cycle rotation");
        println!("    I         - Toggle invert");
        println!("═══════════════════════════════════════════════════");

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            if let Event::RedrawRequested(_) = event {
                self.render(pixels.frame_mut());

                if let Err(err) = pixels.render() {
                    eprintln!("pixels.render() failed: {}", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                let fps = self.fps_counter.tick();
                if fps > 0.0 {
                    window.set_title(&format!(
                        "LyMonS Emulator - {} ({}x{}) - {:.1} FPS",
                        display_type, width, height, fps
                    ));
                }
            }

            if input.update(&event) {
                // Close events
                if input.key_pressed(VirtualKeyCode::Escape) || input.key_pressed(VirtualKeyCode::Q) {
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                // Toggle grid
                if input.key_pressed(VirtualKeyCode::G) {
                    self.config.show_grid = !self.config.show_grid;
                    println!("Pixel grid: {}", if self.config.show_grid { "ON" } else { "OFF" });
                }

                // Toggle FPS
                if input.key_pressed(VirtualKeyCode::F) {
                    self.config.show_fps = !self.config.show_fps;
                    println!("FPS counter: {}", if self.config.show_fps { "ON" } else { "OFF" });
                }

                // Toggle help
                if input.key_pressed(VirtualKeyCode::H) {
                    self.config.show_help = !self.config.show_help;
                    println!("Help overlay: {}", if self.config.show_help { "ON" } else { "OFF" });
                }

                // Save screenshot
                if input.key_pressed(VirtualKeyCode::S) {
                    println!("Screenshot saved (TODO: implement)");
                }

                // Cycle brightness
                if input.key_pressed(VirtualKeyCode::B) {
                    let mut state = self.state.lock().unwrap();
                    state.brightness = match state.brightness {
                        0..=63 => 128,
                        64..=191 => 255,
                        _ => 64,
                    };
                    println!("Brightness: {}", state.brightness);
                }

                // Cycle rotation
                if input.key_pressed(VirtualKeyCode::R) {
                    let mut state = self.state.lock().unwrap();
                    state.rotation = match state.rotation {
                        0 => 90,
                        90 => 180,
                        180 => 270,
                        _ => 0,
                    };
                    println!("Rotation: {}°", state.rotation);
                }

                // Toggle invert
                if input.key_pressed(VirtualKeyCode::I) {
                    let mut state = self.state.lock().unwrap();
                    state.inverted = !state.inverted;
                    println!("Inverted: {}", state.inverted);
                }

                // Trigger weather mode (manual override)
                // Toggle between WeatherCurrent and WeatherForecast if already in weather mode
                if input.key_pressed(VirtualKeyCode::W) {
                    let mut state = self.state.lock().unwrap();

                    // Toggle based on what's currently showing
                    let new_mode = match state.current_display_mode {
                        crate::display::DisplayMode::WeatherCurrent => {
                            println!("Switching to weather forecast");
                            crate::display::DisplayMode::WeatherForecast
                        }
                        crate::display::DisplayMode::WeatherForecast => {
                            println!("Switching to weather current");
                            crate::display::DisplayMode::WeatherCurrent
                        }
                        _ => {
                            println!("Weather current mode triggered (manual override active)");
                            crate::display::DisplayMode::WeatherCurrent
                        }
                    };

                    state.requested_mode = Some(new_mode);
                    state.manual_mode_override = true;
                }

                // Trigger clock mode (manual override)
                if input.key_pressed(VirtualKeyCode::C) {
                    let mut state = self.state.lock().unwrap();
                    state.requested_mode = Some(crate::display::DisplayMode::Clock);
                    state.manual_mode_override = true;
                    println!("Clock mode triggered (manual override active)");
                }

                // Return to automatic mode
                if input.key_pressed(VirtualKeyCode::A) {
                    let mut state = self.state.lock().unwrap();
                    state.manual_mode_override = false;
                    state.requested_mode = None;
                    println!("Automatic mode switching re-enabled");
                }

                // Cycle through easter eggs (requires track playing)
                if input.key_pressed(VirtualKeyCode::E) {
                    let mut state = self.state.lock().unwrap();
                    state.cycle_easter_egg = true;
                    state.manual_mode_override = true;
                    state.requested_mode = Some(crate::display::DisplayMode::EasterEggs);
                    println!("Cycling to next easter egg animation (manual mode locked)");
                }

                // Cycle through visualizations (requires track playing)
                if input.key_pressed(VirtualKeyCode::V) {
                    let mut state = self.state.lock().unwrap();
                    state.cycle_visualization = true;
                    state.manual_mode_override = true;
                    state.requested_mode = Some(crate::display::DisplayMode::Visualizer);
                    println!("Cycling to next visualization (manual mode locked)");
                }
            }

            // Request redraw on every loop iteration (not just on input)
            // This ensures the display updates continuously as frames are rendered
            window.request_redraw();
        });
    }

    fn render(&self, frame: &mut [u8]) {
        let state = self.state.lock().unwrap();

        // Apply brightness
        let brightness_factor = state.brightness as f32 / 255.0;

        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            if i < state.buffer.len() {
                let mut rgba = state.buffer[i].to_rgba();

                // Apply brightness
                rgba[0] = (rgba[0] as f32 * brightness_factor) as u8;
                rgba[1] = (rgba[1] as f32 * brightness_factor) as u8;
                rgba[2] = (rgba[2] as f32 * brightness_factor) as u8;

                // Apply invert
                if state.inverted {
                    rgba[0] = 255 - rgba[0];
                    rgba[1] = 255 - rgba[1];
                    rgba[2] = 255 - rgba[2];
                }

                pixel.copy_from_slice(&rgba);
            } else {
                // Background color
                pixel.copy_from_slice(&self.config.bg_color);
            }
        }

        // TODO: Draw grid if enabled
        // TODO: Draw FPS counter if enabled
        // TODO: Draw help overlay if enabled
    }
}
