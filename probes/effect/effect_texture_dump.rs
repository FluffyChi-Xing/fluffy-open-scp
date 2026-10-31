//! Decode one RW4 texture (`0x2F4E681B`) from a package to PNG.
//!
//! cargo run -p probes --release --example effect_texture_dump -- \
//!   <package> <group_hex> <instance_hex> <out.png>

use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).expect("open package");
    let group = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("group");
    let instance = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).expect("instance");
    let entry = package
        .entries()
        .iter()
        .find(|entry| {
            entry.id.type_id == 0x2F4E_681B
                && entry.id.group == group
                && entry.id.instance == instance
        })
        .expect("texture entry");
    let bytes = package.read(entry).expect("read texture");
    let file = rw4::Rw4File::parse(&bytes).expect("parse rw4");
    let section = file
        .sections_of_type(rw4::SectionType::TEXTURE)
        .next()
        .expect("texture section");
    let decoded = file.decode_texture(&bytes, section.number).expect("decode");
    let rgba = decoded.decode_top_mip_rgba().expect("rgba");
    let image = image::RgbaImage::from_raw(decoded.width.into(), decoded.height.into(), rgba)
        .expect("image");
    image.save(&args[3]).expect("save png");
    println!(
        "{} {}x{} -> {}",
        args[2], decoded.width, decoded.height, args[3]
    );
}
