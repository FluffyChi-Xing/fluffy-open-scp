//! 校验 Dalian 包内 LOD 模型与离线 dump 一致 + 模板引用图。
use dbpf::{Package, ResourceId};
use rw4::{Rw4File, SectionType};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let dumped = std::fs::read("tmp/rw4-compare/dalian_model.rw4")?;
    for inst in [0xB9B0_8494u32, 0xB9B0_8495, 0xB9B0_8496, 0xB9B0_8497] {
        let e = pkg
            .entry(ResourceId {
                type_id: 0x2F4E_681B,
                group: 0,
                instance: inst,
            })
            .unwrap();
        let data = pkg.read(e)?;
        println!(
            "{inst:08X}: {} bytes, matches_dump={}",
            data.len(),
            data == dumped
        );
    }
    let f = Rw4File::parse(&dumped)?;
    for s in f.sections() {
        if matches!(
            s.type_code,
            SectionType::TRIANGLE_ARRAY | SectionType::VERTEX_ARRAY | SectionType::TEXTURE
        ) {
            println!(
                "[{}] {:?}: {:02x?}",
                s.number,
                s.type_name(),
                f.payload(&dumped, s.number)?
                    .iter()
                    .take(28)
                    .collect::<Vec<_>>()
            );
        }
    }
    Ok(())
}
