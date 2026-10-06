//! 扫描游戏 .package：查找 render script（含 "scDecals" 字节）与 renderGroup 魔法值 0x96AF4B50。
//! 用法: cargo run -p dbpf --example scan_decals -- <package目录> [输出目录]

use dbpf::Package;
use std::fs;
use std::path::{Path, PathBuf};

const NEEDLE_SCRIPT: &[u8] = b"scDecals";
const MAGIC_GROUP1: u32 = 0x96AF4B50;
const DECAL_SET_BASE: u32 = 0x0D10_9050;
/// 解压后超过该大小的条目跳过（地形/音频等大资源与本调查无关）
const MAX_SCAN_SIZE: usize = 64 * 1024 * 1024;

fn collect_packages(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_packages(&p, out);
        } else if p.extension().is_some_and(|x| x == "package") {
            out.push(p);
        }
    }
}

fn contains_u32(hay: &[u8], v: u32) -> bool {
    let le = v.to_le_bytes();
    hay.windows(4).any(|w| w == le)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("需要 package 目录"));
    let out_dir = PathBuf::from(args.next().unwrap_or_else(|| "tmp/renderscript".into()));
    fs::create_dir_all(&out_dir).ok();

    let mut packages = Vec::new();
    collect_packages(&dir, &mut packages);
    println!("发现 {} 个 package", packages.len());

    for pkg_path in packages {
        let pkg = match Package::open(&pkg_path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("跳过 {}: {e}", pkg_path.display());
                continue;
            }
        };
        let pkg_name = pkg_path.file_name().unwrap().to_string_lossy().to_string();
        let mut hits = 0usize;
        for entry in pkg.entries() {
            let raw = match pkg.read_raw(entry) {
                Ok(r) => r,
                Err(_) => continue,
            };
            // 未压缩条目直接扫；压缩条目先解压（限制大小）
            let data_owned;
            let data: &[u8] = if raw.len() >= 2 && raw[0] == 0x10 && raw[1] == 0xFB {
                match pkg.read(entry) {
                    Ok(d) => {
                        if d.len() > MAX_SCAN_SIZE {
                            continue;
                        }
                        data_owned = d;
                        &data_owned
                    }
                    Err(_) => continue,
                }
            } else {
                if raw.len() > MAX_SCAN_SIZE {
                    continue;
                }
                raw
            };
            let id = entry.id;
            let has_script = data.windows(8).any(|w| w == NEEDLE_SCRIPT);
            let has_group1 = contains_u32(data, MAGIC_GROUP1);
            let has_decalset = contains_u32(data, DECAL_SET_BASE);
            if has_script || has_group1 {
                hits += 1;
                println!(
                    "[HIT] {} T{:08X} G{:08X} I{:08X} size={} script={} group1={} decalset={}",
                    pkg_name, id.type_id, id.group, id.instance,
                    data.len(), has_script, has_group1, has_decalset
                );
                if has_script {
                    let fname = format!(
                        "{}/{}_{:08X}_{:08X}_{:08X}.bin",
                        out_dir.display(),
                        pkg_name.trim_end_matches(".package"),
                        id.type_id, id.group, id.instance
                    );
                    fs::write(&fname, data).ok();
                    println!("  dumped -> {fname}");
                }
            }
        }
        if hits > 0 {
            println!("{}: {} 处命中", pkg_name, hits);
        }
    }
    println!("扫描完成");
}
