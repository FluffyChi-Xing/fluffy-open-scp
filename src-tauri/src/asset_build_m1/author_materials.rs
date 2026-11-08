//! Strict LOTM9 material compiler ported from the latest SC5 Asset Packager.
//!
//! The six payloads are produced directly from the source groups. No display
//! RGB baking, material deduplication, palette resizing, or legacy NH fallback
//! participates in this route.

use std::collections::HashMap;

use image::RgbaImage;
use rw4::{Rw4File, SectionType};

use super::author_building40::write_with_author_layout;
use super::lotm::LotmDocument;
use super::single_compatible::{SurfaceAtlases, SurfacePlan, plan_surfaces, render_atlases};

const FLOAT_TEXTURE_TEMPLATE: &[u8] =
    include_bytes!("../../resources/sc5/DCBEA56D.texture-template.rw4");
const RGBA_TEXTURE_TEMPLATE: &[u8] =
    include_bytes!("../../resources/sc5/2EF7D20A.texture-template.rw4");

#[derive(Debug)]
pub struct AuthorMaterials {
    pub plan: SurfacePlan,
    pub atlases: SurfaceAtlases,
    pub params_f32: Vec<u8>,
    /// Building4 texture slots 0 through 5, in native resource order.
    pub slots: [Vec<u8>; 6],
}

/// Compile the exact parameter table, five images and six package payloads.
pub fn compile_materials(
    document: &LotmDocument,
    night_only: bool,
) -> Result<AuthorMaterials, String> {
    let plan = plan_surfaces(document)?;
    let atlases = render_atlases(document, &plan, night_only)?;
    let params_f32 = parameter_table_bytes(&plan);
    let slots = [
        encode_float_texture(&params_f32, plan.width, 4)?,
        encode_raster(&atlases.tint)?,
        encode_raster(&atlases.normal)?,
        encode_raster(&atlases.shader)?,
        encode_rgba_texture(&atlases.palette)?,
        encode_rgba_texture(&atlases.interior)?,
    ];
    Ok(AuthorMaterials {
        plan,
        atlases,
        params_f32,
        slots,
    })
}

/// NumPy `(4, width, 4)` little-endian Float32 table layout.
pub fn parameter_table_bytes(plan: &SurfacePlan) -> Vec<u8> {
    let width = plan.width as usize;
    let mut table = vec![0f32; 4 * width * 4];
    for (group, values) in plan.rows.iter().enumerate() {
        for line in 0..4 {
            for column in 0..4 {
                table[(line * width + group) * 4 + column] = values[line * 4 + column];
            }
        }
    }
    table.iter().flat_map(|value| value.to_le_bytes()).collect()
}

fn rgba_pixels(image: &RgbaImage) -> Vec<[u8; 4]> {
    image.pixels().map(|pixel| pixel.0).collect()
}

fn encode_raster(image: &RgbaImage) -> Result<Vec<u8>, String> {
    super::materials::encode_rgba_raster(image.width(), image.height(), &rgba_pixels(image))
}

fn encode_float_texture(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    if bytes.len() != width as usize * height as usize * 16 {
        return Err("author materials: Float32 parameter table size mismatch".into());
    }
    if bytes
        .chunks_exact(4)
        .any(|value| !f32::from_le_bytes(value.try_into().unwrap()).is_finite())
    {
        return Err("author materials: non-finite parameter texel".into());
    }
    texture_wrapper(FLOAT_TEXTURE_TEMPLATE, bytes.to_vec(), width, height, 116)
}

fn encode_rgba_texture(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bgra = Vec::with_capacity(image.width() as usize * image.height() as usize * 4);
    for pixel in image.pixels() {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    texture_wrapper(
        RGBA_TEXTURE_TEMPLATE,
        bgra,
        image.width(),
        image.height(),
        21,
    )
}

fn texture_wrapper(
    template: &[u8],
    data: Vec<u8>,
    width: u32,
    height: u32,
    format: u32,
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        return Err("author materials: texture dimensions exceed native wrapper".into());
    }
    let file = Rw4File::parse(template).map_err(|error| error.to_string())?;
    if file.file_type() != rw4::FileType::Texture || file.sections().len() != 2 {
        return Err("author materials: unexpected texture wrapper layout".into());
    }
    let textures: Vec<_> = file.sections_of_type(SectionType::TEXTURE).collect();
    if textures.len() != 1 {
        return Err("author materials: expected exactly one TEXTURE section".into());
    }
    let texture = textures[0];
    let mut header = file
        .payload(template, texture.number)
        .map_err(|error| error.to_string())?
        .to_vec();
    if header.len() < 32 {
        return Err("author materials: truncated TEXTURE header".into());
    }
    header[0..4].copy_from_slice(&format.to_le_bytes());
    header[12..14].copy_from_slice(&(width as u16).to_le_bytes());
    header[14..16].copy_from_slice(&(height as u16).to_le_bytes());
    header[16..20]
        .copy_from_slice(&(if format == 116 { 0x180 } else { 0x120 } as u32).to_le_bytes());
    let blob = u32::from_le_bytes(header[28..32].try_into().unwrap());
    let mut replacements = HashMap::new();
    replacements.insert(texture.number, header);
    replacements.insert(blob, data.clone());
    let output = write_with_author_layout(template, &replacements)?;
    let check = Rw4File::parse(&output).map_err(|error| error.to_string())?;
    if check
        .payload(&output, blob)
        .map_err(|error| error.to_string())?
        != data
    {
        return Err("author materials: texture writeback mismatch".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn parameter_layout_is_line_major_with_group_columns() {
        let plan = SurfacePlan {
            cell: 512,
            width: 2,
            grid: 2,
            atlas: 1024,
            surface_of_row: vec![0, 0],
            rows: vec![[0.0; 16], [1.0; 16]],
            top_enabled_by_row: vec![false, false],
            room_keys: vec!["room".into()],
            room_atlas: 1024,
            room_of_row: vec![0, 0],
            palette_source: vec![[0, 0], [0, 0]],
            room_params: vec![[0.0, 0.0]],
        };
        let bytes = parameter_table_bytes(&plan);
        assert_eq!(bytes.len(), 4 * 2 * 4 * 4);
        let floats: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|value| f32::from_le_bytes(value.try_into().unwrap()))
            .collect();
        let line = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        for actual in floats.chunks_exact(8) {
            assert_eq!(actual, &line);
        }
    }

    #[test]
    fn native_templates_keep_their_expected_identity() {
        assert_eq!(FLOAT_TEXTURE_TEMPLATE.len(), 4_084);
        assert_eq!(RGBA_TEXTURE_TEMPLATE.len(), 33_140);
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(FLOAT_TEXTURE_TEMPLATE)),
            "0c9669599d2ad19cdff6073d06edf24906e055388f354fc9557d4f2b200eea15"
        );
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(RGBA_TEXTURE_TEMPLATE)),
            "0232c2abf4fa7bda376b8caa27fb461e65b7f22a2a6c0a8fa2856cb71afc41ae"
        );
    }
}
