//! dump Dalian 包 lot 的尺寸/BBox/LOD 引用（离线侦察）。
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let mut lots = 0usize;
    for e in pkg.entries() {
        if e.id.type_id != 0x00B1_B104 {
            continue;
        }
        lots += 1;
        println!(
            "== lot {:08X} {:08X} {:08X}",
            e.id.type_id, e.id.group, e.id.instance
        );
        let pf = PropertyFile::parse(&pkg.read(e)?)?;
        for p in &pf.values {
            let brief = match &p.kind {
                Kind::Scalar(sc_properties::Value::Key(k)) => format!(
                    "Key(t={:08X} g={:08X} i={:08X})",
                    k.type_id, k.group, k.instance
                ),
                Kind::Array(v) if v.len() > 4 => format!("[{} items]", v.len()),
                other => format!("{other:?}"),
            };
            let line = format!("  {:08X}: {}", p.hash, brief);
            if line.len() < 180 {
                println!("{line}");
            } else {
                println!("  {:08X}: <{} chars>", p.hash, line.len());
            }
        }
        if lots >= 2 {
            break;
        }
    }
    println!("total lot entries scanned: {lots}");
    Ok(())
}
