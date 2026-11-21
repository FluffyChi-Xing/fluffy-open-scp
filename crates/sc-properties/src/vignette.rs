//! Scene actor tables read by SC::cVignetteGame::FillEntryFromProps (0x70A7F0).
//! Preserve raw values; actor identifiers are local IDs, not resource TGIs.
use crate::{Kind, PropertyFile, Value};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub type_id: u32,
    pub group_id: u32,
    pub instance_id: u32,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub id: u32,
    pub models: Vec<Reference>,
    pub templates: Vec<Reference>,
    pub appearances: Vec<Appearance>,
    pub speed: Option<f32>,
    pub scale: Option<f32>,
    pub animation: Option<Reference>,
    pub spawn: Option<Reference>,
    pub destroy: Option<Reference>,
    pub resource_bind: Option<Reference>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub body: Option<i32>,
    pub skin: Option<i32>,
    pub skin_max: Option<i32>,
    pub outfit: Option<i32>,
    pub outfit_max: Option<i32>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnIssue {
    pub property: u32,
    pub expected: usize,
    pub actual: usize,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub actors: Vec<Actor>,
    pub self_destruct: Option<bool>,
    pub recognized: bool,
    pub issues: Vec<ColumnIssue>,
}
fn values(file: &PropertyFile, hash: u32) -> Vec<&Value> {
    file.values
        .iter()
        .filter(|p| p.hash == hash)
        .flat_map(|p| match &p.kind {
            Kind::Array(v) => v.iter().collect(),
            Kind::Scalar(v) => vec![v],
            Kind::Empty => vec![],
        })
        .collect()
}
fn reference(v: &Value) -> Option<Reference> {
    if let Value::Key(k) = v {
        Some(Reference {
            type_id: k.type_id,
            group_id: k.group,
            instance_id: k.instance,
        })
    } else {
        None
    }
}
fn key(file: &PropertyFile, hash: u32, i: usize) -> Option<Reference> {
    values(file, hash).get(i).and_then(|v| reference(v))
}
fn int(file: &PropertyFile, hash: u32, i: usize) -> Option<i32> {
    match values(file, hash).get(i)? {
        Value::Int32(v) => Some(*v),
        _ => None,
    }
}
fn float(file: &PropertyFile, hash: u32, i: usize) -> Option<f32> {
    match values(file, hash).get(i)? {
        Value::Float(v) if v.is_finite() => Some(*v),
        _ => None,
    }
}
/// The three tables join through mActorEntries[actorID], never by global row index.
/// Source lines 444–510 (actors), 585–700 (palettes), 702ff (improvisation).
pub fn parse(file: &PropertyFile) -> Scene {
    let mut scene = Scene {
        actors: vec![],
        self_destruct: values(file, 0x0D6DBF4A).first().and_then(|v| {
            if let Value::Bool(b) = v {
                Some(*b)
            } else {
                None
            }
        }),
        recognized: false,
        issues: vec![],
    };
    for (ids, columns) in [
        (
            0x0D649949,
            vec![
                0x0D2E8A61, 0x0D6F3BE1, 0x0D37C853, 0x0D560650, 0x0D802215, 0x0D9E4D6E, 0x0DC0A590,
            ],
        ),
        (
            0x0D9BDA82,
            vec![
                0x0DB5452F, 0x0D2E8A61, 0x0D6F3BE1, 0x0D9BDA86, 0x0D9BDA83, 0x0D9BDA84, 0x0D9BDA87,
                0x0D9BDA88,
            ],
        ),
        (0x0D649C45, vec![]),
    ] {
        let rows = values(file, ids);
        if rows.is_empty() {
            continue;
        }
        scene.recognized = true;
        for hash in columns {
            let actual = values(file, hash).len();
            if actual > 0 && actual != rows.len() {
                scene.issues.push(ColumnIssue {
                    property: hash,
                    expected: rows.len(),
                    actual,
                });
            }
        }
        for (i, row) in rows.iter().enumerate() {
            let Some(id) = reference(row).map(|k| k.instance_id) else {
                scene.issues.push(ColumnIssue {
                    property: ids,
                    expected: rows.len(),
                    actual: i,
                });
                continue;
            };
            let index = scene
                .actors
                .iter()
                .position(|a| a.id == id)
                .unwrap_or_else(|| {
                    scene.actors.push(Actor {
                        id,
                        models: vec![],
                        templates: vec![],
                        appearances: vec![],
                        speed: None,
                        scale: None,
                        animation: None,
                        spawn: None,
                        destroy: None,
                        resource_bind: None,
                    });
                    scene.actors.len() - 1
                });
            let actor = &mut scene.actors[index];
            if ids == 0x0D9BDA82 {
                if let Some(k) = key(file, 0x0D2E8A61, i) {
                    actor.models.push(k);
                }
                if let Some(k) = key(file, 0x0DB5452F, i) {
                    actor.templates.push(k);
                }
                actor.appearances.push(Appearance {
                    body: int(file, 0x0D9BDA86, i),
                    skin: int(file, 0x0D9BDA83, i),
                    skin_max: int(file, 0x0D9BDA84, i),
                    outfit: int(file, 0x0D9BDA87, i),
                    outfit_max: int(file, 0x0D9BDA88, i),
                });
                actor.scale = float(file, 0x0D6F3BE1, i).or(actor.scale);
            } else if ids == 0x0D649949 {
                if let Some(k) = key(file, 0x0D2E8A61, i) {
                    actor.models.push(k);
                }
                actor.speed = float(file, 0x0D560650, i);
                actor.scale = float(file, 0x0D6F3BE1, i);
                actor.animation = key(file, 0x0D37C853, i);
                actor.spawn = key(file, 0x0D802215, i);
                actor.destroy = key(file, 0x0D9E4D6E, i);
                actor.resource_bind = key(file, 0x0DC0A590, i);
            }
        }
    }
    scene
}
/// Evidence only: no assumption that a C600 property owns a renderable model.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportDetails {
    pub effects: Vec<Reference>,
    pub sinks: Vec<Reference>,
    pub graphics: Vec<Reference>,
}
pub fn transport_details(file: &PropertyFile) -> TransportDetails {
    let refs = |hash| {
        values(file, hash)
            .iter()
            .filter_map(|v| reference(v))
            .collect()
    };
    TransportDetails {
        effects: refs(0x0BEE4AB0),
        sinks: refs(0x0D9A718B),
        graphics: refs(0x0D8C29C3),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, PropType, Property};
    fn col(hash: u32, prop_type: PropType, vals: Vec<Value>) -> Property {
        Property {
            hash,
            prop_type,
            kind: Kind::Array(vals),
            encoding: Default::default(),
        }
    }
    fn k(id: u32) -> Value {
        Value::Key(Key {
            instance: id,
            type_id: 0,
            group: 0,
        })
    }
    #[test]
    fn joins_reordered_actor_tables_and_preserves_negative_ranges() {
        let file = PropertyFile {
            claimed_count: 4,
            values: vec![
                col(0x0D649949, PropType::Key, vec![k(1), k(2)]),
                col(
                    0x0D560650,
                    PropType::Float,
                    vec![Value::Float(5.), Value::Float(9.)],
                ),
                col(0x0D9BDA82, PropType::Key, vec![k(2), k(1)]),
                col(
                    0x0D9BDA83,
                    PropType::Int32,
                    vec![Value::Int32(-1), Value::Int32(3)],
                ),
            ],
        };
        let parsed = parse(&file);
        assert_eq!(parsed.actors.len(), 2);
        assert_eq!(parsed.actors[0].speed, Some(5.));
        assert_eq!(parsed.actors[0].appearances[0].skin, Some(3));
        assert_eq!(parsed.actors[1].appearances[0].skin, Some(-1));
        assert!(parsed.issues.is_empty());
    }
    #[test]
    fn incomplete_columns_are_reported_without_losing_actors() {
        let file = PropertyFile {
            claimed_count: 2,
            values: vec![
                col(0x0D9BDA82, PropType::Key, vec![k(1), k(2)]),
                col(0x0D9BDA83, PropType::Int32, vec![Value::Int32(-1)]),
            ],
        };
        let parsed = parse(&file);
        assert_eq!(parsed.issues[0].expected, 2);
        assert_eq!(parsed.issues[0].actual, 1);
        assert_eq!(parsed.actors[1].appearances[0].skin, None);
    }
}
