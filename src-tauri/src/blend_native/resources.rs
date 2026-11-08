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
        // Blender maps bytes to floats with `b * (1/255)`, not `b / 255`;
        // the two differ by one ULP for about half of all byte values, which
        // is enough to flip the odd 8-bit rounding tie in the normal remap.
        const SCALE: f32 = 1.0 / 255.0;
        let mut value = [
            pixel[0] as f32 * SCALE,
            pixel[1] as f32 * SCALE,
            pixel[2] as f32 * SCALE,
            pixel[3] as f32 * SCALE,
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
            // numpy's `np.round` is half-to-even; matching it keeps the last
            // bit of every generated map identical to the plug-in oracle.
            bytes.push((value.clamp(0.0, 1.0) * 255.0).round_ties_even() as u8);
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

/// Port of `native_controls.office_room_sheet`.
///
/// Rebuilds the Office interior atlas: 4 room variants × 4 day/night states are
/// cut from 32×32 tiles of the source and laid out as a 16×16 grid of tiles.
/// Only one state keeps its alpha; the others are fully transparent.
fn office_room_sheet(source: &ImageF32) -> Result<ImageF32, String> {
    if source.height < 64 || source.width < 256 {
        return Err("Office room source is too small".into());
    }
    let width = source.width as usize;
    let mut pixels = vec![[0.0f32; 4]; 512 * 512];
    for row in 0..16usize {
        for variant in 0..4usize {
            for state in 0..4usize {
                let x = ((variant % 2) * 4 + state) * 32;
                let y = (variant / 2) * 32;
                for dy in 0..32 {
                    let source_row = y + dy;
                    for dx in 0..32 {
                        let mut pixel = source.pixels[source_row * width + x + dx];
                        if state != 1 {
                            pixel[3] = 0.0;
                        }
                        pixels[(row * 32 + dy) * 512 + (variant * 4 + state) * 32 + dx] = pixel;
                    }
                }
            }
        }
    }
    Ok(ImageF32::new(512, 512, pixels))
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
        // The plug-in reads pack images through `bpy.types.Image.pixels`,
        // which returns the **raw 8-bit buffer** for byte images (colour
        // management only applies at shader/render sampling).  Its `srgb()`
        // helper therefore encodes those raw values as if they were linear.
        // Decoding PNGs to linear here would darken every palette/interior
        // map, so all slots stay raw.
        let _ = slot;
        false
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
        // `resources.image()` rebuilds the Office room sheet in memory so the
        // 4 variants × 4 day/night states land in the layout the native room
        // shader expects.  The plug-in keys this off a custom property on the
        // packed image; a fresh load from disk always migrates.
        let image = if pack == "Office" && slot == SLOT_INTERIOR {
            office_room_sheet(&image)?
        } else {
            image
        };
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
        let image = ImageF32::new(2, 1, vec![[1.0, 0.5, 0.25, 1.0], [0.0, 0.25, 0.5, 0.0]]);
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
