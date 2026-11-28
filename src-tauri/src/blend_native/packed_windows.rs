//! Recover the five bindings and effective recipe saved in an Editor window.
//! See Editor resources.py::configure_window and native_asset.py::recipe.

use std::collections::HashMap;
use std::io::Cursor;

use base64::{Engine, engine::general_purpose::STANDARD};

use super::catalog::{Catalog, Entry, Pack};
use super::dump::{Bundle, PackedWindow};
use super::resources::PackStore;
use super::window_signature::parse_window_entry;

pub(super) fn recover(
    bundle: &Bundle,
    catalog: &mut Catalog,
    store: PackStore,
) -> Result<(HashMap<String, Entry>, PackStore), String> {
    let index = catalog.index();
    let mut entries = HashMap::new();
    let mut images = HashMap::new();
    let mut sources: Vec<(&PackedWindow, String)> = Vec::new();
    for material in &bundle.materials {
        let Some(source) = bundle.packed_windows.get(&material.name) else {
            continue;
        };
        let Some(entry) = material
            .prop_str("ny_resource_signature")
            .and_then(|signature| parse_window_entry(signature.split_once(':')?.0, signature))
        else {
            continue;
        };
        if index.contains(&entry.id) || catalog.pack(&entry.pack).is_some() {
            continue;
        }
        let [w, h, x, y] = source.rect;
        let [_, _, room_size, room_offset] = source.row;
        if w <= 0.0
            || h <= 0.0
            || x < 0.0
            || y < 0.0
            || w + x > 1.0
            || h + y > 1.0
            || room_size <= 0.0
            || room_offset < 0.0
            || room_size + room_offset > 1.0
        {
            return Err(format!("invalid packed window parameters for {}", entry.id));
        }
        let pack = if let Some((_, pack)) = sources.iter().find(|(saved, _)| *saved == source) {
            pack.clone()
        } else {
            let mut number = sources.len();
            let pack = loop {
                let key = format!("__openscp_packed_window_{number}");
                if !catalog.packs.contains_key(&key) {
                    break key;
                }
                number += 1;
            };
            let mut exterior = None;
            for (index, name) in source.images.iter().enumerate() {
                let bytes = bundle
                    .packed_window_images
                    .get(name)
                    .and_then(|encoded| STANDARD.decode(encoded).ok())
                    .ok_or_else(|| format!("missing packed window image {name}"))?;
                let dimensions =
                    image::ImageReader::with_format(Cursor::new(&bytes), image::ImageFormat::Png)
                        .into_dimensions()
                        .map_err(|error| format!("packed window image {name}: {error}"))?;
                if (index < 3 && dimensions != *exterior.get_or_insert(dimensions))
                    || (index == 3 && dimensions != (512, 16))
                {
                    return Err(format!(
                        "incompatible packed window image {name}: {dimensions:?}"
                    ));
                }
                images.insert((pack.clone(), (index + 1) as u8), bytes);
            }
            // The Editor's window node has already selected the palette layer
            // and room tile in row 3, including any room-tile override.
            catalog.packs.insert(
                pack.clone(),
                Pack {
                    name: entry.pack.clone(),
                    columns: 1,
                    rows: vec![vec![source.row.to_vec()]],
                    palette_row: source.palette_row,
                    spec_mode: 0.0,
                },
            );
            sources.push((source, pack.clone()));
            pack
        };
        entries.insert(
            material.name.clone(),
            Entry {
                pack,
                rect: source.rect,
                recipe: 0,
                recipe_layer: None,
                room_tile: None,
                ..entry
            },
        );
    }
    Ok((entries, store.with_additional_packs(images)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_native::resources::decode_png;
    use serde_json::json;

    #[test]
    fn restores_effective_material_bindings_and_keeps_installed_pack_precedence() {
        let slots = ["tint", "normal", "shader", "palette", "room"];
        let mut images = HashMap::new();
        for (index, name) in slots.iter().enumerate() {
            let (width, height) = if index == 3 { (512, 16) } else { (16, 16) };
            let image = image::RgbaImage::from_pixel(
                width,
                height,
                image::Rgba([24 + index as u8, 3, 2, 1]),
            );
            let mut png = Cursor::new(Vec::new());
            image.write_to(&mut png, image::ImageFormat::Png).unwrap();
            images.insert(name, STANDARD.encode(png.into_inner()));
        }
        let data = json!({
            "version": 1, "unit": 1,
            "materials": [{"name": "Window", "props": {
                "ny_resource_signature": "style:Extension:42:上层:window-exterior-v3-guarded:[0.5,0.5,0,0]:null"
            }}],
            "packed_windows": {"Window": {
                "images": slots, "rect": [0.25,0.25,0.125,0.125],
                "row": [0.25,0.25,0.125,0.5], "palette_row": 0.125
            }},
            "packed_window_images": images
        });
        let bundle = Bundle::parse(&serde_json::to_vec(&data).unwrap()).unwrap();
        let mut catalog = Catalog::parse(b"{}").unwrap();
        let (entries, store) = recover(&bundle, &mut catalog, PackStore::new(".")).unwrap();
        let entry = &entries["Window"];
        assert_eq!(entry.rect, [0.25, 0.25, 0.125, 0.125]);
        assert_eq!(
            catalog.window_recipe(Some(entry), ""),
            Some([0.25, 0.25, 0.125, 0.5])
        );
        for (index, name) in slots.iter().enumerate() {
            let expected = decode_png(&STANDARD.decode(&images[name]).unwrap(), false).unwrap();
            let actual = store.load(&entry.pack, (index + 1) as u8).unwrap();
            assert_eq!(actual.pixels, expected.pixels);
        }

        let mut installed =
            Catalog::parse(br#"{"packs":{"Extension":{"name":"Installed","columns":63}}}"#)
                .unwrap();
        let (entries, _) = recover(&bundle, &mut installed, PackStore::new(".")).unwrap();
        assert!(entries.is_empty());
    }
}
