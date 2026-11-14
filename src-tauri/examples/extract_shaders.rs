//! 从 DBPF 包里提取 DXBC 着色器字节码到磁盘（只读源包）。
//!
//! 用法：
//!   cargo run -p fluffy-open-scp --example extract_shaders -- <out_dir> <package> [<package>...]
//!
//! 判定：解压后的资源以 `DXBC`（0x44 0x58 0x42 0x43）开头。输出文件名
//! `<type>_<group>_<instance>.dxbc`，同名不同包时追加序号，避免覆盖。
//! 输出目录约定放在 `tmp/`（.gitignore 已排除）。

use dbpf::Package;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: extract_shaders <out_dir> <package> [<package>...]");
        std::process::exit(2);
    }
    let out_dir = PathBuf::from(&args[0]);
    fs::create_dir_all(&out_dir).expect("create out dir");
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut manifest = String::new();
    let mut total = 0usize;

    for package_path in &args[1..] {
        let package = Package::open(package_path).expect("open package");
        let mut count = 0usize;
        for entry in package.entries() {
            let Ok(bytes) = package.read(entry) else { continue };
            // 两种形态都收：DXBC 字节码，或随包分发的 HLSL 文本
            // （`technique`/`SamplerState`/`float4 ` 等标记）。
            let markers: [&[u8]; 4] = [b"DXBC", b"technique", b"SamplerState", b"float4 "];
            let head = &bytes[..bytes.len().min(4096)];
            let found = markers.iter().find_map(|m| {
                head.windows(m.len())
                    .position(|w| w == *m)
                    .map(|offset| (offset, *m))
            });
            let Some((offset, marker)) = found else { continue };
            let bytes = &bytes[offset..];
            let _ = marker;
            let id = entry.id;
            let base = format!(
                "{:08X}_{:08X}_{:08X}",
                id.type_id, id.group, id.instance
            );
            let index = seen.entry(base.clone()).or_insert(0);
            let name = if *index == 0 {
                format!("{base}.dxbc")
            } else {
                format!("{base}_{index}.dxbc")
            };
            *index += 1;
            let path: PathBuf = out_dir.join(&name);
            fs::write(&path, &bytes).expect("write shader");
            manifest.push_str(&format!(
                "{name}\t{}\t{}\n",
                Path::new(package_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                bytes.len()
            ));
            count += 1;
        }
        println!("{}: {count} shaders", package_path);
        total += count;
    }
    fs::write(out_dir.join("INDEX.tsv"), manifest).expect("write index");
    println!("total {total} shaders -> {}", out_dir.display());
}
