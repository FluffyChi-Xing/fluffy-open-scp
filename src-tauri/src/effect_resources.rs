//! Locate Swarm collections and decode their referenced sprite textures on demand.

use std::collections::HashSet;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dbpf::Package;

const RW4_TYPE: u32 = 0x2F4E_681B;
const MAX_TEXTURE_PIXELS: u32 = 1024 * 1024;
const MAX_TEXTURE_PNG_BYTES: usize = 512 * 1024;

pub(super) fn locate_effect_packages(
    current: &Arc<Package>,
    manager: &crate::package_service::PackageManager,
    store: &sc_store::Store,
) -> Vec<Arc<Package>> {
    let mut packages = vec![Arc::clone(current)];
    if let Ok(open) = manager.all_packages_with_ids() {
        packages.extend(open.into_iter().map(|(_, package)| package));
    }
    let mut dirs = Vec::<PathBuf>::new();
    if let Ok(Some(settings)) = store.app_settings() {
        if let Some(game_data_path) = settings.game_data_path {
            let data_path = PathBuf::from(game_data_path);
            dirs.push(data_path.join("SimCityData"));
            dirs.push(data_path);
        }
    }
    for package in &packages {
        for ancestor in package.path().ancestors().take(5) {
            dirs.push(ancestor.to_path_buf());
            dirs.push(ancestor.join("SimCityData"));
        }
    }
    let mut seen: HashSet<PathBuf> = packages.iter().map(|p| p.path().to_path_buf()).collect();
    for dir in dirs {
        for name in [
            "SimCity_App.package",
            "SimCity_DLC0.package",
            "SimCity_Graphics.package",
        ] {
            let path = dir.join(name);
            if path.is_file() && seen.insert(path.clone()) {
                if let Ok(package) = Package::open(&path) {
                    packages.push(Arc::new(package));
                }
            }
        }
    }
    packages
}

pub(super) fn decode_effect_texture(
    packages: &[Arc<Package>],
    group: u32,
    instance: u32,
) -> Option<String> {
    let bytes = packages.iter().find_map(|package| {
        let entry = package.entries().iter().find(|entry| {
            entry.id.type_id == RW4_TYPE
                && entry.id.instance == instance
                && (group == u32::MAX || group == 0 || entry.id.group == group)
        })?;
        package.read(entry).ok()
    })?;
    let file = rw4::Rw4File::parse(&bytes).ok()?;
    let section = file
        .sections_of_type(rw4::SectionType::TEXTURE)
        .next()?
        .number;
    let texture = file.decode_texture(&bytes, section).ok()?;
    if u32::from(texture.width) * u32::from(texture.height) > MAX_TEXTURE_PIXELS {
        return None;
    }
    let rgba = texture.decode_top_mip_rgba().ok()?;
    let image = image::RgbaImage::from_raw(texture.width.into(), texture.height.into(), rgba)?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    if png.len() > MAX_TEXTURE_PNG_BYTES {
        return None;
    }
    Some(STANDARD.encode(png))
}
