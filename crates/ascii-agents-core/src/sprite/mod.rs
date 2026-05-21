use std::collections::HashMap;

pub mod animator;
pub mod blit;
pub mod format;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// A single pixel: `Some(rgb)` or `None` (transparent).
pub type Pixel = Option<Rgb>;

#[derive(Debug, Clone, Default)]
pub struct Palette {
    map: HashMap<char, Pixel>,
}

impl Palette {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: char, pixel: Pixel) {
        self.map.insert(key, pixel);
    }

    pub fn get(&self, key: char) -> Option<Pixel> {
        self.map.get(&key).copied()
    }

    /// Replace one palette key's color — used for per-agent recoloring.
    pub fn with_override(&self, key: char, pixel: Pixel) -> Self {
        let mut out = self.clone();
        out.map.insert(key, pixel);
        out
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    /// Row-major, length = width * height.
    pub pixels: Vec<Pixel>,
}

#[derive(Debug, Clone)]
pub struct Sprite {
    pub frames: Vec<Frame>,
    pub frame_ms: u32,
}

/// A flat RGB buffer used as a blit target. Alpha is ignored — transparent
/// pixels leave the underlying buffer unchanged.
#[derive(Debug, Clone)]
pub struct RgbBuffer {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<Rgb>,
}

impl RgbBuffer {
    pub fn filled(width: u16, height: u16, fill: Rgb) -> Self {
        Self {
            width,
            height,
            pixels: vec![fill; (width as usize) * (height as usize)],
        }
    }

    pub fn get(&self, x: u16, y: u16) -> Rgb {
        self.pixels[(y as usize) * (self.width as usize) + (x as usize)]
    }

    pub fn put(&mut self, x: u16, y: u16, rgb: Rgb) {
        let i = (y as usize) * (self.width as usize) + (x as usize);
        self.pixels[i] = rgb;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_get_and_override() {
        let mut p = Palette::new();
        p.insert('B', Some(Rgb(0, 0, 255)));
        assert_eq!(p.get('B'), Some(Some(Rgb(0, 0, 255))));
        let p2 = p.with_override('B', Some(Rgb(255, 0, 0)));
        assert_eq!(p2.get('B'), Some(Some(Rgb(255, 0, 0))));
        assert_eq!(p.get('B'), Some(Some(Rgb(0, 0, 255))));
    }

    #[test]
    fn rgb_buffer_put_get_roundtrip() {
        let mut b = RgbBuffer::filled(3, 2, Rgb(0, 0, 0));
        b.put(1, 1, Rgb(10, 20, 30));
        assert_eq!(b.get(1, 1), Rgb(10, 20, 30));
        assert_eq!(b.get(0, 0), Rgb(0, 0, 0));
    }
}
