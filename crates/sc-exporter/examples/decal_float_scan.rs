//! 只读扫描：为 holo/墙招牌判别找静态数据依据。
//!
//! 遍历 lot 的 decal unit（限招牌 0x73684EFC / 涂鸦 0xE5390A98 两字典族），
//! 沿 +axisZ 做 3×3 足迹射线（far = depth×scale），打印每个 decal 的
//! scale/depth/material_data 与命中距离分布，末尾按 material_data 聚合
//! 命中/未命中计数——若 holo 的 material_data 与墙招牌系统性不同，
//! 即得到数据驱动的浮空判据。
//!
//! 用法：
//!   cargo run -p sc-exporter --release --example decal_float_scan -- \
//!       <package> [max_lots] [extra_package...]

use dbpf::Package;
use sc_properties::decal::{DecalDictionary, DECAL_ATLAS_INSTANCE_TYPES};
use sc_properties::{assemble_units, inherit, Key, LotEditorDocument, PropertyFile};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;
const SIGN_MATERIAL: u32 = 0x7368_4EFC;
const GRAFFITI_MATERIAL: u32 = 0xE539_0A98;

struct Geometry {
    verts: Vec<[f32; 3]>,
    tris: Vec<[usize; 3]>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: decal_float_scan <package> [max] [extra...]").clone();
    let max_lots: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(200);
    let extras: Vec<String> = args.iter().skip(2).cloned().collect();

    let mut packages: Vec<Package> = vec![Package::open(&path).expect("open lot package")];
    for extra in &extras {
        if let Ok(pkg) = Package::open(extra) {
            packages.push(pkg);
        }
    }

    // decal entry id → 字典 material
    let mut entry_material: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for package in &packages {
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE
                || !DECAL_ATLAS_INSTANCE_TYPES.contains(&(entry.id.group as u16))
            {
                continue;
            }
            let Ok(bytes) = package.read(entry) else { continue };
            if let Ok(dict) = DecalDictionary::parse(&bytes) {
                let Some(material) = dict.material else { continue };
                for e in &dict.entries {
                    if let Some(id) = e.id {
                        entry_material.insert(id.instance, material.instance);
                    }
                }
            }
        }
    }

    let mut handled = 0usize;
    // 聚合：(material_data 离散化) → [命中, 未命中]
    let mut aggregate: std::collections::BTreeMap<String, [usize; 2]> =
        std::collections::BTreeMap::new();
    for entry in packages[0].entries() {
        if entry.id.type_id != PROPERTY_TYPE {
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

        // 收集目标族 decal：id → 字典 material
        let mut decals: Vec<(usize, usize, u32)> = Vec::new(); // (cat, index, dictmat)
        for cat in 0..3u32 {
            let Some(value) = flattened.get(0x0D10_9050 + cat) else { continue };
            let Some(arr) = value.array() else { continue };
            for (index, v) in arr.iter().enumerate() {
                if let sc_properties::Value::Key(k) = v {
                    if let Some(&mat) = entry_material.get(&k.instance) {
                        if mat == SIGN_MATERIAL || mat == GRAFFITI_MATERIAL {
                            decals.push((cat as usize, index, mat));
                        }
                    }
                }
            }
        }
        if decals.is_empty() {
            continue;
        }

        // 几何（顶点最多的模型/LOD）
        let doc = LotEditorDocument::from_property_file(flattened.clone());
        let mut candidates: Vec<u32> = Vec::new();
        if let Some(key) = &doc.model {
            candidates.push(key.instance);
        }
        for key in doc.model_lods.iter().flatten() {
            candidates.push(key.instance);
        }
        let mut best: Option<Geometry> = None;
        for instance in &candidates {
            if let Some(geometry) = load_geometry(&packages, *instance) {
                if best.as_ref().is_none_or(|b| geometry.verts.len() > b.verts.len()) {
                    best = Some(geometry);
                }
            }
        }
        let Some(geometry) = best else { continue };

        let mut printed_header = false;
        for unit in &units.units {
            let sc_properties::LotUnit::Decal {
                index,
                category,
                transform: Some(transform),
                scale,
                depth,
                material_data,
                ..
            } = unit
            else {
                continue;
            };
            let Some(&(_, _, dictmat)) = decals
                .iter()
                .find(|(c, i, _)| c == category && i == index)
            else {
                continue;
            };
            let m = &transform.matrix;
            if m.len() != 12 {
                continue;
            }
            let scale = scale.unwrap_or(1.0).abs().max(1e-3);
            let data_depth = depth.map(|d| d * scale).unwrap_or(scale * 2.5);
            let axis = [m[6], m[7], m[8]];
            let alen = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2])
                .sqrt()
                .max(1e-9);
            let axis = [axis[0] / alen, axis[1] / alen, axis[2] / alen];
            let origin = [m[9], m[10], m[11]];
            let bx = [m[0], m[1], m[2]];
            let by = [m[3], m[4], m[5]];
            let n = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
            let (nx, ny) = (n(bx), n(by));

            let mut hits: Vec<f32> = Vec::new();
            for iy in 0..3 {
                for ix in 0..3 {
                    let u = (ix as f32 - 1.0) * 0.6 * scale;
                    let v = (iy as f32 - 1.0) * 0.6 * scale;
                    let sample = [
                        origin[0] + bx[0] * (u / nx) + by[0] * (v / ny),
                        origin[1] + bx[1] * (u / nx) + by[1] * (v / ny),
                        origin[2] + bx[2] * (u / nx) + by[2] * (v / ny),
                    ];
                    let mut best_t: Option<f32> = None;
                    for tri in &geometry.tris {
                        if let Some(t) = ray_tri(
                            sample,
                            axis,
                            geometry.verts[tri[0]],
                            geometry.verts[tri[1]],
                            geometry.verts[tri[2]],
                        ) {
                            if t >= 0.0 && t <= data_depth && best_t.is_none_or(|b| t < b) {
                                best_t = Some(t);
                            }
                        }
                    }
                    if let Some(t) = best_t {
                        hits.push(t);
                    }
                }
            }
            hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let md = material_data
                .map(|v| format!("[{:.2},{:.2},{:.2}]", v[0], v[1], v[2]))
                .unwrap_or_else(|| "None".into());
            let agg = aggregate.entry(format!("{dictmat:08X}:{md}")).or_default();
            if !printed_header {
                println!("\n== lot 0x{:08X} ==", entry.id.instance);
                printed_header = true;
            }
            if hits.is_empty() {
                agg[1] += 1;
                println!(
                    "  [cat{category}:{index}] {} scale={scale:.2} depth={:?} dataDepth={data_depth:.2} md={} → MISS",
                    if dictmat == SIGN_MATERIAL { "sign" } else { "graffiti" },
                    depth, md
                );
            } else {
                agg[0] += 1;
                println!(
                    "  [cat{category}:{index}] {} scale={scale:.2} depth={:?} dataDepth={data_depth:.2} md={} → hit {}/9 min={:.2} med={:.2} max={:.2}",
                    if dictmat == SIGN_MATERIAL { "sign" } else { "graffiti" },
                    depth, md,
                    hits.len(),
                    hits[0],
                    hits[hits.len() / 2],
                    hits[hits.len() - 1]
                );
            }
        }
        handled += 1;
        if handled >= max_lots {
            break;
        }
    }
    println!("\n==== 聚合（dictmat:material_data → [hit, miss]）====");
    for (key, [hit, miss]) in &aggregate {
        println!("  {key}: hit={hit} miss={miss}");
    }
    println!("共处理 {handled} 个含招牌/涂鸦 decal 的 lot");
}

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
