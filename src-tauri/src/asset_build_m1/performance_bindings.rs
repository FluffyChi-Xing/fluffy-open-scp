//! Explicit editor-selected performances; the legacy recipe path is unchanged.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Preset {
    id: String,
    vignette_id: [u32; 3],
    definition: [u32; 3],
    sink: [u32; 3],
    anchors: Vec<Anchor>,
}
#[derive(Deserialize)]
struct Anchor {
    id: u32,
}
fn catalog() -> Result<Vec<Preset>, String> {
    serde_json::from_str(include_str!("../../resources/sc5/performance-catalog.json"))
        .map_err(|e| e.to_string())
}
fn selected(scene: &SceneDocument) -> Result<Vec<Preset>, String> {
    let mut catalog = catalog()?;
    scene
        .performance_presets
        .iter()
        .map(|id| {
            let index = catalog
                .iter()
                .position(|p| &p.id == id)
                .ok_or_else(|| format!("[performance] unknown preset {id}"))?;
            Ok(catalog.remove(index))
        })
        .collect()
}
fn key(value: [u32; 3]) -> Key {
    Key {
        type_id: value[0],
        group: value[1],
        instance: value[2],
    }
}
fn expected(scene: &SceneDocument) -> Result<Vec<[Key; 3]>, String> {
    let locations: HashSet<_> = scene
        .units
        .iter()
        .filter_map(|u| match u {
            SceneUnit::Spawner { id, .. } => Some(id.instance),
            _ => None,
        })
        .collect();
    let mut rows: Vec<[Key; 3]> = Vec::new();
    for preset in selected(scene)? {
        if preset.definition[1] != VIGNETTE_GROUP
            || preset.vignette_id[2] == 0
            || preset.sink[2] == 0
        {
            return Err(format!("[performance] invalid native row {}", preset.id));
        }
        for anchor in preset.anchors {
            if !locations.contains(&anchor.id) {
                return Err(format!(
                    "[performance] {} requires anchor {:08X}",
                    preset.id, anchor.id
                ));
            }
        }
        let row = [
            key(preset.vignette_id),
            key(preset.definition),
            key(preset.sink),
        ];
        if let Some(previous) = rows.iter().find(|r| r[0].instance == row[0].instance) {
            if previous != &row {
                return Err(format!(
                    "[performance] conflicting vignette ID {:08X}",
                    row[0].instance
                ));
            }
        } else {
            rows.push(row);
        }
    }
    Ok(rows)
}
pub(super) fn bind(bytes: &[u8], scene: &SceneDocument, rotation: u32) -> Result<Bindings, String> {
    let selected = expected(scene)?;
    let mut file = PropertyFile::parse(bytes).map_err(|e| e.to_string())?;
    for (index, hash) in [VIGNETTE_ID, VIGNETTE_DEFINITION, VIGNETTE_SINK]
        .into_iter()
        .enumerate()
    {
        write_array(
            &mut file,
            hash,
            PropType::Key,
            selected
                .iter()
                .map(|r| Value::Key(r[index].clone()))
                .collect(),
        );
    }
    let positions = authored_locations(scene, rotation)?;
    for (index, hash) in LOCATION_COLUMNS.into_iter().enumerate() {
        write_array(
            &mut file,
            hash,
            if index == 0 {
                PropType::Key
            } else {
                PropType::Transform
            },
            positions
                .iter()
                .map(|(k, t)| {
                    if index == 0 {
                        Value::Key(k.clone())
                    } else {
                        Value::Transform(t.clone())
                    }
                })
                .collect(),
        );
    }
    let bytes = super::super::property_edit::encode(&file)?;
    verify(&bytes, scene)?;
    verify_authored_locations(
        &PropertyFile::parse(&bytes).map_err(|e| e.to_string())?,
        scene,
        rotation,
    )?;
    Ok(Bindings {
        bytes,
        native_definitions: selected
            .iter()
            .map(|r| (r[1].group, r[1].instance))
            .collect(),
        report: serde_json::json!({"mode":"explicit","presets":scene.performance_presets,"vignettes":selected.iter().map(|r|format!("{:08X}",r[1].instance)).collect::<Vec<_>>() }),
    })
}
pub(super) fn verify(bytes: &[u8], scene: &SceneDocument) -> Result<(), String> {
    let file = PropertyFile::parse(bytes).map_err(|e| e.to_string())?;
    if rows(&file)? != expected(scene)? {
        return Err(
            "[performance] final vignette registration differs from selected presets".into(),
        );
    }
    let actual: HashSet<_> = locations(&file)?.iter().map(|(k, _)| k.instance).collect();
    for preset in selected(scene)? {
        for anchor in preset.anchors {
            if !actual.contains(&anchor.id) {
                return Err(format!(
                    "[performance] missing final anchor {:08X}",
                    anchor.id
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn explicit_preset_registers_all_three_columns_and_rejects_missing_anchors() {
        let preset = catalog()
            .unwrap()
            .into_iter()
            .find(|p| p.id.starts_with("college-") && !p.anchors.is_empty())
            .unwrap();
        let units:Vec<_>=preset.anchors.iter().enumerate().map(|(i,a)|json!({"kind":"spawner","index":i,"id":format!("spawner:{i}"),"spawnerId":{"typeId":0,"group":0,"instance":a.id},"performancePresets":if i==0 {vec![preset.id.clone()]} else {vec![]},"transform":{"matrix":[1,0,0,0,1,0,0,0,1,10,20,0]}})).collect();
        let mut scene =
            super::super::super::scene::parse_document(&json!({"units":units})).unwrap();
        let empty = super::super::super::property_edit::encode(&PropertyFile::default()).unwrap();
        let result = bind(&empty, &scene, 180).unwrap();
        verify(&result.bytes, &scene).unwrap();
        let file = PropertyFile::parse(&result.bytes).unwrap();
        assert_eq!(rows(&file).unwrap()[0][1].instance, preset.definition[2]);
        scene.units.pop();
        assert!(bind(&empty, &scene, 0).is_err());
        assert!(verify(&result.bytes, &scene).is_err());
    }
    #[test]
    fn old_document_has_no_explicit_selection() {
        let scene = super::super::super::scene::parse_document(&json!({"units":[]})).unwrap();
        assert!(scene.performance_presets.is_empty());
    }
}
