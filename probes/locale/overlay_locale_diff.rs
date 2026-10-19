//! 探针：对比 baseline 包与 OpenSCP 产出 overlay 包的 locale 词条资源字节与
//! 属性表 Text 引用，定位游戏内名称显示 `***` 的差异。仅开发用。
//!
//! 用法：cargo run -p probes --release --example overlay_locale_diff --
//!   <baseline.package> <overlay.package>
use dbpf::Package;
use sc_properties::locale::LOCALE_RESOURCE_TYPE;
use sc_properties::{PropertyFile, Value};
use std::collections::BTreeMap;

const TYPE_PROPERTY: u32 = 0x00B1_B104;

fn census(p: &Package, label: &str) -> BTreeMap<u32, usize> {
    let mut m: BTreeMap<u32, usize> = BTreeMap::new();
    for e in p.entries() {
        *m.entry(e.id.type_id).or_default() += 1;
    }
    println!("== {label}: {} entries ==", p.entries().len());
    for (t, n) in &m {
        println!("  type {:08X}: {}", t, n);
    }
    m
}

/// 读出全部 locale 表：instance -> 原始字节。
fn locale_tables(p: &Package, label: &str) -> BTreeMap<u32, Vec<u8>> {
    let mut m = BTreeMap::new();
    for e in p.entries() {
        if e.id.type_id != LOCALE_RESOURCE_TYPE {
            continue;
        }
        let data = p.read(e).expect("read locale");
        println!(
            "[{label}] locale {:08X} group {:08X}: stored={} raw={} compressed={}",
            e.id.instance,
            e.id.group,
            e.stored_len(),
            data.len(),
            e.compressed
        );
        m.insert(e.id.instance, data);
    }
    m
}

fn preview(data: &[u8], n: usize) -> String {
    let s: String = data
        .iter()
        .take(n)
        .map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' })
        .collect();
    s
}

/// 解析表（保留原始键，含大小写），返回 (原始键 -> 文本)。
fn raw_items(data: &[u8]) -> BTreeMap<String, String> {
    let text = String::from_utf8_lossy(data);
    let text = text.trim_start_matches('\u{feff}');
    match serde_json::from_str::<BTreeMap<String, String>>(text) {
        Ok(m) => m,
        Err(e) => {
            println!("    !! JSON parse error: {e}");
            BTreeMap::new()
        }
    }
}

fn dump_locale(label: &str, tables: &BTreeMap<u32, Vec<u8>>, max_text: usize) {
    println!("---- locale tables [{label}] ----");
    for (inst, data) in tables {
        let items = raw_items(data);
        println!(
            "table {:08X}: {}B prefix={:02X?} entries={} head={:?}",
            inst,
            data.len(),
            &data[..data.len().min(4)],
            items.len(),
            preview(data, 60)
        );
        for (k, v) in items.iter().take(200) {
            let vt: String = v.chars().take(max_text).collect();
            println!("    {k} = {vt}");
        }
        if items.len() > 200 {
            println!("    ... ({} more)", items.len() - 200);
        }
    }
}

/// 全包属性里的 Text 引用：((table, instance) -> 出现处的 TGI+hash 列表)。
fn text_refs(p: &Package, label: &str) -> BTreeMap<(u32, u32), Vec<String>> {
    let mut refs: BTreeMap<(u32, u32), Vec<String>> = BTreeMap::new();
    for e in p.entries() {
        if e.id.type_id != TYPE_PROPERTY {
            continue;
        }
        let data = match p.read(e) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let Ok(file) = PropertyFile::parse(&data) else {
            println!("[{label}] prop {:08X}:{:08X}:{:08X} PARSE FAIL", e.id.type_id, e.id.group, e.id.instance);
            continue;
        };
        for prop in &file.values {
            let single = match &prop.kind {
                sc_properties::Kind::Scalar(v) => Some(v),
                sc_properties::Kind::Array(vs) => {
                    for v in vs {
                        if let Value::Text(t) = v {
                            refs.entry((t.table_id, t.instance_id)).or_default().push(format!(
                                "{:08X}:{:08X}#{:08X}",
                                e.id.type_id, e.id.group, e.id.instance
                            ));
                        }
                    }
                    None
                }
                sc_properties::Kind::Empty => None,
            };
            if let Some(Value::Text(t)) = single {
                refs.entry((t.table_id, t.instance_id)).or_default().push(format!(
                    "{:08X}:{:08X}#{:08X}",
                    e.id.type_id, e.id.group, e.id.instance
                ));
            }
        }
    }
    println!(
        "---- text refs [{label}]: {} distinct (table,instance) ----",
        refs.len()
    );
    for ((t, i), srcs) in &refs {
        println!("  ({t:08X}, {i:08X}) <- {}", srcs.join(", "));
    }
    refs
}

fn main() {
    let mut args = std::env::args().skip(1);
    let baseline_path = args.next().expect("baseline path");
    let overlay_path = args.next().expect("overlay path");

    let baseline = Package::open(&baseline_path).unwrap();
    let overlay = Package::open(&overlay_path).unwrap();

    census(&baseline, "baseline");
    census(&overlay, "overlay");

    let b_tables = locale_tables(&baseline, "baseline");
    let o_tables = locale_tables(&overlay, "overlay");

    dump_locale("baseline", &b_tables, 40);
    dump_locale("overlay", &o_tables, 40);

    // 表级差异
    println!("---- table-level diff ----");
    for (inst, b_data) in &b_tables {
        match o_tables.get(inst) {
            None => println!("table {inst:08X}: only in baseline"),
            Some(o_data) => {
                if b_data == o_data {
                    println!("table {inst:08X}: identical bytes");
                } else {
                    println!("table {inst:08X}: DIFFERS ({}B vs {}B)", b_data.len(), o_data.len());
                    let bi = raw_items(b_data);
                    let oi = raw_items(o_data);
                    for (k, bv) in &bi {
                        match oi.get(k) {
                            None => println!("    key {k}: only in baseline = {bv:.30}"),
                            Some(ov) => {
                                if bv != ov {
                                    println!("    key {k}: text differs");
                                }
                            }
                        }
                    }
                    for k in oi.keys() {
                        if !bi.contains_key(k) {
                            println!("    key {k}: only in overlay");
                        }
                    }
                    // 大小写不敏感对比：找出仅大小写不同的键对
                    let lower_b: BTreeMap<String, &String> =
                        bi.keys().map(|k| (k.to_lowercase(), k)).collect();
                    for k in oi.keys() {
                        let lk = k.to_lowercase();
                        if !bi.contains_key(k) {
                            if let Some(bk) = lower_b.get(&lk) {
                                println!("    CASE-ONLY DIFF: overlay key {k} vs baseline key {bk}");
                            }
                        }
                    }
                }
            }
        }
    }
    for inst in o_tables.keys() {
        if !b_tables.contains_key(inst) {
            println!("table {inst:08X}: only in overlay");
        }
    }

    let _ = text_refs(&baseline, "baseline");
    let _ = text_refs(&overlay, "overlay");
}
