//! 生成 locale 伴生测试包：把 overlay 的新增词条与原版表合并，按
//! 宽松/紧凑两种排版 × 全量/部分两种范围输出 3 个对照包，用于实机
//! 判定 LocaleManager 的加载目录、JSON 排版敏感性与同名表合并语义。
//!
//! 用法：cargo run -p probes --release --example locale_companion_build --
//!   <overlay.package> <原版Locale语言目录Data.package> <输出目录>
//!
//! 产物（写入 <输出目录>）：
//!   companion-full-pretty.package    原版全表 + 新增词条，键序/缩进/冒号风格仿原版
//!   companion-full-compact.package   原版全表 + 新增词条，紧凑 JSON（OpenSCP 风格）
//!   companion-partial-pretty.package 仅新增词条，仿原版排版
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::locale::LOCALE_RESOURCE_TYPE;

const GROUP: u32 = 0x02FA_BF01;

/// 表内容：有序 (原始键, 文本)，注释键（"//"）原样保留。
type Table = Vec<(String, String)>;

/// 保序解析扁平 string:string JSON（BOM 后）。只支持一层对象，键值均为字符串。
fn parse_ordered(data: &[u8]) -> Result<Table, String> {
    let text = String::from_utf8(data.to_vec()).map_err(|e| e.to_string())?;
    let mut s = text.trim_start_matches('\u{feff}').trim();
    let unquote = |tok: &str| -> Result<String, String> {
        serde_json::from_str::<String>(tok).map_err(|e| format!("bad string token: {e}"))
    };
    let mut out = Vec::new();
    let mut scan = |s: &str| -> Result<Table, String> {
        let b = s.as_bytes();
        let mut i = 0usize;
        let mut res = Vec::new();
        if b[i] != b'{' {
            return Err("expected {".into());
        }
        i += 1;
        loop {
            while i < b.len() && (b[i] as char).is_whitespace() {
                i += 1;
            }
            if i >= b.len() {
                return Err("unexpected eof".into());
            }
            if b[i] == b'}' {
                break;
            }
            if b[i] != b'"' {
                return Err(format!("expected key quote at {i}"));
            }
            let kstart = i;
            i += 1;
            while b[i] != b'"' || b[i - 1] == b'\\' {
                i += 1;
            }
            let key = unquote(&s[kstart..=i])?;
            i += 1;
            while i < b.len() && (b[i] as char).is_whitespace() {
                i += 1;
            }
            if b[i] != b':' {
                return Err(format!("expected colon at {i}"));
            }
            i += 1;
            while i < b.len() && (b[i] as char).is_whitespace() {
                i += 1;
            }
            if b[i] != b'"' {
                return Err(format!("expected value quote at {i}"));
            }
            let vstart = i;
            i += 1;
            while b[i] != b'"' || b[i - 1] == b'\\' {
                i += 1;
            }
            let val = unquote(&s[vstart..=i])?;
            i += 1;
            res.push((key, val));
            while i < b.len() && (b[i] as char).is_whitespace() {
                i += 1;
            }
            if i < b.len() && b[i] == b',' {
                i += 1;
            }
        }
        Ok(res)
    };
    out = scan(s)?;
    Ok(out)
}

fn escape(v: &str) -> String {
    serde_json::to_string(v).expect("escape")
}

/// 仿原版排版：BOM + `{\r\n    "k" : "v",\r\n ...}`（尾键后换行缩进再收 `}`）。
fn serialize_pretty(t: &Table) -> Vec<u8> {
    let mut s = String::from("\u{feff}{\r\n");
    for (i, (k, v)) in t.iter().enumerate() {
        s.push_str("    ");
        s.push_str(&escape(k));
        s.push_str(" : ");
        s.push_str(&escape(v));
        if i + 1 < t.len() {
            s.push(',');
        }
        s.push_str("\r\n");
    }
    s.push('}');
    s.into_bytes()
}

/// 紧凑排版：BOM + `{"k":"v",...}`。
fn serialize_compact(t: &Table) -> Vec<u8> {
    let mut s = String::from("\u{feff}{");
    for (i, (k, v)) in t.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&escape(k));
        s.push(':');
        s.push_str(&escape(v));
    }
    s.push('}');
    s.into_bytes()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let overlay_path = args.next().expect("overlay");
    let vanilla_path = args.next().expect("vanilla locale Data.package");
    let out_dir = args.next().expect("out dir");
    std::fs::create_dir_all(&out_dir).unwrap();

    let overlay = Package::open(&overlay_path).unwrap();
    let vanilla = Package::open(&vanilla_path).unwrap();

    // overlay 新增词条（保序）
    let mut custom: Vec<(u32, Table)> = Vec::new();
    for e in overlay.entries() {
        if e.id.type_id != LOCALE_RESOURCE_TYPE {
            continue;
        }
        let data = overlay.read(e).unwrap();
        let t = parse_ordered(&data)
            .unwrap_or_else(|err| panic!("overlay table {:08X}: {err}", e.id.instance));
        println!("overlay 表 {:08X}: {} 词条", e.id.instance, t.len());
        custom.push((e.id.instance, t));
    }

    // 原版表（保序）
    let mut vanilla_tables: Vec<(u32, Table)> = Vec::new();
    for e in vanilla.entries() {
        if e.id.type_id != LOCALE_RESOURCE_TYPE {
            continue;
        }
        let data = vanilla.read(e).unwrap();
        let t = parse_ordered(&data)
            .unwrap_or_else(|err| panic!("vanilla table {:08X}: {err}", e.id.instance));
        vanilla_tables.push((e.id.instance, t));
    }
    println!("原版 locale 表总数: {}", vanilla_tables.len());

    let mut variants: Vec<(&str, Vec<OverlayEntry>)> = Vec::new();
    let mut push_variant = |name: &'static str, full: bool, pretty: bool| {
        let mut entries = Vec::new();
        for (inst, add) in &custom {
            let table: Table = if full {
                let base = vanilla_tables
                    .iter()
                    .find(|(i, _)| i == inst)
                    .unwrap_or_else(|| panic!("原版无表 {inst:08X}"))
                    .1
                    .clone();
                let mut merged = base;
                for (k, v) in add {
                    match merged.iter_mut().find(|(ek, _)| ek == k) {
                        Some(slot) => slot.1 = v.clone(),
                        None => merged.push((k.clone(), v.clone())),
                    }
                }
                merged
            } else {
                add.clone()
            };
            let data = if pretty {
                serialize_pretty(&table)
            } else {
                serialize_compact(&table)
            };
            println!(
                "{name}: 表 {:08X} -> {} 词条 {}B",
                inst,
                table.len(),
                data.len()
            );
            entries.push(OverlayEntry::new(
                ResourceId {
                    type_id: LOCALE_RESOURCE_TYPE,
                    group: GROUP,
                    instance: *inst,
                },
                data,
            ));
        }
        variants.push((name, entries));
    };
    push_variant("companion-full-pretty", true, true);
    push_variant("companion-full-compact", true, false);
    push_variant("companion-partial-pretty", false, true);

    for (name, entries) in &variants {
        let bytes = dbpf::write_uncompressed_overlay(entries).expect("write overlay");
        let path = format!("{out_dir}/{name}.package");
        std::fs::write(&path, &bytes).unwrap();
        println!("写出 {path} ({}B)", bytes.len());
    }
}
