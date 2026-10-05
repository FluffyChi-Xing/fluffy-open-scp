//! 冒烟工具：按 BBox 形状找树模型（高/宽 > 1.3 且高 3-20m）。
//! 用法：cargo run -p sc-exporter --release --example find_tree_models -- <pkg> [...]
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    for path in &args {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter().filter(|e| e.id.type_id == 0x2F4E_681B) {
            let Ok(data) = package.read(e) else { continue };
            let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
            let Some(bbox) = file.sections_of_type(rw4::SectionType::BBOX).next() else {
                continue;
            };
            // BBox section：min/max 各 3 float（偏移 8 常见；直接扫全段找 float 对）
            let payload = file.payload(&data, bbox.number).expect("bbox payload");
            if payload.len() < 24 {
                continue;
            }
            let rd = |off: usize| -> f32 {
                f32::from_le_bytes(payload[off..off + 4].try_into().unwrap())
            };
            // 布局探测：min=(0,4,8) max=(12,16,20) 或 min=(0..) 打印候选
            let (minx, miny, minz, maxx, maxy, maxz) = (rd(0), rd(4), rd(8), rd(12), rd(16), rd(20));
            if !minx.is_finite() || !maxz.is_finite() {
                continue;
            }
            let (w, d, h) = (maxx - minx, maxy - miny, maxz - minz);
            if !(1.5..25.0).contains(&h) || w <= 0.2 || d <= 0.2 || w > 20.0 {
                continue;
            }
            let aspect = h / w.max(d);
            if aspect < 1.3 {
                continue;
            }
            println!(
                "{path}: I {:08X} {}B bbox {:.1}x{:.1}x{:.1} 高宽比 {:.1}",
                e.id.instance,
                data.len(),
                w,
                d,
                h,
                aspect
            );
        }
    }
}
