//! 只读探针：钉死 decal 投影盒的语义（`modelToTexture` 的盒中心与 Z 厚度）。
//!
//! 对每个 decal 输出：
//!   · 12 float 原始矩阵与解出的原点/基向量（确认 WPF 行主序解读）
//!   · **原点到最近三角形**的距离（顶点太稀疏，必须用面）
//!   · XY 窗口（|x| ≤ scale、|y| ≤ scale/aspect）内的 z 分布与 min|z|
//!
//! 判读：若 origin→最近面 ≈ 0 且窗口内 z 聚在 0 → 墙面过原点（depth 非平面距离）；
//! 若窗口内 min|z| 明显 = depth → 平面在 -depth（旧 quad 位置对）。
//!
//! 用法：
//!   cargo run -p sc-exporter --release --example decal_projection_probe -- \
//!       <package> <lot_instance|-> [max_lots] [extra_package...]

use dbpf::Package;
use sc_properties::decal::{DecalDictionary, DECAL_ATLAS_INSTANCE_TYPES};
use sc_properties::{assemble_units, inherit, Key, LotEditorDocument, PropertyFile, UnitTransform};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;

struct Geometry {
    verts: Vec<[f32; 3]>,
    /// 三角形顶点下标（每 3 个一组）。
    tris: Vec<[usize; 3]>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: decal_projection_probe <package> <lot|-> [max] [extra...]").clone();
    let target = args.get(1).cloned().unwrap_or_else(|| "-".into());
    let max_lots: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(4);
    let extras: Vec<String> = args.iter().skip(3).cloned().collect();

    let mut packages: Vec<Package> = vec![Package::open(&path).expect("open lot package")];
    for extra in &extras {
        if let Ok(pkg) = Package::open(extra) {
            packages.push(pkg);
        }
    }
    let target_instance = (target != "-")
        .then(|| u32::from_str_radix(target.trim_start_matches("0x"), 16).ok())
        .flatten();

    let atlases = collect_atlases(&packages);

    let mut handled = 0usize;
    for entry in packages[0].entries() {
        if entry.id.type_id != PROPERTY_TYPE {
            continue;
        }
        if let Some(target) = target_instance
            && entry.id.instance != target
        {
            continue;
        }
        let Ok(raw) = packages[0].read(entry) else { continue };
        let Ok(raw_file) = PropertyFile::parse(&raw) else { continue };
        let resolve = |key: &Key| -> Option<PropertyFile> {
            for source in &packages {
                for candidate in source.entries() {
                    if candidate.id.instance == key.instance
                        && (key.type_id == 0 || candidate.id.type_id == key.type_id)
                    {
                        let Ok(bytes) = source.read(candidate) else { continue };
                        if let Ok(file) = PropertyFile::parse(&bytes) {
                            return Some(file);
                        }
                    }
                }
            }
            None
        };
        let flattened = inherit::flatten_parent_inheritance(raw_file, resolve);
        let units = assemble_units(&flattened);
        let doc = LotEditorDocument::from_property_file(flattened.clone());
        let decals: Vec<_> = units
            .units
            .iter()
            .filter_map(|unit| match unit {
                sc_properties::LotUnit::Decal {
                    index,
                    category,
                    transform: Some(transform),
                    scale,
                    depth,
                    ..
                } => Some((*index, *category, transform.clone(), *scale, *depth)),
                _ => None,
            })
            .collect();
        if decals.is_empty() {
            continue;
        }

        // 候选模型：model + 全部 LOD，取顶点最多者
        let mut candidates: Vec<u32> = Vec::new();
        if let Some(key) = &doc.model {
            candidates.push(key.instance);
        }
        for key in doc.model_lods.iter().flatten() {
            candidates.push(key.instance);
        }
        println!("\n==== lot 0x{:08X}  decals={} ====", entry.id.instance, decals.len());
        let mut best: Option<(u32, Geometry)> = None;
        for instance in &candidates {
            let Some(geometry) = load_geometry(&packages, *instance) else {
                println!("     model 0x{instance:08X}: 不可解");
                continue;
            };
            let (min, max) = bbox(&geometry.verts);
            println!(
                "     model 0x{instance:08X}: verts={} tris={} bbox=({:.1},{:.1},{:.1})–({:.1},{:.1},{:.1})",
                geometry.verts.len(),
                geometry.tris.len(),
                min[0], min[1], min[2], max[0], max[1], max[2]
            );
            if best.as_ref().is_none_or(|(_, b)| geometry.verts.len() > b.verts.len()) {
                best = Some((*instance, geometry));
            }
        }
        let Some((model_instance, geometry)) = best else {
            println!("     （无可用模型）");
            handled += 1;
            if target_instance.is_some() || handled >= max_lots {
                return;
            }
            continue;
        };
        println!("     → 采用 0x{model_instance:08X}（顶点最多者）");

        for (index, category, transform, scale, depth) in &decals {
            report_decal(
                &geometry,
                &atlases,
                &flattened,
                *index,
                *category,
                transform,
                *scale,
                *depth,
            );
        }
        handled += 1;
        if target_instance.is_some() || handled >= max_lots {
            return;
        }
    }
    println!("\n共处理 {handled} 个含 decal 的 lot");
}

#[allow(clippy::too_many_arguments)]
fn report_decal(
    geometry: &Geometry,
    atlases: &[DecalDictionary],
    file: &PropertyFile,
    index: usize,
    category: usize,
    transform: &UnitTransform,
    scale: Option<f32>,
    depth: Option<f32>,
) {
    let m = transform.matrix;
    if m.len() != 12 {
        println!("  [{category}:{index}] transform 长度 {} ≠ 12，跳过", m.len());
        return;
    }
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let basis = [
        [m[0], m[1], m[2]],
        [m[3], m[4], m[5]],
        [m[6], m[7], m[8]],
    ];
    let origin = [m[9], m[10], m[11]];
    let length = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let norms = [length(basis[0]), length(basis[1]), length(basis[2])];
    let scale = scale.unwrap_or(1.0).abs().max(1e-3);
    let aspect = decal_aspect(atlases, file, category, index).unwrap_or(1.0).max(1e-3);
    let half_y = scale / aspect;
    let depth = depth.unwrap_or(0.0);

    // 原点到最近**三角形**的距离
    let mut nearest = f32::MAX;
    for tri in &geometry.tris {
        let d = point_tri_distance(
            origin,
            geometry.verts[tri[0]],
            geometry.verts[tri[1]],
            geometry.verts[tri[2]],
        );
        if d < nearest {
            nearest = d;
        }
    }

    let locals: Vec<[f32; 3]> = geometry
        .verts
        .iter()
        .map(|p| {
            let d = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
            [
                dot(d, basis[0]) / (norms[0] * norms[0]).max(1e-9),
                dot(d, basis[1]) / (norms[1] * norms[1]).max(1e-9),
                dot(d, basis[2]) / (norms[2] * norms[2]).max(1e-9),
            ]
        })
        .collect();
    let mut inside_z: Vec<f32> = locals
        .iter()
        .filter(|l| l[0].abs() <= scale && l[1].abs() <= half_y)
        .map(|l| l[2])
        .collect();
    inside_z.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let q = |v: &[f32], t: f64| -> f32 {
        if v.is_empty() {
            return f32::NAN;
        }
        v[((v.len() - 1) as f64 * t).round() as usize]
    };

    println!(
        "  [{category}:{index}] scale={scale:.3} half_y={half_y:.3} depth={depth:.3} |g|={:.3},{:.3},{:.3}",
        norms[0], norms[1], norms[2]
    );
    println!(
        "      raw12 = [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
        m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]
    );
    println!(
        "      origin=({:.2},{:.2},{:.2})   原点→最近**面**距离={nearest:.3}",
        origin[0], origin[1], origin[2]
    );
    println!(
        "      XY 窗口 {} 顶点，z: min={:.3} p25={:.3} med={:.3} p75={:.3} max={:.3}",
        inside_z.len(),
        q(&inside_z, 0.0),
        q(&inside_z, 0.25),
        q(&inside_z, 0.5),
        q(&inside_z, 0.75),
        q(&inside_z, 1.0)
    );

    // 自适应盒验证：足迹内 3×3 采样点沿 ±局部Z 射线求最近面
    let mut hits: Vec<f32> = Vec::new();
    let mut signs: (usize, usize) = (0, 0);
    let mut miss = 0usize;
    for iy in 0..3 {
        for ix in 0..3 {
            let u = (ix as f32 - 1.0) * 0.6 * scale;
            let v = (iy as f32 - 1.0) * 0.6 * half_y;
            let sample = [
                origin[0] + basis[0][0] * (u / norms[0].max(1e-9)) + basis[1][0] * (v / norms[1].max(1e-9)),
                origin[1] + basis[0][1] * (u / norms[0].max(1e-9)) + basis[1][1] * (v / norms[1].max(1e-9)),
                origin[2] + basis[0][2] * (u / norms[0].max(1e-9)) + basis[1][2] * (v / norms[1].max(1e-9)),
            ];
            let axis = basis[2];
            let mut best: Option<f32> = None;
            for tri in &geometry.tris {
                if let Some(t) = ray_tri(sample, axis, geometry.verts[tri[0]], geometry.verts[tri[1]], geometry.verts[tri[2]]) {
                    if best.is_none_or(|b| t.abs() < b.abs()) {
                        best = Some(t);
                    }
                }
            }
            match best {
                Some(t) => {
                    hits.push(t);
                    if t >= 0.0 {
                        signs.0 += 1;
                    } else {
                        signs.1 += 1;
                    }
                }
                None => miss += 1,
            }
        }
    }
    hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "      ★ 足迹 3×3 射线（±Z）：命中 {}/9（+Z {} / -Z {}），|t|: min={:.3} med={:.3} max={:.3}  离散度={:.3}",
        hits.len(),
        signs.0,
        signs.1,
        q(&hits, 0.0).abs(),
        q(&hits, 0.5).abs(),
        q(&hits, 1.0).abs(),
        if hits.is_empty() { f32::NAN } else { q(&hits, 1.0) - q(&hits, 0.0) }
    );
    let _ = miss;
}

/// Möller–Trumbore：射线 (orig, dir 单位向量) 与三角形求交，返回有符号参数 t。
fn ray_tri(orig: [f32; 3], dir: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<f32> {
    const EPS: f32 = 1e-6;
    let sub = |x: [f32; 3], y: [f32; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let cross = |x: [f32; 3], y: [f32; 3]| {
        [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ]
    };
    let dot = |x: [f32; 3], y: [f32; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < EPS {
        return None;
    }
    let inv = 1.0 / det;
    let tvec = sub(orig, a);
    let u = dot(tvec, p) * inv;
    if !(-EPS..=1.0 + EPS).contains(&u) {
        return None;
    }
    let q = cross(tvec, e1);
    let v = dot(dir, q) * inv;
    if v < -EPS || u + v > 1.0 + EPS {
        return None;
    }
    Some(dot(e2, q) * inv)
}

/// 点到三角形的最短距离（Ericson, Real-Time Collision Detection）。
fn point_tri_distance(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let sub = |x: [f32; 3], y: [f32; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let dot = |x: [f32; 3], y: [f32; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return dot(ap, ap).sqrt();
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return dot(bp, bp).sqrt();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        let q = [a[0] + ab[0] * v, a[1] + ab[1] * v, a[2] + ab[2] * v];
        return length3(sub(p, q));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return dot(cp, cp).sqrt();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        let q = [a[0] + ac[0] * w, a[1] + ac[1] * w, a[2] + ac[2] * w];
        return length3(sub(p, q));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let q = [b[0] + (c[0] - b[0]) * w, b[1] + (c[1] - b[1]) * w, b[2] + (c[2] - b[2]) * w];
        return length3(sub(p, q));
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let q = [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ];
    length3(sub(p, q))
}

fn length3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn decal_aspect(
    atlases: &[DecalDictionary],
    file: &PropertyFile,
    category: usize,
    index: usize,
) -> Option<f32> {
    let ids = file.get(0x0D10_9050 + category as u32)?.array()?;
    let sc_properties::Value::Key(key) = ids.get(index)? else {
        return None;
    };
    atlases.iter().find_map(|dict| {
        dict.entries
            .iter()
            .find(|entry| entry.id.map(|k| k.instance) == Some(key.instance))
            .and_then(|entry| entry.aspect_ratio)
    })
}

fn collect_atlases(packages: &[Package]) -> Vec<DecalDictionary> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for package in packages {
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE {
                continue;
            }
            if !DECAL_ATLAS_INSTANCE_TYPES.contains(&(entry.id.group as u16)) {
                continue;
            }
            if !seen.insert((entry.id.group, entry.id.instance)) {
                continue;
            }
            let Ok(bytes) = package.read(entry) else { continue };
            if let Ok(dict) = DecalDictionary::parse(&bytes) {
                out.push(dict);
            }
        }
    }
    out
}

fn bbox(vertices: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in vertices {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    (min, max)
}

/// 模型全部 mesh 的顶点与三角形（模型局部空间；数据帧 Z-up）。
fn load_geometry(packages: &[Package], instance: u32) -> Option<Geometry> {
    for package in packages {
        for entry in package.entries() {
            if entry.id.type_id != RW4_MODEL_TYPE || entry.id.instance != instance {
                continue;
            }
            let bytes = package.read(entry).ok()?;
            let file = rw4::Rw4File::parse(&bytes).ok()?;
            let mut geometry = Geometry { verts: Vec::new(), tris: Vec::new() };
            for section in file.sections_of_type(rw4::SectionType::MESH) {
                let Ok(mesh) = file.decode_mesh(&bytes, section.number) else { continue };
                let base = geometry.verts.len();
                for vertex in &mesh.vertices {
                    if let Some(position) = vertex.position() {
                        geometry.verts.push(position);
                    }
                }
                for tri in &mesh.triangles {
                    geometry.tris.push([
                        base + tri[0] as usize,
                        base + tri[1] as usize,
                        base + tri[2] as usize,
                    ]);
                }
            }
            if geometry.verts.is_empty() {
                return None;
            }
            return Some(geometry);
        }
    }
    None
}
