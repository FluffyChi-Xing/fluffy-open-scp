//! 检查 R12：lot BBox / LotSize vs 模型 BBox / CollisionMesh 头的一致性。
use dbpf::{Package, ResourceId};
use rw4::{Rw4File, SectionType};
use sc_properties::{Kind, PropertyFile, Value};
fn f32s(b: &[u8]) -> Vec<f32> {
    b.chunks(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pkg = Package::open("tmp/nh_r12_minimal.package")?;
    // lot BA5224C6（R1 原件）
    let lot = pkg
        .entry(ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0xBA52_24C6,
        })
        .unwrap();
    let pf = PropertyFile::parse(&pkg.read(lot)?)?;
    for p in &pf.values {
        match p.hash {
            0x00F9_EFBA => println!("lot BBox: {:?}", p.kind),
            0x0CCB_7FC8 => println!("lot LotSize: {:?}", p.kind),
            0x00F9_EFBB..=0x00F9_EFBE => println!("lot LOD: {:?}", p.kind),
            _ => {}
        }
    }
    // 模型 8A096154（载荷 = 塔楼）
    let m = pkg
        .entry(ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: 0x8A09_6154,
        })
        .unwrap();
    let bytes = pkg.read(m)?;
    let f = Rw4File::parse(&bytes)?;
    for s in f.sections() {
        if matches!(s.type_code, SectionType::BBOX | SectionType::COLLISION_MESH) {
            let p = f.payload(&bytes, s.number)?;
            let v = f32s(&p[..32.min(p.len())]);
            println!(
                "model {:?} ({}B): min=({:.1},{:.1},{:.1}) max=({:.1},{:.1},{:.1})",
                s.type_name(),
                p.len(),
                v[0],
                v[1],
                v[2],
                v[4],
                v[5],
                v[6]
            );
        }
    }
    Ok(())
}
