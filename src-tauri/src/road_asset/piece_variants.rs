use super::{GRAPHICS, SIM, property, property_id};
use dbpf::Package;
use sc_properties::{PropertyFile, Value};

const ERROR: &str = "road_piece_variant_missing";

pub(super) fn validate(
    surface: &PropertyFile,
    game: &Package,
    scripts: &Package,
) -> Result<(), String> {
    // Retail InitPieceTypeInfos enables each mode only when its native
    // PathEntry resolves. The first key is read from the stock Key array.
    for field in [0x0E09_30E9, 0x0E09_30F1] {
        let Some(Value::Key(piece)) = surface
            .get(field)
            .and_then(|prop| prop.array())
            .and_then(|keys| keys.first())
        else {
            return Err(ERROR.into());
        };
        if piece.instance == 0 || scripts.entry(property_id(SIM, piece.instance)).is_none() {
            return Err(ERROR.into());
        }
        let graphics = property(game, property_id(GRAPHICS, piece.instance)).map_err(|_| ERROR)?;
        let Some(Value::Key(appearance)) = graphics.get(0x0953_2375).and_then(|prop| prop.scalar())
        else {
            return Err(ERROR.into());
        };
        if appearance.instance == 0
            || game
                .entry(property_id(appearance.group, appearance.instance))
                .is_none()
        {
            return Err(ERROR.into());
        }
    }
    Ok(())
}
