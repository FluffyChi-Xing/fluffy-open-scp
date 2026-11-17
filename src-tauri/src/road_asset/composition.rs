use super::{Recipe, RoadSchema, property, property_id};
use dbpf::{OverlayEntry, Package};
use sc_properties::{Key, Kind, PropertyFile, Value};
use serde::Deserialize;

const APPEARANCE: u32 = 0x0955_8B7E;
const COMPONENTS: u32 = 0xBCC5_3802;
const POSITIONS: u32 = 0x0955_882C;
const END: u32 = 0x0E60_4A97;
const SPRING_TREES: u32 = 0xE5CE_83A6;
const TRAM_TRACKS: u32 = 0x355D_03EA;

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Composition {
    #[serde(default)]
    pub(crate) left_trees: bool,
    #[serde(default)]
    pub(crate) right_trees: bool,
    #[serde(default)]
    pub(crate) tram: bool,
}

impl Composition {
    pub(crate) fn active(self) -> bool {
        self.left_trees || self.right_trees || self.tram
    }

    pub(super) fn validate(self, recipe: Recipe) -> Result<(), String> {
        if self.active() && !matches!(recipe, Recipe::MediumStreet | Recipe::Streetcar)
            || self.tram && !matches!(recipe, Recipe::MediumStreet)
        {
            return Err("road_composition_recipe_invalid".into());
        }
        Ok(())
    }
}

fn column(file: &PropertyFile, hash: u32) -> Result<&[Value], String> {
    file.get(hash)
        .and_then(|p| p.array())
        .ok_or_else(|| format!("road_composition_column_missing {hash:08X}"))
}

fn key_mut(file: &mut PropertyFile, hash: u32) -> Result<&mut Key, String> {
    match file
        .values
        .iter_mut()
        .find(|p| p.hash == hash)
        .map(|p| &mut p.kind)
    {
        Some(Kind::Scalar(Value::Key(key))) => Ok(key),
        _ => Err(format!("road_composition_link_missing {hash:08X}")),
    }
}

fn row(file: &PropertyFile, hash: u32, instance: u32) -> Result<usize, String> {
    column(file, hash)?
        .iter()
        .position(|v| matches!(v, Value::Key(k) if k.instance == instance))
        .ok_or_else(|| format!("road_composition_row_missing {instance:08X}"))
}

// Copy every target column from the matching native row. The only optional
// donor column with a nonzero added value is the tree's end clearance.
fn append_component(
    target: &mut PropertyFile,
    donor: &PropertyFile,
    index: usize,
    offset: Option<[f32; 3]>,
) -> Result<(), String> {
    let old_len = column(target, COMPONENTS)?.len();
    let donor_len = column(donor, COMPONENTS)?.len();
    if target.get(END).is_none() && offset.is_some() {
        let mut end = donor.get(END).ok_or("road_tree_end_missing")?.clone();
        end.kind = Kind::Array(vec![Value::Float(0.0); old_len]);
        target.values.push(end);
    }

    for col in &mut target.values {
        let Kind::Array(values) = &mut col.kind else {
            continue;
        };
        if values.len() != old_len {
            return Err(format!("road_component_column_misaligned {:08X}", col.hash));
        }
        let value = if col.hash == POSITIONS && offset.is_some() {
            Value::Vector3(offset.ok_or("road_component_position_missing")?)
        } else if col.hash == END && donor.get(END).is_none() {
            Value::Float(0.0)
        } else {
            let source = column(donor, col.hash)?;
            if source.len() != donor_len {
                return Err(format!("road_component_column_misaligned {:08X}", col.hash));
            }
            source
                .get(index)
                .ok_or("road_component_row_missing")?
                .clone()
        };
        values.push(value);
    }
    Ok(())
}

pub(crate) fn appearance(
    game: &Package,
    recipe: Recipe,
    composition: Composition,
    future: bool,
    mut target: PropertyFile,
) -> Result<PropertyFile, String> {
    composition.validate(recipe)?;
    if composition.tram {
        let donor_id = if future { 0x3D8B_3372 } else { 0x7842_CB94 };
        let donor = property(game, property_id(APPEARANCE, donor_id))?;
        let index = row(&donor, COMPONENTS, TRAM_TRACKS)?;
        property(game, property_id(0x0D94_2043, TRAM_TRACKS))?;
        append_component(&mut target, &donor, index, None)?;
    }
    if composition.left_trees || composition.right_trees {
        let donor = property(game, property_id(APPEARANCE, SPRING_TREES))?;
        let verge = match recipe {
            Recipe::MediumStreet => 10.5,
            Recipe::Streetcar => 22.575,
            _ => return Err("road_composition_recipe_invalid".into()),
        };
        for (enabled, component, x) in [
            (composition.left_trees, 0xC8B4_254B, -verge),
            (composition.right_trees, 0x41C2_D443, verge),
        ] {
            if enabled {
                let index = row(&donor, COMPONENTS, component)?;
                let definition = property(game, property_id(0x0D94_2043, component))?;
                let Some(Value::Key(tree)) = definition.get(0x0D8C_29C3).and_then(|p| p.scalar())
                else {
                    return Err("road_tree_model_missing".into());
                };
                property(game, property_id(tree.group, tree.instance))?;
                append_component(&mut target, &donor, index, Some([x, 0.0, 0.0]))?;
            }
        }
    }
    Ok(target)
}

fn encode(file: &PropertyFile) -> Result<Vec<u8>, String> {
    file.encode_canonical().map_err(|error| error.to_string())
}

pub(super) fn graphics(
    schema: &RoadSchema,
    game: &Package,
    scripts: &Package,
    graphics: &mut PropertyFile,
) -> Result<Vec<OverlayEntry>, String> {
    let mut entries = Vec::new();
    for (column, future, instance) in [
        (0x0953_2375, false, schema.identity),
        (0x107E_2B61, true, schema.identity ^ 0x8000_0000),
    ] {
        let link = key_mut(graphics, column)?;
        let target = property_id(APPEARANCE, instance);
        if link.group != APPEARANCE
            || instance == 0
            || [game, scripts]
                .iter()
                .any(|package| package.entry(target).is_some())
        {
            return Err("road_appearance_identity_invalid".into());
        }
        let base = property(game, property_id(link.group, link.instance))?;
        let composed = appearance(game, schema.recipe, schema.composition, future, base)?;
        link.instance = instance;
        entries.push(OverlayEntry::new(target, encode(&composed)?));
    }
    if schema.composition.tram {
        // IsReallyIntersection compares this lane configuration by instance.
        // A transition to a vanilla street or avenue has different lanes.
        let slot = graphics
            .values
            .iter_mut()
            .find(|p| p.hash == 0x0EA2_41EA)
            .ok_or("road_tram_lane_configuration_missing")?;
        let Kind::Array(values) = &mut slot.kind else {
            return Err("road_tram_lane_configuration_invalid".into());
        };
        let [Value::Key(key)] = values.as_mut_slice() else {
            return Err("road_tram_lane_configuration_invalid".into());
        };
        key.instance = schema.identity;
    }
    Ok(entries)
}

pub(super) fn simulation(
    schema: &RoadSchema,
    scripts: &Package,
    mut sim: PropertyFile,
    mut layout: PropertyFile,
) -> Result<(Vec<u8>, OverlayEntry), String> {
    // B89F598C is the native light rail + sidewalk layout in a 24 m envelope.
    let tram = property(scripts, property_id(super::SIM, 0xB89F_598C))?;
    let index = row(&tram, 0x0BD8_1A2D, 0xA63D_DC77)?;
    let count = column(&layout, 0x0BD8_1A2D)?.len();
    let donor_count = column(&tram, 0x0BD8_1A2D)?.len();
    for hash in 0x0BD8_1A2D..=0x0BD8_1A31 {
        let source = column(&tram, hash)?;
        let slot = layout
            .values
            .iter_mut()
            .find(|p| p.hash == hash)
            .ok_or("road_lane_table_missing")?;
        let Kind::Array(values) = &mut slot.kind else {
            return Err("road_lane_table_invalid".into());
        };
        if values.len() != count || source.len() != donor_count {
            return Err("road_lane_table_invalid".into());
        }
        values.push(source[index].clone());
    }
    let filters = layout
        .values
        .iter_mut()
        .find(|p| p.hash == 0x0D64_98E8)
        .and_then(|p| match &mut p.kind {
            Kind::Array(values) => Some(values),
            _ => None,
        })
        .ok_or("road_tram_filters_missing")?;
    for filter in column(&tram, 0x0D64_98E8)? {
        if !filters.contains(filter) {
            filters.push(filter.clone());
        }
    }
    let link = key_mut(&mut sim, sc_properties::PARENT_HASH)?;
    link.group = 0x48E0_C400;
    link.instance = schema.identity;
    let layout_id = property_id(link.group, link.instance);
    Ok((
        encode(&sim)?,
        OverlayEntry::new(layout_id, encode(&layout)?),
    ))
}
