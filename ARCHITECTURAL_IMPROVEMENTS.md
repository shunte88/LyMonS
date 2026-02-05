# Architectural Improvements - Location, Astral, and Splash Screen

## Overview
Refactored core services to be modular and independent, enabling features like auto-brightness without requiring weather service activation.

## 1. Location Service (`src/location.rs`)

### Purpose
Centralized location determination for all services (weather, astronomical calculations, timezone, etc.)

### Features
- **Config-first approach**: Checks config for user-specified lat/lng
- **GeoIP fallback**: Uses ipapi.co lookup if not configured
- **Coordinate validation**: Validates lat (-90 to 90) and lng (-180 to 180)
- **Source tracking**: Records whether location came from config or GeoIP

### API
```rust
pub async fn get_location(
    config_lat: Option<f64>,
    config_lng: Option<f64>,
) -> Result<Location, LocationError>
```

### Usage Example
```rust
let location = location::get_location(
    config.latitude,
    config.longitude
).await?;

info!("Location: {}", location); // Prints: "New York, NY (40.7128, -74.0060) [geoip]"
```

## 2. Astral Service (`src/astral.rs`)

### Purpose
Independent astronomical calculations (sunrise, sunset, moonrise, moonset) - no weather dependency

### Features
- **Daily caching**: Calculations cached for current day
- **Automatic updates**: Recalculates when date changes
- **Utility methods**:
  - `is_daytime()` - Check if currently daytime
  - `minutes_until_next_event()` - Schedule auto-brightness changes
  - `get_today()` - Get cached or calculate astral data
- **Location updates**: Can update location and invalidate cache

### API
```rust
let astral = AstralService::new(location);
let data = astral.get_today();

// Auto-brightness scheduling
if let Some(minutes) = astral.minutes_until_next_event() {
    info!("Next sunrise/sunset in {} minutes", minutes);
}

// Daytime check
if astral.is_daytime() {
    set_brightness(255); // Full brightness
} else {
    set_brightness(64);  // Dimmed
}
```

### Use Cases
1. **Auto-brightness**: Adjust screen brightness at dawn/dusk
2. **Weather display**: Show sunrise/sunset times
3. **General info**: Display astronomical data even without weather service
4. **Energy saving**: Dim display at night automatically

## 3. Config Updates (`src/config.rs`)

### New Fields
```yaml
# Optional: Specify location for astronomical calculations
# If not provided, will use GeoIP lookup
latitude: 40.7128    # New York City
longitude: -74.0060
```

### Benefits
- Users can specify exact location for accurate sun times
- Faster startup (no GeoIP lookup needed)
- Works offline
- More accurate for specific addresses

## 4. Splash Screen Layout (`layout_manager.rs`)

### Purpose
Proper splash screen using layout manager system (replaces old hardcoded approach)

### Fields
1. **logo_svg**: Full-screen background SVG (`./assets/lymonslogo.svg`)
2. **version**: Version string (e.g., "LyMonS v0.2.3") - White, FONT_6X13_BOLD
3. **build_date**: Build date - Cyan, FONT_5X8
4. **status**: Optional status message (e.g., "Determining location...") - Green, FONT_5X8

### Layout
```
┌────────────────────────────┐
│                            │
│    [LOGO SVG BACKGROUND]   │
│                            │
│      Initializing...       │ ← status (optional)
│                            │
│     LyMonS v0.2.3         │ ← version
│       2026-02-04           │ ← build_date
└────────────────────────────┘
```

### Features
- **Color-aware**: Uses color proxy system (adapts to mono/grayscale)
- **Centered layout**: All text centered using layout manager
- **Task progress**: Can update status field to show:
  - "Determining location..."
  - "Calculating astronomical data..."
  - "Fetching weather..."
  - "Initializing display..."

## Integration Points

### Startup Sequence (Recommended)
```rust
// 1. Load config
let config = Config::load()?;

// 2. Show splash screen
display.show_splash(&config, "Initializing...");

// 3. Determine location
display.update_splash_status("Determining location...");
let location = location::get_location(
    config.latitude,
    config.longitude
).await?;

// 4. Initialize astral service
display.update_splash_status("Calculating astronomical data...");
let astral = AstralService::new(location.clone());
let astral_data = astral.get_today();

// 5. Initialize weather (if enabled)
if config.weather_enabled {
    display.update_splash_status("Fetching weather...");
    weather.initialize(location).await?;
}

// 6. Complete initialization
display.update_splash_status("Ready!");
sleep(Duration::from_millis(500));

// 7. Switch to main display
display.set_mode(DisplayMode::Clock);
```

### Auto-Brightness Integration
```rust
// In main loop or timer
if let Some(minutes) = astral.minutes_until_next_event() {
    if minutes <= 5 {
        // Approaching sunrise/sunset, prepare for brightness change
        schedule_brightness_check();
    }
}

// Brightness adjustment
let brightness = if astral.is_daytime() {
    config.brightness_day.unwrap_or(255)
} else {
    config.brightness_night.unwrap_or(64)
};
display.set_brightness(brightness);
```

## Benefits

### Modularity
- Services independent of each other
- Weather service optional
- Astral calculations always available
- Location determined once, used everywhere

### Performance
- Daily caching of astronomical calculations
- GeoIP lookup only if needed
- Fast config-based location lookup

### User Experience
- Clear initialization progress via splash screen
- Auto-brightness for better readability
- Accurate sun times for any location

### Maintenance
- Clear separation of concerns
- Testable services
- Easy to extend (add moon calculations, twilight times, etc.)

## TODO / Future Enhancements

1. **Moon Calculations**: Implement moonrise/moonset in astral.rs
2. **Twilight Support**: Add civil/nautical/astronomical twilight times
3. **Solunatus Integration**: Add alternative calculation library as fallback
4. **Config GUI**: Add location picker UI
5. **Timezone Support**: Use location for automatic timezone detection
6. **Weather Integration**: Update weather service to use location service
7. **Splash Animation**: Add fade-in effect (from original display.rs)
8. **Progress Bar**: Visual progress indicator on splash

## Files Modified/Created

### New Files
- `src/location.rs` - Location service
- `src/astral.rs` - Astronomical calculations service
- `ARCHITECTURAL_IMPROVEMENTS.md` - This document

### Modified Files
- `src/config.rs` - Added latitude/longitude fields
- `src/main.rs` - Added module declarations (location, astral)
- `src/display/layout_manager.rs` - Added create_splash_page() method

### Ready for Integration
All modules compile successfully and are ready to be integrated into the main application flow.
