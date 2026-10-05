//! 手动取证：车辆 RW4 的 section 构成（找内嵌纹理）。
//! cargo test -p fluffy-open-scp --release section_dump -- --ignored --nocapture
#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn dump_vehicle_rw4_sections() {
        let package =
            dbpf::Package::open(r"D:\ea-games\SimCity\SimCityData\SimCity_Graphics.package")
                .expect("open");
        // 材质槽位引用逐个验尸（0xbce4fc08=slot0 / 0xed73eeea=slot1）
        for inst in [0xBCE4_FC08u32, 0xED73_EEEA] {
            if let Some(entry) = package
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && e.id.type_id == 0x2F4E_681B)
            {
                let data = package.read(entry).expect("read");
                let file = rw4::Rw4File::parse(&data).expect("parse");
                println!("== slot ref {inst:08X}:");
                for section in file.sections() {
                    println!(
                        "  #{:<3} type={:<18} size={}",
                        section.number,
                        rw4::SectionType::name(section.type_code).unwrap_or("UNKNOWN"),
                        section.size
                    );
                }
                for section in file.sections_of_type(rw4::SectionType::TEXTURE) {
                    let t = file.decode_texture(&data, section.number).expect("tex");
                    let px = t.decode_top_mip_rgba().expect("decode");
                    let (w, h) = (t.width as usize, t.height as usize);
                    let img = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
                        let at = (x as usize + y as usize * w) * 4;
                        image::Rgba([px[at], px[at + 1], px[at + 2], px[at + 3]])
                    });
                    let path = format!("tmp/slotref_{inst:08X}_{}.png", section.number);
                    img.save(&path).expect("save");
                    println!("  texture {}x{} → {}", w, h, path);
                }
            }
        }
        let entry = package
            .entries()
            .iter()
            .find(|e| e.id.instance == 0xCA26_5D8B && e.id.type_id == 0x2F4E_681B)
            .expect("model");
        let data = package.read(entry).expect("read");
        let file = rw4::Rw4File::parse(&data).expect("parse");
        for section in file.sections() {
            println!(
                "#{:<3} type={:<18} size={}",
                section.number,
                rw4::SectionType::name(section.type_code).unwrap_or("UNKNOWN"),
                section.size
            );
        }
        // 内嵌纹理解码落盘
        for section in file.sections_of_type(rw4::SectionType::TEXTURE) {
            let texture = file.decode_texture(&data, section.number);
            match texture {
                Ok(t) => {
                    let px = t.decode_top_mip_rgba().expect("decode");
                    let (w, h) = (t.width as usize, t.height as usize);
                    let img = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
                        let at = (x as usize + y as usize * w) * 4;
                        image::Rgba([px[at], px[at + 1], px[at + 2], px[at + 3]])
                    });
                    let path = format!("tmp/vehicle_tex_{}.png", section.number);
                    img.save(&path).expect("save");
                    println!(
                        "texture #{}: {}x{} → {}",
                        section.number, t.width, t.height, path
                    );
                }
                Err(e) => println!("texture #{} decode err: {e}", section.number),
            }
        }
    }
}
