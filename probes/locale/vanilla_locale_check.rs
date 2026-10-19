//! 探针：在原版 locale 包里查找 overlay 属性引用的字符串 ID，并对比
//! overlay 表与原版表的字节格式。仅开发用。
//!
//! 用法：cargo run -p probes --release --example vanilla_locale_check --
//!   <overlay.package> <zh-tw Data.package> [en-us Data.package]
use dbpf::Package;
use sc_properties::locale::LOCALE_RESOURCE_TYPE;
use std::collections::BTreeMap;

const WANT: [u32; 9] = [
    0x0E94_855A, // overlay 引用，overlay 表缺失
    0x0EA1_FFF1, // 同上
    0x0EBC_E055, // 同上
    0x0494_EEC2, // overlay 表内 = 消防局
    0x32F3_974A, // overlay 表内 = 城市擁有鎮公所
    0xBFCB_5E74, // overlay 表内
    0xC839_C746, // overlay 表内
    0x0D9A_B095, // baseline 引用
    0x0D9A_B094, // baseline 引用
];
const TABLES: [u32; 3] = [0x4B54_417A, 0x50AA_0BEA, 0x6C96_9DEE];

fn tables_of(p: &Package) -> BTreeMap<u32, Vec<u8>> {
    let mut m = BTreeMap::new();
    for e in p.entries() {
        if e.id.type_id == LOCALE_RESOURCE_TYPE {
            m.insert(e.id.instance, p.read(e).unwrap());
        }
    }
    m
}

fn raw_items(data: &[u8]) -> BTreeMap<String, String> {
    let text = String::from_utf8_lossy(data);
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str(text).unwrap_or_default()
}

fn fmt_bytes(data: &[u8]) -> String {
    let s = String::from_utf8_lossy(data).to_string();
    s.escape_debug().to_string()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let overlay_path = args.next().unwrap();
    let zh_path = args.next().unwrap();
    let en_path = args.next();

    let overlay = Package::open(&overlay_path).unwrap();
    let zh = Package::open(&zh_path).unwrap();
    let o_tabs = tables_of(&overlay);
    let z_tabs = tables_of(&zh);
    println!(
        "zh-tw package: {} locale tables; overlay: {} tables",
        z_tabs.len(),
        o_tabs.len()
    );

    // 1) 覆盖表 vs 原版表：字节对比 + 原版表里是否包含 overlay 的键
    for t in TABLES {
        let o = o_tabs.get(&t);
        let z = z_tabs.get(&t);
        println!("---- table {t:08X} ----");
        match (o, z) {
            (None, _) => println!("  overlay: 无"),
            (Some(o), None) => println!("  overlay {}B; 原版无此表", o.len()),
            (Some(o), Some(z)) => {
                let identical = o == z;
                println!(
                    "  overlay {}B vs 原版 {}B, 字节一致={}",
                    o.len(),
                    z.len(),
                    identical
                );
                let oz = raw_items(o);
                let zz = raw_items(z);
                for k in oz.keys() {
                    println!(
                        "    overlay 键 {k}: 原版存在={}",
                        zz.contains_key(k)
                    );
                }
                if !identical {
                    println!("  overlay 原始字节: {}", fmt_bytes(o));
                    println!("  原版原始字节(前400字符): {}", {
                        let s = fmt_bytes(z);
                        s.chars().take(400).collect::<String>()
                    });
                }
            }
        }
    }

    // 2) 在原版全部表中查找目标字符串 id
    println!("---- 在 zh-tw 全部 locale 表中查找目标 id ----");
    let mut found_any = false;
    for (t, data) in &z_tabs {
        let items = raw_items(data);
        for w in WANT {
            let key = format!("0x{w:08x}");
            if let Some(v) = items.get(&key) {
                found_any = true;
                let vt: String = v.chars().take(40).collect();
                println!("  ({t:08X}, {w:08X}) = {vt}");
            }
        }
    }
    if !found_any {
        println!("  （zh-tw 原版全部表中均未找到任何目标 id）");
    }

    // 3) en-us 同样查一遍
    if let Some(en) = en_path {
        let en = Package::open(&en).unwrap();
        let e_tabs = tables_of(&en);
        println!("---- 在 en-us 全部 locale 表中查找目标 id ----");
        let mut found_any = false;
        for (t, data) in &e_tabs {
            let items = raw_items(data);
            for w in WANT {
                let key = format!("0x{w:08x}");
                if let Some(v) = items.get(&key) {
                    found_any = true;
                    let vt: String = v.chars().take(60).collect();
                    println!("  ({t:08X}, {w:08X}) = {vt}");
                }
            }
        }
        if !found_any {
            println!("  （en-us 原版全部表中均未找到任何目标 id）");
        }
    }

    // 4) 原版表键格式样本（大小写/顺序/空白）
    for t in [0x50AA0BEAu32, 0x6C969DEE] {
        if let Some(z) = z_tabs.get(&t) {
            println!("---- 原版 zh-tw 表 {t:08X} 头部字节 ----");
            println!("  {}", {
                let s = fmt_bytes(z);
                s.chars().take(300).collect::<String>()
            });
        }
    }
}
