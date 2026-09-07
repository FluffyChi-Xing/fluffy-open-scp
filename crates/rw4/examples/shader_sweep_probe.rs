//! sweep 探针：全包统计 uvKind 分布 + 前端常数采样（TEXCOORD_0=0,0）整 mesh discard 预测。
//! 只在主包内解析材质资源（EP1 槽位资源基本同包）。
//!
//! 用法：cargo run -p rw4 --release --example shader_sweep_probe -- <package> [上限=200]

const RW4_IMAGE: u32 = 0x2F4E_681B;
const RASTER_IMAGE: u32 = 0x2F4E_681C;

struct MatTex {
    params: Option<(Vec<[f32; 4]>, usize)>,
    tint: Option<(Vec<u8>, usize, usize)>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, rest) = args.split_first().expect("usage: <package> [上限] [跨包...]");
    let cap = rest
        .first()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(200);
    let extras: Vec<_> = rest
        .iter()
        .skip_while(|s| s.parse::<usize>().is_ok())
        .map(|p| dbpf::Package::open(p).expect("open extra package"))
        .collect();
    let package = dbpf::Package::open(path).expect("open package");
    let find_entry = |inst: u32, types: [u32; 2]| -> Option<_> {
        for pkg in std::iter::once(&package).chain(extras.iter()) {
            if let Some(e) = pkg
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && (e.id.type_id == types[0] || e.id.type_id == types[1]))
                .cloned()
            {
                return Some((pkg, e));
            }
        }
        None
    };

    let mut models_seen = 0usize;
    let mut models_kind2 = 0usize;
    let mut models_unresolvable = 0usize;
    let mut kind_counts = [0usize; 3];
    let mut kind2_meshes = 0usize;
    let mut kind2_discarded = 0usize;
    let mut green_tint = 0usize; // 常数 tint 纹素 G 明显高于 R/B（发绿特征）
    let mut dark_tint = 0usize; // 亮度 b < 0.25（发黑特征）

    let entries: Vec<_> = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == RW4_IMAGE)
        .cloned()
        .collect();
    let stride = entries.len().div_ceil(cap).max(1);

    for entry in entries.into_iter().step_by(stride) {
        let Ok(data) = package.read(&entry) else { continue };
        let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
        let bindings = file.decode_mesh_material_bindings(&data);
        if bindings.is_empty() {
            continue;
        }
        // 材质资源缓存（主包内）
        let mut cache: std::collections::BTreeMap<u32, MatTex> = std::collections::BTreeMap::new();
        for b in &bindings {
            if cache.contains_key(&b.material_section) {
                continue;
            }
            let mut t = MatTex { params: None, tint: None };
            if let Ok(rw4::MaterialSection::Decoded(mat)) =
                file.decode_material(&data, b.material_section)
            {
                if let Some(inst) = mat.slot_texture(0) {
                    if let Some((pkg, e)) = find_entry(inst, [RW4_IMAGE, RW4_IMAGE]) {
                        if let Ok(bytes) = pkg.read(&e) {
                            if let Ok(tf) = rw4::Rw4File::parse(&bytes) {
                                if let Some(sec) =
                                    tf.sections_of_type(rw4::SectionType::TEXTURE).next()
                                {
                                    if let Ok(tex) = tf.decode_texture(&bytes, sec.number) {
                                        if let Ok(px) = tex.decode_palette_f32() {
                                            t.params =
                                                Some((px, usize::from(tex.width)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(inst) = mat.slot_texture(1) {
                    if let Some((pkg, e)) = find_entry(inst, [RASTER_IMAGE, RW4_IMAGE]) {
                        if let Ok(bytes) = pkg.read(&e) {
                            let rgba = if e.id.type_id == RASTER_IMAGE {
                                rw4::RasterImage::parse(&bytes)
                                    .and_then(|r| r.decode_top_mip_rgba())
                                    .map(|p| {
                                        (p, r_width(&bytes), r_height(&bytes))
                                    })
                            } else {
                                rw4::Rw4File::parse(&bytes).and_then(|tf| {
                                    let sec = tf
                                        .sections_of_type(rw4::SectionType::TEXTURE)
                                        .next()
                                        .ok_or(rw4::Error::SectionNumberOutOfRange {
                                            number: 0,
                                            count: 0,
                                        })?
                                        .number;
                                    let tex = tf.decode_texture(&bytes, sec)?;
                                    let rgba = tex.decode_top_mip_rgba()?;
                                    Ok((rgba, u32::from(tex.width), u32::from(tex.height)))
                                })
                            };
                            if let Ok((rgba, w, h)) = rgba {
                                t.tint = Some((
                                    rgba,
                                    usize::try_from(w).unwrap_or(0),
                                    usize::try_from(h).unwrap_or(0),
                                ));
                            }
                        }
                    }
                }
            }
            cache.insert(b.material_section, t);
        }

        models_seen += 1;
        let mut any_kind2 = false;
        let mut unresolvable = false;
        for section in file.sections_of_type(rw4::SectionType::MESH) {
            let Ok(mesh) = file.decode_mesh(&data, section.number) else { continue };
            if !mesh.is_exportable() {
                continue;
            }
            let mut kind = 0u8;
            let mut g0 = None;
            for v in &mesh.vertices {
                for (e, val) in &v.components {
                    if e.usage != rw4::DeclarationUsage::TexCoord {
                        continue;
                    }
                    match val {
                        rw4::ComponentValue::Float2(_) => kind = kind.max(1),
                        rw4::ComponentValue::Float4(f) => {
                            if f[0].abs() <= 8.0 && f[1].abs() <= 8.0 {
                                kind = kind.max(1);
                            } else {
                                kind = kind.max(2);
                            }
                        }
                        _ => {}
                    }
                }
                if g0.is_none() {
                    g0 = v.d3d_color_g();
                }
            }
            kind_counts[usize::from(kind)] += 1;
            if kind != 2 {
                continue;
            }
            any_kind2 = true;
            kind2_meshes += 1;
            let mat_sec = bindings
                .iter()
                .find(|b| b.mesh_section == section.number)
                .map(|b| b.material_section);
            let Some(mat_sec) = mat_sec else { continue };
            let Some(t) = cache.get(&mat_sec) else { continue };
            let (Some((params, cols)), Some((tint_rgba, tw, th))) =
                (t.params.as_ref(), t.tint.as_ref())
            else {
                unresolvable = true;
                continue;
            };
            let m = usize::from(g0.unwrap_or(0));
            let Some(xform) = params.get(cols + m).copied() else { continue };
            let tuv = [xform[2], xform[3]];
            let tx = ((tuv[0] - tuv[0].floor()).clamp(0.0, 0.999) * *tw as f32) as usize;
            let ty = ((tuv[1] - tuv[1].floor()).clamp(0.0, 0.999) * *th as f32) as usize;
            if let Some(px) = tint_rgba.get((ty * *tw + tx) * 4..) {
                if px[3] < 128 {
                    kind2_discarded += 1;
                } else {
                    if u32::from(px[1]) > u32::from(px[0]) + 60
                        && u32::from(px[1]) > u32::from(px[2]) + 60
                    {
                        green_tint += 1;
                    }
                    if u32::from(px[2]) < 64 {
                        dark_tint += 1;
                    }
                }
            }
        }
        if any_kind2 {
            models_kind2 += 1;
            if unresolvable {
                models_unresolvable += 1;
            }
        }
    }

    let total_mesh: usize = kind_counts.iter().sum();
    println!("sweep：抽样 {models_seen} 模型（步长 {stride}），mesh 总数 {total_mesh}");
    println!(
        "  kind 分布：[0 无UV={}  1 常规贴图={}  2 tint着色器={}]（kind2 占 {:.1}%）",
        kind_counts[0],
        kind_counts[1],
        kind_counts[2],
        if total_mesh > 0 { kind_counts[2] as f64 / total_mesh as f64 * 100.0 } else { 0.0 }
    );
    println!(
        "  含 kind2 模型 {models_kind2}；kind2 mesh {kind2_meshes}，其中前端预测整 mesh discard {kind2_discarded}（{:.1}%）",
        if kind2_meshes > 0 { kind2_discarded as f64 / kind2_meshes as f64 * 100.0 } else { 0.0 }
    );
    println!(
        "  非 discard 的 kind2 中：发绿特征（G>R+60 且 G>B+60）{green_tint}，发暗特征（b<0.25）{dark_tint}"
    );
    println!("  材质不可解（缺 slot0/tint）的含 kind2 模型：{models_unresolvable}");
}

fn r_width(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes.get(4).copied().unwrap_or(0), bytes.get(5).copied().unwrap_or(0), bytes.get(6).copied().unwrap_or(0), bytes.get(7).copied().unwrap_or(0)])
}

fn r_height(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes.get(8).copied().unwrap_or(0), bytes.get(9).copied().unwrap_or(0), bytes.get(10).copied().unwrap_or(0), bytes.get(11).copied().unwrap_or(0)])
}
