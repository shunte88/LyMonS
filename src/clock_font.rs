use embedded_graphics::{
    pixelcolor::BinaryColor,
    image::ImageRaw,
};
use crate::imgdata::get_glyph_slice;
use log::{info};

// Define the dimensions of our custom clock digits.
// User specified: 15 pixels wide, 44 pixels high.
pub const CLK_DIGIT_WIDTH: u32 = 25;
pub const CLK_DIGIT_HEIGHT: u32 = 44;

/// A struct that encapsulates all necessary data for a specific clock font.
/// This allows different clock fonts to be loaded and used interchangeably.
pub struct ClockFontData<'a> {
    pub digit_width: u32,
    pub digit_height: u32,
    // Store ImageRaw references directly for '0' through '9'
    digits: [ImageRaw<'a, BinaryColor>; 10], 
    colon: ImageRaw<'a, BinaryColor>,
    space: ImageRaw<'a, BinaryColor>,
    minus: ImageRaw<'a, BinaryColor>,
}

impl<'a> ClockFontData<'a> {
    /// Constructs a new `ClockFontData` instance from pre-sliced `ImageRaw` data.
    /// This constructor is generic and can be used by any font source.
    pub fn new(
        digit_width: u32,
        digit_height: u32,
        digits: [ImageRaw<'a, BinaryColor>; 10],
        colon: ImageRaw<'a, BinaryColor>,
        space: ImageRaw<'a, BinaryColor>,
        minus: ImageRaw<'a, BinaryColor>,
    ) -> Self {
        ClockFontData {
            digit_width,
            digit_height,
            digits,
            colon,
            space,
            minus,
        }
    }

    /// Retrieves the `ImageRaw` for a given character from this font.
    pub fn get_char_image_raw(&self, character: char) -> Option<&ImageRaw<'a, BinaryColor>> {
        match character {
            '0'..='9' => self.digits.get(character.to_digit(10).unwrap() as usize),
            ':' => Some(&self.colon),
            ' ' => Some(&self.space),
            '-' => Some(&self.minus),
            _ => None, // Character not supported by this font
        }
    }
}

/// Initializes and returns a `ClockFontData` instance for the 7-segment clock font.
fn new_clock_font(raw_font: &'static [u8]) -> ClockFontData<'static> {

    let mut digits: [ImageRaw<'static, BinaryColor>; 10] = [
        ImageRaw::<BinaryColor>::new(&[], 0); // Initialize with dummy values
        10
    ];
    for i in 0..10 {
        digits[i] = ImageRaw::<BinaryColor>::new(
            get_glyph_slice(raw_font,i, CLK_DIGIT_WIDTH, CLK_DIGIT_HEIGHT), CLK_DIGIT_WIDTH);
    }
    let colon = ImageRaw::<BinaryColor>::new(
        get_glyph_slice(raw_font,10, CLK_DIGIT_WIDTH, CLK_DIGIT_HEIGHT), CLK_DIGIT_WIDTH);
    let space = ImageRaw::<BinaryColor>::new(
        get_glyph_slice(raw_font,11, CLK_DIGIT_WIDTH, CLK_DIGIT_HEIGHT), CLK_DIGIT_WIDTH);
    let minus = ImageRaw::<BinaryColor>::new(
        get_glyph_slice(raw_font,12, CLK_DIGIT_WIDTH, CLK_DIGIT_HEIGHT), CLK_DIGIT_WIDTH);

    ClockFontData::new(
        CLK_DIGIT_WIDTH,
        CLK_DIGIT_HEIGHT,
        digits,
        colon,
        space,
        minus,
    )

}

/// Sets the clock display font
pub fn set_clock_font(font_name: &str) -> ClockFontData<'static> {
    info!("Load font: {}",font_name);
    match font_name {
        "space1999" => {
            new_clock_font(include_bytes!("../data/space1999_12x20.bin"))
            },
        "holfestus" => {
            new_clock_font(include_bytes!("../data/holfestus_12x20.bin"))
            },
        "solfestus" => {
            new_clock_font(include_bytes!("../data/solfestus_12x20.bin"))
            },
        "holdeco" => {
            new_clock_font(include_bytes!("../data/holdeco_12x20.bin"))
            },
        "soldeco" => {
            new_clock_font(include_bytes!("../data/soldeco_12x20.bin"))
            },
        "noto" => {
            new_clock_font(include_bytes!("../data/noto_12x20.bin"))
            },
        "roboto" => {
            new_clock_font(include_bytes!("../data/roboto_12x20.bin"))
            },
        "7seg" => {
            new_clock_font(include_bytes!("../data/7seg_12x20.bin"))
        },
        _ => {
            new_clock_font(include_bytes!("../data/7seg_12x20.bin"))
        }
    }

}

