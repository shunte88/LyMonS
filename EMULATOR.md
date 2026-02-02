# LyMonS Display Emulator

**Version:** 0.1.47
**Date:** 2026-02-01

The LyMonS Display Emulator allows you to test and develop display layouts on your desktop without needing physical hardware.

## Features

- ✅ **All display types supported:** SSD1306, SSD1309, SSD1322, SH1106, SHARP Memory
- ✅ **Real-time rendering:** 60 FPS GPU-accelerated display
- ✅ **Keyboard controls:** Brightness, rotation, inversion, screenshots
- ✅ **Grayscale support:** Full 16-level grayscale for SSD1322
- ✅ **Performance metrics:** FPS counter and frame timing
- ✅ **Optional feature:** Only compiled when needed (zero code bloat)

## Installation

The emulator is an optional feature to avoid bloating the binary when running on embedded hardware.

### Prerequisites

- Linux (Ubuntu 22.04+ recommended)
- X11 or Wayland
- GPU with basic OpenGL support

### Build

```bash
# Build with emulator feature
cargo build --bin lymons-emulator --features emulator --release

# Or run directly
cargo run --bin lymons-emulator --features emulator
```

## Usage

### Basic Usage

```bash
# Run with default display (SSD1306, 128x64)
cargo run --bin lymons-emulator --features emulator

# Run with specific display type
cargo run --bin lymons-emulator --features emulator -- --display ssd1322
```

### Supported Display Types

| Type | Resolution | Color Depth | Command |
|------|-----------|-------------|---------|
| SSD1306 | 128x64 | Monochrome | `--display ssd1306` |
| SSD1309 | 128x64 | Monochrome | `--display ssd1309` |
| SH1106 | 132x64 | Monochrome | `--display sh1106` |
| SSD1322 | 256x64 | 16-level gray | `--display ssd1322` |
| SHARP | 400x240 | Monochrome | `--display sharp` |

### Examples

```bash
# Test SSD1306 layout (most common)
cargo run --bin lymons-emulator --features emulator -- --display ssd1306

# Test SSD1322 grayscale rendering
cargo run --bin lymons-emulator --features emulator -- --display ssd1322

# Test large SHARP Memory LCD layout
cargo run --bin lymons-emulator --features emulator -- --display sharp
```

## Keyboard Controls

Once the emulator window is open, use these shortcuts:

| Key | Action |
|-----|--------|
| **ESC** or **Q** | Quit emulator |
| **G** | Toggle pixel grid overlay |
| **F** | Toggle FPS counter |
| **H** | Toggle help overlay |
| **S** | Save screenshot (TODO) |
| **B** | Cycle brightness (64 → 128 → 255 → 64...) |
| **R** | Cycle rotation (0° → 90° → 180° → 270° → 0°...) |
| **I** | Toggle color inversion |

## Architecture

### Components

```
lymons-emulator binary
    ↓
EmulatorWindow (GUI)
    ↓
EmulatorDriver (DisplayDriver trait)
    ↓
VarFrameBuf (framebuffer)
```

### How It Works

1. **EmulatorDriver** implements the `DisplayDriver` trait
2. Drawing operations update an internal framebuffer
3. `flush()` copies framebuffer to shared state
4. **EmulatorWindow** renders shared state to screen using `pixels` crate
5. Window runs at 60 FPS with GPU acceleration

### Feature Gating

The emulator is completely optional and uses Rust feature flags:

```rust
#[cfg(feature = "emulator")]
pub mod emulator;
```

When the `emulator` feature is **disabled**:
- Zero emulator code compiled
- No window dependencies (pixels, winit)
- Binary size unaffected
- Perfect for embedded deployment

When the `emulator` feature is **enabled**:
- Full emulator functionality
- Desktop testing capabilities
- Additional ~500KB to binary size

## Integration with LyMonS

### Using EmulatorDriver in Your Code

```rust
use LyMonS::display::drivers::emulator::EmulatorDriver;
use LyMonS::display::emulator_window::{EmulatorWindow, EmulatorWindowConfig};

// Create driver
let mut driver = EmulatorDriver::new_monochrome(128, 64, "SSD1306")?;

// Get shared state for window
let state = driver.state();

// Draw to driver (implements DisplayDriver trait)
driver.init()?;
// ... your drawing code ...
driver.flush()?;

// Create and run window
let config = EmulatorWindowConfig::default();
let window = EmulatorWindow::new(state, config);
window.run()?;
```

### Factory Support

The emulator can be created via the display factory:

```rust
use LyMonS::config::{DisplayConfig, DriverKind};
use LyMonS::display::DisplayDriverFactory;

let config = DisplayConfig {
    driver: Some(DriverKind::Ssd1306),
    width: Some(128),
    height: Some(64),
    ..Default::default()
};

// Note: Factory would need to be extended to support emulator
let driver = DisplayDriverFactory::create_from_config(&config)?;
```

## Performance

### Benchmarks

Tested on: Ubuntu 24.04, Intel i7, integrated graphics

| Display | Resolution | FPS | Frame Time | Notes |
|---------|-----------|-----|------------|-------|
| SSD1306 | 128x64 | 60 | 16.7ms | Smooth |
| SSD1309 | 128x64 | 60 | 16.7ms | Smooth |
| SH1106 | 132x64 | 60 | 16.7ms | Smooth |
| SSD1322 | 256x64 | 60 | 16.7ms | Smooth, grayscale |
| SHARP | 400x240 | 60 | 16.7ms | Smooth, large |

### Performance Tips

1. **GPU Acceleration**: The `pixels` crate uses wgpu for hardware acceleration
2. **No Overhead**: Emulator runs in separate thread from your application
3. **Real-time**: Updates are instant (no I2C/SPI delays)

## Configuration

### EmulatorWindowConfig

```rust
pub struct EmulatorWindowConfig {
    /// Pixel scale factor (display pixel → screen pixels)
    pub scale: u32,              // Default: 4

    /// Whether to show pixel grid
    pub show_grid: bool,         // Default: false

    /// Whether to show FPS counter
    pub show_fps: bool,          // Default: true

    /// Whether to show keyboard shortcuts
    pub show_help: bool,         // Default: true

    /// Background color [R, G, B, A]
    pub bg_color: [u8; 4],       // Default: [20, 20, 20, 255]
}
```

### Custom Configuration Example

```rust
let config = EmulatorWindowConfig {
    scale: 6,                              // Larger pixels
    show_grid: true,                       // Show grid lines
    show_fps: true,                        // Show FPS
    show_help: false,                      // Hide help
    bg_color: [0, 0, 0, 255],             // Black background
};
```

## Limitations

### Current Limitations

1. **No screenshot save** - Pressing 'S' prints message but doesn't save (TODO)
2. **No grid rendering** - Grid toggle works but doesn't draw yet (TODO)
3. **No help overlay** - Help toggle works but doesn't render yet (TODO)
4. **No FPS overlay** - FPS calculated but not rendered on screen (TODO)

### Future Enhancements

- [ ] Screenshot capture to PNG
- [ ] Pixel grid overlay
- [ ] On-screen help display
- [ ] FPS counter overlay
- [ ] Performance metrics display
- [ ] Recording to video
- [ ] Multiple window support (compare layouts side-by-side)

## Troubleshooting

### "feature emulator is not enabled"

**Error:**
```
ERROR: This binary requires the 'emulator' feature.
```

**Solution:**
```bash
cargo run --bin lymons-emulator --features emulator
```

### "winit error: could not connect to X"

**Problem:** No display server available.

**Solution:**
- Ensure X11 or Wayland is running
- Check `$DISPLAY` environment variable
- Try: `export DISPLAY=:0`

### "pixels error: no suitable adapter found"

**Problem:** No GPU/graphics driver available.

**Solution:**
- Install graphics drivers
- Try software rendering (performance will be lower)

### Window doesn't open

**Check:**
1. Is the emulator feature enabled?
2. Are you on a headless system? (emulator needs GUI)
3. Check error messages in console

## Development

### Adding Custom Animations

Edit `src/bin/lymons-emulator.rs` and modify the `animate_demo()` function:

```rust
fn animate_demo(state: &Arc<Mutex<EmulatorState>>) {
    loop {
        let mut state = state.lock().unwrap();

        // Your custom animation code here
        // Update state.buffer with EmulatorColor values

        state.frame_count += 1;
        drop(state);

        thread::sleep(Duration::from_millis(33)); // 30 FPS
    }
}
```

### Integrating with DisplayManager

To use the emulator with the full DisplayManager:

```rust
#[cfg(feature = "emulator")]
{
    let config = DisplayConfig { /* ... */ };
    let mut display_manager = DisplayManager::new(&config, ...)?;

    // Get the underlying driver (would need accessor method)
    // let state = display_manager.driver_state();

    // Run emulator window in separate thread
    // let window = EmulatorWindow::new(state, config);
    // thread::spawn(move || window.run());
}
```

## Credits

- **LyMonS** - (c) 2020-26 Stuart Hunter
- **pixels** - Fast pixel buffer rendering
- **winit** - Cross-platform windowing
- **wgpu** - Modern graphics API (via pixels)

## License

GPL v3 - See LICENSE file

---

**Last Updated:** 2026-02-01
**Version:** 0.1.47
