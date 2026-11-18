use super::{END_CAP, OFFSET, SCALE, START_CAP, SWEEP_OFFSET, TAGS, composition, mesh, set_last};
use crate::road_asset::{authored::Layout, draft::StripKind};
use dbpf::{OverlayEntry, ResourceId};
use sc_properties::{PropertyFile, Value};

const ERROR: &str = "road_draft_end_cap_missing";
const STEPS: usize = 24;

type Edges = [f32; 2];

fn arc(edges: Edges, sin: f32, cos: f32, height: f32) -> [f32; 3] {
    let radius = (edges[1] - edges[0]) / 2.0;
    [
        (edges[0] + edges[1]) / 2.0 - radius * cos,
        radius * sin,
        height,
    ]
}

fn ring(
    outer: Edges,
    inner: Edges,
    surface: &mesh::Surface,
    height: f32,
    sidewalk: bool,
) -> mesh::SurfaceMesh {
    let mut geometry = mesh::SurfaceMesh::new();
    let outer_width = outer[1] - outer[0];
    let inner_width = inner[1] - inner[0];
    let sidewalk_repeats = ((outer_width + inner_width) * std::f32::consts::FRAC_PI_4
        / surface.period)
        .round()
        .max(1.0);
    geometry.strip((0..=STEPS).map(|step| {
        let fraction = step as f32 / STEPS as f32;
        let (sin, cos) = match step {
            0 => (0.0, 1.0),
            STEPS => (0.0, -1.0),
            _ => (fraction * std::f32::consts::PI).sin_cos(),
        };
        let mut uvs = surface.uv;
        if sidewalk {
            // Both edges at the same angle must sample the same row of tiles.
            // Native cap D112341B pairs them this way from seam to seam.
            let along = fraction * sidewalk_repeats;
            uvs[0][1] = along;
            uvs[1][1] = along;
        } else {
            for uv in &mut uvs {
                uv[1] += fraction * outer_width * std::f32::consts::FRAC_PI_2 / surface.period;
            }
        }
        (
            [arc(outer, sin, cos, height), arc(inner, sin, cos, height)],
            uvs,
        )
    }));
    geometry
}

pub(super) fn append(
    target: &mut PropertyFile,
    donor: &PropertyFile,
    plan: &Layout,
    surfaces: &[mesh::Surface],
    appearance: u32,
    required: bool,
    reserve: &mut impl FnMut(ResourceId) -> Result<(), &'static str>,
) -> Result<Vec<OverlayEntry>, String> {
    let mut rows = [None; 2];
    let offsets = composition::column(donor, SWEEP_OFFSET)?;
    for (index, tag) in composition::column(donor, TAGS)?.iter().enumerate() {
        let Value::Key(tag) = tag else {
            return Err(ERROR.into());
        };
        let side = match tag.instance {
            START_CAP => 0,
            END_CAP => 1,
            _ => continue,
        };
        let Some(Value::Float(offset)) = offsets.get(index) else {
            return Err(ERROR.into());
        };
        if rows[side].is_none_or(|(_, best): (usize, f32)| best.abs() < offset.abs()) {
            rows[side] = Some((index, *offset));
        }
    }
    if !required && rows == [None, None] {
        return Ok(Vec::new());
    }
    let [Some(start), Some(end)] = rows else {
        return Err(ERROR.into());
    };
    let lane = |kind| matches!(kind, StripKind::Forward | StripKind::Backward);
    let first = plan
        .bands
        .iter()
        .position(|band| lane(band.kind))
        .ok_or(ERROR)?;
    let last = plan
        .bands
        .iter()
        .rposition(|band| lane(band.kind))
        .ok_or(ERROR)?;
    let [left, .., right] = plan.bands.as_slice() else {
        return Err(ERROR.into());
    };
    let outside = [
        left.position - left.width / 2.0,
        right.position + right.width / 2.0,
    ];
    let mut verge = [
        left.position + left.width / 2.0,
        right.position - right.width / 2.0,
    ];
    let paved = [
        plan.bands[first].position - plan.bands[first].width / 2.0,
        plan.bands[last].position + plan.bands[last].width / 2.0,
    ];
    let depth = (outside[1] - outside[0]) / 2.0;
    // A sidewalk can border a lane directly. Deriving adjacent edges from
    // different strip centers may introduce a rounding difference.
    if (verge[0] - paved[0]).abs() < 1e-5 {
        verge[0] = paved[0];
    }
    if (verge[1] - paved[1]).abs() < 1e-5 {
        verge[1] = paved[1];
    }
    if !(depth > 0.0
        && outside[0] < verge[0]
        && verge[0] <= paved[0]
        && paved[0] < paved[1]
        && paved[1] <= verge[1]
        && verge[1] < outside[1])
    {
        return Err(ERROR.into());
    }
    let mut parts = vec![(outside, verge, &surfaces[0], 0.0)];
    if let Some(i) = (1..first)
        .next()
        .or_else(|| (last + 1..surfaces.len() - 1).next())
    {
        parts.push((verge, paved, &surfaces[i], 0.0));
    }
    let middle = (paved[0] + paved[1]) / 2.0;
    parts.push((paved, [middle; 2], &surfaces[first], 0.0));
    if plan.lane_offset > 0.0 {
        let median = first + plan.lanes as usize;
        parts.push((
            [-plan.lane_offset, plan.lane_offset],
            [0.0; 2],
            &surfaces[median],
            0.002,
        ));
    }
    let mut entries = Vec::new();
    for (index, (outer, inner, material, height)) in parts.into_iter().enumerate() {
        let id = mesh::id(appearance, "end-cap", index);
        entries.extend(ring(outer, inner, material, height, index == 0).encode(
            id,
            material,
            mesh::SweepBounds::EndCapDepth(depth),
            reserve,
        )?);
        for (row, sweep) in [start, end] {
            let Some(Value::Vector3([_, sy, sz])) = composition::column(donor, SCALE)?.get(row)
            else {
                return Err(ERROR.into());
            };
            let Some(Value::Vector3([_, y, z])) = composition::column(donor, OFFSET)?.get(row)
            else {
                return Err(ERROR.into());
            };
            if *sy == 0.0 || *y == 0.0 || sweep == 0.0 {
                return Err(ERROR.into());
            }
            mesh::append(target, donor, row, id)?;
            set_last(
                target,
                OFFSET,
                Value::Vector3([0.0, y.signum() * depth, *z]),
            )?;
            set_last(target, SCALE, Value::Vector3([1.0, sy.signum(), *sz]))?;
            set_last(target, SWEEP_OFFSET, Value::Float(sweep.signum() * depth))?;
            set_last(target, 0x0955_882D, Value::Bool(false))?;
            set_last(target, 0x0955_882E, Value::Bool(false))?;
        }
    }
    Ok(entries)
}
