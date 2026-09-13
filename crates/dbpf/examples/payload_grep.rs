//! 探针：在包内**解压后**的资源负载里搜 ASCII 子串（DBPF 条目多为 RefPack 压缩，
//! 直接对文件做 grep 会漏）。用于定位 UI 布局/分类表等文本型资源。仅开发用。
//!
//! 用法：
//!   cargo run -p dbpf --release --example payload_grep -- <needle>[,<needle>...] <package...>
//!   [--context=N] 打印命中处前后 N 字符（默认 120）
//!   [--limit=N]   每个 needle 最多打印多少条命中（默认 6）

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let needles: Vec<String> = args
        .first()
        .expect("usage: payload_grep <needle[,needle...]> <package...>")
        .split(',')
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let context: usize = args
        .iter()
        .find_map(|a| a.strip_prefix("--context=").and_then(|v| v.parse().ok()))
        .unwrap_or(120);
    let limit: usize = args
        .iter()
        .find_map(|a| a.strip_prefix("--limit=").and_then(|v| v.parse().ok()))
        .unwrap_or(6);
    let paths: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();

    let mut counts = vec![0usize; needles.len()];
    for path in &paths {
        let Ok(package) = dbpf::Package::open(path) else { continue };
        let label = path
            .split(['/', '\\'])
            .next_back()
            .unwrap_or(path)
            .to_string();
        for (index_in_package, entry) in package.entries().iter().enumerate() {
            let Ok(data) = package.read(entry) else { continue };
            // 负载可能是二进制；按 latin1 角度做字节级搜索
            let lower: Vec<u8> = data.iter().map(|b| b.to_ascii_lowercase()).collect();
            for (index, needle) in needles.iter().enumerate() {
                let Some(pos) = find(&lower, needle.as_bytes()) else { continue };
                counts[index] += 1;
                if counts[index] > limit {
                    continue;
                }
                let start = pos.saturating_sub(context / 2);
                let end = (pos + context).min(data.len());
                let snippet: String = data[start..end]
                    .iter()
                    .map(|b| {
                        if (0x20..0x7f).contains(b) {
                            *b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                println!(
                    "[{needle}] {} idx={index_in_package} type=0x{:08X} group=0x{:08X} instance=0x{:08X} size={} @{pos}",
                    label, entry.id.type_id, entry.id.group, entry.id.instance, data.len()
                );
                println!("    …{snippet}…");
            }
        }
    }
    println!("\n=== 命中统计 ===");
    for (index, needle) in needles.iter().enumerate() {
        println!("  {needle}: {}", counts[index]);
    }
    let _ = std::io::stdout().flush();
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}
