//! 扫描游戏包里的大地面 lot（40E1C000、>3KB、带 BoundingBox），
//! 输出候选 donor lot（克隆参照用）。
//! Usage: cargo run --release -p fluffy-open-scp --example scan_big_lots -- <game.package>
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let path = std::env::args().nth(1).expect("package path");
    let package = Package::open(std::path::Path::new(&path)).expect("open");
    let mut rows: Vec<(u64, String)> = Vec::new();
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 || entry.id.group != 0x40E1_C000 {
            continue;
        }
        let bytes = package.read(entry).expect("read");
        if bytes.len() < 3000 {
            continue;
        }
        let Ok(file) = PropertyFile::parse(&bytes) else {
            continue;
        };
        let bbox = file.get(0x00F9_EFBA).map(|property| match &property.kind {
            Kind::Scalar(Value::BoundingBox { min, max }) => {
                format!("[{:.0},{:.0}..{:.0},{:.0}]", min[0], min[1], max[0], max[1])
            }
            Kind::Array(values) => values
                .iter()
                .find_map(|value| match value {
                    Value::BoundingBox { min, max } => Some(format!(
                        "[{:.0},{:.0}..{:.0},{:.0}]",
                        min[0], min[1], max[0], max[1]
                    )),
                    _ => None,
                })
                .unwrap_or_else(|| "?".into()),
            _ => "?".into(),
        });
        let Some(bbox) = bbox else { continue };
        rows.push((
            bytes.len() as u64,
            format!(
                "{:08X} {}B bbox={} parent={:08X}",
                entry.id.instance,
                bytes.len(),
                bbox,
                file.get(0x00B2_CCCB)
                    .and_then(|property| match &property.kind {
                        Kind::Scalar(Value::Key(key)) => Some(key.instance),
                        _ => None,
                    })
                    .unwrap_or(0),
            ),
        ));
    }
    rows.sort();
    for (_, row) in rows.iter().rev() {
        println!("{row}");
    }
    let _ = ResourceId {
        type_id: 0,
        group: 0,
        instance: 0,
    };
}
