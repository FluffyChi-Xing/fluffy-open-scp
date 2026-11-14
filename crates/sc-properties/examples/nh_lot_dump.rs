//! dump NH lot 的全部 property（离线诊断 LOD 模型引用）。
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let lot = pkg
        .entry(ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0xA420_E06D,
        })
        .expect("lot");
    let pf = PropertyFile::parse(&pkg.read(lot)?)?;
    for p in &pf.values {
        println!("hash {:08X}: {:?}", p.hash, p.kind);
    }
    Ok(())
}
