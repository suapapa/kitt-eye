#![no_std]

use smart_leds::RGB8;

/// Number of LEDs on the strip (KnightRider reference used 12).
pub const LED_COUNT: usize = 8;

/// Default head color (KnightRider: red).
pub const BASE_COLOR: RGB8 = RGB8::new(255, 0, 0);

/// Fade retention per step (0..=255). Higher = slower trail fade.
pub const FADE_FACTOR: u8 = 180;

/// Milliseconds between animation steps.
pub const ANIM_SPEED_MS: u32 = 35;

/// Classic KITT bounce scanner with trailing fade.
pub struct KittScanner {
    pixels: [RGB8; LED_COUNT],
    pos: usize,
    dir: i8,
    color: RGB8,
}

impl KittScanner {
    pub const fn new(color: RGB8) -> Self {
        Self {
            pixels: [RGB8::new(0, 0, 0); LED_COUNT],
            pos: 0,
            dir: 1,
            color,
        }
    }

    /// Apply one fade + head step. Returns the pixel buffer to write out.
    pub fn step(&mut self) -> &[RGB8; LED_COUNT] {
        for pixel in &mut self.pixels {
            pixel.r = scale8(pixel.r, FADE_FACTOR);
            pixel.g = scale8(pixel.g, FADE_FACTOR);
            pixel.b = scale8(pixel.b, FADE_FACTOR);
        }

        self.pixels[self.pos] = self.color;

        let next = self.pos as i8 + self.dir;
        if next >= (LED_COUNT as i8 - 1) {
            self.pos = LED_COUNT - 1;
            self.dir = -1;
        } else if next <= 0 {
            self.pos = 0;
            self.dir = 1;
        } else {
            self.pos = next as usize;
        }

        &self.pixels
    }
}

#[inline]
fn scale8(value: u8, scale: u8) -> u8 {
    ((u16::from(value) * u16::from(scale)) / 255) as u8
}
