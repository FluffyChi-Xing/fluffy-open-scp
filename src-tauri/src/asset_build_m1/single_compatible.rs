//! Pure-Rust input normalization for the author's single-mesh compiler.
//!
//! The author's `compile_tower_asset.py` accepts at most 64 material groups.
//! This module reproduces its first step exactly: hash the six-slot surface
//! payload plus the Float32 parameter block, keep one material per unique
//! combination, and remap every mesh reference. It deliberately does not touch
//! pixels or parameters yet; that is the next stage of the port.

use image::RgbaImage;
use sha2::{Digest, Sha256};

use super::lotm::{LotmDocument, LotmMaterial, MeshMaterialRef};

/// prepare_asset 0.8.6: `require(count<=128,'Palette profile supports at most
/// 128 source groups')`; the parameter table width is the next power of two.
pub const MAX_MATERIAL_ROWS: usize = 128;
pub const CELL: u32 = 512;

/// Layout of the author's single-mesh surface/palette/room tables.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfacePlan {
    pub cell: u32,
    /// Parameter table width: next power of two of the source group count
    /// (`width=2**int(np.ceil(np.log2(count)))`).
    pub width: u32,
    /// Power-of-two cell grid edge, matching `2**ceil(log2(ceil(sqrt(n))))`.
    pub grid: u32,
    pub atlas: u32,
    /// Unique (tint, normal, shader) group selected for each material row.
    pub surface_of_row: Vec<usize>,
    /// Per-row 4x4 Float32 block after palette/region rewriting.
    pub rows: Vec<[f32; 16]>,
    /// Per-row source Top-region presence (row2 cols 0/1 BEFORE the fallback
    /// rewrite) — the relief gain applies only to materials whose source Top
    /// region is non-empty (author `if j==2 and np.any(p[2,:2])`).
    pub top_enabled_by_row: Vec<bool>,
    pub room_keys: Vec<String>,
    pub room_atlas: u32,
    pub room_of_row: Vec<usize>,
    /// Source palette columns [Base, Top] per material row.
    pub palette_source: Vec<[u32; 2]>,
    /// Original room projection parameters (x scale, y offset) per room.
    pub room_params: Vec<[f32; 2]>,
}

fn f32_params(material: &LotmMaterial) -> Result<[f32; 16], String> {
    let bytes = material
        .params_f32
        .as_deref()
        .ok_or("author materials: material has no Float32 params")?;
    if bytes.len() != 64 {
        return Err(format!(
            "author materials: params are {} bytes, expected exactly 64 for param_cols=1",
            bytes.len()
        ));
    }
    let mut values = [0f32; 16];
    for (index, value) in values.iter_mut().enumerate() {
        *value = f32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap());
        if !value.is_finite() {
            return Err("author materials: non-finite source parameter".into());
        }
    }
    Ok(values)
}

fn surface_key(material: &LotmMaterial) -> String {
    let png = |index: usize| material.pngs.get(index).and_then(Option::as_ref);
    format!(
        "{}:{}:{}",
        digest(png(LotmMaterial::PNG_TINT)),
        digest(png(LotmMaterial::PNG_NORMAL)),
        digest(png(LotmMaterial::PNG_SHADER)),
    )
}

fn room_key(material: &LotmMaterial, params: &[f32; 16]) -> String {
    // Python converts these Float32 values to float and uses exact numeric
    // equality. Decimal formatting can merge distinct values; normalized bits
    // cannot, while still unifying -0.0 and +0.0 like Python.
    let key = |value: f32| if value == 0.0 { 0 } else { value.to_bits() };
    format!(
        "{}:{:08x}:{:08x}",
        digest(
            material
                .pngs
                .get(LotmMaterial::PNG_INTERIOR)
                .and_then(Option::as_ref)
        ),
        key(params[2]),
        key(params[3]),
    )
}

fn power_of_two_grid(unique: usize) -> u32 {
    let root = (unique as f64).sqrt().ceil().max(1.0) as u32;
    let mut grid = 1u32;
    while grid < root {
        grid *= 2;
    }
    grid
}

/// Reproduce the author's surface/palette/room parameter layout.
pub fn plan_surfaces(document: &LotmDocument) -> Result<SurfacePlan, String> {
    let count = document.materials.len();
    if count == 0 || count > MAX_MATERIAL_ROWS || document.meshes.len() != count {
        return Err(format!(
            "author materials: require 1..=128 groups and one material per mesh ({} meshes, {count} materials)",
            document.meshes.len()
        ));
    }
    let mut surface_keys: Vec<String> = Vec::new();
    let mut room_keys: Vec<String> = Vec::new();
    let mut surface_of_row = Vec::with_capacity(count);
    let mut room_of_row = Vec::with_capacity(count);
    let mut rows = Vec::with_capacity(count);
    let mut palette_source = Vec::with_capacity(count);
    let mut room_params: Vec<[f32; 2]> = Vec::new();
    for (group, material) in document.materials.iter().enumerate() {
        if material.param_cols != 1 {
            return Err(format!("author materials: group {group} has param_cols={}, expected 1", material.param_cols));
        }
        let params = f32_params(material)?;
        let surface = surface_key(material);
        let surface_index = surface_keys.iter().position(|key| key == &surface).unwrap_or_else(|| {
            surface_keys.push(surface);
            surface_keys.len() - 1
        });
        let room = room_key(material, &params);
        let room_index = room_keys.iter().position(|key| key == &room).unwrap_or_else(|| {
            room_keys.push(room);
            room_keys.len() - 1
        });
        surface_of_row.push(surface_index);
        room_of_row.push(room_index);
        if room_params.len() <= room_index {
            room_params.push([params[2], params[3]]);
        }
        palette_source.push([0, 1].map(|layer| {
            let selected = (params[layer] as f64 * 512.0).round_ties_even() as i64;
            selected.rem_euclid(512) as u32
        }));
        rows.push(params);
    }
    // The extra cell is the mandatory flat-normal/zero-height Top fallback.
    let grid = power_of_two_grid(surface_keys.len() + 1);
    let atlas = grid * CELL;
    let room_atlas = (64 * room_keys.len().next_power_of_two() as u32).max(1024);
    let grid_f = grid as f32;
    let atlas_f = atlas as f32;
    let width = count.next_power_of_two() as u32;
    let top_enabled_by_row: Vec<bool> = rows
        .iter()
        .map(|params| params[8] != 0.0 || params[9] != 0.0)
        .collect();
    for (row, params) in rows.iter_mut().enumerate() {
        let x = ((surface_of_row[row] as u32 % grid) * CELL) as f32;
        let y = ((surface_of_row[row] as u32 / grid) * CELL) as f32;
        for layer in 0..2 {
            params[layer] = (row * 4 + layer * 2) as f32 / 512.0;
        }
        for line in [1usize, 2] {
            let offset = line * 4;
            if params[offset] == 0.0 && params[offset + 1] == 0.0 {
                continue;
            }
            params[offset] /= grid_f;
            params[offset + 1] /= grid_f;
            params[offset + 2] = params[offset + 2] / grid_f + x / atlas_f;
            params[offset + 3] = params[offset + 3] / grid_f + y / atlas_f;
        }
        if params[2] != 0.0 {
            params[2] = 64.0 / room_atlas as f32;
            params[3] = room_of_row[row] as f32 * 64.0 / room_atlas as f32;
        }
        params[14] *= grid_f;
        params[15] *= grid_f;
        if params[8] == 0.0 && params[9] == 0.0 {
            params[8] = 1.0 / grid_f;
            params[9] = 1.0 / grid_f;
            params[10] = (grid_f - 1.0) / grid_f;
            params[11] = (grid_f - 1.0) / grid_f;
        }
    }
    Ok(SurfacePlan {
        cell: CELL,
        width,
        grid,
        atlas,
        surface_of_row,
        rows,
        top_enabled_by_row,
        room_keys,
        room_atlas,
        room_of_row,
        palette_source,
        room_params,
    })
}

fn digest(bytes: Option<&Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    if let Some(bytes) = bytes {
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}

fn material_key(material: &LotmMaterial) -> String {
    let png = |index: usize| material.pngs.get(index).and_then(Option::as_ref);
    format!(
        "{}:{}:{}:{}:{}",
        digest(png(LotmMaterial::PNG_TINT)),
        digest(png(LotmMaterial::PNG_NORMAL)),
        digest(png(LotmMaterial::PNG_SHADER)),
        digest(png(LotmMaterial::PNG_PALETTE)),
        digest(material.params_f32.as_ref()),
    )
}

/// Collapse identical material rows and remap every mesh to its unique row.
pub fn deduplicate(document: &LotmDocument) -> Result<LotmDocument, String> {
    if document.materials.is_empty() {
        return Err("single-compatible: input has no materials".into());
    }
    let mut rows: Vec<LotmMaterial> = Vec::new();
    let mut row_by_key: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut material_row = Vec::with_capacity(document.materials.len());
    for material in &document.materials {
        let key = material_key(material);
        let row = *row_by_key.entry(key).or_insert_with(|| {
            rows.push(material.clone());
            rows.len() - 1
        });
        material_row.push(row);
    }
    if rows.len() > MAX_MATERIAL_ROWS {
        return Err(format!(
            "single-compatible: {} unique material rows exceed the engine limit {}",
            rows.len(),
            MAX_MATERIAL_ROWS
        ));
    }
    let mesh_material = document
        .mesh_material
        .iter()
        .map(|reference| {
            let row = material_row
                .get(reference.material as usize)
                .copied()
                .ok_or_else(|| {
                    format!(
                        "single-compatible: mesh references material {} out of range",
                        reference.material
                    )
                })?;
            Ok(MeshMaterialRef {
                material: row as u32,
                uv_kind: reference.uv_kind,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(LotmDocument {
        version: document.version,
        meshes: document.meshes.clone(),
        materials: rows,
        mesh_material,
        diagnostics: document.diagnostics.clone(),
    })
}

/// Rendered PNG payloads for the single material's six slots.
#[derive(Debug)]
pub struct SurfaceAtlases {
    pub tint: RgbaImage,
    pub normal: RgbaImage,
    pub shader: RgbaImage,
    pub palette: RgbaImage,
    pub interior: RgbaImage,
}

fn decode_png(material: &LotmMaterial, index: usize) -> Result<RgbaImage, String> {
    let bytes = material
        .pngs
        .get(index)
        .and_then(Option::as_ref)
        .ok_or_else(|| format!("single-compatible: material is missing PNG slot {index}"))?;
    image::load_from_memory(bytes)
        .map(|image| image.to_rgba8())
        .map_err(|error| format!("single-compatible: decode slot {index}: {error}"))
}

/// Nearest-neighbour resize without alpha premultiplication, using the
/// author's sampler (`Image.resize(..., NEAREST)`): each destination pixel
/// maps to `floor((i + 0.5) * src / dst)` of the source. The `image` crate's
/// resize path premultiplies alpha and rounds differently.
fn nearest_resize(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let mut output = RgbaImage::new(width, height);
    if source.width() == 0 || source.height() == 0 {
        return output;
    }
    for y in 0..height {
        let sy = (((y as f64 + 0.5) * source.height() as f64 / height as f64).floor() as u64)
            .min(source.height() as u64 - 1) as u32;
        for x in 0..width {
            let sx = (((x as f64 + 0.5) * source.width() as f64 / width as f64).floor() as u64)
                .min(source.width() as u64 - 1) as u32;
            output.put_pixel(x, y, *source.get_pixel(sx, sy));
        }
    }
    output
}

/// Render the exact 512-cell layout planned by [`plan_surfaces`].
pub fn render_atlases(
    document: &LotmDocument,
    plan: &SurfacePlan,
    night_only: bool,
) -> Result<SurfaceAtlases, String> {
    let size = plan.atlas;
    let mut tint = RgbaImage::new(size, size);
    let mut normal = RgbaImage::new(size, size);
    let mut shader = RgbaImage::new(size, size);
    let surface_count = plan.surface_of_row.iter().max().copied().unwrap_or(0) + 1;
    for surface in 0..surface_count {
        let Some(row) = plan.surface_of_row.iter().position(|item| *item == surface) else {
            continue;
        };
        let material = &document.materials[row];
        let x = (surface as u32 % plan.grid) * plan.cell;
        let y = (surface as u32 / plan.grid) * plan.cell;
        for (target, slot) in [
            (&mut tint, LotmMaterial::PNG_TINT),
            (&mut normal, LotmMaterial::PNG_NORMAL),
            (&mut shader, LotmMaterial::PNG_SHADER),
        ] {
            let mut source = decode_png(material, slot)?;
            if slot == LotmMaterial::PNG_SHADER && plan.top_enabled_by_row[row] {
                let grid = plan.grid as f64;
                for pixel in source.pixels_mut() {
                    let height = 23.0 + (pixel[0] as f64 - 23.0) / grid;
                    pixel[0] = height.round_ties_even().clamp(0.0, 255.0) as u8;
                    // NumPy's float64 -> uint8 cast truncates; it does not round.
                    pixel[1] = (pixel[1] as f64 * grid).clamp(0.0, 255.0) as u8;
                }
            }
            let resized = if source.width() == plan.cell && source.height() == plan.cell {
                source
            } else {
                nearest_resize(&source, plan.cell, plan.cell)
            };
            for cy in 0..plan.cell {
                for cx in 0..plan.cell {
                    target.put_pixel(x + cx, y + cy, *resized.get_pixel(cx, cy));
                }
            }
        }
    }
    let fallback_origin = size - plan.cell;
    for y in fallback_origin..size {
        for x in fallback_origin..size {
            normal.put_pixel(x, y, image::Rgba([128, 128, 255, 255]));
            shader.put_pixel(x, y, image::Rgba([23, 255, 0, 255]));
        }
    }

    let mut palette = RgbaImage::new(512, 16);
    for (row, material) in document.materials.iter().enumerate() {
        let source = decode_png(material, LotmMaterial::PNG_PALETTE)?;
        if source.width() < 512 || source.height() != 16 {
            return Err(format!(
                "author materials: group {row} palette is {}x{}, expected width>=512 and height=16",
                source.width(), source.height()
            ));
        }
        for layer in 0..2usize {
            let source_column = plan.palette_source[row][layer];
            let destination = (row * 4 + layer * 2) as u32;
            for column in 0..2u32 {
                for y in 0..16u32 {
                    let pixel = *source.get_pixel((source_column + column) % 512, y);
                    palette.put_pixel(destination + column, y, pixel);
                }
            }
        }
    }

    let diagnostic: serde_json::Value = serde_json::from_str(&document.diagnostics)
        .map_err(|error| format!("author materials: invalid diagnostics JSON: {error}"))?;
    let profile = diagnostic.get("nativeControlProfile").and_then(serde_json::Value::as_str);
    let migrate_room_rgb = profile == Some("native-room-states-transparent-top-v3");
    let office_recipe = profile == Some("window-relative-relief-hdr-room-v2");
    let mut interior = RgbaImage::new(plan.room_atlas, plan.room_atlas);
    for (room, parameters) in plan.room_params.iter().enumerate() {
        let row = plan.room_of_row.iter().position(|item| item == &room)
            .ok_or("author materials: room has no material row")?;
        let mut source = decode_png(&document.materials[row], LotmMaterial::PNG_INTERIOR)?;
        if migrate_room_rgb {
            for pixel in source.pixels_mut() {
                for channel in 0..3 {
                    let value = pixel[channel] as f64 / 255.0;
                    let linear = if value <= 0.04045 {
                        value / 12.92
                    } else {
                        ((value + 0.055) / 1.055).powf(2.4)
                    };
                    pixel[channel] = (linear * 255.0).round_ties_even().clamp(0.0, 255.0) as u8;
                }
            }
        }
        if parameters[0] > 0.0 && office_recipe && source.width() == 512
            && source.height() == 512 && parameters[0] == 1.0 / 16.0
        {
            let sheet = image::load_from_memory(include_bytes!("../../resources/sc5/office-room.png"))
                .map_err(|error| format!("office room source: {error}"))?.to_rgba8();
            let source_has_alpha = source.pixels().any(|pixel| pixel[3] != 0);
            let mut corrected = RgbaImage::new(512, 512);
            for row_index in 0..16u32 {
                for variant in 0..4u32 {
                    for state in 0..4u32 {
                        let sx = ((variant % 2) * 4 + state) * 32;
                        let sy = (variant / 2) * 32;
                        let dx = (variant * 4 + state) * 32;
                        let dy = row_index * 32;
                        for y in 0..32u32 {
                            for x in 0..32u32 {
                                let mut pixel = *sheet.get_pixel(sx + x, sy + y);
                                if state != 1 { pixel[3] = 0; }
                                corrected.put_pixel(dx + x, dy + y, pixel);
                            }
                        }
                    }
                }
            }
            if !source_has_alpha {
                for pixel in corrected.pixels_mut() { pixel[3] = 0; }
            }
            source = corrected;
        }
        let strip = render_room_strip(&source, parameters, night_only, room)?;
        for y in 0..64u32 {
            for x in 0..1024u32 {
                interior.put_pixel(x, room as u32 * 64 + y, *strip.get_pixel(x, y));
            }
        }
    }

    Ok(SurfaceAtlases { tint, normal, shader, palette, interior })
}

/// Render one source room into the native 4 variants × 4 occupancy states.
/// The source must already use the native column contract. Copying emission
/// from even columns was a model-specific compensation that hid invalid source
/// exports and changed the reference packager's pixels, so it is forbidden.
fn render_room_strip(
    source: &RgbaImage,
    parameters: &[f32; 2],
    night_only: bool,
    room: usize,
) -> Result<RgbaImage, String> {
    let room_h = source.height() as f64;
    let room_w = source.width() as f64;
    let scale = parameters[0] as f64;
    let offset = parameters[1] as f64;
    let tile = |step: u32| (step as f64 + 0.5) / 64.0;
    let yy: Vec<u32> = (0..64u32)
        .map(|row| ((tile(row) * scale + offset).rem_euclid(1.0) * room_h).floor() as u32)
        .collect();
    let pixel = |state: u32, tx: u32, ty: u32| {
        let xx = (((tile(tx) + state as f64) * scale).rem_euclid(1.0) * room_w).floor() as u32;
        *source.get_pixel(xx, yy[ty as usize])
    };
    let apply_night_only = night_only && parameters[0] > 0.0;
    let mut strip = RgbaImage::new(1024, 64);
    let mut sampled_alpha = 0u64;
    let mut output_night_alpha = 0u64;
    for state in 0..16u32 {
        for ty in 0..64u32 {
            for tx in 0..64u32 {
                sampled_alpha += pixel(state, tx, ty)[3] as u64;
                let mut value = pixel(state, tx, ty);
                if apply_night_only && state % 4 != 1 {
                    value[3] = 0;
                }
                if state % 4 == 1 {
                    output_night_alpha += value[3] as u64;
                }
                strip.put_pixel(state * 64 + tx, ty, value);
            }
        }
    }
    if apply_night_only && sampled_alpha > 0 && output_night_alpha == 0 {
        return Err(format!(
            "author materials: room {room} has sampled source emission but all native night states are empty; source columns 1/5/9/13 must contain native night emission (even-column migration is forbidden)"
        ));
    }
    Ok(strip)
}

fn patch_tint_column(glb: &[u8], column: u8) -> Vec<u8> {
    let mut bytes = glb.to_vec();
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes[20..20 + json_len]) else {
        return bytes;
    };
    let bin_start = 20 + json_len + 8;
    let Some(meshes) = json["meshes"].as_array() else {
        return bytes;
    };
    for mesh in meshes {
        let Some(primitives) = mesh["primitives"].as_array() else {
            continue;
        };
        for primitive in primitives {
            let Some(index) = primitive["attributes"]["TEXCOORD_1"].as_u64() else {
                continue;
            };
            let Some(accessor) = json["accessors"].get(index as usize) else {
                continue;
            };
            if accessor["type"] != "VEC2" || accessor["componentType"] != 5126 {
                continue;
            }
            let Some(view_index) = accessor["bufferView"].as_u64() else {
                continue;
            };
            let Some(view) = json["bufferViews"].get(view_index as usize) else {
                continue;
            };
            let stride = view["byteStride"].as_u64().unwrap_or(8) as usize;
            let start = bin_start
                + view["byteOffset"].as_u64().unwrap_or(0) as usize
                + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
            let count = accessor["count"].as_u64().unwrap_or(0) as usize;
            for vertex in 0..count {
                let at = start + vertex * stride;
                if at + 4 <= bytes.len() {
                    bytes[at..at + 4].copy_from_slice(&(column as f32 / 255.0).to_le_bytes());
                }
            }
        }
    }
    bytes
}

fn png_bytes(image: &RgbaImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("PNG encoding cannot fail for a valid RgbaImage");
    bytes
}

/// Serialize the rendered single material as a LOTM v9 document that the
/// existing Rust RW4/property pipeline can consume.
pub fn compose_lotm(
    document: &LotmDocument,
    plan: &SurfacePlan,
    atlases: &SurfaceAtlases,
) -> LotmDocument {
    let width = plan.width as usize;
    let mut table = vec![0f32; 4 * width * 4];
    for (row, values) in plan.rows.iter().enumerate() {
        for line in 0..4 {
            for column in 0..4 {
                table[(line * width + row) * 4 + column] = values[line * 4 + column];
            }
        }
    }
    let params: Vec<u8> = table.iter().flat_map(|value| value.to_le_bytes()).collect();
    LotmDocument {
        version: 9,
        meshes: document
            .meshes
            .iter()
            .enumerate()
            .map(|(mesh, glb)| {
                let column = document
                    .mesh_material
                    .get(mesh)
                    .map(|reference| reference.material as u8)
                    .unwrap_or(0);
                patch_tint_column(glb, column)
            })
            .collect(),
        materials: vec![LotmMaterial {
            pngs: vec![
                None,
                Some(png_bytes(&atlases.normal)),
                None,
                None,
                Some(png_bytes(&atlases.tint)),
                Some(png_bytes(&atlases.palette)),
                Some(png_bytes(&atlases.shader)),
                Some(png_bytes(&atlases.interior)),
            ],
            params_f32: Some(params),
            param_cols: plan.width,
        }],
        mesh_material: vec![
            MeshMaterialRef {
                material: 0,
                uv_kind: 0,
            };
            document.meshes.len()
        ],
        diagnostics: document.diagnostics.clone(),
    }
}

/// Encode a LOTM v9 document in the container order consumed by the builder.
pub fn write_lotm(document: &LotmDocument) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"LOTM");
    out.extend_from_slice(&9u32.to_le_bytes());
    out.extend_from_slice(&(document.meshes.len() as u32).to_le_bytes());
    for mesh in &document.meshes {
        out.extend_from_slice(&(mesh.len() as u32).to_le_bytes());
        out.extend_from_slice(mesh);
    }
    out.extend_from_slice(&(document.materials.len() as u32).to_le_bytes());
    for material in &document.materials {
        for index in 0..8 {
            let png = material.pngs.get(index).and_then(Option::as_ref);
            let bytes = png.map(Vec::as_slice).unwrap_or(&[]);
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        let params = material.params_f32.as_deref().unwrap_or(&[]);
        out.extend_from_slice(&(params.len() as u32).to_le_bytes());
        out.extend_from_slice(params);
        out.extend_from_slice(&material.param_cols.to_le_bytes());
    }
    for reference in &document.mesh_material {
        out.extend_from_slice(&reference.material.to_le_bytes());
        out.push(reference.uv_kind);
    }
    let diagnostic = document.diagnostics.as_bytes();
    out.extend_from_slice(&(diagnostic.len() as u32).to_le_bytes());
    out.extend_from_slice(diagnostic);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material(params: u8) -> LotmMaterial {
        LotmMaterial {
            pngs: vec![None; 8],
            params_f32: Some(vec![params; 64]),
            param_cols: 1,
        }
    }

    #[test]
    fn collapses_identical_rows_and_remaps_meshes() {
        let document = LotmDocument {
            version: 9,
            meshes: vec![vec![], vec![], vec![]],
            materials: vec![material(0), material(1), material(0)],
            mesh_material: vec![
                MeshMaterialRef {
                    material: 0,
                    uv_kind: 0,
                },
                MeshMaterialRef {
                    material: 1,
                    uv_kind: 0,
                },
                MeshMaterialRef {
                    material: 2,
                    uv_kind: 0,
                },
            ],
            diagnostics: String::new(),
        };
        let output = deduplicate(&document).unwrap();
        assert_eq!(output.materials.len(), 2);
        assert_eq!(
            output
                .mesh_material
                .iter()
                .map(|reference| reference.material)
                .collect::<Vec<_>>(),
            vec![0, 1, 0]
        );
    }

    #[test]
    fn rejects_more_than_128_unique_rows() {
        // prepare_asset 0.8.6: `require(count<=128,'Palette profile supports
        // at most 128 source groups')`.
        let document = LotmDocument {
            version: 9,
            meshes: vec![vec![]],
            materials: (0..129).map(material).collect(),
            mesh_material: vec![MeshMaterialRef {
                material: 128,
                uv_kind: 0,
            }],
            diagnostics: String::new(),
        };
        assert!(deduplicate(&document).is_err());
    }

    fn room_source(emission_states: &[u32]) -> RgbaImage {
        let mut source = RgbaImage::new(512, 512);
        for state in 0..8u32 {
            let alpha = if emission_states.contains(&state) { 200 } else { 0 };
            for y in 0..64u32 {
                for x in 0..64u32 {
                    source.put_pixel(
                        state * 64 + x,
                        y,
                        image::Rgba([(state * 20) as u8, 80, 120, alpha]),
                    );
                }
            }
        }
        source
    }

    #[test]
    fn paired_room_columns_are_rejected_instead_of_silently_migrated() {
        let source = room_source(&[0, 2, 4, 6]);
        let error = render_room_strip(&source, &[0.125, 0.0], true, 7).unwrap_err();
        assert!(error.contains("room 7"), "{error}");
        assert!(error.contains("even-column migration is forbidden"), "{error}");
    }

    #[test]
    fn native_room_columns_keep_their_original_night_sampling() {
        let source = room_source(&(0..8).collect::<Vec<_>>());
        let strip = render_room_strip(&source, &[0.125, 0.0], true, 0).unwrap();
        for state in [1u32, 5, 9, 13] {
            let pixel = strip.get_pixel(state * 64 + 32, 32);
            assert_eq!(pixel[0], ((state % 8) * 20) as u8, "state {state}");
            assert_eq!(pixel[3], 200, "state {state}");
        }
    }

    #[test]
    fn dark_room_stays_dark_without_triggering_the_emission_gate() {
        let source = room_source(&[]);
        let strip = render_room_strip(&source, &[0.125, 0.0], true, 0).unwrap();
        assert!(strip.pixels().all(|pixel| pixel[3] == 0));
    }

    #[test]
    fn room_with_emission_but_no_native_night_output_is_rejected() {
        let source = room_source(&[2]);
        let error = render_room_strip(&source, &[0.125, 0.0], true, 11).unwrap_err();
        assert!(error.contains("room 11"), "{error}");
        assert!(error.contains("all native night states are empty"), "{error}");
    }

    /// Disabled Top regions receive the atlas' bottom-right fallback cell
    /// (prepare_asset `p[2]=[1/sg,1/sg,(sg-1)/sg,(sg-1)/sg]`).
    #[test]
    fn plan_writes_fallback_region_for_disabled_top() {
        let document = LotmDocument {
            version: 9,
            meshes: vec![vec![]],
            materials: vec![material(0)],
            mesh_material: vec![MeshMaterialRef {
                material: 0,
                uv_kind: 0,
            }],
            diagnostics: String::new(),
        };
        let plan = plan_surfaces(&document).expect("plan");
        assert_eq!(plan.width, 1);
        let row = &plan.rows[0];
        // One source material + the mandatory Top fallback cell ⇒ grid = 2.
        // The author's fallback is [1/sg, 1/sg, (sg-1)/sg, (sg-1)/sg].
        assert!((row[8] - 0.5).abs() < 1e-6);
        assert!((row[10] - 0.5).abs() < 1e-6);
    }
}
