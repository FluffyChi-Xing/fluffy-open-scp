//! 列出 Dalian 包全部条目 + lot 内的模型引用（离线侦察 R10 模板）。
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    println!("== entries ==");
    for e in pkg.entries() {
        println!(
            "{:08X} {:08X} {:08X} {}B",
            e.id.type_id,
            e.id.group,
            e.id.instance,
            pkg.read(e)?.len()
        );
    }
    println!("== lot model refs ==");
    for e in pkg.entries() {
        if e.id.type_id != 0x00B1_B104 {
            continue;
        }
        let pf = match PropertyFile::parse(&pkg.read(e)?) {
            Ok(p) => p,
            Err(_) => continue,
        };
        for p in &pf.values {
            match &p.kind {
                Kind::Scalar(Value::Key(k))
                    if k.type_id == 0x2F4E_681B || k.type_id == 0x2F4E_681C =>
                {
                    println!(
                        "lot {:08X}/{:08X} hash {:08X} -> t={:08X} i={:08X}",
                        e.id.group, e.id.instance, p.hash, k.type_id, k.instance
                    );
                }
                Kind::Array(vals) => {
                    for v in vals {
                        if let Value::Key(k) = v {
                            if k.type_id == 0x2F4E_681B || k.type_id == 0x2F4E_681C {
                                println!(
                                    "lot {:08X}/{:08X} hash {:08X} [arr] -> t={:08X} i={:08X}",
                                    e.id.group, e.id.instance, p.hash, k.type_id, k.instance
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let _ = ResourceId {
        type_id: 0,
        group: 0,
        instance: 0,
    };
    Ok(())
}
