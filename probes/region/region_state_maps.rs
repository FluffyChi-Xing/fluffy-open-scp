//! 探针：区域存档状态文件按 32776 B 步长切 128² u16 地图，渲染非零地图。仅开发用。
fn main() {
    let d = std::fs::read("tmp/egb_region.bin").unwrap();
    println!("文件 {} B", d.len());
    let stride = 32776usize; // 8 B 头 + 128×128×u16
    let mut start = 0x7cusize;
    let mut idx = 0usize;
    std::fs::create_dir_all("tmp/region_state").unwrap();
    while start + stride <= d.len() + 0 {
        let map = &d[start + 8..start + stride];
        let nonzero = map.chunks_exact(2).filter(|c| c[0] | c[1] != 0).count();
        if nonzero > 100 {
            let mut mn = u16::MAX;
            let mut mx = 0u16;
            let mut sum = 0u64;
            for c in map.chunks_exact(2) {
                let v = u16::from_le_bytes([c[0], c[1]]);
                mn = mn.min(v);
                mx = mx.max(v);
                sum += u64::from(v);
            }
            let img = image::GrayImage::from_fn(128, 128, |x, y| {
                let i = (y as usize) * 128 + x as usize;
                let v = u16::from_le_bytes([map[i * 2], map[i * 2 + 1]]);
                let s = (v / 256) as u8;
                image::Luma([s])
            });
            let path = format!("tmp/region_state/map_{idx:02}_off{start:06X}.png");
            image::DynamicImage::ImageLuma8(img)
                .resize(512, 512, image::imageops::FilterType::Nearest)
                .save(&path)
                .unwrap();
            println!(
                "map[{idx}] @{start:06X}: 非零 {nonzero}/16384 min={mn} max={mx} 均值={:.0} -> {path}",
                sum as f64 / 16384.0
            );
        }
        start += stride;
        idx += 1;
    }
    println!("共 {idx} 槽");
}
