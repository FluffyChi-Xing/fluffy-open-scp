//! 生成器：把菜单条目的经济字段从实际安装包提取成 costs.json，供 UI 工作台
//! 离线装配。数据链全部经反编译源码（D:\ea-games\simcity_dev\new-cource）+ 包
//! 实测定案（2026-10-18）：
//!
//! - **道路/步道**（cToolPathPlacer::GetPaletteRolloverCost）：rollover 造价 =
//!   0xD13BB4A6 指向的分段定义（组 40E1C400）上的 0x0E76A7E2（float，取整）；
//!   分段无此键 = §0（SimCity 2013 道路铺设免费，街道/大道无键即免费）。
//!   菜单条目自身也可带 0x0E76A7E2（部分步道=1），分段优先。
//! - **建筑 plop**（cToolPlop::FillFromProps）：菜单条目 0x0975695F（menuUnit）
//!   → 单元配置 instance → **EcoGame 脚本包**（SimCityUserData/EcoGame/*.package）
//!   组 40E0C000/48E0C000 的同 instance Unit 条目：0x0A8A41DA 建造价（float）+
//!   子单位（0x07E0AD0A 一层求和）+ 0x09AE19D7 维护/小时。已对拍：诊所
//!   §20000/§400、镇公所 §0/§200、风电厂 §3000/§40。
//! - **资产配置 Cost**（0x0C8EE92F，组 40E1C000/61F0C000）：交易/模块价，
//!   与菜单无键引用，按 6C969DEE 名称字串联查（menuUnit 无命中时的兜底）。
//!
//! 用法：cargo run -p probes --release --example menu_cost_dump -- <out.json> <package...>
//! （SimCityData 包、EcoGame 脚本包、Locale 语言包可以混着传，按类型/group
//! 自动分类；包顺序 = 名称联查优先级。语言包提供全部 locale 表 —— DLC 条目
//! 的标题用独立表（如 CoT 7B45C298），不能只认 6C969DEE。）
//!
//! v3 追加：建筑子菜单（Unit 子单位 0x07E0AD0A → 反查 menuUnit 指向它的
//! 菜单条目）+ 名称/描述（全表 locale）+ 子菜单条目的图标/大图提取。
use sc_properties::{Kind, PropertyFile, Value};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const TYPE_PNG: u32 = 0x2F7D_0004;
const TYPE_JPG: u32 = 0x3F86_62EA;
const TYPE_LOCALE: u32 = 0x0A98_EAF0;
const ICON_GROUP: u32 = 0x40E0_2400;
const LOCALE_NAMES: u32 = 0x6C96_9DEE;

const HASH_TITLE: u32 = 0x0A09_F5FA; // 菜单标题 Text
const HASH_DESC: u32 = 0x0A09_F5FB; // 菜单描述 Text
const HASH_MENU_UNIT: u32 = 0x0975_695F; // menuUnit（原 C# 菜单复制亦用此名）
const HASH_PIECE: u32 = 0xD13B_B4A6; // 道路分段引用
const HASH_PATH_COST: u32 = 0x0E76_A7E2; // 路径单价（float）
const HASH_CONFIG_COST: u32 = 0x0C8E_E92F; // 资产配置 Cost（float）
const HASH_CONSTRUCTION: u32 = 0x0A8A_41DA; // 建造价（float，Unit 条目）
const HASH_MAINT: u32 = 0x09AE_19D7; // 维护/小时（Unit 条目）
const HASH_CHILD_UNITS: u32 = 0x07E0_AD0A; // 子单位 Key 数组（一层求和）
const HASH_ADDON_CONTAINER: u32 = 0x0AFE_27CA; // 建筑子菜单容器（Unit 条目 → 容器 instance）
const HASH_TOOL_ICON_KEY: u32 = 0x0977_AA8F; // 槽位图标键
const HASH_UI_CATEGORIES: u32 = 0x0DB9_FC63; // 归属菜单数组
const HASH_UI_POSITION: u32 = 0x0DC1_E3E0; // 菜单内排序
const HASH_STAT_TITLE: u32 = 0x0EB1_FC05; // 统计行模板 Text 数组
const HASH_STAT_PARAM: u32 = 0x0EB1_FC22; // 统计行参数 = Unit 条目上的 float 属性 hash
const HASH_UNLOCK_STRING: u32 = 0x0DE8_4DDC; // 解锁条件 Text 数组
const HASH_PARENT: u32 = 0x00B2_CCCB; // unit → unit parent（占位 unit 指向真身）
/// 疑似字段但已排除建造上限语义：0x0A7917C8 在 133 条目上取值 1~66，
/// 其中可无限建造的诊所=1；游戏「可用:N」徽章数值来源仍未定位
/// （徽章文案 = locale 4B54417A:0x0edf2f12「可用：」，数值来自运行时）。
const HASH_FLAG_0A7917C8: u32 = 0x0A79_17C8;

const MENU_INSTANCE_TYPES: [u16; 2] = [0x8A01, 0xC900];
const PIECE_GROUPS: [u32; 1] = [0x40E1_C400];
const CONFIG_GROUPS: [u32; 2] = [0x40E1_C000, 0x61F0_C000];
const UNIT_GROUPS: [u32; 2] = [0x40E0_C000, 0x48E0_C000];

fn values_of(file: &PropertyFile, hash: u32) -> Vec<&Value> {
    match file.get(hash).map(|e| &e.kind) {
        Some(Kind::Scalar(v)) => vec![v],
        Some(Kind::Array(values)) => values.iter().collect(),
        _ => Vec::new(),
    }
}

fn num_of(file: &PropertyFile, hash: u32) -> Option<i32> {
    values_of(file, hash).iter().find_map(|v| match v {
        Value::Int32(n) => Some(*n),
        Value::UInt32(n) => Some(*n as i32),
        Value::Float(f) => Some(*f as i32),
        _ => None,
    })
}

fn key_instance(file: &PropertyFile, hash: u32) -> Option<u32> {
    values_of(file, hash).into_iter().find_map(|v| match v {
        Value::Key(k) => Some(k.instance),
        _ => None,
    })
}

fn key_instances(file: &PropertyFile, hash: u32) -> Vec<u32> {
    values_of(file, hash)
        .into_iter()
        .filter_map(|v| match v {
            Value::Key(k) => Some(k.instance),
            _ => None,
        })
        .collect()
}

struct MenuRow {
    name: Option<u32>,
    /// 建筑子菜单容器（Unit 条目 0x0AFE27CA 的目标；有子菜单的建筑才有）。
    addon_container: Option<u32>,
    /// 完整标题 Text 引用（表 id + 字符串 id；DLC 用独立表）。
    title: Option<(u32, u32)>,
    /// 完整描述 Text 引用。
    desc: Option<(u32, u32)>,
    /// 解锁条件文案（全表 locale 解析；多条件按序）。
    unlock: Vec<String>,
    /// 统计行（scan 期定形：label/值尾注/是否消耗参数/字面量值）。
    stat_rows: Vec<StatRow>,
    /// 统计行参数（与 stat_rows 平行；值 = Unit 条目上的 float 属性 hash）。
    stat_params: Vec<u32>,
    menu_unit: Option<u32>,
    piece: Option<u32>,
    menu_path_cost: Option<i32>,
    icon_key: Option<u32>,
    /// uiCategories（归属容器；子菜单容器命中用）。
    cats: Vec<u32>,
    pos: Option<i32>,
}

/// EcoGame Unit 条目的语义视图。DLC 包的 Unit 组号各不相同
/// （40E0C000/48E0C000/41EFC000/47EFC000/61EFC000…），按语义键识别。
struct EcoUnit {
    construction: Option<i32>,
    maintenance: Option<i32>,
    children: Vec<u32>,
    addon_container: Option<u32>,
    floats: BTreeMap<u32, f32>,
    /// unit → unit parent（占位 unit 经 parent 指向真身，如 V博士大樓）。
    parent: Option<u32>,
    /// 未证实语义字段（0x0A7917C8）：诊所=1 但可无限建造，已排除建造上限。
    flag_0a7917c8: Option<i32>,
}

/// locale 表集合：从任意传入包自动收集 0x0A98EAF0 资源（BOM + 紧凑 JSON）。
struct LocaleTables {
    tables: BTreeMap<u32, BTreeMap<String, String>>,
}

impl LocaleTables {
    fn load(packages: &[dbpf::Package]) -> Self {
        let mut tables = BTreeMap::new();
        for package in packages {
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
        }
        Self { tables }
    }

    fn resolve(&self, table: u32, instance: u32) -> Option<String> {
        self.tables
            .get(&table)?
            .get(&format!("0x{:08x}", instance))
            .cloned()
    }
}

/// Unit 条目语义判定（跨 DLC 包组号各不相同：40E0C000/48E0C000/47EFC000/
/// 61EFC000…，但都携带建造成本/维护/子单位键之一）。
fn is_unit_entry(file: &PropertyFile) -> bool {
    num_of(file, HASH_CONSTRUCTION).is_some()
        || num_of(file, HASH_MAINT).is_some()
        || !key_instances(file, HASH_CHILD_UNITS).is_empty()
}

/// 去掉模板里的粗体标签。
fn clean_tags(s: &str) -> String {
    s.replace("<B>", "")
        .replace("</B>", "")
        .replace("<b>", "")
        .replace("</b>", "")
}

/// 统计行模板拆分：`<B>監牢</B>：~amount:number~` → ("監牢", Some(""))，
/// `~amount~ ~exponent~W` 的后缀保留为值尾注（其余占位符剔除）。
/// 一行统计的定形结果。
struct StatRow {
    label: String,
    /// 有占位符时：Unit float 之后的值尾注（如 " 分鐘"、" W"）。
    suffix: Option<String>,
    /// 是否消耗一个 Unit float 参数。
    has_param: bool,
    /// 无占位符时的字面量值（如 "無"、"高"）。
    literal: Option<String>,
}

/// 统计行模板拆分：`<B>監牢</B>：~amount:number~` → 監牢 + 消耗参数；
/// `空氣汙染率：無`（无占位符）→ label/字面量值按最后一个冒号切分。
fn split_template(t: &str) -> StatRow {
    let clean = clean_tags(t);
    if let Some(start) = clean.find('~') {
        if let Some(rel) = clean[start + 1..].find('~') {
            let end = start + 1 + rel;
            let label = clean[..start]
                .trim()
                .trim_end_matches(['：', ':'])
                .trim()
                .to_string();
            let mut suffix = clean[end + 1..].to_string();
            while let Some(s) = suffix.find('~') {
                if let Some(rel) = suffix[s + 1..].find('~') {
                    suffix.replace_range(s..s + rel + 2, "");
                } else {
                    break;
                }
            }
            return StatRow {
                label,
                suffix: Some(suffix),
                has_param: true,
                literal: None,
            };
        }
    }
    let (label, literal) = match clean.rsplit_once(['：', ':']) {
        Some((head, rest)) => (head.trim().to_string(), Some(rest.trim().to_string())),
        None => (clean.trim().to_string(), None),
    };
    StatRow {
        label,
        suffix: None,
        has_param: false,
        literal,
    }
}

fn text_ref(file: &PropertyFile, hash: u32) -> Option<(u32, u32)> {
    values_of(file, hash).into_iter().find_map(|v| match v {
        Value::Text(t) => Some((t.table_id, t.instance_id)),
        _ => None,
    })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out_json = args
        .next()
        .expect("usage: menu_cost_dump <out.json> <package...>");
    let packages: Vec<String> = args.collect();
    assert!(!packages.is_empty(), "need at least one package");

    let mut menus: BTreeMap<u32, MenuRow> = BTreeMap::new();
    let mut piece_path_cost: BTreeMap<u32, i32> = BTreeMap::new();
    // name id -> [(包序, config instance, Cost)]（仅带 Cost 的配置）
    let mut config_by_name: BTreeMap<u32, Vec<(usize, u32, i32)>> = BTreeMap::new();
    // eco unit instance -> (constr, maint, children, 子菜单容器)
    // eco unit instance -> 完整语义视图（见 EcoUnit）
    let mut units: BTreeMap<u32, EcoUnit> = BTreeMap::new();
    // 全属性 parent 关系（instance → 0x00B2CCCB 目标；占位 unit 沿此指向真身）
    let mut prop_parents: BTreeMap<u32, u32> = BTreeMap::new();

    let mut opened = Vec::new();
    for (package_index, path) in packages.iter().enumerate() {
        let package = dbpf::Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e:?}"));
        let _ = package_index;
        opened.push(package);
    }
    let locales = LocaleTables::load(&opened);
    for (package_index, package) in opened.iter().enumerate() {
        for entry in package.entries() {
            if entry.id.type_id != TYPE_PROPERTY {
                continue;
            }
            let Ok(data) = package.read(entry) else {
                continue;
            };
            let Ok(file) = PropertyFile::parse(&data) else {
                continue;
            };
            if let Some(parent) = key_instance(&file, HASH_PARENT) {
                prop_parents.insert(entry.id.instance, parent);
            }
            let instance_type = (entry.id.group & 0xFFFF) as u16;
            if MENU_INSTANCE_TYPES.contains(&instance_type) {
                menus.insert(
                    entry.id.instance,
                    MenuRow {
                        name: values_of(&file, HASH_TITLE)
                            .into_iter()
                            .find_map(|v| match v {
                                Value::Text(t) if t.table_id == LOCALE_NAMES => Some(t.instance_id),
                                _ => None,
                            }),
                        title: text_ref(&file, HASH_TITLE),
                        desc: text_ref(&file, HASH_DESC),
                        unlock: values_of(&file, HASH_UNLOCK_STRING)
                            .into_iter()
                            .filter_map(|v| match v {
                                Value::Text(t) => locales.resolve(t.table_id, t.instance_id),
                                _ => None,
                            })
                            .collect(),
                        stat_params: values_of(&file, HASH_STAT_PARAM)
                            .into_iter()
                            .filter_map(|v| match v {
                                Value::UInt32(u) => Some(*u),
                                Value::Int32(i) => Some(*i as u32),
                                _ => None,
                            })
                            .collect(),
                        stat_rows: values_of(&file, HASH_STAT_TITLE)
                            .into_iter()
                            .map(|v| match v {
                                Value::Text(t) => {
                                    let resolved = locales
                                        .resolve(t.table_id, t.instance_id)
                                        .unwrap_or_else(|| format!("0x{:08x}", t.instance_id));
                                    split_template(&resolved)
                                }
                                _ => StatRow {
                                    label: String::new(),
                                    suffix: None,
                                    has_param: false,
                                    literal: None,
                                },
                            })
                            .collect(),
                        menu_unit: key_instance(&file, HASH_MENU_UNIT),
                        piece: key_instance(&file, HASH_PIECE),
                        menu_path_cost: num_of(&file, HASH_PATH_COST),
                        icon_key: key_instance(&file, HASH_TOOL_ICON_KEY),
                        addon_container: None,
                        cats: key_instances(&file, HASH_UI_CATEGORIES),
                        pos: num_of(&file, HASH_UI_POSITION),
                    },
                );
            } else if PIECE_GROUPS.contains(&entry.id.group) {
                if let Some(cost) = num_of(&file, HASH_PATH_COST) {
                    piece_path_cost.insert(entry.id.instance, cost);
                }
            } else if CONFIG_GROUPS.contains(&entry.id.group) {
                let mut names = Vec::new();
                for prop in &file.values {
                    for v in values_of(&file, prop.hash) {
                        if let Value::Text(t) = v {
                            names.push(t.instance_id);
                        }
                    }
                }
                names.sort();
                names.dedup();
                if let Some(cost) = num_of(&file, HASH_CONFIG_COST) {
                    for name in &names {
                        config_by_name.entry(*name).or_default().push((
                            package_index,
                            entry.id.instance,
                            cost,
                        ));
                    }
                }
            } else if is_unit_entry(&file) {
                let mut floats = BTreeMap::new();
                for prop in &file.values {
                    for v in values_of(&file, prop.hash) {
                        if let Value::Float(f) = v {
                            floats.insert(prop.hash, *f);
                        }
                    }
                }
                units.insert(
                    entry.id.instance,
                    EcoUnit {
                        construction: num_of(&file, HASH_CONSTRUCTION),
                        maintenance: num_of(&file, HASH_MAINT),
                        children: key_instances(&file, HASH_CHILD_UNITS),
                        addon_container: key_instance(&file, HASH_ADDON_CONTAINER),
                        floats,
                        parent: key_instance(&file, HASH_PARENT),
                        flag_0a7917c8: num_of(&file, HASH_FLAG_0A7917C8),
                    },
                );
            }
        }
    }

    // 二遍：menuUnit → Unit 条目 0x0AFE27CA = 建筑子菜单容器
    for row in menus.values_mut() {
        if let Some(unit) = row.menu_unit {
            if let Some(unit) = units.get(&unit) {
                row.addon_container = unit.addon_container;
            }
        }
    }

    // 建筑造价：menuUnit → EcoGame Unit（顶层 + 一层子单位求和，镜像 cToolPlop）。
    // 维护费同结构求和后取负 —— 游戏弹窗显示「-300/小時」= -(200 顶层 + 100 子单位)
    // （公車站 79F3ED13 实测对拍）。
    // 占位 unit 跟随：部分 DLC 建筑（V博士大樓）的 menuUnit 指向无造价键的
    // 占位条目（可能根本不是 Unit），真身沿 0x00B2CCCB parent 链可达。最多跟两级。
    let resolve_unit = |menu_unit: u32| -> Option<(&EcoUnit, Option<u32>)> {
        if let Some(unit) = units.get(&menu_unit) {
            if unit.construction.is_some() || unit.maintenance.is_some() {
                return Some((unit, None));
            }
        }
        let mut candidate = prop_parents.get(&menu_unit).copied();
        for _ in 0..2 {
            let parent = candidate?;
            if let Some(unit) = units.get(&parent) {
                if unit.construction.is_some() || unit.maintenance.is_some() {
                    return Some((unit, Some(parent)));
                }
            }
            candidate = prop_parents.get(&parent).copied();
        }
        None
    };
    let plop_cost = |menu_unit: u32| -> (Option<i32>, Option<i32>) {
        let Some((unit, _)) = resolve_unit(menu_unit) else {
            return (None, None);
        };
        let mut total = unit.construction;
        let mut maint_total = unit.maintenance;
        if let Some(base) = unit.construction {
            for child in &unit.children {
                if let Some(child_unit) = units.get(child) {
                    if let Some(c) = child_unit.construction {
                        total = Some(base + c);
                    }
                }
            }
        }
        if let Some(base_maint) = unit.maintenance {
            for child in &unit.children {
                if let Some(child_unit) = units.get(child) {
                    if let Some(m) = child_unit.maintenance {
                        maint_total = Some(base_maint + m);
                    }
                }
            }
        }
        (total, maint_total.map(|m| -m))
    };

    // ── 子菜单：Unit 子单位（0x07E0AD0A）→ 反查 menuUnit 指向它的菜单条目。
    // 消防局等建筑的附加模块（旗杆/旗帜/瞭望塔）= 父建筑 Unit 的子单位；
    // 模块菜单条目的 menuUnit 指向同一子单位。同时提取子菜单条目的图标/大图。
    let mut unit_to_menu: BTreeMap<u32, u32> = BTreeMap::new();
    for (menu, row) in &menus {
        if let Some(unit) = row.menu_unit {
            unit_to_menu.entry(unit).or_insert(*menu);
        }
    }
    // 图片资源索引：(type, instance) → (包序, entry)
    let mut asset_index: std::collections::HashMap<(u32, u32), (usize, dbpf::IndexEntry)> =
        std::collections::HashMap::new();
    for (package_index, package) in opened.iter().enumerate() {
        for entry in package.entries() {
            if entry.id.type_id == TYPE_PNG || entry.id.type_id == TYPE_JPG {
                asset_index
                    .entry((entry.id.type_id, entry.id.instance))
                    .or_insert((package_index, entry.clone()));
            }
        }
    }
    let assets_dir = PathBuf::from(&out_json)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("replica/assets"))
        .expect("derive assets dir from out_json");
    std::fs::create_dir_all(&assets_dir).ok();
    let extract_asset = |type_id: u32, instance: u32, file_name: String| -> Option<String> {
        let (package_index, entry) = asset_index.get(&(type_id, instance))?;
        let out = assets_dir.join(&file_name);
        if !out.exists() {
            let data = opened[*package_index].read(entry).ok()?;
            std::fs::write(&out, data).ok()?;
        }
        Some(format!("/game-ui/replica/assets/{file_name}"))
    };

    let mut submenus: BTreeMap<u32, Vec<serde_json::Value>> = BTreeMap::new();
    for (menu, row) in &menus {
        let Some(unit) = row.menu_unit else { continue };
        let Some(unit) = units.get(&unit) else {
            continue;
        };
        let children = &unit.children;
        let addon_container = unit.addon_container;
        // 两种机制取并集：
        // a) 附加面板容器（Unit 0x0AFE27CA → 容器；成员 = uiCategories 含该容器的条目，
        //    如大学 9D2EF585 / 各消防警察模块面板）；
        // b) 子单位反查（0x07E0AD0A → menuUnit 指向子单位的菜单条目，如公車站車庫）。
        let mut candidate_menus: Vec<u32> = Vec::new();
        if let Some(container) = addon_container {
            for (candidate, candidate_row) in &menus {
                if candidate != menu && candidate_row.cats.contains(&container) {
                    candidate_menus.push(*candidate);
                }
            }
        }
        for child in children {
            if let Some(child_menu) = unit_to_menu.get(child) {
                if child_menu != menu && !candidate_menus.contains(child_menu) {
                    candidate_menus.push(*child_menu);
                }
            }
        }
        // 按 uiPosition 排序输出
        candidate_menus
            .sort_by_key(|candidate| menus.get(candidate).and_then(|r| r.pos).unwrap_or(i32::MAX));
        let mut items = Vec::new();
        for child_menu in candidate_menus {
            let Some(child_row) = menus.get(&child_menu) else {
                continue;
            };
            // 名称用全表 locale（DLC 条目在独立表）；无名的跳过
            let Some(name) = child_row.title.and_then(|(t, i)| locales.resolve(t, i)) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let desc = child_row
                .desc
                .and_then(|(t, i)| locales.resolve(t, i))
                .unwrap_or_default();
            let (cost, maintenance) = child_row.menu_unit.map(plop_cost).unwrap_or((None, None));
            let preview = child_row.icon_key.and_then(|icon| {
                extract_asset(TYPE_PNG, icon, format!("tool_icon_{icon:08X}.png"))
            });
            let marquee =
                extract_asset(TYPE_JPG, child_menu, format!("tool_{:08X}.jpg", child_menu));
            items.push(json!({
                "instance": format!("{child_menu:08X}"),
                "name": name,
                "desc": desc,
                "preview": preview,
                "marquee": marquee,
                "cost": cost,
                "maintenance": maintenance,
            }));
        }
        if !items.is_empty() {
            submenus.insert(*menu, items);
        }
    }

    let mut costs: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for (menu, row) in &menus {
        // 道路：分段单价优先（无键 = §0，引擎语义）；菜单级键兜底
        let road_cost = row
            .piece
            .map(|piece| piece_path_cost.get(&piece).copied().unwrap_or(0))
            .or(row.menu_path_cost);
        // 建筑：menuUnit → EcoGame；无命中时按名称联查资产配置 Cost 兜底
        let (cost, maintenance) = row.menu_unit.map(plop_cost).unwrap_or((None, None));
        let mut config_cost = None;
        let mut config_ref = None;
        if cost.is_none() {
            if let Some(name) = row.name {
                if let Some(candidates) = config_by_name.get(&name) {
                    if let Some((_, config, value)) =
                        candidates.iter().min_by_key(|(pkg, inst, _)| (*pkg, *inst))
                    {
                        config_cost = Some(*value);
                        config_ref = Some(*config);
                    }
                }
            }
        }
        // 名称/描述/解锁/统计（全表 locale 解析）——工作台用它回填未解析条目。
        let name = row.title.and_then(|(t, i)| locales.resolve(t, i));
        let desc = row.desc.and_then(|(t, i)| locales.resolve(t, i));
        // 模板驱动 + 参数游标：带 ~占位符~ 的行消耗一个 Unit float 参数；
        // 无占位符的行（如「空氣汙染率：無」）不消耗，值取冒号后字面量。
        let unit_floats = row
            .menu_unit
            .and_then(|unit| units.get(&unit))
            .map(|unit| &unit.floats);
        let stats: Vec<serde_json::Value> = row
            .stat_rows
            .iter()
            .zip(row.stat_params.iter())
            .map(|(stat_row, hash)| {
                let value = if stat_row.has_param {
                    unit_floats
                        .and_then(|floats| floats.get(hash))
                        .copied()
                        .map(|f| {
                            let num = if f.fract() == 0.0 {
                                format!("{}", f as i64)
                            } else {
                                format!("{f}")
                            };
                            match &stat_row.suffix {
                                Some(suffix) if !suffix.is_empty() => format!("{num}{suffix}"),
                                _ => num,
                            }
                        })
                } else {
                    stat_row.literal.clone()
                };
                json!({ "label": stat_row.label, "value": value })
            })
            .filter(|row| !row["label"].as_str().unwrap_or("").is_empty())
            .collect();
        if road_cost.is_none()
            && cost.is_none()
            && maintenance.is_none()
            && config_cost.is_none()
            && name.is_none()
            && row.unlock.is_empty()
            && row.stat_rows.is_empty()
        {
            continue;
        }
        let mut entry = serde_json::Map::new();
        if let Some(name) = name {
            entry.insert("name".into(), json!(name));
        }
        if let Some(desc) = desc {
            entry.insert("desc".into(), json!(desc));
        }
        if !row.unlock.is_empty() {
            entry.insert("unlock".into(), json!(row.unlock));
        }
        if !row.stat_rows.is_empty() {
            entry.insert("stats".into(), json!(stats));
        }
        if let Some(v) = road_cost {
            entry.insert("roadCost".into(), json!(v));
        }
        if let Some(v) = cost {
            entry.insert("cost".into(), json!(v));
        }
        if let Some(v) = maintenance {
            entry.insert("maintenance".into(), json!(v));
        }
        if let Some(v) = config_cost {
            entry.insert("configCost".into(), json!(v));
        }
        if let Some(u) = row.menu_unit {
            entry.insert("unit".into(), json!(format!("{u:08X}")));
        }
        if let Some(c) = config_ref {
            entry.insert("config".into(), json!(format!("{c:08X}")));
        }
        if let Some(p) = row.piece {
            entry.insert("piece".into(), json!(format!("{p:08X}")));
        }
        if let Some(c) = row.addon_container {
            entry.insert("addonContainer".into(), json!(format!("{c:08X}")));
        }
        costs.insert(format!("{menu:08X}"), json!(entry));
    }
    let submenus_json: serde_json::Map<String, serde_json::Value> = submenus
        .into_iter()
        .map(|(menu, items)| (format!("{menu:08X}"), json!(items)))
        .collect();
    let submenu_containers: serde_json::Map<String, serde_json::Value> = menus
        .iter()
        .filter_map(|(menu, row)| {
            row.addon_container
                .map(|c| (format!("{menu:08X}"), json!(format!("{c:08X}"))))
        })
        .collect();
    let payload = json!({
        "format": "openscp-menu-costs/3",
        "source": "paths: piece 0x0E76A7E2 (cToolPathPlacer); plop: menuUnit 0x0975695F -> EcoGame 40E0C000 0x0A8A41DA/0x09AE19D7 (cToolPlop); submenus: unit children 0x07E0AD0A -> menuUnit reverse; fallback: config 0x0C8EE92F by 6C969DEE name",
        "costs": costs,
        "submenus": submenus_json,
        "submenuContainers": submenu_containers,
    });
    std::fs::write(
        &out_json,
        serde_json::to_string_pretty(&payload).unwrap()
            + "
",
    )
    .expect("write out.json");
    eprintln!(
        "菜单 {} 条，经济字段 {} 条，子菜单 {} 组 → {}",
        menus.len(),
        costs.len(),
        submenus_json.len(),
        out_json
    );
}
