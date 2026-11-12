//! Source-bound atlas catalog (port of `resources.CATALOG`).
//!
//! The catalog is data, not code: pack palettes, atlas rectangles and recipe
//! rows are loaded from `catalog.json`.  No Blender dependency.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub packs: BTreeMap<String, Pack>,
    #[serde(default)]
    pub windows: Vec<Entry>,
    #[serde(default)]
    pub decor: Vec<Entry>,
    #[serde(default)]
    pub surfaces: Vec<Entry>,
    #[serde(default)]
    pub modules: Vec<Entry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pack {
    pub name: String,
    pub columns: u32,
    /// `rows[0][recipe]` is a `[u, v, palette_offset, room_offset]` quad.
    #[serde(default)]
    pub rows: Vec<Vec<Vec<f64>>>,
    #[serde(default)]
    pub palette_row: f64,
    #[serde(default)]
    pub spec_mode: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub pack: String,
    /// `[width, height, x, y]` in atlas UV space.
    pub rect: [f64; 4],
    #[serde(default)]
    pub recipe: usize,
    pub recipe_layer: Option<String>,
    pub size: Option<[f64; 2]>,
    pub rotation: Option<f64>,
    pub shade_mean: Option<f64>,
    pub room_tile: Option<[f64; 2]>,
}

impl Catalog {
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    pub fn index(&self) -> CatalogIndex {
        let mut entries = BTreeMap::new();
        for entry in self
            .windows
            .iter()
            .chain(&self.decor)
            .chain(&self.surfaces)
            .chain(&self.modules)
        {
            entries.insert(entry.id.clone(), entry.clone());
        }
        CatalogIndex { entries }
    }

    pub fn pack(&self, name: &str) -> Option<&Pack> {
        self.packs.get(name)
    }

    /// Recipe row used by the native exporter's palette offsets.
    pub fn window_recipe(&self, entry: Option<&Entry>, pack: &str) -> Option<[f64; 4]> {
        let pack = entry.map(|entry| entry.pack.as_str()).unwrap_or(pack);
        let recipe = entry.map(|entry| entry.recipe).unwrap_or(19);
        let row = self.packs.get(pack)?.rows.first()?.get(recipe)?;
        if row.len() < 4 {
            return None;
        }
        let mut result = [row[0], row[1], row[2], row[3]];
        if let Some(tile) = entry.and_then(|entry| entry.room_tile) {
            result[2] = tile[0];
            result[3] = tile[1];
        }
        // resources.py::window_recipe selects the migrated Office room sheet.
        if pack == "Office" {
            result[2] = 1.0 / 16.0;
            result[3] = 0.0;
        }
        Some(result)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CatalogIndex {
    entries: BTreeMap<String, Entry>,
}

impl CatalogIndex {
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Catalog {
        Catalog::parse(
            r#"{
              "packs": {"Commercial": {"name": "商业", "columns": 63,
                "rows": [[[0.0,0.0,0.0,0.0],[0.1,0.2,0.3,0.4]]],
                "palette_row": 9.6, "spec_mode": 3.3}},
              "windows": [{"id":"w1","name":"窗","pack":"Commercial",
                "rect":[0.1,0.2,0.3,0.4],"recipe":1,"recipe_layer":"底层",
                "size":[2.0,2.5],"shade_mean":0.5}],
              "decor": [], "surfaces": [], "modules": []
            }"#
            .as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn index_covers_every_entry_family() {
        let catalog = fixture();
        let index = catalog.index();
        assert!(index.contains("w1"));
        assert_eq!(catalog.pack("Commercial").unwrap().palette_row, 9.6);
    }

    #[test]
    fn recipe_row_and_room_tile_override() {
        let mut catalog = fixture();
        catalog.packs.insert("Office".into(), catalog.packs["Commercial"].clone());
        let index = catalog.index();
        let entry = index.get("w1").unwrap();
        let row = catalog.window_recipe(Some(entry), "Commercial").unwrap();
        assert_eq!(row, [0.1, 0.2, 0.3, 0.4]);
        let mut tiled = entry.clone();
        tiled.room_tile = Some([0.7, 0.8]);
        assert_eq!(
            catalog.window_recipe(Some(&tiled), "Commercial").unwrap(),
            [0.1, 0.2, 0.7, 0.8]
        );
        tiled.pack = "Office".into();
        assert_eq!(
            catalog.window_recipe(Some(&tiled), "Office").unwrap(),
            [0.1, 0.2, 1.0 / 16.0, 0.0]
        );
    }
}
