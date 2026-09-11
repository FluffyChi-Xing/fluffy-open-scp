//! 冒烟工具：在 shader 源（0x0469A3F7）里按关键词检索，打印命中片段。
//!
//! 用法：cargo run -p sc-exporter --release --example shader_grep -- <package> <keyword> [keyword...]

use dbpf::Package;

const SHADER_TYPE: u32 = 0x0469_A3F7;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, keywords) = args
        .split_first()
        .expect("usage: shader_grep <package> <keyword> [keyword...]");
    if let Some(target) = path.strip_prefix("dump:") {
        // dump:<package>:<instance_hex> → 输出全文到 stdout
        let (pkg, inst_raw) = target.rsplit_once(':').expect("dump:<package>:<instance>");
        let inst = u32::from_str_radix(inst_raw.trim_start_matches("0x"), 16).unwrap();
        let package = Package::open(pkg).expect("open");
        let entry = package
            .entries()
            .iter()
            .find(|e| e.id.type_id == SHADER_TYPE && e.id.instance == inst)
            .cloned()
            .expect("shader not found");
        let raw = package.read(&entry).unwrap();
        use std::io::Write;
        std::io::stdout().write_all(&raw).unwrap();
        return;
    }
    assert!(!keywords.is_empty(), "need at least one keyword");
    let package = Package::open(path).expect("open package");
    let mut hits = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != SHADER_TYPE {
            continue;
        }
        let Ok(raw) = package.read(entry) else { continue };
        let text = String::from_utf8_lossy(&raw).to_string();
        let lower = text.to_lowercase();
        if !keywords
            .iter()
            .all(|k| lower.contains(&k.to_lowercase()))
        {
            continue;
        }
        hits += 1;
        println!(
            "== shader 0x{:08x} ({} bytes) ==",
            entry.id.instance,
            raw.len()
        );
        // 打印每个关键词命中行 ±6 行上下文
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            if keywords
                .iter()
                .any(|k| line_lower.contains(&k.to_lowercase()))
            {
                let start = index.saturating_sub(6);
                let end = (index + 7).min(lines.len());
                for context in &lines[start..end] {
                    println!("  {context}");
                }
                println!("  ----");
            }
        }
    }
    println!("matched shaders: {hits}");
}
