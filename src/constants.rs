//! This module contains global constants used across the display and other modules.

// tags for track playing detail placement
// should move these into display - use same paradigm as status bar 
pub const TAG_DISPLAY_ALBUMARTIST: usize = 0;
pub const TAG_DISPLAY_ALBUM: usize = 1;
pub const TAG_DISPLAY_TITLE: usize = 2;
pub const TAG_DISPLAY_ARTIST: usize = 3;

/// The total width of the OLED display in pixels.
pub const DISPLAY_WIDTH: u32 = 128;
/// The total height of the OLED display in pixels.
pub const DISPLAY_HEIGHT: u32 = 64;

/// X-offset for the region where most text content and glyphs are displayed.
pub const DISPLAY_REGION_X_OFFSET: i32 = 0; // Starts from the left edge
/// Y-offset for the top of the displayable region.
pub const DISPLAY_REGION_Y_OFFSET: i32 = 0; // Starts from the top edge
/// The width of the displayable region for text and glyphs.
pub const DISPLAY_REGION_WIDTH: u32 = DISPLAY_WIDTH - 2; // Occupies adjusted width
/// The height of the displayable region for text and glyphs.
#[allow(dead_code)]
pub const DISPLAY_REGION_HEIGHT: u32 = DISPLAY_HEIGHT; // Occupies full height

/// Maximum number of lines that can be displayed on the screen.
/// Line 0 is for status, lines 1-4 are for scrolling text, line 5 is for player track info.
pub const MAX_LINES: usize = 6;

/// The height of the main font used for scrolling text (FONT_5X8).
pub const MAIN_FONT_HEIGHT: u32 = 8;
/// The vertical spacing between lines of main text.
pub const MAIN_LINE_SPACING: i32 = 2; // Additional pixels between lines

/// The width of standard 8x8 glyphs.
pub const GLYPH_WIDTH: u32 = 8;
/// The height of standard 8x8 glyphs.
#[allow(dead_code)]
pub const GLYPH_HEIGHT: u32 = 8;

// Clock display constants
/// Horizontal gap between clock digits (e.g., between HH, HM, MM segments).
pub const CLOCK_DIGIT_GAP_HORIZONTAL: i32 = 1; // For spacing between H and H, M and M, and H and colon
/// Horizontal gap specifically between the colon and the first minute digit.
pub const CLOCK_COLON_MINUTE_GAP: i32 = 0; // Wider gap for visual separation
/// Vertical gap between the clock digits and the seconds progress bar.
pub const CLOCK_PROGRESS_BAR_GAP: i32 = 4;
/// Vertical gap between the seconds progress bar and the date line.
pub const PROGRESS_BAR_DATE_GAP: i32 = 2;
/// Height of the font used for the date string (FONT_6X10).
pub const DATE_FONT_HEIGHT: u32 = 10;

// Player progress bar constants (Line 5)
/// Y-position for the player track progress bar.
pub const PLAYER_PROGRESS_BAR_Y_POS: i32 = 51; // Line 5 starts at this Y position
/// Width of the player track progress bar.
pub const PLAYER_PROGRESS_BAR_WIDTH: u32 = DISPLAY_WIDTH - 4; // 2 pixels padding on each side
/// Height of the player track progress bar.
pub const PLAYER_PROGRESS_BAR_HEIGHT: u32 = 4;
/// Thickness of the border for the player track progress bar.
pub const PLAYER_PROGRESS_BAR_BORDER_THICKNESS: u32 = 1;

/// Y-position for the player track info line (current time | mode | remaining time).
pub const PLAYER_TRACK_INFO_LINE_Y_POS: i32 = DISPLAY_HEIGHT as i32 - MAIN_FONT_HEIGHT as i32; // Bottom of the screen

pub const LYMONS_LOGO_WIDTH: u32 = 108;
#[allow(dead_code)]
pub const LYMONS_LOGO_HEIGHT: u32 = 44;

// Compact Cassette Easter Egg dimensions and positions
pub const CASSETTE_BODY_WIDTH: u32 = 100;
pub const CASSETTE_BODY_HEIGHT: u32 = 50;
pub const CASSETTE_BODY_X: i32 = ((DISPLAY_WIDTH - CASSETTE_BODY_WIDTH) / 2) as i32;
pub const CASSETTE_BODY_Y: i32 = ((DISPLAY_HEIGHT - CASSETTE_BODY_HEIGHT) / 2) as i32;
pub const CASSETTE_BODY_CORNER_RADIUS: u32 = 4;

pub const CASSETTE_HUB_RADIUS: u32 = 7; // Radius of the rotating hubs
pub const CASSETTE_HUB_LEFT_CENTER_X: i32 = CASSETTE_BODY_X + 25; // Center X for left hub
pub const CASSETTE_HUB_RIGHT_CENTER_X: i32 = CASSETTE_BODY_X + CASSETTE_BODY_WIDTH as i32 - 25; // Center X for right hub
pub const CASSETTE_HUB_CENTER_Y: i32 = CASSETTE_BODY_Y + 18; // Center Y for both hubs

pub const CASSETTE_TAPE_WINDOW_X: i32 = CASSETTE_BODY_X + 20;
pub const CASSETTE_TAPE_WINDOW_Y: i32 = CASSETTE_BODY_Y + 32;
pub const CASSETTE_TAPE_WINDOW_WIDTH: u32 = CASSETTE_BODY_WIDTH - 40; // 20px padding on each side
pub const CASSETTE_TAPE_WINDOW_HEIGHT: u32 = 10;
pub const CASSETTE_TAPE_WINDOW_BORDER_THICKNESS: u32 = 1;

pub const HUB_ROTATION_SPEED_DPS: f32 = 180.0; // Degrees per second for hub rotation
