//! 冒烟探针：从 cpp(0x0469a3f7) shader 容器抽名字，穷举哈希算法匹配 0x2D 引用值。
//!
//! 用法：cargo run -p rw4 --release --example shader_name_hash -- <package> [target...]

const CPP_TYPE: u32 = 0x0469_A3F7;

fn fnv1a(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in data {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn fnv1(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in data {
        h = h.wrapping_mul(0x0100_0193);
        h ^= u32::from(*b);
    }
    h
}

fn djb2(data: &[u8]) -> u32 {
    let mut h: u32 = 5381;
    for b in data {
        h = h.wrapping_mul(33).wrapping_add(u32::from(*b));
    }
    h
}

fn sdbm(data: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for b in data {
        h = u32::from(*b)
            .wrapping_add(h.wrapping_shl(6))
            .wrapping_add(h.wrapping_shl(16))
            .wrapping_sub(h);
    }
    h
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, targets) = args.split_first().expect("usage: shader_name_hash <package> [target...]");
    let targets: Vec<u32> = targets
        .iter()
        .filter_map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .collect();
    let package = dbpf::Package::open(path).expect("open package");

    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut cpp_count = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != CPP_TYPE {
            continue;
        }
        cpp_count += 1;
        let entry = entry.clone();
        let Ok(data) = package.read(&entry) else { continue };
        // 抽取 \0 结尾的标识符串（3~40 字符，字母/数字/下划线开头为字母）
        let mut start: Option<usize> = None;
        for (i, &b) in data.iter().enumerate() {
            let ident = b.is_ascii_alphanumeric() || b == b'_';
            if ident && start.is_none() {
                start = Some(i);
            } else if !ident {
                if let Some(s) = start {
                    let len = i - s;
                    if (3..=40).contains(&len)
                        && data[s].is_ascii_alphabetic()
                        && data[s..i]
                            .iter()
                            .all(|c| c.is_ascii_alphanumeric() || *c == b'_')
                    {
                        names.insert(String::from_utf8_lossy(&data[s..i]).to_string());
                    }
                    start = None;
                }
            }
        }
    }
    println!("cpp resources: {cpp_count}, unique identifier strings: {}", names.len());

    // 找含投影/UV 数学关键字的 shader 源码并落盘
    let keywords = ["frac(", "TEXCOORD", "worldToClip", "atlas", "Pallast", "facade"];
    let mut dumped = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != CPP_TYPE {
            continue;
        }
        let entry = entry.clone();
        let Ok(data) = package.read(&entry) else { continue };
        let text = String::from_utf8_lossy(&data);
        let hits: Vec<&str> = keywords.iter().filter(|k| text.contains(*k)).copied().collect();
        if hits.is_empty() {
            continue;
        }
        let name = format!("cpp_0x{:08X}_{}.txt", entry.id.instance, hits.join("-").replace('(', ""));
        std::fs::write(format!("tmp/shaders/{name}"), text.as_bytes()).ok();
        println!(
            "cpp 0x{:08X}: {} bytes, keywords [{}]",
            entry.id.instance,
            data.len(),
            hits.join(",")
        );
        dumped += 1;
    }
    println!("dumped {dumped} shader sources to tmp/shaders/");

    let algorithms: [(&str, fn(&[u8]) -> u32); 4] = [
        ("fnv1a", fnv1a),
        ("fnv1", fnv1),
        ("djb2", djb2),
        ("sdbm", sdbm),
    ];
    let mut hits = 0usize;
    for name in &names {
        let bytes = name.as_bytes();
        for (label, algo) in &algorithms {
            let h = algo(bytes);
            if targets.contains(&h) {
                println!("MATCH: {name:?} {label} = 0x{h:08X}");
                hits += 1;
            }
            // 变体：小写 / 大写
            let lower: Vec<u8> = bytes.to_ascii_lowercase();
            if targets.contains(&algo(&lower)) {
                println!("MATCH: {name:?} (lower) {label}");
                hits += 1;
            }
        }
    }
    println!("hash hits: {hits}");
}
