//! 冒烟探针：facade 世界投影 UV 数学取证（阶段 4）。
//!
//! 用法：cargo run -p rw4 --release --example facade_probe -- <package> 0xMODEL

const RW4_IMAGE: u32 = 0x2F4E_681B;
const RASTER_IMAGE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, model) = args.split_first().expect("usage: facade_probe <package> 0xMODEL");
    let instance = u32::from_str_radix(model[0].trim_start_matches("0x"), 16).expect("instance");
    let package = dbpf::Package::open(path).expect("open package");
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == RW4_IMAGE && e.id.instance == instance)
        .cloned()
        .expect("model not found");
    let data = package.read(&entry).expect("read model");
    let file = rw4::Rw4File::parse(&data).expect("parse rw4");

    // VertexFormat 声明
    for section in file.sections_of_type(rw4::SectionType::VERTEX_FORMAT) {
        if let Ok(format) = file.decode_vertex_format(&data, section.number) {
            println!("VertexFormat #{}: elements:", section.number);
            for element in &format.elements {
                println!(
                    "    usage={:?} type={:?} index={} offset={}",
                    element.usage, element.decl_type, element.index, element.offset
                );
            }
        }
    }

    // 逐 mesh FLOAT4 统计
    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(&data, section.number) else {
            continue;
        };
        let mut min = [f32::MAX; 4];
        let mut max = [f32::MIN; 4];
        let mut count = 0usize;
        for v in &mesh.vertices {
            for (element, value) in &v.components {
                if element.usage != rw4::DeclarationUsage::TexCoord {
                    continue;
                }
                if let rw4::ComponentValue::Float4(f) = value {
                    count += 1;
                    for c in 0..4 {
                        min[c] = min[c].min(f[c]);
                        max[c] = max[c].max(f[c]);
                    }
                }
            }
        }
        if count == 0 {
            continue;
        }
        println!(
            "mesh #{}: {count} floats, X[{:.2},{:.2}] Y[{:.2},{:.2}] Z[{:.2},{:.2}] W[{:.2},{:.2}]",
            section.number, min[0], max[0], min[1], max[1], min[2], max[2], min[3], max[3]
        );
    }

    // ---- 求解器：材质参数表 + tint map + palette + 顶点样本 ----
    let mesh = file
        .sections_of_type(rw4::SectionType::MESH)
        .find_map(|s| {
            file.decode_mesh(&data, s.number)
                .ok()
                .filter(|m| m.vertices.iter().any(|v| v.d3d_color_g().is_some()))
        });
    let Some(mesh) = mesh else {
        println!("no mesh with D3DCOLOR");
        return;
    };

    let mut params: Option<(Vec<[f32; 4]>, usize, usize)> = None;
    let mut tint: Option<(Vec<u8>, usize, usize)> = None;
    let mut palette: Option<(Vec<u8>, usize, usize)> = None;
    'materials: for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
        let Ok(rw4::MaterialSection::Decoded(mat)) = file.decode_material(&data, section.number)
        else {
            continue;
        };
        let resolve = |instance: u32| -> Option<(Vec<u8>, usize, usize)> {
            let tex_entry = package
                .entries()
                .iter()
                .find(|e| {
                    e.id.instance == instance
                        && (e.id.type_id == RASTER_IMAGE || e.id.type_id == RW4_IMAGE)
                })
                .cloned()?;
            let tex_data = package.read(&tex_entry).ok()?;
            if tex_entry.id.type_id == RASTER_IMAGE {
                let raster = rw4::RasterImage::parse(&tex_data).ok()?;
                let width = raster.width as usize;
                let height = raster.height as usize;
                Some((raster.decode_top_mip_rgba().ok()?, width, height))
            } else {
                let tex_file = rw4::Rw4File::parse(&tex_data).ok()?;
                let sec = tex_file
                    .sections_of_type(rw4::SectionType::TEXTURE)
                    .next()?
                    .number;
                let tex = tex_file.decode_texture(&tex_data, sec).ok()?;
                let width = usize::try_from(tex.width).unwrap_or(0);
                let height = usize::try_from(tex.height).unwrap_or(0);
                Some((tex.decode_top_mip_rgba().ok()?, width, height))
            }
        };
        if params.is_none() {
            if let Some(instance) = mat.slot_texture(0) {
                let tex_entry = package
                    .entries()
                    .iter()
                    .find(|e| e.id.instance == instance && e.id.type_id == RW4_IMAGE)
                    .cloned();
                if let Some(tex_entry) = tex_entry {
                    if let Ok(tex_data) = package.read(&tex_entry) {
                        if let Ok(tex_file) = rw4::Rw4File::parse(&tex_data) {
                            if let Some(sec) = tex_file
                                .sections_of_type(rw4::SectionType::TEXTURE)
                                .next()
                                .map(|s| s.number)
                            {
                                if let Ok(tex) = tex_file.decode_texture(&tex_data, sec) {
                                    if let Ok(pixels) = tex.decode_palette_f32() {
                                        params = Some((
                                            pixels,
                                            usize::from(tex.width),
                                            usize::from(tex.height),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if tint.is_none() {
            if let Some(instance) = mat.slot_texture(1) {
                tint = resolve(instance);
            }
        }
        if palette.is_none() {
            if let Some(instance) = mat.slot_texture(4) {
                palette = resolve(instance);
            }
        }
        if params.is_some() && tint.is_some() && palette.is_some() {
            break 'materials;
        }
    }
    let (
        Some((params, param_cols, param_rows)),
        Some((tint_rgba, tint_w, tint_h)),
        Some((pal_rgba, pal_w, pal_h)),
    ) = (params, tint, palette)
    else {
        println!("missing slot0/slot1/slot4 for solver");
        return;
    };
    println!(
        "solver: params {param_cols}x{param_rows}, tint {tint_w}x{tint_h}, palette {pal_w}x{pal_h}"
    );

    // 顶点样本（f4 + G）
    let samples: Vec<([f32; 4], u8)> = mesh
        .vertices
        .iter()
        .take(1500)
        .filter_map(|v| {
            let g = v.d3d_color_g()?;
            let (_, value) = v
                .components
                .iter()
                .find(|(e, _)| e.usage == rw4::DeclarationUsage::TexCoord)?;
            let f = match value {
                rw4::ComponentValue::Float4(f) => *f,
                _ => return None,
            };
            Some((f, g))
        })
        .collect();
    println!("samples: {}", samples.len());

    let tint_texel = |u: f32, v: f32| -> Option<[u8; 4]> {
        if !u.is_finite() || !v.is_finite() {
            return None;
        }
        let fu = u - u.floor();
        let fv = v - v.floor();
        let x = ((fu * tint_w as f32) as usize).min(tint_w - 1);
        let y = ((fv * tint_h as f32) as usize).min(tint_h - 1);
        let px = tint_rgba.get((y * tint_w + x) * 4..)?;
        Some([px[0], px[1], px[2], px[3]])
    };

    // ---- regionXform 行 oracle：texel(m, r) = params[r*cols + m] ----
    println!("--- regionXform row oracle (alpha>0.5 rate + per-G tint.rg spread) ---");
    for candidate_row in 0..param_rows {
        let mut alpha_pass = 0usize;
        let mut tested = 0usize;
        let mut rg_by_g: std::collections::BTreeMap<u8, Vec<[f32; 2]>> =
            std::collections::BTreeMap::new();
        for (f, g) in &samples {
            let m = *g as usize;
            let Some(texel) = params.get(candidate_row * param_cols + m).copied() else {
                continue;
            };
            let bu = (f[0] - f[0].floor()) * texel[0] + texel[2];
            let bv = (f[1] - f[1].floor()) * texel[1] + texel[3];
            if let Some(t) = tint_texel(bu, bv) {
                tested += 1;
                if u32::from(t[3]) > 128 {
                    alpha_pass += 1;
                }
                rg_by_g
                    .entry(*g)
                    .or_default()
                    .push([f32::from(t[0]), f32::from(t[1])]);
            }
        }
        let mut spread_sum = 0f64;
        let mut spread_n = 0usize;
        for uvs in rg_by_g.values() {
            if uvs.len() < 2 {
                continue;
            }
            let mean = [
                uvs.iter().map(|c| c[0]).sum::<f32>() / uvs.len() as f32,
                uvs.iter().map(|c| c[1]).sum::<f32>() / uvs.len() as f32,
            ];
            for c in uvs {
                spread_sum += f64::from((c[0] - mean[0]).abs() + (c[1] - mean[1]).abs());
                spread_n += 1;
            }
        }
        let spread = if spread_n > 0 {
            spread_sum / spread_n as f64
        } else {
            f64::MAX
        };
        println!(
            "    row{candidate_row}: alpha>0.5={:.1}%  per-G tint.rg spread={:.2}",
            if tested > 0 {
                alpha_pass as f64 / tested as f64 * 100.0
            } else {
                0.0
            },
            spread
        );
    }

    // ---- 最终色烘焙 oracle：palette 查色后 per-G 颜色一致性 ----
    println!("--- final color bake oracle (palette lookup, per-G color spread) ---");
    for xform_row in 0..param_rows {
        for pal_row in 0..param_rows {
            let mut by_g: std::collections::BTreeMap<u8, Vec<[f32; 3]>> =
                std::collections::BTreeMap::new();
            for (f, g) in &samples {
                let m = *g as usize;
                let Some(xform) = params.get(xform_row * param_cols + m).copied() else {
                    continue;
                };
                let Some(pal_origin) = params.get(pal_row * param_cols + m).copied() else {
                    continue;
                };
                let bu = (f[0] - f[0].floor()) * xform[0] + xform[2];
                let bv = (f[1] - f[1].floor()) * xform[1] + xform[3];
                let Some(t) = tint_texel(bu, bv) else {
                    continue;
                };
                let pu = pal_origin[0] + f32::from(t[0]) / 255.0 * 0.125 + 1.0 / 1024.0;
                let pv = pal_origin[1] + f32::from(t[1]) / 255.0 * 0.125 + 1.0 / 32.0;
                let x = ((pu - pu.floor()).clamp(0.0, 0.999) * pal_w as f32) as usize;
                let y = ((pv - pv.floor()).clamp(0.0, 0.999) * pal_h as f32) as usize;
                let Some(px) = pal_rgba.get((y * pal_w + x) * 4..) else {
                    continue;
                };
                let bright = f32::from(t[2]) / 255.0 * 2.0;
                by_g.entry(*g).or_default().push([
                    f32::from(px[0]) * bright,
                    f32::from(px[1]) * bright,
                    f32::from(px[2]) * bright,
                ]);
            }
            let mut spread_sum = 0f64;
            let mut spread_n = 0usize;
            let mut groups = 0usize;
            for colors in by_g.values() {
                if colors.len() < 2 {
                    continue;
                }
                groups += 1;
                let mean = [
                    colors.iter().map(|c| c[0]).sum::<f32>() / colors.len() as f32,
                    colors.iter().map(|c| c[1]).sum::<f32>() / colors.len() as f32,
                    colors.iter().map(|c| c[2]).sum::<f32>() / colors.len() as f32,
                ];
                for c in colors {
                    spread_sum += f64::from(
                        (c[0] - mean[0]).abs() + (c[1] - mean[1]).abs() + (c[2] - mean[2]).abs(),
                    );
                    spread_n += 1;
                }
            }
            let spread = if spread_n > 0 {
                spread_sum / spread_n as f64
            } else {
                f64::MAX
            };
            if spread < 200.0 {
                println!(
                    "    xform=row{xform_row} pal=row{pal_row}: groups={groups} spread={spread:.1}"
                );
            }
        }
    }
}
