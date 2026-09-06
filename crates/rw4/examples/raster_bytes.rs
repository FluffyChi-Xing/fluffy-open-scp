//! 字节序取证：打印 raster 顶层 mip 每个字节位置(0..3)的统计与前 8 像素。仅开发用。
//! 用法：cargo run -p rw4 --example raster_bytes -- <package> [instance_hex]

use rw4::RasterImage;

fn main() {
    let path = std::env::args().nth(1).expect("usage: raster_bytes <package> [instance]");
    let want = std::env::args().nth(2).and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
    let package = dbpf::Package::open(&path).expect("open package");
    let mut shown = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x2F4E_681C { continue; }
        if let Some(want) = want { if entry.id.instance != want { continue; } }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(r) = RasterImage::parse(&data) else { continue };
        if !r.is_raw_rgba() { continue; }
        let Some(mip) = r.mips.first() else { continue };
        let px = mip.len() / 4;
        if px == 0 { continue; }
        let mut stats = [(0u64, 0u64, u8::MAX, 0u8); 4]; // (sum, count, min, max)
        for p in mip.as_chunks::<4>().0 {
            for (i, &b) in p.iter().enumerate() {
                stats[i].0 += b as u64;
                stats[i].1 += 1;
                stats[i].2 = stats[i].2.min(b);
                stats[i].3 = stats[i].3.max(b);
            }
        }
        println!("0x{:08X} {}x{} pixFmt={}", entry.id.instance, r.width, r.height, r.pixel_format);
        for (i, (sum, count, min, max)) in stats.iter().enumerate() {
            println!("  byte{i}: avg={:.1} min={min} max={max}", *sum as f64 / *count as f64);
        }
        println!("  first8: {:?}", &mip[..mip.len().min(32)]);
        shown += 1;
        if shown >= 6 { break; }
    }
    if shown == 0 { println!("no raw rasters found"); }
}
