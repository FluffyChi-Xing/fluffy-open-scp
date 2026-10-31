//! 探针：在目录下全部包中查找包含指定 Text 引用（table,instance）的
//! property 资源（0x00B1B104），输出 TGI 与命中的属性 hash。仅开发用。
//!
//! 用法：cargo run -p probes --release --example text_ref_hunt --
//!   <目录或包> <table_hex> <instance_hex> [table_hex instance_hex ...]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::path::{Path, PathBuf};

fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(&p, out);
            } else if p.extension().is_some_and(|x| x == "package") {
                out.push(p);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(&args[0]);
    let want: Vec<(u32, u32)> = args[1..]
        .chunks(2)
        .map(|c| {
            (
                u32::from_str_radix(c[0].trim_start_matches("0x"), 16).unwrap(),
                u32::from_str_radix(c[1].trim_start_matches("0x"), 16).unwrap(),
            )
        })
        .collect();
    let mut pkgs = Vec::new();
    if root.is_dir() {
        collect(&root, &mut pkgs);
    } else {
        pkgs.push(root.clone());
    }
    for path in &pkgs {
        let Ok(pkg) = Package::open(path) else {
            continue;
        };
        for e in pkg.entries() {
            if e.id.type_id != 0x00B1_B104 {
                continue;
            }
            let Ok(data) = pkg.read(e) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else {
                continue;
            };
            let mut hits: Vec<u32> = Vec::new();
            for prop in &file.values {
                let vals: Vec<&Value> = match &prop.kind {
                    Kind::Scalar(v) => vec![v],
                    Kind::Array(vs) => vs.iter().collect(),
                    Kind::Empty => vec![],
                };
                for v in vals {
                    if let Value::Text(t) = v {
                        if want.contains(&(t.table_id, t.instance_id)) {
                            hits.push(prop.hash);
                        }
                    }
                }
            }
            if !hits.is_empty() {
                let name = path.file_name().unwrap().to_string_lossy();
                println!(
                    "{name} {:08X}:{:08X}:{:08X} <- {}",
                    e.id.type_id,
                    e.id.group,
                    e.id.instance,
                    hits.iter()
                        .map(|h| format!("{h:08X}"))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
    }
}
