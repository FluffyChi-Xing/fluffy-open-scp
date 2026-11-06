//! Pack atlas image access.
//!
//! This is the only I/O edge of the texture pipeline: files are decoded once
//! into linear float RGBA buffers; every consumer below is pure.
//!
//! Colour space mirrors the plug-in (`resources.image`): slots 4/5 are sRGB
//! and are converted to linear on read, slots 1/2/3 are Non-Color and stay
//! raw.  The recipe writer converts palette/interior back to sRGB on output.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const SLOT_TINT: u8 = 1;
pub const SLOT_NORMAL: u8 = 2;
pub const SLOT_SHADER: u8 = 3;
pub const SLOT_PALETTE: u8 = 4;
pub const SLOT_INTERIOR: u8 = 5;

#[derive(Debug, Clone)]
pub struct ImageF32 {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA, top-down, straight alpha.
    pub pixels: Vec<[f32; 4]>,
}

impl ImageF32 {
    pub fn new(width: u32, height: u32, pixels: Vec<[f32; 4]>) -> Self {
        debug_assert_eq!(pixels.len(), (width as usize) * (height as usize));
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn get(&self, x: u32, y: u32) -> [f32; 4] {
        self.pixels[(y as usize) * (self.width as usize) + x as usize]
    }

    pub fn row(&self, y: u32) -> &[[f32; 4]] {
        let width = self.width as usize;
        &self.pixels[(y as usize) * width..(y as usize + 1) * width]
    }
}

/// Decode a PNG into linear float RGBA (`srgb == true`) or raw RGBA.
pub fn decode_png(bytes: &[u8], srgb: bool) -> Result<ImageF32, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| format!("pack PNG decode failed: {error}"))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let mut pixels = Vec::with_capacity((width as usize) * (height as usize));
    for pixel in image.pixels() {
        let mut value = [
            pixel[0] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[2] as f32 / 255.0,
            pixel[3] as f32 / 255.0,
        ];
        if srgb {
            for channel in &mut value[..3] {
                *channel = srgb_to_linear(*channel);
            }
        }
        pixels.push(value);
    }
    Ok(ImageF32::new(width, height, pixels))
}

pub fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.max(0.0).powf(1.0 / 2.4) - 0.055
    }
}

/// Encode a float buffer as PNG, converting RGB to sRGB when requested.
pub fn encode_png(image: &ImageF32, srgb: bool) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity((image.width as usize) * (image.height as usize) * 4);
    for pixel in &image.pixels {
        for (channel, value) in pixel.iter().enumerate() {
            let value = if srgb && channel < 3 {
                linear_to_srgb(*value)
            } else {
                *value
            };
            bytes.push((value.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    let buffer = image::RgbaImage::from_raw(image.width, image.height, bytes)
        .ok_or("PNG buffer size mismatch")?;
    let mut png = Vec::new();
    buffer
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| format!("PNG encode failed: {error}"))?;
    Ok(png)
}

/// Filesystem-backed pack store (`assets/packs/<pack>/slotN.png`).
#[derive(Debug, Clone)]
pub struct PackStore {
    root: PathBuf,
    cache: Rc<RefCell<HashMap<(String, u8), ImageF32>>>,
}

impl PackStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            cache: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn slot_is_srgb(slot: u8) -> bool {
        slot == SLOT_PALETTE || slot == SLOT_INTERIOR
    }

    pub fn load(&self, pack: &str, slot: u8) -> Result<ImageF32, String> {
        let key = (pack.to_string(), slot);
        if let Some(image) = self.cache.borrow().get(&key) {
            return Ok(image.clone());
        }
        let path = self.root.join(pack).join(format!("slot{slot}.png"));
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("pack image {}: {error}", path.display()))?;
        let image = decode_png(&bytes, Self::slot_is_srgb(slot))?;
        self.cache.borrow_mut().insert(key, image.clone());
        Ok(image)
    }

    pub fn path(&self, pack: &str, slot: u8) -> PathBuf {
        self.root.join(pack).join(format!("slot{slot}.png"))
    }

    pub fn catalog_path(&self) -> PathBuf {
        self.root.join("catalog.json")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_round_trip_is_stable() {
        for value in [0.0, 0.05, 0.2, 0.5, 0.8, 1.0] {
            let linear = srgb_to_linear(value);
            let back = linear_to_srgb(linear);
            assert!((back - value).abs() < 1e-4, "{value} -> {linear} -> {back}");
        }
        assert!((srgb_to_linear(0.5) - 0.214_041).abs() < 1e-5);
    }

    #[test]
    fn png_round_trip_preserves_pixels() {
        let image = ImageF32::new(
            2,
            1,
            vec![[1.0, 0.5, 0.25, 1.0], [0.0, 0.25, 0.5, 0.0]],
        );
        let png = encode_png(&image, false).unwrap();
        let decoded = decode_png(&png, false).unwrap();
        assert_eq!(decoded.width, 2);
        for (left, right) in image.pixels.iter().zip(&decoded.pixels) {
            for channel in 0..4 {
                assert!((left[channel] - right[channel]).abs() < 1.0 / 255.0 + 1e-6);
            }
        }
    }
}
