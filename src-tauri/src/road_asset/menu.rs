use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Key, Kind, PropertyFile, Value};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::asset_build_m1::menu::{
    ICON_TYPE, MARQUEE_TYPE, MenuRegistrationSpec, build_icon_png, build_marquee_jpg,
};

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Settings {
    pub(super) position: Option<i32>,
    #[serde(default)]
    pub(super) icon_png: String,
    #[serde(default)]
    pub(super) marquee_png: String,
}

pub(super) fn apply(
    settings: &Settings,
    root: &Path,
    id: u32,
    sources: &[&Package],
    menu: &mut PropertyFile,
) -> Result<Vec<OverlayEntry>, String> {
    if let Some(position) = settings.position {
        let count = menu
            .get(0x0DB9_FC63)
            .and_then(|p| p.array())
            .ok_or("road_menu_category_missing")?
            .len();
        let order = menu
            .values
            .iter_mut()
            .find(|p| p.hash == 0x0DC1_E3E0)
            .ok_or("road_menu_order_missing")?;
        match &mut order.kind {
            Kind::Array(values)
                if values.len() == count && values.iter().all(|v| matches!(v, Value::Int32(_))) =>
            {
                // Packager menu_locations.py:65–85 writes one position per category.
                values.fill(Value::Int32(position));
            }
            _ => return Err("road_menu_order_invalid".into()),
        }
    }
    let mut images = Vec::new();
    for (path, field, kind) in [
        (&settings.icon_png, 0x0977_AA8F, ICON_TYPE),
        (&settings.marquee_png, 0x0DDE_EE56, MARQUEE_TYPE),
    ] {
        if path.is_empty() {
            continue;
        }
        crate::mod_flow::safe_path(root, path)?;
        let slot = menu
            .values
            .iter_mut()
            .find(|p| p.hash == field)
            .ok_or("road_menu_artwork_missing")?;
        let Kind::Scalar(Value::Key(key)) = &mut slot.kind else {
            return Err("road_menu_artwork_invalid".into());
        };
        if key.type_id != kind {
            return Err("road_menu_artwork_invalid".into());
        }
        // Keep the road donor's artwork group as read by cToolPathPlacer.
        let target = ResourceId {
            type_id: kind,
            group: key.group,
            instance: id,
        };
        if sources.iter().any(|source| source.entry(target).is_some()) {
            return Err("road_identity_collision".into());
        }
        let spec: MenuRegistrationSpec = serde_json::from_value(serde_json::json!({
            "title": "", "iconPng": path, "marqueePng": path,
        }))
        .map_err(|error| error.to_string())?;
        let bytes = if kind == ICON_TYPE {
            build_icon_png(&spec, id, root)?
        } else {
            build_marquee_jpg(&spec, id, root)?
        };
        *key = Key {
            type_id: kind,
            group: target.group,
            instance: id,
        };
        images.push(OverlayEntry::new(target, bytes));
    }
    Ok(images)
}
