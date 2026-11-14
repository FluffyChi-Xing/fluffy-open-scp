//! 生成器辅助：为 extra-containers（公共交通/公園等运行时装配分类）提取
//! 菜单条目的完整元数据 —— 槽位图标、rollover 大图、名称/描述（含 DLC
//! 独立 locale 表）、统计行（unitEffectTitle 模板 + unitEffectParameter
//! 指向 menuUnit 的 EcoGame Unit 条目上的 float 值，cToolPlop/BuildToolJSValue
//! 反编译定案）。产出 JSON 供 tools/generate-extra-containers.cjs 合并。
//!
//! 用法：cargo run -p probes --release --example menu_asset_extract --
//!   <assets-dir> <meta-out.json> <locale-Data.package> <package...> -- <instance...>
use sc_properties::{Kind, PropertyFile, Value};
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::PathBuf;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const TYPE_PNG: u32 = 0x2F7D_0004;
const TYPE_JPG: u32 = 0x3F86_62EA;
const TYPE_LOCALE: u32 = 0x0A98_EAF0;
const ICON_GROUP: u32 = 0x40E0_2400;
const UNIT_GROUPS: [u32; 2] = [0x40E0_C000, 0x48E0_C000];

const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_DESC: u32 = 0x0A09_F5FB;
const HASH_MENU_UNIT: u32 = 0x0975_695F;
const HASH_TOOL_ICON_KEY: u32 = 0x0977_AA8F;
/// 分区类工具的槽位图标键（zoning 等无 0x0977AA8F 的条目用）。
const HASH_PALETTE_ICON_KEY: u32 = 0x0C65_D729;
const HASH_EFFECT_TITLE: u32 = 0x0EB1_FC05;
const HASH_EFFECT_PARAM: u32 = 0x0EB1_FC22;

struct LocaleTables {
    tables: BTreeMap<u32, BTreeMap<String, String>>,
}

impl LocaleTables {
    fn load(package: &dbpf::Package) -> Self {
        let mut tables = BTreeMap::new();
        for entry in package.entries() {
            if entry.id.type_id != TYPE_LOCALE {
                continue;
            }
            let Ok(data) = package.read(entry) else {
                continue;
            };
            let text = String::from_utf8_lossy(&data);
            let text = text.trim_start_matches('\u{feff}');
            let Ok(parsed) = serde_json::from_str::<BTreeMap<String, String>>(text) else {
                continue;
            };
            tables.insert(entry.id.instance, parsed);
        }
        Self { tables }
    }

    /// Text 引用 → 字符串（0x 前缀小写键，与提取产物一致）。
    fn resolve(&self, table: u32, instance: u32) -> Option<String> {
        self.tables
            .get(&table)?
            .get(&format!("0x{:08x}", instance))
            .cloned()
    }
}

fn texts_of(file: &PropertyFile, hash: u32) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    if let Some(entry) = file.get(hash) {
        let values: Vec<&Value> = match &entry.kind {
            Kind::Scalar(v) => vec![v],
            Kind::Array(values) => values.iter().collect(),
            Kind::Empty => Vec::new(),
        };
        for v in values {
            if let Value::Text(t) = v {
                out.push((t.table_id, t.instance_id));
            }
        }
    }
    out
}

fn u32s_of(file: &PropertyFile, hash: u32) -> Vec<u32> {
    if let Some(entry) = file.get(hash) {
        match &entry.kind {
            Kind::Scalar(Value::UInt32(v)) => return vec![*v],
            Kind::Array(values) => {
                return values
                    .iter()
                    .filter_map(|v| match v {
                        Value::UInt32(n) => Some(*n),
                        _ => None,
                    })
                    .collect();
            }
            _ => {}
        }
    }
    Vec::new()
}

fn key_instance(file: &PropertyFile, hash: u32) -> Option<u32> {
    if let Some(entry) = file.get(hash) {
        match &entry.kind {
            Kind::Scalar(Value::Key(k)) => return Some(k.instance),
            Kind::Array(values) => {
                return values.iter().find_map(|v| match v {
                    Value::Key(k) => Some(k.instance),
                    _ => None,
                });
            }
            _ => {}
        }
    }
    None
}

fn float_of(file: &PropertyFile, hash: u32) -> Option<f32> {
    let entry = file.get(hash)?;
    let values: Vec<&Value> = match &entry.kind {
        Kind::Scalar(v) => vec![v],
        Kind::Array(values) => values.iter().collect(),
        Kind::Empty => return None,
    };
    values.iter().find_map(|v| match v {
        Value::Float(f) => Some(*f),
        Value::Int32(n) => Some(*n as f32),
        Value::UInt32(n) => Some(*n as f32),
        _ => None,
    })
}

/// "<B>公車數量</B>：~amount:number~" → ("公車數量", " 個模擬市民" 等后缀)
fn split_effect_template(template: &str) -> (String, String) {
    let cleaned = template
        .replace("<B>", "")
        .replace("</B>", "")
        .replace("<b>", "")
        .replace("</b>", "");
    match cleaned.find("~amount") {
        Some(pos) => {
            let label = cleaned[..pos].to_string();
            // 跳过 ~amount:number~ 令牌本体，保留其后缀（如「個模擬市民」）
            let after = match cleaned[pos + 1..].find('~') {
                Some(end) => cleaned[pos + end + 2..].to_string(),
                None => String::new(),
            };
            (label, after)
        }
        None => (cleaned.clone(), String::new()),
    }
}

fn trim_label(label: &str) -> String {
    label
        .trim_end_matches('：')
        .trim_end_matches(':')
        .trim()
        .to_string()
}

fn format_amount(value: f32) -> String {
    if value.fract() == 0.0 && value.abs() < 1e9 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let assets_dir = PathBuf::from(args.next().expect("usage: menu_asset_extract <assets-dir> <meta-out.json> <locale-package> <packages...> -- <instances...>"));
    let meta_out = PathBuf::from(args.next().expect("missing meta-out.json"));
    let locale_package_path = args.next().expect("missing locale Data.package");
    let mut packages = Vec::new();
    let mut instances = Vec::new();
    let mut after_dashdash = false;
    for arg in args {
        if arg == "--" {
            after_dashdash = true;
            continue;
        }
        if after_dashdash {
            instances
                .push(u32::from_str_radix(arg.trim_start_matches("0x"), 16).expect("bad instance"));
        } else {
            packages.push(arg);
        }
    }
    assert!(!instances.is_empty(), "no instances given");
    std::fs::create_dir_all(&assets_dir).expect("create assets dir");

    // locale 表只从语言包读（6C969DEE 基础表与 DLC 表 7B45C298 都在其中）
    let locale_package = dbpf::Package::open(&locale_package_path)
        .unwrap_or_else(|e| panic!("open locale package: {e:?}"));
    let locales = LocaleTables::load(&locale_package);

    // 打开全部资源包（含 EcoGame 脚本包 —— Unit 条目在这里）
    let mut opened = Vec::new();
    for path in &packages {
        if let Ok(package) = dbpf::Package::open(path) {
            opened.push(package);
        } else {
            eprintln!("skip {path}");
        }
    }

    let read_property = |instances_of: &dyn Fn(u32) -> bool,
                         groups: &[u32],
                         type_match: &dyn Fn(u32, u32) -> bool|
     -> Vec<(u32, u32, Vec<u8>)> {
        let mut out = Vec::new();
        for package in &opened {
            for entry in package.entries() {
                if entry.id.type_id != TYPE_PROPERTY || !instances_of(entry.id.instance) {
                    continue;
                }
                if !groups.is_empty() && !groups.contains(&entry.id.group) {
                    continue;
                }
                let _ = type_match;
                if let Ok(data) = package.read(entry) {
                    out.push((entry.id.group, entry.id.instance, data));
                }
            }
        }
        out
    };

    // 1) 菜单条目本体
    let wanted = |x: u32| instances.contains(&x);
    let menu_entries = read_property(&wanted, &[], &|_, _| true);

    // 2) Unit 条目（menuUnit 目标；group 40E0C000/48E0C000）
    let mut unit_targets: Vec<u32> = Vec::new();
    let mut menu_meta: BTreeMap<u32, serde_json::Value> = BTreeMap::new();
    let mut icon_map: BTreeMap<u32, u32> = BTreeMap::new();
    for (_, instance, data) in &menu_entries {
        let Ok(file) = PropertyFile::parse(data) else {
            continue;
        };
        if let Some(unit) = key_instance(&file, HASH_MENU_UNIT) {
            unit_targets.push(unit);
        }
        if let Some(icon) = key_instance(&file, HASH_TOOL_ICON_KEY)
            .or_else(|| key_instance(&file, HASH_PALETTE_ICON_KEY))
        {
            icon_map.insert(*instance, icon);
        }
        let title = texts_of(&file, HASH_TITLE).first().copied();
        let desc = texts_of(&file, HASH_DESC).first().copied();
        let effect_titles = texts_of(&file, HASH_EFFECT_TITLE);
        let effect_params = u32s_of(&file, HASH_EFFECT_PARAM);
        menu_meta.insert(
            *instance,
            json!({
                "title": title.map(|(t, i)| json!({"table": format!("{t:08X}"), "id": format!("{i:08X}")})),
                "desc": desc.map(|(t, i)| json!({"table": format!("{t:08X}"), "id": format!("{i:08X}")})),
                "effectTitles": effect_titles.iter().map(|(t, i)| json!({"table": format!("{t:08X}"), "id": format!("{i:08X}")})).collect::<Vec<_>>(),
                "effectParams": effect_params.iter().map(|p| format!("{p:08X}")).collect::<Vec<_>>(),
                "unit": key_instance(&file, HASH_MENU_UNIT).map(|u| format!("{u:08X}")),
            }),
        );
    }

    let unit_wanted = |x: u32| unit_targets.contains(&x);
    let unit_entries = read_property(&unit_wanted, &UNIT_GROUPS, &|_, _| true);
    let unit_files: Vec<(u32, PropertyFile)> = unit_entries
        .iter()
        .filter_map(|(_, instance, data)| PropertyFile::parse(data).ok().map(|f| (*instance, f)))
        .collect();

    // 3) 组装 meta：名称/desc/统计行
    let mut meta: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for (_, instance, _) in &menu_entries {
        let Some(info) = menu_meta.get(instance) else {
            continue;
        };
        let mut row = serde_json::Map::new();
        // 名称/描述：全表解析（基础 6C969DEE 与 DLC 7B45C298 统一处理）
        for (key, prop) in [("name", "title"), ("desc", "desc")] {
            if let Some(text_ref) = info[prop].as_object() {
                let table = u32::from_str_radix(
                    text_ref["table"]
                        .as_str()
                        .unwrap_or("")
                        .trim_start_matches("0x"),
                    16,
                )
                .unwrap_or(0);
                let id = u32::from_str_radix(
                    text_ref["id"]
                        .as_str()
                        .unwrap_or("")
                        .trim_start_matches("0x"),
                    16,
                )
                .unwrap_or(0);
                if let Some(text) = locales.resolve(table, id) {
                    row.insert(key.to_string(), json!(text));
                }
            }
        }
        // 统计行：label 模板（desc 表）+ 值（menuUnit Unit 条目上的 float）
        let unit_file = info["unit"]
            .as_str()
            .and_then(|u| u32::from_str_radix(u.trim_start_matches("0x"), 16).ok())
            .and_then(|u| unit_files.iter().find(|(i, _)| *i == u).map(|(_, f)| f));
        let titles = info["effectTitles"].as_array().cloned().unwrap_or_default();
        let params = info["effectParams"].as_array().cloned().unwrap_or_default();
        let stats: Vec<serde_json::Value> = titles
            .iter()
            .enumerate()
            .filter_map(|(i, title_ref)| {
                let table =
                    u32::from_str_radix(title_ref["table"].as_str()?.trim_start_matches("0x"), 16)
                        .ok()?;
                let id =
                    u32::from_str_radix(title_ref["id"].as_str()?.trim_start_matches("0x"), 16)
                        .ok()?;
                let template = locales.resolve(table, id)?;
                let (label, suffix) = split_effect_template(&template);
                let value = params
                    .get(i)
                    .and_then(|p| p.as_str())
                    .and_then(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).ok())
                    .and_then(|hash| unit_file.and_then(|f| float_of(f, hash)))
                    .map(|v| format_amount(v));
                Some(json!({
                    "label": trim_label(&label),
                    "suffix": suffix,
                    "value": value,
                }))
            })
            .collect();
        if !stats.is_empty() {
            row.insert("stats".into(), json!(stats));
        }
        meta.insert(format!("{instance:08X}"), json!(row));
    }

    // 4) 图标映射（entry instance → icon PNG instance）
    let icon_map_json: serde_json::Map<String, serde_json::Value> = icon_map
        .iter()
        .map(|(menu, icon)| (format!("{menu:08X}"), json!(format!("{icon:08X}"))))
        .collect();

    let payload = json!({
        "format": "openscp-extra-meta/1",
        "meta": meta,
        "iconMap": icon_map_json,
    });
    std::fs::write(
        &meta_out,
        serde_json::to_string_pretty(&payload).unwrap() + "\n",
    )
    .expect("write meta-out");
    eprintln!(
        "meta {} 条、icon {} 个 → {}",
        meta.len(),
        icon_map.len(),
        meta_out.display()
    );

    // 5) 图片提取（大图 + 图标 PNG），已存在则跳过
    let wanted_icons: HashSet<u32> = icon_map.values().copied().collect();
    let mut marquees = 0usize;
    let mut icons = 0usize;
    let instance_set: HashSet<u32> = instances.iter().copied().collect();
    // 大图：instance 同菜单条目
    for package in &opened {
        for entry in package.entries() {
            if entry.id.type_id != TYPE_JPG || !instance_set.contains(&entry.id.instance) {
                continue;
            }
            let out = assets_dir.join(format!("tool_{:08X}.jpg", entry.id.instance));
            if out.exists() {
                continue;
            }
            if let Ok(data) = package.read(entry) {
                if std::fs::write(&out, &data).is_ok() {
                    marquees += 1;
                }
            }
        }
    }
    for package in &opened {
        for entry in package.entries() {
            // 图标 PNG 多在组 0x40E02400，分区类在组 0 —— instance 匹配即可
            if entry.id.type_id != TYPE_PNG || !wanted_icons.contains(&entry.id.instance) {
                continue;
            }
            let out = assets_dir.join(format!("tool_icon_{:08X}.png", entry.id.instance));
            if out.exists() {
                continue;
            }
            if let Ok(data) = package.read(entry) {
                if std::fs::write(&out, &data).is_ok() {
                    icons += 1;
                }
            }
        }
    }
    eprintln!(
        "新增大图 {marquees} 张、图标 {icons} 张 → {}",
        assets_dir.display()
    );
}
