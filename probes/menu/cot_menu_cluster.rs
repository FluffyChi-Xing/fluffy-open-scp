//! 探针：聚类 DLC/EP1 菜单条目（C900 InstanceType 组）的 uiCategories，
//! 输出 容器 instance → 成员数 + 样本标题（全表 locale 解析），
//! 用于定位城市发展 9 个 CoT 菜单的容器 id。仅开发用。
//!
//! 用法：cargo run -p probes --release --example cot_menu_cluster --
//!   <locale.package> <package...>
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_UI_CATEGORIES: u32 = 0x0DB9_FC63;

fn main() {
    let mut args = std::env::args().skip(1);
    let locale_path = args.next().expect("locale package");
    let packages: Vec<String> = args.collect();

    // 全表 locale
    let locale_pkg = Package::open(&locale_path).unwrap();
    let mut tables: BTreeMap<u32, BTreeMap<String, String>> = BTreeMap::new();
    for e in locale_pkg.entries() {
        if e.id.type_id != 0x0A98_EAF0 {
            continue;
        }
        let Ok(data) = locale_pkg.read(e) else {
            continue;
        };
        let text = String::from_utf8_lossy(&data);
        let text = text.trim_start_matches('\u{feff}');
        if let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(text) {
            tables.insert(e.id.instance, map);
        }
    }
    let resolve = |t: u32, i: u32| -> Option<String> {
        tables.get(&t)?.get(&format!("0x{:08x}", i)).cloned()
    };

    // 聚类：容器 → 成员 (instance, title)
    let mut clusters: BTreeMap<u32, Vec<(u32, String)>> = BTreeMap::new();
    for path in &packages {
        let Ok(p) = Package::open(path) else { continue };
        for e in p.entries() {
            if e.id.type_id != TYPE_PROPERTY {
                continue;
            }
            let instance_type = (e.id.group & 0xFFFF) as u16;
            if instance_type != 0xC900 && instance_type != 0x8A01 {
                continue;
            }
            let Ok(data) = p.read(e) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else {
                continue;
            };
            let title = file
                .get(HASH_TITLE)
                .map(|prop| match &prop.kind {
                    Kind::Scalar(Value::Text(t)) => resolve(t.table_id, t.instance_id)
                        .unwrap_or_else(|| format!("?{:08X}:{:08X}", t.table_id, t.instance_id)),
                    Kind::Array(vs) => vs
                        .iter()
                        .filter_map(|v| match v {
                            Value::Text(t) => Some(
                                resolve(t.table_id, t.instance_id)
                                    .unwrap_or_else(|| format!("?{:08X}", t.instance_id)),
                            ),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" | "),
                    _ => String::new(),
                })
                .unwrap_or_default();
            let cats: Vec<u32> = file
                .get(HASH_UI_CATEGORIES)
                .map(|prop| match &prop.kind {
                    Kind::Scalar(Value::Key(k)) => vec![k.instance],
                    Kind::Array(vs) => vs
                        .iter()
                        .filter_map(|v| match v {
                            Value::Key(k) => Some(k.instance),
                            _ => None,
                        })
                        .collect(),
                    _ => vec![],
                })
                .unwrap_or_default();
            for cat in cats {
                clusters
                    .entry(cat)
                    .or_default()
                    .push((e.id.instance, title.clone()));
            }
        }
    }

    println!("容器总数: {}", clusters.len());
    for (cat, members) in &clusters {
        let names: Vec<String> = members.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>();
        println!(
            "{cat:08X}: {} 成员 → {}",
            members.len(),
            names.iter().take(6).cloned().collect::<Vec<_>>().join("、")
        );
    }
}
