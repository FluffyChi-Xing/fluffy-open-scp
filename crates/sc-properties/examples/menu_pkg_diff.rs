//! 对比 R1 实证菜单包与 R11 包的菜单资源（menu2/unit/lot/ERZ/locale）。
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
fn dump_pkg(path: &str, label: &str) -> Vec<(u32, u32, u32, usize)> {
    let pkg = Package::open(path).unwrap();
    println!("== {label} ==");
    let mut list = Vec::new();
    for e in pkg.entries() {
        let n = pkg.read(e).unwrap().len();
        println!(
            "{:08X} {:08X} {:08X} {}B",
            e.id.type_id, e.id.group, e.id.instance, n
        );
        list.push((e.id.type_id, e.id.group, e.id.instance, n));
    }
    list
}
fn dump_lot(path: &str, group: u32, instance: u32, label: &str) {
    let pkg = Package::open(path).unwrap();
    let e = match pkg.entry(ResourceId {
        type_id: 0x00B1_B104,
        group,
        instance,
    }) {
        Some(e) => e,
        None => {
            println!("{label}: lot not found");
            return;
        }
    };
    let pf = PropertyFile::parse(&pkg.read(e).unwrap()).unwrap();
    println!(
        "-- {label} lot {instance:08X}: {} props --",
        pf.values.len()
    );
    for p in &pf.values {
        let brief = match &p.kind {
            Kind::Scalar(Value::Key(k)) => format!(
                "Key(t={:08X} g={:08X} i={:08X})",
                k.type_id, k.group, k.instance
            ),
            Kind::Array(v) if v.len() > 3 => format!("[{}]", v.len()),
            other => format!("{other:?}"),
        };
        let line = format!("  {:08X} {}", p.hash, brief);
        if line.len() < 200 {
            println!("{line}");
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let r1 = "tmp/asset-build-fixtures/commercial-model/.build/fc209ccc34e1512ceb4b1b2ec22d4343f7dfd582b982c11eee65c4e455e4a7c9/packages/commercial-menu-park.package";
    let r11 = "tmp/nh_r11_menu_tower.package";
    dump_pkg(r1, "R1 proven");
    dump_pkg(r11, "R11");
    dump_lot(r1, 0x40E1_C000, 0, "R1"); // instance 未知，先扫全部 lot
    Ok(())
}
