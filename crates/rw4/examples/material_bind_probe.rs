//! 冒烟探针：验证 RW4 模型 mesh↔material↔texture 绑定链（仅开发用）。
//!
//! 统计全部模型的 material 纹理引用（含跨包解析率）；detail 模式深挖一个
//! 模型：section 布局、材质引用表、顶点 D3DCOLOR 元素分段、UV 通道、
//! 外部贴图资源实况。
//!
//! 用法：cargo run -p rw4 --example material_bind_probe -- \
//!   <package> <detail_instance|-> <max|-> [lookup_package ...]

use std::collections::HashMap;

const RW4_IMAGE: u32 = 0x2F4E_681B;
const RASTER_IMAGE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: material_bind_probe <package> <detail|-> <max|-> [lookup...]");
    let detail = args.get(1).filter(|v| v.as_str() != "-")
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
    let max_print: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(8);

    let package = dbpf::Package::open(path).expect("open main package");
    let lookups: Vec<_> = args.iter().skip(3)
        .map(|p| dbpf::Package::open(p).expect("open lookup package"))
        .collect();

    // 贴图资源索引：instance → (type_id, 来源包序号)
    let mut textures: HashMap<u32, (u32, usize)> = HashMap::new();
    for (i, pkg) in std::iter::once(&package).chain(lookups.iter()).enumerate() {
        for entry in pkg.entries() {
            if entry.id.type_id == RW4_IMAGE || entry.id.type_id == RASTER_IMAGE {
                textures.entry(entry.id.instance).or_insert((entry.id.type_id, i));
            }
        }
    }
    println!("--- texture index: {} instances (main rw4={} raster={})",
        textures.len(),
        package.entries().iter().filter(|e| e.id.type_id == RW4_IMAGE).count(),
        package.entries().iter().filter(|e| e.id.type_id == RASTER_IMAGE).count());

    let mut models = 0usize;
    let mut with_material = 0usize;
    let mut material_count = 0usize;
    let mut material_raw = 0usize;
    let mut mesh_count = 0usize;
    let mut models_multi_material = 0usize;
    let mut models_dual_uv = 0usize;
    let mut with_float2_uv = 0usize;
    let mut float4_uv_small = 0usize;
    let mut float4_uv_huge = 0usize;
    let mut shape_hist: std::collections::BTreeMap<(usize, usize), usize> = std::collections::BTreeMap::new();
    let mut slot_hist = [[0usize; 2]; 6]; // [slot][解析成功数, 本包命中数]
    let mut multi_layout: Option<(u32, Vec<(u32, String)>)> = None;
    let mut float2_models: Vec<u32> = Vec::new();
    let mut printed = 0usize;

    for entry in package.entries() {
        if entry.id.type_id != RW4_IMAGE { continue; }
        if Some(entry.id.instance) == detail { continue; }
        let entry = entry.clone();
        models += 1;
        let data = match package.read(&entry) { Ok(d) => d, Err(_) => continue };
        let file = match rw4::Rw4File::parse(&data) { Ok(f) => f, Err(_) => continue };

        let mut line = format!("0x{:08X}", entry.id.instance);
        let mut has_mat = false;
        let mut mat_sections = 0usize;
        let mut has_uv = false;
        let mut has_float4 = false;
        let mut has_dual_uv = false;
        let mut f4_max_xy = 0f32;
        for mesh_sec in file.sections_of_type(rw4::SectionType::MESH) {
            mesh_count += 1;
            if let Ok(m) = file.decode_mesh(&data, mesh_sec.number) {
                for v in &m.vertices {
                    if v.has_float2_uv() { has_uv = true; }
                    let mut tex_channels = 0usize;
                    for (el, val) in &v.components {
                        if el.usage == rw4::DeclarationUsage::TexCoord {
                            tex_channels += 1;
                            if let rw4::ComponentValue::Float4(f) = val {
                                has_float4 = true;
                                f4_max_xy = f4_max_xy.max(f[0].abs()).max(f[1].abs());
                            }
                        }
                    }
                    if tex_channels >= 2 { has_dual_uv = true; }
                }
            }
        }
        if has_dual_uv { models_dual_uv += 1; }
        if has_uv {
            line.push_str(" uv=float2");
            if float2_models.len() < 12 {
                float2_models.push(entry.id.instance);
            }
        }
        if has_uv { with_float2_uv += 1; }
        else if has_float4 {
            if f4_max_xy <= 8.0 { float4_uv_small += 1; } else { float4_uv_huge += 1; }
        }
        for mat_sec in file.sections_of_type(rw4::SectionType::MATERIAL) {
            mat_sections += 1;
            let mat = match file.decode_material(&data, mat_sec.number) {
                Ok(rw4::MaterialSection::Decoded(m)) => m,
                Ok(rw4::MaterialSection::Raw(_)) => { material_raw += 1; continue; }
                Err(_) => continue,
            };
            has_mat = true;
            material_count += 1;
            let mut parts = Vec::new();
            for r in mat.texture_slots() {
                let slot = r.slot_byte() as usize;
                if slot >= 6 { continue; }
                match textures.get(&r.texture_instance) {
                    Some((_, pkg_idx)) => {
                        slot_hist[slot][0] += 1;
                        if *pkg_idx == 0 { slot_hist[slot][1] += 1; }
                        parts.push(format!("s{}:0x{:08X}{}", slot, r.texture_instance,
                            if *pkg_idx == 0 { "" } else { "*" }));
                    }
                    None => parts.push(format!("s{}:0x{:08X}?", slot, r.texture_instance)),
                }
            }
            if printed < max_print {
                line.push_str(&format!("  mat#{} [{}]", mat_sec.number, parts.join(" ")));
            }
        }
        if has_mat {
            with_material += 1;
            if mat_sections > 1 { models_multi_material += 1; }
        }
        let mesh_sections_n = mesh_count_sections(&file);
        *shape_hist.entry((mesh_sections_n, mat_sections)).or_insert(0) += 1;
        if multi_layout.is_none() && mesh_count_sections(&file) > 1 && mat_sections > 1 {
            // 仅选全部 mesh 可解码的静态模型（0x41B1BAC0 为蒙皮变体，解码失败）
            let decoded_ok = file.sections_of_type(rw4::SectionType::MESH).filter_map(|s| file.decode_mesh(&data, s.number).ok()).count() == mesh_count_sections(&file);
            if decoded_ok {
                let layout = file
                    .sections()
                    .iter()
                    .map(|sec| {
                        (
                            sec.number,
                            sec.type_name().unwrap_or("??").to_string(),
                        )
                    })
                    .collect();
                multi_layout = Some((entry.id.instance, layout));
            }
        }
        if has_mat && printed < max_print {
            println!("{line}");
            printed += 1;
        }
    }

    println!("--- float2-UV models (up to 12):");
    for instance in &float2_models {
        println!("    0x{instance:08X}");
    }
    println!("--- (mesh,material) shape histogram:");
    for ((m, s), count) in &shape_hist {
        println!("    {m} mesh / {s} material: {count} models");
    }
    println!("--- models={models} with_material={with_material} materials={material_count} raw={material_raw} meshes={mesh_count} models_multi_material={models_multi_material} models_dual_uv={models_dual_uv} with_float2_uv={with_float2_uv} float4_uv_small(<=8)={float4_uv_small} float4_uv_huge={float4_uv_huge}");
    if let Some((instance, layout)) = multi_layout {
        println!("--- multi-mesh section layout of 0x{instance:08X}:");
        for (number, ty) in layout {
            println!("    #{number:<3} {ty}");
        }
        if let Some(entry) = package
            .entries()
            .iter()
            .find(|e| e.id.instance == instance && e.id.type_id == RW4_IMAGE)
            .cloned()
        {
            if let Ok(data) = package.read(&entry) {
                if let Ok(file) = rw4::Rw4File::parse(&data) {
                    for sec in file.sections() {
                        if sec.type_name() == Some("MeshMaterialAssignment") {
                            let start = sec.pos as usize;
                            let end = (start + sec.size as usize).min(data.len());
                            println!(
                                "    assignment #{}: type=0x{:08X} size={} bytes={:02x?}",
                                sec.number,
                                sec.type_code,
                                sec.size,
                                &data[start..end],
                            );
                        }
                    }
                }
            }
        }
    }
    for (s, [ok, local]) in slot_hist.iter().enumerate() {
        println!("    slot{s}: resolved={ok} (in_main={local})");
    }

    if let Some(inst) = detail {
        dump_detail(&package, inst);
    }
}

fn mesh_count_sections(file: &rw4::Rw4File) -> usize {
    file.sections_of_type(rw4::SectionType::MESH).count()
}

fn dump_detail(package: &dbpf::Package, instance: u32) {
    let perf = std::time::Instant::now();
    let entry = package.entries().iter()
        .find(|e| e.id.instance == instance && e.id.type_id == RW4_IMAGE)
        .expect("model instance not found").clone();
    let data = package.read(&entry).expect("read model");
    let file = rw4::Rw4File::parse(&data).expect("parse rw4");
    println!("\n=== detail 0x{instance:08X} ===");
    println!("sections: {}", file.sections().len());
    for sec in file.sections() {
        println!("  #{:<3} {:<20} size={:<7} align={}", sec.number,
            sec.type_name().unwrap_or("??"), sec.size, sec.alignment);
        let ty = sec.type_name().unwrap_or("");
        if matches!(ty, "Mesh" | "TriangleArray" | "VertexArray") {
            let start = sec.pos as usize;
            let end = (start + sec.size as usize).min(data.len());
            println!("       bytes: {:02x?}", &data[start..end.min(start + 40)]);
        }
        if ty == "Mesh" {
            // 解析扩展头并切 TA 索引切片，验证索引是否池相对
            let start = sec.pos as usize;
            let w = |i: usize| u32::from_le_bytes(data[start + i * 4..start + i * 4 + 4].try_into().unwrap());
            let (tri_sec, tri_count, start_index, index_count, f7, vert_count) =
                (w(2), w(3), w(5), w(6), w(7), w(8));
            let ta = &file.sections()[tri_sec as usize];
            let ta_start = ta.pos as usize;
            let ta_index_count = u32::from_le_bytes(data[ta_start + 8..ta_start + 12].try_into().unwrap());
            let blob_sec = u32::from_le_bytes(data[ta_start + 24..ta_start + 28].try_into().unwrap());
            let blob = &file.sections()[blob_sec as usize];
            let blob_start = blob.pos as usize;
            let lo = (start_index * 2) as usize;
            let hi = lo + (index_count * 2) as usize;
            let slice = &data[blob_start + lo..blob_start + hi];
            let mut mn = u16::MAX;
            let mut mx = 0u16;
            for i in (0..slice.len()).step_by(2) {
                let v = u16::from_le_bytes([slice[i], slice[i + 1]]);
                mn = mn.min(v);
                mx = mx.max(v);
            }
            println!("       tri_sec={tri_sec} tri={tri_count} start_idx={start_index} idx_count={index_count} f7={f7} verts={vert_count} | TA total_idx={ta_index_count} slice idx range=[{mn},{mx}]");
        }
    }

    for mat_sec in file.sections_of_type(rw4::SectionType::MATERIAL) {
        let mat = match file.decode_material(&data, mat_sec.number) {
            Ok(rw4::MaterialSection::Decoded(m)) => m,
            _ => { println!("material #{}: RAW/unparsed", mat_sec.number); continue; }
        };
        println!("material #{}: size={} header={} additional={}B",
            mat_sec.number, mat.size, hex(&mat.header), mat.additional_data.len());
        for r in &mat.texture_refs {
            println!("  ref slot=0x{:08X} instance=0x{:08X} unk=0x{:08X}/0x{:08X}/0x{:08X}/0x{:08X}",
                r.slot, r.texture_instance, r.unknown2, r.unknown3, r.unknown4, r.unknown5);
            dump_texture_resource(package, r.texture_instance);
            if let Some(stats) = texture_channel_stats(package, r.texture_instance) {
                println!("       channel mean R={:.1} G={:.1} B={:.1} A={:.1}", stats[0], stats[1], stats[2], stats[3]);
            }
        }
    }

    for mesh_sec in file.sections_of_type(rw4::SectionType::MESH) {
        let mesh = match file.decode_mesh(&data, mesh_sec.number) {
            Ok(m) => m,
            Err(e) => { println!("mesh #{}: ERR {e}", mesh_sec.number); continue; }
        };
        // 精细渲染链耗时：D3DCOLOR 顶点色映射
        let _ = std::time::Instant::now();
        let colored: Vec<[f32; 3]> = mesh.vertices.iter()
            .map(|v| v.d3d_color_g().map(|_| [0.5f32, 0.5, 0.5]).unwrap_or([1.0; 3]))
            .collect();
        println!("mesh #{}: tris={} verts={} (color-map {} verts, +{:?})",
            mesh_sec.number, mesh.triangles.len(), mesh.vertices.len(), colored.len(), perf.elapsed());
        println!("  total elapsed since parse start: {:?}", perf.elapsed());
        let vert_elem = vertex_elements(&mesh);
        let mut elems: Vec<(usize, [u8; 4])> = Vec::new();
        for (vi, e) in vert_elem.iter().enumerate() {
            if *e as usize == elems.len() {
                elems.push((*e as usize, color_of(&mesh.vertices[vi]).unwrap_or([0; 4])));
            }
        }
        println!("  elements(D3DCOLOR segmentation): {}", elems.len());
        for (i, (e, c)) in elems.iter().take(10).enumerate() {
            println!("    elem{i}(#{e}): argb=({},{},{},{})", c[0], c[1], c[2], c[3]);
        }
        let mut tri_elem = std::collections::HashMap::new();
        for t in &mesh.triangles {
            *tri_elem.entry(vert_elem[t[0] as usize]).or_insert(0usize) += 1;
        }
        let mut tris: Vec<_> = tri_elem.into_iter().collect();
        tris.sort();
        if tris.len() <= 12 {
            println!("  triangles per element: {tris:?}");
        } else {
            println!("  triangles per element: {} groups, top: {:?}",
                tris.len(), &tris[..6]);
        }
        let mut uv2 = 0usize; let mut uv4 = 0usize; let mut f4max = 0f32; let mut normals = 0usize;
        for v in &mesh.vertices {
            let mut has2 = false; let mut has4 = false;
            for (el, val) in &v.components {
                match (el.usage, val) {
                    (rw4::DeclarationUsage::TexCoord, rw4::ComponentValue::Float2(_)) => has2 = true,
                    (rw4::DeclarationUsage::TexCoord, rw4::ComponentValue::Float4(f)) => {
                        has4 = true;
                        f4max = f4max.max(f[0].abs()).max(f[1].abs()).max(f[2].abs()).max(f[3].abs());
                    }
                    (rw4::DeclarationUsage::Normal, _) => normals += 1,
                    _ => {}
                }
            }
            if has2 { uv2 += 1; } else if has4 { uv4 += 1; }
        }
        println!("  uv: FLOAT2 verts={uv2} FLOAT4-only verts={uv4} (FLOAT4 max|val|={f4max:.2}) normals={normals}");
    }
}


fn texture_channel_stats(package: &dbpf::Package, instance: u32) -> Option<[f32; 4]> {
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.instance == instance && (e.id.type_id == RW4_IMAGE || e.id.type_id == RASTER_IMAGE))
        .cloned()?;
    let data = package.read(&entry).ok()?;
    let rgba = if entry.id.type_id == RASTER_IMAGE {
        rw4::RasterImage::parse(&data).ok()?.decode_top_mip_rgba().ok()?
    } else {
        let file = rw4::Rw4File::parse(&data).ok()?;
        let sec = file.sections_of_type(rw4::SectionType::TEXTURE).next()?.number;
        file.decode_texture(&data, sec).ok()?.decode_top_mip_rgba().ok()?
    };
    let n = (rgba.len() / 4).max(1);
    let mut sum = [0f64; 4];
    for px in rgba.as_chunks::<4>().0 {
        for c in 0..4 {
            sum[c] += f64::from(px[c]);
        }
    }
    Some([
        (sum[0] / n as f64) as f32,
        (sum[1] / n as f64) as f32,
        (sum[2] / n as f64) as f32,
        (sum[3] / n as f64) as f32,
    ])
}
fn dump_texture_resource(package: &dbpf::Package, instance: u32) {
    let Some(entry) = package.entries().iter()
        .find(|e| e.id.instance == instance
            && (e.id.type_id == RASTER_IMAGE || e.id.type_id == RW4_IMAGE))
        .map(|e| e.clone())
    else {
        println!("    -> resource 0x{instance:08X}: NOT in this package");
        return;
    };
    let data = match package.read(&entry) { Ok(d) => d, Err(e) => { println!("    -> read err {e}"); return; } };
    if entry.id.type_id == RASTER_IMAGE {
        match rw4::RasterImage::parse(&data) {
            Ok(r) => println!("    -> raster {}x{} mips={} pixFmt={} {}", r.width, r.height, r.mip_count, r.pixel_format,
                if r.is_raw_rgba() { "raw RGBA" } else { "compressed" }),
            Err(e) => println!("    -> raster parse err {e}"),
        }
        return;
    }
    let file = match rw4::Rw4File::parse(&data) { Ok(f) => f, Err(e) => { println!("    -> rw4 parse err {e}"); return; } };
    let sec = match file.sections_of_type(rw4::SectionType::TEXTURE).next() {
        Some(s) => s.number,
        None => { println!("    -> rw4 file without Texture section"); return; }
    };
    match file.decode_texture(&data, sec) {
        Ok(tex) => {
            println!("    -> rw4 texture type=0x{:08X} {}x{} mips={}", tex.texture_type, tex.width, tex.height, tex.mip_count());
            if tex.texture_type == rw4::TEXTURE_TYPE_PALETTE_F32 {
                // 4×f32/像素；C# DecodePaletteLuminance 取每像素第 1 个 f32 为亮度
                let px = (tex.width as usize).min(tex.blob.len() / 16);
                let vals: Vec<String> = (0..px).take(12)
                    .map(|x| f32::from_le_bytes(tex.blob[x * 16..x * 16 + 4].try_into().unwrap()).to_string())
                    .collect();
                println!("       palette row0 first floats: [{}]", vals.join(", "));
            } else {
                match tex.decode_top_mip_rgba() {
                    Ok(rgba) => println!("       decoded {}px", rgba.len() / 4),
                    Err(e) => println!("       decode err {e}"),
                }
            }
        }
        Err(e) => println!("    -> texture decode err {e}"),
    }
}

fn color_of(v: &rw4::DecodedVertex) -> Option<[u8; 4]> {
    v.components.iter().find_map(|(_, val)| match val {
        rw4::ComponentValue::D3DColor { a, r, g, b } => Some([*a, *r, *g, *b]),
        _ => None,
    })
}

/// 每顶点元素号：C# RW4Mesh.Read——相邻顶点 D3DCOLOR 不同则开新元素。
fn vertex_elements(mesh: &rw4::DecodedMesh) -> Vec<u32> {
    let mut out = Vec::with_capacity(mesh.vertices.len());
    let mut prev: Option<[u8; 4]> = None;
    let mut elem = 0u32;
    for v in &mesh.vertices {
        let Some(c) = color_of(v) else { continue };
        if prev != Some(c) {
            elem += 1;
            prev = Some(c);
        }
        out.push(elem - 1);
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}
