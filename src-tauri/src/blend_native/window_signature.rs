//! Recover a missing atlas entry from the material's saved resource signature.
//! SC5 Asset Editor 1.23.7 resources.py::configure_window (lines 39–65)
//! records the style, pack, recipe, layer, rectangle, and room tile.

use super::catalog::{Catalog, Entry};

pub(super) fn missing_pack<'a>(
    catalog: &Catalog,
    style: &str,
    signature: &'a str,
) -> Option<&'a str> {
    let mut fields = signature.splitn(3, ':');
    if fields.next()? != style {
        return None;
    }
    let pack = fields.next()?;
    fields.next()?;
    (!pack.is_empty() && catalog.pack(pack).is_none()).then_some(pack)
}

pub(super) fn recover_window_entry(
    catalog: &Catalog,
    style: &str,
    signature: &str,
) -> Option<Entry> {
    let entry = parse_window_entry(style, signature)?;
    catalog.window_recipe(Some(&entry), &entry.pack)?;
    Some(entry)
}

pub(super) fn parse_window_entry(style: &str, signature: &str) -> Option<Entry> {
    let mut fields = signature.splitn(3, ':');
    if fields.next()? != style {
        return None;
    }
    let pack = fields.next()?;
    let payload = fields.next()?;
    let (recipe, layer, mut geometry) = if pack == "Commercial"
        && (payload.starts_with('[') || payload.starts_with("window-exterior-v"))
    {
        // resources.py::configure_window writes a short signature for presets.
        // native_asset.py:364–367 exports those presets as recipe 19, top layer.
        (19, "上层", payload)
    } else {
        let mut fields = payload.splitn(3, ':');
        (fields.next()?.parse().ok()?, fields.next()?, fields.next()?)
    };
    if !matches!(layer, "上层" | "底层") {
        return None;
    }
    // This optional marker versions the Editor's exterior preview cache.
    // The atlas rectangle and room tile that follow it retain the same format.
    if let Some(revision) = geometry.strip_prefix("window-exterior-v") {
        let (version, rest) = revision.split_once(':')?;
        let number = version.split('-').next()?;
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        geometry = rest;
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
    Some(Entry {
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_unlisted_commercial_preset_from_short_signature() {
        let mut rows = vec![vec![0.0; 4]; 20];
        rows[19] = vec![0.25, 0.5, 0.125, 0.0];
        let catalog = Catalog::parse(
            &serde_json::to_vec(&serde_json::json!({
                "packs": {"Commercial": {"name":"Commercial", "columns":63, "rows":[rows]}}
            }))
            .unwrap(),
        )
        .unwrap();
        let entry = recover_window_entry(
            &catalog,
            "NEW_PRESET",
            "NEW_PRESET:Commercial:window-exterior-v7:[0.25,0.125,0.5,0.5]:null",
        )
        .unwrap();
        assert_eq!(entry.recipe, 19);
        assert_eq!(entry.recipe_layer.as_deref(), Some("上层"));
        assert_eq!(entry.rect, [0.25, 0.125, 0.5, 0.5]);
    }

    #[test]
    fn recovers_missing_window_without_substituting_other_recipe_rows() {
        let catalog = Catalog::parse(
            br#"{"packs":{"FuturisticMain":{"name":"Future","columns":63,
            "rows":[[[0,0,0,0],[0,0,0,0],[0,0,0,0],[0.01953125,0.0234375,0.125,0.375]]]}}}"#,
        )
        .unwrap();
        let style = "FuturisticMain_windows_008";
        let signature = "FuturisticMain_windows_008:FuturisticMain:3:上层:window-exterior-v3-guarded:[0.18359375,0.25390625,0.3203125,0.61328125]:null";
        let entry = recover_window_entry(&catalog, style, signature).unwrap();
        assert_eq!(entry.rect, [0.18359375, 0.25390625, 0.3203125, 0.61328125]);
        assert_eq!(entry.recipe_layer.as_deref(), Some("上层"));
        assert_eq!(
            catalog.window_recipe(Some(&entry), &entry.pack),
            Some([0.01953125, 0.0234375, 0.125, 0.375]),
        );
        for revision in [
            "",
            "window-exterior-v1:",
            "window-exterior-v2-recess:",
            "window-exterior-v12-updated:",
        ] {
            let other = signature.replace("window-exterior-v3-guarded:", revision);
            let other = recover_window_entry(&catalog, style, &other).unwrap();
            assert_eq!(other.rect, entry.rect);
            assert_eq!(other.recipe_layer, entry.recipe_layer);
            assert_eq!(
                catalog.window_recipe(Some(&other), &other.pack),
                catalog.window_recipe(Some(&entry), &entry.pack),
            );
        }
        assert!(recover_window_entry(&catalog, "other", signature).is_none());
        assert!(recover_window_entry(&catalog, style, &signature.replace(":3:", ":4:")).is_none());
        assert!(
            recover_window_entry(
                &catalog,
                style,
                &signature.replace(":FuturisticMain:", ":MissingPack:")
            )
            .is_none()
        );
        assert!(
            recover_window_entry(&catalog, style, &signature.replace(":上层:", ":unknown:"))
                .is_none()
        );
        assert!(
            recover_window_entry(&catalog, style, &signature.replace("0.18359375", "1.0"))
                .is_none()
        );
        assert!(
            recover_window_entry(&catalog, style, &signature.replace(":null", ":broken")).is_none()
        );
    }
}
