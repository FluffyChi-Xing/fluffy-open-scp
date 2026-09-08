//! 推导 mask 光栅尺寸 ↔ LotSize 换算：扫包内同时带两者的 lot。仅开发用。
use sc_properties::{Key, PropertyFile};

const LOT_SIZE: u32 = 0x0CCB_7FC8;
const LOT_MASK: u32 = 0x0CCB_7FD5;

fn main() {
    for path in std::env::args().skip(1) {
        let package = dbpf::Package::open(&path).expect("open package");
        // 光栅尺寸表：instance -> (w,h)（RasterImage + RW4 双类型）
        let mut raster_sizes = std::collections::HashMap::new();
        let entries: Vec<_> = package.entries().to_vec();
        for e in &entries {
            if e.id.type_id == 0x2F4E_681C {
                if let Ok(data) = package.read(e) {
                    if let Ok(r) = rw4::RasterImage::parse(&data) {
                        raster_sizes.insert(e.id.instance, (r.width, r.height));
                    }
                }
            }
        }
        println!("== {}（raster {} 张）", path.rsplit('/').next().unwrap(), raster_sizes.len());
        let mut shown = 0;
        for e in &entries {
            if e.id.type_id != 0x00B1_B104 { continue; }
            let Ok(data) = package.read(e) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            let size = file.get(LOT_SIZE).and_then(|p| match &p.kind {
                sc_properties::Kind::Scalar(sc_properties::Value::Vector2(v)) => Some((v[0], v[1])),
                _ => None,
            });
            let mask: Option<Key> = file.get(LOT_MASK).and_then(|p| match &p.kind {
                sc_properties::Kind::Scalar(sc_properties::Value::Key(k)) => Some(*k),
                sc_properties::Kind::Array(v) => v.first().and_then(|x| match x { sc_properties::Value::Key(k) => Some(*k), _ => None }),
                _ => None,
            });
            if let (Some((sx, sy)), Some(k)) = (size, mask) {
                let dim = raster_sizes.get(&k.instance);
                println!("  lot 0x{:08X}: size=({sx},{sy}) mask=0x{:08X} raster={dim:?}", e.id.instance, k.instance);
                shown += 1;
                if shown >= 10 { break; }
            }
        }
    }
}
