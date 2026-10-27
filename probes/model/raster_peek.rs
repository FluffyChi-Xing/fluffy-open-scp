//! 单光栅尺寸。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let package = dbpf::Package::open(&path).expect("open");
    for a in args {
        let inst = u32::from_str_radix(a.trim_start_matches("0x"), 16).unwrap();
        if let Some(e) = package.entries().iter().find(|e| e.id.instance == inst) {
            if let Ok(data) = package.read(e) {
                if e.id.type_id == 0x2F4E_681C {
                    if let Ok(r) = rw4::RasterImage::parse(&data) {
                        println!("0x{inst:08X}: raster {}x{} fmt={} raw_rgba={}", r.width, r.height, r.pixel_format, r.is_raw_rgba());
                        continue;
                    }
                }
                if let Ok(f) = rw4::Rw4File::parse(&data) {
                    for s in f.sections_of_type(rw4::SectionType::TEXTURE) {
                        if let Ok(t) = f.decode_texture(&data, s.number) {
                            println!("0x{inst:08X}: rw4-texture #{} {}x{}", s.number, t.width, t.height);
                        }
                    }
                }
            }
        }
    }
}
