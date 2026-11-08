//! Pure-Rust port of the author's single-mesh LOTM atlas compiler.
//! Reads a multi-material LOTM, deduplicates materials/triples, builds
//! 1024px tint/normal/shader atlases, a horizontal 2-column palette and a
//! 46-row Float32 parameter table, then writes a one-material LOTM.

use fluffy_open_scp_lib::asset_build_m1::lotm;
use image::{RgbaImage, imageops::FilterType};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Cursor;

const CELL: u32 = 512;
const GRID: u32 = 5;
const ATLAS: u32 = CELL * GRID;

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn params(m: &lotm::LotmMaterial) -> Vec<f32> {
    m.params_f32
        .as_deref()
        .unwrap_or(&[])
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn png(m: &lotm::LotmMaterial, i: usize) -> Vec<u8> {
    m.pngs.get(i).and_then(|v| v.clone()).unwrap_or_default()
}

fn patch_column(glb: &[u8], column: u8) -> Vec<u8> {
    let mut b = glb.to_vec();
    let jlen = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    let json: serde_json::Value = serde_json::from_slice(&b[20..20 + jlen]).unwrap();
    let bin_start = 20 + jlen + 8;
    let Some(meshes) = json["meshes"].as_array() else {
        return b;
    };
    for mesh in meshes {
        let Some(prims) = mesh["primitives"].as_array() else {
            continue;
        };
        for prim in prims {
            let Some(idx) = prim["attributes"]["TEXCOORD_1"].as_u64() else {
                continue;
            };
            let Some(acc) = json["accessors"].get(idx as usize) else {
                continue;
            };
            if acc["type"] != "VEC2" || acc["componentType"] != 5126 {
                continue;
            }
            let Some(buffer_view) = acc["bufferView"].as_u64() else {
                continue;
            };
            let Some(view) = json["bufferViews"].get(buffer_view as usize) else {
                continue;
            };
            let stride = view["byteStride"].as_u64().unwrap_or(8) as usize;
            let start = bin_start
                + view["byteOffset"].as_u64().unwrap_or(0) as usize
                + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
            let count = acc["count"].as_u64().unwrap_or(0) as usize;
            for i in 0..count {
                let at = start + i * stride;
                b[at..at + 4].copy_from_slice(&(column as f32 / 255.0).to_le_bytes());
            }
        }
    }
    b
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .ok_or("usage: single_compatible <input.lotm> <output.lotm>")?;
    let output = env::args()
        .nth(2)
        .ok_or("usage: single_compatible <input.lotm> <output.lotm>")?;
    let bytes = fs::read(input)?;
    let doc = lotm::parse(&bytes)?;
    if doc.materials.is_empty() {
        return Err("input has no materials".into());
    }

    // Full material rows (palette/params included).
    let mut row_of_key: HashMap<String, usize> = HashMap::new();
    let mut row_sources: Vec<usize> = Vec::new();
    let mut mesh_rows = Vec::new();
    for (i, m) in doc.materials.iter().enumerate() {
        let key = format!(
            "{}:{}:{}:{}:{}",
            hash(&png(m, 4)),
            hash(&png(m, 1)),
            hash(&png(m, 6)),
            hash(&png(m, 5)),
            hash(m.params_f32.as_deref().unwrap_or(&[]))
        );
        let row = *row_of_key.entry(key).or_insert_with(|| {
            row_sources.push(i);
            row_sources.len() - 1
        });
        mesh_rows.push(row);
    }
    if row_sources.len() > 63 {
        return Err(format!("{} unique rows exceed 63", row_sources.len()).into());
    }

    // Unique tint/normal/shader triples share one atlas cell.
    let mut triple_cell: HashMap<String, usize> = HashMap::new();
    let mut triple_source: Vec<usize> = Vec::new();
    for (i, m) in doc.materials.iter().enumerate() {
        let key = format!(
            "{}:{}:{}",
            hash(&png(m, 4)),
            hash(&png(m, 1)),
            hash(&png(m, 6))
        );
        if !triple_cell.contains_key(&key) {
            triple_cell.insert(key, triple_source.len());
            triple_source.push(i);
        }
    }
    if triple_source.len() > 63 {
        return Err(format!("{} unique triples exceed 63", triple_source.len()).into());
    }

    let mut tint = RgbaImage::new(ATLAS, ATLAS);
    let mut normal = RgbaImage::new(ATLAS, ATLAS);
    let mut shader = RgbaImage::new(ATLAS, ATLAS);
    for (cell, source) in triple_source.iter().enumerate() {
        let m = &doc.materials[*source];
        let x = (cell as u32 % GRID) * CELL;
        let y = (cell as u32 / GRID) * CELL;
        for (slot, target) in [(4usize, &mut tint), (1, &mut normal), (6, &mut shader)] {
            let data = png(m, slot);
            if data.is_empty() {
                continue;
            }
            let image = image::load_from_memory(&data)?.to_rgba8();
            let resized = image::imageops::resize(&image, CELL, CELL, FilterType::Nearest);
            image::imageops::overlay(target, &resized, x as i64, y as i64);
        }
    }
    // Transparent Top placeholder cell (62) avoids a zero region.
    let placeholder = 24usize;
    let (px, py) = (
        (placeholder as u32 % GRID) * CELL,
        (placeholder as u32 / GRID) * CELL,
    );
    for y in py..py + 4 {
        for x in px..px + 4 {
            tint.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            shader.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
        }
    }

    // Horizontal palette: each row owns two 1px columns (Base, Top).
    let mut palette = RgbaImage::new(512, 16);
    for (row, source) in row_sources.iter().enumerate() {
        let m = &doc.materials[*source];
        let p = params(m);
        let source_palette = image::load_from_memory(&png(m, 5))?.to_rgba8();
        let source_palette = image::imageops::resize(&source_palette, 512, 16, FilterType::Nearest);
        let base_col = (p.get(0).copied().unwrap_or(0.0) * 511.0).round() as u32;
        let top_col = (p.get(1).copied().unwrap_or(0.0) * 511.0).round() as u32;
        let block = (row * 4) as u32;
        for y in 0..16 {
            for offset in 0..2 {
                for (dest, col) in [
                    (block + offset, base_col + offset),
                    (block + 2 + offset, top_col + offset),
                ] {
                    palette.put_pixel(dest, y, *source_palette.get_pixel(col.min(511), y));
                }
            }
        }
    }

    // 4 rows x 46 columns Float32.
    let cols = row_sources.len();
    let mut table = vec![0f32; 4 * 64 * 4];
    let grid_count = GRID as f32;
    for (row, source) in row_sources.iter().enumerate() {
        let m = &doc.materials[*source];
        let mut p = params(m);
        if p.len() < 16 {
            p.resize(16, 0.0);
        }
        let cell = *triple_cell
            .get(&format!(
                "{}:{}:{}",
                hash(&png(m, 4)),
                hash(&png(m, 1)),
                hash(&png(m, 6))
            ))
            .unwrap_or(&0);
        let cx = ((cell as u32 % GRID) * CELL) as f32;
        let cy = ((cell as u32 / GRID) * CELL) as f32;
        let block = (row * 4) as f32;
        p[0] = block / 512.0;
        p[1] = (block + 2.0) / 512.0;
        p[2] = 0.0;
        p[3] = 0.0;
        let inset = 0.0 / CELL as f32;
        for r in [1usize, 2] {
            let o = r * 4;
            p[o] = p[o] / grid_count * (1.0 - 2.0 * inset);
            p[o + 1] = p[o + 1] / grid_count * (1.0 - 2.0 * inset);
            p[o + 2] = p[o + 2] / grid_count + (cx + 1.0) / ATLAS as f32;
            p[o + 3] = p[o + 3] / grid_count + (cy + 1.0) / ATLAS as f32;
        }
        if p[8] == 0.0 || p[9] == 0.0 {
            p[8] = 4.0 / ATLAS as f32;
            p[9] = 4.0 / ATLAS as f32;
            p[10] = px as f32 / ATLAS as f32;
            p[11] = py as f32 / ATLAS as f32;
        }
        p[14] *= grid_count;
        p[15] *= grid_count;
        for r in 0..4 {
            for c in 0..4 {
                table[(r * 64 + row) * 4 + c] = p[r * 4 + c];
            }
        }
    }

    // Patch each GLB's material row and write a one-material LOTM.
    let mut meshes_out = Vec::new();
    for (i, glb) in doc.meshes.iter().enumerate() {
        meshes_out.push(patch_column(glb, mesh_rows[i] as u8));
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"LOTM");
    out.extend_from_slice(&9u32.to_le_bytes());
    out.extend_from_slice(&(meshes_out.len() as u32).to_le_bytes());
    for glb in &meshes_out {
        out.extend_from_slice(&(glb.len() as u32).to_le_bytes());
        out.extend_from_slice(glb);
    }
    out.extend_from_slice(&1u32.to_le_bytes());
    let mut push_png = |data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    };
    push_png(&[]);
    let mut png_bytes = |img: &RgbaImage| {
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    };
    push_png(&png_bytes(&normal));
    push_png(&[]);
    push_png(&[]);
    push_png(&png_bytes(&tint));
    push_png(&png_bytes(&palette));
    push_png(&png_bytes(&shader));
    // Interior: reuse the first source slot5, else 8x8 transparent.
    let interior = doc
        .materials
        .iter()
        .map(|m| png(m, 7))
        .find(|v| !v.is_empty())
        .unwrap_or_else(|| png_bytes(&RgbaImage::new(8, 8)));
    push_png(&interior);
    let raw: Vec<u8> = table.iter().flat_map(|v| v.to_le_bytes()).collect();
    out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    out.extend_from_slice(&raw);
    out.extend_from_slice(&64u32.to_le_bytes());
    for _ in &meshes_out {
        out.extend_from_slice(&0u32.to_le_bytes());
        out.push(0);
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    fs::write(&output, out)?;
    println!(
        "WROTE {} meshes={} materials=1 rows={} triples={} bytes={}",
        &output,
        meshes_out.len(),
        cols,
        triple_source.len(),
        fs::metadata(&output)?.len()
    );
    Ok(())
}
