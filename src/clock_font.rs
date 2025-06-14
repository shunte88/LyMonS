use embedded_graphics::{
    pixelcolor::BinaryColor,
    image::ImageRaw,
};

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
