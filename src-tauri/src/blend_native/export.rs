//! Native export orchestration (port of `native_asset._native_export_impl`).
//!
//! The only module that combines state, recipes, geometry and writers.  It
//! returns the LOTM bytes; file placement stays in the command layer.

use super::catalog::{Catalog, CatalogIndex, Entry};
use super::channels::StateResolver;
use super::decals;
use super::dump::{Bundle, BundleMaterial, BundleObject};
use super::geometry;
use super::glb::{self, GlbMesh};
use super::layout::{LayoutValues, layout_values};
use super::lotm::{self, LotmDocument, LotmMaterial};
use super::material_state::{self, actual_layout};
use super::recipe::{self, Kind, RecipeInput};
use super::resources::PackStore;
use super::settings::SurfaceState;
use std::collections::HashMap;

pub struct ExportInput<'a> {
    pub bundle: &'a Bundle,
    pub catalog: &'a Catalog,
    pub index: &'a CatalogIndex,
    pub store: &'a PackStore,
}

#[derive(Debug, Clone, Default)]
pub struct ExportReport {
    pub meshes: usize,
    pub materials: usize,
    pub windows: usize,
    pub lit_windows: usize,
    pub decor_panels: usize,
    pub decals: usize,
    pub triangles: usize,
    /// `(kind|entry, triangles)` for every non-empty group.
    pub group_stats: Vec<(String, usize)>,
    pub lotm: Vec<u8>,
}

#[derive(Debug)]
struct Group {
    name: String,
    key: String,
    order: usize,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    tangents: Vec<[f32; 4]>,
    uv0: Vec<[f32; 2]>,
    uv1: Vec<[f32; 2]>,
    uv2: Vec<[f32; 2]>,
    uv3: Vec<[f32; 2]>,
    indices: Vec<u32>,
    maps: [Vec<u8>; 5],
    params: [f32; 16],
    param_cols: u32,
}

pub fn export(input: &ExportInput<'_>) -> Result<ExportReport, String> {
    let origin = bounds_origin(input.bundle);
    let unit = input.bundle.unit;
    let mut groups: Vec<Group> = Vec::new();
    let mut by_key: HashMap<String, usize> = HashMap::new();
    let mut windows = 0usize;
    let mut lit_windows = 0usize;
    let mut decor_panels = 0usize;
    let mut decals = 0usize;
    let mut source_faces = 0usize;

    for object in input.bundle.building_objects() {
        let mesh = object.mesh.as_ref().ok_or("building object has no mesh")?;
        if mesh.polygons.is_empty() {
            continue;
        }
        let mut resolver = StateResolver::new(input.bundle);
        let mut rendered: HashMap<u32, SurfaceState> = HashMap::new();
        for face_index in 0..mesh.polygons.len() {
            source_faces += 1;
            let resolved =
                resolver.resolve(input.bundle, Some(input.catalog), object, face_index)?;
            let mut state = resolved.state.clone();
            let role = resolved.role.clone();
            let material = mesh
                .materials
                .get(mesh.polygons[face_index].material as usize)
                .and_then(|name| name.as_deref())
                .and_then(|name| input.bundle.material(name));

            // native_asset re-renders every face whose material carries
            // `ny_kind` through material_state.recover, overriding the
            // channel state (this is where recovered glow/palette live).
            if let Some(material) = material {
                if material.prop_str("ny_kind").is_some() {
                    let material_index = mesh.polygons[face_index].material;
                    if let Some(cached) = rendered.get(&material_index) {
                        state = cached.clone();
                    } else {
                        let (recovered, _) = material_state::recover(
                            input.bundle,
                            Some(input.catalog),
                            material,
                            &state,
                        )
                        .map_err(|error| format!("{}: {error}", material.name))?;
                        rendered.insert(material_index, recovered.clone());
                        state = recovered;
                    }
                }
            }

            let texture_key = if state.base_role == "ROOF" {
                state.roof_texture.clone()
            } else {
                state.wall_texture.clone()
            };
            let entry = input.index.get(&texture_key);
            let color = if state.base_role == "ROOF" {
                state.roof_color
            } else {
                state.wall_color
            };
            // recipe(): body palrow = _source_body_palette if enabled else -1
            let body_enabled = state
                .source_body_palette_enabled
                .unwrap_or(state.palette_row >= 0.0);
            let body_palette = if body_enabled {
                state.source_body_palette.unwrap_or(state.palette_row)
            } else {
                -1.0
            };
            let mut body_state = state.clone();
            body_state.palette_row = body_palette;
            let body_input = RecipeInput {
                kind: Kind::Body,
                entry,
                color,
                normal: state.texture_normal,
                palette_row: body_palette,
                variant: None,
                relief: state.relief_strength,
                night_variation: state.night_variation,
                glow: state.glow,
                texture_contrast: state.texture_contrast,
                shared_room: false,
                base_role: &state.base_role,
                wall_color: state.wall_color,
                roof_color: state.roof_color,
            };
            let body_key = recipe_key(
                Kind::Body,
                entry,
                &body_state,
                color,
                state.texture_normal,
                None,
                false,
            );
            let body_group = if let Some(index) = by_key.get(&body_key) {
                *index
            } else {
                let output = recipe::build(input.store, input.catalog, &body_input)?;
                ensure_group(&mut groups, &mut by_key, body_key, "NY_Native", output)
            };

            let world: Vec<[f64; 3]> = mesh.polygons[face_index]
                .vertices
                .iter()
                .map(|index| world_point(object, *index))
                .collect();
            let normal = world_normal(object, mesh.polygons[face_index].normal);
            let mut body_uv = mapped_uv(mesh, face_index, "NY_MaterialUV");
            if role == "WINDOW" && state.base_uv_mode == "SURFACE" {
                body_uv = mapped_uv(mesh, face_index, "NY_SurfaceUV");
            }
            if entry.is_some() {
                if let Some(material) = material {
                    let layer =
                        material_state::base_uv_name(input.bundle, material, "NY_MaterialUV");
                    body_uv = mapped_uv(mesh, face_index, &layer);
                }
            }
            if body_uv.is_none() {
                body_uv = Some(world_frame_uv(&world, normal));
            }
            let mut body_uv = body_uv.unwrap();
            if let Some(entry) = entry {
                apply_texture_transform(&mut body_uv, entry, &state);
            } else {
                body_uv.fill([0.5, 0.5]);
            }
            if matrix_determinant(&object.matrix) < 0.0 {
                body_uv.reverse();
            }
            let world = if matrix_determinant(&object.matrix) < 0.0 {
                let mut reversed = world.clone();
                reversed.reverse();
                reversed
            } else {
                world
            };
            let triangles = geometry::tessellate(&world, normal);
            for triangle in &triangles {
                let positions = triangle.map(|index| world[index]);
                let uvs = triangle.map(|index| body_uv[index]);
                add_polygon(
                    &mut groups[body_group],
                    &positions,
                    normal,
                    &uvs,
                    &uvs,
                    0.75,
                    0,
                    0.0,
                    origin,
                    unit,
                    false,
                );
                // 1.23.7 bodyBackfaceMode=inset-1mm: a reversed copy 1 mm
                // inside the surface, with the negated normal.
                let back_positions: Vec<[f64; 3]> = positions.iter().rev().copied().collect();
                let back_uvs: Vec<[f64; 2]> = uvs.iter().rev().copied().collect();
                let negated = [-normal[0], -normal[1], -normal[2]];
                add_polygon(
                    &mut groups[body_group],
                    &back_positions,
                    negated,
                    &back_uvs,
                    &back_uvs,
                    0.75,
                    0,
                    0.001 / unit,
                    origin,
                    unit,
                    false,
                );
            }

            if state.decal_enabled {
                if let Some(decal_entry) = input.index.get(&state.decal_style) {
                    let duv = decals::coordinates(object, face_index).map_err(|error| {
                        decal_error(error, object, face_index, &role, &state, mesh)
                    })?;
                    let frame = decals::frame(object, face_index).map_err(|error| {
                        decal_error(error, object, face_index, &role, &state, mesh)
                    })?;
                    let mut width = frame.width;
                    let mut height = frame.height;
                    if let (Some(values), Some(heights)) = (
                        mesh.face_attributes.get("NY_Decal_width"),
                        mesh.face_attributes.get("NY_Decal_height"),
                    ) {
                        if let (Some(width_value), Some(height_value)) = (
                            values.get(face_index).and_then(|value| value.as_f64()),
                            heights.get(face_index).and_then(|value| value.as_f64()),
                        ) {
                            width = width_value;
                            height = height_value;
                        }
                    }
                    let decal_palette = input
                        .catalog
                        .pack(&decal_entry.pack)
                        .map(|pack| pack.palette_row)
                        .unwrap_or(0.0);
                    let motif_input = RecipeInput {
                        kind: Kind::Motif,
                        entry: Some(decal_entry),
                        color: state.decal_color,
                        normal: state.decal_normal,
                        palette_row: decal_palette,
                        variant: None,
                        relief: state.relief_strength,
                        night_variation: state.night_variation,
                        glow: state.glow,
                        texture_contrast: state.texture_contrast,
                        shared_room: false,
                        base_role: &state.base_role,
                        wall_color: state.wall_color,
                        roof_color: state.roof_color,
                    };
                    // native_asset passes {**state, palette_row: pack row}
                    // into the motif recipe, so the key must use that row too.
                    let mut motif_state = state.clone();
                    motif_state.palette_row = decal_palette;
                    let motif_key = recipe_key(
                        Kind::Motif,
                        Some(decal_entry),
                        &motif_state,
                        state.decal_color,
                        state.decal_normal,
                        None,
                        false,
                    );
                    let motif_group = if let Some(index) = by_key.get(&motif_key) {
                        *index
                    } else {
                        let output = recipe::build(input.store, input.catalog, &motif_input)?;
                        ensure_group(&mut groups, &mut by_key, motif_key, "NY_Native", output)
                    };
                    let mut duv = duv;
                    if matrix_determinant(&object.matrix) < 0.0 {
                        duv.reverse();
                    }
                    for [x, y, cell_width, cell_height] in decals::cells(&state, width, height)? {
                        for triangle in &triangles {
                            let polygon: Vec<([f64; 3], [f64; 2])> = triangle
                                .iter()
                                .map(|index| (world[*index], [duv[*index][0], duv[*index][1]]))
                                .collect();
                            let clipped = geometry::clip_polygon(
                                &polygon,
                                x,
                                x + cell_width,
                                y,
                                y + cell_height,
                            );
                            if clipped.len() < 3 {
                                continue;
                            }
                            let uv: Vec<[f64; 2]> = clipped
                                .iter()
                                .map(|(_, value)| {
                                    [
                                        (value[0] - x) / cell_width,
                                        1.0 - (value[1] - y) / cell_height,
                                    ]
                                })
                                .collect();
                            let positions: Vec<[f64; 3]> =
                                clipped.iter().map(|(position, _)| *position).collect();
                            add_polygon(
                                &mut groups[motif_group],
                                &positions,
                                normal,
                                &uv,
                                &uv,
                                0.75,
                                0,
                                0.07 / unit,
                                origin,
                                unit,
                                false,
                            );
                            let mut reversed = positions.clone();
                            reversed.reverse();
                            let mut reversed_uv = uv.clone();
                            reversed_uv.reverse();
                            let negated = [-normal[0], -normal[1], -normal[2]];
                            add_polygon(
                                &mut groups[motif_group],
                                &reversed,
                                negated,
                                &reversed_uv,
                                &reversed_uv,
                                0.75,
                                0,
                                0.07 / unit,
                                origin,
                                unit,
                                false,
                            );
                        }
                        decals += 1;
                    }
                }
            }

            if role != "WINDOW" {
                continue;
            }
            let material = material.ok_or_else(|| "window face has no material".to_string())?;
            let surface_layer = material_state::surface_uv_name(input.bundle, material);
            let mut surface = mapped_uv(mesh, face_index, &surface_layer)
                .ok_or_else(|| format!("窗户面缺少 {surface_layer} UV：{}", object.name))?;
            if matrix_determinant(&object.matrix) < 0.0 {
                surface.reverse();
            }
            let (mut width, mut height) =
                material_state::face_span(input.bundle, object, face_index, material);
            if material.prop_bool("ny_dynamic_layout") {
                if let (Some(values), Some(heights)) = (
                    mesh.face_attributes.get("NY_Facade_width"),
                    mesh.face_attributes.get("NY_Facade_height"),
                ) {
                    if let (Some(width_value), Some(height_value)) = (
                        values.get(face_index).and_then(|value| value.as_f64()),
                        heights.get(face_index).and_then(|value| value.as_f64()),
                    ) {
                        width = width_value;
                        height = height_value;
                    }
                }
            }
            let computed = layout_values(
                width,
                height,
                &state,
                input.index.contains(&state.decor_style),
            );
            let layout = actual_layout(input.bundle, object, face_index, material, computed);
            if i128::from(layout.ng) * i128::from(layout.nc) * i128::from(layout.nr) > 20_000 {
                return Err("单面窗户超过 20000，请检查窗户尺寸或面 UV".into());
            }
            let style_entry = style_entry(input.index, &state.style)
                .ok_or_else(|| format!("unknown window style {}", state.style))?;
            let stored_seed = mesh
                .face_attributes
                .get("NY_Facade_nightseed")
                .and_then(|values| values.get(face_index))
                .and_then(|value| value.as_f64());
            let window_seed = stored_seed
                        .map(|value| value.round_ties_even() as i32)
                .unwrap_or_else(|| super::appearance::face_seed(&object.name, face_index as u32));
            let surface_triangles: Vec<Vec<([f64; 3], [f64; 2])>> = triangles
                .iter()
                .map(|triangle| {
                    triangle
                        .iter()
                        .map(|index| (world[*index], surface[*index]))
                        .collect()
                })
                .collect();
            let group_mode = state.night_room_mode == "GROUP";
            for row in 0..layout.nr {
                let y = layout.start_y + row as f64 * layout.row_pitch;
                if y < layout.y_min - 1e-5 || y + layout.height > layout.y_max + 1e-5 {
                    continue;
                }
                for group in 0..layout.ng {
                    for column in 0..layout.nc {
                        let x = layout.start_x
                            + group as f64 * layout.group_pitch
                            + column as f64 * layout.col_pitch;
                        let (lit, hue) = super::appearance::room_choices(
                            window_seed,
                            row,
                            group,
                            column,
                            layout.nc,
                            &state.night_room_mode,
                            state.night_seed,
                            state.night_ratio,
                        );
                        let variant = if lit { Some(hue + 1) } else { Some(0) };
                        // recipe(): window palrow = _source_window_palette,
                        // falling back to the pack row when negative.
                        let window_palette_row =
                            if state.source_window_palette.unwrap_or(state.palette_row) >= 0.0 {
                                state.source_window_palette.unwrap_or(state.palette_row)
                            } else {
                                input
                                    .catalog
                                    .pack(&style_entry.pack)
                                    .map(|pack| pack.palette_row)
                                    .unwrap_or(0.0)
                            };
                        let window_input = RecipeInput {
                            kind: Kind::Window,
                            entry: Some(&style_entry),
                            color: state.frame_color,
                            normal: state.normal,
                            palette_row: window_palette_row,
                            variant,
                            relief: state.relief_strength,
                            night_variation: state.night_variation,
                            glow: state.glow,
                            texture_contrast: state.texture_contrast,
                            shared_room: group_mode,
                            base_role: &state.base_role,
                            wall_color: state.wall_color,
                            roof_color: state.roof_color,
                        };
                        let mut window_state = state.clone();
                        window_state.palette_row = window_palette_row;
                        let window_key = recipe_key(
                            Kind::Window,
                            Some(&style_entry),
                            &window_state,
                            state.frame_color,
                            state.normal,
                            variant,
                            group_mode,
                        );
                        let window_group = if let Some(index) = by_key.get(&window_key) {
                            *index
                        } else {
                            let output = recipe::build(input.store, input.catalog, &window_input)?;
                            ensure_group(&mut groups, &mut by_key, window_key, "NY_Native", output)
                        };
                        let seed = 0.5
                            + ((window_seed
                                + row * 101
                                + group * 37
                                + if group_mode { 0 } else { column * 17 })
                            .rem_euclid(2039)) as f64
                                / 4078.0;
                        let room = group_mode.then(|| {
                            [
                                layout.start_x + group as f64 * layout.group_pitch,
                                y,
                                layout.width + (layout.nc - 1) as f64 * layout.col_pitch,
                                layout.height,
                            ]
                        });
                        if overlay(
                            &mut groups[window_group],
                            &surface_triangles,
                            normal,
                            x,
                            y,
                            layout.width,
                            layout.height,
                            &layout,
                            &state,
                            seed,
                            state.preserve_frame,
                            room,
                            origin,
                            unit,
                        ) {
                            windows += 1;
                            lit_windows += usize::from(lit);
                        }
                        if layout.panel_height > 0.0 {
                            let legacy_panel = legacy_decor_entry(&state.decor_style);
                            let panel_entry = input
                                .index
                                .get(&state.decor_style)
                                .or(legacy_panel.as_ref());
                            if let Some(panel_entry) = panel_entry {
                                let palette = input
                                    .catalog
                                    .pack(&panel_entry.pack)
                                    .map(|pack| pack.palette_row)
                                    .unwrap_or(0.0);
                                let panel_input = RecipeInput {
                                    kind: Kind::Decor,
                                    entry: Some(panel_entry),
                                    color: state.panel_color,
                                    normal: state.panel_normal,
                                    palette_row: palette,
                                    variant: None,
                                    relief: state.relief_strength,
                                    night_variation: state.night_variation,
                                    glow: state.glow,
                                    texture_contrast: state.texture_contrast,
                                    shared_room: false,
                                    base_role: &state.base_role,
                                    wall_color: state.wall_color,
                                    roof_color: state.roof_color,
                                };
                                let panel_key = recipe_key(
                                    Kind::Decor,
                                    Some(panel_entry),
                                    &state,
                                    state.panel_color,
                                    state.panel_normal,
                                    None,
                                    false,
                                );
                                let panel_group = if let Some(index) = by_key.get(&panel_key) {
                                    *index
                                } else {
                                    let output =
                                        recipe::build(input.store, input.catalog, &panel_input)?;
                                    ensure_group(
                                        &mut groups,
                                        &mut by_key,
                                        panel_key,
                                        "NY_Native",
                                        output,
                                    )
                                };
                                for decor_column in 0..layout.decor_cols {
                                    for decor_row in 0..layout.decor_rows {
                                        let panel_x = x + decor_column as f64 * layout.width
                                            / layout.decor_cols as f64;
                                        let panel_y = y - layout.panel_height
                                            + decor_row as f64 * layout.panel_height
                                                / layout.decor_rows as f64;
                                        if overlay(
                                            &mut groups[panel_group],
                                            &surface_triangles,
                                            normal,
                                            panel_x,
                                            panel_y,
                                            layout.width / layout.decor_cols as f64,
                                            layout.panel_height / layout.decor_rows as f64,
                                            &layout,
                                            &state,
                                            seed,
                                            false,
                                            None,
                                            origin,
                                            unit,
                                        ) {
                                            decor_panels += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut meshes = Vec::new();
    let mut materials = Vec::new();
    let mut mesh_materials = Vec::new();
    let mut triangles_total = 0usize;
    for group in &groups {
        if group.indices.is_empty() {
            continue;
        }
        if let Some(triangle) = geometry::first_reversed_triangle(
            &group.positions,
            &group.normals,
            &group.indices,
        ) {
            return Err(format!(
                "{}: triangle {triangle} has winding opposite to its normal",
                group.name
            ));
        }
        let glb = glb::encode(&GlbMesh {
            name: group.name.clone(),
            positions: group.positions.clone(),
            normals: group.normals.clone(),
            tangents: group.tangents.clone(),
            uvs: [
                group.uv0.clone(),
                group.uv1.clone(),
                group.uv2.clone(),
                group.uv3.clone(),
            ],
            indices: group.indices.clone(),
            preview_png: None,
        });
        let material_index = materials.len() as u32;
        materials.push(LotmMaterial::from_recipe(
            group.maps.clone(),
            group.params,
            group.param_cols,
        ));
        meshes.push(glb);
        mesh_materials.push((material_index, 2));
        triangles_total += group.indices.len() / 3;
    }
    let group_stats: Vec<(String, usize)> = groups
        .iter()
        .filter(|group| !group.indices.is_empty())
        .map(|group| (group.key.clone(), group.indices.len() / 3))
        .collect();
    let report = ExportReport {
        meshes: meshes.len(),
        materials: materials.len(),
        windows,
        lit_windows,
        decor_panels,
        decals,
        triangles: triangles_total,
        group_stats,
        lotm: Vec::new(),
    };
    // nativeControlProfile matches Building40's accepted room-state contract.
    // Interiors already place night emission in columns 1/5/9/13 via the Office
    // room sheet (`resources::office_room_sheet`).
    let diagnostics = format!(
        "{{\"schema\":\"openscp.ny-native/1\",\"source\":\"OpenSCP native Rust\",\"nativeControlProfile\":\"native-room-states-transparent-top-v3\",\"windows\":{windows},\"litWindows\":{lit_windows},\"darkWindows\":{},\"decorPanels\":{decor_panels},\"sourceFaces\":{source_faces},\"nativeMaterials\":{},\"warnings\":[],\"decals\":{decals},\"triangles\":{triangles_total}}}",
        windows - lit_windows,
        materials.len(),
    );
    let document = LotmDocument {
        meshes,
        materials,
        mesh_materials,
        diagnostics,
    };
    let mut report = report;
    report.lotm = lotm::encode(&document);
    super::trace::emit(
        "native_export_done",
        serde_json::json!({
            "meshes": report.meshes,
            "materials": report.materials,
            "windows": report.windows,
            "lit_windows": report.lit_windows,
            "decor_panels": report.decor_panels,
            "decals": report.decals,
            "triangles": report.triangles,
            "lotm_bytes": report.lotm.len(),
        }),
    );
    for (key, triangles) in &report.group_stats {
        super::trace::emit(
            "native_group",
            serde_json::json!({ "key": key, "triangles": triangles }),
        );
    }
    Ok(report)
}

fn ensure_group(
    groups: &mut Vec<Group>,
    by_key: &mut HashMap<String, usize>,
    key: String,
    prefix: &str,
    output: recipe::RecipeOutput,
) -> usize {
    if let Some(index) = by_key.get(&key) {
        return *index;
    }
    let index = groups.len();
    groups.push(Group {
        name: format!("{prefix}_{index:03}"),
        key: key.clone(),
        order: index,
        positions: Vec::new(),
        normals: Vec::new(),
        tangents: Vec::new(),
        uv0: Vec::new(),
        uv1: Vec::new(),
        uv2: Vec::new(),
        uv3: Vec::new(),
        indices: Vec::new(),
        maps: output.maps,
        params: output.params,
        param_cols: output.param_cols,
    });
    by_key.insert(key, index);
    index
}

#[allow(clippy::too_many_arguments)]
fn add_polygon(
    group: &mut Group,
    positions: &[[f64; 3]],
    normal: [f64; 3],
    uv: &[[f64; 2]],
    top: &[[f64; 2]],
    seed: f64,
    material_index: u8,
    offset: f64,
    origin: [f64; 3],
    unit: f64,
    flip: bool,
) {
    if positions.len() < 3 {
        return;
    }
    let (tangent, hand) = geometry::tangent(positions, top, normal);
    let start = group.positions.len() as u32;
    let order: Vec<usize> = if flip {
        (0..positions.len()).rev().collect()
    } else {
        (0..positions.len()).collect()
    };
    for index in order {
        let point = positions[index];
        let shifted = [
            (point[0] - origin[0]) * unit + normal[0] * offset * unit,
            (point[1] - origin[1]) * unit + normal[1] * offset * unit,
            (point[2] - origin[2]) * unit + normal[2] * offset * unit,
        ];
        group
            .positions
            .push([shifted[0] as f32, shifted[1] as f32, shifted[2] as f32]);
        group
            .normals
            .push([normal[0] as f32, normal[1] as f32, normal[2] as f32]);
        group.tangents.push([
            tangent[0] as f32,
            tangent[1] as f32,
            tangent[2] as f32,
            hand as f32,
        ]);
        group.uv0.push([uv[index][0] as f32, uv[index][1] as f32]);
        group.uv1.push([material_index as f32 / 255.0, seed as f32]);
        group.uv2.push([uv[index][0] as f32, uv[index][1] as f32]);
        group.uv3.push([top[index][0] as f32, top[index][1] as f32]);
    }
    for index in 1..positions.len() as u32 - 1 {
        // native_asset.addpoly skips degenerate triangles; clipping produces
        // collinear leftovers that must not inflate the vertex/index stream.
        let a = positions[0];
        let b = positions[index as usize];
        let c = positions[index as usize + 1];
        let cross = cross(sub(b, a), sub(c, a));
        let a = group.positions[start as usize];
        let b = group.positions[(start + index) as usize];
        let c = group.positions[(start + index + 1) as usize];
        if dot(cross, cross) > 1e-16 && a != b && b != c && a != c {
            group
                .indices
                .extend([start, start + index, start + index + 1]);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn overlay(
    group: &mut Group,
    triangles: &[Vec<([f64; 3], [f64; 2])>],
    normal: [f64; 3],
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    layout: &LayoutValues,
    state: &SurfaceState,
    seed: f64,
    preserve: bool,
    room: Option<[f64; 4]>,
    origin: [f64; 3],
    unit: f64,
) -> bool {
    let x0 = x.max(layout.x_min);
    let x1 = (x + width).min(layout.x_max);
    let y0 = y.max(layout.y_min);
    let y1 = (y + height).min(layout.y_max);
    if x1 <= x0 || y1 <= y0 {
        return false;
    }
    let border_x = state.frame_border.min(width * 0.24);
    let border_y = state.frame_border.min(height * 0.24);
    let mut xs = vec![x0, x1];
    let mut ys = vec![y0, y1];
    if preserve {
        for value in [x + border_x, x + width - border_x] {
            if value > x0 && value < x1 {
                xs.push(value);
            }
        }
        for value in [y + border_y, y + height - border_y] {
            if value > y0 && value < y1 {
                ys.push(value);
            }
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup();
    let sliced = |value: f64, total: f64, border: f64| {
        if !preserve {
            return value / total;
        }
        if value < border {
            return 0.07 * value / border;
        }
        if value > total - border {
            return 1.0 - 0.07 * (total - value) / border;
        }
        0.07 + 0.86 * (value - border) / (total - 2.0 * border)
    };
    let offset = state.offset.clamp(0.02, 0.06) / unit;
    let mut found = false;
    for xa in 0..xs.len() - 1 {
        for ya in 0..ys.len() - 1 {
            for triangle in triangles {
                let clipped =
                    geometry::clip_polygon(triangle, xs[xa], xs[xa + 1], ys[ya], ys[ya + 1]);
                if clipped.len() < 3 {
                    continue;
                }
                let coords: Vec<[f64; 2]> = clipped
                    .iter()
                    .map(|(_, uv)| {
                        [
                            sliced(uv[0] - x, width, border_x),
                            1.0 - sliced(uv[1] - y, height, border_y),
                        ]
                    })
                    .collect();
                let base: Vec<[f64; 2]> = if let Some(room) = room {
                    clipped
                        .iter()
                        .map(|(_, uv)| {
                            [
                                ((uv[0] - room[0]) / room[2]).clamp(0.00001, 0.99999),
                                (1.0 - (uv[1] - room[1]) / room[3]).clamp(0.00001, 0.99999),
                            ]
                        })
                        .collect()
                } else {
                    coords.clone()
                };
                let positions: Vec<[f64; 3]> =
                    clipped.iter().map(|(position, _)| *position).collect();
                let mut reversed_positions = positions.clone();
                reversed_positions.reverse();
                let mut reversed_base = base.clone();
                reversed_base.reverse();
                let mut reversed_coords = coords.clone();
                reversed_coords.reverse();
                let negated = [-normal[0], -normal[1], -normal[2]];
                add_polygon(
                    group,
                    &reversed_positions,
                    negated,
                    &reversed_base,
                    &reversed_coords,
                    seed,
                    0,
                    offset,
                    origin,
                    unit,
                    false,
                );
                add_polygon(
                    group, &positions, normal, &base, &coords, seed, 0, offset, origin, unit, false,
                );
                found = true;
            }
        }
    }
    found
}

/// Adds the face context the plug-in never shows, so a rejected decal face can
/// be found in Blender without guessing.  The stored coordinates matter: a
/// schema-2 face carries its own decal UVs and only needs the planarity check
/// when `NY_DecalCoord` is missing (e.g. an extractor that drops corner
/// attributes).
fn decal_error(
    error: String,
    object: &BundleObject,
    face_index: usize,
    role: &str,
    state: &SurfaceState,
    mesh: &super::dump::BundleMesh,
) -> String {
    let schema = mesh
        .face_attributes
        .get("NY_Decal_schema")
        .and_then(|values| values.get(face_index))
        .and_then(|value| value.as_f64());
    let stored = mesh.corner_attributes.contains_key("NY_DecalCoord");
    super::trace::emit(
        "decal_failed",
        serde_json::json!({
            "object": object.name,
            "face": face_index,
            "role": role,
            "style": state.decal_style,
            "vertices": mesh.polygons[face_index].vertices.len(),
            "schema": schema,
            "stored_coordinates": stored,
        }),
    );
    format!(
        "{error}（对象 {} 面 {}，{} 顶点，角色 {role}，贴花样式 {}，schema {}，已存坐标 {stored}）",
        object.name,
        face_index,
        mesh.polygons[face_index].vertices.len(),
        state.decal_style,
        schema
            .map(|value| value.to_string())
            .unwrap_or_else(|| "无".into()),
    )
}

fn recipe_key(
    kind: Kind,
    entry: Option<&Entry>,
    state: &SurfaceState,
    color: [f32; 4],
    normal: f64,
    variant: Option<usize>,
    shared: bool,
) -> String {
    let id = entry.map(|entry| entry.id.as_str()).unwrap_or("");
    format!(
        "{kind:?}|{id}|{:016x}|{:08x},{:08x},{:08x},{:08x}|{:016x}|{:016x}|{:016x}|{:?}|{:016x}|{:016x}|{shared}",
        state.palette_row.to_bits(),
        color[0].to_bits(),
        color[1].to_bits(),
        color[2].to_bits(),
        color[3].to_bits(),
        normal.to_bits(),
        state.glow.to_bits(),
        state.texture_contrast.to_bits(),
        variant,
        state.night_variation.to_bits(),
        state.relief_strength.to_bits(),
    )
}

fn world_point(object: &BundleObject, index: u32) -> [f64; 3] {
    let mesh = object.mesh.as_ref().unwrap();
    let point = mesh.vertices[index as usize];
    let m = &object.matrix;
    [
        m[0] * point[0] + m[1] * point[1] + m[2] * point[2] + m[3],
        m[4] * point[0] + m[5] * point[1] + m[6] * point[2] + m[7],
        m[8] * point[0] + m[9] * point[1] + m[10] * point[2] + m[11],
    ]
}

fn world_normal(object: &BundleObject, normal: [f64; 3]) -> [f64; 3] {
    let m = &object.matrix;
    let a = [m[0], m[1], m[2]];
    let b = [m[4], m[5], m[6]];
    let c = [m[8], m[9], m[10]];
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-12 {
        return normal;
    }
    let cofactor = [cross(b, c), cross(c, a), cross(a, b)];
    let inv = 1.0 / det;
    // (M^-1)^T n: the adjugate rows are det*M^-1, so combine them row-wise.
    let n = [
        (cofactor[0][0] * normal[0] + cofactor[0][1] * normal[1] + cofactor[0][2] * normal[2])
            * inv,
        (cofactor[1][0] * normal[0] + cofactor[1][1] * normal[1] + cofactor[1][2] * normal[2])
            * inv,
        (cofactor[2][0] * normal[0] + cofactor[2][1] * normal[1] + cofactor[2][2] * normal[2])
            * inv,
    ];
    normalize(n).unwrap_or(normal)
}

fn mapped_uv(
    mesh: &super::dump::BundleMesh,
    face_index: usize,
    layer: &str,
) -> Option<Vec<[f64; 2]>> {
    let uvs = mesh.uv_layers.get(layer)?;
    let polygon = mesh.polygons.get(face_index)?;
    let mut result = Vec::with_capacity(polygon.loops.len());
    for loop_index in &polygon.loops {
        result.push(*uvs.get(*loop_index as usize)?);
    }
    Some(result)
}

fn world_frame_uv(world: &[[f64; 3]], normal: [f64; 3]) -> Vec<[f64; 2]> {
    let up = if normal[2].abs() < 0.8 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let right = normalize(cross(up, normal)).unwrap_or([1.0, 0.0, 0.0]);
    let up = normalize(cross(normal, right)).unwrap_or([0.0, 1.0, 0.0]);
    let origin = world[0];
    world
        .iter()
        .map(|point| {
            let delta = sub(*point, origin);
            [dot(delta, right), dot(delta, up)]
        })
        .collect()
}

fn apply_texture_transform(uv: &mut [[f64; 2]], entry: &Entry, state: &SurfaceState) {
    let [sx, sy] = entry.size.unwrap_or([2.0, 2.0]);
    let angle = state.texture_rotation + entry.rotation.unwrap_or(0.0).to_radians();
    let (sin, cos) = angle.sin_cos();
    let scale = state.texture_scale.max(0.01);
    for value in uv {
        let x = value[0] / (sx * scale);
        let y = value[1] / (sy * scale);
        value[0] = x * cos - y * sin - state.texture_offset_x;
        value[1] = -(x * sin + y * cos - state.texture_offset_y);
    }
}

fn style_entry(index: &CatalogIndex, style: &str) -> Option<Entry> {
    if let Some(entry) = index.get(style) {
        return Some(entry.clone());
    }
    // First three presets are authored as normalized rects, the rest as pixel
    // rectangles in the shared 256×256 window sheet (`ATLAS_WINDOWS` in the
    // plug-in).  Keep the pixel form so the derived rects stay bit-identical
    // (rounding 0.1640625 to 0.1641 shifts every texture transform).
    let rect = match style {
        "CLASSIC" => Some([0.125, 0.199, 0.449, 0.371]),
        "PANEL" => Some([0.187, 0.187, 0.012, 0.383]),
        "TALL" => Some([0.145, 0.265, 0.582, 0.367]),
        _ => ATLAS_WINDOWS
            .iter()
            .find(|(id, _)| *id == style)
            .map(|(_, [x0, y0, x1, y1])| {
                [(x1 - x0) / 256.0, (y1 - y0) / 256.0, x0 / 256.0, y0 / 256.0]
            }),
    };
    rect.map(|rect| Entry {
        id: style.to_string(),
        name: style.to_string(),
        pack: "Commercial".into(),
        rect,
        recipe: 19,
        recipe_layer: Some("上层".into()),
        size: None,
        rotation: None,
        shade_mean: None,
        room_tile: None,
    })
}

/// Pixel rectangles of the shared 256×256 window sheet (plug-in `ATLAS_WINDOWS`).
const ATLAS_WINDOWS: [(&str, [f64; 4]); 17] = [
    ("SQUARE", [3.0, 99.0, 53.0, 149.0]),
    ("SASH_EQUAL", [76.0, 94.0, 112.0, 130.0]),
    ("TRANSOM_MEDIUM", [113.0, 94.0, 149.0, 147.0]),
    ("TRANSOM_TALL", [150.0, 94.0, 186.0, 163.0]),
    ("FIVE_LIGHT", [3.0, 154.0, 53.0, 175.0]),
    ("HORIZONTAL_SASH", [3.0, 173.0, 53.0, 196.0]),
    ("NARROW_TRANSOM", [54.0, 154.0, 90.0, 222.0]),
    ("STACKED_PAIR", [95.0, 155.0, 130.0, 217.0]),
    ("SINGLE_UPPER", [95.0, 155.0, 130.0, 186.0]),
    ("SINGLE_LOWER", [95.0, 184.0, 130.0, 217.0]),
    ("TRIPLE_LEFT", [135.0, 172.0, 162.0, 253.0]),
    ("TRIPLE_RIGHT", [158.0, 172.0, 185.0, 253.0]),
    ("BAY_GRID", [187.0, 128.0, 239.0, 182.0]),
    ("BAY_TRANSOM", [187.0, 183.0, 239.0, 253.0]),
    ("FRENCH_PAIR", [7.0, 194.0, 49.0, 251.0]),
    ("FRENCH_LEFT", [7.0, 194.0, 28.0, 251.0]),
    ("FRENCH_RIGHT", [28.0, 194.0, 49.0, 251.0]),
];

fn legacy_decor_entry(style: &str) -> Option<Entry> {
    matches!(style, "RECESSED" | "TWIN" | "BRICKS").then(|| Entry {
        id: "legacy_panel".into(),
        name: "legacy_panel".into(),
        pack: "Commercial".into(),
        rect: [34.0 / 256.0, 29.0 / 256.0, 56.0 / 256.0, 222.0 / 256.0],
        recipe: 19,
        recipe_layer: Some("底层".into()),
        size: None,
        rotation: None,
        shade_mean: None,
        room_tile: None,
    })
}

fn bounds_origin(bundle: &Bundle) -> [f64; 3] {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut found = false;
    for object in bundle.building_objects() {
        if let Some(mesh) = &object.mesh {
            for index in 0..mesh.vertices.len() {
                let point = world_point(object, index as u32);
                for axis in 0..3 {
                    min[axis] = min[axis].min(point[axis]);
                    max[axis] = max[axis].max(point[axis]);
                }
                found = true;
            }
        }
    }
    if !found {
        return [0.0, 0.0, 0.0];
    }
    [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, min[2]]
}

fn matrix_determinant(matrix: &[f64]) -> f64 {
    let a = [matrix[0], matrix[1], matrix[2]];
    let b = [matrix[4], matrix[5], matrix[6]];
    let c = [matrix[8], matrix[9], matrix[10]];
    dot(a, cross(b, c))
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn normalize(a: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(a, a).sqrt();
    (length > 1e-12).then(|| [a[0] / length, a[1] / length, a[2] / length])
}

#[cfg(test)]
mod export_tests {
    use super::*;
    use std::fs;

    #[test]
    fn window_presets_derive_rects_from_pixel_rectangles() {
        // Rounding these to 4 decimals (0.1641 instead of 0.1640625) shifts
        // every texture transform and splits window groups the plug-in merges.
        let catalog =
            Catalog::parse(br#"{"packs":{},"windows":[],"decor":[],"surfaces":[],"modules":[]}"#)
                .unwrap();
        let index = catalog.index();
        let french = style_entry(&index, "FRENCH_PAIR").expect("preset");
        assert_eq!(french.rect, [0.1640625, 0.22265625, 0.02734375, 0.7578125]);
        let square = style_entry(&index, "SQUARE").expect("preset");
        assert_eq!(
            square.rect,
            [50.0 / 256.0, 50.0 / 256.0, 3.0 / 256.0, 99.0 / 256.0]
        );
    }

    /// End-to-end native export of an extracted bundle.
    ///
    /// ```text
    /// OPENSCP_TEST_BUNDLE=<bundle.json>     /// OPENSCP_TEST_PACKS=<assets/packs>     /// OPENSCP_TEST_OUT=<out.lotm>     /// cargo test -p fluffy-open-scp native_export_fixture -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "requires an extracted bundle and the vendored atlas packs"]
    fn native_export_fixture() {
        let bundle_path = std::env::var("OPENSCP_TEST_BUNDLE").expect("bundle");
        let packs_path = std::env::var("OPENSCP_TEST_PACKS").expect("packs");
        let out_path = std::env::var("OPENSCP_TEST_OUT").expect("out");
        let bundle = Bundle::parse(&fs::read(bundle_path).unwrap()).unwrap();
        let store = PackStore::new(packs_path);
        let catalog = Catalog::parse(&fs::read(store.catalog_path()).unwrap()).unwrap();
        let index = catalog.index();
        let report = export(&ExportInput {
            bundle: &bundle,
            catalog: &catalog,
            index: &index,
            store: &store,
        })
        .expect("native export");
        fs::write(out_path, &report.lotm).unwrap();
        let mut kinds: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for (key, triangles) in &report.group_stats {
            let kind = key.split('|').next().unwrap_or("?");
            *kinds.entry(kind.to_string()).or_insert(0) += triangles;
        }
        let mut group_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for (key, _) in &report.group_stats {
            let kind = key.split('|').next().unwrap_or("?");
            *group_counts.entry(kind.to_string()).or_insert(0) += 1;
        }
        println!("KINDTRIS {kinds:?}");
        println!("KINDGROUPS {group_counts:?}");
        let mut stats = report.group_stats.clone();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        for (index, (key, triangles)) in stats.iter().take(12).enumerate() {
            println!("group {index}: {key} tris={triangles}");
        }
        println!(
            "meshes={} materials={} windows={} decor={} decals={} triangles={} bytes={}",
            report.meshes,
            report.materials,
            report.windows,
            report.decor_panels,
            report.decals,
            report.triangles,
            report.lotm.len()
        );
    }
}
