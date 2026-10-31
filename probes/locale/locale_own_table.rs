//! 探针：把包内散落在原版表 ID 里的自定义词条合并进 OpenSCP 自有表
//! （OPENSCP_LOCALE_TABLE），并把属性中指向这些自定义串的 Text 引用的
//! 表字段改指新表（原版串引用不动）。产出单文件裸包。仅开发用。
//!
//! 用法：cargo run -p probes --release --example locale_own_table --
//!   <in.package> <out.package>
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::locale::LOCALE_RESOURCE_TYPE;
use sc_properties::locale::OPENSCP_LOCALE_TABLE;

const VANILLA_TABLES: [u32; 3] = [0x4B54_417A, 0x50AA_0BEA, 0x6C96_9DEE];
/// 这 5 个自定义串 ID 的表字段要改指；其余引用（原版串）一律不动。
const CUSTOM_IDS: [u32; 5] = [
    0x0494_EEC2,
    0xBFCB_5E74,
    0x32F3_974A,
    0x7A43_0858,
    0xC839_C746,
];

fn patch_property(blob: &mut [u8]) -> usize {
    let mut patched = 0;
    for t in VANILLA_TABLES {
        let tb = t.to_be_bytes();
        let mut i = 0;
        while i + 8 <= blob.len() {
            if blob[i..i + 4] == tb {
                let inst = u32::from_be_bytes([blob[i + 4], blob[i + 5], blob[i + 6], blob[i + 7]]);
                if CUSTOM_IDS.contains(&inst) {
                    blob[i..i + 4].copy_from_slice(&OPENSCP_LOCALE_TABLE.to_be_bytes());
                    patched += 1;
                    i += 8;
                    continue;
                }
            }
            i += 1;
        }
    }
    patched
}

fn main() {
    let mut args = std::env::args().skip(1);
    let inp = args.next().unwrap();
    let out = args.next().unwrap();
    let p = Package::open(&inp).unwrap();

    let mut merged: Vec<(String, String)> = Vec::new();
    let mut entries: Vec<OverlayEntry> = Vec::new();
    for e in p.entries() {
        let data = p.read(e).unwrap();
        if e.id.type_id == LOCALE_RESOURCE_TYPE {
            // 合并进自有表
            let text = String::from_utf8_lossy(&data);
            let text = text.trim_start_matches('\u{feff}');
            let map: std::collections::BTreeMap<String, String> =
                serde_json::from_str(text).expect("locale json");
            for (k, v) in map {
                if !merged.iter().any(|(ek, _)| *ek == k) {
                    merged.push((k, v));
                }
            }
            continue;
        }
        let mut data = data;
        if e.id.type_id == 0x00B1_B104 {
            let n = patch_property(&mut data);
            println!(
                "属性 {:08X}:{:08X}: 改指自有表 {} 处",
                e.id.type_id, e.id.instance, n
            );
        }
        entries.push(OverlayEntry::new(e.id, data));
    }

    // 自有表：仿原版宽松排版
    let mut body = String::from("\u{feff}{\r\n");
    for (i, (k, v)) in merged.iter().enumerate() {
        body.push_str(&format!(
            "    {}: {}{}\r\n",
            serde_json::to_string(k).unwrap(),
            serde_json::to_string(v).unwrap(),
            if i + 1 < merged.len() { "," } else { "" }
        ));
    }
    body.push('}');
    println!("自有表 {:08X}: {} 词条", OPENSCP_LOCALE_TABLE, merged.len());
    entries.push(OverlayEntry::new(
        ResourceId {
            type_id: LOCALE_RESOURCE_TYPE,
            group: 0x02FA_BF01,
            instance: OPENSCP_LOCALE_TABLE,
        },
        body.into_bytes(),
    ));

    let bytes = dbpf::write_uncompressed_overlay(&entries).expect("write");
    std::fs::write(&out, &bytes).expect("write file");
    println!("写出 {out}: {} 条目 {}B", entries.len(), bytes.len());
}
