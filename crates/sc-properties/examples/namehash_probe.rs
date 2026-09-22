//! name-hash 穷举探针：多种名字模板/坐标域/mip 变体 × FNV1a，对照包内 instance。仅开发用。
//! 用法：namehash_probe <package> <region-group> [type-hex]
use std::collections::HashSet;

fn fnv1a(name: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}
fn fnv1(name: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in name.bytes() {
        h = h.wrapping_mul(0x0100_0193);
        h ^= b as u32;
    }
    h
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let group = u32::from_str_radix(&args.next().expect("group").trim_start_matches("0x"), 16).unwrap();
    let ty = args
        .next()
        .map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap())
        .unwrap_or(0x3E42_1F0);
    let package = dbpf::Package::open(&path).expect("open");
    let mut group_insts = HashSet::new();
    let mut all_insts = HashSet::new();
    for e in package.entries() {
        if e.id.type_id == ty {
            all_insts.insert(e.id.instance);
            if e.id.group == group {
                group_insts.insert(e.id.instance);
            }
        }
    }
    println!("group insts: {}, all-type insts: {}", group_insts.len(), all_insts.len());

    let prefixes = ["heightmap", "ecomap"];
    let lo = -(64i32);
    let hi = 64i32;
    let mut hits = 0;
    for pfx in prefixes {
        for mip in 0..10u32 {
            for x in lo..=hi {
                for y in lo..=hi {
                    for &(xf, yf) in &[("{x:02}", "{y:02}"), ("{x}", "{y}"), ("{x:03}", "{y:03}")] {
                        let name = pfx
                            .replace("{x}", &format!("_x{}", x))
                            .replace("{y}", &format!("_y{}", y))
                            .replace("{x:02}", &format!("_x{x:02}"))
                            .replace("{y:02}", &format!("_y{y:02}"))
                            .replace("{x:03}", &format!("_x{x:03}"))
                            .replace("{y:03}", &format!("_y{y:03}"));
                        let name = format!("{name}_mip{mip}");
                        for (tag, h) in [("1a", fnv1a(&name)), ("1", fnv1(&name))] {
                            for (set, label) in [(&group_insts, "grp"), (&all_insts, "all")] {
                                if set.contains(&h) {
                                    println!("HIT[{tag}/{label}] {name} -> 0x{h:08X}");
                                    hits += 1;
                                    if hits > 40 { println!("..."); return; }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("total hits: {hits}");
}
