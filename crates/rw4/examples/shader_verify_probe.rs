//! 验证探针（2026-09-08 四症状分析取证，只读不改）：
//! A. uvKind=2 mesh 的 GLB TEXCOORD_0 断链 + 前端常数 tint 采样 → 整 mesh discard 预测
//! B. D3DCOLOR R vs G 通道（源码 materialIndex=In.color.r，当前实现用 G）
//! C. slot2 法线图平坦区主色（源码 ApplyNormalMap: B=沿法线轴 → 平坦应≈(128,128,255)）
//! D. slot0 参数表行语义（源码 row0=(palU,palU2,interiorScale,interiorOffset)、V=variation 行）
//!
//! 用法：cargo run -p rw4 --release --example shader_verify_probe -- <package> 0xMODEL [跨包...]

const RW4_IMAGE: u32 = 0x2F4E_681B;
const RASTER_IMAGE: u32 = 0x2F4E_681C;

struct MatTex {
    params: Option<(Vec<[f32; 4]>, usize)>,
    tint: Option<(Vec<u8>, usize, usize)>,
    tint_fmt: Option<String>,
    palette: Option<(Vec<u8>, usize, usize)>,
    palette_fmt: Option<String>,
    normal: Option<(Vec<u8>, usize, usize, String)>,
    slot3: Option<(Vec<u8>, usize, usize, String)>,
    slot5: Option<(Vec<u8>, usize, usize, String)>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, rest) = args.split_first().expect("usage: <package> 0xMODEL [extra...]");
    let (model, extra) = rest.split_first().expect("missing 0xMODEL");
    let instance = u32::from_str_radix(model.trim_start_matches("0x"), 16).expect("instance");

    let package = dbpf::Package::open(path).expect("open package");
    let extras: Vec<_> = extra
        .iter()
        .map(|p| dbpf::Package::open(p).expect("open extra package"))
        .collect();
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == RW4_IMAGE && e.id.instance == instance)
        .cloned()
        .expect("model not found in main package");
    let data = package.read(&entry).expect("read model");
    let file = rw4::Rw4File::parse(&data).expect("parse rw4");
    let bindings = file.decode_mesh_material_bindings(&data);

    // ---- 跨包资源解析（主包优先）----
    let find_rgba = |inst: u32| -> Option<(Vec<u8>, u32, u32, String)> {
        for pkg in std::iter::once(&package).chain(extras.iter()) {
            let Some(e) = pkg
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && (e.id.type_id == RASTER_IMAGE || e.id.type_id == RW4_IMAGE))
                .cloned()
            else {
                continue;
            };
            let bytes = pkg.read(&e).ok()?;
            if e.id.type_id == RASTER_IMAGE {
                let raster = rw4::RasterImage::parse(&bytes).ok()?;
                let rgba = raster.decode_top_mip_rgba().ok()?;
                return Some((
                    rgba,
                    raster.width,
                    raster.height,
                    format!("raster:pixFmt{}", raster.pixel_format),
                ));
            }
            let tex_file = rw4::Rw4File::parse(&bytes).ok()?;
            let sec = tex_file.sections_of_type(rw4::SectionType::TEXTURE).next()?.number;
            let tex = tex_file.decode_texture(&bytes, sec).ok()?;
            let fmt = match tex.texture_type {
                rw4::TEXTURE_TYPE_DXT1 => "DXT1",
                rw4::TEXTURE_TYPE_DXT5 => "DXT5",
                21 => "rawBGRA",
                other => return Some((Vec::new(), 0, 0, format!("unk{other}"))),
            };
            let rgba = tex.decode_top_mip_rgba().ok()?;
            return Some((rgba, u32::from(tex.width), u32::from(tex.height), fmt.into()));
        }
        None
    };
    let find_params = |inst: u32| -> Option<(Vec<[f32; 4]>, usize)> {
        for pkg in std::iter::once(&package).chain(extras.iter()) {
            let Some(e) = pkg
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && e.id.type_id == RW4_IMAGE)
                .cloned()
            else {
                continue;
            };
            let bytes = pkg.read(&e).ok()?;
            let tex_file = rw4::Rw4File::parse(&bytes).ok()?;
            let Some(sec) = tex_file.sections_of_type(rw4::SectionType::TEXTURE).next() else {
                continue;
            };
            let tex = tex_file.decode_texture(&bytes, sec.number).ok()?;
            if let Ok(pixels) = tex.decode_palette_f32() {
                return Some((pixels, usize::from(tex.width)));
            }
        }
        None
    };

    // ---- 逐材质资源缓存 ----
    let mut mat_cache: std::collections::BTreeMap<u32, MatTex> = std::collections::BTreeMap::new();
    for b in &bindings {
        if mat_cache.contains_key(&b.material_section) {
            continue;
        }
        let mut t = MatTex {
            params: None,
            tint: None,
            tint_fmt: None,
            palette: None,
            palette_fmt: None,
            normal: None,
            slot3: None,
            slot5: None,
        };
        if let Ok(rw4::MaterialSection::Decoded(mat)) = file.decode_material(&data, b.material_section) {
            if let Some(inst) = mat.slot_texture(0) {
                t.params = find_params(inst);
            }
            if let Some(inst) = mat.slot_texture(1) {
                if let Some((rgba, w, h, fmt)) = find_rgba(inst) {
                    t.tint = Some((rgba, usize::try_from(w).unwrap_or(0), usize::try_from(h).unwrap_or(0)));
                    t.tint_fmt = Some(fmt);
                }
            }
            if let Some(inst) = mat.slot_texture(4) {
                if let Some((rgba, w, h, fmt)) = find_rgba(inst) {
                    t.palette = Some((rgba, usize::try_from(w).unwrap_or(0), usize::try_from(h).unwrap_or(0)));
                    t.palette_fmt = Some(fmt);
                }
            }
            if let Some(inst) = mat.slot_texture(2) {
                if let Some((rgba, w, h, fmt)) = find_rgba(inst) {
                    t.normal = Some((rgba, usize::try_from(w).unwrap_or(0), usize::try_from(h).unwrap_or(0), fmt));
                }
            }
            if let Some(inst) = mat.slot_texture(3) {
                if let Some((rgba, w, h, fmt)) = find_rgba(inst) {
                    t.slot3 = Some((rgba, usize::try_from(w).unwrap_or(0), usize::try_from(h).unwrap_or(0), fmt));
                }
            }
            if let Some(inst) = mat.slot_texture(5) {
                if let Some((rgba, w, h, fmt)) = find_rgba(inst) {
                    t.slot5 = Some((rgba, usize::try_from(w).unwrap_or(0), usize::try_from(h).unwrap_or(0), fmt));
                }
            }
        }
        mat_cache.insert(b.material_section, t);
    }

    // ============ Part A：uvKind=2 断链预测 ============
    println!("=== A. uvKind=2 GLB TEXCOORD_0 断链 + 常数采样 discard 预测 ===");
    let mut kind_counts = [0usize; 3];
    let mut kind2_total = 0usize;
    let mut kind2_discarded = 0usize;
    let mut printed = 0usize;
    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(&data, section.number) else { continue };
        if !mesh.is_exportable() {
            continue;
        }
        // 与 package_service::mesh_uv_kind 相同的判定
        let mut kind = 0u8;
        let mut has_float2 = false;
        let mut f4_max = 0f32;
        let mut xy_min = [f32::MAX; 2];
        let mut xy_max = [f32::MIN; 2];
        let mut zw_min = [f32::MAX; 2];
        let mut zw_max = [f32::MIN; 2];
        let mut f4_count = 0usize;
        for v in &mesh.vertices {
            for (e, val) in &v.components {
                if e.usage != rw4::DeclarationUsage::TexCoord {
                    continue;
                }
                match val {
                    rw4::ComponentValue::Float2(_) => {
                        has_float2 = true;
                        kind = kind.max(1);
                    }
                    rw4::ComponentValue::Float4(f) => {
                        f4_count += 1;
                        f4_max = f4_max.max(f[0].abs()).max(f[1].abs());
                        for c in 0..2 {
                            xy_min[c] = xy_min[c].min(f[c]);
                            xy_max[c] = xy_max[c].max(f[c]);
                            zw_min[c] = zw_min[c].min(f[2 + c]);
                            zw_max[c] = zw_max[c].max(f[2 + c]);
                        }
                        if f[0].abs() <= 8.0 && f[1].abs() <= 8.0 {
                            kind = kind.max(1);
                        } else {
                            kind = kind.max(2);
                        }
                    }
                    _ => {}
                }
            }
        }
        kind_counts[usize::from(kind)] += 1;
        if kind != 2 || f4_count == 0 {
            continue;
        }
        kind2_total += 1;
        // GLB 导出规则（gltf.rs）：无 Float2 且 f4_max>8 → has_uv=false → TEXCOORD_0=(0,0)
        let glb_uv_zeroed = !has_float2 && f4_max > 8.0;
        // 前端预测：vTintUv=(0,0) → tUv = xform.zw（该 mesh 材质 row1）
        let mat_sec = bindings
            .iter()
            .find(|b| b.mesh_section == section.number)
            .map(|b| b.material_section);
        let (mut fe_discard, mut fe_tint, mut real_pass, mut real_n) = (false, [0u8; 4], 0usize, 0usize);
        if let Some(ms) = mat_sec {
            if let Some(t) = mat_cache.get(&ms) {
                if let (Some((params, cols)), Some((tint_rgba, tw, th))) =
                    (t.params.as_ref(), t.tint.as_ref())
                {
                    let cols = *cols;
                    // 当前实现用 G 作 materialIndex
                    let m = mesh
                        .vertices
                        .iter()
                        .find_map(|v| v.d3d_color_g())
                        .unwrap_or(0) as usize;
                    if let Some(xform) = params.get(cols + m).copied() {
                        let tuv = [xform[2], xform[3]];
                        let tx = ((tuv[0] - tuv[0].floor()).clamp(0.0, 0.999) * *tw as f32) as usize;
                        let ty = ((tuv[1] - tuv[1].floor()).clamp(0.0, 0.999) * *th as f32) as usize;
                        if let Some(px) = tint_rgba.get((ty * *tw + tx) * 4..) {
                            fe_tint.copy_from_slice(&px[..4]);
                            fe_discard = px[3] < 128;
                        }
                        // 对照：真实 UV 的逐顶点 alpha 通过率
                        for v in &mesh.vertices {
                            let Some(f) = v.components.iter().find_map(|(e, val)| {
                                (e.usage == rw4::DeclarationUsage::TexCoord)
                                    .then(|| match val {
                                        rw4::ComponentValue::Float4(f) => Some(*f),
                                        _ => None,
                                    })
                                    .flatten()
                            }) else {
                                continue;
                            };
                            let bu = (f[0] - f[0].floor()) * xform[0] + xform[2];
                            let bv = (f[1] - f[1].floor()) * xform[1] + xform[3];
                            let tx = ((bu - bu.floor()).clamp(0.0, 0.999) * *tw as f32) as usize;
                            let ty = ((bv - bv.floor()).clamp(0.0, 0.999) * *th as f32) as usize;
                            if let Some(px) = tint_rgba.get((ty * *tw + tx) * 4..) {
                                real_n += 1;
                                if px[3] >= 128 {
                                    real_pass += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        if fe_discard {
            kind2_discarded += 1;
        }
        if printed < 12 {
            printed += 1;
            println!(
                "  mesh #{} mat={:?} verts={} uvKind=2 glb_uv_zeroed={} xy[{:.2},{:.2}][{:.2},{:.2}] zw[{:.2},{:.2}][{:.2},{:.2}]\n    前端常数tintRGBA={:?} → 整mesh discard={}  |  真实UV alpha≥0.5 率={:.1}%",
                section.number, mat_sec, mesh.vertices.len(), glb_uv_zeroed,
                xy_min[0], xy_max[0], xy_min[1], xy_max[1],
                zw_min[0], zw_max[0], zw_min[1], zw_max[1],
                fe_tint, fe_discard,
                if real_n > 0 { real_pass as f64 / real_n as f64 * 100.0 } else { f64::NAN },
            );
        }
    }
    println!(
        "  汇总：mesh 按 kind [0={} 1={} 2={}]；kind2={kind2_total}，其中前端预测整mesh discard={}（{:.1}%）",
        kind_counts[0], kind_counts[1], kind_counts[2],
        kind2_total,
        if kind2_total > 0 { kind2_discarded as f64 / kind2_total as f64 * 100.0 } else { 0.0 },
    );

    // ============ Part B：R vs G 通道 ============
    println!("=== B. D3DCOLOR R vs G（源码 materialIndex=In.color.r；当前实现用 G）===");
    let (mut total, mut r_eq_g) = (0usize, 0usize);
    let mut rg_hist: std::collections::BTreeMap<(u8, u8), usize> = std::collections::BTreeMap::new();
    let mut samples: Vec<([f32; 4], u8, u8)> = Vec::new(); // (f4, r, g)
    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(&data, section.number) else { continue };
        for v in &mesh.vertices {
            let Some(val) = v.components.iter().find_map(|(e, val)| {
                (e.usage == rw4::DeclarationUsage::Color).then_some(val)
            }) else {
                continue;
            };
            let rw4::ComponentValue::D3DColor { r, g, .. } = val else { continue };
            total += 1;
            if r == g {
                r_eq_g += 1;
            }
            *rg_hist.entry((*r, *g)).or_default() += 1;
            if let Some((_, f)) = v.components.iter().find_map(|(e, val)| {
                (e.usage == rw4::DeclarationUsage::TexCoord).then_some(()).and_then(|_| match val {
                    rw4::ComponentValue::Float4(f) => Some(((), *f)),
                    _ => None,
                })
            }) {
                if samples.len() < 20000 {
                    samples.push((f, *r, *g));
                }
            }
        }
    }
    println!("  顶点带 COLOR：{total}，r==g 占 {:.1}%，不同(r,g) 组合 {} 种",
        if total > 0 { r_eq_g as f64 / total as f64 * 100.0 } else { 0.0 },
        rg_hist.len());
    // 用 row1 xform + tint.a>0.5 率与 per-通道 tint.rg spread 对比（哪个通道是 materialIndex）
    // u_byte：tint 图哪个字节当 U 位置（byte0=R直读 / byte2=BGRA 假设）
    let channel_stats = |key_r: bool, u_byte: usize| -> (f64, f64) {
        let mut pass = 0usize;
        let mut tested = 0usize;
        let mut rg_by: std::collections::BTreeMap<u8, Vec<[f32; 2]>> = std::collections::BTreeMap::new();
        for &(f, r, g) in &samples {
            let m = (if key_r { r } else { g }) as usize;
            let Some(t) = mat_cache.values().find_map(|t| t.params.as_ref()) else { return (f64::NAN, f64::NAN) };
            let Some(xform) = t.0.get(t.1 + m).copied() else { continue };
            let Some((tint_rgba, tw, th)) = mat_cache.values().find_map(|t| t.tint.as_ref()) else { return (f64::NAN, f64::NAN) };
            let bu = (f[0] - f[0].floor()) * xform[0] + xform[2];
            let bv = (f[1] - f[1].floor()) * xform[1] + xform[3];
            let tx = ((bu - bu.floor()).clamp(0.0, 0.999) * *tw as f32) as usize;
            let ty = ((bv - bv.floor()).clamp(0.0, 0.999) * *th as f32) as usize;
            if let Some(px) = tint_rgba.get((ty * *tw + tx) * 4..) {
                tested += 1;
                if px[3] >= 128 {
                    pass += 1;
                }
                rg_by.entry(if key_r { r } else { g }).or_default().push([f32::from(px[u_byte]), f32::from(px[1])]);
            }
        }
        let mut spread_sum = 0f64;
        let mut spread_n = 0usize;
        for uvs in rg_by.values() {
            if uvs.len() < 2 { continue }
            let mean = [
                uvs.iter().map(|c| c[0]).sum::<f32>() / uvs.len() as f32,
                uvs.iter().map(|c| c[1]).sum::<f32>() / uvs.len() as f32,
            ];
            for c in uvs {
                spread_sum += f64::from((c[0] - mean[0]).abs() + (c[1] - mean[1]).abs());
                spread_n += 1;
            }
        }
        (
            if tested > 0 { pass as f64 / tested as f64 * 100.0 } else { f64::NAN },
            if spread_n > 0 { spread_sum / spread_n as f64 } else { f64::MAX },
        )
    };
    for (label, key_r, u_byte) in [
        ("按G(byte0=R直读)", false, 0),
        ("按G(byte2=BGRA)  ", false, 2),
        ("按R(byte0=R直读)", true, 0),
        ("按R(byte2=BGRA)  ", true, 2),
    ] {
        let (rate, spread) = channel_stats(key_r, u_byte);
        println!("  {label}：alpha率={rate:.1}% per-通道 spread={spread:.2}");
    }
    let top: Vec<_> = rg_hist.iter().rev().take(8).collect();
    println!("  最高频 (r,g) 组合：{top:?}");

    // ============ Part C：slot2 法线图主色 + 各槽位贴图家族 ============
    println!("=== C. 贴图家族与法线主色（源码语义：平坦区应≈(128,128,255)，B=沿法线轴）===");
    for (sec, t) in &mat_cache {
        if let (Some((_, tw, th)), Some(fmt)) = (t.tint.as_ref(), t.tint_fmt.as_ref()) {
            println!("  mat#{sec}: slot1 tint {tw}x{th} [{fmt}]");
        }
        if let (Some((_, pw, ph)), Some(fmt)) = (t.palette.as_ref(), t.palette_fmt.as_ref()) {
            println!("  mat#{sec}: slot4 palette {pw}x{ph} [{fmt}]");
        }
        let Some((rgba, w, h, fmt)) = t.normal.as_ref() else { continue };
        if rgba.is_empty() {
            println!("  mat#{sec}: slot2 {fmt}（不可解码）");
            continue;
        }
        let mut freq: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
        let mut sum = [0f64; 4];
        let n = rgba.len() / 4;
        for px in rgba.as_chunks::<4>().0 {
            *freq.entry([px[0], px[1], px[2]]).or_default() += 1;
            for c in 0..4 {
                sum[c] += f64::from(px[c]);
            }
        }
        let mode = freq.iter().max_by_key(|(_, c)| **c).unwrap();
        let (mode, mode_n) = (*mode.0, *mode.1);
        let mean: [f64; 4] = [sum[0] / n as f64, sum[1] / n as f64, sum[2] / n as f64, sum[3] / n as f64];
        println!(
            "  mat#{sec}: slot2 {w}x{h} [{fmt}] 主色={mode:?}（占{:.1}%）均值=({:.0},{:.0},{:.0}) alpha均值={:.0}（spec）",
            mode_n as f64 / n as f64 * 100.0, mean[0], mean[1], mean[2], mean[3],
        );
        // slot3 / slot5 一并 dump（校验槽位映射假设）
        for (label, slot) in [("slot3", t.slot3.as_ref()), ("slot5", t.slot5.as_ref())] {
            let Some((rgba, w, h, fmt)) = slot else { continue };
            if rgba.is_empty() {
                println!("  mat#{sec}: {label} {w}x{h} [{fmt}]（不可解码）");
                continue;
            }
            let mut f3: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
            let mut s = [0f64; 4];
            let n3 = rgba.len() / 4;
            for px in rgba.as_chunks::<4>().0 {
                *f3.entry([px[0], px[1], px[2]]).or_default() += 1;
                for c in 0..4 {
                    s[c] += f64::from(px[c]);
                }
            }
            let m3 = f3.iter().max_by_key(|(_, c)| **c).unwrap();
            let (m3c, m3n) = (*m3.0, *m3.1);
            println!(
                "  mat#{sec}: {label} {w}x{h} [{fmt}] 主色={m3c:?}（占{:.1}%）均值=({:.0},{:.0},{:.0}) alpha均值={:.0}",
                m3n as f64 / n3 as f64 * 100.0, s[0] / n3 as f64, s[1] / n3 as f64, s[2] / n3 as f64, s[3] / n3 as f64,
            );
        }
    }

    // ============ Part D：slot0 参数表行语义 ============
    println!("=== D. slot0 参数表（源码：row0=(palU,palU2,interiorScale,interiorOffset) V=variation 行不在表内）===");
    let Some((params, cols)) = mat_cache.values().find_map(|t| t.params.as_ref()) else {
        println!("  无 slot0 参数表");
        return;
    };
    let cols = *cols;
    let rows = params.len() / cols;
    let is_multiple_of = |v: f32, q: f64| -> bool {
        let k = f64::from(v) / q;
        (k - k.round()).abs() < 0.01
    };
    for row in 0..rows.min(4) {
        let vals: Vec<f32> = (0..cols).map(|m| params[row * cols + m][0]).collect();
        let uniq: std::collections::BTreeSet<_> = vals.iter().map(|v| format!("{v:.3}")).collect();
        let e8 = vals.iter().filter(|v| is_multiple_of(**v, 0.125)).count();
        let e16 = vals.iter().filter(|v| is_multiple_of(**v, 1.0 / 16.0)).count();
        println!(
            "  row{row} x 分量：{}列 uniq={}，1/8 整数倍 {}，1/16 整数倍 {}，范围 [{:.3},{:.3}]",
            cols, uniq.len(), e8, e16,
            vals.iter().cloned().fold(f32::MAX, f32::min),
            vals.iter().cloned().fold(f32::MIN, f32::max),
        );
    }
    // row0.y（当前被当 V 用）量化检验：V 应为 1/8 整数倍（variation 0..7）
    let row0y: Vec<f32> = (0..cols).map(|m| params[m][1]).collect();
    let e8y = row0y.iter().filter(|v| is_multiple_of(**v, 0.125)).count();
    println!(
        "  row0.y（当前实现当 palV）：1/8 整数倍 {e8y}/{}，范围 [{:.3},{:.3}]，样例 {:?}",
        cols,
        row0y.iter().cloned().fold(f32::MAX, f32::min),
        row0y.iter().cloned().fold(f32::MIN, f32::max),
        &row0y[..row0y.len().min(8)],
    );
    // 前端 vs 后端 palOrigin 行分歧：前端采 v=0.625=row2，后端 bake 用 row0
    println!("  前 6 列对照（前端 palOrigin=row2 vs 后端 row0）：");
    for m in 0..6.min(cols) {
        println!(
            "    col{m:<3} row0=({:.3},{:.3},{:.3},{:.3})  row2=({:.3},{:.3},{:.3},{:.3})",
            params[m][0], params[m][1], params[m][2], params[m][3],
            params[2 * cols + m][0], params[2 * cols + m][1], params[2 * cols + m][2], params[2 * cols + m][3],
        );
    }

    // ============ Part E：tint 字节序 A/B（RGBA直读 vs BGRA） ============
    // pixFmt21=D3DFMT_A8R8G8B8 → D3D 内存布局 B,G,R,A。若 BGRA 成立：
    // 游戏的 U 位置=tint.r=byte2、亮度=tint.b=byte0（与当前实现互换）。
    println!("=== E. tint 亮度通道 A/B（当前=byte2，BGRA 假设=byte0）===");
    let Some((tint_rgba, tint_w, tint_h)) = mat_cache.values().find_map(|t| t.tint.as_ref()) else {
        return;
    };
    for byte in [0usize, 2] {
        let mut dark = 0usize;
        let mut total = 0usize;
        let mut dark_valid = 0usize;
        let mut total_valid = 0usize;
        for px in tint_rgba.as_chunks::<4>().0 {
            total += 1;
            if u32::from(px[byte]) < 64 {
                dark += 1;
            }
            if px[3] >= 128 {
                total_valid += 1;
                if u32::from(px[byte]) < 64 {
                    dark_valid += 1;
                }
            }
        }
        println!(
            "  byte{byte} < 64：全图 {:.1}%；仅 alpha≥0.5 有效区 {:.1}%（{} px）",
            dark as f64 / total as f64 * 100.0,
            if total_valid > 0 { dark_valid as f64 / total_valid as f64 * 100.0 } else { f64::NAN },
            total_valid,
        );
    }
    // slot4 调色板是否灰阶（灰阶 → tint 链不可能产生饱和绿）
    if let Some((pal, _, _)) = mat_cache.values().find_map(|t| t.palette.as_ref()) {
        let mut max_ab = 0i32;
        for px in pal.as_chunks::<4>().0 {
            max_ab = max_ab.max((i32::from(px[0]) - i32::from(px[2])).abs())
                .max((i32::from(px[0]) - i32::from(px[1])).abs())
                .max((i32::from(px[1]) - i32::from(px[2])).abs());
        }
        println!("  slot4 调色板 RGB 通道最大差值={max_ab}（≈0 = 灰阶）");
    }
    // resolve_palette 垃圾色验证：slot0 f32 当 RGB → B 恒 ~0.125
    println!("  resolve_palette（slot0 f32 当颜色）样例（前 6 列 row0）：");
    for m in 0..6.min(cols) {
        let c = params[m];
        println!(
            "    col{m}: RGB=({:.0},{:.0},{:.0})（真实语义: palU={:.3} palU2={:.3} interiorScale={:.3}）",
            c[0] * 255.0, c[1] * 255.0, c[2] * 255.0, c[0], c[1], c[2],
        );
    }
    // tint 图四通道 ASCII 目检（24×24 均值降采样，' .:*#@' 六级）
    println!("  tint 图通道目检（左→右：byte0 byte1 byte2 alpha）：");
    let (tw, th) = (*tint_w, *tint_h);
    let cell_w = tw / 24;
    let cell_h = th / 24;
    let ramp = [' ', '.', ':', '*', '#', '@'];
    for chan in 0..4 {
        print!("  ch{chan} |");
        for cy in 0..24 {
            let mut sum = 0f64;
            let mut n = 0usize;
            for y in (cy * cell_h)..((cy + 1) * cell_h) {
                for x in (0..24).map(|cx| cx * cell_w) {
                    if let Some(px) = tint_rgba.get((y * tw + x) * 4..) {
                        sum += f64::from(px[chan]);
                        n += 1;
                    }
                }
            }
            let v = if n > 0 { sum / n as f64 / 255.0 } else { 0.0 };
            print!("{}", ramp[(v * 5.99).clamp(0.0, 5.0) as usize]);
        }
        println!("|");
    }
    // 两条链对 mesh 顶点的烘焙色对照（前 6 个 G 组）
    println!("  烘焙色对照（U=byte0/亮度=byte2 → U=byte2/亮度=byte0）：");
    let Some((pal_rgba, pal_w, pal_h)) = mat_cache.values().find_map(|t| t.palette.as_ref()) else {
        return;
    };
    let mesh_samples: Vec<([f32; 4], u8)> =
        samples.iter().map(|(f, _, g)| (*f, *g)).collect();
    let bake = |u_byte: usize, b_byte: usize| -> std::collections::BTreeMap<u8, [f64; 3]> {
        let mut acc: std::collections::BTreeMap<u8, (f64, [f64; 3])> = Default::default();
        for (f, g) in &mesh_samples {
            let Some(t) = mat_cache.values().find_map(|t| t.params.as_ref()) else { return Default::default() };
            let Some(xform) = t.0.get(t.1 + *g as usize).copied() else { continue };
            let Some(pal_origin) = t.0.get(*g as usize).copied() else { continue };
            let bu = (f[0] - f[0].floor()) * xform[0] + xform[2];
            let bv = (f[1] - f[1].floor()) * xform[1] + xform[3];
            let tx = ((bu - bu.floor()).clamp(0.0, 0.999) * *tint_w as f32) as usize;
            let ty = ((bv - bv.floor()).clamp(0.0, 0.999) * *tint_h as f32) as usize;
            let Some(t4) = tint_rgba.get((ty * tint_w + tx) * 4..) else { continue };
            let pu = pal_origin[0] + f32::from(t4[u_byte]) / 255.0 * (0.5 / *pal_w as f32)
                + 0.25 / *pal_w as f32;
            let pv = 0.0 + f32::from(t4[1]) / 255.0 * (0.5 / *pal_h as f32) + 0.25 / *pal_h as f32;
            let px = ((pu - pu.floor()).clamp(0.0, 0.999) * *pal_w as f32) as usize;
            let py = ((pv - pv.floor()).clamp(0.0, 0.999) * *pal_h as f32) as usize;
            let Some(p) = pal_rgba.get((py * pal_w + px) * 4..) else { continue };
            let bright = f32::from(t4[b_byte]) as f32 / 255.0 * 2.0;
            let e = acc.entry(*g).or_default();
            e.0 += 1.0;
            for c in 0..3 {
                e.1[c] += f64::from(p[c]) * bright as f64;
            }
        }
        acc.into_iter()
            .map(|(g, (n, sum))| (g, [sum[0] / n, sum[1] / n, sum[2] / n]))
            .collect()
    };
    let cur = bake(0, 2);
    let fix = bake(2, 0);
    for ((g, c0), (_, c1)) in cur.iter().zip(fix.iter()).take(8) {
        println!(
            "    G={g:<3} 当前=({:.0},{:.0},{:.0})  修正=({:.0},{:.0},{:.0})",
            c0[0], c0[1], c0[2], c1[0], c1[1], c1[2],
        );
    }
}
