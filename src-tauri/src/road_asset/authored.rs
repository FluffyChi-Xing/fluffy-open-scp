use super::{
    Recipe, RoadSchema, composition,
    draft::{Strip, StripKind},
};
use dbpf::OverlayEntry;
use sc_properties::{Kind, PropertyFile, Value};

pub(super) struct Band {
    pub kind: StripKind,
    pub position: f32,
    pub width: f32,
}

pub(super) struct Layout {
    pub bands: Vec<Band>,
    pub lanes: i32,
    pub lane_width: f32,
    pub lane_offset: f32,
    pub walk_width: f32,
    pub walk_offset: f32,
    pub width: f32,
}

fn lane(kind: StripKind) -> bool {
    matches!(kind, StripKind::Forward | StripKind::Backward)
}

pub(super) fn plan(section: &[Strip], recipe: Recipe) -> Result<Layout, String> {
    let unsupported = "road_draft_layout_unsupported";
    if matches!(recipe, Recipe::Dirt | Recipe::Streetcar)
        || section.iter().any(|strip| strip.kind == StripKind::Rail)
        || section
            .first()
            .is_none_or(|strip| strip.kind != StripKind::Walk)
        || section
            .last()
            .is_none_or(|strip| strip.kind != StripKind::Walk)
        || section
            .iter()
            .filter(|strip| strip.kind == StripKind::Walk)
            .count()
            != 2
    {
        return Err(unsupported.into());
    }
    let first = section
        .iter()
        .position(|strip| lane(strip.kind))
        .ok_or(unsupported)?;
    let left_count = section[first..]
        .iter()
        .take_while(|strip| strip.kind == section[first].kind)
        .count();
    let right = first
        + left_count
        + section[first + left_count..]
            .iter()
            .take_while(|strip| !lane(strip.kind))
            .count();
    if right >= section.len() || section[right].kind == section[first].kind {
        return Err(unsupported.into());
    }
    let right_count = section[right..]
        .iter()
        .take_while(|strip| strip.kind == section[right].kind)
        .count();
    let lane_width = section[first].width;
    if left_count != right_count
        || left_count > 3
        || section[right + right_count..]
            .iter()
            .any(|strip| lane(strip.kind))
        || section
            .iter()
            .filter(|strip| lane(strip.kind))
            .any(|strip| (strip.width - lane_width).abs() > 1e-8)
    {
        return Err(unsupported.into());
    }
    let mut length = 0.0;
    let edges: Vec<_> = section
        .iter()
        .map(|strip| {
            let start = length;
            length += strip.width;
            start
        })
        .collect();
    let lane_offset = (edges[right] - edges[first + left_count]) / 2.0;
    let axis = edges[right] - lane_offset;
    let right_walk_start = *edges.last().ok_or(unsupported)? - axis;
    let walk_offset = (axis - section[0].width).max(right_walk_start);
    let walk_width = axis.min(length - axis) - walk_offset;
    let width = 2.0 * axis.max(length - axis);
    if walk_width <= 0.0 || !width.is_finite() || width as f32 > f32::MAX {
        return Err(unsupported.into());
    }
    Ok(Layout {
        bands: section
            .iter()
            .zip(edges)
            .map(|(strip, start)| Band {
                kind: strip.kind,
                position: (start + strip.width / 2.0 - axis) as f32,
                width: strip.width as f32,
            })
            .collect(),
        lanes: left_count as i32,
        lane_width: lane_width as f32,
        lane_offset: lane_offset as f32,
        walk_width: walk_width as f32,
        walk_offset: walk_offset as f32,
        width: width as f32,
    })
}

pub(super) fn simulation(
    schema: &RoadSchema,
    plan: &Layout,
    mut sim: PropertyFile,
    mut layout: PropertyFile,
) -> Result<(Vec<u8>, OverlayEntry), String> {
    let handlers = composition::column(&layout, 0x0BD8_1A2D)?;
    let numbers = [0xCAD3_7BD0, 0x54AB_DF8C, 0xE756_5405]
        .map(|id| composition::row(&layout, 0x0BD8_1A2D, id));
    let [worker, car, walk] = numbers;
    let indices = [worker?, car?, walk?];
    if handlers.len() != indices.len() {
        return Err("road_draft_lane_donor_invalid".into());
    }
    for (hash, data) in [
        (
            0x0BD8_1A2E,
            [
                Value::Int32(plan.lanes),
                Value::Int32(plan.lanes),
                Value::Int32(1),
            ],
        ),
        (
            0x0BD8_1A2F,
            [
                Value::Float(plan.lane_width),
                Value::Float(plan.lane_width),
                Value::Float(plan.walk_width),
            ],
        ),
        (
            0x0BD8_1A31,
            [
                Value::Float(plan.lane_offset),
                Value::Float(plan.lane_offset),
                Value::Float(plan.walk_offset),
            ],
        ),
    ] {
        let field = layout
            .values
            .iter_mut()
            .find(|field| field.hash == hash)
            .ok_or("road_lane_table_missing")?;
        let Kind::Array(rows) = &mut field.kind else {
            return Err("road_lane_table_invalid".into());
        };
        if rows.len() != indices.len() {
            return Err("road_lane_table_invalid".into());
        }
        for (index, value) in indices.into_iter().zip(data) {
            rows[index] = value;
        }
    }
    let field = layout
        .values
        .iter_mut()
        .find(|field| field.hash == 0x09E6_794C)
        .ok_or("road_draft_width_missing")?;
    if !matches!(field.kind, Kind::Scalar(Value::Float(_))) {
        return Err("road_draft_width_missing".into());
    }
    field.kind = Kind::Scalar(Value::Float(plan.width));
    // InitPieceTypeInfos uses these links to switch to native bridge/tunnel
    // pieces. Those pieces have independent widths and textures.
    sim.values
        .retain(|field| !matches!(field.hash, 0x0E09_30E9 | 0x0E09_30F1));
    let link = composition::key_mut(&mut sim, sc_properties::PARENT_HASH)?;
    link.group = 0x48E0_C400;
    link.instance = schema.identity;
    let id = super::property_id(link.group, link.instance);
    Ok((
        sim.encode_canonical().map_err(|e| e.to_string())?,
        OverlayEntry::new(id, layout.encode_canonical().map_err(|e| e.to_string())?),
    ))
}
