//! Derives the game-side geometry route from the author IR and LOTM facts.
//! The route is an implementation detail; authors should only need to provide
//! a model and its semantic scene document.

use super::lotm::{LotmDocument, LotmMaterial};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryRoute {
    pub name: &'static str,
    pub reason: String,
}

pub fn derive(explicit: Option<&str>, doc: &LotmDocument) -> Result<GeometryRoute, String> {
    if let Some(name) = explicit {
        return Ok(GeometryRoute {
            name: route_name(name)?,
            reason: "declared by schema".into(),
        });
    }

    let mesh_count = doc.meshes.len();
    let used_materials: std::collections::BTreeSet<u32> = doc
        .mesh_material
        .iter()
        .map(|binding| binding.material)
        .collect();
    if !doc.meshes.is_empty() && doc.mesh_material.len() != doc.meshes.len() {
        return Err(format!(
            "LOTM mesh/material binding is incomplete: {} meshes but {} bindings",
            doc.meshes.len(),
            doc.mesh_material.len()
        ));
    }
    if used_materials
        .iter()
        .any(|index| (*index as usize) >= doc.materials.len())
    {
        return Err("LOTM mesh/material binding points outside the material table".into());
    }
    let material_count = used_materials.len();
    if material_count == 0 {
        return Ok(GeometryRoute {
            name: "nh-partitioned",
            reason: "no mesh references a LOTM material group; use the external-slot route".into(),
        });
    }
    // Latest author route: preserve every source group as a material row in
    // ONE Building40 mesh and compile all six native slots. Never silently
    // fall back to NH atlas baking or mesh partitioning for a new LOTM.
    Ok(GeometryRoute {
        name: "building40",
        reason: format!(
            "{material_count} mesh-referenced LOTM material groups compile into the author's single Building40 mesh and six-slot native material table"
        ),
    })
}

fn route_name(name: &str) -> Result<&'static str, String> {
    match name {
        // 2026-11-09：生产几何只允许 Building40（Packager 算法的 Rust 内核）。
        // nh-atlas / merged-single / 拆 mesh 系一律移除。
        "building40" => Ok("building40"),
        other => Err(format!(
            "unknown/disallowed geometry route {other:?}: production path is \"building40\" only \
             (Rust asset_build_m1); nh-atlas/merged-single and mesh-splitting routes are removed"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::lotm;

    fn doc(meshes: usize, materials: usize) -> LotmDocument {
        LotmDocument {
            version: 9,
            meshes: vec![Vec::new(); meshes],
            materials: (0..materials)
                .map(|_| lotm::LotmMaterial {
                    pngs: vec![],
                    params_f32: None,
                    param_cols: 0,
                })
                .collect(),
            mesh_material: if materials == 0 {
                Vec::new()
            } else {
                (0..meshes)
                    .map(|index| lotm::MeshMaterialRef {
                        material: (index % materials) as u32,
                        uv_kind: 2,
                    })
                    .collect()
            },
            diagnostics: String::new(),
        }
    }

    #[test]
    fn infers_single_material_route() {
        assert_eq!(derive(None, &doc(66, 1)).unwrap().name, "building40");
    }

    #[test]
    fn infers_four_material_route_without_user_configuration() {
        assert_eq!(derive(None, &doc(4, 4)).unwrap().name, "building40");
    }

    #[test]
    fn infers_building40_for_many_materials() {
        assert_eq!(derive(None, &doc(66, 66)).unwrap().name, "building40");
    }

    #[test]
    fn preserves_plugin_native_material_graph() {
        let mut value = doc(66, 66);
        for material in &mut value.materials {
            material.params_f32 = Some(vec![0; 64]);
            material.param_cols = 1;
            material.pngs = (0..8)
                .map(|slot| {
                    matches!(
                        slot,
                        LotmMaterial::PNG_NORMAL
                            | LotmMaterial::PNG_TINT
                            | LotmMaterial::PNG_PALETTE
                            | LotmMaterial::PNG_SHADER
                            | LotmMaterial::PNG_INTERIOR
                    )
                    .then(|| vec![1])
                })
                .collect();
        }
        let route = derive(None, &value).unwrap();
        assert_eq!(route.name, "building40", "{}", route.reason);
    }

    #[test]
    fn rejects_incomplete_mesh_material_bindings() {
        let mut value = doc(2, 2);
        value.mesh_material.pop();
        let error = derive(None, &value).unwrap_err();
        assert!(error.contains("binding is incomplete"), "{error}");
    }

    #[test]
    fn rejects_legacy_explicit_routes() {
        let error = derive(Some("nh-atlas"), &doc(4, 4)).unwrap_err();
        assert!(error.contains("building40"), "{error}");
        let error = derive(Some("merged-single"), &doc(4, 4)).unwrap_err();
        assert!(error.contains("building40"), "{error}");
    }
}
