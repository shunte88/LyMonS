/*
 *  display/layout.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Adaptive layout system for different display resolutions
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

use crate::display::traits::{DisplayCapabilities, ColorDepth};

/// Layout configuration for different display resolutions
///
/// This provides adaptive layout parameters that scale UI elements
/// appropriately for different display sizes and aspect ratios.
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Display width in pixels
    pub width: u32,

    /// Display height in pixels
    pub height: u32,

    /// Color depth
    pub color_depth: ColorDepth,

    /// Layout category (determines which preset to use)
    pub category: LayoutCategory,

    /// Status bar configuration
    pub status_bar: StatusBarLayout,

    /// Content area configuration
    pub content_area: ContentAreaLayout,

    /// Font sizes
    pub fonts: FontSizes,

    /// Asset directory path
    pub asset_path: String,

    /// Visualizer panel dimensions
    pub visualizer: VisualizerLayout,

    /// Clock display layout
    pub clock: ClockLayout,

    /// Weather display layout
    pub weather: WeatherLayout,
}

/// Layout category based on display resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutCategory {
    /// Small displays (128x64 or similar)
    Small,

    /// Medium displays (128x64 to 256x64)
    Medium,

    /// Large displays (256x64)
    Large,

    /// Extra large displays (400x240 or higher)
    ExtraLarge,
}

/// Status bar layout configuration
#[derive(Debug, Clone)]
pub struct StatusBarLayout {
    /// Height of status bar in pixels
    pub height: u32,

    /// Y position (typically 0)
    pub y: u32,

    /// Icon size for status indicators
    pub icon_size: u32,

    /// Spacing between status elements
    pub spacing: u32,

    /// Font size for status text
    pub font_size: FontSize,
}

/// Content area layout (below status bar)
#[derive(Debug, Clone)]
pub struct ContentAreaLayout {
    /// Y position where content starts (after status bar)
    pub y: u32,

    /// Available height for content
    pub height: u32,

    /// Left margin
    pub margin_left: u32,

    /// Right margin
    pub margin_right: u32,

    /// Top margin (within content area)
    pub margin_top: u32,

    /// Bottom margin
    pub margin_bottom: u32,
}

/// Visualizer layout configuration
#[derive(Debug, Clone)]
pub struct VisualizerLayout {
    /// Width of visualizer area
    pub width: u32,

    /// Height of visualizer area
    pub height: u32,

    /// Number of VU meter segments (for segmented displays)
    pub vu_segments: u32,

    /// Peak meter height
    pub peak_height: u32,

    /// Histogram bar width
    pub hist_bar_width: u32,

    /// Whether to use wide layout
    pub is_wide: bool,
}

/// Clock display layout
#[derive(Debug, Clone)]
pub struct ClockLayout {
    /// Digit width for large clock digits
    pub digit_width: u32,

    /// Digit height for large clock digits
    pub digit_height: u32,

    /// Spacing between digits
    pub digit_spacing: u32,

    /// Date text font size
    pub date_font_size: FontSize,

    /// Y position for clock
    pub clock_y: u32,

    /// Y position for date
    pub date_y: u32,
}

/// Weather display layout
#[derive(Debug, Clone)]
pub struct WeatherLayout {
    /// Icon size for weather icons
    pub icon_size: u32,

    /// Font size for temperature
    pub temp_font_size: FontSize,

    /// Font size for condition text
    pub condition_font_size: FontSize,

    /// Font size for forecast
    pub forecast_font_size: FontSize,

    /// Number of forecast days to show
    pub forecast_days: u32,
}

/// Font size categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSize {
    /// Very small (4x6 or similar)
    Tiny,

    /// Small (5x8 or 6x10)
    Small,

    /// Medium (6x13 or 7x14)
    Medium,

    /// Large (custom/bitmap fonts)
    Large,

    /// Extra large (custom large fonts)
    ExtraLarge,
}

/// Font sizes for different UI elements
#[derive(Debug, Clone)]
pub struct FontSizes {
    /// Status bar text
    pub status: FontSize,

    /// Main content text (scrolling track info)
    pub content: FontSize,

    /// Time/duration text
    pub time: FontSize,

    /// Clock digits (uses bitmap fonts)
    pub clock_digits: FontSize,

    /// Weather information
    pub weather: FontSize,
}

impl LayoutConfig {
    /// Create a layout configuration from display capabilities
    ///
    /// This automatically selects the appropriate layout preset based on
    /// the display resolution and capabilities.
    pub fn for_display(capabilities: &DisplayCapabilities) -> Self {
        let width = capabilities.width;
        let height = capabilities.height;
        let color_depth = capabilities.color_depth;

        // Determine layout category based on resolution
        let category = Self::categorize_display(width, height);

        match category {
            LayoutCategory::Small => Self::small_layout(width, height, color_depth),
            LayoutCategory::Medium => Self::medium_layout(width, height, color_depth),
            LayoutCategory::Large => Self::large_layout(width, height, color_depth),
            LayoutCategory::ExtraLarge => Self::extra_large_layout(width, height, color_depth),
        }
    }

    /// Categorize display size
    fn categorize_display(width: u32, height: u32) -> LayoutCategory {
        match (width, height) {
            (w, h) if w >= 400 || h >= 240 => LayoutCategory::ExtraLarge,
            (w, h) if w >= 256 && h >= 64 => LayoutCategory::Large,
            (w, h) if w >= 132 && h >= 64 => LayoutCategory::Medium,
            _ => LayoutCategory::Small,
        }
    }

    /// Layout for small displays (128x64)
    fn small_layout(width: u32, height: u32, color_depth: ColorDepth) -> Self {
        Self {
            width,
            height,
            color_depth,
            category: LayoutCategory::Small,
            status_bar: StatusBarLayout {
                height: 8,
                y: 0,
                icon_size: 6,
                spacing: 2,
                font_size: FontSize::Tiny,
            },
            content_area: ContentAreaLayout {
                y: 8,
                height: height - 8,
                margin_left: 1,
                margin_right: 1,
                margin_top: 1,
                margin_bottom: 1,
            },
            fonts: FontSizes {
                status: FontSize::Tiny,
                content: FontSize::Small,
                time: FontSize::Tiny,
                clock_digits: FontSize::Large,
                weather: FontSize::Small,
            },
            asset_path: "./assets/ssd1309/".to_string(), // 128-width assets
            visualizer: VisualizerLayout {
                width: 128,
                height: 56,
                vu_segments: 10,
                peak_height: 8,
                hist_bar_width: 2,
                is_wide: false,
            },
            clock: ClockLayout {
                digit_width: 17,
                digit_height: 44,
                digit_spacing: 2,
                date_font_size: FontSize::Small,
                clock_y: 2,
                date_y: 55,
            },
            weather: WeatherLayout {
                icon_size: 32,
                temp_font_size: FontSize::Medium,
                condition_font_size: FontSize::Small,
                forecast_font_size: FontSize::Tiny,
                forecast_days: 3,
            },
        }
    }

    /// Layout for medium displays (132x64)
    fn medium_layout(width: u32, height: u32, color_depth: ColorDepth) -> Self {
        let mut layout = Self::small_layout(width, height, color_depth);
        layout.category = LayoutCategory::Medium;
        layout.visualizer.width = width;
        layout
    }

    /// Layout for large displays (256x64)
    fn large_layout(width: u32, height: u32, color_depth: ColorDepth) -> Self {
        Self {
            width,
            height,
            color_depth,
            category: LayoutCategory::Large,
            status_bar: StatusBarLayout {
                height: 10,
                y: 0,
                icon_size: 8,
                spacing: 3,
                font_size: FontSize::Small,
            },
            content_area: ContentAreaLayout {
                y: 10,
                height: height - 10,
                margin_left: 2,
                margin_right: 2,
                margin_top: 2,
                margin_bottom: 2,
            },
            fonts: FontSizes {
                status: FontSize::Small,
                content: FontSize::Medium,
                time: FontSize::Small,
                clock_digits: FontSize::ExtraLarge,
                weather: FontSize::Medium,
            },
            asset_path: "./assets/ssd1322/".to_string(), // 256-width assets
            visualizer: VisualizerLayout {
                width: 256,
                height: 54,
                vu_segments: 20,
                peak_height: 12,
                hist_bar_width: 4,
                is_wide: true,
            },
            clock: ClockLayout {
                digit_width: 34,
                digit_height: 44,
                digit_spacing: 3,
                date_font_size: FontSize::Medium,
                clock_y: 10,
                date_y: 55,
            },
            weather: WeatherLayout {
                icon_size: 48,
                temp_font_size: FontSize::Large,
                condition_font_size: FontSize::Medium,
                forecast_font_size: FontSize::Small,
                forecast_days: 3,
            },
        }
    }

    /// Layout for extra large displays (400x240)
    fn extra_large_layout(width: u32, height: u32, color_depth: ColorDepth) -> Self {
        Self {
            width,
            height,
            color_depth,
            category: LayoutCategory::ExtraLarge,
            status_bar: StatusBarLayout {
                height: 20,
                y: 0,
                icon_size: 16,
                spacing: 5,
                font_size: FontSize::Medium,
            },
            content_area: ContentAreaLayout {
                y: 20,
                height: height - 20,
                margin_left: 5,
                margin_right: 5,
                margin_top: 5,
                margin_bottom: 5,
            },
            fonts: FontSizes {
                status: FontSize::Medium,
                content: FontSize::Large,
                time: FontSize::Medium,
                clock_digits: FontSize::ExtraLarge,
                weather: FontSize::Large,
            },
            asset_path: "./assets/sharp400/".to_string(), // 400-width assets
            visualizer: VisualizerLayout {
                width: 400,
                height: 220,
                vu_segments: 40,
                peak_height: 20,
                hist_bar_width: 8,
                is_wide: true,
            },
            clock: ClockLayout {
                digit_width: 68,
                digit_height: 120,
                digit_spacing: 5,
                date_font_size: FontSize::Large,
                clock_y: 30,
                date_y: 180,
            },
            weather: WeatherLayout {
                icon_size: 96,
                temp_font_size: FontSize::ExtraLarge,
                condition_font_size: FontSize::Large,
                forecast_font_size: FontSize::Medium,
                forecast_days: 5, // More room for forecast
            },
        }
    }

    /// Get the asset path for a specific asset type
    pub fn asset_path_for(&self, asset_type: AssetType) -> String {
        match asset_type {
            AssetType::Weather => {
                // Weather icons are in basic/ or mono/ subdirectories
                match self.color_depth {
                    ColorDepth::Monochrome => "./assets/mono/".to_string(),
                    ColorDepth::Gray4 => "./assets/basic/".to_string(),
                }
            }
            AssetType::Visualizer => {
                self.asset_path.clone()
            }
            AssetType::EasterEgg => {
                "./assets/".to_string()
            }
        }
    }

    /// Calculate scaled value based on display width
    ///
    /// Scales a value from a reference width (128) to the current display width
    pub fn scale_width(&self, reference_value: u32) -> u32 {
        (reference_value * self.width) / 128
    }

    /// Calculate scaled value based on display height
    ///
    /// Scales a value from a reference height (64) to the current display height
    pub fn scale_height(&self, reference_value: u32) -> u32 {
        (reference_value * self.height) / 64
    }

    /// Get recommended scroll speed based on display width
    pub fn scroll_speed(&self) -> u32 {
        match self.category {
            LayoutCategory::Small => 1,
            LayoutCategory::Medium => 1,
            LayoutCategory::Large => 2,
            LayoutCategory::ExtraLarge => 3,
        }
    }

    /// Get recommended frame rate based on display capabilities
    pub fn recommended_fps(&self) -> u32 {
        match self.category {
            LayoutCategory::Small | LayoutCategory::Medium => 30,
            LayoutCategory::Large => 60,
            LayoutCategory::ExtraLarge => 60,
        }
    }

    /// Check if this layout supports grayscale
    pub fn supports_grayscale(&self) -> bool {
        matches!(self.color_depth, ColorDepth::Gray4)
    }
}

/// Asset type for path resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    /// Weather icons
    Weather,

    /// Visualizer panels and backgrounds
    Visualizer,

    /// Easter egg animations
    EasterEgg,
}

/// Helper function to get layout for specific resolution
pub fn layout_for_resolution(width: u32, height: u32, color_depth: ColorDepth) -> LayoutConfig {
    let capabilities = DisplayCapabilities {
        width,
        height,
        color_depth,
        supports_rotation: false,
        max_fps: 60,
        supports_brightness: true,
        supports_invert: false,
    };

    LayoutConfig::for_display(&capabilities)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_layout() {
        let layout = layout_for_resolution(128, 64, ColorDepth::Monochrome);
        assert_eq!(layout.category, LayoutCategory::Small);
        assert_eq!(layout.visualizer.is_wide, false);
        assert!(layout.asset_path.contains("ssd1309"));
    }

    #[test]
    fn test_large_layout() {
        let layout = layout_for_resolution(256, 64, ColorDepth::Gray4);
        assert_eq!(layout.category, LayoutCategory::Large);
        assert_eq!(layout.visualizer.is_wide, true);
        assert!(layout.asset_path.contains("ssd1322"));
    }

    #[test]
    fn test_extra_large_layout() {
        let layout = layout_for_resolution(400, 240, ColorDepth::Monochrome);
        assert_eq!(layout.category, LayoutCategory::ExtraLarge);
        assert_eq!(layout.weather.forecast_days, 5);
        assert!(layout.asset_path.contains("sharp400"));
    }

    #[test]
    fn test_scaling() {
        let layout = layout_for_resolution(256, 64, ColorDepth::Monochrome);
        assert_eq!(layout.scale_width(64), 128); // 64 * 256 / 128 = 128
        assert_eq!(layout.scale_height(32), 32); // Same height
    }

    #[test]
    fn test_asset_paths() {
        let mono_layout = layout_for_resolution(128, 64, ColorDepth::Monochrome);
        assert!(mono_layout.asset_path_for(AssetType::Weather).contains("mono"));

        let gray_layout = layout_for_resolution(256, 64, ColorDepth::Gray4);
        assert!(gray_layout.asset_path_for(AssetType::Weather).contains("basic"));
    }
}
