use super::*;
use crate::{
    Key, Kind, PropertyFile, Value,
    region_3d::{self, fnv1_lower},
    region_bootstrap,
    region_state::{RegionState, read_state},
    region_write::{REGION_FIELD_PX, heightmap_entries},
};
use dbpf::{OverlayEntry, Package, ResourceId};
use serde_json::{Value as Json, json};
use std::collections::{HashMap, HashSet};

const TEMPLATE: &str = "OpenSCPHalfCoast";
const SITE: i32 = 1026;
const REGION_NUMERIC_ID: &str = "1840525230";
const FULL: i32 = 65534;
const RESOURCE_MAPS: [u32; 4] = [0xd779c976, 0xd74af22b, 0x76d10ff2, 0x0f92dc56];
const DONOR_REGION: u32 = 0xc2a9c48f;
const DONOR_REGION_NUMERIC_ID: &str = "257369091";
const DONOR_CITY: u32 = 0xc8156be2;
const DONOR_META_GROUP: u32 = 0xdacb5a49;
const PROP: u32 = 0x00b1b104;
const F0: u32 = 0x03e421f0;
const ED: u32 = 0x03e421ed;
const ENV: u32 = 0x494f8678;
const CITY_LIST: u32 = 0x2b9c480c;
const META: u32 = 0x4529f96f;
const BINDINGS: u32 = 0x95f4aef0;
const ROUTES: u32 = 0xf9b19bd3;
const LOCAL_TEXT_KEYS: [&str; 4] = [
    "openscp_halfcoast_region_name",
    "openscp_halfcoast_region_description",
    "openscp_halfcoast_city_name",
    "openscp_halfcoast_city_description",
];

#[derive(Clone, Copy)]
pub enum RegionLocale {
    EnUs,
    ZhCn,
    ZhTw,
}

pub struct RetailSources<'a> {
    pub terrain: &'a Package,
    pub ep1: &'a Package,
    pub app: &'a Package,
    pub game: &'a Package,
    pub locale: &'a Package,
    pub language: RegionLocale,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionBuildReport {
    pub template: &'static str,
    pub region_environment: u32,
    pub city_environment: u32,
    pub city_uid: i32,
    pub region_side_meters: i32,
    pub city_height_samples_per_side: usize,
    pub city_eco_cells_per_side: usize,
    pub resource_value: i32,
    pub highway_controls: [[f32; 3]; 4],
    pub package_entries: usize,
}

fn id(type_id: u32, group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id,
        group,
        instance,
    }
}

fn from_key(key: Key) -> ResourceId {
    id(key.type_id, key.group, key.instance)
}

fn key(resource: ResourceId) -> Value {
    Value::Key(Key {
        type_id: resource.type_id,
        group: resource.group,
        instance: resource.instance,
    })
}

fn region_group() -> u32 {
    fnv1_lower(REGION_NUMERIC_ID)
}

fn city_group() -> u32 {
    fnv1_lower("openscp_halfcoast_city_environment")
}

fn catalog_group() -> u32 {
    fnv1_lower("openscp_halfcoast_template_resources")
}

fn city_list_id(region_name: &str) -> ResourceId {
    // Retail MakeCityListGroupID (0xBEB5D0) calls SPGroupIDFromName
    // (0x435960), which sets bit 31 after hashing the name.
    id(
        PROP,
        fnv1_lower(&format!("00000000_citylist_{region_name}")) | 0x8000_0000,
        CITY_LIST,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_list_uses_native_group_id_for_both_hash_ranges() {
        assert_eq!(city_list_id(DONOR_REGION_NUMERIC_ID).group, 0xf0e2_e39c);
        assert_eq!(city_list_id(REGION_NUMERIC_ID).group, 0xa53a_3cf6);
    }
}

fn read(package: &Package, resource: ResourceId) -> Result<Vec<u8>, String> {
    let entry = package
        .entry(resource)
        .ok_or_else(|| format!("Missing donor resource {resource:?}"))?;
    package.read(entry).map_err(|e| e.to_string())
}

fn props_at(package: &Package, resource: ResourceId) -> Result<PropertyFile, String> {
    PropertyFile::parse(&read(package, resource)?).map_err(|e| e.to_string())
}

fn set(props: &mut PropertyFile, field: u32, kind: Kind) -> Result<(), String> {
    let item = props
        .values
        .iter_mut()
        .find(|p| p.hash == field)
        .ok_or_else(|| format!("Missing donor property {field:08X}"))?;
    item.kind = kind;
    Ok(())
}

fn push_prop(
    entries: &mut Vec<OverlayEntry>,
    resource: ResourceId,
    props: &PropertyFile,
) -> Result<(), String> {
    let bytes = props.encode_canonical().map_err(|e| e.to_string())?;
    let decoded = PropertyFile::parse(&bytes).map_err(|e| e.to_string())?;
    if decoded.values != props.values {
        return Err(format!("Property encoding changed values for {resource:?}"));
    }
    entries.push(OverlayEntry::new(resource, bytes));
    Ok(())
}

fn remap_property_keys(props: &mut PropertyFile, groups: &[(u32, u32)]) {
    for prop in &mut props.values {
        let values = match &mut prop.kind {
            Kind::Scalar(value) => std::slice::from_mut(value),
            Kind::Array(values) => values,
            Kind::Empty => continue,
        };
        for value in values {
            if let Value::Key(key) = value
                && let Some((_, target)) = groups.iter().find(|(source, _)| *source == key.group)
            {
                key.group = *target;
            }
        }
    }
}

fn terrain_entries(
    sources: &RetailSources<'_>,
    generated: &CoastalRegion,
) -> Result<Vec<OverlayEntry>, String> {
    let table_id = city_list_id(DONOR_REGION_NUMERIC_ID);
    let mut table = props_at(sources.terrain, table_id)?;
    let row = table
        .get(0x16b7b1ef)
        .and_then(|p| p.array())
        .and_then(|rows| rows.iter().position(|v| *v == Value::UInt32(DONOR_CITY)))
        .ok_or("Cape Trinity city table is missing the template city")?;
    let target_table = city_list_id(REGION_NUMERIC_ID);
    let groups = [
        (DONOR_REGION, region_group()),
        (DONOR_CITY, city_group()),
        (table_id.group, target_table.group),
    ];
    let mut entries = Vec::new();

    for field in [0x0543bc96, 0x16b7b1ef, 0x9f2f9b65, 0xc0b13e7e, 0xf01de4b1] {
        let value = table
            .get(field)
            .and_then(|p| p.array())
            .and_then(|rows| rows.get(row))
            .ok_or_else(|| format!("CityList column {field:08X} is incomplete"))?;
        let value = match field {
            0x16b7b1ef => Value::UInt32(city_group()),
            0xf01de4b1 => Value::Vector2([0.0, 0.0]),
            _ => value.clone(),
        };
        set(&mut table, field, Kind::Array(vec![value]))?;
    }
    push_prop(&mut entries, target_table, &table)?;

    for entry in sources.terrain.entries().iter().filter(|e| {
        e.id.type_id == PROP
            && e.id.group == DONOR_REGION
            && e.id.instance != water_bounds::INSTANCE
    }) {
        let mut props = props_at(sources.terrain, entry.id)?;
        remap_property_keys(&mut props, &groups);
        if entry.id.instance == 0x51e7a18d {
            set(
                &mut props,
                0x4ee97f5b,
                Kind::Scalar(Value::String8(REGION_NUMERIC_ID.into())),
            )?;
        }
        // Terrain and resources are baked. Clear the donor's authored brush transforms and parallel inputs.
        for field in [0x02a907b5, 0x02a907b6, 0x03a23f97, 0x03dd8e59, 0x03dd8e5a] {
            if props.get(0x0dba3a9c).is_some() && props.get(field).is_some() {
                set(&mut props, field, Kind::Array(Vec::new()))?;
            }
        }
        push_prop(
            &mut entries,
            id(PROP, region_group(), entry.id.instance),
            &props,
        )?;
    }

    let mut environment = props_at(sources.terrain, id(PROP, DONOR_CITY, ENV))?;
    remap_property_keys(&mut environment, &groups);
    set(&mut environment, 0x1b53d525, Kind::Scalar(Value::Int32(0)))?;
    push_prop(&mut entries, id(PROP, city_group(), ENV), &environment)?;

    // Preserve the native 32 km backing extent. Outside the 8 km view, extend the nearest edge samples.
    let start = (REGION_FIELD_PX - HEIGHT_SIDE) / 2;
    let mut field = Vec::with_capacity(REGION_FIELD_PX * REGION_FIELD_PX);
    for y in 0..REGION_FIELD_PX {
        let row = y.saturating_sub(start).min(HEIGHT_SIDE - 1) * HEIGHT_SIDE;
        for x in 0..REGION_FIELD_PX {
            field.push(generated.heights[row + x.saturating_sub(start).min(HEIGHT_SIDE - 1)]);
        }
    }
    let mut terrain = heightmap_entries(sources.terrain, DONOR_REGION, &field)?;
    let tiles: HashMap<_, _> = terrain.iter().map(|e| (e.id.instance, &e.data)).collect();
    let bounds = water_bounds::from_f0(|instance| {
        tiles
            .get(&instance)
            .map(|tile| tile.as_slice())
            .ok_or_else(|| format!("Missing regional F0 tile {instance:08X}"))
    })?;
    let min_max_id = id(PROP, region_group(), water_bounds::INSTANCE);
    let mut props = props_at(sources.terrain, id(PROP, DONOR_REGION, min_max_id.instance))?;
    set(
        &mut props,
        water_bounds::HEIGHT_MIN_MAX,
        Kind::Array(bounds),
    )?;
    push_prop(&mut entries, min_max_id, &props)?;
    for entry in &mut terrain {
        entry.id.group = region_group();
    }
    entries.extend(terrain);

    let mut ed_count = 0;
    for entry in sources
        .terrain
        .entries()
        .iter()
        .filter(|e| e.id.group == DONOR_REGION && e.id.type_id == ED)
    {
        let mut tile = read(sources.terrain, entry.id)?;
        if tile.len() != 20 + 128 * 128 * 4 {
            return Err(format!("Unexpected ED tile size for {:?}", entry.id));
        }
        for pixel in tile[20..].chunks_exact_mut(4) {
            // Groundwater and soil are full, with a clear test surface and no inherited road marks.
            pixel.copy_from_slice(&[255, 0, 255, 0]);
        }
        entries.push(OverlayEntry::new(
            id(ED, region_group(), entry.id.instance),
            tile,
        ));
        ed_count += 1;
    }
    if ed_count != 341 {
        return Err(format!("Expected 341 ED tiles, found {ed_count}"));
    }

    let source = sources
        .terrain
        .entries()
        .iter()
        .find(|e| e.id.type_id == F0 && e.id.group == DONOR_CITY)
        .ok_or("Missing stock city F0")?;
    let height = read(sources.terrain, source.id)?;
    let mut city = height
        .get(..20)
        .ok_or("City F0 header is truncated")?
        .to_vec();
    for offset in [4, 8] {
        city[offset..offset + 4].copy_from_slice(&(HEIGHT_SIDE as u32).to_be_bytes());
    }
    city[16..20].copy_from_slice(&((generated.heights.len() * 2) as u32).to_be_bytes());
    for pixel in &generated.heights {
        city.extend(pixel.to_le_bytes());
    }
    entries.push(OverlayEntry::new(
        id(F0, city_group(), source.id.instance),
        city,
    ));
    Ok(entries)
}

fn decode_state(raw: &[u8]) -> Result<RegionState, String> {
    read_state(&super::bootstrap::decompress(raw)?)
}

fn state_entry(
    source: &[u8],
    target: ResourceId,
    environment: u32,
    city: bool,
    generated: &CoastalRegion,
) -> Result<OverlayEntry, String> {
    let source = super::bootstrap::decompress(source)?;
    let mut state = read_state(&source)?;
    state.environment = environment;
    let mut expected_resources = HashSet::from(RESOURCE_MAPS);
    for map in &mut state.maps {
        if city {
            map.size = ECO_SIDE;
            let stock_default = match map.id {
                // Donor city land-use baselines and resource mirror maps.
                0xcc5539cc | 0xcc5539cd => 32372,
                0xcc7a0ae4 | 0xcc7a0ae5 => 32500,
                0x138eb1ef | 0xf99e6cea | 0x28f896a4 => FULL,
                _ => 0,
            };
            map.values = match map.id {
                0x6f9f6ab3 => generated.ground.clone(),
                0xab4a0ea0 => generated.water_depth.clone(),
                _ => vec![stock_default; ECO_SIDE * ECO_SIDE],
            };
        }
        if expected_resources.remove(&map.id) && city {
            map.values.fill(FULL);
        }
    }
    if !expected_resources.is_empty() {
        return Err(format!(
            "Missing stock resource maps: {expected_resources:?}"
        ));
    }
    Ok(OverlayEntry::new(
        target,
        super::bootstrap::encode(&source, generated, &state)?,
    ))
}

fn template_entries(
    sources: &RetailSources<'_>,
    generated: &CoastalRegion,
) -> Result<Vec<OverlayEntry>, String> {
    let ep = sources.ep1;
    let bindings = region_bootstrap::catalog(ep)?;
    let donor = bindings
        .iter()
        .find(|entry| entry.region == DONOR_REGION)
        .ok_or("Cape Trinity template missing")?;
    let source_meta = from_key(donor.metadata);
    if source_meta.group != DONOR_META_GROUP {
        return Err("Unexpected Cape Trinity metadata group".into());
    }
    let target_meta = id(source_meta.type_id, catalog_group(), META);
    let mut metadata: Json =
        serde_json::from_slice(&read(ep, source_meta)?).map_err(|e| e.to_string())?;
    let original_city = metadata["map"]["cities"][0].clone();
    let original_box = metadata["boxes"][0].clone();
    if original_city["uid"] != SITE.to_string() || !original_box.is_object() {
        return Err("Unexpected Cape Trinity metadata row".into());
    }
    let mut city = original_city;
    let mut city_box = original_box;
    for location in [&mut city, &mut city_box] {
        location["x"] = json!(0);
        location["y"] = json!(0);
    }
    city["z"] = json!(SEA);
    metadata["map"]["cities"] = json!([city]);
    metadata["map"]["greatWorks"] = json!([]);
    metadata["boxes"] = json!([city_box]);
    metadata["name"] = json!(TEMPLATE);
    metadata["localizedName"] = json!(TEMPLATE);
    // The retail payload represents template key as a decimal string.
    metadata["key"] = json!((catalog_group() & 0x7fff_ffff).to_string());

    let mut entries = vec![
        OverlayEntry::new(
            target_meta,
            serde_json::to_vec(&metadata).map_err(|e| e.to_string())?,
        ),
        OverlayEntry::new(id(0x08068aee, catalog_group(), ROUTES), vec![0; 12]),
    ];
    let mut keys = Vec::new();
    for (site, environment) in [(0, region_group()), (SITE, city_group())] {
        let (_, source) = donor
            .sites
            .iter()
            .find(|(uid, _)| uid == &site.to_string())
            .ok_or("Donor bootstrap site missing")?;
        let source = from_key(*source);
        let target = id(source.type_id, catalog_group(), source.instance);
        entries.push(state_entry(
            &read(ep, source)?,
            target,
            environment,
            site != 0,
            generated,
        )?);
        keys.push(key(target));
    }

    let metadata_key = key(source_meta);
    let mut binding = None;
    for entry in ep
        .entries()
        .iter()
        .filter(|e| e.id.type_id == PROP && e.id.group == BINDINGS)
    {
        let props = props_at(ep, entry.id)?;
        if props
            .get(0xf240a8ca)
            .is_some_and(|p| p.kind == Kind::Scalar(metadata_key.clone()))
        {
            binding = Some(props);
            break;
        }
    }
    let mut binding = binding.ok_or("Cape Trinity registration property missing")?;
    remap_property_keys(&mut binding, &[(DONOR_META_GROUP, catalog_group())]);
    set(&mut binding, 0xf240a8ca, Kind::Scalar(key(target_meta)))?;
    set(&mut binding, 0x46066280, Kind::Array(keys))?;
    set(
        &mut binding,
        0x46066281,
        Kind::Array(vec![Value::Int32(0), Value::Int32(SITE)]),
    )?;
    push_prop(
        &mut entries,
        id(
            PROP,
            BINDINGS,
            fnv1_lower("openscp_halfcoast_template_binding"),
        ),
        &binding,
    )?;
    Ok(entries)
}

fn ui_entries(
    sources: &RetailSources<'_>,
    generated: &CoastalRegion,
) -> Result<Vec<OverlayEntry>, String> {
    let localized =
        LOCAL_TEXT_KEYS.map(|name| format!("{TEMPLATE}.json!0x{:08x}", fnv1_lower(name)));
    let [region_name, region_description, city_name, city_description] = localized;
    let registry = format!(
        r#"{TEMPLATE}:{{Enabled:1,regionName:"{region_name}",regionDescription:"{region_description}",regionLoadingImage:"{TEMPLATE}_LoadingBackDrop.png",labels:{{}},cities:{{{SITE}:{{cityName:"{city_name}",cityDescription:"{city_description}",percentageBuildable:"50",residentialDesirability:"3",wind:"3",water:"3",coal:"3",rawOre:"3",crudeOil:"3",tornadoes:"0",earthquakes:"0",rail:"false",shipping:"false",resources:[]}}}}}},"#,
    );
    let mut entries = Vec::new();
    for (source, instance) in [(sources.app, 0x696447ea), (sources.game, 0x7ccc548c)] {
        let resource = id(0x67771f5c, 0x40464200, instance);
        let bytes = read(source, resource)?;
        let source = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        let marker = "CapeTrinity:{Enabled:1,regionName:";
        if source.matches(marker).count() != 1 || source.contains(TEMPLATE) {
            return Err(format!(
                "Cannot unambiguously extend UI registry {resource:?}"
            ));
        }
        let mut result = source.replacen(marker, &format!("{registry}{marker}"), 1);
        if instance == 0x7ccc548c {
            result = single_city_trade_ui(result)?;
        }
        entries.push(OverlayEntry::new(resource, result.into_bytes()));
    }

    let labels = match sources.language {
        RegionLocale::EnUs => [
            "Half Coast",
            "An eight-kilometre coast with one city.",
            "East Shore",
            "A highway crosses the eastern half.",
        ],
        RegionLocale::ZhCn => [
            "半海湾",
            "八公里见方的海岸，设有一座城市。",
            "东岸",
            "一条高速公路贯穿东半部陆地。",
        ],
        RegionLocale::ZhTw => [
            "半海灣",
            "八公里見方的海岸，設有一座城市。",
            "東岸",
            "一條高速公路貫穿東半部陸地。",
        ],
    };
    let stock_table = sources
        .locale
        .entries()
        .iter()
        .find(|entry| {
            entry.id.type_id == 0x0a98eaf0
                && entry.id.instance == fnv1_lower("automatedregiontemplates")
        })
        .ok_or("Stock locale table missing")?;
    let table = id(
        stock_table.id.type_id,
        stock_table.id.group,
        fnv1_lower(TEMPLATE),
    );
    if [
        sources.locale,
        sources.app,
        sources.game,
        sources.ep1,
        sources.terrain,
    ]
    .iter()
    .any(|source| source.entry(table).is_some())
    {
        return Err("New Half Coast locale table collides with an existing table".into());
    }
    let mut locale = String::from("\u{feff}{\r\n");
    for (i, (name, label)) in LOCAL_TEXT_KEYS.into_iter().zip(labels).enumerate() {
        let comma = if i + 1 == LOCAL_TEXT_KEYS.len() {
            ""
        } else {
            ","
        };
        let label = serde_json::to_string(label).map_err(|err| err.to_string())?;
        locale.push_str(&format!(
            "    \"0x{:08x}\": {label}{comma}\r\n",
            fnv1_lower(name)
        ));
    }
    locale.push_str("}\r\n");
    entries.push(OverlayEntry::new(table, locale.into_bytes()));

    let preview = super::preview::isometric_png(generated)?;
    let landscape = super::preview::landscape_png(generated)?;
    for (name, png) in [
        (format!("{TEMPLATE}_IsometricMap"), &preview),
        (format!("{TEMPLATE}_LoadingBackDrop"), &landscape),
        (format!("BrowseSellSheet_{TEMPLATE}"), &landscape),
        (format!("thumb-{TEMPLATE}"), &landscape),
    ] {
        let resource = id(0x2f7d0004, 0, fnv1_lower(&name));
        if sources.app.entry(resource).is_some() || sources.game.entry(resource).is_some() {
            return Err(format!("UI artwork TGI collision {resource:?}"));
        }
        entries.push(OverlayEntry::new(resource, png.clone()));
    }
    Ok(entries)
}

fn single_city_trade_ui(mut source: String) -> Result<String, String> {
    // Same failure points as the retail EdgeWaterBay r3 replay. The new
    // template has no external trade routes and one city to initialize.
    let scoped = format!(
        "b.mRegionData.mRegionTypeName===\"{TEMPLATE}\"&&Object.keys(b.mRegionData.mBoxes).length===1&&Object.prototype.hasOwnProperty.call(b.mRegionData.mBoxes,{SITE})"
    );
    let init = "if(0===b.mRegionData.mTotals.tradeUnits){for(var g in this.mRegionData.mBoxes)";
    let edge = "var q=f[o];b.mRegionData.mCityTradeData[g].connections[q.toCity]=";
    for (marker, replacement) in [
        (
            init,
            format!(
                "if({scoped}&&!b.mRegionData.mCityTradeData[{SITE}])b.mRegionData.mCityTradeData[{SITE}]={{connections:{{}}}};{init}"
            ),
        ),
        (
            edge,
            format!(
                "var q=f[o];if({scoped}&&!Object.prototype.hasOwnProperty.call(b.mRegionData.mBoxes,q.toCity))continue;b.mRegionData.mCityTradeData[g].connections[q.toCity]="
            ),
        ),
    ] {
        if source.matches(marker).count() != 1 {
            return Err("Retail single-city trade callback anchor is ambiguous".into());
        }
        source = source.replacen(marker, &replacement, 1);
    }
    Ok(source)
}

/// Build an offline candidate. Requires an 8192 m-capable city runtime for first claim.
pub fn build_coastal_region(
    sources: &RetailSources<'_>,
    spec: CoastalRegionSpec,
) -> Result<Vec<u8>, String> {
    let generated = generate_coastal_region(spec);
    if sources
        .game
        .entry(id(PROP, 0x40e1c400, HIGHWAY_PATH_ENTRY))
        .is_none()
    {
        return Err("Highway PathEntry missing".into());
    }
    let mut entries = terrain_entries(sources, &generated)?;
    entries.extend(template_entries(sources, &generated)?);
    for entry in &entries {
        if [sources.terrain, sources.ep1, sources.app, sources.game]
            .iter()
            .any(|package| package.entry(entry.id).is_some())
        {
            return Err(format!("New region collides with stock TGI {:?}", entry.id));
        }
    }
    entries.extend(ui_entries(sources, &generated)?);
    if entries
        .iter()
        .map(|entry| entry.id)
        .collect::<HashSet<_>>()
        .len()
        != entries.len()
    {
        return Err("Duplicate region output TGI".into());
    }
    dbpf::write_uncompressed_overlay(&entries).map_err(|e| e.to_string())
}

/// Check the package on disk through the existing native readers.
pub fn verify_coastal_region(
    package: &Package,
    spec: CoastalRegionSpec,
) -> Result<RegionBuildReport, String> {
    let generated = generate_coastal_region(spec);
    let templates = region_3d::parse_region_templates(package);
    let template = templates
        .iter()
        .find(|template| template.name == TEMPLATE)
        .ok_or("New region missing from template catalog")?;
    let bindings = region_bootstrap::catalog(package)?;
    let binding = bindings
        .iter()
        .find(|binding| binding.region == region_group())
        .ok_or("Missing generated region binding")?;
    let metadata: Json = serde_json::from_slice(&read(package, from_key(binding.metadata))?)
        .map_err(|e| e.to_string())?;
    let descriptor = props_at(package, id(PROP, region_group(), 0x51e7a18d))?;
    let Some(Kind::Scalar(Value::String8(region_name))) =
        descriptor.get(0x4ee97f5b).map(|prop| &prop.kind)
    else {
        return Err("Missing terrain region name".into());
    };
    if fnv1_lower(region_name) != region_group()
        || package.entry(city_list_id(region_name)).is_none()
        || descriptor.get(0xfb7a85a0).map(|prop| &prop.kind)
            != Some(&Kind::Scalar(key(id(0, 0, CITY_LIST))))
    {
        return Err("Native terrain CityList lookup does not resolve".into());
    }
    let site = SITE.to_string();
    let plots =
        region_3d::plot_table(package, region_group()).ok_or("Missing generated CityList")?;
    if bindings.len() != 1
        || template.region != Some(region_group())
        || template.cities.len() != 1
        || template.cities[0].0 != site
        || !template.great_works.is_empty()
        || metadata["boxes"].as_array().map(Vec::len) != Some(1)
        || metadata["boxes"][0]["x"] != metadata["map"]["cities"][0]["x"]
        || metadata["boxes"][0]["y"] != metadata["map"]["cities"][0]["y"]
        || plots != (vec![site.clone()], vec![(0.0, 0.0)], vec![city_group()])
        || region_3d::region_visible_size(package, region_group()) != SIDE_METERS as f32
    {
        return Err("Invalid single-slot catalog or CityList".into());
    }
    for (site, environment, size) in [
        ("0", region_group(), 128),
        (site.as_str(), city_group(), ECO_SIDE),
    ] {
        let (_, key) = binding
            .sites
            .iter()
            .find(|(uid, _)| uid == site)
            .ok_or("Generated bootstrap site missing")?;
        let state = decode_state(&read(package, from_key(*key))?)?;
        if binding.sites.len() != 2
            || state.environment != environment
            || state.maps.iter().any(|m| m.size != size)
            || !state.units.is_empty()
            || state.version != 17
            || !super::bootstrap::has_highway(&state, &generated.highway)
        {
            return Err(format!("Invalid state bindings or highway at site {site}"));
        }
        if site != "0" {
            for id in RESOURCE_MAPS {
                if !state
                    .maps
                    .iter()
                    .any(|m| m.id == id && m.values.iter().all(|&v| v == FULL))
                {
                    return Err(format!(
                        "City resource map {id:08X} is missing or below full"
                    ));
                }
            }
            for (id, expected) in [
                (0x6f9f6ab3, &generated.ground),
                (0xab4a0ea0, &generated.water_depth),
            ] {
                if !state
                    .maps
                    .iter()
                    .any(|m| m.id == id && &m.values == expected)
                {
                    return Err(format!(
                        "City surface map {id:08X} does not match generated values"
                    ));
                }
            }
        }
    }

    let actual = crate::region_write::read_region_field(package, region_group())?;
    let start = (REGION_FIELD_PX - HEIGHT_SIDE) / 2;
    if generated
        .heights
        .chunks_exact(HEIGHT_SIDE)
        .enumerate()
        .any(|(y, row)| {
            let offset = (start + y) * REGION_FIELD_PX + start;
            actual[offset..offset + HEIGHT_SIDE] != *row
        })
    {
        return Err("Regional F0 crop does not match the city heightfield".into());
    }
    let min_max = props_at(package, id(PROP, region_group(), water_bounds::INSTANCE))?;
    let expected_bounds =
        water_bounds::from_f0(|instance| read(package, id(F0, region_group(), instance)))?;
    if min_max
        .get(water_bounds::HEIGHT_MIN_MAX)
        .and_then(|p| p.array())
        != Some(expected_bounds.as_slice())
    {
        return Err("Regional water LOD bounds do not match the F0 tiles".into());
    }
    let city_tiles = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == F0 && e.id.group == city_group())
        .collect::<Vec<_>>();
    if city_tiles.len() != 1 {
        return Err("Expected one city height resource".into());
    }
    let city_height = read(package, city_tiles[0].id)?;
    if city_height.len() != 20 + generated.heights.len() * 2
        || city_height[20..]
            .chunks_exact(2)
            .zip(&generated.heights)
            .any(|(raw, &v)| raw != v.to_le_bytes())
    {
        return Err("City F0 does not match the generated field".into());
    }
    for instance in [0x696447ea, 0x7ccc548c] {
        let bytes = read(package, id(0x67771f5c, 0x40464200, instance))?;
        let js = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        let marker = format!("{TEMPLATE}:{{Enabled:1");
        let entry = js
            .split_once(&marker)
            .and_then(|(_, tail)| tail.split_once("CapeTrinity:{"))
            .map(|(entry, _)| entry)
            .ok_or("Generated UI registry entry missing")?;
        let cities = format!("cities:{{{SITE}:{{");
        if !entry.contains(&cities)
            || !["water", "coal", "rawOre", "crudeOil"]
                .iter()
                .all(|name| entry.contains(&format!(r#"{name}:"3""#)))
        {
            return Err("UI city resource ratings or UID do not match bootstrap".into());
        }
    }
    let table = package
        .entries()
        .iter()
        .find(|entry| entry.id.type_id == 0x0a98eaf0 && entry.id.instance == fnv1_lower(TEMPLATE))
        .ok_or("Missing Half Coast locale table")?;
    let body = read(package, table.id)?;
    let json = body
        .strip_prefix(b"\xef\xbb\xbf{\r\n")
        .ok_or("Locale table must have BOM and CRLF")?;
    let locale: Json =
        serde_json::from_slice(&[b"{", json].concat()).map_err(|err| err.to_string())?;
    if LOCAL_TEXT_KEYS.iter().any(|name| {
        let key = format!("0x{:08x}", fnv1_lower(name));
        !locale[&key].as_str().is_some_and(|s| !s.is_empty())
    }) {
        return Err("New locale labels missing from package".into());
    }
    Ok(RegionBuildReport {
        template: TEMPLATE,
        region_environment: region_group(),
        city_environment: city_group(),
        city_uid: SITE,
        region_side_meters: SIDE_METERS,
        city_height_samples_per_side: HEIGHT_SIDE,
        city_eco_cells_per_side: ECO_SIDE,
        resource_value: FULL,
        highway_controls: generated.highway.controls,
        package_entries: package.entries().len(),
    })
}
