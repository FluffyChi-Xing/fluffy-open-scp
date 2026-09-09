//! GlassBox 引擎痕迹探针：在指定 package 中解出目标 typeId 的资源，
//! 打印头部 hex+ASCII、可打印字符串样本，并统计关键词命中。
//!
//! ```text
//! cargo run -p dbpf --example probe_engine -- <package> <typeid-hex>... [--keywords k1,k2]
//! ```

fn main() -> dbpf::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut keywords: Vec<String> = vec![
        "glassbox", "engine", "simcity", "rule", "module", "lua", "python",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    if let Some(pos) = args.iter().position(|a| a == "--keywords") {
        keywords = args[pos + 1].split(',').map(String::from).collect();
        args.drain(pos..=pos + 1);
    }
    // --scan-text <typeid>：扫描该类型全部资源，按关键词总命中排序输出 top TGI
    if let Some(pos) = args.iter().position(|a| a == "--scan-text") {
        let type_id =
            u32::from_str_radix(args[pos + 1].trim_start_matches("0x"), 16).expect("hex typeid");
        args.drain(pos..=pos + 1);
        let path = args
            .first()
            .expect("usage: probe_engine <package> --scan-text <typeid> [--keywords ...]");
        return scan_text(path, type_id, &keywords);
    }
    let path = args
        .first()
        .expect("usage: probe_engine <package> <typeid-hex>... [--keywords k1,k2] [--group hex]")
        .clone();
    // --group <hex>：按 group 过滤
    let mut group_filter: Option<u32> = None;
    if let Some(pos) = args.iter().position(|a| a == "--group") {
        group_filter = Some(
            u32::from_str_radix(args[pos + 1].trim_start_matches("0x"), 16).expect("hex group"),
        );
        args.drain(pos..=pos + 1);
    }
    let types: Vec<u32> = args[1..]
        .iter()
        .map(|t| u32::from_str_radix(t.trim_start_matches("0x"), 16).expect("hex typeid"))
        .collect();

    let package = dbpf::Package::open(path)?;
    let matches: Vec<_> = package
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            types.contains(&e.id.type_id)
                && group_filter.is_none_or(|g| e.id.group == g)
        })
        .collect();
    println!(
        "package {} — {} entries, {} match types {:08X?}",
        package.path().display(),
        package.entries().len(),
        matches.len(),
        types
    );

    for (idx, (n, entry)) in matches.iter().take(6).enumerate() {
        let data = match package.read(entry) {
            Ok(data) => data,
            Err(error) => {
                println!("\n#{n} {} — read failed: {error}", entry.id);
                continue;
            }
        };
        println!(
            "\n===== #{n} tgi={} ({} bytes, entry {}) =====",
            entry.id,
            data.len(),
            n
        );
        // 头部 128B hex+ascii
        for chunk in data.chunks(16).take(8) {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
            let ascii: String = chunk
                .iter()
                .map(|&b| {
                    if (0x20..0x7F).contains(&b) {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            println!(
                "  {:04X}  {:<47}  {}",
                chunk.as_ptr() as usize - data.as_ptr() as usize,
                hex.join(" "),
                ascii
            );
        }
        // 关键词命中
        let lower: Vec<u8> = data.to_ascii_lowercase();
        for kw in &keywords {
            let needle = kw.as_bytes();
            let count = lower.windows(needle.len()).filter(|w| *w == needle).count();
            if count > 0 {
                println!("  keyword '{kw}': {count} hits");
            }
        }
        // 前 3 个长度 ≥6 的可打印字符串段
        let mut strings: Vec<String> = Vec::new();
        let mut current = String::new();
        for &b in data.iter().take(4096) {
            if (0x20..0x7F).contains(&b) {
                current.push(b as char);
            } else {
                if current.len() >= 6 && strings.len() < 12 {
                    strings.push(current.clone());
                }
                current.clear();
            }
        }
        if !strings.is_empty() {
            println!("  strings: {}", strings.join(" | "));
        }
        if idx == 5 {
            println!("\n... (only first 6 shown)");
        }
    }
    Ok(())
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

fn scan_text(path: &str, type_id: u32, keywords: &[String]) -> dbpf::Result<()> {
    let package = dbpf::Package::open(path)?;
    let mut results: Vec<(usize, dbpf::ResourceId, Vec<(String, usize)>)> = Vec::new();
    let mut scanned = 0usize;
    for (n, entry) in package.entries().iter().enumerate() {
        if entry.id.type_id != type_id {
            continue;
        }
        scanned += 1;
        let Ok(data) = package.read(entry) else {
            continue;
        };
        let lower = data.to_ascii_lowercase();
        let mut hits: Vec<(String, usize)> = Vec::new();
        for kw in keywords {
            // "hex:<hexbytes>" 支持原始字节匹配（如属性哈希 6041610C），
            // 此时直接在未转小写的原文上匹配。
            let (count, label) = if let Some(hex) = kw.strip_prefix("hex:") {
                let needle = hex_bytes(hex);
                (
                    data.windows(needle.len())
                        .filter(|w| *w == needle.as_slice())
                        .count(),
                    kw.clone(),
                )
            } else {
                let needle = kw.as_bytes();
                (
                    lower.windows(needle.len()).filter(|w| *w == needle).count(),
                    kw.clone(),
                )
            };
            if count > 0 {
                hits.push((label, count));
            }
        }
        if !hits.is_empty() {
            results.push((n, entry.id, hits));
        }
    }
    let total: usize = results
        .iter()
        .map(|(_, _, hits)| hits.iter().map(|(_, n)| n).sum::<usize>())
        .sum();
    results.sort_by_key(|(_, _, hits)| {
        std::cmp::Reverse(hits.iter().map(|(_, n)| n).sum::<usize>())
    });
    println!(
        "scanned {scanned} resources of type {type_id:08X} — {} with hits, {total} total hits",
        results.len()
    );
    for (n, id, hits) in results.iter().take(25) {
        let summary: Vec<String> = hits.iter().map(|(k, n)| format!("{k}×{n}")).collect();
        println!("  #{n} {id}  {}", summary.join(" "));
    }
    Ok(())
}
