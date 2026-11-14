//! 逐字段 diff R1 实证 vs R11 的 menu2/unit/lot property。
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
fn load(path: &str) -> Vec<(u32, u32, u32, PropertyFile)> {
    let pkg = Package::open(path).unwrap();
    let mut out = Vec::new();
    for e in pkg.entries() {
        if !matches!(e.id.type_id, 0x00B1_B104 | 0x2F7D_0004) {
            continue;
        }
        if let Ok(pf) = PropertyFile::parse(&pkg.read(e).unwrap()) {
            out.push((e.id.type_id, e.id.group, e.id.instance, pf));
        }
    }
    out
}
fn fmt_kind(k: &Kind) -> String {
    match k {
        Kind::Scalar(Value::Key(k2)) => format!(
            "Key(t={:08X},g={:08X},i={:08X})",
            k2.type_id, k2.group, k2.instance
        ),
        other => format!("{other:?}").chars().take(90).collect(),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let r1 = load(
        "tmp/asset-build-fixtures/commercial-model/.build/fc209ccc34e1512ceb4b1b2ec22d4343f7dfd582b982c11eee65c4e455e4a7c9/packages/commercial-menu-park.package",
    );
    let r11 = load("tmp/nh_r11_menu_tower.package");
    for (t, g, i, pf) in &r11 {
        let twin = r1.iter().find(|(t1, g1, _, _)| t1 == t && g1 == g);
        println!(
            "== R11 {:08X}/{:08X}/{:08X} ({} props) vs R1 {}",
            t,
            g,
            i,
            pf.values.len(),
            twin.map(|x| format!("{} props", x.3.values.len()))
                .unwrap_or("ABSENT".into())
        );
        for p in &pf.values {
            let r1v = twin.and_then(|x| x.3.values.iter().find(|q| q.hash == p.hash));
            match r1v {
                Some(q) if fmt_kind(&q.kind) == fmt_kind(&p.kind) => {}
                Some(q) => println!(
                    "  DIFF {:08X}:\n    R11 {}\n    R1  {}",
                    p.hash,
                    fmt_kind(&p.kind),
                    fmt_kind(&q.kind)
                ),
                None => println!("  ONLY-R11 {:08X} {}", p.hash, fmt_kind(&p.kind)),
            }
        }
        if let Some((_, _, _, pf1)) = twin {
            for p in &pf1.values {
                if !pf.values.iter().any(|q| q.hash == p.hash) {
                    println!("  ONLY-R1 {:08X} {}", p.hash, fmt_kind(&p.kind));
                }
            }
        }
    }
    for (t, g, i, pf) in &r1 {
        if !r11.iter().any(|(t1, g1, _, _)| t1 == t && g1 == g) {
            println!(
                "== R1-only resource {:08X}/{:08X}/{:08X} ({} props)",
                t,
                g,
                i,
                pf.values.len()
            );
        }
    }
    Ok(())
}
