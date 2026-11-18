use super::{Streetlights, composition, property, property_id};
use dbpf::Package;
use sc_properties::{Key, Kind, PropertyFile, Value};

const APPEARANCE: u32 = 0x0955_8B7E;
const COMPONENT: u32 = 0x0D94_2043;
const KEYS: u32 = 0xBCC5_3802;
const OFFSET: u32 = 0x0955_882C;
const FUTURE_TAG: u32 = 0x0A49_B278;
const ERROR: &str = "road_streetlight_donor_invalid";
pub(super) const INSET: f32 = 0.5;

pub(super) fn model(future: bool) -> u32 {
    if future { 0xCD4B_0A4F } else { 0x4542_BF83 }
}

pub(super) fn source(game: &Package, future: bool) -> Result<PropertyFile, String> {
    property(
        game,
        property_id(APPEARANCE, if future { 0x66BB_53D0 } else { 0xF890_0236 }),
    )
}

pub(super) fn sides(choice: Streetlights) -> [bool; 2] {
    [
        matches!(choice, Streetlights::Both | Streetlights::Left),
        matches!(choice, Streetlights::Both | Streetlights::Right),
    ]
}

pub(super) fn append(
    target: &mut PropertyFile,
    game: &Package,
    future: bool,
    choice: Streetlights,
    positions: [f32; 2],
) -> Result<(), String> {
    let enabled = sides(choice);
    if enabled == [false; 2] {
        return Ok(());
    }
    let mut source = source(game, future)?;
    if let Some(tag) = target
        .get(FUTURE_TAG)
        .filter(|_| source.get(FUTURE_TAG).is_none())
    {
        let mut tag = tag.clone();
        tag.kind = Kind::Array(vec![
            Value::Key(Key {
                type_id: 0,
                group: 0,
                instance: 0
            });
            composition::column(&source, KEYS)?.len()
        ]);
        source.values.push(tag);
    }
    let offsets = composition::column(&source, OFFSET)?;
    for (side, x) in positions.into_iter().enumerate() {
        if !enabled[side] {
            continue;
        }
        let instance = if future {
            0x1A56_D0D2
        } else {
            [0x810E_C613, 0xF22D_80C4][side]
        };
        let component = property(game, property_id(COMPONENT, instance))?;
        let Some(Value::Key(key)) = component.get(0x0D8C_29C3).and_then(|p| p.scalar()) else {
            return Err(ERROR.into());
        };
        if key.instance != model(future)
            || game.entry(property_id(key.group, key.instance)).is_none()
        {
            return Err(ERROR.into());
        }
        let donor_x = [-11.5, 11.5][side];
        let row = composition::column(&source, KEYS)?
            .iter()
            .zip(offsets)
            .position(|(key, offset)| {
                matches!(key, Value::Key(key) if key.instance == instance)
                    && matches!(offset, Value::Vector3(offset) if offset[0] == donor_x)
            })
            .ok_or(ERROR)?;
        let Value::Vector3([_, y, z]) = offsets[row] else {
            return Err(ERROR.into());
        };
        composition::append_component(target, &source, row, Some([x, y, z]))?;
    }
    Ok(())
}
