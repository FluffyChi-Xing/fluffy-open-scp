//! Ground vignette rows copied from retail Lots. The game supplies the
//! referenced definitions. Building authors supply their own UnitLocations.

use super::{Recipe, ResolvedBehavior, SceneDocument, SceneUnit};

const WORKPLACE: Recipe = Recipe {
    lot: 0xC8AC_E042,
    snapshot: include_bytes!("../../resources/sc5/spawner-workplace.bin"),
    preferred_slots: &[],
    definitions: &[(
        0x2A29_A2A5,
        &[0x8D4D_2706, 0x8D4D_2705, 0x8D4D_2704, 0x8D4D_2703],
    )],
};

const AMBIENT: Recipe = Recipe {
    lot: 0x8B74_1349,
    snapshot: include_bytes!("../../resources/sc5/spawner-ground-ambient.bin"),
    preferred_slots: &[],
    definitions: &[(0x9A84_297C, &[0x3B8A_DFE0, 0x3B8A_DFE3])],
};

pub(super) fn select(
    program: &ResolvedBehavior,
    scene: Option<&SceneDocument>,
) -> Option<&'static Recipe> {
    if !scene.is_some_and(|scene| {
        scene
            .units
            .iter()
            .any(|unit| matches!(unit, SceneUnit::Spawner { .. }))
    }) {
        return None;
    }
    let has_jobs = [0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7]
        .iter()
        .any(|hash| program.set_float_fields.get(hash).is_some_and(|value| *value > 0.0));
    Some(if has_jobs {
        &WORKPLACE
    } else {
        &AMBIENT
    })
}
