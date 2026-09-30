//! 冒烟工具：包 TOC 全量清单——按类型分组统计 + 全条目 TGI 列表（可选按类型过滤）。
//! 用途：社区 mod 包差异分析（BOC/外围区建造Mod 等）。
//! 用法：cargo run -p sc-exporter --release --example pkg_toc -- <pkg> [pkg...] [--type=0xB1B104]
use dbpf::Package;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut type_filter: Option<u32> = None;
    let mut paths = Vec::new();
    for a in args {
        if let Some(t) = a.strip_prefix("--type=0x").or_else(|| a.strip_prefix("--type=0X")) {
            type_filter = u32::from_str_radix(t, 16).ok();
        } else if let Some(t) = a.strip_prefix("--type=") {
            type_filter = t.parse().ok();
        } else {
            paths.push(a);
        }
    }
    for path in &paths {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let pkg = match Package::open(path) {
            Ok(p) => p,
            Err(e) => {
                println!("### {name}: 打开失败 {e}");
                continue;
            }
        };
        let mut by_type: BTreeMap<u32, Vec<(u32, u32, usize)>> = BTreeMap::new();
        for e in pkg.entries() {
            let size = e.compressed_size as usize;
            by_type.entry(e.id.type_id).or_default().push((e.id.group, e.id.instance, size));
        }
        println!("### {name}: {} 类型 / {} 条目", by_type.len(), pkg.entries().len());
        for (t, items) in &by_type {
            if let Some(f) = type_filter {
                if *t != f {
                    continue;
                }
            }
            let total: usize = items.iter().map(|s| s.2).sum();
            println!(
                "  type {t:08X}: {} 条 / {} bytes",
                items.len(),
                total
            );
            if let Some(f) = type_filter {
                if *t == f {
                    for (g, i, sz) in items {
                        println!("    G {g:08X} I {i:08X} ({sz}B)");
                    }
                }
            }
        }
        if type_filter.is_none() {
            println!();
        }
    }
}
