/*
 *  tests/display_integration.rs
 *
 *  Integration tests for display system
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 */

// Note: Since LyMonS is a binary crate without [lib] section, integration tests
// cannot import from it. These tests would work if the project is restructured as:
// - src/lib.rs (library code)
// - src/main.rs or src/bin/*.rs (binary)
//
// For now, unit tests in each module (with #[cfg(test)]) work fine.

// Placeholder test to make cargo happy
#[test]
fn test_placeholder() {
    // Integration tests would go here if the project had a lib crate
    assert!(true);
}

#[test]
fn test_layout_config_128x64() {
    let caps = DisplayCapabilities {
        width: 128,
        height: 64,
        color_depth: ColorDepth::Monochrome,
        supports_rotation: false,
        max_fps: 30,
        supports_brightness: true,
        supports_invert: false,
    };

    let layout = LayoutConfig::for_display(&caps);

    assert_eq!(layout.width, 128);
    assert_eq!(layout.height, 64);
    assert!(layout.status_bar.height > 0);
    assert!(layout.content_area.height > 0);
}

#[test]
fn test_layout_config_256x64() {
    let caps = DisplayCapabilities {
        width: 256,
        height: 64,
        color_depth: ColorDepth::Gray4,
        supports_rotation: false,
        max_fps: 60,
        supports_brightness: true,
        supports_invert: false,
    };

    let layout = LayoutConfig::for_display(&caps);

    assert_eq!(layout.width, 256);
    assert_eq!(layout.height, 64);
    assert!(layout.visualizer.width >= 256);
}

#[test]
fn test_layout_config_400x240() {
    let caps = DisplayCapabilities {
        width: 400,
        height: 240,
        color_depth: ColorDepth::Monochrome,
        supports_rotation: false,
        max_fps: 60,
        supports_brightness: true,
        supports_invert: false,
    };

    let layout = LayoutConfig::for_display(&caps);

    assert_eq!(layout.width, 400);
    assert_eq!(layout.height, 240);
    assert!(layout.weather.forecast_days >= 3);
}

#[test]
fn test_color_depth_variants() {
    let mono = ColorDepth::Monochrome;
    let gray = ColorDepth::Gray4;

    // Just ensure they're different
    assert_ne!(
        std::mem::discriminant(&mono),
        std::mem::discriminant(&gray)
    );
}
