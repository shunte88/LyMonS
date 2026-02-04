# LyMonS Refactoring Project - Current State

## ⚠️ IMPORTANT NOTES

- **LMS Player Name**: `mythy` (use this for testing and development)
- **Weather API Key**: `-W "pm9iWRnHWowgH7cNOGeWmtgFYa6LLelk,F,,42.36141470379943,-71.1040784239152"`
- **Font Standard**: Always use `iso_8859_13` fonts for accented character support, never `ascii`
- **Display Test Modes**: Use emulator keyboard shortcuts:
  - `W` key - Toggle weather mode (Current ⇄ Forecast)
  - `C` key - Lock to clock mode
  - `A` key - Return to automatic mode switching
- **Weather Data Source**: Tomorrow.io API via background channel (not LMS)
- **Weather Icons**: Mono SVG files in `./assets/mono/` (BinaryColor and Gray4 displays)
- **Config File**: `config.yaml` with `display.emulated: true` for emulator mode

---

**Location**: `/data2/refactor/LyMonS/REFACTORING_STATE.md`
**Last Updated**: 2026-02-04 08:45 UTC
**Current Phase**: Weather System Complete ✓ (Current + Forecast + Wide Display + Gray4 SVG) - SSD1309 Validated
**Next Phase**: Moon Phase Integration, Wide Display Hardware Testing

> **Note**: This file is maintained at the project root and persists across reboots.
> It serves as the primary reference for project state and resumption context.

---

## Project Overview

Refactoring LyMonS (Logitech Media Server Monitor) from monolithic `display_old.rs` to component-based architecture using a layout manager pattern.

**Key Goals**:
- Migrate from single large file to modular component system
- Zero allocations in render loop for embedded performance
- Maintain feature parity with original implementation
- Support multiple display drivers (SSD1306, SSD1309, SSD1322, SH1106)

---

## LATEST: Complete Weather System with Toggle (2026-02-04)

### Weather Current Display ✓

Implemented full weather current display with:
- **Weather icon**: SVG rendering from `./assets/mono/` (30x30 icons)
- **Weather glyphs**: 12x12 pixel glyphs for temp, humidity, wind, precipitation
- **All text fields**: Conditions (centered), temperature, humidity, wind, precipitation
- **Field alignment**: Conditions text centered using field alignment property
- **Both display types**: Monochrome (BinaryColor) and Grayscale (Gray4)

### Weather Forecast Display ✓

Implemented 3-day weather forecast with:
- **3-column layout**: Day 1, Day 2, Day 3 side-by-side
- **SVG icons**: 30x30 weather icons for each day (monochrome only)
- **Day names**: Abbreviated day names (Mon, Tue, Wed) - centered with border
- **Temperature range**: Min|Max format (e.g., "45°|62°") - centered
- **Precipitation**: Probability percentage (e.g., "20%") - centered
- **Bordered data boxes**: 1px border around temp/precip data
- **Both display types**: Full rendering for Mono and Gray4 (including SVG icons)

**Files Modified**:
- `src/display/manager.rs` - Weather rendering methods (~lines 1285-1900)
  - `render_weather_current()` - Weather current with SVG icons and glyphs
  - `render_weather_forecast()` - 3-day forecast with layout-based rendering
  - `render_weather_fields_mono()` - Monochrome current rendering with SVG
  - `render_weather_fields_gray4()` - Grayscale current rendering
  - `render_forecast_fields_mono()` - Monochrome forecast with 3 columns
  - `render_forecast_fields_gray4()` - Grayscale forecast (SVG pending)
  - `draw_weather_glyph()` - 12x12 weather glyph pixel-by-pixel rendering
  - Helper methods: `render_forecast_icon_mono()`, `render_forecast_icon_gray4()`, `render_centered_text_mono/gray4()` (with border support), `render_box_mono/gray4()`
  - `update_emulator_current_mode()` - Sync current mode to emulator state

- `src/display/emulator_window.rs` - Keyboard shortcut handlers (~lines 257-279)
  - **W key**: Toggle WeatherCurrent ⇄ WeatherForecast (based on current_display_mode)
  - **C key**: Lock to Clock mode
  - **A key**: Return to automatic mode

- `src/display/drivers/emulator.rs` - EmulatorState extended (~line 95-101)
  - Added `requested_mode: Option<DisplayMode>` - Keyboard request
  - Added `manual_mode_override: bool` - Persistent mode locking
  - Added `current_display_mode: DisplayMode` - Track what's actually showing (for toggle)

- `src/main.rs` - Mode control logic (~lines 175-201, 889-905)
  - Check manual override before running mode controller
  - Read emulated flag from config.yaml
  - Call `update_emulator_current_mode()` after mode changes

### Manual Mode Override System ✓

Keyboard-triggered modes now persist (no automatic switching):
1. Press W/C to lock into a specific mode
2. `manual_mode_override = true` prevents mode controller from running
3. Press A to re-enable automatic mode switching
4. Fixes "blink and you've missed it" issue

### W Key Toggle Mechanism ✓

Smooth toggling between weather displays:
1. **EmulatorState tracks current mode**: `current_display_mode` field stores what's actually displaying
2. **Keyboard handler checks current mode**: W key reads `current_display_mode` to determine toggle direction
3. **Main loop updates tracking**: After each mode change, calls `update_emulator_current_mode()`
4. **No debouncing issues**: State remains consistent across key presses

**Flow**:
- Press W while showing Clock → WeatherCurrent
- Press W while showing WeatherCurrent → WeatherForecast
- Press W while showing WeatherForecast → WeatherCurrent
- Cycle repeats smoothly

### Config File Support ✓

Emulator mode can now be enabled via config.yaml:
```yaml
display:
  driver: ssd1309
  width: 128
  height: 64
  emulated: true
  brightness: 200
```

Usage: `./target/release/LyMonS --name mythy -W "<weather-config>" --config config.yaml`

### SVG Weather Icon Rendering ✓

- Uses `crate::drawsvg::get_svg()` to render SVG to BinaryColor buffer
- Uses `crate::drawsvg::get_svg_gray4_binary()` for Gray4 displays
- Renders SVG as `ImageRaw<BinaryColor>` or `ImageRaw<Gray4>` and draws to display
- Path construction: `./assets/mono/{weather_data.weather_code.svg}`
- Works on SSD1309 (128x64 monochrome) and SSD1322 (256x64 grayscale) - current and forecast
- Gray4 uses binary threshold approach (RGB >= 128 = white, < 128 = black)

### Wide Display Support ✓ (2026-02-04)

Implemented conditional rendering for wide displays (width > 128):

**Weather Current** - Added fields for wide displays:
- Sunrise time: "Sunrise: HH:MM AM/PM" (right side)
- Sunset time: "Sunset: HH:MM AM/PM" (right side)
- Moon phase: "Moon: HH:MM AM/PM" (moonrise time, right side)
- Text-only display (no glyphs found for sunrise/sunset)
- Moon phase glyphs exist (8 phases, 30x30px) but not yet integrated

**Weather Forecast** - Added 6-day forecast for wide displays:
- Narrow (≤128px): Days 1-3 visible
- Wide (>128px): Days 1-6 visible
- Days 4-6 rendered conditionally based on width check
- Same column layout pattern extended to days 4-6

**Weather API Updates**:
- Fetch extended to 8 days (`nowPlus8d`)
- Daily start time changed to 0 (midnight)
- Forecast data array sized for 7 days (0-6)

**Files Modified**:
- `src/weather.rs` (lines 195, 503-548) - Fetch 8 days, process 7
- `src/display/layout_manager.rs` (lines 182-520) - Conditional fields for wide displays
- `src/display/manager.rs` (lines 1337-1373, 1478-1496, 1708-1827) - Render sunrise/sunset/moon, days 4-6

### Gray4 SVG Rendering ✓ (2026-02-04)

Implemented SVG rendering for Gray4 displays using binary threshold approach:

**Binary Threshold Method**:
- Pixel walk approach: Calculate luminance for each pixel
- RGB >= 128 → White (Gray4 value 15)
- RGB < 128 → Black (Gray4 value 0)
- Simple, fast, and works well for weather icons

**Implementation**:
- `src/svgimage.rs` (lines 365-424) - `render_to_buffer_gray4_binary()`
- `src/drawsvg.rs` (lines 179-194) - `get_svg_gray4_binary()`
- `src/display/manager.rs` (lines 1924-1945) - `render_forecast_icon_gray4()`
- Updated all day icon rendering in `render_forecast_fields_gray4()` to use new function

**Note**: Full 16-level grayscale (`render_to_buffer_gray4()`) still available for future use.

### Forecast Border Fix ✓ (2026-02-04)

Fixed day header borders not rendering in forecast display:

**Root Cause**: `render_centered_text_mono/gray4()` functions didn't check or draw field borders.

**Fix**: Added border rendering to both functions:
- Check `field.border > 0` before drawing text
- Use `PrimitiveStyle::with_stroke()` to draw border rectangle
- Applied to both mono and gray4 versions
- Day headers now show 1px borders as specified in layout

**Files Modified**:
- `src/display/manager.rs` (lines 1925-1981) - Border support added

### Weather Glyphs Inventory

**Available** (`src/weather_glyph.rs`):
- THERMO_RAW_DATA: 4 glyphs (12x12px)
  - Index 0: Temperature (thermometer)
  - Index 1: Wind
  - Index 2: Humidity
  - Index 3: Precipitation
- MOON_PHASE_RAW_DATA: 8 moon phase glyphs (30x30px)
  - New, Waxing Crescent, First Quarter, Waxing Gibbous
  - Full, Waning Gibbous, Third Quarter, Waning Crescent

**Not Available**:
- Sunrise/Sunset glyphs (currently text-only display)
- Could be added in future if needed

### Known Limitations & TODO

1. **Moon Phase Glyphs** - Glyphs exist but not yet integrated
   - 8 phases available at 30x30px
   - Need to add rendering logic similar to weather glyphs
   - Could replace text "Moon: HH:MM" with visual moon phase

2. **Sunrise/Sunset Visual** - Currently text-only
   - No glyphs available in weather_glyph.rs
   - Could create or add sunrise/sunset icons if desired

3. **Wide Display Testing** - Need to test on actual SSD1322 (256x64)
   - Currently validated on emulator only
   - May need layout tweaks for real hardware

4. **SVG Icon Caching** - Each frame re-renders SVG from file
   - Could cache rendered buffers for performance
   - Low priority (60 FPS still achievable)

### Testing - SSD1309 (128x64 Monochrome) ✓ VALIDATED

Build and run emulator:
```bash
cargo build --release --features emulator
./target/release/LyMonS --name mythy \
  -W "pm9iWRnHWowgH7cNOGeWmtgFYa6LLelk,F,,42.36141470379943,-71.1040784239152" \
  --config config.yaml
```

**Test Results** (All passing):
- ✓ Clock displays automatically
- ✓ Press **W** → Weather current with SVG icon, glyphs, all data fields, centered conditions
- ✓ Press **W** again → Weather forecast with 3 columns, SVG icons, day names, temps (min|max), precip %
- ✓ Press **W** again → Back to weather current (smooth toggle)
- ✓ Press **C** → Clock mode (persists, manual override active)
- ✓ Press **A** → Automatic mode switching re-enabled
- ✓ SVG icons render correctly from `./assets/mono/`
- ✓ Text alignment works (conditions centered)
- ✓ Bordered boxes render on forecast
- ✓ No debouncing issues with rapid key presses

### Implementation Details

**Weather Current Page Layout** (`layout_manager.rs` lines 182-287):
- Weather icon field: 30x30 glyph/SVG at top-left
- Four data rows with glyph + text pattern:
  - Temperature: Thermometer glyph + "72(68) °F" (actual/feels-like)
  - Humidity: Droplet glyph + "65%"
  - Wind: Wind glyph + "12 mph NE"
  - Precipitation: Rain glyph + "20%"
- Conditions text: Centered at bottom with FONT_7X14 (e.g., "Mostly Clear")

**Weather Forecast Page Layout** (`layout_manager.rs` lines 289-420):
- Three 40px-wide columns starting at x=4, 44, 84
- Each column contains:
  - 30x30 SVG icon at top (centered within column)
  - Day name with border (FONT_4X6, centered)
  - Bordered data box (22px high)
  - Temperature range inside box (FONT_4X6, centered, "45°|62°" format)
  - Precipitation below temp (FONT_4X6, centered, "20%" format)

**Rendering Pipeline**:
1. `render_weather_current/forecast()` - Fetch data, format strings, dispatch by framebuffer type
2. `render_*_fields_mono/gray4()` - Loop through page fields, render each by name
3. Helper methods handle specific rendering tasks:
   - `render_forecast_icon_mono()` - SVG loading and ImageRaw drawing
   - `render_centered_text_*()` - Text with alignment calculation
   - `render_box_*()` - Bordered rectangles
   - `draw_weather_glyph()` - Pixel-by-pixel 12x12 glyph rendering

**Data Flow**:
- Weather data from `self.weather_display.weather_data()` (Vec<WeatherData>)
- Index 0 = current weather, 1-3 = forecast days
- SVG paths from `weather_data.weather_code.svg` field
- Temperature/humidity/wind/precip from WeatherData fields

---

## PREVIOUS: Grayscale/Color Support (2026-02-03)

### Overview
Implemented universal color support system that works across monochrome (BinaryColor) and grayscale (Gray4) displays without code duplication.

### Architecture: Field-Based Color System ✓

**Core Principle**: Color abstraction at the Field level, not component level.

```rust
// Field already has color information
pub struct Field {
    pub fg_color: Color,      // Semantic color (Black, Gray, White, etc.)
    pub bg_color: Option<Color>,
    // ... other fields
}

// Color enum converts to target display type
pub enum Color {
    Black, DarkGray, Gray, LightGray, White,
    Grayscale(u8),
}

impl Color {
    pub fn to_binary(&self) -> BinaryColor { ... }
    pub fn to_gray4(&self) -> Gray4 { ... }
}

// ConvertColor trait for generic conversion
pub trait ConvertColor<C> {
    fn to_color(self) -> C;
}

// Implementations
impl ConvertColor<BinaryColor> for Color { ... }
impl ConvertColor<Gray4> for Color { ... }
impl ConvertColor<BinaryColor> for BinaryColor { ... }  // Identity
impl ConvertColor<Gray4> for BinaryColor { ... }        // Mono→Gray
```

### Component Pattern: Generic Rendering

```rust
// Before (hardcoded BinaryColor)
pub fn render_field<D>(&self, field: &Field, target: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = BinaryColor>,
{
    let style = MonoTextStyle::new(font, BinaryColor::On);
    // ...
}

// After (generic over color type)
pub fn render_field<D, C>(&self, field: &Field, target: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = C>,
    C: PixelColor,
    Color: ConvertColor<C>,
{
    use crate::display::color_proxy::ConvertColor;
    let style = MonoTextStyle::new(font, field.fg_color.to_color());  // Auto-converts!
    // ...
}
```

### Manager Pattern: Framebuffer Dispatching

```rust
// Dispatch rendering based on framebuffer type
match &mut self.framebuffer {
    FrameBuffer::Mono(fb) => {
        // Render with BinaryColor
        self.status_bar.render_field(field, fb)?;
        self.scrolling_text.render_field(field, fb)?;
    }
    FrameBuffer::Gray4(fb) => {
        // Same calls, different color type - automatic!
        self.status_bar.render_field(field, fb)?;
        self.scrolling_text.render_field(field, fb)?;
    }
}
```

### Files Modified

1. **`src/display/color_proxy.rs`** - Extended ConvertColor trait
   - Added `impl ConvertColor<BinaryColor> for Color`
   - Added `impl ConvertColor<Gray4> for Color`
   - Enables `.to_color()` to work generically

2. **`src/display/components/status_bar.rs`** - Made color-generic
   - Changed `render_field<D>` to `render_field<D, C>`
   - Uses `field.fg_color.to_color()` instead of `field.fg_binary()`
   - Works on mono AND grayscale displays

3. **`src/display/components/scrollers.rs`** - Made color-generic
   - Changed `render_field<D>` to `render_field<D, C>`
   - Uses `field.fg_color.to_color()`
   - Scrolling text works on all display types

4. **`src/display/manager.rs`** - Updated render_scrolling()
   - Added framebuffer type dispatch (Mono/Gray4 branches)
   - Both branches call same component methods
   - Zero code duplication

5. **`src/display/components/visualizer.rs`** - Basic structure
   - Added `viz_state: LastVizState` field
   - Added `render_mono()` and `render_gray4()` dispatcher stubs
   - Ready for actual drawing code

### Testing Results (2026-02-03 22:30 UTC)

✅ **Build Status**: Compiles cleanly with 176 warnings (no errors)
✅ **Runtime Status**: Emulator running successfully on ssd1322 (256x64 Gray4)
✅ **Scrolling Mode**: Works on both monochrome and grayscale displays
✅ **Status Bar**: Works on both display types
✅ **No Regressions**: Existing functionality preserved

**Command Used**:
```bash
./target/debug/LyMonS --name mythy --emulated --viz vu_stereo
```

### Clock Mode Implementation (2026-02-03 23:15 UTC) ✓

**Approach**: Clock is monochrome-only ("lowest common denominator" design) but must work on both display types.

**Key Changes**:

1. **Added Gray4 support to ClockDisplay** (`src/display/components/clock.rs`):
   - Added `render_gray4()` method that mirrors `render()` logic
   - Created `draw_clock_char_gray4()` to convert BinaryColor font bitmaps to Gray4::WHITE
   - Uses `GetPixel` trait to iterate over BinaryColor ImageRaw and draw as white pixels

2. **Updated manager render_clock()** (`src/display/manager.rs`):
   - Restructured to avoid borrow checker conflicts
   - Renders metrics first (before borrowing framebuffer)
   - Match on framebuffer type: Mono vs Gray4
   - Each branch renders clock_digits, seconds_progress, and date fields
   - Gray4 branch uses `Gray4::WHITE` and `Gray4::BLACK` instead of BinaryColor

3. **Borrow Checker Solution**:
   - `render_metrics()` needs mutable framebuffer access
   - Solution: Call it before the `match &mut self.framebuffer` statement
   - Prevents double-borrow conflict

**Files Modified**:
- `src/display/components/clock.rs`: +88 lines (render_gray4, draw_clock_char_gray4)
- `src/display/manager.rs`: Refactored render_clock() with framebuffer dispatching

**Why It Works**:
- Clock font data is BinaryColor (1-bit bitmap)
- For Mono displays: Draw directly with BinaryColor
- For Gray4 displays: Convert On → Gray4::WHITE, Off → Gray4::BLACK
- Single-color design means no complex color mapping needed

### Track Playing Page Completion (2026-02-03 23:30 UTC) ✓

**Issue Found**: Gray4 branch had incomplete field rendering - progress_bar and info_line were marked TODO.

**What Was Added**:

1. **Progress Bar on Gray4** (`src/display/manager.rs:680-717`):
   - Added full progress bar rendering for grayscale displays
   - Uses `field.fg_color.to_color()` for dynamic color conversion
   - Outline and fill both use field's foreground color
   - Matches Mono branch logic: outline, fill based on track progress

2. **Info Line on Gray4** (`src/display/manager.rs:718-765`):
   - Current time (left-aligned)
   - Mode text (centered)
   - Remaining/total time (right-aligned)
   - Uses `field.fg_color.to_color()` for text color
   - Identical logic to Mono branch but color-generic

**Pattern Used**:
```rust
// Get color from field and convert to target type
use crate::display::color_proxy::ConvertColor;
let text_color = field.fg_color.to_color();
let style = MonoTextStyle::new(font, text_color);
```

**Files Modified**:
- `src/display/manager.rs`: +87 lines for Gray4 progress_bar and info_line

**Result**: Track playing page now fully functional on both monochrome and grayscale displays with all fields rendering correctly.

### Session Summary (2026-02-04 04:15 UTC) ✓

**Major Accomplishments**:
1. ✅ **Status Bar with Glyphs** - Volume, Repeat, Shuffle icons + centered bitrate
2. ✅ **Clock Mode** - Complete for Mono + Gray4, pixel-perfect positioning
3. ✅ **Track Playing** - Completed Gray4 progress_bar + info_line
4. ✅ **Gray4 SVG Renderer** - Extended for weather icon support
5. ✅ **Help Text** - Fixed typo, marked emulated as internal
6. ✅ **Tested** - Both SSD1322 (256x64 Gray4) and SSD1309 (128x64 Mono)

**Technical Details**:
- Pixel-by-pixel glyph rendering for cross-color-depth support
- Text baseline: `glyph_y + 7` for perfect alignment with 8x8 glyphs
- Field-based color with `ConvertColor` trait pattern

### Current Mode Status

| Mode | Mono (BinaryColor) | Gray4 (Grayscale) | Notes |
|------|-------------------|-------------------|-------|
| Track Playing | ✅ Complete | ✅ Complete | All fields + glyph status bar |
| Clock | ✅ Complete | ✅ Complete | Perfect positioning |
| Weather | 🔄 Next | 🔄 Next | SVG ready, need field rendering |
| Easter Eggs | ⏳ Pending | ⏳ Pending | After weather |
| Visualizer | ⏳ Pending | ⏳ Pending | Drawing code needed |

### Pattern for Remaining Modes

**Step-by-Step Checklist** (apply to each mode):

1. **Make component render_field() generic**:
   ```rust
   pub fn render_field<D, C>(&self, field: &Field, target: &mut D)
   where
       D: DrawTarget<Color = C>,
       C: PixelColor,
       Color: ConvertColor<C>,
   ```

2. **Replace color references**:
   - `field.fg_binary()` → `field.fg_color.to_color()`
   - `BinaryColor::On` → `field.fg_color.to_color()` (or `Color::White.to_color()`)
   - `BinaryColor::Off` → `Color::Black.to_color()`

3. **Update manager render function**:
   ```rust
   match &mut self.framebuffer {
       FrameBuffer::Mono(fb) => { /* render fields */ }
       FrameBuffer::Gray4(fb) => { /* same render fields */ }
   }
   ```

4. **Test on both displays**:
   - Run with `--emulated` (defaults to ssd1322 Gray4)
   - Run with `--driver ssd1309` (monochrome)

### Known Issues & TODOs

**Immediate**:
- [ ] Clock mode needs color-generic implementation
- [ ] Weather mode needs restored render methods
- [ ] Easter eggs mode needs pattern applied
- [ ] Visualizer needs actual drawing code (currently stubs)
- [ ] Progress bar in scrolling mode uses hardcoded BinaryColor (low priority)

**Future Enhancements**:
- SVG selection based on color depth (mono vs color glyphs)
- Color palette for themed displays
- User-configurable field colors in layouts

### Why This Architecture Works

**Separation of Concerns**:
- Field: "What color should this be?" (semantic)
- Color: "How do I look on this display?" (conversion)
- Component: "Draw using the field's color" (agnostic)
- Manager: "Here's the right framebuffer" (dispatch)

**Benefits**:
- ✅ Zero code duplication (same component code for all display types)
- ✅ Type-safe (compile-time checks)
- ✅ Extensible (add RGB/full color support later)
- ✅ Maintainable (change in one place)
- ✅ Testable (mock different display types)

**Key Insight**: Let Rust's type system and trait system do the work. The `.to_color()` method automatically picks the right conversion based on the target color type `C`.

---

## Completed Work

### 1. Clock Pages ✓
- Small and large clock layouts
- Time/date display with customizable fonts
- ISO-8859-13 font support for international characters

### 2. Weather Pages ✓
- Current conditions and forecast display
- Icon-based weather visualization
- SVG rendering for weather icons
- Automatic polling and updates

### 3. Easter Eggs ✓
- 12 whimsical audio-related animations:
  - bass, cassette, ibmpc, moog, radio40, radio50
  - reel2reel, scope, technics, tubeamp, tvtime, vcr
- SVG template rendering with dynamic variables
- Text overlays (artist, title, time) with wrapping support
- Audio quality indicators (SD/HD/DSD)
- Track progress animations

### 4. Peak Meters ✓ (COMPLETED & TESTED)
- Mono and stereo peak meters
- 19 level brackets (-36dB to +8dB)
- Real-time audio visualization from shared memory
- Peak hold and decay
- SVG background panels
- Integration with Visualizer backend

**Files Modified for Peak Meters**:
- `src/display/components/visualizer.rs` - Added draw_peak_mono/pair methods
- `src/display/manager.rs` - Implemented render_visualizer with frame consumption
- `src/main.rs` - Enabled visualizer setup (uncommented)

**Issues Found & Fixed**:
1. **Visualization Type Not Set** - VisualizerComponent initialized with NoVisualization
   - Fixed: Added `set_visualization_type()` call in `setup_visualizer()`
2. **Blank Screen After First Frame** - Framebuffer cleared each frame but background not redrawn
   - Fixed: Changed to always redraw SVG background every frame (not just on init)
   - Removed early-return optimization that skipped rendering

**Testing Results** (2026-02-03 16:48 UTC):
- ✓ PeakStereo: Working correctly with dual meters, SVG background visible
- ✓ PeakMono: Working correctly with single taller meter, SVG background visible
- ✓ Meters respond to audio levels when playing
- ✓ Frame rates stable after initial SVG load

### 5. Histogram Visualizations ✓ (COMPLETED & TESTED)
- Mono histogram (frequency spectrum bars)
- Stereo histogram (left/right frequency spectrum)
- Real-time FFT analysis from audio stream
- Body decay physics (rise fast, fall slow)
- Peak cap markers with hold and decay
- Smooth animations with configurable timing
- DRY code refactoring for maintainability

**Files Modified for Histograms**:
- `src/display/components/visualizer.rs` - Added histogram rendering methods
  - `draw_hist_mono()` - Mono histogram with decay physics
  - `draw_hist_stereo()` - Stereo histogram with independent L/R channels
  - `draw_hist_panel_with_caps()` - Panel rendering with bars and caps
  - `update_body_decay()` - Bar decay physics helper
  - `update_caps()` - Cap hold and decay helper
  - `update_histogram_channel_physics()` - DRY helper for single channel physics
- `src/display/manager.rs` - Added histogram payload handling in `render_visualizer()`

**Issues Found & Fixed**:
1. **Top Border Artifact** - 2-3 pixel line across top of screen
   - Fixed: Removed unnecessary border line from draw_hist_panel_with_caps
2. **Caps Not Decaying** - Circular data flow bug
   - Root cause: Cloning bands from viz_state then copying back to same location
   - Fixed: Removed unnecessary band cloning/copying, use data in-place
3. **Decay Timing Tuning** - Found optimal decay parameters through testing
   - Too fast (96 levels/sec): Caps disappeared too quickly
   - Too slow (4 levels/sec): Caps lingered too long
   - **Perfect (8 levels/sec)**: Smooth, visible, responsive
4. **Code Duplication** - Decay/cap physics duplicated between mono and stereo
   - Fixed: Extracted common logic into `update_histogram_channel_physics()` helper
   - Eliminated ~20 lines of duplicate code, single source of truth

**Final Decay Parameters**:
```rust
const CAP_HOLD: Duration = Duration::from_millis(500);  // 0.5 sec hold
const CAP_DECAY_LPS: f32 = 8.0;  // 1 pixel per 1/8 second
const HIST_DECAY_PER_TICK: u8 = 1;  // Bar decay rate
```

**Testing Results** (2026-02-03 17:16 UTC):
- ✓ HistMono: Frequency spectrum displaying correctly
- ✓ HistStereo: Dual panels with independent L/R channels working perfectly
- ✓ Bar decay: Smooth rise and fall physics working
- ✓ Peak caps: Hold 0.5 sec, decay at 8 levels/sec (perfect!)
- ✓ No visual artifacts, clean rendering
- ✓ Responsive to audio dynamics
- ✓ DRY refactoring: Code is maintainable and reusable

---

## Architecture

### Component Pattern

Each display component follows this pattern:

```rust
pub struct ComponentName {
    layout: LayoutConfig,
    state: ComponentState,
    // Component-specific fields
}

impl ComponentName {
    pub fn new(layout: LayoutConfig) -> Self { ... }

    pub fn render<D>(&mut self, target: &mut D) -> Result<(), D::Error>
    where D: DrawTarget<Color = BinaryColor> { ... }

    // Helper methods as needed
}
```

### Key Components

1. **ClockComponent** (`src/display/components/clock.rs`)
   - Renders time/date displays
   - Supports multiple layouts (small/large)

2. **WeatherComponent** (`src/display/components/weather.rs`)
   - Weather data display with icons
   - Current conditions and forecasts

3. **ScrollingComponent** (`src/display/components/scrollers.rs`)
   - Scrolling text for track info
   - Multiple scroll modes

4. **EasterEggsComponent** (`src/display/components/easter_eggs.rs`)
   - SVG-based animations
   - Text overlays with wrapping

5. **VisualizerComponent** (`src/display/components/visualizer.rs`)
   - Audio visualizations (peak meters, VU, histograms)
   - Consumes frames from Visualizer backend

### Display Manager

**File**: `src/display/manager.rs`

Central coordinator that:
- Manages all components
- Owns framebuffer
- Dispatches rendering based on display mode
- Handles mode transitions
- Consumes data from backends (LMS, Weather, Visualizer)

**Display Modes**:
```rust
pub enum DisplayMode {
    Clock,          // Clock face
    Scrolling,      // Track info scrolling
    Weather,        // Weather display
    EasterEggs,     // Audio animations
    Visualizer,     // Audio visualizations
}
```

**Mode Priority (when playing)**:
1. EasterEggs (if egg_type != 255)
2. Visualizer (if visualizer_type != "no_viz")
3. Scrolling (default)

---

## Visualizer Backend Architecture

### Components

1. **VisReader** (`src/vision.rs`)
   - Reads from `/dev/shm/squeezelite-*` shared memory
   - Lock-free reading with pthread rwlock
   - De-interleaves stereo audio samples

2. **Visualizer** (`src/visualizer.rs`)
   - Spawns async worker task
   - Computes audio metrics (peak, RMS, FFT)
   - Publishes VizFrameOut frames to channel
   - Supports 9 visualization types

3. **VizFrameOut** structure:
```rust
pub struct VizFrameOut {
    pub ts: i64,
    pub playing: bool,
    pub sample_rate: u32,
    pub kind: Visualization,
    pub payload: VizPayload,
}

pub enum VizPayload {
    VuStereo { l_db: f32, r_db: f32 },
    VuMono { db: f32 },
    PeakStereo { l_level: u8, r_level: u8, l_hold: u8, r_hold: u8 },
    PeakMono { level: u8, hold: u8 },
    HistStereo { bands_l: Vec<u8>, bands_r: Vec<u8> },
    HistMono { bands: Vec<u8> },
    // ... more variants
}
```

### Data Flow

```
Audio Playback (Squeezelite)
    ↓ writes to
Shared Memory (/dev/shm/squeezelite-*)
    ↓ read by
VisReader (vision.rs)
    ↓ processes audio
Visualizer Worker (visualizer.rs)
    ↓ publishes frames
mpsc::channel<VizFrameOut>
    ↓ consumed by
DisplayManager.render_visualizer()
    ↓ updates
VisualizerComponent.viz_state
    ↓ renders
Peak Meters / VU / Histograms
```

---

## Current Implementation: Peak Meters

### Key Methods

**VisualizerComponent** (`src/display/components/visualizer.rs`):

```rust
fn draw_peak_mono<D>(
    target: &mut D,
    level: u8,      // 0-48 (PEAK_METER_LEVELS_MAX)
    hold: u8,       // Peak hold value
    vk: Visualization,
    state: &mut LastVizState,
) -> Result<bool, D::Error>
```

- Draws 19 rectangular segments
- Segment positions: x=15, spacing=5 or 7
- Segment sizes: 2x36 (negative) or 4x36 (positive)
- Colors: On if level >= bracket threshold

```rust
fn draw_peak_pair<D>(
    target: &mut D,
    l_level: u8, r_level: u8,
    l_hold: u8, r_hold: u8,
    vk: Visualization,
    state: &mut LastVizState,
) -> Result<bool, D::Error>
```

- Same as mono but draws two rows (y=7 and y=40)
- Height: 17 pixels per meter
- Separate left/right channels

**Level Brackets** (19 segments):
```rust
[-36, -30, -20, -17, -13, -10, -8, -7, -6, -5,
 -4,  -3,  -2,  -1,   0,   2,  3,  5,  8]  // dBFS
```

### State Tracking

**LastVizState** (`src/vision.rs`):
- `last_peak_m`, `last_peak_l`, `last_peak_r` - Current levels
- `last_hold_m`, `last_hold_l`, `last_hold_r` - Peak hold values
- `buffer: Vec<u8>` - SVG background panel
- `init: bool` - First render flag

Only redraws when levels change (optimization).

### Rendering Pipeline

```rust
// DisplayManager.render_visualizer()
1. Try to receive latest VizFrameOut from visualizer.rx channel
2. Extract level/hold values from payload
3. Update viz_state with new values
4. Call visualizer.render(framebuffer)
   → Reads from viz_state
   → Calls draw_peak_mono/pair
   → Draws rectangles to framebuffer
```

---

## Testing Peak Meters

### Command Line
```bash
# Mono peak meter
cargo run --features emulator -- --name mythy --emulated -a peak_mono

# Stereo peak meters (default)
cargo run --features emulator -- --name mythy --emulated -a peak_stereo
```

### Requirements
1. Squeezelite running with shared memory enabled
2. Audio playing through LMS
3. Shared memory file exists: `/dev/shm/squeezelite-*`

### Verification
- Check logs for "Visualizer setup complete: peak_stereo"
- Mode should change to "Display mode changed: Clock -> Visualizer"
- Emulator window should show bouncing meters
- Meters respond to audio levels in real-time

---

## Next Tasks

### Immediate (Visualizations)

1. **VU Meters** - Analog-style VU meters with needle physics
   - VuStereo - Left/Right meters
   - VuMono - Single mono meter
   - Needle animation with inertia/damping
   - Already have VuNeedle physics in `src/vuphysics.rs`

2. **Histogram Visualizations** - Frequency spectrum bars
   - HistStereo - Left/Right frequency bands
   - HistMono - Mono frequency bands
   - Use SpectrumEngine (already in `src/spectrum.rs`)
   - Bar decay and peak caps

3. **Combination Modes**
   - VuStereoWithCenterPeak - L/R VU + center peak meter
   - AioVuMono - All-in-One with VU + track info
   - AioHistMono - All-in-One with histogram + track info

### Reference Implementation

All visualization rendering in `src/display_old.rs`:
- Lines 1884-2016: Peak meters (DONE ✓)
- Lines 2018-2175: Histogram pairs
- Lines 2177-2343: Histogram mono
- Lines 2345-2593: VU stereo
- Lines 2595-2737: VU mono
- Lines 2739-2985: Combination modes

### Future Enhancements

1. **Easter Egg Animations** - Enhance visual movement
   - Cassette/reel2reel tape transfer animation
   - Tubeamp VU meter bounce
   - Better SVG template variable ranges

2. **Additional Display Modes**
   - Spectrum analyzer
   - Waveform display
   - Custom user layouts

3. **Performance Optimization**
   - Profile render loop
   - Optimize SVG parsing/rendering
   - Reduce allocations further

---

## Important Code Patterns

### Zero Allocation Rendering

```rust
// Pre-allocate buffers in component
pub struct Component {
    buffer: Vec<u8>,  // Reused across renders
}

// Borrow management
let value = self.get_value();  // Extract before mutable borrow
{
    let fb = self.framebuffer.as_mono_mut();
    // Use fb with extracted value
}  // End borrow scope
```

### Text Rendering with Wrapping

```rust
use embedded_text::{
    alignment::{HorizontalAlignment, VerticalAlignment},
    style::TextBoxStyleBuilder,
    TextBox,
};

let textbox_style = TextBoxStyleBuilder::new()
    .alignment(HorizontalAlignment::Left)
    .vertical_alignment(VerticalAlignment::Top)
    .build();

TextBox::with_textbox_style(text, rect, char_style, textbox_style)
    .draw(target)?;
```

### SVG Rendering

```rust
// Template variables in SVG files:
// {{artist}}, {{title}}, {{track-percent}}, {{progress-arc}}, etc.

let svg_data = template_svg.replace("{{artist}}", artist);
let renderer = SvgImageRenderer::new(&svg_data, width, height)?;
renderer.render_to_buffer(&mut buffer)?;
let raw_image = ImageRaw::<BinaryColor>::new(&buffer, width);
Image::new(&raw_image, position).draw(target)?;
```

### ISO-8859-13 Fonts

```rust
// Always use ISO fonts for international character support
use embedded_graphics::mono_font::iso_8859_13::{
    FONT_4X6, FONT_5X7, FONT_5X8,
    FONT_6X10, FONT_6X13_BOLD, FONT_7X14
};
```

---

## Known Issues

### Easter Eggs
- **Animation Limited**: SVG template variables update but visual movement is subtle
  - Cassette/reel2reel tape spools don't show visible transfer
  - Tubeamp VU meters are static (no bounce)
  - Less dynamic than original C implementation (7 years ago)
  - TODO: Enhance SVG artwork and animation parameters

### Visualizations
- **Peak Hold Decay**: Using simple saturating_sub, could enhance with:
  - Configurable hold time
  - Slower decay rate
  - Visual peak markers

### General
- Many unused imports (165 warnings)
- Some code duplication in component patterns
- Could benefit from macro for common component methods

---

## Build & Run Commands

### Development Build
```bash
cargo build --features emulator
```

### Run with Emulator
```bash
# Clock mode
cargo run --features emulator -- --name mythy --emulated -F=roboto

# Easter eggs
cargo run --features emulator -- --name mythy --emulated -F=roboto \
  -E=tvtime  # or bass, cassette, ibmpc, etc.

# Visualizations
cargo run --features emulator -- --name mythy --emulated -F=roboto \
  -a peak_stereo  # or peak_mono, vu_stereo, vu_mono, hist_stereo, etc.

# Weather
cargo run --features emulator -- --name mythy --emulated -F=roboto \
  -W=API_KEY,F,,lat,lon
```

### Hardware Build (Production)
```bash
cargo build --release
```

---

## File Organization

```
src/
├── main.rs                    - Entry point, CLI args, main loop
├── display/
│   ├── mod.rs                 - Display module exports
│   ├── manager.rs             - DisplayManager (central coordinator)
│   ├── mode_controller.rs     - Display mode logic
│   ├── layout_manager.rs      - Layout definitions
│   ├── components/
│   │   ├── clock.rs           - Clock component ✓
│   │   ├── weather.rs         - Weather component ✓
│   │   ├── scrollers.rs       - Scrolling text component ✓
│   │   ├── easter_eggs.rs     - Easter eggs component ✓
│   │   └── visualizer.rs      - Visualizer component (in progress)
│   ├── drivers/               - Hardware display drivers
│   └── emulator_controller.rs - Emulator integration
├── visualizer.rs              - Visualizer backend (audio processing)
├── vision.rs                  - Shared memory reader (VisReader)
├── spectrum.rs                - FFT/spectrum analysis
├── vuphysics.rs               - VU meter needle physics
├── eggs.rs                    - Easter egg SVG management
├── weather.rs                 - Weather data fetching
├── sliminfo.rs                - LMS communication
└── display_old.rs             - Original implementation (reference)
```

---

## Key Dependencies

- **embedded-graphics** 0.8 - Graphics primitives
- **embedded-text** 0.8 - Text layout with wrapping
- **resvg** 0.45 - SVG rendering
- **tokio** 1.47 - Async runtime
- **winit** 0.30 - Window management (emulator)
- **pixels** 0.14 - Framebuffer rendering (emulator)

---

## Current Status

**✓ PEAK METERS & HISTOGRAMS FULLY IMPLEMENTED, TESTED, AND WORKING**

Four visualization types are now complete and production-ready:

**Peak Meters** (PeakStereo, PeakMono):
- ✓ Real-time audio level display from shared memory
- ✓ Stereo (dual 17px) or mono (single 36px) meters
- ✓ 19 level brackets with proper dB scaling (-36dB to +8dB)
- ✓ SVG background panels rendering correctly
- ✓ Mode switching working correctly

**Histograms** (HistStereo, HistMono):
- ✓ Frequency spectrum bars from FFT analysis
- ✓ Mono: Single downmix panel
- ✓ Stereo: Dual panels with independent L/R channels
- ✓ Body decay physics (rise fast, fall slow)
- ✓ Peak cap markers with optimized timing (0.5 sec hold, 8 levels/sec decay)
- ✓ DRY code architecture with reusable physics helper
- ✓ Clean rendering, no artifacts

**Testing Summary** (2026-02-03 17:16 UTC):

**PeakStereo Test**:
- Command: `cargo run --features emulator -- --name mythy --emulated -a peak_stereo`
- Result: ✓ Dual meters visible, SVG background persistent

**PeakMono Test**:
- Command: `cargo run --features emulator -- --name mythy --emulated -a peak_mono`
- Result: ✓ Single meter visible, properly centered

**HistMono Test**:
- Command: `cargo run --features emulator -- --name mythy --emulated -a hist_mono`
- Result: ✓ Frequency spectrum with smooth cap physics working perfectly

**HistStereo Test**:
- Command: `cargo run --features emulator -- --name mythy --emulated -a hist_stereo`
- Result: ✓ Dual L/R panels, independent decay, looks fantastic!

**Issues Encountered & Resolved**:
1. Peak meters: Visualization type not being set → Fixed in setup_visualizer()
2. Peak meters: Blank screen after first frame → Fixed by always redrawing SVG background
3. Histogram: Top border artifact → Removed unnecessary border line
4. Histogram: Caps not decaying → Fixed circular data flow bug
5. Histogram: Decay timing → Tuned to optimal 8 levels/sec with 0.5 sec hold
6. Histogram: Code duplication → Refactored with DRY helper function

**Next Session**: Implement VU meters (analog needle animation with physics) or combination modes

---

## Automation Possibilities

To automate this state tracking:

1. **Git Hooks** - Update this file on commit
   ```bash
   # .git/hooks/pre-commit
   # Update timestamp, git status, etc.
   ```

2. **Task Tool Integration** - Use TaskCreate/TaskUpdate
   - Track each component as a task
   - Update status automatically
   - Generate reports

3. **CI/CD** - Automated testing
   - Run emulator tests
   - Screenshot comparison
   - Performance benchmarks

4. **Documentation Generation** - From code comments
   - Extract /// doc comments
   - Generate architecture diagrams
   - API documentation

**Recommendation**: For now, manual updates work well. As project matures, add Git hooks and task tracking.

---

## Contact Points for Resumption

When resuming this project:

1. **Read this file first** - Complete context
2. **Check last git commit** - Recent changes
3. **Review TaskList** - If using task tools
4. **Run latest test** - Verify current state
5. **Reference display_old.rs** - For next component to port

**Quick Start for Next Session**:
```bash
# 1. Navigate to project
cd /data2/refactor/LyMonS

# 2. Read this state file
cat REFACTORING_STATE.md

# 3. Check current state
git status
git log --oneline -10

# 4. Build and test current state
cargo build --features emulator
cargo run --features emulator -- --name mythy --emulated -a peak_stereo

# 5. Start next task (VU meters)
# - Read src/display_old.rs lines 2345-2593
# - Port to src/display/components/visualizer.rs
# - Test with -a vu_stereo or -a vu_mono
```

---

**End of State Document**

---

## Session 2026-02-04: Weather Mode & Emulator Keyboard Shortcuts

### Weather Current Display Implementation ✓

Completed implementation of weather current page with all text fields and glyphs:

**Fields Implemented**:
- Temperature with feels-like (e.g., "72(68) °F") - FONT_6X13_BOLD
- Humidity percentage (e.g., "65%") - FONT_5X8
- Wind speed and direction (e.g., "10 mph NW") - FONT_5X8
- Precipitation probability (e.g., "20%") - FONT_5X8
- Conditions text (e.g., "Partly Cloudy") - FONT_7X14 (ISO charset)
- Small 12x12 weather glyphs (temp, humidity, wind, precip icons from THERMO_RAW_DATA)

**Technical Approach**:
- Static helper methods to avoid borrow checker issues
- `render_weather_fields_mono()` and `render_weather_fields_gray4()`
- `draw_weather_glyph()` for 12x12 pixel-by-pixel glyph rendering
- Works on both SSD1309 (Mono) and SSD1322 (Gray4) displays
- SVG weather icon rendering deferred (needs SVG integration into DisplayManager)

**Files Modified**:
- `src/display/manager.rs`: Added weather rendering methods (~200 lines)
- All fonts use `iso_8859_13` for accented character support

### Emulator Keyboard Shortcuts ✓

Added keyboard shortcuts to trigger display modes for testing:

**Implementation**:
1. Extended `EmulatorState` with `requested_mode: Option<DisplayMode>` field
2. Added keyboard handlers in `emulator_window.rs`:
   - `W` key → Trigger Weather Current mode
   - `C` key → Trigger Clock mode
3. Added mode checking in unified display loop (main.rs)
4. DisplayManager checks emulator state each loop iteration

**Files Modified**:
- `src/display/drivers/emulator.rs`: Added `requested_mode` field, `take_requested_mode()` method
- `src/display/emulator_window.rs`: Added W/C key handlers, updated help text
- `src/display/manager.rs`: Added `emulator_state` field, `set_emulator_state()`, `check_emulator_mode_request()`
- `src/main.rs`: Added mode override check in unified loop (line ~180)

**Testing**: Confirmed working - pressing W triggers weather mode override.

### Pending Work

**Weather Mode**:
- Large SVG weather icon rendering (needs SVG renderer integration)
- Weather forecast page (3-day with small icons)

**Other Modes**:
- Easter Eggs mode (apply same pattern)
- Visualizer mode (VU meters, histograms)

---

