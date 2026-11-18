use super::{
    COMPONENT, KEYS, OFFSET, ROTATION, SCALE, TAGS, composition, mesh, property, property_id,
    set_last,
};
use crate::road_asset::{
    authored::{Band, Layout},
    draft::StripKind,
};
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Key, Kind, PropType, Property, PropertyFile, Value};

// Native intersection matching uses the bounds of each component carrying these tags.
const ROLES: [u32; 4] = [0xE816_F049, 0x4A00_A8C3, 0x9FDD_012C, 0x003E_B432];
const GROUP_TEMPLATE: u32 = 0xD795_5CF6;
const EMPTY_COMPONENT: u32 = 0xC7EE_8594;
const FUTURE_TAG: u32 = 0x0A49_B278;
const ERROR: &str = "road_draft_junction_donor_invalid";

pub(super) fn align_bounds(
    target: &mut PropertyFile,
    donor: &PropertyFile,
    plan: &Layout,
    id: ResourceId,
) -> Result<OverlayEntry, String> {
    let row = composition::row(donor, KEYS, EMPTY_COMPONENT)?;
    let no_tag = Value::Key(Key {
        type_id: 0,
        group: 0,
        instance: 0,
    });
    if let Some(tag) = target
        .get(FUTURE_TAG)
        .filter(|_| donor.get(FUTURE_TAG).is_none())
    {
        // High-density future appearances include one more optional mesh tag.
        let mut donor = donor.clone();
        let mut tag = tag.clone();
        tag.kind = Kind::Array(vec![
            no_tag.clone();
            composition::column(&donor, KEYS)?.len()
        ]);
        donor.values.push(tag);
        composition::append_component(target, &donor, row, None)?;
    } else {
        composition::append_component(target, donor, row, None)?;
    }
    set_last(target, OFFSET, Value::Vector3([0.0; 3]))?;
    set_last(target, SCALE, Value::Vector3([1.0; 3]))?;
    set_last(target, TAGS, no_tag)?;
    set_last(
        target,
        KEYS,
        Value::Key(Key {
            type_id: 0,
            group: id.group,
            instance: id.instance,
        }),
    )?;
    // The default-material pass renders untextured ribbons. A group of two
    // kExtrusionNone components contributes point bounds without making faces.
    // Omit the model column: InitExtrusionComponents otherwise promotes every
    // kExtrusionNone row with that column to kExtrusionModel.
    let mut keys = donor.get(KEYS).ok_or(ERROR)?.clone();
    let empty = composition::column(donor, KEYS)?.get(row).ok_or(ERROR)?;
    keys.kind = Kind::Array(vec![empty.clone(), empty.clone()]);
    let mut offsets = donor.get(OFFSET).ok_or(ERROR)?.clone();
    let half = plan.width * 0.5;
    offsets.kind = Kind::Array(vec![
        Value::Vector3([-half, 0.0, 0.0]),
        Value::Vector3([half, 0.0, 0.0]),
    ]);
    let bounds = PropertyFile {
        claimed_count: 0,
        values: vec![
            Property {
                hash: 0x0D94_2BCE,
                prop_type: PropType::UInt32,
                kind: Kind::Scalar(Value::UInt32(3_944_382_562)),
                encoding: Default::default(),
            },
            keys,
            offsets,
        ],
    };
    Ok(OverlayEntry::new(
        id,
        bounds.encode_canonical().map_err(|e| e.to_string())?,
    ))
}

pub(super) struct Bands {
    first_lane: usize,
    last_lane: usize,
    tables: [PropertyFile; 4],
}

impl Bands {
    pub(super) fn new(game: &Package, plan: &Layout) -> Result<Self, String> {
        let is_lane = |band: &Band| matches!(band.kind, StripKind::Forward | StripKind::Backward);
        let first_lane = plan.bands.iter().position(is_lane).ok_or(ERROR)?;
        let last_lane = plan.bands.iter().rposition(is_lane).ok_or(ERROR)?;
        let mut template = property(game, property_id(COMPONENT, GROUP_TEMPLATE))?;
        if !matches!(
            template.get(0x0D94_2BCE).and_then(|field| field.scalar()),
            Some(Value::UInt32(3_944_382_562))
        ) {
            return Err(ERROR.into());
        }
        for column in &mut template.values {
            if let Kind::Array(rows) = &mut column.kind {
                rows.clear();
            }
        }
        Ok(Self {
            first_lane,
            last_lane,
            tables: std::array::from_fn(|_| template.clone()),
        })
    }

    pub(super) fn slices(
        &self,
        index: usize,
        band: &Band,
        uv: [f32; 2],
    ) -> Vec<(usize, [f32; 2], Band)> {
        let [left, right] = [
            band.position - band.width / 2.0,
            band.position + band.width / 2.0,
        ];
        if (self.first_lane..=self.last_lane).contains(&index) && left < -1e-5 && right > 1e-5 {
            let middle = uv[0] + (uv[1] - uv[0]) * (-left / band.width);
            return vec![
                (
                    0,
                    [uv[0], middle],
                    Band {
                        kind: band.kind,
                        position: left / 2.0,
                        width: -left,
                    },
                ),
                (
                    1,
                    [middle, uv[1]],
                    Band {
                        kind: band.kind,
                        position: right / 2.0,
                        width: right,
                    },
                ),
            ];
        }
        vec![(
            0,
            uv,
            Band {
                kind: band.kind,
                position: band.position,
                width: band.width,
            },
        )]
    }

    pub(super) fn mesh_shoulders(&self, surfaces: &[mesh::Surface]) -> [bool; 4] {
        let compatible = |side: &[mesh::Surface]| {
            side.first()
                .is_some_and(|first| side.iter().all(|s| s.shares_material(first)))
        };
        [
            compatible(&surfaces[..self.first_lane]),
            false,
            false,
            compatible(&surfaces[self.last_lane + 1..]),
        ]
    }

    pub(super) fn table(&mut self, index: usize, position: f32) -> &mut PropertyFile {
        let group = self.group(index, position);
        &mut self.tables[group]
    }

    pub(super) fn group(&self, index: usize, position: f32) -> usize {
        if index < self.first_lane {
            0
        } else if index > self.last_lane {
            3
        } else if position < 0.0 {
            1
        } else {
            2
        }
    }

    pub(super) fn into_tables(self) -> [PropertyFile; 4] {
        self.tables
    }
}

pub(super) fn append_shoulder(
    target: &mut PropertyFile,
    donor: &PropertyFile,
    row: usize,
    group: usize,
    plan: &Layout,
    surfaces: &[mesh::Surface],
    model: ResourceId,
    reserve: &mut impl FnMut(ResourceId) -> Result<(), &'static str>,
) -> Result<[OverlayEntry; 2], String> {
    let is_lane = |band: &Band| matches!(band.kind, StripKind::Forward | StripKind::Backward);
    let range = if group == 0 {
        0..plan.bands.iter().position(is_lane).ok_or(ERROR)?
    } else {
        plan.bands.iter().rposition(is_lane).ok_or(ERROR)? + 1..plan.bands.len()
    };
    let material = surfaces.get(range.start).ok_or(ERROR)?;
    let period = material.period;
    let mut geometry = mesh::SurfaceMesh::new();
    for i in range {
        let band = &plan.bands[i];
        let surface = &surfaces[i];
        if !surface.shares_material(material) {
            return Err(ERROR.into());
        }
        let xs = [
            band.position - band.width / 2.0,
            band.position + band.width / 2.0,
        ];
        geometry.strip([0.0, period].into_iter().map(|y| {
            let mut uv = surface.uv;
            let travel = y / surface.period * (uv[1][1] - uv[0][1]);
            uv[0][1] += travel;
            uv[1][1] = uv[0][1];
            (xs.map(|x| [x, y, 0.0]), uv)
        }));
    }
    // Init's T-junction branch adjusts the selected component's own offset.
    // CreateExtrusionGeometry ignores that offset when the component is a group.
    mesh::append(target, donor, row, model)?;
    set_last(target, OFFSET, Value::Vector3([0.0; 3]))?;
    set_last(target, ROTATION, Value::Vector3([0.0; 3]))?;
    set_last(target, SCALE, Value::Vector3([1.0; 3]))?;
    set_last(target, 0x0955_882D, Value::Bool(true))?;
    set_last(target, 0x0955_882E, Value::Bool(true))?;
    set_last(
        target,
        TAGS,
        Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: ROLES[group],
        }),
    )?;
    let edge = plan.width * if group == 0 { -0.5 } else { 0.5 };
    geometry.encode(
        model,
        material,
        mesh::SweepBounds::ShoulderEdge(edge),
        reserve,
    )
}

pub(super) fn append(
    target: &mut PropertyFile,
    donor: &PropertyFile,
    group: usize,
    row: usize,
    children: PropertyFile,
    id: ResourceId,
) -> Result<OverlayEntry, String> {
    if composition::column(&children, KEYS)?.is_empty() {
        return Err(ERROR.into());
    }
    composition::append_component(target, donor, row, None)?;
    set_last(target, OFFSET, Value::Vector3([0.0; 3]))?;
    set_last(target, ROTATION, Value::Vector3([0.0; 3]))?;
    set_last(target, SCALE, Value::Vector3([1.0; 3]))?;
    set_last(
        target,
        TAGS,
        Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: ROLES[group],
        }),
    )?;
    set_last(
        target,
        KEYS,
        Value::Key(Key {
            type_id: 0,
            group: id.group,
            instance: id.instance,
        }),
    )?;
    Ok(OverlayEntry::new(
        id,
        children
            .encode_canonical()
            .map_err(|error| error.to_string())?,
    ))
}
