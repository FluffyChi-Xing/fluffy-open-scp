use super::{
    COMPONENT, KEYS, OFFSET, ROTATION, SCALE, TAGS, composition, property, property_id, set_last,
};
use crate::road_asset::{
    authored::{Band, Layout},
    draft::StripKind,
};
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Key, Kind, PropertyFile, Value};

// Native intersection matching uses the bounds of each component carrying these tags.
const ROLES: [u32; 4] = [0xE816_F049, 0x4A00_A8C3, 0x9FDD_012C, 0x003E_B432];
const GROUP_TEMPLATE: u32 = 0xD795_5CF6;
const EMPTY_COMPONENT: u32 = 0xC7EE_8594;
const FUTURE_TAG: u32 = 0x0A49_B278;
const ERROR: &str = "road_draft_junction_donor_invalid";

pub(super) fn align_bounds(
    target: &mut PropertyFile,
    donor: &PropertyFile,
    game: &Package,
    plan: &Layout,
) -> Result<(), String> {
    let [left, .., right] = plan.bands.as_slice() else {
        return Err(ERROR.into());
    };
    let left_edge = left.position - left.width * 0.5;
    let right_edge = right.position + right.width * 0.5;
    let half_width = plan.width * 0.5;
    if (left_edge + right_edge).abs() < 0.001 {
        return Ok(());
    }
    let row = composition::row(donor, KEYS, EMPTY_COMPONENT)?;
    let empty = property(game, property_id(COMPONENT, EMPTY_COMPONENT))?;
    if !matches!(
        empty.get(0x0D94_2BCE).and_then(|field| field.scalar()),
        Some(Value::UInt32(3_354_297_748))
    ) {
        return Err(ERROR.into());
    }
    // The engine places both junction edges at half the calculated width.
    // Its empty component contributes a point to those bounds without drawing.
    let narrow_edge = if -left_edge > right_edge {
        half_width
    } else {
        -half_width
    };
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
    set_last(target, OFFSET, Value::Vector3([narrow_edge, 0.0, 0.0]))?;
    set_last(target, TAGS, no_tag)
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

    pub(super) fn table(&mut self, index: usize, position: f32) -> &mut PropertyFile {
        let group = if index < self.first_lane {
            0
        } else if index > self.last_lane {
            3
        } else if position < 0.0 {
            1
        } else {
            2
        };
        &mut self.tables[group]
    }

    pub(super) fn into_tables(self) -> [PropertyFile; 4] {
        self.tables
    }
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
