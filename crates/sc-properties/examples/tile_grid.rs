//! 按"包索引序 × 指定列宽"把区域高度图 tile 渲染成大图（BMP 24bit）。
//! 用途：确定 RegionTerrain tile 的真实网格排布（尝试不同列宽看哪个连贯）。
//! 用法：cargo run -p sc-properties --release --example tile_grid -- \
//!   <package> <group-hex> <type-hex> <width> <out.bmp>
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (package_path, group, type_hex, width, out) = (
        &args[0],
        u32::from_str_radix(args[1].trim_start_matches("0x"), 16).unwrap(),
        u32::from_str_radix(args[2].trim_start_matches("0x"), 16).unwrap(),
        args[3].parse::<usize>().unwrap(),
        &args[4],
    );
    let package = Package::open(package_path).unwrap();
    let mut tiles: Vec<(u32, Vec<u16>)> = Vec::new();
    for entry in package.entries().iter().filter(|e| e.id.group == group && e.id.type_id == type_hex) {
        let data = package.read(entry).unwrap();
        let hdr = 20usize;
        let cells: Vec<u16> = data[hdr..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        tiles.push((entry.id.instance, cells));
    }
    println!("tiles={}", tiles.len());
    let n = (tiles[0].1.len() as f64).sqrt().round() as usize;
    let height = (tiles.len() + width - 1) / width;
    let w = width * n;
    let h = height * n;
    let mut canvas = vec![0u16; w * h];
    for (ti, (_, cells)) in tiles.iter().enumerate() {
        let tx = ti % width;
        let ty = ti / width;
        for y in 0..n {
            for x in 0..n {
                canvas[(ty * n + y) * w + tx * n + x] = cells[y * n + x];
            }
        }
    }
    let mut minv = u16::MAX;
    let mut maxv = 0u16;
    for v in &canvas {
        minv = minv.min(*v);
        maxv = maxv.max(*v);
    }
    let span = (maxv - minv).max(1) as f32;
    let row_pad = (4 - (w * 3) % 4) % 4;
    let data_size = (h * (w * 3 + row_pad)) as u32;
    let mut bmp = Vec::new();
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(54 + data_size).to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(w as i32).to_le_bytes());
    bmp.extend_from_slice(&(h as i32).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&24u16.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 24]);
    for y in (0..h).rev() {
        for x in 0..w {
            let v = (((canvas[y * w + x] - minv) as f32 / span) * 255.0) as u8;
            bmp.extend_from_slice(&[v, v, v]);
        }
        for _ in 0..row_pad { bmp.push(0); }
    }
    std::fs::write(out, &bmp).unwrap();
    println!("grid {width}x{height} written -> {out}");
}
