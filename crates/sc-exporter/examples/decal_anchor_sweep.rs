//! 只读扫查：decal 原点与建筑表面的耦合统计（2026-10-01 引擎对齐验证）。
//!
//! 引擎渲染侧无任何距离判定（SC_cVolumeDecalManager 实锤），"贴墙"必然
//! 烤在数据里。本探针跨 lot 统计：每个 decal 的**原点→建筑最近三角形
//! 距离**，按材质族（字典 0xCAAD8C9 materialInstance）分组输出
//! min/med/max——若 wall 族聚在 ≈0、浮空族（全息/破洞）显著离墙，
//! 即证明"判定条件在摆放期已烤入变换"。
//!
//! 用法：cargo run -p sc-exporter --release --example decal_anchor_sweep -- \
//!     <lot_package> [max_lots] [extra_package...]

use dbpf::Package;
use sc_properties::decal::{DecalDictionary, DECAL_ATLAS_INSTANCE_TYPES};
use sc_properties::{assemble_units, inherit, Key, LotEditorDocument, PropertyFile};
use std::collections::HashMap;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;

#[derive(Clone)]
struct Geometry {
    verts: Vec<[f32; 3]>,
    tris: Vec<[usize; 3]>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: decal_anchor_sweep <package> [max] [extra...]").clone();
    let max_lots: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(40);
    let mut packages: Vec<Package> = vec![Package::open(&path).expect("open lot package")];
    for extra in &args.iter().skip(2).cloned().collect::<Vec<_>>() {
        if let Ok(pkg) = Package::open(extra) {
            packages.push(pkg);
        }
    }

    // 字典：id_instance → (material_instance, aspect)
    let mut id_to_family: HashMap<u32, (u32, f32)> = HashMap::new();
    for pkg in &packages {
        for e in pkg.entries() {
            if e.id.type_id != PROPERTY_TYPE || !DECAL_ATLAS_INSTANCE_TYPES.contains(&(e.id.group as u16)) {
                continue;
            }
            let Ok(bytes) = pkg.read(e) else { continue };
            let Ok(dict) = DecalDictionary::parse(&bytes) else { continue };
            let family = dict.material.as_ref().map(|k| k.instance).unwrap_or(0);
            let aspect = dict.entries.iter().find_map(|en| en.aspect_ratio).unwrap_or(1.0);
            for en in &dict.entries {
                if let Some(k) = en.id {
                    id_to_family.entry(k.instance).or_insert((family, aspect));
                }
            }
        }
    }

    // family → 距离收集
    let mut by_family: HashMap<u32, Vec<(f32, f32, u32, u32)>> = HashMap::new(); // (dist, depth, lot, in_bbox)
    let mut lots_with_decals = 0usize;
    let mut geometry_cache: HashMap<u32, Option<Geometry>> = HashMap::new();

    for entry in packages[0].entries() {
        if entry.id.type_id != PROPERTY_TYPE || lots_with_decals >= max_lots {
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
                        if let Ok(bytes) = source.read(candidate) {
                            if let Ok(file) = PropertyFile::parse(&bytes) {
                                return Some(file);
                            }
                        }
                    }
                }
            }
            None
        };
        let flattened = inherit::flatten_parent_inheritance(raw_file, resolve);
        let units = assemble_units(&flattened);
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
        let doc = LotEditorDocument::from_property_file(flattened.clone());
        let Some(model) = doc.model.as_ref() else { continue };
        let geometry = if let Some(g) = geometry_cache.get(&model.instance) {
            g.clone()
        } else {
            let g = load_geometry(&packages, model.instance);
            geometry_cache.insert(model.instance, g.clone());
            g
        };
        let Some(geometry) = geometry else { continue };

        // decal id 键（按 category）
        let id_at = |cat: usize, idx: usize| -> Option<u32> {
            let ids = flattened.get(0x0D10_9050 + cat as u32)?.array()?;
            match ids.get(idx)? {
                sc_properties::Value::Key(k) => Some(k.instance),
                _ => None,
            }
        };

        lots_with_decals += 1;
        // 包围盒（用于判定 origin 是否在建筑体量内）
        let mut bbmin = [f32::MAX; 3];
        let mut bbmax = [f32::MIN; 3];
        for v in &geometry.verts {
            for i in 0..3 {
                bbmin[i] = bbmin[i].min(v[i]);
                bbmax[i] = bbmax[i].max(v[i]);
            }
        }
        let inside_bbox = |o: [f32; 3]| {
            (0..3).all(|i| o[i] >= bbmin[i] - 0.5 && o[i] <= bbmax[i] + 0.5)
        };
        for (index, category, transform, scale, depth) in decals {
            let m = transform.matrix;
            if m.len() != 12 {
                continue;
            }
            let origin = [m[9], m[10], m[11]];
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
            if !nearest.is_finite() {
                continue;
            }
            let (family, _aspect) = id_at(category, index)
                .and_then(|id| id_to_family.get(&id).copied())
                .unwrap_or((0, 1.0));
            let in_bb = inside_bbox(origin);
            by_family
                .entry(family)
                .or_default()
                .push((nearest, depth.unwrap_or(f32::NAN), entry.id.instance, in_bb as u32));
        }
    }

    println!(
        "扫查 {lots_with_decals} 个含 decal 的 lot；字典 id 索引 {} 条；按材质族统计 origin→建筑最近面距离：\n",
        id_to_family.len()
    );
    let mut fams: Vec<_> = by_family.keys().copied().collect();
    fams.sort();
    for fam in fams {
        let mut rows = by_family[&fam].clone();
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let n = rows.len();
        let med = rows[n / 2].0;
        let depths: Vec<f32> = rows.iter().map(|r| r.1).filter(|d| d.is_finite()).collect();
        let dmed = if depths.is_empty() {
            f32::NAN
        } else {
            let mut d = depths.clone();
            d.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            d[d.len() / 2]
        };
        println!(
            "family 0x{fam:08X}: n={n}  dist min={:.3} med={:.3} max={:.3}  | depth med={dmed:.3}",
            rows[0].0, med, rows[n - 1].0
        );
        let buckets: [(f32, f32); 7] =
            [(0.0, 0.1), (0.1, 0.5), (0.5, 1.0), (1.0, 2.0), (2.0, 5.0), (5.0, 15.0), (15.0, f32::MAX)];
        let hist: Vec<String> = buckets
            .iter()
            .map(|(lo, hi)| {
                let c = rows.iter().filter(|r| r.0 >= *lo && r.0 < *hi).count();
                format!("{lo}-{hi}:{c}")
            })
            .collect();
        let in_bb = rows.iter().filter(|r| r.3 == 1).count();
        println!("    直方图 {}", hist.join("  "));
        println!("    在建筑 bbox 内（±0.5m 容差）: {in_bb}/{n}");
    }
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

/// 点到三角形的最短距离（Ericson）。
fn point_tri_distance(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let sub = |x: [f32; 3], y: [f32; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot3(ab, ap);
    let d2 = dot3(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return dot3(ap, ap).sqrt();
    }
    let bp = sub(p, b);
    let d3 = dot3(ab, bp);
    let d4 = dot3(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return dot3(bp, bp).sqrt();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        let q = [a[0] + ab[0] * v, a[1] + ab[1] * v, a[2] + ab[2] * v];
        return length3(sub(p, q));
    }
    let cp = sub(p, c);
    let d5 = dot3(ab, cp);
    let d6 = dot3(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return dot3(cp, cp).sqrt();
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

fn dot3(x: [f32; 3], y: [f32; 3]) -> f32 {
    x[0] * y[0] + x[1] * y[1] + x[2] * y[2]
}

fn length3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}
