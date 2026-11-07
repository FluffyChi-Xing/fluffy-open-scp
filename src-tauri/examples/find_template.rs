//! Locate a template/unit property file by instance across packages, and
//! dump its bundle table (07E0AD23/24).
//! Usage: ... -- <package>... <hex-instance>
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let want: u32 = u32::from_str_radix(
        args.last().expect("instance").trim_start_matches("0x"),
        16,
    )
    .expect("hex");
    for path in &args[2..args.len() - 1] {
        let Ok(package) = Package::open(std::path::Path::new(path)) else { continue };
        for entry in package.entries() {
            if entry.id.type_id != 0x00B1_B104 || entry.id.instance != want {
                continue;
            }
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&bytes) else { continue };
            let bundles = file.get(0x07E0_AD23).and_then(|p| match &p.kind {
                Kind::Array(vs) => Some(
                    vs.iter()
                        .filter_map(|v| match v {
                            Value::Key(k) => Some(k.instance),
                            _ => None,
                        })
                        .collect::<Vec<u32>>(),
                ),
                _ => None,
            });
            println!(
                "{} {:#010x}:{:#010x}:{:08X}  size={}B",
                std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                entry.id.type_id,
                entry.id.group,
                entry.id.instance,
                bytes.len()
            );
            if let Some(bundles) = bundles {
                println!("  bundles({}):", bundles.len());
                for b in &bundles {
                    println!("    {b:08X}");
                }
            } else {
                println!("  (无 07E0AD23 bundle 表)");
                for property in &file.values {
                    let brief = match &property.kind {
                        Kind::Scalar(Value::Key(k)) => format!("Key {:08X}", k.instance),
                        Kind::Scalar(Value::Float(x)) => format!("Float {x}"),
                        Kind::Scalar(Value::Int32(x)) => format!("Int32 {x}"),
                        other => {
                            let t = format!("{other:?}");
                            t.chars().take(40).collect()
                        }
                    };
                    println!("    {:08X} {}", property.hash, brief);
                }
            }
        }
    }
}
