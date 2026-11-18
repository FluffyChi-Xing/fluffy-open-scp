use super::{
    RoadSchema,
    authored::Layout,
    composition,
    draft::{StripKind, native_template},
    property, property_id,
};
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Key, Kind, PropertyFile, Value, flatten_parent_inheritance};

mod end_cap;
mod junction;
mod mesh;

const APPEARANCE: u32 = 0x0955_8B7E;
const COMPONENT: u32 = 0x0D94_2043;
const KEYS: u32 = 0xBCC5_3802;
const OFFSET: u32 = 0x0955_882C;
const ROTATION: u32 = 0x9690_6170;
const SCALE: u32 = 0x79F5_0F26;
const SWEEP_OFFSET: u32 = 0x0955_882A;
const TAGS: u32 = 0x0A49_AEA4;
const MODEL: u32 = 0x0955_8827;
const START_CAP: u32 = 0xEEC1_AE29;
const END_CAP: u32 = 0xF580_FB24;

fn source_row(file: &PropertyFile, x: f32) -> Result<usize, String> {
    let positions = composition::column(file, OFFSET)?;
    let rotations = composition::column(file, ROTATION)?;
    positions
        .iter()
        .zip(rotations)
        .enumerate()
        .filter_map(|(index, values)| match values {
            (Value::Vector3(point), Value::Vector3(rotation))
                if point[2] == 0.0 && rotation[1] == 0.0 && rotation[2] == 0.0 =>
            {
                Some((index, (point[0] - x).abs()))
            }
            _ => None,
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
        .ok_or_else(|| "road_draft_component_missing".into())
}

fn set_last(file: &mut PropertyFile, field: u32, value: Value) -> Result<(), String> {
    let property = file
        .values
        .iter_mut()
        .find(|column| column.hash == field)
        .ok_or("road_draft_component_missing")?;
    match &mut property.kind {
        Kind::Array(rows) if !rows.is_empty() => {
            *rows.last_mut().expect("checked row") = value;
            Ok(())
        }
        _ => Err("road_draft_component_missing".into()),
    }
}

fn component(game: &Package, donor: &PropertyFile, index: usize) -> Result<PropertyFile, String> {
    let Some(Value::Key(key)) = composition::column(donor, KEYS)?.get(index) else {
        return Err("road_draft_component_missing".into());
    };
    let leaf = property(game, property_id(key.group, key.instance))?;
    Ok(flatten_parent_inheritance(leaf, |parent| {
        property(game, property_id(parent.group, parent.instance))
            .or_else(|_| property(game, property_id(COMPONENT, parent.instance)))
            .ok()
    }))
}

fn vector2(file: &PropertyFile, field: u32) -> Result<[f32; 2], String> {
    match file.get(field).and_then(|field| field.scalar()) {
        Some(Value::Vector2(vector)) => Ok(*vector),
        _ => Err("road_draft_ribbon_missing".into()),
    }
}

fn sample_ribbon(
    donor: &PropertyFile,
    index: usize,
    mut ribbon: PropertyFile,
    fraction: [f32; 2],
) -> Result<(PropertyFile, [f32; 3], f32), String> {
    let width = vector2(&ribbon, 0x0D94_2BD1)?[0];
    let Some(Value::Vector3(source_scale)) = composition::column(donor, SCALE)?.get(index) else {
        return Err("road_draft_ribbon_missing".into());
    };
    if width <= 0.0 || source_scale[0] == 0.0 {
        return Err("road_draft_ribbon_missing".into());
    }
    let first = vector2(&ribbon, 0x0D94_2BCF)?;
    let last = vector2(&ribbon, 0x0D94_2BD0)?;
    let (left, right) = if source_scale[0] < 0.0 {
        (last[0], first[0])
    } else {
        (first[0], last[0])
    };
    for (field, t, y) in [
        (0x0D94_2BCF, fraction[0], first[1]),
        (0x0D94_2BD0, fraction[1], last[1]),
    ] {
        let field = ribbon
            .values
            .iter_mut()
            .find(|prop| prop.hash == field)
            .ok_or("road_draft_ribbon_missing")?;
        field.kind = Kind::Scalar(Value::Vector2([left + (right - left) * t, y]));
    }
    Ok((ribbon, *source_scale, width))
}

fn add_ribbon(
    target: &mut PropertyFile,
    donor: &PropertyFile,
    index: usize,
    ribbon: PropertyFile,
    id: ResourceId,
    band: &super::authored::Band,
    fraction: [f32; 2],
) -> Result<OverlayEntry, String> {
    let (ribbon, source_scale, width) = sample_ribbon(donor, index, ribbon, fraction)?;
    composition::append_component(target, donor, index, None)?;
    set_last(target, OFFSET, Value::Vector3([band.position, 0.0, 0.0]))?;
    set_last(
        target,
        KEYS,
        Value::Key(Key {
            type_id: 0,
            group: id.group,
            instance: id.instance,
        }),
    )?;
    set_last(
        target,
        SCALE,
        Value::Vector3([band.width / width, source_scale[1], source_scale[2]]),
    )?;
    Ok(OverlayEntry::new(
        id,
        ribbon.encode_canonical().map_err(|e| e.to_string())?,
    ))
}

pub(super) fn graphics(
    schema: &RoadSchema,
    plan: &Layout,
    game: &Package,
    scripts: &Package,
    graphics: &mut PropertyFile,
) -> Result<Vec<OverlayEntry>, String> {
    let template = native_template(schema.recipe)?;
    let ordinary = composition::key_mut(graphics, 0x0953_2375)?;
    let ordinary = property(game, property_id(ordinary.group, ordinary.instance))?;
    let trees = property(game, property_id(APPEARANCE, 0xE5CE_83A6))?;
    let shade = [0xC8B4_254B, 0x41C2_D443].map(|id| composition::row(&trees, KEYS, id));
    let [first_tree, second_tree] = shade;
    let tree_rows = [first_tree?, second_tree?];
    let mut entries = Vec::new();
    let mut identities = std::collections::HashSet::new();
    let mut reserve = |id| {
        if [game, scripts]
            .iter()
            .any(|source| source.entry(id).is_some())
            || !identities.insert(id)
        {
            Err("road_identity_collision")
        } else {
            Ok(())
        }
    };
    // Retail cGraphicsPath::Init loads both the switched extrusion (10248904)
    // and the live transition surfaces (107E2B61, 10485F9D) from the path entry.
    for (field, mask, caps, trees_visible) in [
        (0x0953_2375, 0, true, true),
        (0x1024_8904, 0x4000_0000, true, true),
        (0x107E_2B61, 0x8000_0000, true, false),
        (0x1048_5F9D, 0xC000_0000, false, false),
    ] {
        let instance = schema.identity ^ mask;
        let link = composition::key_mut(graphics, field)?;
        if link.group != APPEARANCE || instance == 0 {
            return Err("road_draft_component_missing".into());
        }
        let donor = property(game, property_id(APPEARANCE, link.instance))?;
        let grass_id = if mask == 0 { 0xE5CE_83A6 } else { 0x88B5_B07C };
        let grass = property(game, property_id(APPEARANCE, grass_id))?;
        let median = composition::row(
            &grass,
            KEYS,
            if mask == 0 { 0x104D_7368 } else { 0xF447_4630 },
        )?;
        let mut target = donor.clone();
        // Existing at-grade rows describe the complete stock road. Rebuild that
        // table from its native component rows without copying stock lane geometry.
        for column in &mut target.values {
            if let Kind::Array(values) = &mut column.kind {
                values.clear()
            }
        }
        let target_id = property_id(APPEARANCE, instance);
        reserve(target_id)?;
        let mut junction_bands = junction::Bands::new(game, plan)?;
        let mut surfaces = Vec::with_capacity(plan.bands.len());
        let mut ribbons = Vec::with_capacity(plan.bands.len());
        let mut tree_positions = Vec::new();
        for (index, band) in plan.bands.iter().enumerate() {
            let side = if band.position < 0.0 { -1.0 } else { 1.0 };
            let (source, source_index, sample_width) = match band.kind {
                StripKind::Walk => (
                    &donor,
                    source_row(&donor, side * template.walk_offset as f32)?,
                    Some(template.walk_width as f32),
                ),
                StripKind::Forward | StripKind::Backward => (
                    &donor,
                    source_row(
                        &donor,
                        side * (template.offset + template.lanes as f64 * template.width / 2.0)
                            as f32,
                    )?,
                    Some(template.width as f32),
                ),
                _ => (&grass, median, None),
            };
            let ribbon = component(game, source, source_index)?;
            let Some(Value::Vector3(scale)) = composition::column(source, SCALE)?.get(source_index)
            else {
                return Err("road_draft_ribbon_missing".into());
            };
            let donor_width = (vector2(&ribbon, 0x0D94_2BD1)?[0] * scale[0]).abs();
            if donor_width == 0.0 {
                return Err("road_draft_ribbon_missing".into());
            }
            let fraction = sample_width.map_or(1.0, |width| {
                if band.kind == StripKind::Walk {
                    (width / donor_width).min(1.0)
                } else {
                    1.0 / (donor_width / width).round().max(1.0)
                }
            });
            let slice = if side < 0.0 {
                [0.0, fraction]
            } else {
                [1.0 - fraction, 1.0]
            };
            let (sample, _, _) = sample_ribbon(source, source_index, ribbon.clone(), slice)?;
            surfaces.push(mesh::Surface::from_ribbon(&sample)?);
            ribbons.push((source, source_index, ribbon, slice));
            if trees_visible && band.kind == StripKind::Trees {
                tree_positions.push(band.position);
            }
        }
        let mesh_shoulders = junction_bands.mesh_shoulders(&surfaces);
        for (index, (source, source_index, ribbon, slice)) in ribbons.into_iter().enumerate() {
            let band = &plan.bands[index];
            let group = junction_bands.group(index, band.position);
            if mesh_shoulders[group] {
                continue;
            }
            let digest = crate::mod_flow::digest(
                format!("openscp/road/ribbon/{instance:08X}/{index}").as_bytes(),
            );
            let id = property_id(
                COMPONENT,
                u32::from_str_radix(&digest[..8], 16).map_err(|e| e.to_string())?,
            );
            reserve(id)?;
            for (part, slice, child) in junction_bands.slices(index, band, slice) {
                let child_id = if part == 0 {
                    id
                } else {
                    let digest = crate::mod_flow::digest(
                        format!("openscp/road/ribbon/{instance:08X}/{index}/right").as_bytes(),
                    );
                    let right = property_id(
                        COMPONENT,
                        u32::from_str_radix(&digest[..8], 16).map_err(|e| e.to_string())?,
                    );
                    reserve(right)?;
                    right
                };
                entries.push(add_ribbon(
                    junction_bands.table(index, child.position),
                    source,
                    source_index,
                    ribbon.clone(),
                    child_id,
                    &child,
                    slice,
                )?);
            }
        }
        for (group, table) in junction_bands.into_tables().into_iter().enumerate() {
            let digest = crate::mod_flow::digest(
                format!("openscp/road/junction/{instance:08X}/{group}").as_bytes(),
            );
            let id = property_id(
                COMPONENT,
                u32::from_str_radix(&digest[..8], 16).map_err(|e| e.to_string())?,
            );
            let side = if group < 2 { -1.0 } else { 1.0 };
            let x = if matches!(group, 0 | 3) {
                template.walk_offset
            } else {
                template.offset + template.lanes as f64 * template.width / 2.0
            };
            let row = source_row(&donor, (side * x) as f32)?;
            if mesh_shoulders[group] {
                let model = mesh::id(instance, "shoulder", group);
                entries.extend(junction::append_shoulder(
                    &mut target,
                    &donor,
                    row,
                    group,
                    plan,
                    &surfaces,
                    model,
                    &mut reserve,
                )?);
                continue;
            }
            reserve(id)?;
            entries.push(junction::append(
                &mut target,
                &donor,
                group,
                row,
                table,
                id,
            )?);
        }
        for position in tree_positions {
            for tree in tree_rows {
                composition::append_component(
                    &mut target,
                    &trees,
                    tree,
                    Some([position, 0.0, 0.0]),
                )?;
            }
        }
        if caps {
            entries.extend(end_cap::append(
                &mut target,
                &donor,
                plan,
                &surfaces,
                instance,
                mask == 0,
                &mut reserve,
            )?);
        }
        let bounds = property_id(COMPONENT, mesh::id(instance, "junction-width", 0).instance);
        reserve(bounds)?;
        entries.push(junction::align_bounds(
            &mut target,
            &ordinary,
            plan,
            bounds,
        )?);
        let count = composition::column(&target, KEYS)?.len();
        if count == 0
            || target
                .values
                .iter()
                .filter_map(|prop| prop.array())
                .any(|rows| rows.len() != count)
        {
            return Err("road_draft_component_missing".into());
        }
        link.instance = instance;
        entries.push(OverlayEntry::new(
            target_id,
            target.encode_canonical().map_err(|e| e.to_string())?,
        ));
    }
    // InitFromSegments gets the arm width from the active draw mode. Use the
    // authored width during valid placement as well as after construction.
    composition::key_mut(graphics, 0x0953_2377)?.instance = schema.identity;
    let field = graphics
        .values
        .iter_mut()
        .find(|field| field.hash == 0x0EA2_41EA)
        .ok_or("road_draft_lane_donor_invalid")?;
    let Kind::Array(lanes) = &mut field.kind else {
        return Err("road_draft_lane_donor_invalid".into());
    };
    let [Value::Key(config)] = lanes.as_mut_slice() else {
        return Err("road_draft_lane_donor_invalid".into());
    };
    config.instance = schema.identity;
    Ok(entries)
}
