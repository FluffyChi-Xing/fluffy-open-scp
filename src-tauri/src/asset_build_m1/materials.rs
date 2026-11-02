//! Container materials → package resources.
//!
//! The workbench LOTM container carries each material's six-slot textures as
//! PNGs plus an f32 parameter table.  The Property Viewer resolves material
//! texture refs against **game-native** resources only
//! (`resolve_slot_texture`: raster pixFmt21 or RW4-wrapped TEXTURE), so raw
//! PNG bytes must be re-encoded before they enter a package — writing them
//! verbatim is why v1 builds lost every texture (user report 2026-10-30).
//!
//! Encoding rules:
//! - PNG slots → uncompressed raster (pixFmt 21, generated mip chain);
//! - `paramsF32` → standalone RW4 wrapping one TEXTURE section of type 116
//!   (A32B32G32R32F palette); slot0 parameter tables are looked up through
//!   RW4 resources on the viewer side;
//! - non-PNG slot payloads (already game rasters) are preserved verbatim;
//! - materials whose resolved slots are byte-identical share one entry.

use super::lotm::{LotmDocument, LotmMaterial};
use dbpf::ResourceId;
use rw4::{RasterImage, Rw4File, SectionType, build_file, writer_section};
use sha2::{Digest, Sha256};

/// One material's resolved texture payloads: six slots (slot0..slot5).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedMaterial {
    /// `(payload, is_png)` per slot; PNG payloads get raster-encoded, all
    /// others enter the package verbatim.
    pub slots: [Option<(Vec<u8>, bool)>; 6],
}

impl ResolvedMaterial {
    /// Content fingerprint for dedup (payload bytes only).
    fn key(&self) -> Vec<u8> {
        let mut key = Vec::new();
        for slot in &self.slots {
            match slot {
                Some((bytes, is_png)) => {
                    key.push(*is_png as u8);
                    key.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                    key.extend_from_slice(&Sha256::digest(bytes));
                }
                None => key.push(0),
            }
        }
        key
    }
}

/// Deduped material plan: `mesh_material[m]` indexes `unique`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialPlan {
    pub unique: Vec<ResolvedMaterial>,
    /// Per container mesh → unique material index.
    pub mesh_material: Vec<u32>,
}

/// Resolve and dedupe the container's materials.  Slot mapping mirrors the
/// viewer's building-chain semantics: slot0 = f32 parameter table (else raw
/// diffuse), slot1 = tint control map (else diffuse), slot2 = normal,
/// slot3 = shader map (else roughness), slot4 = palette, slot5 = interior.
pub fn resolve_materials(doc: &LotmDocument) -> Result<MaterialPlan, String> {
    if doc.materials.is_empty() {
        // Plain imported containers carry no six-slot materials; one shared
        // empty material keeps every mesh bound (schema slots may still
        // supply textures on the package side).
        return Ok(MaterialPlan {
            unique: vec![ResolvedMaterial::default()],
            mesh_material: vec![0; doc.meshes.len()],
        });
    }
    let mut unique: Vec<ResolvedMaterial> = Vec::new();
    let mut keys: Vec<Vec<u8>> = Vec::new();
    let mut mesh_material = Vec::with_capacity(doc.meshes.len());
    for (mesh_index, reference) in doc.mesh_material.iter().enumerate() {
        let material = doc
            .materials
            .get(reference.material as usize)
            .ok_or_else(|| {
                format!(
                    "mesh {mesh_index} references material {} outside the container",
                    reference.material
                )
            })?;
        let resolved = resolve_one(material)?;
        let key = resolved.key();
        let index = match keys.iter().position(|existing| *existing == key) {
            Some(index) => index,
            None => {
                keys.push(key);
                unique.push(resolved);
                unique.len() - 1
            }
        };
        mesh_material.push(index as u32);
    }
    Ok(MaterialPlan {
        unique,
        mesh_material,
    })
}

fn resolve_one(material: &LotmMaterial) -> Result<ResolvedMaterial, String> {
    let png = |index: usize| -> Option<(Vec<u8>, bool)> {
        material
            .pngs
            .get(index)
            .cloned()
            .flatten()
            .map(|bytes| (bytes, true))
    };
    let raw = |index: usize| -> Option<(Vec<u8>, bool)> {
        material
            .pngs
            .get(index)
            .cloned()
            .flatten()
            .map(|bytes| (bytes, false))
    };
    let params = material
        .params_f32
        .as_ref()
        .filter(|bytes| bytes.len() >= 16 && material.param_cols > 0)
        .map(|bytes| encode_params_rw4(bytes, material.param_cols).map(|rw4| (rw4, false)))
        .transpose()?;
    let mut slots: [Option<(Vec<u8>, bool)>; 6] = Default::default();
    // slot0: f32 parameter table (RW4 palette), else v10 raw diffuse.
    slots[0] = params.or_else(|| png(LotmMaterial::PNG_SLOT0));
    // slot1: tint control map, else the base color as simple diffuse.
    slots[1] = png(LotmMaterial::PNG_TINT).or_else(|| png(LotmMaterial::PNG_BASE));
    slots[2] = png(LotmMaterial::PNG_NORMAL);
    slots[3] = png(LotmMaterial::PNG_SHADER).or_else(|| png(LotmMaterial::PNG_ROUGHNESS));
    // slot4 = tint palette (512×16), slot5 = interior room atlas — the
    // game-verified contract from building-texture-export.ts and the HLSL
    // note (slot4 512×16 = 256×8 tint palette). Author model refs confirmed:
    // slot4 → the 512×16 wrapper, slot5 → the 2048² room atlas.
    slots[4] = png(LotmMaterial::PNG_PALETTE);
    slots[5] = png(LotmMaterial::PNG_INTERIOR);
    // Nothing decoded as PNG keeps legacy opaque payloads (schema slots).
    if slots.iter().all(Option::is_none) {
        slots[1] = raw(LotmMaterial::PNG_BASE);
    }
    Ok(ResolvedMaterial { slots })
}

/// Encode a PNG slot payload as an uncompressed game raster (pixFmt 21 with
/// a generated mip chain).  Transparent pixels keep their RGB — the game
/// samples the color channels regardless of alpha.
pub fn encode_png_raster(png: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(png)
        .map_err(|e| format!("slot PNG decode failed: {e}"))?
        .to_rgba8();
    let (width, height) = (image.width(), image.height());
    let pixels: Vec<[u8; 4]> = image
        .pixels()
        .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
        .collect();
    encode_rgba_raster(width, height, &pixels)
}

/// Encode raw RGBA pixels as an uncompressed game raster — the in-memory
/// form of `encode_png_raster`, used for the rotation-transformed ground
/// mask (`raster_rgba(rotated_mask)` in the author chain).
pub fn encode_rgba_raster(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
) -> Result<Vec<u8>, String> {
    if pixels.len() != (width as usize) * (height as usize) {
        return Err("raster pixel buffer size mismatch".into());
    }
    // Match authored_materials.raster_rgba: independent-channel floor
    // averaging, BGRA byte order, pixFmt 21, big-endian header.
    let mut current: Vec<[u8; 4]> = pixels.to_vec();
    let mut mips = Vec::new();
    let (mut w, mut h) = (width, height);
    loop {
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for pixel in &current {
            bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
        mips.push(bgra);
        if w == 1 && h == 1 {
            break;
        }
        let nw = (w / 2).max(1) as usize;
        let nh = (h / 2).max(1) as usize;
        let mut next = vec![[0u8; 4]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut sum = [0u32; 4];
                let mut count = 0u32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let sx = (x * 2 + dx).min(w as usize - 1);
                        let sy = (y * 2 + dy).min(h as usize - 1);
                        let pixel = current[sy * w as usize + sx];
                        for channel in 0..4 {
                            sum[channel] += pixel[channel] as u32;
                        }
                        count += 1;
                    }
                }
                next[y * nw + x] = [
                    (sum[0] / count) as u8,
                    (sum[1] / count) as u8,
                    (sum[2] / count) as u8,
                    (sum[3] / count) as u8,
                ];
            }
        }
        current = next;
        w = nw as u32;
        h = nh as u32;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&2u32.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&(mips.len() as u32).to_be_bytes());
    out.extend_from_slice(&8u32.to_be_bytes());
    out.extend_from_slice(&21u32.to_be_bytes());
    for mip in mips {
        out.extend_from_slice(&(mip.len() as u32).to_be_bytes());
        out.extend_from_slice(&mip);
    }
    Ok(out)
}

/// Wrap a palette/interior PNG in the author's RGBA texture template.
/// The author stores these two slots as `texture_rw4(fmt=21)` wrappers with a
/// raw BGRA top mip, not as standalone Raster resources.
pub fn encode_png_texture(png: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(png)
        .map_err(|error| format!("texture PNG decode failed: {error}"))?
        .to_rgba8();
    let (width, height) = (image.width(), image.height());
    let mut bgra = Vec::with_capacity((width * height * 4) as usize);
    for pixel in image.pixels() {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    const TEMPLATE: &[u8] =
        include_bytes!("../../resources/sc5/2EF7D20A.texture-template.rw4");
    let file = Rw4File::parse(TEMPLATE).map_err(|error| error.to_string())?;
    let texture = file
        .sections_of_type(SectionType::TEXTURE)
        .next()
        .ok_or("texture template has no TEXTURE section")?;
    let mut texture_payload = file
        .payload(TEMPLATE, texture.number)
        .map_err(|error| error.to_string())?
        .to_vec();
    if texture_payload.len() < 32 {
        return Err("texture template header is truncated".into());
    }
    texture_payload[0..4].copy_from_slice(&21u32.to_le_bytes());
    texture_payload[12..14].copy_from_slice(&(width as u16).to_le_bytes());
    texture_payload[14..16].copy_from_slice(&(height as u16).to_le_bytes());
    texture_payload[16..20].copy_from_slice(&0x120u32.to_le_bytes());
    let blob = u32::from_le_bytes(texture_payload[28..32].try_into().unwrap());
    let mut replacements = std::collections::HashMap::new();
    replacements.insert(texture.number, texture_payload);
    replacements.insert(blob, bgra);
    rw4::writer::replace_template_sections(TEMPLATE, &replacements)
        .map_err(|error| format!("texture wrapper: {error}"))
}

/// Wrap an f32 parameter table (row-major `cols × 4` floats) into a minimal
/// RW4 file with one paletteF32 TEXTURE section — the shape the viewer's
/// slot0 lookup expects (`Rw4File → TEXTURE → decode_palette_f32`).
pub fn encode_params_rw4(params_f32: &[u8], cols: u32) -> Result<Vec<u8>, String> {
    if cols == 0 || params_f32.len() % 16 != 0 {
        return Err("parameter table must be a multiple of 16 bytes".into());
    }
    let texels = params_f32.len() / 16;
    if texels % cols as usize != 0 {
        return Err(format!(
            "parameter table has {texels} texels, not a multiple of {cols} columns"
        ));
    }
    let rows = (texels / cols as usize) as u16;
    let cols = u16::try_from(cols).map_err(|_| "parameter columns overflow u16")?;
    // Texture header: type, 8, unknown1, width, height, mipmapInfo(mips<<8),
    // 0, 0, data section ref — mirror `DecodedTexture::parse`.
    let mut texture = Vec::new();
    texture.extend_from_slice(&116u32.to_le_bytes());
    texture.extend_from_slice(&8u32.to_le_bytes());
    texture.extend_from_slice(&0u32.to_le_bytes());
    texture.extend_from_slice(&cols.to_le_bytes());
    texture.extend_from_slice(&rows.to_le_bytes());
    texture.extend_from_slice(&0x180u32.to_le_bytes());
    texture.extend_from_slice(&0u32.to_le_bytes());
    texture.extend_from_slice(&0u32.to_le_bytes());
    texture.extend_from_slice(&0i32.to_le_bytes()); // data section number (Blob #0)
    // A palette lookup is a Texture resource, not a Model.  The native
    // NightHouse palette is `file_type=0x04000000`, Blob #0 then Texture #1.
    let rw4 = build_file(
        0x0400_0000,
        &[
            writer_section(SectionType::BLOB, params_f32.to_vec()),
            writer_section(SectionType::TEXTURE, texture),
        ],
        &[],
    );
    let file = Rw4File::parse(&rw4).map_err(|e| format!("params RW4 parse failed: {e}"))?;
    let section = file
        .sections_of_type(SectionType::TEXTURE)
        .next()
        .ok_or("params RW4 lost its texture section")?;
    let decoded = file
        .decode_texture(&rw4, section.number)
        .map_err(|e| format!("params RW4 texture decode failed: {e}"))?;
    let readback = decoded
        .decode_palette_f32()
        .map_err(|e| format!("params RW4 palette decode failed: {e}"))?;
    if readback.len() != texels {
        return Err("params RW4 round-trip texel count mismatch".into());
    }
    Ok(rw4)
}

/// Materialize a plan into package resources: deduped payload set plus the
/// six texture instance ids per unique material.  `allocate` assigns an
/// instance for `(type_id, bytes)`; the caller owns collision handling and
/// must insert the returned payloads under exactly those instances.
pub struct MaterialResources {
    /// `(type_id, instance, bytes)` for every produced resource.
    pub payloads: Vec<(u32, u32, Vec<u8>)>,
    /// Texture instance ids per unique material (slot0..slot5; 0 = unbound).
    pub unique_instances: Vec<[u32; 6]>,
}

pub fn build_resources<F>(plan: &MaterialPlan, mut allocate: F) -> Result<MaterialResources, String>
where
    F: FnMut(u32, &[u8]) -> Result<u32, String>,
{
    // Dedup payloads across all slots by content so identical textures share
    // one instance (105 façade materials can reference the same atlas).
    let mut payloads: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    let mut instance_of: std::collections::HashMap<(u32, [u8; 32]), u32> =
        std::collections::HashMap::new();
    let mut unique_instances = Vec::with_capacity(plan.unique.len());
    for material in &plan.unique {
        let mut instances = [0u32; 6];
        for (slot, payload) in material.slots.iter().enumerate() {
            let Some((bytes, is_png)) = payload else {
                continue;
            };
            let is_png = *is_png;
            // slot0 non-PNG payloads are the wrapped f32 parameter tables;
            // every other slot is a raster.
            let texture_slot = slot == 4 || slot == 5;
            let type_id = if (slot == 0 && !is_png) || (texture_slot && is_png) {
                rw4_model_type()
            } else {
                raster_type()
            };
            let encoded_key = (type_id, Sha256::digest(bytes).into());
            let instance = if let Some(instance) = instance_of.get(&encoded_key) {
                *instance
            } else {
                let prepared = if texture_slot && is_png {
                    encode_png_texture(bytes)?
                } else if type_id == raster_type() && is_png {
                    encode_png_raster(bytes)?
                } else {
                    bytes.clone()
                };
                let instance = allocate(type_id, &prepared)?;
                payloads.push((type_id, instance, prepared));
                instance_of.insert(encoded_key, instance);
                instance
            };
            instances[slot] = instance;
        }
        unique_instances.push(instances);
    }
    Ok(MaterialResources {
        payloads,
        unique_instances,
    })
}

pub fn raster_type() -> u32 {
    0x2F4E_681C
}

pub fn rw4_model_type() -> u32 {
    0x2F4E_681B
}

/// Validate the external resources referenced by a compiled RW4 material.
///
/// RW4 parsing and round-trip decoding only proves that section offsets are
/// internally consistent. The game resolves material slots by TGI later, so
/// a model can still be structurally valid while pointing at resources that
/// are absent from the output package. Reject that state before publishing a
/// package. Zero is an intentional unbound slot and remains allowed for
/// legacy/plain GLB inputs.
pub fn validate_model_resource_closure(
    model: &[u8],
    resources: &std::collections::HashMap<ResourceId, Vec<u8>>,
) -> Result<(), String> {
    let file = Rw4File::parse(model).map_err(|e| format!("model RW4 parse failed: {e}"))?;
    for section in file.sections_of_type(SectionType::MATERIAL) {
        let material = file
            .decode_material(model, section.number)
            .map_err(|e| format!("material section #{} decode failed: {e}", section.number))?;
        let rw4::MaterialSection::Decoded(material) = material else {
            return Err(format!(
                "material section #{} has an unknown layout",
                section.number
            ));
        };
        for slot in material.texture_slots() {
            let instance = slot.texture_instance;
            if instance == 0 {
                continue;
            }
            let rw4_id = ResourceId {
                type_id: rw4_model_type(),
                group: 0,
                instance,
            };
            let raster_id = ResourceId {
                type_id: raster_type(),
                group: 0,
                instance,
            };
            // Native materials use RW4 wrappers for slot0 and, in some
            // families, for palette/interior slots as well. Resolve the
            // exact TGI by trying the wrapper first, then the raster form.
            let (type_id, id, bytes) = if let Some(bytes) = resources.get(&rw4_id) {
                (rw4_model_type(), rw4_id, bytes)
            } else if let Some(bytes) = resources.get(&raster_id) {
                if slot.slot_byte() == 0 {
                    return Err(format!(
                        "material section #{} slot0 instance 0x{:08X} is not an RW4 parameter resource",
                        section.number, instance
                    ));
                }
                (raster_type(), raster_id, bytes)
            } else {
                return Err(format!(
                    "material section #{} slot{} references missing resource instance 0x{:08X}",
                    section.number,
                    slot.slot_byte(),
                    instance
                ));
            };
            if type_id == rw4_model_type() {
                let parameter = Rw4File::parse(bytes).map_err(|e| {
                    format!(
                        "material section #{} slot0 {} is not RW4: {e}",
                        section.number, id
                    )
                })?;
                if parameter
                    .sections_of_type(SectionType::TEXTURE)
                    .next()
                    .is_none()
                {
                    return Err(format!(
                        "material section #{} slot0 {} has no TEXTURE section",
                        section.number, id
                    ));
                }
            } else {
                RasterImage::parse(bytes).map_err(|e| {
                    format!(
                        "material section #{} slot{} {} is not raster: {e}",
                        section.number,
                        slot.slot_byte(),
                        id
                    )
                })?;
            }
        }
    }
    Ok(())
}

/// PNG magic check used by schema slot files.
pub fn is_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::lotm::MeshMaterialRef;

    fn tiny_png() -> Vec<u8> {
        let mut rgba = vec![0u8; 4 * 4 * 4];
        for (index, px) in rgba.chunks_exact_mut(4).enumerate() {
            px.copy_from_slice(&[(index * 7) as u8, 64, 128, 255]);
        }
        let image = image::RgbaImage::from_raw(4, 4, rgba).expect("png buffer");
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png encode");
        png
    }

    #[test]
    fn png_encodes_to_decodable_raster() {
        let png = tiny_png();
        assert!(is_png(&png));
        let raster = encode_png_raster(&png).unwrap();
        let decoded = RasterImage::parse(&raster).unwrap();
        assert_eq!(decoded.pixel_format, 21);
        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 4);
        let rgba = decoded.decode_top_mip_rgba().unwrap();
        assert_eq!(rgba.len(), 4 * 4 * 4);
    }

    #[test]
    fn params_table_wraps_into_palette_rw4() {
        let mut table = Vec::new();
        for row in 0..4u32 {
            let row = row as f32;
            for value in [row, 1.0, 2.0, 3.0] {
                table.extend_from_slice(&value.to_le_bytes());
            }
        }
        let rw4 = encode_params_rw4(&table, 1).unwrap();
        let file = Rw4File::parse(&rw4).unwrap();
        assert_eq!(file.file_type(), rw4::FileType::Texture);
        assert_eq!(
            file.sections().iter().map(|s| s.type_code).collect::<Vec<_>>(),
            vec![SectionType::BLOB, SectionType::TEXTURE]
        );
        let section = file
            .sections_of_type(SectionType::TEXTURE)
            .next()
            .expect("texture section");
        let texture = file.decode_texture(&rw4, section.number).unwrap();
        assert_eq!(texture.texture_type, 116);
        assert_eq!(texture.width, 1);
        assert_eq!(texture.height, 4);
        let pixels = texture.decode_palette_f32().unwrap();
        assert_eq!(pixels.len(), 4);
        assert_eq!(pixels[2][0], 2.0);
    }

    #[test]
    fn identical_materials_dedupe_to_one_entry() {
        let png = tiny_png();
        let material = LotmMaterial {
            pngs: vec![
                Some(png.clone()),
                None,
                None,
                None,
                Some(png.clone()),
                None,
                None,
                None,
            ],
            params_f32: None,
            param_cols: 0,
        };
        let doc = LotmDocument {
            version: 9,
            meshes: vec![vec![0u8; 4], vec![0u8; 4]],
            materials: vec![material.clone(), material],
            mesh_material: vec![
                MeshMaterialRef {
                    material: 0,
                    uv_kind: 2,
                },
                MeshMaterialRef {
                    material: 1,
                    uv_kind: 2,
                },
            ],
            diagnostics: String::new(),
        };
        let plan = resolve_materials(&doc).unwrap();
        assert_eq!(plan.unique.len(), 1);
        assert_eq!(plan.mesh_material, vec![0, 0]);
        let resources = build_resources(&plan, |type_id, _| Ok(type_id)).unwrap();
        // slot1 tint only → one raster payload.
        assert_eq!(resources.payloads.len(), 1);
        assert_eq!(resources.payloads[0].0, 0x2F4E681C);
        assert_eq!(resources.unique_instances[0][1], 0x2F4E681C);
        assert_eq!(resources.unique_instances[0][2], 0);
    }

    #[test]
    fn container_without_materials_resolves_to_one_shared_empty_material() {
        let doc = LotmDocument {
            version: 9,
            meshes: vec![vec![0u8; 4], vec![0u8; 4], vec![0u8; 4]],
            materials: Vec::new(),
            mesh_material: vec![
                MeshMaterialRef {
                    material: 0,
                    uv_kind: 0,
                };
                3
            ],
            diagnostics: String::new(),
        };
        let plan = resolve_materials(&doc).unwrap();
        assert_eq!(plan.unique.len(), 1);
        assert_eq!(plan.mesh_material, vec![0, 0, 0]);
        let resources = build_resources(&plan, |type_id, _| Ok(type_id)).unwrap();
        assert!(resources.payloads.is_empty(), "no textures, no payloads");
        assert_eq!(resources.unique_instances, vec![[0u32; 6]]);
    }
}
