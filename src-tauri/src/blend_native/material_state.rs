//! Node-graph derived material state (port of `material_state` / `engine_tree`).
//!
//! Pure accessors over the extraction bundle.  No I/O and no Blender types.

use super::dump::{Bundle, BundleMaterial, BundleObject, NodeTree};
use super::layout::LayoutValues;

/// The plug-in's `engine_tree(m)`: the inner `NY_Facade_Engine` group when
/// present, otherwise the material's own node tree.
pub fn engine_tree<'a>(bundle: &'a Bundle, material: &BundleMaterial) -> Option<&'a NodeTree> {
    let tree = bundle.tree(material.tree.as_deref()?)?;
    let engine = tree
        .node("NY_Facade_Engine")
        .filter(|node| node.node_type == "GROUP");
    if let Some(engine) = engine {
        if let Some(name) = engine.node_tree.as_deref() {
            if let Some(inner) = bundle.tree(name) {
                return Some(inner);
            }
        }
    }
    Some(tree)
}

/// UV layer consumed by the window/facade surface (`NY_Surface_UV` node).
pub fn surface_uv_name(bundle: &Bundle, material: &BundleMaterial) -> String {
    bundle
        .tree(material.tree.as_deref().unwrap_or(""))
        .and_then(|tree| tree.node("NY_Surface_UV"))
        .and_then(|node| node.uv_map.clone())
        .unwrap_or_else(|| "NY_SurfaceUV".to_string())
}

/// UV layer consumed by the body texture pattern (`NY_Resource_Pattern` group).
pub fn base_uv_name(bundle: &Bundle, material: &BundleMaterial, default: &str) -> String {
    engine_tree(bundle, material)
        .and_then(|tree| tree.node("NY_Resource_Pattern"))
        .and_then(|node| node.node_tree.as_deref())
        .and_then(|name| bundle.tree(name))
        .and_then(|pattern| pattern.node("UV"))
        .and_then(|node| node.uv_map.clone())
        .unwrap_or_else(|| default.to_string())
}

fn face_attribute(object: &BundleObject, face_index: usize, name: &str) -> Option<f64> {
    object
        .mesh
        .as_ref()?
        .face_attributes
        .get(name)?
        .get(face_index)?
        .as_f64()
}

/// `material_state.actual_layout`: face attributes override the computed grid,
/// then `NY_Layout_*` node defaults fill the remaining keys.
pub fn actual_layout(
    bundle: &Bundle,
    object: &BundleObject,
    face_index: usize,
    material: &BundleMaterial,
    computed: LayoutValues,
) -> LayoutValues {
    let Some(tree) = engine_tree(bundle, material) else {
        return computed;
    };
    let mut result = computed;
    for key in [
        "width",
        "height",
        "colpitch",
        "grouppitch",
        "rowpitch",
        "nc",
        "ng",
        "nr",
        "startx",
        "starty",
        "xmin",
        "xmax",
        "ymin",
        "ymax",
        "panelheight",
        "crossbar",
        "framex",
        "framey",
        "preserve",
        "decorcols",
        "decorrows",
        "decoinset",
    ] {
        if let Some(attribute) = tree.node(&format!("NY_FaceLayout_{key}")) {
            if let Some(name) = attribute.attribute_name.as_deref() {
                if let Some(value) = face_attribute(object, face_index, name) {
                    set_layout_key(&mut result, key, value);
                }
            }
        } else if let Some(node) = tree.node(&format!("NY_Layout_{key}")) {
            if let Some(value) = node.output(0).and_then(|socket| socket.f64()) {
                set_layout_key(&mut result, key, value);
            }
        }
    }
    result.nc = result.nc.max(0);
    result.ng = result.ng.max(0);
    result.nr = result.nr.max(0);
    result.decor_cols = result.decor_cols.max(0);
    result.decor_rows = result.decor_rows.max(0);
    result
}

fn set_layout_key(layout: &mut LayoutValues, key: &str, value: f64) {
    match key {
        "width" => layout.width = value,
        "height" => layout.height = value,
        "colpitch" => layout.col_pitch = value,
        "grouppitch" => layout.group_pitch = value,
        "rowpitch" => layout.row_pitch = value,
        "nc" => layout.nc = value.round_ties_even() as i32,
        "ng" => layout.ng = value.round_ties_even() as i32,
        "nr" => layout.nr = value.round_ties_even() as i32,
        "startx" => layout.start_x = value,
        "starty" => layout.start_y = value,
        "xmin" => layout.x_min = value,
        "xmax" => layout.x_max = value,
        "ymin" => layout.y_min = value,
        "ymax" => layout.y_max = value,
        "panelheight" => layout.panel_height = value,
        "crossbar" => layout.crossbar = value.round() as i32,
        "framex" => layout.frame_x = value,
        "framey" => layout.frame_y = value,
        "preserve" => layout.preserve = value.round() as i32,
        "decorcols" => layout.decor_cols = value.round_ties_even() as i32,
        "decorrows" => layout.decor_rows = value.round_ties_even() as i32,
        "decoinset" => layout.decor_inset = value.round() as i32,
        _ => {}
    }
}

/// Facade width/height used for the window grid.
///
/// Order mirrors `native_asset`: dynamic face attributes, then the material's
/// `ny_span_x/y`, then the authored surface-UV extent.
pub fn face_span(
    bundle: &Bundle,
    object: &BundleObject,
    face_index: usize,
    material: &BundleMaterial,
) -> (f64, f64) {
    if material.prop_bool("ny_dynamic_layout") {
        if let (Some(width), Some(height)) = (
            face_attribute(object, face_index, "NY_Facade_width"),
            face_attribute(object, face_index, "NY_Facade_height"),
        ) {
            if width > 0.0 && height > 0.0 {
                return (width, height);
            }
        }
    }
    if let (Some(width), Some(height)) = (
        material.prop_f64("ny_span_x"),
        material.prop_f64("ny_span_y"),
    ) {
        return (width, height);
    }
    surface_uv_extent(bundle, object, face_index, material).unwrap_or((0.0, 0.0))
}

fn surface_uv_extent(
    bundle: &Bundle,
    object: &BundleObject,
    face_index: usize,
    material: &BundleMaterial,
) -> Option<(f64, f64)> {
    let layer = surface_uv_name(bundle, material);
    let mesh = object.mesh.as_ref()?;
    let uvs = mesh.uv_layers.get(&layer)?;
    let polygon = mesh.polygons.get(face_index)?;
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for loop_index in &polygon.loops {
        let uv = uvs.get(*loop_index as usize)?;
        min[0] = min[0].min(uv[0]);
        min[1] = min[1].min(uv[1]);
        max[0] = max[0].max(uv[0]);
        max[1] = max[1].max(uv[1]);
    }
    Some((max[0] - min[0], max[1] - min[1]))
}

#[cfg(test)]
mod tests {
    use super::super::dump::Bundle;
    use super::*;

    fn bundle() -> Bundle {
        Bundle::parse(
            r#"{
              "version":1,"unit":1.0,
              "objects":[{"name":"o","props":{},"layers":[],"matrix":[],"mesh":{
                "vertices":[],"polygons":[],"uv_layers":{},"face_attributes":{},"materials":[]}}],
              "materials":[{"name":"m","props":{},"tree":"T"}],
              "node_trees":{
                "T":{"name":"T","nodes":[
                  {"name":"NY_Surface_UV","type":"UVMAP","uv_map":"SURF","inputs":[],"outputs":[]},
                  {"name":"BSDF","type":"BSDF_PRINCIPLED","inputs":[],"outputs":[]}
                ],"links":[]},
                "PAT":{"name":"PAT","nodes":[
                  {"name":"UV","type":"UVMAP","uv_map":"BODY","inputs":[],"outputs":[]}
                ],"links":[]}
              }
            }"#
            .as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn surface_uv_defaults_to_contract_layer() {
        let bundle = bundle();
        let material = bundle.material("m").unwrap();
        assert_eq!(surface_uv_name(&bundle, material), "SURF");
        assert_eq!(
            base_uv_name(&bundle, material, "NY_MaterialUV"),
            "NY_MaterialUV"
        );
    }
}

/// Node-graph state recovery (port of `material_state.recover`).
///
/// Used only for materials without `ny_layer`/`ny_face_state`; it reads the
/// engine group controls and the resource-pattern group back into the state.
pub fn recover(
    bundle: &Bundle,
    catalog: Option<&super::catalog::Catalog>,
    material: &BundleMaterial,
    base: &super::settings::SurfaceState,
) -> Result<(super::settings::SurfaceState, String), String> {
    use super::settings::SurfaceState;
    let engine = engine_tree(bundle, material).ok_or("material has no node tree")?;
    let material_tree = bundle.tree(material.tree.as_deref().unwrap_or(""));
    let role = material.prop_str("ny_kind").unwrap_or("WALL").to_string();
    let mut state = base.clone();
    state.base_role = if role == "ROOF" { "ROOF" } else { "WALL" }.to_string();

    let output = |tree: &NodeTree, name: &str| -> Option<f32> {
        tree.node(name)
            .and_then(|node| node.output(0))
            .and_then(|socket| socket.f32())
    };
    let input = |tree: &NodeTree, name: &str, index: usize| -> Option<f32> {
        tree.node(name)
            .and_then(|node| node.input(index))
            .and_then(|socket| socket.f32())
    };
    let colour = |tree: &NodeTree, name: &str| -> Option<[f32; 4]> {
        tree.node(name)
            .and_then(|node| node.output(0))
            .and_then(|socket| socket.vec4())
    };
    let input_colour = |tree: &NodeTree, name: &str, index: usize| -> Option<[f32; 4]> {
        tree.node(name)
            .and_then(|node| node.input(index))
            .and_then(|socket| socket.vec4())
    };
    let control = |key: &str, default: f64| -> f64 {
        if let Some(outer) = material_tree.and_then(|tree| tree.node("NY_Facade_Engine")) {
            if let Some(socket) = outer.input_named(key) {
                if let Some(value) = socket.f64() {
                    return value;
                }
            }
        }
        if let Some(core) = engine.node("Group") {
            if let Some(socket) = core.input_named(key) {
                if let Some(value) = socket.f64() {
                    return value;
                }
            }
        }
        default
    };

    if role == "WINDOW" {
        let mut v = std::collections::BTreeMap::new();
        for node in &engine.nodes {
            if let Some(key) = node.name.strip_prefix("NY_Layout_") {
                if let Some(value) = node.output(0).and_then(|socket| socket.f64()) {
                    v.insert(key.to_string(), value);
                }
            }
        }
        if !v.contains_key("width") || !v.contains_key("height") {
            eprintln!(
                "[recover-missing-layout] {} tree={:?}",
                material.name, material.tree
            );
        }
        let width = *v.get("width").ok_or("window material lacks layout width")?;
        let height = *v
            .get("height")
            .ok_or("window material lacks layout height")?;
        let nc = (v.get("nc").copied().unwrap_or(1.0).round() as i32).max(1);
        let ng = (v.get("ng").copied().unwrap_or(1.0).round() as i32).max(1);
        let nr = (v.get("nr").copied().unwrap_or(1.0).round() as i32).max(1);
        let col_pitch = v.get("colpitch").copied().unwrap_or(width);
        let group_pitch = v.get("grouppitch").copied().unwrap_or(width);
        let row_pitch = v.get("rowpitch").copied().unwrap_or(height);
        let col_gap = (col_pitch - width).max(0.0);
        let group_gap = (group_pitch - (nc as f64 * width + (nc - 1) as f64 * col_gap)).max(0.0);
        let span_w = material
            .prop_f64("ny_span_x")
            .unwrap_or(*v.get("xmax").unwrap_or(&0.0));
        let span_h = material
            .prop_f64("ny_span_y")
            .unwrap_or(*v.get("ymax").unwrap_or(&0.0));
        let start_x = *v.get("startx").unwrap_or(&0.0);
        let start_y = *v.get("starty").unwrap_or(&0.0);
        let margin = *v.get("xmin").unwrap_or(&0.0);
        let panel_height = *v.get("panelheight").unwrap_or(&0.0);
        let decor_cols = *v.get("decorcols").unwrap_or(&1.0);
        let decor_rows = *v.get("decorrows").unwrap_or(&1.0);
        let decor_inset = *v.get("decoinset").unwrap_or(&0.0);
        state.style = material
            .prop_str("ny_style")
            .unwrap_or(&state.style)
            .to_string();
        state.layout = "GRID".into();
        state.width = width;
        state.height = height;
        state.columns = nc;
        state.groups = ng;
        state.rows = nr;
        state.col_gap = col_gap;
        state.group_gap = group_gap;
        state.row_gap = (row_pitch - height).max(0.0);
        state.vertical_mode = "GAP".into();
        state.row_pitch = row_pitch;
        state.margin = margin;
        state.shift_x = start_x
            - (span_w
                - (ng as f64 * (nc as f64 * width + (nc - 1) as f64 * col_gap)
                    + (ng - 1) as f64 * group_gap))
                / 2.0;
        state.shift_y = start_y
            - (span_h - (nr as f64 * height + (nr - 1) as f64 * (row_pitch - height))) / 2.0;
        state.preserve_frame = *v.get("preserve").unwrap_or(&0.0) != 0.0;
        state.frame_border = v
            .get("framex")
            .copied()
            .unwrap_or(0.1)
            .max(v.get("framey").copied().unwrap_or(0.1));
        state.panel = panel_height > 0.0;
        state.panel_height = panel_height.max(0.05);
        state.decor_style = if !state.panel {
            "NONE".into()
        } else if decor_rows >= 2.0 {
            "BRICKS".into()
        } else if decor_cols >= 2.0 {
            "TWIN".into()
        } else if decor_inset != 0.0 {
            "RECESSED".into()
        } else {
            "PLAIN".into()
        };
        if let Some(color) = colour(engine, "NY_Surface_Wall") {
            state.wall_color = color;
        }
        state.roof_color = state.wall_color;
        if let Some(color) = input_colour(engine, "NY_Tint", 2) {
            state.frame_color = color;
        }
        if let Some(color) = colour(engine, "NY_Surface_Panel") {
            state.panel_color = color;
        }
        if let Some(value) = input(engine, "Normal Map", 1) {
            state.normal = value as f64;
        }
        if let Some(value) = input(engine, "NY_Decor_Normal", 1) {
            state.panel_normal = value as f64;
        }
        if let Some(value) = input(engine, "NY_Surface_Bump", 1) {
            state.wall_normal = value as f64;
        }
        state.base_uv_mode = material
            .prop_str("ny_base_uv_mode")
            .unwrap_or("SURFACE")
            .to_string();
    } else {
        let base_colour = input_colour(engine, "NY_BaseTextureColor", 1).or_else(|| {
            material_tree
                .and_then(|tree| {
                    tree.nodes
                        .iter()
                        .find(|node| node.node_type == "BSDF_PRINCIPLED")
                })
                .and_then(|node| node.input_named("Base Color"))
                .filter(|socket| !socket.linked)
                .and_then(|socket| socket.vec4())
        });
        if let Some(color) = base_colour {
            if state.base_role == "ROOF" {
                state.roof_color = color;
            } else {
                state.wall_color = color;
            }
        }
        state.roof_style = material
            .prop_str("ny_roof_style")
            .unwrap_or("FLAT")
            .to_string();
        state.base_uv_mode = "MATERIAL".into();
    }

    state.glow = control("Interior Glow", state.glow);
    state.roughness = control("Roughness Floor", state.roughness);
    state.palette_row = control("Palette Row", state.palette_row);
    state.source_window_palette = Some(state.palette_row);
    // material_state.recover: node missing (or <=0.5) means WINDOW.
    state.night_room_mode = engine
        .node("NY_NightRoomMode")
        .and_then(|node| node.output(0))
        .and_then(|socket| socket.f64())
        .map(|value| if value > 0.5 { "GROUP" } else { "WINDOW" })
        .unwrap_or("WINDOW")
        .into();
    if let Some(bsdf) = material_tree.and_then(|tree| {
        tree.nodes
            .iter()
            .find(|node| node.node_type == "BSDF_PRINCIPLED")
    }) {
        if let Some(value) = bsdf.input_named("Metallic").and_then(|socket| socket.f64()) {
            state.base_metallic = value;
        }
        if let Some(value) = bsdf
            .input_named("Roughness")
            .and_then(|socket| socket.f64())
        {
            state.base_roughness = value;
        }
    }
    for key in [
        "night_ratio",
        "night_seed",
        "night_variation",
        "relief_strength",
    ] {
        if let Some(node) = engine.node(&format!("NY_Appearance_{key}")) {
            if let Some(value) = node.output(0).and_then(|socket| socket.f64()) {
                match key {
                    "night_seed" => state.night_seed = value.round_ties_even() as i32,
                    "night_ratio" => state.night_ratio = value,
                    "night_variation" => state.night_variation = value,
                    _ => state.relief_strength = value,
                }
            }
        }
    }

    if let Some(catalog) = catalog {
        if let Some(pattern) = engine.node("NY_Resource_Pattern") {
            if let Some(tree) = pattern
                .node_tree
                .as_deref()
                .and_then(|name| bundle.tree(name))
            {
                let rect_scale = tree
                    .node("RectScale")
                    .and_then(|node| node.input(1))
                    .and_then(|socket| socket.array());
                let rect_offset = tree
                    .node("RectOffset")
                    .and_then(|node| node.input(1))
                    .and_then(|socket| socket.array());
                if let (Some(rs), Some(ro)) = (rect_scale, rect_offset) {
                    let rw = rs.first().and_then(|value| value.as_f64()).unwrap_or(1.0);
                    let rh = rs.get(1).and_then(|value| value.as_f64()).unwrap_or(1.0);
                    let rx = ro.first().and_then(|value| value.as_f64()).unwrap_or(0.0);
                    let by = ro.get(1).and_then(|value| value.as_f64()).unwrap_or(0.0);
                    let rect = [rw, rh, rx, 1.0 - by - rh];
                    // Blender renames duplicate datablocks with a numeric
                    // suffix (NY_PACK_Copan_slot1.001); match like the plug-in's
                    // startswith('NY_PACK_<pack>_slot1').
                    let pack = tree
                        .node("Slot1")
                        .and_then(|node| node.image.as_deref())
                        .and_then(|name| {
                            let rest = name.strip_prefix("NY_PACK_")?;
                            let cut = rest.find("_slot")?;
                            (cut > 0).then(|| &rest[..cut])
                        });
                    if let Some(pack) = pack {
                        let palette_offset = tree
                            .node("PaletteX")
                            .and_then(|node| node.input(2))
                            .and_then(|socket| socket.f64())
                            .map(|value| value - 1.0 / 1024.0);
                        let mut best: Option<(&super::catalog::Entry, f64)> = None;
                        for entry in catalog
                            .windows
                            .iter()
                            .chain(&catalog.decor)
                            .chain(&catalog.surfaces)
                            .chain(&catalog.modules)
                        {
                            if entry.pack != pack
                                || entry
                                    .rect
                                    .iter()
                                    .zip(&rect)
                                    .any(|(a, b)| (a - b).abs() >= 1e-6)
                            {
                                continue;
                            }
                            let score = catalog
                                .pack(&entry.pack)
                                .and_then(|pack| pack.rows.first())
                                .and_then(|rows| rows.get(entry.recipe))
                                .map(|row| {
                                    let base = if entry.recipe_layer.as_deref() == Some("上层") {
                                        row[1]
                                    } else {
                                        row[0]
                                    };
                                    (base - palette_offset.unwrap_or(0.0)).abs()
                                })
                                .unwrap_or(f64::INFINITY);
                            if best.map_or(true, |(_, current)| score < current) {
                                best = Some((entry, score));
                            }
                        }
                        if let Some((entry, _)) = best {
                            if state.base_role == "ROOF" {
                                state.roof_texture = entry.id.clone();
                            } else {
                                state.wall_texture = entry.id.clone();
                            }
                            if let Some(value) = tree
                                .node("Scale")
                                .and_then(|node| node.input(1))
                                .and_then(|socket| socket.array())
                                .and_then(|values| values.first())
                                .and_then(|value| value.as_f64())
                            {
                                state.texture_scale = value
                                    / entry.size.map(|size| size[0]).unwrap_or(2.0).max(0.001);
                            }
                            if let Some(value) = tree
                                .node("Rotate")
                                .and_then(|node| node.input_named("Angle"))
                                .and_then(|socket| socket.f64())
                            {
                                state.texture_rotation =
                                    value - entry.rotation.unwrap_or(0.0).to_radians();
                            }
                            if let Some(value) = tree
                                .node("Contrast")
                                .and_then(|node| node.input(1))
                                .and_then(|socket| socket.f64())
                            {
                                state.texture_contrast = value;
                            }
                            if let Some(value) = tree
                                .node("Normal")
                                .and_then(|node| node.input_named("Strength"))
                                .and_then(|socket| socket.f64())
                            {
                                state.texture_normal = value;
                            }
                            state.base_uv_mode = if tree
                                .node("UV")
                                .and_then(|node| node.uv_map.as_deref())
                                .is_some_and(|uv| uv == "NY_MaterialUV" || uv == "UVBase")
                            {
                                "MATERIAL".into()
                            } else {
                                "SURFACE".into()
                            };
                            if let Some(values) = tree
                                .node("NY_TextureOffset")
                                .and_then(|node| node.input(1))
                                .and_then(|socket| socket.array())
                            {
                                state.texture_offset_x = -values
                                    .first()
                                    .and_then(|value| value.as_f64())
                                    .unwrap_or(0.0);
                                state.texture_offset_y = -values
                                    .get(1)
                                    .and_then(|value| value.as_f64())
                                    .unwrap_or(0.0);
                            }
                            // The plug-in reads the pattern group input's
                            // default_value even when the socket is linked,
                            // and mirrors it into the body palette sources.
                            if let Some(socket) = pattern.input_named("Palette Row") {
                                let pattern_default = socket.f64().unwrap_or(-1.0);
                                let enabled = tree
                                    .node("PaletteEnable")
                                    .and_then(|node| node.input(0))
                                    .and_then(|socket| socket.f64())
                                    .unwrap_or(0.0)
                                    != 0.0;
                                state.palette_row = if enabled { pattern_default } else { -1.0 };
                                state.source_body_palette = Some(pattern_default);
                                state.source_body_palette_enabled = Some(enabled);
                            }
                        }
                    }
                }
            }
        }
    }
    if matches!(
        material.name.as_str(),
        "NY_Batch_354b3bb002" | "NY_Region_09f56574de" | "NY_Region_0a397210ec"
    ) {
        eprintln!(
            "[RUSTRECOVER] {} glow {:?} tn {:?} tc {:?} pal {:?} swp {:?} sbp {:?} sbe {:?} wt {:?} wn {:?}",
            material.name,
            state.glow,
            state.texture_normal,
            state.texture_contrast,
            state.palette_row,
            state.source_window_palette,
            state.source_body_palette,
            state.source_body_palette_enabled,
            state.wall_texture,
            state.normal
        );
    }
    Ok((state, role))
}
