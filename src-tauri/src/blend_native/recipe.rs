//! Per-material texture generation (port of `native_asset.recipe`).
//!
//! Pure: the caller supplies the source pack images and the resolved authoring
//! state; the function returns the five LOTM maps plus the 4×float4 parameter
//! table.  Material grouping/deduplication belongs to `export`.

use super::catalog::{Catalog, Entry};
use super::resources::{
    ImageF32, PackStore, SLOT_INTERIOR, SLOT_NORMAL, SLOT_PALETTE, SLOT_SHADER, SLOT_TINT,
    encode_png, linear_to_srgb, srgb_to_linear,
};
use super::settings::SurfaceState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Body,
    Window,
    Decor,
    Motif,
}

#[derive(Debug, Clone)]
pub struct RecipeInput<'a> {
    pub kind: Kind,
    pub entry: Option<&'a Entry>,
    pub color: [f32; 4],
    pub normal: f64,
    /// Effective palette row after the plug-in's `_source_*` overrides.
    pub palette_row: f64,
    /// `_night_variant`: 0 = dark window, 1..=4 = lit colour.
    pub variant: Option<usize>,
    pub relief: f64,
    pub night_variation: f64,
    pub glow: f64,
    pub texture_contrast: f64,
    pub shared_room: bool,
    pub base_role: &'a str,
    pub wall_color: [f32; 4],
    pub roof_color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct RecipeOutput {
    /// tint, normal, shader, palette, interior (LOTM container order).
    pub maps: [Vec<u8>; 5],
    /// 4 rows of `[pal_origin, base_rect, top_rect, inverse]` floats.
    pub params: [f32; 16],
    pub param_cols: u32,
}

const FLAT_SHADER: f32 = 23.0 / 255.0;

pub fn build(
    store: &PackStore,
    catalog: &Catalog,
    input: &RecipeInput<'_>,
) -> Result<RecipeOutput, String> {
    let pack = input.entry.map(|entry| entry.pack.clone());
    let rect = input
        .entry
        .map(|entry| entry.rect)
        .unwrap_or([1.0, 1.0, 0.0, 0.0]);
    let mut palette_row = input.palette_row;
    let mut active = palette_row >= 0.0;
    if input.kind == Kind::Decor {
        active = false;
    }
    if input.kind == Kind::Window && !active {
        palette_row = pack
            .as_deref()
            .and_then(|pack| catalog.pack(pack))
            .map(|pack| pack.palette_row)
            .unwrap_or(0.0);
        active = true;
    }

    let Some(pack) = pack else {
        return no_pack(input, rect);
    };
    let palette = store.load(&pack, SLOT_PALETTE)?;
    let mut tint = store.load(&pack, SLOT_TINT)?;
    let mut normal = store.load(&pack, SLOT_NORMAL)?;
    let mut shader = store.load(&pack, SLOT_SHADER)?;
    let mut interior = store.load(&pack, SLOT_INTERIOR)?;

    // Palette: shift the first 14 rows by the fractional palette row, multiply
    // by the material colour, then re-encode to sRGB.
    let mut rgb = vec![[0.0f32; 3]; palette.pixels.len()];
    let mut alpha = vec![0.0f32; palette.pixels.len()];
    for (index, pixel) in palette.pixels.iter().enumerate() {
        rgb[index] = [pixel[0], pixel[1], pixel[2]];
        alpha[index] = pixel[3];
    }
    let offset = if active {
        (palette_row.fract() * 16.0 + 1e-6).floor() as i64
    } else {
        0
    };
    // `np.roll` shifts from a snapshot, so wrapped source rows must not read
    // values this loop has already overwritten.
    let palette_source = rgb.clone();
    for row in 0..14 {
        for column in 0..palette.width as usize {
            let target = row * palette.width as usize + column;
            let shifted = if active {
                let source = ((row as i64 + offset).rem_euclid(16)) as usize
                    * palette.width as usize
                    + column;
                palette_source[source]
            } else {
                // Inactive palettes still get the material colour: the plug-in
                // writes a constant 1.0 row and then multiplies by `color`.
                [1.0, 1.0, 1.0]
            };
            rgb[target] = [
                shifted[0] * input.color[0],
                shifted[1] * input.color[1],
                shifted[2] * input.color[2],
            ];
        }
    }
    if input.kind == Kind::Decor {
        for row in 14..16 {
            for column in 0..palette.width as usize {
                rgb[row * palette.width as usize + column][0] = 0.0;
            }
        }
    }

    // Relief/specular remap on the shader map (ports `relief_field`).
    if matches!(input.kind, Kind::Window | Kind::Decor | Kind::Motif) {
        // Height is an atlas-domain distance: the plug-in scales the UI depth by
        // the tile's shorter side, so the same slider looks equal on every pack.
        let strength = input.relief.max(0.0) as f32;
        let scale = ((rect[0].abs() as f32) * 0.5).min(rect[1].abs() as f32);
        let gain = strength * scale / 0.16;
        for pixel in &mut shader.pixels {
            pixel[0] = (FLAT_SHADER + (pixel[0] - FLAT_SHADER) * gain).clamp(0.0, 1.0);
            pixel[1] = (pixel[1] / gain.max(1e-6)).clamp(0.0, 1.0);
        }
    }
    match input.kind {
        Kind::Body => {
            let mean = input
                .entry
                .and_then(|entry| entry.shade_mean)
                .unwrap_or(0.5)
                .max(0.03) as f32;
            let contrast = input.texture_contrast as f32;
            for index in 0..tint.pixels.len() {
                let factor = (1.0
                    + (tint.pixels[index][2] * normal.pixels[index][3] - mean) / mean * contrast)
                    .clamp(0.25, 1.6);
                tint.pixels[index][2] = factor * 0.5;
                normal.pixels[index][3] = 1.0;
            }
        }
        Kind::Decor => {
            for index in 0..tint.pixels.len() {
                tint.pixels[index][0] = 0.0;
                tint.pixels[index][1] = 0.0;
                tint.pixels[index][2] = 0.5;
                normal.pixels[index][3] = 1.0;
            }
        }
        _ => {}
    }
    if input.kind != Kind::Window {
        if input.kind != Kind::Motif {
            for pixel in &mut tint.pixels {
                pixel[3] = 1.0;
            }
        }
        for pixel in &mut shader.pixels {
            pixel[3] = 1.0;
        }
    }

    // Game-packed normal encoding.
    let strength = input.normal.max(0.0) as f32;
    for pixel in &mut normal.pixels {
        let x = (pixel[0] * 2.0 - 1.0) * strength / 1.5;
        let y = (pixel[1] * 2.0 - 1.0) * strength / 1.5;
        pixel[0] = (x * 0.5 + 0.5).clamp(0.0, 1.0);
        pixel[1] = (y * 0.5 + 0.5).clamp(0.0, 1.0);
        pixel[2] = ((1.0 - (x * x + y * y).min(1.0)).max(0.0).sqrt() + 1.0) * 0.5;
    }

    // Interior atlas: sRGB encode and glow-scaled alpha.
    for pixel in &mut interior.pixels {
        pixel[0] = linear_to_srgb(pixel[0]);
        pixel[1] = linear_to_srgb(pixel[1]);
        pixel[2] = linear_to_srgb(pixel[2]);
        pixel[3] *= ((input.glow * 2.0).max(0.0)) as f32;
    }

    let entry = input.entry;
    let source_row = if input.kind == Kind::Window {
        catalog.window_recipe(entry, &pack)
    } else {
        entry.and_then(|entry| {
            catalog
                .pack(&pack)
                .and_then(|pack| pack.rows.first())
                .and_then(|row| row.get(entry.recipe))
                .filter(|row| row.len() >= 4)
                .map(|row| [row[0], row[1], row[2], row[3]])
        })
    }
    .ok_or_else(|| format!("pack {pack} has no recipe row for this material"))?;
    let pal_offset = if entry.and_then(|entry| entry.recipe_layer.as_deref()) == Some("上层") {
        source_row[1]
    } else {
        source_row[0]
    };
    let mut pal_origin = [pal_offset, pal_offset, source_row[2], source_row[3]];
    if input.kind != Kind::Window {
        pal_origin[2] = 0.0;
        pal_origin[3] = 0.0;
    }
    let mut base_rect = rect;
    let mut top_rect = [0.0, 0.0, 0.0, 0.0];

    if matches!(input.kind, Kind::Window | Kind::Decor | Kind::Motif) {
        // One extra palette column carries the Base-layer fallback colour.
        let mut used = vec![false; 512];
        for pixel in &tint.pixels {
            let index = ((pixel[0] * 255.0).round() as i64 + (pal_offset * 512.0).round() as i64)
                .rem_euclid(512);
            used[index as usize] = true;
        }
        let column = (0..512)
            .rev()
            .find(|index| !used[*index])
            .ok_or("palette has no free column for the base fallback")?;
        let base_color = if input.kind == Kind::Decor {
            input.color
        } else if input.base_role == "ROOF" {
            input.roof_color
        } else {
            input.wall_color
        };
        let width = palette.width as usize;
        for row in 0..14 {
            let target = row * width + column;
            rgb[target] = [base_color[0], base_color[1], base_color[2]];
        }
        for row in 14..16 {
            for channel in 0..3 {
                rgb[row * width + column][channel] = 0.0;
            }
        }
        extend_strip(&mut tint, [0.0, 0.0, 0.5, 1.0]);
        extend_strip(&mut normal, [0.5, 0.5, 1.0, 1.0]);
        extend_strip(&mut shader, [0.0, 1.0, 0.0, 1.0]);
        pal_origin[0] = column as f64 / 512.0;
        let [rw, rh, rx, ry] = rect;
        top_rect = [rw * 0.5, rh, rx * 0.5, ry];
        base_rect = [rw.abs() * 0.5, rh.abs(), 0.5, 0.0];
        if input.relief <= 0.0 && !input.shared_room {
            base_rect = top_rect;
            top_rect = [0.0, 0.0, 0.0, 0.0];
            pal_origin[0] = pal_offset;
        }
    }

    if input.kind == Kind::Body {
        // 1.23.7 body branch: the native relief pass does not branch on a zero
        // Top size, so a transparent constant strip keeps Base sampling.
        extend_strip(&mut tint, [0.0, 0.0, 0.5, 0.0]);
        extend_strip(&mut normal, [0.5, 0.5, 1.0, 1.0]);
        extend_strip(&mut shader, [FLAT_SHADER, 1.0, 0.0, 1.0]);
        base_rect = [rect[0] * 0.5, rect[1], rect[2] * 0.5, rect[3]];
        top_rect = [0.5, 1.0, 0.5, 0.0];
    }

    if input.kind == Kind::Window {
        // Source palette self-light must not bypass per-window occupancy.
        // numpy's `rgb[14:]` covers both remaining rows.
        for row in 14..16 {
            for column in 0..palette.width as usize {
                rgb[row * palette.width as usize + column][0] = 0.0;
            }
        }
        match input.variant {
            Some(0) => {
                for pixel in &mut interior.pixels {
                    pixel[3] = 0.0;
                }
            }
            Some(variant) if variant > 0 => {
                let amount = input.night_variation as f32;
                let hue = super::appearance::COLORS[(variant - 1).min(3)];
                let tint = [
                    hue[0] * amount + 1.0 * (1.0 - amount),
                    hue[1] * amount + 0.92 * (1.0 - amount),
                    hue[2] * amount + 0.78 * (1.0 - amount),
                ];
                for pixel in &mut interior.pixels {
                    pixel[0] = linear_to_srgb(srgb_to_linear(pixel[0]) * tint[0]);
                    pixel[1] = linear_to_srgb(srgb_to_linear(pixel[1]) * tint[1]);
                    pixel[2] = linear_to_srgb(srgb_to_linear(pixel[2]) * tint[2]);
                }
            }
            _ => {}
        }
    }

    let palette_png = encode_srgb_palette(&rgb, &alpha, palette.width, palette.height)?;
    let maps = [
        encode_png(&tint, false)?,
        encode_png(&normal, false)?,
        encode_png(&shader, false)?,
        palette_png,
        encode_png(&interior, false)?,
    ];
    let params = params_table(pal_origin, base_rect, top_rect);
    Ok(RecipeOutput {
        maps,
        params,
        param_cols: 1,
    })
}

fn no_pack(input: &RecipeInput<'_>, rect: [f64; 4]) -> Result<RecipeOutput, String> {
    let tint = solid_png([0, 0, 128, 255])?;
    let normal = solid_png([128, 128, 255, 255])?;
    let shader = solid_png([0, 80, 80, 255])?;
    let palette = flat_palette(input.color)?;
    let interior = solid_png([60, 50, 40, 0])?;
    let params = params_table([0.0, 0.0, 0.0, 0.0], rect, [0.0; 4]);
    Ok(RecipeOutput {
        maps: [tint, normal, shader, palette, interior],
        params,
        param_cols: 1,
    })
}

fn params_table(pal_origin: [f64; 4], base_rect: [f64; 4], top_rect: [f64; 4]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for (index, value) in pal_origin.iter().enumerate() {
        out[index] = *value as f32;
    }
    for (index, value) in base_rect.iter().enumerate() {
        out[4 + index] = *value as f32;
    }
    for (index, value) in top_rect.iter().enumerate() {
        out[8 + index] = *value as f32;
    }
    out[12] = 0.0;
    out[13] = 0.0;
    out[14] = 1.0 / base_rect[0].abs().max(1e-7) as f32;
    out[15] = 1.0 / base_rect[1].abs().max(1e-7) as f32;
    out
}

fn extend_strip(image: &mut ImageF32, pixel: [f32; 4]) {
    // native_asset.extend() concatenates a full-width constant strip, not a
    // single column: 256 -> 512.  The Top/Base atlas transform relies on this
    // doubled width.
    let width = image.width as usize;
    let height = image.height as usize;
    let mut pixels = Vec::with_capacity(width * 2 * height);
    for row in 0..height {
        pixels.extend_from_slice(&image.pixels[row * width..(row + 1) * width]);
        pixels.extend(std::iter::repeat(pixel).take(width));
    }
    image.width *= 2;
    image.pixels = pixels;
}

fn encode_srgb_palette(
    rgb: &[[f32; 3]],
    alpha: &[f32],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let mut pixels = Vec::with_capacity(rgb.len());
    for (index, color) in rgb.iter().enumerate() {
        pixels.push([
            linear_to_srgb(color[0]),
            linear_to_srgb(color[1]),
            linear_to_srgb(color[2]),
            alpha[index],
        ]);
    }
    encode_png(&ImageF32::new(width, height, pixels), false)
}

fn flat_palette(color: [f32; 4]) -> Result<Vec<u8>, String> {
    let mut pixels = vec![[0.0f32; 4]; 16 * 512];
    let srgb = [
        linear_to_srgb(color[0]),
        linear_to_srgb(color[1]),
        linear_to_srgb(color[2]),
        1.0,
    ];
    for row in 0..16 {
        for column in 0..512 {
            pixels[row * 512 + column] = if row < 14 { srgb } else { [0.0, 0.0, 0.0, 1.0] };
        }
    }
    encode_png(&ImageF32::new(512, 16, pixels), false)
}

fn solid_png(pixel: [u8; 4]) -> Result<Vec<u8>, String> {
    let value = [
        pixel[0] as f32 / 255.0,
        pixel[1] as f32 / 255.0,
        pixel[2] as f32 / 255.0,
        pixel[3] as f32 / 255.0,
    ];
    encode_png(&ImageF32::new(1, 1, vec![value]), false)
}

/// Effective palette row after the exporter's `_source_*` overrides.
pub fn effective_palette_row(
    state: &SurfaceState,
    kind: Kind,
    source_window_palette: Option<f64>,
    source_body_palette: Option<f64>,
    source_body_enabled: bool,
) -> f64 {
    match kind {
        Kind::Window => source_window_palette.unwrap_or(state.palette_row),
        Kind::Body if source_body_enabled => source_body_palette.unwrap_or(state.palette_row),
        _ => state.palette_row,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_table_matches_layout() {
        let params = params_table([0.25, 0.5, 0.75, 0.125], [0.5, 0.25, 0.0, 0.0], [0.0; 4]);
        assert_eq!(params[0], 0.25);
        assert_eq!(params[4], 0.5);
        assert!((params[14] - 2.0).abs() < 1e-6);
        assert!((params[15] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn effective_palette_row_prefers_source_overrides() {
        let state = SurfaceState::default();
        assert_eq!(
            effective_palette_row(&state, Kind::Window, Some(4.0), None, false),
            4.0
        );
        assert_eq!(
            effective_palette_row(&state, Kind::Body, None, Some(9.6), true),
            9.6
        );
        assert_eq!(
            effective_palette_row(&state, Kind::Body, None, Some(9.6), false),
            -1.0
        );
    }
}
