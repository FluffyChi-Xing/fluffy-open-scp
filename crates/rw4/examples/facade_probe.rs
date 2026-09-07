//! 冒烟探针：facade 世界投影 UV 数学取证（阶段 4）。
//!
//! 对指定模型：逐 mesh dump FLOAT4 TEXCOORD 各分量 min/max/mean；
//! 逐材质 dump 28B 头（u32/f32 双视图）、additional_data、尾 data 的
//! f32 视图——寻找材质裁剪窗（W,H,U,V ×2）。
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

    // VertexFormat 真实声明（用户逆向确认的 ground truth）
    for section in file.sections_of_type(rw4::SectionType::VERTEX_FORMAT) {
        if let Ok(format) = file.decode_vertex_format(data.as_slice(), section.number) {
            println!("VertexFormat #{}: elements:", section.number);
            for element in &format.elements {
                println!(
                    "    usage={:?} type={:?} index={} offset={}",
                    element.usage, element.decl_type, element.index, element.offset
                );
            }
        }
    }

    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let mesh = match file.decode_mesh(&data, section.number) {
            Ok(m) => m,
            Err(e) => {
                println!("mesh #{}: ERR {e}", section.number);
                continue;
            }
        };
        let mut min = [f32::MAX; 4];
        let mut max = [f32::MIN; 4];
        let mut sum = [0f64; 4];
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
                        sum[c] += f64::from(f[c]);
                    }
                }
            }
        }
        if count == 0 {
            println!("mesh #{}: {} verts, no FLOAT4 texcoord", section.number, mesh.vertices.len());
            continue;
        }
        println!(
            "mesh #{}: {} floats",
            section.number, count
        );
        // 前 24 个顶点：position(x,y,z) + FLOAT4，看行/列结构
        for (i, v) in mesh.vertices.iter().take(24).enumerate() {
            let pos = v.position().unwrap_or([0.0; 3]);
            let f4 = v.components.iter().find_map(|(_, value)| match value {
                rw4::ComponentValue::Float4(f) => Some(*f),
                _ => None,
            });
            if let Some(f) = f4 {
                println!(
                    "    v{i:03} pos=({:>8.2},{:>8.2},{:>8.2})  f4=({:>9.3},{:>9.3},{:>9.3},{:>9.3})",
                    pos[0], pos[1], pos[2], f[0], f[1], f[2], f[3]
                );
            }
        }
        let labels = ["X(w?)", "Y(h?)", "Z(x?)", "W(y?)"];
        for c in 0..4 {
            println!(
                "    {} min={:>12.4} max={:>12.4} mean={:>12.4}",
                labels[c],
                min[c],
                max[c],
                (sum[c] / count as f64) as f32
            );
        }
    }

    // ---- UV 公式求解器：遮罩红通道(=调色板列) vs 顶点 D3DCOLOR.G ----
    {
        // 取第一个带 FLOAT4 的 mesh + 第一个可解材质的 slot1 raster
        let mesh_section = file
            .sections_of_type(rw4::SectionType::MESH)
            .find_map(|s| {
                file.decode_mesh(&data, s.number)
                    .ok()
                    .filter(|m| m.vertices.iter().any(|v| v.d3d_color_g().is_some()))
                    .map(|m| m)
            });
        let mask_rgba = file
            .sections_of_type(rw4::SectionType::MATERIAL)
            .find_map(|s| {
                let mat = file.decode_material(&data, s.number).ok()?;
                let mat = match mat {
                    rw4::MaterialSection::Decoded(m) => m,
                    _ => return None,
                };
                let instance = mat.slot_texture(1)?;
                let _ = instance;
                // 取 slot5（DXT5 细节/facade 图）优先，回退 slot1
                let instance5 = mat.slot_texture(5).or(Some(instance))?;
                let tex_entry = package
                    .entries()
                    .iter()
                    .find(|e| e.id.instance == instance5 && (e.id.type_id == RASTER_IMAGE || e.id.type_id == RW4_IMAGE))
                    .cloned()?;
                let tex_data = package.read(&tex_entry).ok()?;
                let (rgba, width, height) = if tex_entry.id.type_id == RASTER_IMAGE {
                    let raster = rw4::RasterImage::parse(&tex_data).ok()?;
                    let width = u32::from(raster.width);
                    let height = u32::from(raster.height);
                    (raster.decode_top_mip_rgba().ok()?, width, height)
                } else {
                    let tex_file = rw4::Rw4File::parse(&tex_data).ok()?;
                    let sec = tex_file
                        .sections_of_type(rw4::SectionType::TEXTURE)
                        .next()?
                        .number;
                    let tex = tex_file.decode_texture(&tex_data, sec).ok()?;
                    let width = u32::from(tex.width);
                    let height = u32::from(tex.height);
                    (tex.decode_top_mip_rgba().ok()?, width, height)
                };
                Some((rgba, width, height))
            });
        if let (Some(mesh), Some((mask, mask_w, mask_h))) = (mesh_section, mask_rgba) {
            let sample = |u: f32, v: f32, flip_v: bool| -> Option<[u8; 3]> {
                if !u.is_finite() || !v.is_finite() {
                    return None;
                }
                // true modulo（Rust fract 保号，负 UV 会 clamp 到 0 列污染结果）
                let fu = u - u.floor();
                let fv = v - v.floor();
                let fv = if flip_v { 1.0 - fv } else { fv };
                let x = ((fu * mask_w as f32) as usize).min(mask_w as usize - 1);
                let y = ((fv * mask_h as f32) as usize).min(mask_h as usize - 1);
                let px = mask.get((y * mask_w as usize + x) * 4..)?;
                Some([px[0], px[1], px[2]])
            };
            println!("--- UV hypothesis solver (mask_red == D3DCOLOR.G match rate) ---");
            // 固定样本集（顶点 f4 + 期望列）
            let samples: Vec<([f32; 4], u8)> = mesh
                .vertices
                .iter()
                .take(1200)
                .filter_map(|v| {
                    let g = v.d3d_color_g()?;
                    let (_, value) = v
                        .components
                        .iter()
                        .find(|(e, _)| e.usage == rw4::DeclarationUsage::TexCoord)?;
                    let f: [f32; 4] = match value {
                        rw4::ComponentValue::Float4(f) => *f,
                        rw4::ComponentValue::Float2(uv) => [uv[0], uv[1], 0.0, 0.0],
                        _ => return None,
                    };
                    Some((f, g))
                })
                .collect();
            let rate = |sx: f32, sy: f32, flip_v: bool| -> f64 {
                let mut tested = 0usize;
                let mut matched = 0usize;
                for (f, g) in &samples {
                    let (u, v_coord) = (f[0] / sx, f[1] / sy);
                    if sample(u, v_coord, flip_v).is_some() {
                        tested += 1;
                    }
                }
                if tested > 0 { matched as f64 / tested as f64 } else { 0.0 }
            };
            // 聚类验证：同 G 顶点的 (f0 mod 512, f1 mod 512) 是否聚集，且聚类处 mask_red 是否 == G
            let mut by_g: std::collections::BTreeMap<u8, Vec<[f32; 2]>> =
                std::collections::BTreeMap::new();
            for (f, g) in &samples {
                by_g.entry(*g).or_default().push([f[0], f[1]]);
            }
            println!("    --- per-G cluster (mod 512, n>=4): uv mean/R + mask_red(normal|flip) vs G ---");
            let circ = |vals: &[f32]| -> (f32, f64) {
                let (mut sx, mut sy) = (0f64, 0f64);
                for v in vals {
                    let a = f64::from(*v) * std::f64::consts::TAU;
                    sx += a.cos();
                    sy += a.sin();
                }
                let r = (sx * sx + sy * sy).sqrt() / vals.len() as f64;
                let m = sy.atan2(sx) / std::f64::consts::TAU / std::f64::consts::TAU;
                ((m + 1.0).fract() as f32, r)
            };
            for (g, uvs) in by_g.iter().filter(|(_, uvs)| uvs.len() >= 4).take(14) {
                let us: Vec<f32> = uvs.iter().map(|uv| uv[0]).collect();
                let vs: Vec<f32> = uvs.iter().map(|uv| uv[1]).collect();
                let (mu_c, ru) = circ(&us);
                let (mv_c, rv) = circ(&vs);
                let (mu, mv) = (mu_c * 512.0, mv_c * 512.0);
                let c0 = sample(mu / 512.0, mv / 512.0, false)
                    .map(|c| format!("({},{},{})", c[0], c[1], c[2]))
                    .unwrap_or("?".into());
                let c1 = sample(mu / 512.0, mv / 512.0, true)
                    .map(|c| format!("({},{},{})", c[0], c[1], c[2]))
                    .unwrap_or("?".into());
                println!(
                    "    G={g:<3} n={n:<5} u={mu:>6.1}px (R={ru:.2}) v={mv:>6.1}px (R={rv:.2})  mask(normal)={c0} mask(flip)={c1}",
                    n = uvs.len()
                );
            }
            // 粗扫：方格尺度 1..1200
            let mut best = (0f64, 0f32);
            for s in 1..=1200 {
                let r = rate(s as f32, s as f32, false);
                if r > best.0 {
                    best = (r, s as f32);
                }
            }
            println!("    square-scale best: s={} rate={:.1}%", best.1, best.0 * 100.0);
            for flip in [false, true] {
                println!(
                    "    s=1 flip_v={flip}: match rate={:.1}%",
                    rate(1.0, 1.0, flip) * 100.0
                );
            }
            // 独立细扫 sx/sy（在最优附近 ±40）
            let mut best2 = (0f64, 0f32, 0f32);
            for sx in (best.1 as i32 - 40).max(1)..=(best.1 as i32 + 40) {
                for sy in (best.1 as i32 - 40).max(1)..=(best.1 as i32 + 40) {
                    let r = rate(sx as f32, sy as f32, false);
                    if r > best2.0 {
                        best2 = (r, sx as f32, sy as f32);
                    }
                }
            }
            println!(
                "    refined best: sx={} sy={} rate={:.1}%",
                best2.1, best2.2, best2.0 * 100.0
            );
        }
    }

    for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
        let payload = match file.payload(data.as_slice(), section.number) {
            Ok(p) => p,
            Err(e) => {
                println!("material #{}: payload ERR {e}", section.number);
                continue;
            }
        };
        println!("material #{}: size={} payload={}", section.number, section.size, payload.len());
        let header = &payload[..payload.len().min(28)];
        print!("    header u32:");
        for chunk in header.chunks_exact(4) {
            print!(" {}", u32::from_le_bytes(chunk.try_into().unwrap()));
        }
        println!();
        print!("    header f32:");
        for chunk in header.chunks_exact(4) {
            print!(" {:.3}", f32::from_le_bytes(chunk.try_into().unwrap()));
        }
        println!();
        // additional_data 与尾 data 的 f32 视图（跳过 0x2D 扫描区，直接全打）
        let decoded = file.decode_material(&data, section.number).ok();
        if let Some(rw4::MaterialSection::Decoded(mat)) = decoded {
            let dump_f32 = |label: &str, bytes: &[u8]| {
                print!("    {label} ({}B) f32:", bytes.len());
                for chunk in bytes.chunks_exact(4) {
                    print!(" {:.3}", f32::from_le_bytes(chunk.try_into().unwrap()));
                }
                println!();
            };
            dump_f32("additional", &mat.additional_data);
            dump_f32("tail-data", &mat.data);
        }
    }
}
