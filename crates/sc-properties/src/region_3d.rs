//! 区域 3D 预览数据装配：高度场 + 地面场编码 PNG + 地块/伟工位（含名字链）。
//!
//! 机制来源（docs/re/map-3d-preview-research.md）：
//! - 高度 z(米) = raw/32 − 1024（引擎 VS/CPU 三方实证）；
//! - ED（0x03E421ED）= A8R8G8B8：小端 b0=地下水、b1=森林、b2=土壤；
//!   ExportRegionEcoMapsRecursively 在道路控制区清除 G/B，故 b0==0 可沿用为路网近似。
//! - 伟工位 = EP1 包区域模板 JSON（instance 4529F96F）`map.greatWorks[]`，
//!   与区域组按城市坐标 join；
//! - 名字链 = Game 包注册表 JS（模板名→stringID）→ locale JSON（zh-tw
//!   instance = FNV1_lower("automatedregiontemplates")）查表。
use std::collections::HashMap;

use image::{Rgba, RgbaImage};
use serde::Serialize;

use crate::region_map::{self, SHARED_ED_GRID, SHARED_TILE_GRID};

/// FNV-1（先乘后异或）+ 逐字节 ASCII 小写——引擎 SPIDFromName 同款（L1153188）。
pub fn fnv1_lower(s: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in s.bytes() {
        h = h.wrapping_mul(0x0100_0193);
        h ^= u32::from(b.to_ascii_lowercase());
    }
    h
}

/// Default cMaterialGroundType color faces. Assets stay in the user's App package.
/// ED has a 20-byte header and little-endian A8R8G8B8 pixels (BGRA bytes).
pub fn terrain_detail_pngs(package: &dbpf::Package) -> HashMap<String, Vec<u8>> {
    ["dirt", "grass", "cliff", "sand"].into_iter().zip([
        "terrain_dirtdetail", "terrain_grassdetail", "terrain_cliffx", "terrain_beach",
    ]).filter_map(|(slot, name)| {
        let entry = package.entries().iter().find(|e|
            e.id.type_id == 0x03E4_21ED && e.id.group == 0 && e.id.instance == fnv1_lower(name))?;
        let bytes = package.read(entry).ok()?;
        if bytes.len() != 20 + 256 * 256 * 4 { return None; }
        let pixels: Vec<u8> = bytes[20..].chunks_exact(4)
            .flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        let image = RgbaImage::from_raw(256, 256, pixels)?;
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image).write_to(
            &mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
        Some((slot.to_string(), png))
    }).collect()
}

/// 4096² u16 高度场拼合（tile 头 20 B 跳过）。
pub fn assembled_height(package: &dbpf::Package, group: u32) -> Option<Vec<u16>> {
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let d = package.read(e).ok()?;
            if d.len() == 131092 {
                tiles.insert(
                    e.id.instance,
                    d[20..]
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect(),
                );
            }
        }
    }
    let mut hgt = vec![0u16; 4096 * 4096];
    for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            let px = tiles.get(inst)?;
            for y in 0..256 {
                hgt[(ty * 256 + y) * 4096 + tx * 256..(ty * 256 + y) * 4096 + tx * 256 + 256]
                    .copy_from_slice(&px[y * 256..(y + 1) * 256]);
            }
        }
    }
    Some(hgt)
}

/// ED 三通道拼合（2048² @16m/格）：b0=地下水、b1=森林、b2=土壤。
pub fn assembled_ed(package: &dbpf::Package, group: u32) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let mut b0 = vec![0u8; 2048 * 2048];
    let mut b1 = vec![0u8; 2048 * 2048];
    let mut b2 = vec![0u8; 2048 * 2048];
    for (ty, row) in SHARED_ED_GRID.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            let e = package
                .entries()
                .iter()
                .find(|e| e.id.type_id == 0x03E4_21ED && e.id.group == group && e.id.instance == *inst)?;
            let d = package.read(e).ok()?;
            if d.len() != 65556 {
                return None;
            }
            for y in 0..128 {
                for x in 0..128 {
                    let o = 20 + (y * 128 + x) * 4;
                    let v = u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
                    let k = (ty * 128 + y) * 2048 + tx * 128 + x;
                    b0[k] = (v & 0xFF) as u8;
                    b1[k] = ((v >> 8) & 0xFF) as u8;
                    b2[k] = ((v >> 16) & 0xFF) as u8;
                }
            }
        }
    }
    Some((b0, b1, b2))
}

/// 地块（城市或伟工位）。
#[derive(Debug, Clone, Serialize)]
pub struct Region3DPlot {
    /// 世界坐标（米）。
    pub x: f32,
    pub y: f32,
    /// 场地基准面高（米）。城市来自模板 JSON z；伟工位 = 高度图采样。
    pub z: f32,
    /// 地块 uid（地块表名字串，如 "1026"）；伟工位为空。
    pub uid: String,
    /// 本地化名（城市名/区域伟工标签；链路缺失时 None）。
    pub name: Option<String>,
    #[serde(rename = "nameEn")]
    pub name_en: Option<String>,
    /// "city" | "greatwork"。
    pub kind: String,
}

/// 3D 预览数据包（使用游戏区域属性定义的居中可见范围）。
#[derive(Debug, Clone, Serialize)]
pub struct Region3D {
    /// 高度 PNG（RGBA，2048²）：R=raw 高 8 位、G=raw 低 8 位（无损 16-bit 编码）。
    pub height_png: Vec<u8>,
    /// 地面 PNG（RGBA）：R=土壤 b2、G=森林 b1、B=地下水 b0、A=255。
    /// 数据纹理，不能做 sRGB 解码。保持 A=255 避免 canvas 预乘丢失生态值。
    pub ground_png: Vec<u8>,
    /// 场边长（2048）。
    pub size: u32,
    /// 每像素米数（16）。
    pub meters_per_pixel: f32,
    /// 场左上角世界坐标（-16384, -16384）。
    pub origin_world: (f32, f32),
    /// 水面世界高（米，-870）。
    pub water_z: f32,
    /// 高度解码常数：z = raw / 32 - 1024。
    pub height_div: f32,
    pub height_bias: f32,
    pub plots: Vec<Region3DPlot>,
    pub desert: bool,
    pub display_name: Option<String>,
    pub display_name_en: Option<String>,
    /// 资源画刷清单（目标 map 名 + stamp 世界坐标）——侧栏清单用。
    pub brushes: Vec<(String, Vec<(f32, f32)>)>,
}

/// RT0/RT1 区域组 → 模板名先验（z 逐城判别的结论，仅作平手终裁与断链回退）。
/// 同布局组（16 城三图/7 城双图）曾全靠这条表纠偏；详见 docs/re/map-3d-preview-research.md §2。
const TEMPLATE_PRIOR: &[(u32, &str)] = &[
    (0xBEAF_0510, "Confluence"),
    (0x9F73_5B20, "Oasis"),
    (0xD01F_A985, "Horizon"),
    (0x9E9B_1FF0, "CaspianLake"),
    (0xC2A9_C48F, "CapeTrinity"),
    (0xB12D_E348, "LittleGorge"),
    (0xBC35_7A2B, "Sawyer"),
    (0xC041_82E4, "Gallia"),
    (0xA0B6_0DDE, "EdgeWaterBay"),
    (0xE41A_82B8, "Reflection"),
    (0xE018_3D94, "TwinCities"),
    (0xDB25_018C, "Tutorial"),
];

fn template_prior(group: u32) -> Option<&'static str> {
    TEMPLATE_PRIOR
        .iter()
        .find(|(g, _)| *g == group)
        .map(|(_, n)| *n)
}

/// 模板 ↔ 区域 join。同布局区域（16 城三图/7 城三图/2 城双图）坐标完全相同，
/// 用「城市场地整平高 vs 模板 z」逐城判别；仍平手时由先验英文名终裁。
fn match_template<'a>(
    templates: &'a [RegionTemplate],
    uids: &[String],
    pos: &[(f32, f32)],
    ids: &[u32],
    package: &dbpf::Package,
    prior_en: Option<&str>,
) -> Option<&'a RegionTemplate> {
    let mut site_z: Vec<Option<f64>> = vec![None; uids.len()];
    let uid_row: HashMap<&str, usize> = uids
        .iter()
        .enumerate()
        .map(|(i, u)| (u.as_str(), i))
        .collect();
    templates
        .iter()
        .map(|t| {
            let mut pos_matches = 0usize;
            let mut z_matches = 0usize;
            for (uid, x, y, tz) in &t.cities {
                if pos.iter()
                    .any(|(px, py)| {
                        (*px - *x as f32).abs() < 4.0 && (*py - *y as f32).abs() < 4.0
                    })
                {
                    pos_matches += 1;
                }
                if let Some(tz) = tz {
                    if let Some(row) = uid_row.get(uid.as_str()).copied() {
                        if site_z[row].is_none() && row < ids.len() {
                            site_z[row] = city_site_z(package, ids[row]);
                        }
                        if let Some(z) = site_z[row] {
                            if (z - tz).abs() < 15.0 {
                                z_matches += 1;
                            }
                        }
                    }
                }
            }
            let prior_bonus = if prior_en == Some(t.name.as_str()) { 1 } else { 0 };
            (
                pos_matches * 1_000_000 + z_matches * 1_000 + t.cities.len() + prior_bonus,
                t,
            )
        })
        .filter(|(s, _)| *s > 1_000_000)
        .max_by_key(|(s, _)| *s)
        .map(|(_, t)| t)
}

/// 注册表查找：全名 → 归一化（去 `_`/`-` 小写）→ SC_ 前缀剥离 → 包含。
/// （模板名 "SC_Desolation" ↔ 注册表键 "Desolation" 这类差异）
fn registry_lookup<'a>(
    registry: &'a HashMap<String, (u32, HashMap<String, u32>)>,
    template_name: &str,
) -> Option<&'a (u32, HashMap<String, u32>)> {
    let norm = |v: &str| v.to_lowercase().replace(['_', '-'], "");
    let exact = registry.get(template_name);
    if exact.is_some() {
        return exact;
    }
    let tn = norm(template_name);
    if let Some(hit) = registry.iter().find(|(k, _)| norm(k) == tn) {
        return Some(hit.1);
    }
    let stripped = tn.strip_prefix("sc").unwrap_or(&tn).to_string();
    if let Some(hit) = registry.iter().find(|(k, _)| norm(k) == stripped) {
        return Some(hit.1);
    }
    registry
        .iter()
        .find(|(k, _)| {
            let nk = norm(k);
            nk.contains(&tn) || tn.contains(&nk)
        })
        .map(|(_, v)| v)
}

/// 区域地块表（uid + 坐标 + 城市组 id，三数组按行平行，按城市组交集选表）。
pub fn plot_table(
    package: &dbpf::Package,
    group: u32,
) -> Option<(Vec<String>, Vec<(f32, f32)>, Vec<u32>)> {
    let mut city_ids: Vec<u32> = Vec::new();
    for e in package.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 {
            continue;
        }
        let Ok(pf) = crate::PropertyFile::parse(&package.read(e).ok()?) else { continue };
        if let Some(crate::Property { kind: crate::Kind::Scalar(crate::Value::Key(k)), .. }) =
            pf.get(0xC194_9C4D)
        {
            if k.instance == 0x51E7_A18D && k.group == group {
                city_ids.push(e.id.group);
            }
        }
    }
    let set: std::collections::HashSet<u32> = city_ids.iter().copied().collect();
    let mut best: Option<(usize, Vec<String>, Vec<(f32, f32)>, Vec<u32>)> = None;
    for e in package.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C {
            continue;
        }
        let Ok(pt) = crate::PropertyFile::parse(&package.read(e).ok()?) else { continue };
        let ids: Vec<u32> = match pt.get(0x16B7_B1EF) {
            Some(crate::Property { kind: crate::Kind::Array(vs), .. }) => vs
                .iter()
                .filter_map(|v| match v {
                    crate::Value::UInt32(x) => Some(*x),
                    _ => None,
                })
                .collect(),
            _ => continue,
        };
        let ov = ids.iter().filter(|i| set.contains(i)).count();
        if best.as_ref().map(|(b, _, _, _)| ov > *b).unwrap_or(true) {
            let uids: Vec<String> = match pt.get(0x0543_BC96) {
                Some(crate::Property { kind: crate::Kind::Array(vs), .. }) => vs
                    .iter()
                    .filter_map(|v| match v {
                        crate::Value::String8(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            };
            let pos: Vec<(f32, f32)> = match pt.get(0xF01D_E4B1) {
                Some(crate::Property { kind: crate::Kind::Array(vs), .. }) => vs
                    .iter()
                    .filter_map(|v| match v {
                        crate::Value::Vector2(v) => Some((v[0], v[1])),
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            };
            best = Some((ov, uids, pos, ids));
        }
    }
    best.map(|(_, u, p, i)| (u, p, i))
}

/// 城市地块场地基准面高（该城市组 256² 高度图均值，抽样 1/7）。
pub fn city_site_z(package: &dbpf::Package, city_group: u32) -> Option<f64> {
    let mut sum = 0f64;
    let mut n = 0u64;
    for e in package.entries() {
        if e.id.type_id != 0x03E4_21F0 || e.id.group != city_group {
            continue;
        }
        let d = package.read(e).ok()?;
        if d.len() != 131092 {
            continue;
        }
        for i in (0..65536).step_by(7) {
            sum += f64::from(u16::from_le_bytes([d[20 + i * 2], d[21 + i * 2]]));
            n += 1;
        }
    }
    if n == 0 {
        return None;
    }
    Some(sum / n as f64 / 32.0 - 1024.0)
}

/// EP1 区域模板（AutomatedRegionTemplates.json）。
#[derive(Debug, Clone)]
pub struct RegionTemplate {
    pub name: String,
    /// (uid, x, y, z)
    pub cities: Vec<(String, f64, f64, Option<f64>)>,
    /// (name, x, y)
    pub great_works: Vec<(String, f64, f64)>,
}

/// 解析 EP1 包内全部区域模板（type 0x0A98EAF0, instance 0x4529F96F）。
pub fn parse_region_templates(ep1: &dbpf::Package) -> Vec<RegionTemplate> {
    let mut out = Vec::new();
    for e in ep1.entries() {
        if e.id.type_id != 0x0A98_EAF0 || e.id.instance != 0x4529_F96F {
            continue;
        }
        let Ok(d) = ep1.read(e) else { continue };
        let Ok(txt) = std::str::from_utf8(&d) else { continue };
        let txt = txt.trim_start_matches(|c: char| c != '{');
        let Ok(v) = serde_json::from_str::<serde_json::Value>(txt) else { continue };
        let name = v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let mut cities = Vec::new();
        let mut gws = Vec::new();
        if let Some(map) = v.get("map") {
            if let Some(list) = map.get("cities").and_then(|c| c.as_array()) {
                for c in list {
                    let uid = c.get("uid").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let x = c.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let y = c.get("y").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let z = c.get("z").and_then(|x| x.as_f64());
                    cities.push((uid, x, y, z));
                }
            }
            if let Some(list) = map.get("greatWorks").and_then(|c| c.as_array()) {
                for g in list {
                    let n = g.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let x = g.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let y = g.get("y").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    gws.push((n, x, y));
                }
            }
        }
        if !name.is_empty() {
            out.push(RegionTemplate { name, cities, great_works: gws });
        }
    }
    out
}

/// Game 包注册表扫描：模板名 → (区域名 stringID, uid → 城市名 stringID)。
/// 注册表为 Closure 编译 JS（非 JSON），按字面模式扫描：
/// `Name:{Enabled:1,regionName:"AutomatedRegionTemplates.json!0x…"` 与
/// `uid:{cityName:"…!0x…"`。
pub fn parse_template_registry(game: &dbpf::Package) -> HashMap<String, (u32, HashMap<String, u32>)> {
    let mut out = HashMap::new();
    for e in game.entries() {
        if e.id.type_id != 0x6777_1F5C {
            continue;
        }
        let Ok(d) = game.read(e) else { continue };
        let Ok(js) = std::str::from_utf8(&d) else { continue };
        let marker = "regionName:\"AutomatedRegionTemplates.json!0x";
        let mut entries: Vec<(usize, String, u32)> = Vec::new(); // (pos, 模板名, 区域名ID)
        let mut from = 0usize;
        while let Some(rel) = js[from..].find(marker) {
            let pos = from + rel;
            let hex = &js[pos + marker.len()..(pos + marker.len() + 8).min(js.len())];
            if let Ok(id) = u32::from_str_radix(hex.trim_end_matches(|c: char| !c.is_ascii_hexdigit()), 16) {
                // 回溯模板名：前一 entry 结束符 `},` 之后、`:{Enabled:1,` 之前
                let head = &js[..pos];
                let name_start = head.rfind("},").map(|i| i + 2).unwrap_or(0);
                let seg = head[name_start..].trim_start_matches(['\n', '\r', ' ']);
                if let Some(colon) = seg.find(":{Enabled:1,") {
                    entries.push((pos, seg[..colon].trim().to_string(), id));
                }
            }
            from = pos + marker.len();
        }
        if entries.is_empty() {
            continue;
        }
        // 每个条目的城市段 = 本条起点到下一条起点
        for (i, (pos, name, region_id)) in entries.iter().enumerate() {
            let end = entries.get(i + 1).map(|(p, _, _)| *p).unwrap_or(js.len());
            let seg = &js[*pos..end];
            let mut cities = HashMap::new();
            let cmarker = "cityName:\"AutomatedRegionTemplates.json!0x";
            let mut from = 0usize;
            while let Some(rel) = seg[from..].find(cmarker) {
                let p = from + rel;
                let hex = &seg[p + cmarker.len()..(p + cmarker.len() + 8).min(seg.len())];
                if let Ok(id) = u32::from_str_radix(hex.trim_end_matches(|c: char| !c.is_ascii_hexdigit()), 16) {
                    // uid 形如 `,1026:{cityName`——向前取连续数字
                    let head = &seg[..p];
                    let mut uid = String::new();
                    for c in head.chars().rev() {
                        if c.is_ascii_digit() {
                            uid.insert(0, c);
                        } else if !uid.is_empty() {
                            break;
                        }
                    }
                    if !uid.is_empty() {
                        cities.insert(uid, id);
                    }
                }
                from = p + cmarker.len();
            }
            out.insert(name.clone(), (*region_id, cities));
        }
        break; // 单条注册表资源即全量
    }
    out
}

/// locale JSON（stringID → 文本）。取 instance = FNV1_lower("automatedregiontemplates")
/// 的 type 0x0A98EAF0 资源。
pub fn parse_locale_names(locale_pkg: &dbpf::Package) -> HashMap<u32, String> {
    let inst = fnv1_lower("automatedregiontemplates");
    let mut out = HashMap::new();
    for e in locale_pkg.entries() {
        if e.id.type_id != 0x0A98_EAF0 || e.id.instance != inst {
            continue;
        }
        let Ok(d) = locale_pkg.read(e) else { continue };
        let Ok(txt) = std::str::from_utf8(&d) else { continue };
        let txt = txt.trim_start_matches(|c: char| c != '{');
        let Ok(v) = serde_json::from_str::<serde_json::Value>(txt) else { continue };
        if let Some(obj) = v.as_object() {
            for (k, val) in obj {
                let hex = k.trim_start_matches("0x");
                if let Ok(id) = u32::from_str_radix(hex, 16) {
                    if let Some(s) = val.as_str() {
                        out.insert(id, s.to_string());
                    }
                }
            }
        }
    }
    out
}

/// Retail English names keyed by the same registry string IDs as localized names.
/// Kept as a small fallback for installations that ship only a Chinese locale.
fn english_region_name(id: u32) -> Option<String> {
    static NAMES: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| serde_json::from_str(include_str!("region_names_en.json"))
        .expect("valid bundled English region names")).get(&format!("{id:08X}")).cloned()
}

/// 区域列表项（真名链解析后）。
#[derive(Debug, Clone, Serialize)]
pub struct RegionListItem {
    pub group: u32,
    pub numeric_id: String,
    pub plot_count: usize,
    pub display_name: Option<String>,
    pub display_name_en: Option<String>,
}

/// 区域枚举（名字链：模板 join → 注册表 → zh locale；en 回退模板名）。
pub fn region_list_named(
    package: &dbpf::Package,
    ep1: Option<&dbpf::Package>,
    game: Option<&dbpf::Package>,
    locale: Option<&dbpf::Package>,
) -> Vec<RegionListItem> {
    let base = region_map::list_regions(package);
    if base.is_empty() {
        return Vec::new();
    }
    let templates = ep1.map(parse_region_templates).unwrap_or_default();
    let registry = game.map(parse_template_registry).unwrap_or_default();
    let locale_names = locale.map(parse_locale_names).unwrap_or_default();
    let has_locale = locale.is_some();
    base.into_iter()
        .map(|r| {
            let (uids, pos, ids) = plot_table(package, r.group).unwrap_or_default();
            let matched = match_template(
                &templates,
                &uids,
                &pos,
                &ids,
                package,
                template_prior(r.group),
            );
            let reg = matched.and_then(|t| registry_lookup(&registry, &t.name).cloned());
            let zh = if has_locale {
                reg.as_ref().and_then(|(id, _)| locale_names.get(id).cloned())
            } else {
                None
            };
            let display_name =
                zh.or_else(|| region_map::region_display_name(r.group, true).map(|s| s.to_string()));
            let display_name_en = reg.as_ref().and_then(|(id, _)| english_region_name(*id))
                .or_else(|| matched.map(|t| t.name.clone()))
                .or_else(|| region_map::region_display_name(r.group, false).map(|s| s.to_string()));
            RegionListItem {
                group: r.group,
                numeric_id: r.numeric_id,
                plot_count: r.plot_count,
                display_name,
                display_name_en,
            }
        })
        .collect()
}

/// Game visible-area property (cTerrainRegion::FillFromProps, 0x4261B0).
/// Invalid/missing values fall back to the full source field, never plot extents.
pub fn region_visible_size(package: &dbpf::Package, group: u32) -> f32 {
    let value = package.entries().iter()
        .find(|e| e.id.type_id == 0x00B1_B104 && e.id.group == group && e.id.instance == 0x51E7_A18D)
        .and_then(|e| package.read(e).ok())
        .and_then(|bytes| crate::PropertyFile::parse(&bytes).ok())
        .and_then(|pf| match pf.get(1581061855).map(|p| &p.kind) {
            Some(crate::Kind::Scalar(crate::Value::Int32(v))) => Some(*v),
            _ => None,
        });
    validated_visible_size(value) as f32
}

fn validated_visible_size(value: Option<i32>) -> i32 {
    // Current mesh chunks are 2048m. Only accept supported aligned extents.
    value.filter(|v| (2048..=32768).contains(v) && v % 4096 == 0).unwrap_or(32768)
}

#[cfg(test)]
mod visible_bounds_tests {
    use super::validated_visible_size;

    #[test]
    fn english_names_resolve_registry_ids_and_serialize_for_viewer() {
        assert_eq!(super::english_region_name(0x6779B9D2).as_deref(), Some("Trinity Point"));
        assert_eq!(super::english_region_name(0xFC0C91CF).as_deref(), Some("Clearwater"));
        assert_eq!(super::english_region_name(0xCF049F2D).as_deref(), Some("Norwich Hills"));
        assert_eq!(super::english_region_name(0), None);
        let plot = super::Region3DPlot {
            x: 0.0, y: 0.0, z: 0.0, uid: "1027".into(),
            name: Some("清水".into()), name_en: super::english_region_name(0xFC0C91CF),
            kind: "city".into(),
        };
        let json = serde_json::to_value(plot).unwrap();
        assert_eq!(json["nameEn"], "Clearwater");
        assert_eq!(json["name"], "清水");
    }

    #[test]
    fn shipped_region_extents_map_to_safe_centered_source_windows() {
        for side in [8192, 16384, 32768] {
            let side = validated_visible_size(Some(side));
            let first = (32768 - side) / 16; // source 8m pixels at -side/2
            let last = first + side / 8 - 1;
            assert!(first >= 0 && last < 4096);
            assert_eq!(first + last, 4095);
        }
    }

    #[test]
    fn malformed_or_missing_extent_preserves_the_full_field() {
        for value in [None, Some(0), Some(-8192), Some(65536), Some(8193)] {
            assert_eq!(validated_visible_size(value), 32768);
        }
    }
}

/// 装配区域 3D 数据。
///
/// `ep1`/`game`/`locale` 为可选旁包（EP1 模板 / Game 注册表 / locale 包），
/// 缺失时对应能力降级（无伟工位 / 无名字）。
pub fn region_3d(
    package: &dbpf::Package,
    group: u32,
    ep1: Option<&dbpf::Package>,
    game: Option<&dbpf::Package>,
    locale: Option<&dbpf::Package>,
) -> Result<Region3D, Box<dyn std::error::Error>> {
    let hgt = assembled_height(package, group)
        .ok_or("区域高度场拼合失败（F0 tile 不足 341）")?;
    let (b0, b1, b2) =
        assembled_ed(package, group).ok_or("区域 ED 场拼合失败（ED tile 不足 341）")?;
    let sea = region_map::region_water_level(package, group)
        .unwrap_or(region_map::WATER_LEVEL);
    let sea_z = sea as f32 / 32.0 - 1024.0;

    // 先取地块表 + 模板 join（伟工位），据包围盒裁剪装配窗口
    let (uids, pos, ids) = plot_table(package, group).unwrap_or_default();
    let templates = ep1.map(parse_region_templates).unwrap_or_default();
    let matched0 = match_template(
        &templates,
        &uids,
        &pos,
        &ids,
        package,
        template_prior(group),
    );
    // cTerrainRegion::FillFromProps / VisibleRegionContainsXY: the visible
    // square is centered at world origin; it is independent of plot bounds.
    let visible_size = region_visible_size(package, group);
    let sx0 = -visible_size / 2.0;
    let sy0 = sx0;
    let sx1 = visible_size / 2.0;
    // 窗口（4096 空间 8m/px 起点 + 输出 16m/px 宽度）
    let px0 = ((sx0 + 16384.0) / 8.0) as usize;
    let py0 = ((sy0 + 16384.0) / 8.0) as usize;
    let S = (((sx1 - sx0) / 16.0) as usize).max(1);
    let mut hp = RgbaImage::new(S as u32, S as u32);
    let mut gp = RgbaImage::new(S as u32, S as u32);
    let mut land_grass = 0f64;
    let mut land_n = 0f64;
    for y in 0..S {
        for x in 0..S {
            let fx = px0 + x * 2;
            let fy = py0 + y * 2;
            let raw = (u32::from(hgt[fy * 4096 + fx])
                + u32::from(hgt[fy * 4096 + fx + 1])
                + u32::from(hgt[(fy + 1) * 4096 + fx])
                + u32::from(hgt[(fy + 1) * 4096 + fx + 1]))
                / 4;
            hp.put_pixel(
                x as u32,
                y as u32,
                Rgba([((raw >> 8) & 0xFF) as u8, (raw & 0xFF) as u8, 0, 255]),
            );
            // ED 2048 空间直接取子矩形
            let ex = px0 / 2 + x;
            let ey = py0 / 2 + y;
            let k = ey * 2048 + ex;
            gp.put_pixel(x as u32, y as u32, Rgba([b2[k], b1[k], b0[k], 255]));
            if raw as i32 > sea + 100 && raw > 0 {
                // HLSL getGrassAmount: sqrt(soil * waterTable), not forest density.
                land_grass += (f64::from(b2[k]) * f64::from(b0[k])).sqrt() / 255.0;
                land_n += 1.0;
            }
        }
    }
    // Descriptive metadata only; the renderer evaluates each texel independently.
    let desert = if land_n > 0.0 { land_grass / land_n < 20.0 / 255.0 } else { false };
    let mut height_png = Vec::new();
    image::DynamicImage::ImageRgba8(hp).write_to(
        &mut std::io::Cursor::new(&mut height_png),
        image::ImageFormat::Png,
    )?;
    let mut ground_png = Vec::new();
    image::DynamicImage::ImageRgba8(gp).write_to(
        &mut std::io::Cursor::new(&mut ground_png),
        image::ImageFormat::Png,
    )?;

    // 高度采样（16m/格，2×2 均值，窗口局部坐标）
    let sample_z = |wx: f64, wy: f64| -> f32 {
        let cx = ((wx - sx0 as f64) / 16.0).round() as isize;
        let cy = ((wy - sy0 as f64) / 16.0).round() as isize;
        if cx < 0 || cy < 0 || cx >= S as isize || cy >= S as isize {
            return sea_z;
        }
        let fx = px0 + cx as usize * 2;
        let fy = py0 + cy as usize * 2;
        let raw = (u32::from(hgt[fy * 4096 + fx])
            + u32::from(hgt[fy * 4096 + fx + 1])
            + u32::from(hgt[(fy + 1) * 4096 + fx])
            + u32::from(hgt[(fy + 1) * 4096 + fx + 1]))
            / 4;
        raw as f32 / 32.0 - 1024.0
    };

    // 名字链（地块/模板已在上方 join）
    let matched = matched0;
    let registry = game.map(parse_template_registry).unwrap_or_default();
    let locale_names = locale.map(parse_locale_names).unwrap_or_default();
    let reg = matched.and_then(|t| registry_lookup(&registry, &t.name).cloned());

    let mut plots = Vec::new();
    let gw_z = |x: f64, y: f64| sample_z(x, y);
    for (uid, (px, py)) in uids.iter().zip(pos.iter()) {
        let tpl_city = matched
            .and_then(|t| t.cities.iter().find(|(u, _, _, _)| u == uid));
        let z = tpl_city
            .and_then(|(_, _, _, z)| *z)
            .map(|z| z as f32)
            .unwrap_or_else(|| sample_z(*px as f64, *py as f64));
        let name = reg
            .as_ref()
            .and_then(|(_, cities)| cities.get(uid))
            .and_then(|id| locale_names.get(id))
            .cloned();
        plots.push(Region3DPlot {
            x: *px,
            y: *py,
            z,
            uid: uid.clone(),
            name,
            name_en: reg.as_ref().and_then(|(_, cities)| cities.get(uid))
                .and_then(|id| english_region_name(*id)),
            kind: "city".into(),
        });
    }
    if let Some(t) = matched {
        for (n, x, y) in &t.great_works {
            plots.push(Region3DPlot {
                x: *x as f32,
                y: *y as f32,
                z: gw_z(*x, *y),
                uid: String::new(),
                name: region_map::region_display_name(group, true).map(|_| "伟大工程位".to_string()),
                name_en: None, // Generic site label is translated by the viewer.
                kind: "greatwork".into(),
            });
        }
    }

    // 区域名：名字链优先（regionName stringID → zh locale），英文名回退模板名
    let (display_name, display_name_en) = match (&reg, locale.is_some()) {
        (Some(reg), true) => {
            let zh = locale_names.get(&reg.0).cloned();
            let en = english_region_name(reg.0).or_else(|| matched.map(|t| t.name.clone()));
            (zh, en)
        }
        _ => (
            region_map::region_display_name(group, true).map(|s| s.to_string()),
            region_map::region_display_name(group, false).map(|s| s.to_string()),
        ),
    };

    Ok(Region3D {
        height_png,
        ground_png,
        size: S as u32,
        meters_per_pixel: 16.0,
        origin_world: (sx0, sy0),
        water_z: sea_z,
        height_div: 32.0,
        height_bias: -1024.0,
        plots,
        desert,
        display_name,
        display_name_en,
        brushes: region_map::resource_brushes(package, group),
    })
}
