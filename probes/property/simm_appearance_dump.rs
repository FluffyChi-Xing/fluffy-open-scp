//! 探针：扫描全包「小人外观定义」（8 列 0x0CBD25C1..C6 / CA / CB 成对表），
//! 输出每条定义的行数与每行 body/head/outfit 区间 + scale，并附 s3db 名字。
//! 这就是 GI_Simm_* 角色外观表的数据侧证据。仅开发用。
//!
//! 用法：cargo run -p probes --example simm_appearance_dump -- <package> [s3db] [out.json]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const BODY: u32 = 0x0CBD_25C1; // heads 起始（FillFromProps: tableInfo[2].mPropID）
const BODY_MAX: u32 = 0x0CBC_0000; // 占位，实际见下
const C_BODIES_MIN: u32 = 0x0CBD_25C2;
const C_HEADS_MIN: u32 = 0x0CBD_25C1;
const C_OUTFITS_MIN: u32 = 0x0CBD_25C3;
const C_BODIES_MAX: u32 = 0x0CBD_25C5;
const C_HEADS_MAX: u32 = 0x0CBD_25C4;
const C_OUTFITS_MAX: u32 = 0x0CBD_25C6;
const C_SCALE_MIN: u32 = 0x0CBD_25CA;
const C_SCALE_MAX: u32 = 0x0CBD_25CB;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: simm_appearance_dump <package> [s3db] [out.json]");
    let s3db = args.next();
    let out = args.next();
    let package = Package::open(&path).expect("open package");
    let names: BTreeMap<u32, String> = match s3db {
        Some(p) => sc_registry::Registry::open(p)
            .map(|r| {
                r.instances()
                    .iter()
                    .filter(|(_, v)| !v.name.is_empty())
                    .map(|(k, v)| (*k, v.name.clone()))
                    .collect()
            })
            .unwrap_or_default(),
        None => BTreeMap::new(),
    };

    let mut json: Vec<String> = Vec::new();
    let mut count = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        if file.get(C_BODIES_MIN).is_none() {
            continue;
        }
        // 列可能缺省：用最长的数组定行数
        let rows = [
            C_BODIES_MIN,
            C_HEADS_MIN,
            C_OUTFITS_MIN,
            C_BODIES_MAX,
            C_HEADS_MAX,
            C_OUTFITS_MAX,
            C_SCALE_MIN,
            C_SCALE_MAX,
        ]
        .iter()
        .filter_map(|c| file.get(*c).and_then(|p| arr_len(p)))
        .max()
        .unwrap_or(0);
        if rows == 0 {
            continue;
        }
        count += 1;
        let name = names
            .get(&entry.id.instance)
            .cloned()
            .unwrap_or_else(|| "?".into());
        println!(
            "\n=== g=0x{:08X} i=0x{:08X} rows={rows} {name}",
            entry.id.group, entry.id.instance
        );
        let mut row_js: Vec<String> = Vec::new();
        for r in 0..rows {
            let bmin = u8_at(&file, C_BODIES_MIN, r);
            let bmax = u8_at(&file, C_BODIES_MAX, r);
            let hmin = u8_at(&file, C_HEADS_MIN, r);
            let hmax = u8_at(&file, C_HEADS_MAX, r);
            let omin = u8_at(&file, C_OUTFITS_MIN, r);
            let omax = u8_at(&file, C_OUTFITS_MAX, r);
            let smin = f32_at(&file, C_SCALE_MIN, r);
            let smax = f32_at(&file, C_SCALE_MAX, r);
            println!(
                "  row{r:<2} body {bmin:?}..{bmax:?}  head {hmin:?}..{hmax:?}  outfit {omin:?}..{omax:?}  scale {smin:?}..{smax:?}"
            );
            row_js.push(format!(
                "{{\"b\":[{},{},{}],\"h\":[{},{},{}],\"o\":[{},{},{}],\"s\":[{}]}}",
                num(bmin.0),
                num(bmin.1),
                num(bmax.1),
                num(hmin.0),
                num(hmin.1),
                num(hmax.1),
                num(omin.0),
                num(omin.1),
                num(omax.1),
                match (smin, smax) {
                    (Some(a), Some(b)) => format!("{a},{b}"),
                    (Some(a), None) => format!("{a}"),
                    _ => String::new(),
                }
            ));
        }
        json.push(format!(
            "{{\"g\":{},\"i\":{},\"name\":\"{}\",\"rows\":[{}]}}",
            entry.id.group,
            entry.id.instance,
            name.replace('"', "'"),
            row_js.join(",")
        ));
    }
    println!("\n--- appearance definitions: {count}");
    if let Some(out) = out {
        std::fs::write(&out, format!("[{}]", json.join(",\n"))).expect("write out");
        println!("written {out}");
    }
}

fn num(v: Option<u8>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn arr_len(p: &sc_properties::Property) -> Option<usize> {
    match &p.kind {
        Kind::Array(items) => Some(items.len()),
        Kind::Scalar(_) => Some(1),
        _ => None,
    }
}

fn at(p: &sc_properties::Property, i: usize) -> Option<&Value> {
    match &p.kind {
        Kind::Array(items) => items.get(i),
        Kind::Scalar(v) => (i == 0).then_some(v),
        _ => None,
    }
}

fn u8_at(file: &PropertyFile, col: u32, i: usize) -> (Option<u8>, Option<u8>) {
    let Some(p) = file.get(col) else {
        return (None, None);
    };
    match at(p, i) {
        Some(Value::Key(k)) => (Some((k.instance & 0xFF) as u8), Some((k.instance & 0xFF) as u8)),
        Some(Value::Int32(v)) => (Some(*v as u8), Some(*v as u8)),
        Some(Value::UInt32(v)) => (Some(*v as u8), Some(*v as u8)),
        _ => (None, None),
    }
}

fn f32_at(file: &PropertyFile, col: u32, i: usize) -> Option<f32> {
    let p = file.get(col)?;
    match at(p, i) {
        Some(Value::Float(f)) => Some(*f),
        _ => None,
    }
}
