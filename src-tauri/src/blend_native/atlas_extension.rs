//! Additional Editor atlas data can travel with a .blend in <stem>.atlas/.

use std::{
    collections::HashMap,
    fs,
    io::{Cursor, ErrorKind},
    path::Path,
};

use sha2::{Digest, Sha256};

use super::catalog::{Catalog, Entry};
use super::resources::PackStore;

pub(crate) struct AtlasExtension {
    catalog: Catalog,
    images: HashMap<(String, u8), Vec<u8>>,
    pub(crate) fingerprint: String,
}

impl AtlasExtension {
    pub(crate) fn read(source: &Path) -> Result<Option<Self>, String> {
        let root = source.with_extension("atlas");
        let catalog_path = root.join("catalog.json");
        let bytes = match fs::read(&catalog_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound && !root.exists() => return Ok(None),
            Err(error) => return Err(format!("atlas catalog {}: {error}", catalog_path.display())),
        };
        let catalog = Catalog::parse(&bytes).map_err(|error| format!("atlas catalog: {error}"))?;
        let mut digest = Sha256::new();
        digest.update(Sha256::digest(&bytes));
        let mut images = HashMap::new();
        for name in catalog.packs.keys() {
            if name.is_empty() || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
                return Err(format!("atlas catalog: invalid pack identifier {name:?}"));
            }
            let directory = root.join(name);
            if !directory.exists() {
                // An exported catalog may also list packs already shipped with OpenSCP.
                continue;
            }
            digest.update((name.len() as u64).to_le_bytes());
            digest.update(name.as_bytes());
            let mut exterior_size = None;
            for slot in 1..=5 {
                let path = directory.join(format!("slot{slot}.png"));
                let bytes = fs::read(&path)
                    .map_err(|error| format!("atlas image {}: {error}", path.display()))?;
                let size =
                    image::ImageReader::with_format(Cursor::new(&bytes), image::ImageFormat::Png)
                        .into_dimensions()
                        .map_err(|error| format!("atlas image {}: {error}", path.display()))?;
                if (slot <= 3 && size != *exterior_size.get_or_insert(size))
                    || (slot == 4 && size.1 != 16)
                {
                    return Err(format!(
                        "atlas image {}: incompatible dimensions {size:?}",
                        path.display()
                    ));
                }
                digest.update(Sha256::digest(&bytes));
                images.insert((name.clone(), slot), bytes);
            }
        }
        Ok(Some(Self {
            catalog,
            images,
            fingerprint: format!("{:x}", digest.finalize()),
        }))
    }

    pub(crate) fn merge(self, catalog: &mut Catalog, store: PackStore) -> PackStore {
        let mut images = self.images;
        images.retain(|(name, _), _| !catalog.packs.contains_key(name));
        catalog.packs.extend(
            self.catalog
                .packs
                .into_iter()
                .filter(|(name, _)| images.contains_key(&(name.clone(), 1))),
        );
        let index = catalog.index();
        let supported =
            |entry: &Entry| !index.contains(&entry.id) && catalog.packs.contains_key(&entry.pack);
        catalog
            .windows
            .extend(self.catalog.windows.into_iter().filter(&supported));
        catalog
            .decor
            .extend(self.catalog.decor.into_iter().filter(&supported));
        catalog
            .surfaces
            .extend(self.catalog.surfaces.into_iter().filter(&supported));
        catalog
            .modules
            .extend(self.catalog.modules.into_iter().filter(&supported));
        store.with_additional_packs(images)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_new_pack_and_styles_without_replacing_shipped_data() {
        let mut base = Catalog::parse(
            br#"{
            "packs":{"Base":{"name":"shipped","columns":63}},
            "windows":[{"id":"old","pack":"Base","rect":[1,1,0,0]}]
        }"#,
        )
        .unwrap();
        let extension = AtlasExtension {
            catalog: Catalog::parse(
                br#"{
                "packs":{"Base":{"name":"extra","columns":63},
                    "DLC_new":{"name":"new","columns":63,"rows":[[[0.1,0.2,0.3,0.4]]]}},
                "windows":[{"id":"old","pack":"DLC_new","rect":[0.5,0.5,0,0]},
                    {"id":"new","pack":"DLC_new","rect":[0.5,0.5,0,0]},
                    {"id":"unknown","pack":"absent","rect":[1,1,0,0]}]
            }"#,
            )
            .unwrap(),
            images: HashMap::from([
                (("Base".into(), 1), Vec::new()),
                (("DLC_new".into(), 1), Vec::new()),
            ]),
            fingerprint: String::new(),
        };
        extension.merge(&mut base, PackStore::new("."));
        let index = base.index();
        assert_eq!(base.pack("Base").unwrap().name, "shipped");
        assert_eq!(index.get("old").unwrap().pack, "Base");
        assert_eq!(
            base.window_recipe(index.get("new"), ""),
            Some([0.1, 0.2, 0.3, 0.4])
        );
        assert!(!index.contains("unknown"));
    }

    #[test]
    fn reads_all_five_slots_and_tracks_texture_changes() {
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("openscp-atlas-test-{}", std::process::id()));
        let source = path.join("asset.blend");
        let atlas = source.with_extension("atlas");
        let pack = atlas.join("Future_2");
        fs::create_dir_all(&pack).unwrap();
        fs::write(
            atlas.join("catalog.json"),
            br#"{"packs":{"Future_2":{"name":"future","columns":63}}}"#,
        )
        .unwrap();
        let write_slot = |slot, value| {
            image::RgbaImage::from_pixel(16, 16, image::Rgba([value, 0, 0, 255]))
                .save(pack.join(format!("slot{slot}.png")))
                .unwrap();
        };
        for slot in 1..=4 {
            write_slot(slot, slot as u8);
        }
        assert!(AtlasExtension::read(&source).is_err());
        write_slot(5, 5);
        let first = AtlasExtension::read(&source).unwrap().unwrap();
        write_slot(5, 6);
        let second = AtlasExtension::read(&source).unwrap().unwrap();
        assert_ne!(first.fingerprint, second.fingerprint);
        let mut catalog = Catalog::parse(b"{}").unwrap();
        let store = second.merge(&mut catalog, PackStore::new(&path));
        assert!(store.load("Future_2", 5).unwrap().get(0, 0)[0] > 0.0);
        fs::remove_dir_all(path).unwrap();
    }
}
