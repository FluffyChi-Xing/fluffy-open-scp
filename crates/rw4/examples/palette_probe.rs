//! 冒烟探针：统计 slot0 调色板的尺寸分布与行间差异（阶段 1 校准前置取证）。
//!
//! 用法：cargo run -p rw4 --release --example palette_probe -- <package>

use std::collections::BTreeMap;

const RW4_IMAGE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: palette_probe <package>");
    let package = dbpf::Package::open(path).expect("open package");

    // instance → 调色板纹理（RW4 包裹 textureType 116），懒解析缓存
    let mut palette_cache: std::collections::HashMap<u32, Option<(u32, u32, usize, Vec<[f32; 4]>)>> =
        std::collections::HashMap::new();
    let mut size_hist: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    let mut palettes = 0usize;
    let mut columns_total = 0usize;
    let mut columns_distinct = 0usize; // row0 与 row1 差异显著（任一通道 > 0.1）
    let mut columns_multi_row = 0usize; // 高度 > 2（存在更多行变体）

    for entry in package.entries() {
        if entry.id.type_id != RW4_IMAGE {
            continue;
        }
        let entry = entry.clone();
        let Ok(data) = package.read(&entry) else { continue };
        let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
        let Some(mat_sec) = file
            .sections_of_type(rw4::SectionType::MATERIAL)
            .next()
            .map(|s| s.number)
        else {
            continue;
        };
        let Ok(rw4::MaterialSection::Decoded(mat)) = file.decode_material(&data, mat_sec) else {
            continue;
        };
        let Some(slot0) = mat
            .texture_slots()
            .find(|r| r.slot_byte() == 0)
            .map(|r| r.texture_instance)
            .filter(|i| *i != 0)
        else {
            continue;
        };
        let cached = palette_cache.entry(slot0).or_insert_with(|| {
            let Some(tex_entry) = package
                .entries()
                .iter()
                .find(|e| e.id.instance == slot0 && e.id.type_id == RW4_IMAGE)
                .cloned()
            else {
                return None;
            };
            let Ok(tex_data) = package.read(&tex_entry) else { return None };
            let Ok(tex_file) = rw4::Rw4File::parse(&tex_data) else { return None };
            let Some(tex_sec) = tex_file
                .sections_of_type(rw4::SectionType::TEXTURE)
                .next()
                .map(|s| s.number)
            else {
                return None;
            };
            let Ok(tex) = tex_file.decode_texture(&tex_data, tex_sec) else { return None };
            if tex.texture_type != rw4::TEXTURE_TYPE_PALETTE_F32 {
                return None;
            }
            let w = u32::from(tex.width);
            let h = u32::from(tex.height);
            let Ok(pixels) = tex.decode_palette_f32() else { return None };
            Some((w, h, pixels.len(), pixels))
        });
        let Some((w, h, _len, pixels)) = cached else { continue };
        palettes += 1;
        *size_hist.entry((*w, *h)).or_insert(0) += 1;
        columns_total += *w as usize;
        if *h > 2 {
            columns_multi_row += *w as usize;
        }
        for x in 0..*w as usize {
            let row0 = pixels.get(x).copied().unwrap_or([1.0; 4]);
            let row1 = pixels.get(x + *w as usize).copied().unwrap_or(row0);
            let diff = (0..3).any(|c| (row0[c] - row1[c]).abs() > 0.1);
            if diff {
                columns_distinct += 1;
            }
        }
    }

    println!("palettes={palettes} columns_total={columns_total} columns_row0_ne_row1={columns_distinct} columns_height_gt2={columns_multi_row}");
    println!("size histogram (w x h -> count):");
    for ((w, h), count) in &size_hist {
        println!("  {w}x{h}: {count}");
    }
}
