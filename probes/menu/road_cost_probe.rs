//! 探针：道路菜单条目 → piece 定义（0xD13BB4A6）→ 全部键值，
//! 用于定位道路造价/维护的真实字段。仅开发用。
//!
//! 用法：cargo run -p probes --release --example road_cost_probe -- <package>
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const HASH_PIECE: u32 = 0xD13B_B4A6;

const ROADS: [(u32, &str); 7] = [
    (0x7DD8_A627, "低密度泥路"),
    (0xB7D3_F194, "低密道街道"),
    (0x4115_8FE5, "高密度街道"),
    (0x795F_7036, "中等密度街道"),
    (0x2CD1_30D3, "高密度大道"),
    (0x6768_04D7, "中等密度大道"),
    (0x1055_F94B, "高密度電車大道"),
];

fn scalar_text(file: &PropertyFile, hash: u32) -> String {
    match file.get(hash).map(|e| &e.kind) {
        Some(Kind::Scalar(Value::Text(t))) => {
            format!("#{:08X}:{:08X}", t.table_id, t.instance_id)
        }
        _ => "-".into(),
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: road_cost_probe <package>");
    let package = dbpf::Package::open(&path).expect("open package");
    // 收集 piece 定义
    let mut pieces: BTreeMap<u32, PropertyFile> = BTreeMap::new();
    for entry in package.entries() {
        if entry.id.type_id != TYPE_PROPERTY || entry.id.group != 0x40E1_C400 {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        if let Ok(file) = PropertyFile::parse(&data) {
            pieces.insert(entry.id.instance, file);
        }
    }
    // 菜单条目 → piece
    let mut menus: BTreeMap<u32, PropertyFile> = BTreeMap::new();
    for entry in package.entries() {
        if entry.id.type_id != TYPE_PROPERTY || entry.id.group & 0xFFFF != 0x8A01 {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        if let Ok(file) = PropertyFile::parse(&data) {
            menus.insert(entry.id.instance, file);
        }
    }
    for (id, name) in ROADS {
        let Some(menu) = menus.get(&id) else {
            println!("{:08X} {}: 菜单条目缺失", id, name);
            continue;
        };
        let piece_id = match menu.get(HASH_PIECE).map(|e| &e.kind) {
            Some(Kind::Scalar(Value::Key(k))) => k.instance,
            _ => {
                println!("{:08X} {}: 无 piece 引用", id, name);
                continue;
            }
        };
        let Some(piece) = pieces.get(&piece_id) else {
            println!("{:08X} {}: piece {:08X} 不在本包", id, name, piece_id);
            continue;
        };
        println!("== {:08X} → piece {:08X}", id, piece_id);
        let mut keys: Vec<_> = piece.values.iter().map(|e| e.hash).collect();
        keys.sort();
        keys.dedup();
        for hash in keys {
            let entry = piece.get(hash).expect("hash from own list");
            let summary = match &entry.kind {
                Kind::Scalar(v) => format!("{}", ScalarView(v)),
                Kind::Array(values) => values
                    .iter()
                    .map(|v| format!("{}", ScalarView(v)))
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => "?".into(),
            };
            println!("  {:08X} = {}", hash, summary);
        }
    }
}

/// 标量的简短视图（只看数值语义，不看类型名）。
struct ScalarView<'a>(&'a Value);

impl std::fmt::Display for ScalarView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Value::Bool(b) => write!(f, "bool:{}", b),
            Value::Int32(n) => write!(f, "{}", n),
            Value::UInt32(n) => write!(f, "u:{}", n),
            Value::Float(n) => write!(f, "f:{}", n),
            Value::Vector2(v) => write!(f, "({},{})", v[0], v[1]),
            Value::Vector3(v) => write!(f, "({},{},{})", v[0], v[1], v[2]),
            Value::Key(k) => write!(f, "key:{:08X}", k.instance),
            Value::Text(t) => write!(f, "txt:{:08X}:{:08X}", t.table_id, t.instance_id),
            _ => write!(f, "?"),
        }
    }
}
