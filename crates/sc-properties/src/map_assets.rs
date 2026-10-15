//! Original forest meshes and road extrusion components for the map preview.
use crate::{Kind, PropertyFile, Value};
use base64::Engine as _;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapModel {
    pub id: u32,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
    pub diffuse_png_base64: String,
}

fn png(texture: &rw4::DecodedTexture) -> Option<String> {
    let pixels = texture.decode_top_mip_rgba().ok()?;
    let image = image::RgbaImage::from_raw(texture.width.into(), texture.height.into(), pixels)?;
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(out.into_inner()))
}
pub fn texture_png(packages: &[&dbpf::Package], id: u32) -> Option<String> {
    for package in packages {
        if let Some(e) = package.entries().iter().find(|e| {
            matches!(e.id.type_id, 0x2F4E681B | 0x2F4E681C)
                && e.id.instance == id
                && e.id.group == 0
        }) {
            let bytes = package.read(e).ok()?;
            if e.id.type_id == 0x2F4E681C {
                let tex = rw4::RasterImage::parse(&bytes).ok()?;
                let image = image::RgbaImage::from_raw(
                    tex.width,
                    tex.height,
                    tex.decode_top_mip_rgba().ok()?,
                )?;
                let mut out = std::io::Cursor::new(Vec::new());
                image.write_to(&mut out, image::ImageFormat::Png).ok()?;
                return Some(base64::engine::general_purpose::STANDARD.encode(out.into_inner()));
            }
            let file = rw4::Rw4File::parse(&bytes).ok()?;
            let section = file.sections_of_type(rw4::SectionType::TEXTURE).next()?;
            return png(&file.decode_texture(&bytes, section.number).ok()?);
        }
    }
    None
}
pub fn model(package: &dbpf::Package, id: u32) -> Option<MapModel> {
    let e = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E681B && e.id.instance == id && e.id.group == 0)?;
    let bytes = package.read(e).ok()?;
    let file = rw4::Rw4File::parse(&bytes).ok()?;
    let section = file.sections_of_type(rw4::SectionType::MESH).next()?;
    let mesh = file.decode_mesh(&bytes, section.number).ok()?;
    let diffuse_png_base64 = file
        .sections_of_type(rw4::SectionType::TEXTURE)
        .next()
        .and_then(|tex| file.decode_texture(&bytes, tex.number).ok())
        .and_then(|t| png(&t))
        .unwrap_or_default();
    let mut out = MapModel {
        id,
        positions: vec![],
        normals: vec![],
        uvs: vec![],
        indices: mesh.triangles.iter().flatten().map(|&i| i as u32).collect(),
        diffuse_png_base64,
    };
    for v in mesh.vertices {
        out.positions.extend(v.position()?);
        out.normals.extend(v.normal().unwrap_or([0.0, 0.0, 1.0]));
        out.uvs.extend(v.uv()?);
    }
    Some(out)
}
pub fn forest_models(package: &dbpf::Package) -> Vec<MapModel> {
    [0x4DF43690, 0xC2FBD178, 0x1113B131, 0x89D658DF]
        .into_iter()
        .filter_map(|id| model(package, id))
        .collect()
}

fn prop(package: &dbpf::Package, group: u32, id: u32) -> Option<PropertyFile> {
    let read = |group, id| {
        let entry = package
            .entries()
            .iter()
            .find(|e| e.id.type_id == 0x00B1B104 && e.id.group == group && e.id.instance == id)?;
        PropertyFile::parse(&package.read(entry).ok()?).ok()
    };
    Some(crate::inherit::flatten_parent_inheritance(
        read(group, id)?,
        |k| {
            read(k.group, k.instance).or_else(|| {
                if k.group == 0 {
                    read(group, k.instance)
                } else {
                    None
                }
            })
        },
    ))
}
fn values(p: &PropertyFile, hash: u32) -> &[Value] {
    match p.get(hash).map(|p| &p.kind) {
        Some(Kind::Scalar(v)) => std::slice::from_ref(v),
        Some(Kind::Array(v)) => v,
        _ => &[],
    }
}
fn key(p: &PropertyFile, hash: u32) -> Option<crate::Key> {
    match values(p, hash).first()? {
        Value::Key(k) => Some(k.clone()),
        _ => None,
    }
}
fn v2(p: &PropertyFile, hash: u32) -> Option<[f32; 2]> {
    match values(p, hash).first()? {
        Value::Vector2(v) => Some(*v),
        _ => None,
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadRibbon {
    pub component: u32,
    pub offset: [f32; 3],
    pub scale: [f32; 3],
    pub world_size: [f32; 2],
    pub uv_start: [f32; 2],
    pub uv_end: [f32; 2],
    pub texture: u32,
}
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadAssets {
    pub instances: Vec<RoadInstance>,
    pub ribbons: BTreeMap<u32, Vec<RoadRibbon>>,
    pub textures: BTreeMap<u32, String>,
    pub unsupported_components: Vec<String>,
    pub sweeps: BTreeMap<u32, Vec<RoadSweep>>,
    pub models: BTreeMap<u32, MapModel>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadInstance {
    pub property: u32,
    pub model: u32,
    pub scale: f32,
    pub rotation: [f32; 9],
    pub position: [f32; 3],
}

/// Regional junctions are authored units, not intersections reconstructed from
/// a played city. Only units with embedded road paths are included here.
pub fn append_road_instances(
    out: &mut RoadAssets,
    game: &dbpf::Package,
    packages: &[&dbpf::Package],
    units: &[crate::region_state::StateUnit],
) {
    for unit in units {
        let Some(p) = prop(game, 0x40E1C000, unit.id) else {
            continue;
        };
        if values(&p, 0x0CC8FE7A).is_empty() {
            continue;
        }
        let Some(k) = key(&p, 0x00F9EFBB) else {
            continue;
        };
        if k.type_id != 0x2F4E681B || k.group != 0 {
            continue;
        }
        if let std::collections::btree_map::Entry::Vacant(e) = out.models.entry(k.instance) {
            if let Some(m) = std::iter::once(game)
                .chain(packages.iter().copied())
                .find_map(|pkg| model(pkg, k.instance))
            {
                if !m.diffuse_png_base64.is_empty() {
                    e.insert(m);
                }
            }
        }
        if out.models.contains_key(&k.instance) {
            out.instances.push(RoadInstance {
                property: unit.id,
                model: k.instance,
                scale: unit.scale,
                rotation: unit.rotation,
                position: unit.position,
            });
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadSweep {
    pub model: u32,
    pub offset: [f32; 3],
    pub scale: [f32; 3],
    pub rotation: [f32; 3],
    pub interval: f32,
    pub start: f32,
    pub end: f32,
    pub length: f32,
    pub step: f32,
    pub distort: bool,
    pub repeat_uv: bool,
    pub round_intervals: bool,
    pub instance: bool,
    pub stack_to_ground: bool,
    pub relative_to_ground: bool,
}
/// Preserve component provenance; unsupported model/group/prop components are reported.
pub fn road_assets(game: &dbpf::Package, textures: &[&dbpf::Package], ids: &[u32]) -> RoadAssets {
    let texture_packages: Vec<_> = std::iter::once(game)
        .chain(textures.iter().copied())
        .collect();
    let mut out = RoadAssets::default();
    for &id in ids {
        let Some(path) = prop(game, 0x40E1C400, id) else {
            continue;
        };
        let Some(extrusion) = key(&path, 0x09532375) else {
            continue;
        };
        let Some(p) = prop(game, extrusion.group, extrusion.instance) else {
            continue;
        };
        let mut ribbons = Vec::new();
        let mut sweeps = Vec::new();
        let mut pending = vec![(p, [0.0; 3], [1.0; 3], 0)];
        while let Some((p, parent_offset, parent_scale, depth)) = pending.pop() {
            for (i, v) in values(&p, 0xBCC53802).iter().enumerate() {
                let Value::Key(k) = v else { continue };
                if k.instance == 0 {
                    continue;
                }
                // InitExtrusionComponents falls back to the model column when the
                // optional component config does not resolve (bridge deck/cables).
                let c = prop(game, k.group, k.instance)
                    .or_else(|| match values(&p, 0x09558827).get(i) {
                        Some(Value::Key(m)) => prop(game, m.group, m.instance),
                        _ => None,
                    })
                    .unwrap_or_default();
                // Named Empty component: intentionally emits no geometry.
                if matches!(
                    values(&c, 0x0D942BCE).first(),
                    Some(Value::UInt32(0xC7EE8594))
                ) {
                    continue;
                }
                let xyz = |hash, fallback| {
                    let v = match values(&p, hash).get(i) {
                        Some(Value::Vector3(v)) => *v,
                        _ => fallback,
                    };
                    if hash == 0x0955882C {
                        std::array::from_fn(|j| parent_offset[j] + v[j] * parent_scale[j])
                    } else if hash == 0x79F50F26 {
                        std::array::from_fn(|j| v[j] * parent_scale[j])
                    } else {
                        v
                    }
                };
                if !values(&c, 0xBCC53802).is_empty() && depth < 8 {
                    pending.push((
                        c.clone(),
                        xyz(0x0955882C, [0.0; 3]),
                        xyz(0x79F50F26, [1.0; 3]),
                        depth + 1,
                    ));
                    continue;
                }
                let Some(size) = v2(&c, 0x0D942BD1) else {
                    let instance = key(&c, 0x0D8C29C3);
                    let model_key =
                        instance
                            .clone()
                            .or_else(|| match values(&p, 0x09558827).get(i) {
                                Some(Value::Key(k)) if k.instance != 0 => Some(k.clone()),
                                _ => None,
                            });
                    if let Some(mk) = model_key {
                        if let std::collections::btree_map::Entry::Vacant(e) =
                            out.models.entry(mk.instance)
                        {
                            let mp = prop(game, mk.group, mk.instance);
                            let resource = mp
                                .as_ref()
                                .and_then(|p| key(p, 0x00F9EFBB))
                                .map(|k| k.instance)
                                .unwrap_or(mk.instance);
                            if let Some(mut m) =
                                texture_packages.iter().find_map(|pkg| model(pkg, resource))
                            {
                                m.id = mk.instance;
                                let override_texture = match values(&p, 0x0A49AEA4).get(i) {
                                    Some(Value::Key(k)) if k.instance != 0 => Some(k.clone()),
                                    _ => None,
                                };
                                if let Some(t) = override_texture
                                    .or_else(|| key(&c, 0x0E534076))
                                    .or_else(|| mp.as_ref().and_then(|p| key(p, 0x0E534076)))
                                {
                                    if let Some(png) = texture_png(&texture_packages, t.instance) {
                                        m.diffuse_png_base64 = png;
                                    }
                                }
                                if !m.diffuse_png_base64.is_empty() {
                                    e.insert(m);
                                }
                            }
                        }
                        if out.models.contains_key(&mk.instance) {
                            let number = |hash| match values(&p, hash).get(i) {
                                Some(Value::Float(v)) => *v,
                                _ => 0.0,
                            };
                            let flag =
                                |hash| matches!(values(&c, hash).first(), Some(Value::Bool(true)));
                            let table_flag = |hash, default| match values(&p, hash).get(i) {
                                Some(Value::Bool(v)) => *v,
                                _ => default,
                            };
                            sweeps.push(RoadSweep {
                                model: mk.instance,
                                offset: xyz(0x0955882C, [0.0; 3]),
                                scale: xyz(0x79F50F26, [1.0; 3]),
                                rotation: xyz(0x96906170, [0.0; 3]),
                                interval: number(0x09558829),
                                start: number(0x09558828),
                                end: number(0x0E604A97),
                                length: number(0x0955882A),
                                step: number(0x0955882B),
                                distort: table_flag(0x0955882D, true),
                                repeat_uv: table_flag(0x0955882E, false),
                                round_intervals: table_flag(236967757, instance.is_some()),
                                instance: instance.is_some(),
                                stack_to_ground: flag(0x0E8AF8B8),
                                relative_to_ground: flag(240591623),
                            });
                            continue;
                        }
                    }
                    out.unsupported_components
                        .push(format!("{id:08X}:{:08X}:{:08X}", k.group, k.instance));
                    continue;
                };
                let Some(texture) = key(&c, 0x0E534076) else {
                    continue;
                };
                if let std::collections::btree_map::Entry::Vacant(e) =
                    out.textures.entry(texture.instance)
                {
                    if let Some(png) = texture_png(&texture_packages, texture.instance) {
                        e.insert(png);
                    }
                }
                if !out.textures.contains_key(&texture.instance) {
                    continue;
                }
                ribbons.push(RoadRibbon {
                    component: k.instance,
                    offset: xyz(0x0955882C, [0.0; 3]),
                    scale: xyz(0x79F50F26, [1.0; 3]),
                    world_size: size,
                    uv_start: v2(&c, 0x0D942BCF).unwrap_or([0.0; 2]),
                    uv_end: v2(&c, 0x0D942BD0).unwrap_or([1.0; 2]),
                    texture: texture.instance,
                });
            }
        }
        out.ribbons.insert(id, ribbons);
        out.sweeps.insert(id, sweeps);
    }
    out
}
