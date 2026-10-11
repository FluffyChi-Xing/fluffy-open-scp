//! Reference-based Raster catalog. No filename or pixel-color heuristics.
use crate::activity::{AppState, CommandError};
use crate::package_service::TgiDto;
use base64::Engine;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
const RASTER: u32 = 0x2f4e681c;
const RW4: u32 = 0x2f4e681b;
const PROPERTY: u32 = 0x00b1b104;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LotPreviewAtlases {
    lot_surface_png: String,
    lot_normal_atlas_png: Option<String>,
}

/// New masks need shared game atlases, not an arbitrary building Property.
#[tauri::command]
pub async fn read_lot_preview_atlases(
    state: State<'_, AppState>,
) -> Result<Option<LotPreviewAtlases>, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let packages = manager.all_packages_with_ids().map_err(|e| e.to_string())?;
        let Some((_, current)) = packages.first() else {
            return Ok(None);
        };
        let key = |instance| sc_properties::Key {
            instance,
            type_id: 0,
            group: 0,
        };
        // Shared generic_lot diffuse atlas, documented in raster-lot-rendering.md.
        let surface =
            crate::package_service::decode_lot_surface_png(current, &manager, key(0xA0DA9E6C));
        let Ok((lot_surface_png, _)) = surface else {
            return Ok(None);
        };
        let lot_normal_atlas_png =
            crate::package_service::decode_lot_surface_png(current, &manager, key(0x60E7805D))
                .ok()
                .and_then(|(_, mut pixels)| {
                    for pixel in pixels.rgba.chunks_exact_mut(4) {
                        pixel[3] = 255;
                    }
                    let image =
                        image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.rgba)?;
                    let mut output = std::io::Cursor::new(Vec::new());
                    image.write_to(&mut output, image::ImageFormat::Png).ok()?;
                    Some(base64::engine::general_purpose::STANDARD.encode(output.into_inner()))
                });
        Ok(Some(LotPreviewAtlases {
            lot_surface_png,
            lot_normal_atlas_png,
        }))
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(CommandError::internal)
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterCatalogEntry {
    package_id: u64,
    tgi: TgiDto,
    owner_package_id: u64,
    owner: TgiDto,
    entry_index: Option<usize>,
}
fn decal_matches(profile: &str, material: Option<u32>) -> bool {
    match profile {
        "decal-sign" | "decal-sdf" => material == Some(0x73684efc),
        "decal-graffiti" => material == Some(0xe5390a98),
        "decal-interior" => material == Some(0x4491de3a),
        // Raw Decal editing accepts dictionary references regardless of shader routing.
        "decal-rgba" => true,
        _ => false,
    }
}
fn prefer_material_owners(
    entries: &mut Vec<RasterCatalogEntry>,
    owners: &std::collections::BTreeSet<(u64, u32, u32, u32)>,
) {
    let authored = |entry: &RasterCatalogEntry| {
        owners.contains(&(
            entry.owner_package_id,
            entry.owner.type_id,
            entry.owner.group,
            entry.owner.instance,
        ))
    };
    let mask_key = |entry: &RasterCatalogEntry| {
        (
            entry.package_id,
            entry.tgi.type_id,
            entry.tgi.group,
            entry.tgi.instance,
        )
    };
    let material_masks: std::collections::BTreeSet<_> = entries
        .iter()
        .filter(|entry| authored(entry))
        .map(mask_key)
        .collect();
    // Keep every authored variant; suppress bare templates only when their mask
    // has a concrete material owner in the loaded packages.
    entries.retain(|entry| authored(entry) || !material_masks.contains(&mask_key(entry)));
}
#[tauri::command]
pub async fn list_raster_catalog(
    state: State<'_, AppState>,
    package_id: u64,
    profile: String,
) -> Result<Vec<RasterCatalogEntry>, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let packages = manager.all_packages_with_ids().map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        let mut material_owners = std::collections::BTreeSet::new();
        let mut local = std::collections::BTreeMap::new();
        let mut fallback = std::collections::BTreeMap::new();
        for (id, p) in &packages {
            for e in p.entries() {
                if [RASTER, RW4].contains(&e.id.type_id) {
                    local
                        .entry((*id, e.id.instance, e.id.type_id))
                        .or_insert(e.id);
                    fallback
                        .entry((e.id.instance, e.id.type_id))
                        .or_insert((*id, e.id));
                }
            }
        }
        let resolve = |source: u64, instance: u32, types: &[u32]| {
            types
                .iter()
                .find_map(|ty| {
                    local
                        .get(&(source, instance, *ty))
                        .map(|tgi| (source, *tgi))
                })
                .or_else(|| {
                    types
                        .iter()
                        .find_map(|ty| fallback.get(&(instance, *ty)).copied())
                })
        };
        let mut add =
            |source: u64, owner: dbpf::ResourceId, instance: u32, types: &[u32], entry_index| {
                if let Some((id, tgi)) = resolve(source, instance, types) {
                    if id == package_id {
                        out.push(RasterCatalogEntry {
                            package_id: id,
                            tgi: TgiDto::from(tgi),
                            owner_package_id: source,
                            owner: TgiDto::from(owner),
                            entry_index,
                        });
                    }
                }
            };
        let slot = profile
            .strip_prefix("slot")
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|s| *s < 6);
        for (id, p) in &packages {
            for entry in p.entries() {
                if profile == "rgba" {
                    if *id == package_id && entry.id.type_id == RASTER {
                        add(*id, entry.id, entry.id.instance, &[RASTER], None);
                    }
                    continue;
                }
                if let Some(slot) = slot {
                    if entry.id.type_id != RW4 {
                        continue;
                    }
                    let Ok(bytes) = p.read(entry) else { continue };
                    let Ok(file) = rw4::Rw4File::parse(&bytes) else {
                        continue;
                    };
                    for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
                        let Ok(rw4::MaterialSection::Decoded(material)) =
                            file.decode_material(&bytes, section.number)
                        else {
                            continue;
                        };
                        // The building six-slot contract requires a float32 slot0 parameter table.
                        let Some(param) = material.slot_texture(0) else {
                            continue;
                        };
                        let Some((pid, tgi)) = resolve(*id, param, &[RW4]) else {
                            continue;
                        };
                        let Some((_, pkg)) = packages.iter().find(|(id, _)| *id == pid) else {
                            continue;
                        };
                        let Some(e) = pkg.entry(tgi) else { continue };
                        let Ok(data) = pkg.read(e) else { continue };
                        let Ok(texfile) = rw4::Rw4File::parse(&data) else {
                            continue;
                        };
                        let Some(sec) = texfile.sections_of_type(rw4::SectionType::TEXTURE).next()
                        else {
                            continue;
                        };
                        let Ok(tex) = texfile.decode_texture(&data, sec.number) else {
                            continue;
                        };
                        if tex.height != 4
                            || tex.texture_type != 116
                            || tex.blob.len() < usize::from(tex.width) * 64
                        {
                            continue;
                        }
                        if let Some(reference) =
                            material.texture_slots().find(|r| r.slot_byte() == slot)
                        {
                            add(
                                *id,
                                entry.id,
                                reference.texture_instance,
                                &[RASTER, RW4],
                                None,
                            );
                        }
                    }
                    continue;
                }
                if entry.id.type_id != PROPERTY {
                    continue;
                }
                let Ok(bytes) = p.read(entry) else { continue };
                let Ok(file) = sc_properties::PropertyFile::parse(&bytes) else {
                    continue;
                };
                if profile.starts_with("lot-") {
                    let file = crate::package_service::flatten_lot_parents(file, p, &manager);
                    let lot = sc_properties::LotEditorDocument::from_property_file(file);
                    if crate::package_service::lot_colors(&lot)
                        .1
                        .iter()
                        .any(|value| *value)
                    {
                        material_owners.insert((
                            *id,
                            entry.id.type_id,
                            entry.id.group,
                            entry.id.instance,
                        ));
                    }
                    let reference = if profile == "lot-mask" {
                        lot.lot_mask
                    } else {
                        lot.lot_textures
                    };
                    if let Some(key) = reference {
                        add(
                            *id,
                            entry.id,
                            key.instance,
                            if profile == "lot-mask" {
                                &[RASTER]
                            } else {
                                &[RW4]
                            },
                            None,
                        );
                    }
                } else if sc_properties::is_decal_dictionary_group(entry.id.group)
                    && sc_properties::decal::looks_like_dictionary(&file)
                {
                    let dictionary = sc_properties::DecalDictionary::from_file(&file);
                    if decal_matches(&profile, dictionary.material.map(|k| k.instance)) {
                        for e in dictionary.entries {
                            if let Some(key) = e.raster {
                                add(*id, entry.id, key.instance, &[RASTER, RW4], Some(e.index));
                            }
                        }
                    }
                }
            }
        }
        if profile == "lot-mask" {
            prefer_material_owners(&mut out, &material_owners);
        }
        out.sort_by_key(|e| {
            (
                e.tgi.instance,
                e.owner_package_id,
                e.owner.instance,
                e.entry_index,
            )
        });
        out.dedup_by_key(|e| {
            (
                e.tgi.type_id,
                e.tgi.group,
                e.tgi.instance,
                e.owner_package_id,
                e.owner.instance,
                e.entry_index,
            )
        });
        Ok(out)
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(CommandError::internal)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterSourceData {
    width: u32,
    height: u32,
    rgba_base64: Option<String>,
    parameters: Option<Parameters>,
}
#[derive(Serialize)]
pub struct Parameters {
    cols: u32,
    values: Vec<f32>,
}
#[tauri::command]
pub async fn read_raster_source(
    state: State<'_, AppState>,
    package_id: u64,
    tgi: TgiDto,
    parameters: bool,
) -> Result<RasterSourceData, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let p = manager.get(package_id).map_err(|e| e.to_string())?;
        let id: dbpf::ResourceId = tgi.into();
        let entry = p.entry(id).ok_or("Raster source not found")?;
        let bytes = p.read(entry).map_err(|e| e.to_string())?;
        let (width, height, rgba, params) = if id.type_id == RW4 {
            let file = rw4::Rw4File::parse(&bytes).map_err(|e| e.to_string())?;
            let sec = file
                .sections_of_type(rw4::SectionType::TEXTURE)
                .next()
                .ok_or("Texture section missing")?;
            let tex = file
                .decode_texture(&bytes, sec.number)
                .map_err(|e| e.to_string())?;
            if u32::from(tex.width) * u32::from(tex.height) > 4096 * 4096 {
                return Err("Raster exceeds 4096² pixels".into());
            }
            if parameters {
                if tex.height != 4 {
                    return Err("Expected four parameter rows".into());
                }
                let values = tex
                    .decode_palette_f32()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .flatten()
                    .collect();
                (
                    u32::from(tex.width),
                    u32::from(tex.height),
                    None,
                    Some(Parameters {
                        cols: u32::from(tex.width),
                        values,
                    }),
                )
            } else {
                (
                    u32::from(tex.width),
                    u32::from(tex.height),
                    Some(tex.decode_top_mip_rgba().map_err(|e| e.to_string())?),
                    None,
                )
            }
        } else {
            if parameters {
                return Err("Expected float32 texture".into());
            }
            let image = rw4::RasterImage::parse(&bytes).map_err(|e| e.to_string())?;
            if u64::from(image.width) * u64::from(image.height) > 4096 * 4096 {
                return Err("Raster exceeds 4096² pixels".into());
            }
            (
                image.width,
                image.height,
                Some(image.decode_top_mip_rgba().map_err(|e| e.to_string())?),
                None,
            )
        };
        if width as usize * height as usize > 4096 * 4096 {
            return Err("Raster exceeds 4096² pixels".into());
        }
        Ok(RasterSourceData {
            width,
            height,
            rgba_base64: rgba.map(|r| base64::engine::general_purpose::STANDARD.encode(r)),
            parameters: params,
        })
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(CommandError::internal)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mask_templates_do_not_shadow_material_variants() {
        let entry = |mask, owner| RasterCatalogEntry {
            package_id: 1,
            tgi: TgiDto {
                type_id: RASTER,
                group: 0,
                instance: mask,
            },
            owner_package_id: 2,
            owner: TgiDto {
                type_id: PROPERTY,
                group: 0,
                instance: owner,
            },
            entry_index: None,
        };
        let mut entries = vec![entry(10, 1), entry(10, 2), entry(10, 3), entry(20, 4)];
        let owners = [(2, PROPERTY, 0, 2), (2, PROPERTY, 0, 3)]
            .into_iter()
            .collect();
        prefer_material_owners(&mut entries, &owners);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.owner.instance)
                .collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
    }
    #[test]
    fn decal_profiles_are_reference_routed() {
        assert!(decal_matches("decal-sdf", Some(0x73684efc)));
        assert!(!decal_matches("decal-graffiti", Some(0x73684efc)));
        assert!(decal_matches("decal-interior", Some(0x4491de3a)));
        assert!(!decal_matches("decal-sign", None));
        assert!(decal_matches("decal-rgba", None));
    }
}
