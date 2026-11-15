//! Recover a missing atlas entry from the material's saved resource signature.
//! SC5 Asset Editor 1.23.7 resources.py::configure_window (lines 39–65)
//! records the style, pack, recipe, layer, rectangle, and room tile.

use super::catalog::{Catalog, Entry};

pub(super) fn recover_window_entry(
    catalog: &Catalog,
    style: &str,
    signature: &str,
) -> Option<Entry> {
    let mut fields = signature.splitn(5, ':');
    if fields.next()? != style {
        return None;
    }
    let pack = fields.next()?;
    let recipe = fields.next()?.parse().ok()?;
    let layer = fields.next()?;
    if !matches!(layer, "上层" | "底层") {
        return None;
    }
    let mut geometry = fields.next()?;
    for revision in ["window-exterior-v1:", "window-exterior-v2-recess:"] {
        if let Some(rest) = geometry.strip_prefix(revision) {
            geometry = rest;
            break;
        }
    }
    if pack == "Office" {
        geometry = geometry
            .strip_suffix(":native-office-states-v3")
            .unwrap_or(geometry);
    }
    let (rect, room_tile) = geometry.split_once(':')?;
    let rect: [f64; 4] = serde_json::from_str(rect).ok()?;
    let [width, height, x, y] = rect;
    if width <= 0.0 || height <= 0.0 || x < 0.0 || y < 0.0 || x + width > 1.0 || y + height > 1.0 {
        return None;
    }
    let room_tile: Option<[f64; 2]> = serde_json::from_str(room_tile).ok()?;
    let entry = Entry {
        id: style.to_string(),
        name: style.to_string(),
        pack: pack.to_string(),
        rect,
        recipe,
        recipe_layer: Some(layer.to_string()),
        size: None,
        rotation: None,
        shade_mean: None,
        room_tile,
    };
    catalog.window_recipe(Some(&entry), pack)?;
    Some(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_missing_window_without_substituting_other_recipe_rows() {
        let catalog = Catalog::parse(
            br#"{"packs":{"FuturisticMain":{"name":"Future","columns":63,
            "rows":[[[0,0,0,0],[0,0,0,0],[0,0,0,0],[0.01953125,0.0234375,0.125,0.375]]]}}}"#,
        )
        .unwrap();
        let style = "FuturisticMain_windows_008";
        let signature = "FuturisticMain_windows_008:FuturisticMain:3:上层:window-exterior-v2-recess:[0.18359375,0.25390625,0.3203125,0.61328125]:null";
        let entry = recover_window_entry(&catalog, style, signature).unwrap();
        assert_eq!(entry.rect, [0.18359375, 0.25390625, 0.3203125, 0.61328125]);
        assert_eq!(entry.recipe_layer.as_deref(), Some("上层"));
        assert_eq!(
            catalog.window_recipe(Some(&entry), &entry.pack),
            Some([0.01953125, 0.0234375, 0.125, 0.375]),
        );
        assert!(recover_window_entry(&catalog, "other", signature).is_none());
        assert!(recover_window_entry(&catalog, style, &signature.replace(":3:", ":4:")).is_none());
        assert!(
            recover_window_entry(&catalog, style, &signature.replace(":null", ":broken")).is_none()
        );
    }
}
