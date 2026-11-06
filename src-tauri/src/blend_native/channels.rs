//! Per-face material state resolution (port of `channels.face_state`).
//!
//! State comes from, in order: the material's `ny_layer` (object layer
//! settings), the material's own `ny_face_state`, another object's layer with
//! the same uid, node-graph recovery (not ported yet), or the material color.
//! The resolver caches by material index like the plug-in.

use super::catalog::Catalog;
use super::dump::{Bundle, BundleMaterial, BundleObject, Node, NodeTree};
use super::material_state::engine_tree;
use super::settings::SurfaceState;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FaceState {
    pub state: SurfaceState,
    pub role: String,
    pub layer: Option<String>,
}

pub struct StateResolver {
    by_material: HashMap<u32, FaceState>,
    /// uid -> (settings_json, role) across every object in the bundle.
    all_layers: HashMap<String, (String, String)>,
}

impl StateResolver {
    pub fn new(bundle: &Bundle) -> Self {
        let mut all_layers = HashMap::new();
        for object in &bundle.objects {
            for layer in &object.layers {
                all_layers.insert(
                    layer.uid.clone(),
                    (layer.settings_json.clone(), layer.role.clone()),
                );
            }
        }
        Self {
            by_material: HashMap::new(),
            all_layers,
        }
    }

    pub fn resolve(
        &mut self,
        bundle: &Bundle,
        catalog: Option<&Catalog>,
        object: &BundleObject,
        face_index: usize,
    ) -> Result<FaceState, String> {
        let mesh = object
            .mesh
            .as_ref()
            .ok_or_else(|| format!("object {} has no mesh", object.name))?;
        let polygon = mesh
            .polygons
            .get(face_index)
            .ok_or_else(|| format!("face {face_index} out of range"))?;
        let material_index = polygon.material;
        if let Some(cached) = self.by_material.get(&material_index) {
            return Ok(cached.clone());
        }
        let material_name = mesh
            .materials
            .get(material_index as usize)
            .and_then(|name| name.as_deref());
        let material = material_name.and_then(|name| bundle.material(name));

        let mut state = SurfaceState::default();
        let mut role = "WALL".to_string();
        let mut layer_uid = None;

        let layer = material
            .and_then(|material| material.prop_str("ny_layer"))
            .and_then(|uid| object.layer(uid));

        if let Some(layer) = layer {
            let saved = material
                .and_then(|material| material.prop_str("ny_face_state"))
                .filter(|text| !text.is_empty())
                .unwrap_or(&layer.settings_json);
            state = SurfaceState::overlay(saved);
            role = material
                .and_then(|material| material.prop_str("ny_kind"))
                .unwrap_or(&layer.role)
                .to_string();
            if role == "WINDOW" && !SurfaceState::has_key(saved, "base_uv_mode") {
                state.base_uv_mode = material
                    .and_then(|material| material.prop_str("ny_base_uv_mode"))
                    .unwrap_or("SURFACE")
                    .to_string();
            }
            if !SurfaceState::has_key(saved, "base_role") {
                state.base_role = if role == "ROOF" { "ROOF" } else { "WALL" }.to_string();
            }
            if role == "ROOF" && !SurfaceState::has_key(saved, "base_metallic") {
                apply_roof_defaults(&mut state);
            }
            layer_uid = Some(layer.uid.clone());
        } else if let Some(material) = material.filter(|material| material.prop_str("ny_kind").is_some()) {
            role = material
                .prop_str("ny_kind")
                .unwrap_or("WALL")
                .to_string();
            let face_state = material
                .prop_str("ny_face_state")
                .filter(|text| !text.is_empty());
            if let Some(text) = face_state {
                state = SurfaceState::overlay(text);
            } else if let Some((settings, source_role)) = material
                .prop_str("ny_layer")
                .and_then(|uid| self.all_layers.get(uid))
            {
                state = SurfaceState::overlay(settings);
                role = source_role.clone();
            } else {
                let (recovered, recovered_role) =
                    super::material_state::recover(bundle, catalog, material, &state)?;
                state = recovered;
                role = recovered_role;
            }
        } else if let Some(material) = material.filter(|material| !material.prop_bool("ny_unpainted")) {
            if let Some(color) = material_color(bundle, material) {
                state.wall_color = color;
            }
        }

        if role == "WINDOW" {
            if let Some(material) = material {
                if let Some(group) = night_room_mode(bundle, material) {
                    state.night_room_mode =
                        if group { "GROUP" } else { "WINDOW" }.to_string();
                }
            }
        }

        let resolved = FaceState {
            state,
            role,
            layer: layer_uid,
        };
        self.by_material
            .insert(material_index, resolved.clone());
        Ok(resolved)
    }
}

fn apply_roof_defaults(state: &mut SurfaceState) {
    let metal = state.roof_style == "METAL";
    state.base_metallic = if metal { 0.8 } else { 0.0 };
    state.base_roughness = if metal { 0.32 } else { 0.85 };
}

fn material_tree<'a>(
    bundle: &'a Bundle,
    material: &BundleMaterial,
) -> Option<&'a NodeTree> {
    bundle.tree(material.tree.as_deref()?)
}

fn material_color(bundle: &Bundle, material: &BundleMaterial) -> Option<[f32; 4]> {
    let tree = material_tree(bundle, material)?;
    let bsdf = tree
        .nodes
        .iter()
        .find(|node| node.node_type == "BSDF_PRINCIPLED")?;
    let base = bsdf.input_named("Base Color")?;
    if base.linked {
        return None;
    }
    base.vec4()
}

fn night_room_mode(bundle: &Bundle, material: &BundleMaterial) -> Option<bool> {
    let tree = engine_tree(bundle, material)?;
    let node: &Node = tree.node("NY_NightRoomMode")?;
    let value = node.output(0).and_then(|socket| socket.f64())?;
    Some(value > 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Bundle {
        Bundle::parse(
            r#"{
              "version":1,"unit":1.0,
              "objects":[{"name":"o","props":{},"matrix":[],"layers":[
                {"name":"l","uid":"u1","role":"WALL","face_ids":"[]","topology":"[]",
                 "settings_json":"{\"width\":2.0,\"base_role\":\"WALL\"}"}
              ],"mesh":{"vertices":[],"polygons":[
                {"vertices":[0,1,2],"loops":[0,1,2],"normal":[0,0,1],"material":0,"area":1.0}
              ],"uv_layers":{},"face_attributes":{},"materials":["m1"]}}],
              "materials":[
                {"name":"m1","props":{"ny_kind":"WINDOW","ny_layer":"u1",
                  "ny_base_uv_mode":"MATERIAL"},"tree":"T"}
              ],
              "node_trees":{"T":{"name":"T","nodes":[
                {"name":"NY_NightRoomMode","type":"VALUE","inputs":[],
                 "outputs":[{"name":"Value","linked":false,"value":1.0}]}
              ],"links":[]}}
            }"#
            .as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn layer_state_overrides_defaults_and_role() {
        let bundle = fixture();
        let object = &bundle.objects[0];
        let mut resolver = StateResolver::new(&bundle);
        let resolved = resolver.resolve(&bundle, None, object, 0).unwrap();
        assert_eq!(resolved.role, "WINDOW");
        assert!((resolved.state.width - 2.0).abs() < 1e-12);
        assert_eq!(resolved.state.base_uv_mode, "MATERIAL");
        assert_eq!(resolved.state.night_room_mode, "GROUP");
        assert_eq!(resolved.layer.as_deref(), Some("u1"));
    }

    #[test]
    fn missing_recovery_is_an_explicit_error() {
        let bundle = Bundle::parse(
            r#"{"version":1,"unit":1.0,
                "objects":[{"name":"o","props":{},"matrix":[],"layers":[],"mesh":{
                  "vertices":[],"polygons":[{"vertices":[],"loops":[],"normal":[0,0,1],
                  "material":0,"area":0.0}],"uv_layers":{},"face_attributes":{},"materials":["m"]}}],
                "materials":[{"name":"m","props":{"ny_kind":"WALL"},"tree":null}],"node_trees":{}}"#
                .as_bytes(),
        )
        .unwrap();
        let mut resolver = StateResolver::new(&bundle);
        let result = resolver.resolve(&bundle, None, &bundle.objects[0], 0);
        assert!(result.is_err(), "recovery without a node tree must fail");
    }
}
