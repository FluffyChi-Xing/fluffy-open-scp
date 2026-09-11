//! 冒烟工具：解码 "Lot Textures" 图集并切出 4×4=16 格 PNG。
//! 用法：cargo run -p sc-exporter --release --example atlas_dump -- <package> <instance_hex> <out_dir>
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).unwrap();
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).unwrap();
    let out_dir = &args[2];
    std::fs::create_dir_all(out_dir).unwrap();
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.instance == instance)
        .cloned()
        .expect("not found");
    println!("type={:08x}", entry.id.type_id);
    let data = package.read(&entry).unwrap();
    let file = rw4::Rw4File::parse(&data).expect("parse");
    let section = file
        .sections_of_type(rw4::SectionType::TEXTURE)
        .next()
        .map(|s| s.number)
        .expect("no texture");
    let texture = file.decode_texture(&data, section).expect("decode");
    let rgba = texture.decode_top_mip_rgba().expect("pixels");
    let (w, h) = (texture.width as usize, texture.height as usize);
    println!("atlas {w}x{h}");
    let (tw, th) = (w / 4, h / 4);
    for index in 0..16 {
        let ox = (index % 4) * tw;
        let oy = (index / 4) * th;
        let mut pixels = vec![0u8; tw * th * 4];
        for y in 0..th {
            for x in 0..tw {
                let src = ((oy + y) * w + ox + x) * 4;
                let dst = (y * tw + x) * 4;
                pixels[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
            }
        }
        let image = image::RgbaImage::from_raw(tw as u32, th as u32, pixels).unwrap();
        image
            .save(format!("{out_dir}/tile_{index:02}.png"))
            .unwrap();
    }
    println!("16 tiles saved to {out_dir}");
}
