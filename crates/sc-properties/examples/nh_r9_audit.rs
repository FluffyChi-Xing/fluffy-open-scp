//! R9 包审计：与 NH 原包逐条目比对——仅 4 个 LOD 模型条目应不同。
use dbpf::{Package, ResourceId};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let nh = Package::open(&args[0])?;
    let r9 = Package::open(&args[1])?;
    let mut changed = 0usize;
    let mut identical = 0usize;
    for e in nh.entries() {
        let r9e = r9.entry(e.id).expect("entry missing in R9");
        let a = nh.read(e)?;
        let b = r9.read(r9e)?;
        let is_lod =
            e.id.type_id == 0x2F4E_681B && (0x06C5_470C..=0x06C5_470F).contains(&e.id.instance);
        let same = a == b;
        if is_lod {
            assert!(!same, "LOD model {:#x} must differ", e.id.instance);
            changed += 1;
            println!(
                "CHANGED  {:08X} {:08X} {:08X}  {} -> {} B",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                a.len(),
                b.len()
            );
        } else {
            assert!(
                same,
                "entry {:08X}/{:08X}/{:08X} must be byte-identical",
                e.id.type_id, e.id.group, e.id.instance
            );
            identical += 1;
        }
    }
    println!("audit: {changed} changed (expect 4), {identical} identical");
    // 塔楼模型在包内可解析且画域一致
    let m = r9
        .entry(ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: 0x06C5_470C,
        })
        .ok_or("model entry missing")?;
    let bytes = r9.read(m)?;
    let f = rw4::Rw4File::parse(&bytes)?;
    let mesh = f.decode_mesh(&bytes, 5)?;
    println!(
        "in-package model: {} bytes, {} verts, {} tris",
        bytes.len(),
        mesh.vertices.len(),
        mesh.triangles.len()
    );
    Ok(())
}
