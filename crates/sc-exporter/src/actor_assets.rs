//! Actor parts, using the native RW4 skeleton/animation export path.
use dbpf::{Package, ResourceId};
use rw4::{Rw4File, SectionType};
use serde::Serialize;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorPart {
    pub glbs: Vec<Vec<u8>>,
    pub source: Option<Pixels>,
    pub head_joint: Option<usize>,
    pub animation_count: usize,
}
#[derive(Serialize)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
fn hash(name: &str) -> u32 {
    name.bytes()
        .fold(0x811c9dc5, |h, b| h.wrapping_mul(0x01000193) ^ b as u32)
}
/// Exact instance/group lookup; zero type in the descriptor denotes an RW4 part.
pub fn read_part(packages: &[&Package], key: ResourceId) -> Result<ActorPart, String> {
    let pkg = packages
        .iter()
        .find(|p| p.entry(key).is_some())
        .ok_or("actor part missing")?;
    let entry = pkg.entry(key).unwrap();
    if entry.decompressed_size > 32 * 1024 * 1024 {
        return Err("actor part too large".into());
    }
    let data = pkg.read(entry).map_err(|e| e.to_string())?;
    let f = Rw4File::parse(&data).map_err(|e| e.to_string())?;
    let skeleton = f
        .sections_of_type(SectionType::RW4_SKELETON)
        .next()
        .map(|s| f.decode_skeleton(&data, s.number))
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut animations = vec![];
    for s in f.sections_of_type(SectionType::ANIM) {
        let a = f.decode_anim(&data, s.number).map_err(|e| e.to_string())?;
        if skeleton
            .as_ref()
            .is_some_and(|sk| sk.hierarchy.id == a.skeleton_id)
        {
            animations.push(a);
        }
    }
    let head_joint = skeleton.as_ref().and_then(|sk| {
        sk.hierarchy.joints.iter().position(|j| {
            [
                hash("Male_Head"),
                hash("male_head"),
                hash("Female_Head"),
                hash("female_head"),
            ]
            .contains(&j.name_fnv)
        })
    });
    let mut glbs = vec![];
    for s in f.sections_of_type(SectionType::MESH) {
        let mut mesh = f.decode_mesh(&data, s.number).map_err(|e| e.to_string())?;
        // Native sim bodies use SHORT2/FLOAT2 for two influences. The existing
        // exporter expects four lanes; pad this actor-only adapter without changing it.
        for vertex in &mut mesh.vertices {
            for (element, value) in &mut vertex.components {
                match (element.usage, &*value) {
                    (rw4::DeclarationUsage::BlendIndices, rw4::ComponentValue::Short2(v)) => {
                        *value = rw4::ComponentValue::Short4([v[0], v[1], 0, 0]);
                        element.decl_type = rw4::DeclarationType::Short4;
                    }
                    (rw4::DeclarationUsage::BlendWeight, rw4::ComponentValue::Float2(v)) => {
                        *value = rw4::ComponentValue::Float4([v[0], v[1], 0., 0.]);
                        element.decl_type = rw4::DeclarationType::Float4;
                    }
                    _ => {}
                }
            }
        }
        if skeleton.is_some()
            && !animations.is_empty()
            && crate::extract_skin(&mesh, skeleton.as_ref().unwrap(), &animations).is_none()
        {
            return Err("actor skin cannot be exported without losing animation".into());
        }
        if mesh.is_exportable() {
            glbs.push(crate::export_glb(&mesh, skeleton.as_ref(), &animations).bytes)
        }
    }
    if glbs.is_empty() {
        return Err("actor mesh missing".into());
    }
    // Native sim parts carry normal then sourceTex as the two embedded textures.
    // Do not use the existing vehicle fallback, which treats the normal as diffuse.
    let textures: Vec<_> = f.sections_of_type(SectionType::TEXTURE).collect();
    let source = if textures.len() == 2 {
        let t = f
            .decode_texture(&data, textures[1].number)
            .map_err(|e| e.to_string())?;
        Some(Pixels {
            width: t.width as u32,
            height: t.height as u32,
            rgba: t.decode_top_mip_rgba().map_err(|e| e.to_string())?,
        })
    } else {
        None
    };
    Ok(ActorPart {
        glbs,
        source,
        head_joint,
        animation_count: animations.len(),
    })
}
/// simModel/simParticle palette formula: source.R=AO, G=palette V, A=opacity.
/// App renderer resources reference body 69C94C5D and clothes 14C5F13F together.
pub fn palette(packages: &[&Package], instance: u32) -> Result<Pixels, String> {
    let key = ResourceId {
        type_id: 0x2F4E681B,
        group: 0,
        instance,
    };
    let p = packages
        .iter()
        .find(|p| p.entry(key).is_some())
        .ok_or("actor palette missing")?;
    let d = p.read(p.entry(key).unwrap()).map_err(|e| e.to_string())?;
    let f = Rw4File::parse(&d).map_err(|e| e.to_string())?;
    let s = f
        .sections_of_type(SectionType::TEXTURE)
        .next()
        .ok_or("actor palette texture missing")?;
    let t = f.decode_texture(&d, s.number).map_err(|e| e.to_string())?;
    if t.width != 256 || t.height != 32 {
        return Err("actor palette dimensions differ from native contract".into());
    }
    Ok(Pixels {
        width: 256,
        height: 32,
        rgba: t.decode_top_mip_rgba().map_err(|e| e.to_string())?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires installed native sim parts"]
    fn native_body_head_and_palette_contract() {
        let root = std::path::Path::new(
            "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData",
        );
        let app = Package::open(root.join("SimCity_App.package")).unwrap();
        let graphics = Package::open(root.join("SimCity_Graphics.package")).unwrap();
        let packages = vec![&app, &graphics];
        for id in [0xE5537FAB, 0xA4EB8B32, 0xFAC9B16D, 0x66BE4E6E] {
            let part = read_part(
                &packages,
                ResourceId {
                    type_id: 0x2F4E681B,
                    group: 0,
                    instance: id,
                },
            )
            .unwrap();
            assert!(part.animation_count > 0);
            assert!(part.head_joint.is_some(), "head joint {id:08X}");
            assert!(part.source.is_some());
            for glb in &part.glbs {
                let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
                let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + len]).unwrap();
                assert!(json["skins"].as_array().is_some_and(|s| !s.is_empty()));
                assert!(json["animations"].as_array().is_some_and(|s| !s.is_empty()));
            }
            println!(
                "{id:08X} clips={} head={:?}",
                part.animation_count, part.head_joint
            );
        }
        let head = read_part(
            &packages,
            ResourceId {
                type_id: 0x2F4E681B,
                group: 0,
                instance: 0x22A445FE,
            },
        )
        .unwrap();
        assert_eq!(head.animation_count, 0);
        assert_eq!(
            palette(&packages, 0x69C94C5D).unwrap().rgba.len(),
            256 * 32 * 4
        );
        assert_eq!(
            palette(&packages, 0x14C5F13F).unwrap().rgba.len(),
            256 * 32 * 4
        );
    }
}
