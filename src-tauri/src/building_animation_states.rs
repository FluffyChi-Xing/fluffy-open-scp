use dbpf::ResourceId;
use sc_properties::{PropertyFile, Value};

const GRAPHICS_STATES: u32 = 0x0CA5_CF4A;
const MODEL_LODS: [u32; 4] = [0x00F9_EFBB, 0x00F9_EFBC, 0x00F9_EFBD, 0x00F9_EFBE];

/// A graphics variant may replace the model key but omit the choreography
/// table. Recover it only from an explicit sibling state that references the
/// same model resource.
pub(crate) fn matching_animation_state(
    props: &PropertyFile,
    current: ResourceId,
    model_hash: u32,
    resolve: impl Fn(ResourceId) -> Option<PropertyFile>,
) -> Option<PropertyFile> {
    let model_key = props.get(model_hash)?.scalar()?;
    let states = props.get(GRAPHICS_STATES)?.array()?;
    for value in states {
        let Value::Key(key) = value else { continue };
        let id = ResourceId {
            type_id: current.type_id,
            group: key.group,
            instance: key.instance,
        };
        if id == current || key.instance == 0 {
            continue;
        }
        let Some(state) = resolve(id) else { continue };
        if state.get(0x0DA7_CAD5).is_none() {
            continue;
        }
        if MODEL_LODS
            .iter()
            .any(|hash| state.get(*hash).and_then(|p| p.scalar()) == Some(model_key))
        {
            return Some(state);
        }
    }
    None
}
