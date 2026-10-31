//! 比较 NH 四个 LOD 模型条目是否字节相同。
use dbpf::{Package, ResourceId};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let mut prev: Option<Vec<u8>> = None;
    for inst in [0x06C5_470Cu32, 0x06C5_470D, 0x06C5_470E, 0x06C5_470F] {
        let e = pkg
            .entry(ResourceId {
                type_id: 0x2F4E_681B,
                group: 0,
                instance: inst,
            })
            .unwrap();
        let data = pkg.read(e)?;
        let same = prev.as_ref().map(|p| *p == data);
        println!(
            "{inst:08X}: {} bytes, same_as_prev = {:?}",
            data.len(),
            same
        );
        prev = Some(data);
    }
    Ok(())
}
