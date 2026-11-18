//! Claim-card ratings are literals in the retail UI registry, separate from the city EGB.
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{region_3d, region_state::StateMap};
use std::{collections::BTreeMap, path::Path};

use super::City;

pub const UI_ID: ResourceId = ResourceId {
    type_id: 0x67771F5C,
    group: 0x40464200,
    instance: 0x7CCC548C,
};
pub(super) const LAUNCHER_ID: ResourceId = ResourceId {
    instance: 0x696447EA,
    ..UI_ID
};
const LAUNCHER_TWEAK: &str = "000_LauncherTweak.package";
const FIELDS: [(u32, &str); 4] = [
    (0xD779C976, "coal"),
    (0xD74AF22B, "rawOre"),
    (0x76D10FF2, "crudeOil"),
    (0x0F92DC56, "water"),
];
pub(super) type Totals = BTreeMap<(String, u32), u64>;

fn total(map: &StateMap) -> u64 {
    map.values.iter().map(|&amount| amount.max(0) as u64).sum()
}

pub(super) fn collect(changes: &mut Totals, city: &City, amended: &[StateMap]) {
    for (layer, _) in FIELDS {
        let Some(old) = city.maps.iter().find(|m| m.id == layer) else {
            continue;
        };
        let Some(new) = amended.iter().find(|m| m.id == layer) else {
            continue;
        };
        let next = total(new);
        if next != total(old) {
            changes.insert((city.site.clone(), layer), next);
        }
    }
}

pub(super) fn field(js: &str, city_name_id: u32, name: &str) -> Result<(usize, u8), String> {
    let marker = format!("cityName:\"AutomatedRegionTemplates.json!0x{city_name_id:08x}\"");
    let mut matches = js.match_indices(&marker);
    let (start, _) = matches
        .next()
        .ok_or("Claim registry city name is missing")?;
    if matches.next().is_some() {
        return Err("Claim registry city name is ambiguous".into());
    }
    let remaining = &js[start..];
    let (record, _) = remaining
        .split_once('}')
        .ok_or("Truncated claim registry city")?;
    let key = format!("{name}:\"");
    let mut fields = record.match_indices(&key).filter(|(offset, _)| {
        record[..*offset]
            .trim_end_matches(|c: char| c.is_ascii_whitespace())
            .ends_with(',')
    });
    let (offset, _) = fields
        .next()
        .ok_or("Claim registry resource field is missing")?;
    let offset = start + offset + key.len();
    let digit = js
        .as_bytes()
        .get(offset..offset + 2)
        .ok_or("Invalid claim resource rating")?;
    if fields.next().is_some() || !matches!(digit, [b'0'..=b'3', b'"']) {
        return Err("Invalid or ambiguous claim resource rating".into());
    }
    Ok((offset, digit[0] - b'0'))
}

fn rating(amount: u64, per_bar: Option<f64>) -> u8 {
    if amount == 0 {
        0
    } else if let Some(step) = per_bar {
        (amount as f64 / step).round().clamp(1., 3.) as u8
    } else {
        // When vanilla has no nonzero rating in this region there is no
        // reference tonnage for higher tiers. Show presence as a single bar.
        1
    }
}

pub(super) fn overlays(
    terrain_path: &Path,
    group: u32,
    cities: &[City],
    totals: &Totals,
) -> Result<(Vec<OverlayEntry>, bool), String> {
    if totals.is_empty() {
        return Ok((vec![], false));
    }
    let game_path = terrain_path
        .canonicalize()
        .map_err(|err| err.to_string())?
        .with_file_name("SimCity_Game.package");
    let game =
        Package::open(&game_path).map_err(|err| format!("{}: {err}", game_path.display()))?;
    let stock = game
        .read(game.entry(UI_ID).ok_or("Missing retail claim registry")?)
        .map_err(|err| err.to_string())?;
    let js = std::str::from_utf8(&stock).map_err(|err| err.to_string())?;
    let registry = region_3d::parse_template_registry(&game);
    let template =
        region_3d::template_name_for_group(group).ok_or("Missing region template binding")?;
    let (_, names) =
        region_3d::registry_lookup(&registry, template).ok_or("Missing claim registry region")?;
    let mut ratings = Vec::new();
    for (layer, name) in FIELDS {
        if !totals.keys().any(|(_, changed)| *changed == layer) {
            continue;
        }
        let mut samples = Vec::new();
        for city in cities {
            let Some(map) = city.maps.iter().find(|map| map.id == layer) else {
                continue;
            };
            let city_name = names
                .get(&city.site)
                .ok_or("Missing claim registry city binding")?;
            let (_, level) = field(js, *city_name, name)
                .map_err(|err| format!("{template}/{} ({name}): {err}", city.site))?;
            let amount = total(map);
            let per_bar = (amount > 0 && level > 0).then(|| amount as f64 / f64::from(level));
            samples.push((&city.site, per_bar));
        }
        let mut peers: Vec<f64> = samples.iter().filter_map(|sample| sample.1).collect();
        peers.sort_by(f64::total_cmp);
        for (site, per_bar) in samples {
            if let Some(&amount) = totals.get(&(site.clone(), layer)) {
                let step = per_bar.or_else(|| peers.get(peers.len() / 2).copied());
                ratings.push((names[site], name, rating(amount, step)));
            }
        }
    }

    let mut entries = Vec::new();
    patch_script(&mut entries, UI_ID, &stock, &ratings)?;
    let tweak_path = game_path.with_file_name(LAUNCHER_TWEAK);
    let tweak = match tweak_path.try_exists().map_err(|err| err.to_string())? {
        true => Some(Package::open(&tweak_path).map_err(|err| err.to_string())?),
        false => None,
    };
    let app;
    let (launcher, entry, overrides_tweak) = if let Some((package, entry)) = tweak
        .as_ref()
        .and_then(|package| package.entry(LAUNCHER_ID).map(|entry| (package, entry)))
    {
        (package, entry, true)
    } else {
        let app_path = game_path.with_file_name("SimCity_App.package");
        app = Package::open(&app_path).map_err(|err| format!("{}: {err}", app_path.display()))?;
        (
            &app,
            app.entry(LAUNCHER_ID)
                .ok_or("Missing retail launcher claim registry")?,
            false,
        )
    };
    let launcher_stock = launcher.read(entry).map_err(|err| err.to_string())?;
    patch_script(&mut entries, LAUNCHER_ID, &launcher_stock, &ratings)?;
    let earlier_name = overrides_tweak && entries.iter().any(|entry| entry.id == LAUNCHER_ID);
    Ok((entries, earlier_name))
}

fn patch_script(
    entries: &mut Vec<OverlayEntry>,
    id: ResourceId,
    stock: &[u8],
    ratings: &[(u32, &str, u8)],
) -> Result<(), String> {
    let js = std::str::from_utf8(stock).map_err(|err| err.to_string())?;
    let mut edited = stock.to_vec();
    for &(city, name, value) in ratings {
        let (offset, _) =
            field(js, city, name).map_err(|err| format!("{id} city {city:08X} ({name}): {err}"))?;
        edited[offset] = b'0' + value;
    }
    if edited != stock {
        entries.push(OverlayEntry::new(id, edited));
    }
    Ok(())
}
