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
        // 材质内嵌数据对比（黑车 CA265D8B vs 垃圾桶 903A704C）：找漫反射色字段
        for inst in [0xCA26_5D8Bu32, 0x903A_704C, 0xAD64_CB3D] {
            let Some(entry) = package
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && e.id.type_id == 0x2F4E_681B)
            else {
                println!("== model {inst:08X}: 不在 Graphics 包（跳过）");
                continue;
            };
            let data = package.read(entry).expect("read");
            let file = rw4::Rw4File::parse(&data).expect("parse");
            println!("== model {inst:08X}:");
            for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
                let Ok(rw4::MaterialSection::Decoded(m)) =
                    file.decode_material(&data, section.number)
                else {
                    // Raw 材质 = 内嵌 RW4 子文件（RW4w32 签名，textures
                    // 打包进材质段——prop/杂件模型的贴图分发方式）
                    println!("  material #{}: Raw——尝试按内嵌 RW4 解析（{} 字节）", section.number, data.len());
                    if let Ok(embedded) = rw4::Rw4File::parse(&data) {
                        for sec in embedded.sections() {
                            println!(
                                "    内嵌 #{:<3} type={:<18} size={}",
                                sec.number,
                                rw4::SectionType::name(sec.type_code).unwrap_or("UNKNOWN"),
                                sec.size
                            );
                        }
                        for sec in embedded.sections_of_type(rw4::SectionType::TEXTURE) {
                            let t = embedded
                                .decode_texture(&data, sec.number)
                                .expect("decode embedded texture");
                            let px = t.decode_top_mip_rgba().expect("decode mip");
                            let (w, h) = (t.width as usize, t.height as usize);
                            let img = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
                                let at = (x as usize + y as usize * w) * 4;
                                image::Rgba([px[at], px[at + 1], px[at + 2], px[at + 3]])
                            });
                            let path = format!("tmp/embedded_{inst:08X}_{}.png", sec.number);
                            img.save(&path).expect("save");
                            println!("    内嵌纹理 {}x{} → {}", w, h, path);
                        }
                    }
                    continue;
                };
                println!(
                    "  material #{}: header={} vf={} addl={} refs={} tail={}",
                    section.number,
                    m.header.len(),
                    m.vertex_format_data.len(),
                    m.additional_data.len(),
                    m.texture_refs.len(),
                    m.data.len()
                );
                println!("    header: {:02x?}", m.header);
                println!(
                    "    addl:   {:02x?}",
                    m.additional_data.iter().take(48).collect::<Vec<_>>()
                );
                println!(
                    "    tail[:64]: {:02x?}",
                    m.data.iter().take(64).collect::<Vec<_>>()
                );
            }
        }
        // 文件级 TEXTURE 段解码（杂件模型贴图位置实证）
        let game_package = dbpf::Package::open(
            r"D:\ea-games\SimCity\SimCityData\SimCity_Game.package",
        )
        .expect("open game pkg");
        for inst in [0x903A_704Cu32, 0xAD64_CB3D] {
            let Some(entry) = game_package
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && e.id.type_id == 0x2F4E_681B)
            else {
                println!("== model {inst:08X}: 不在 Game 包（跳过）");
                continue;
            };
            let data = game_package.read(entry).expect("read");
            let file = rw4::Rw4File::parse(&data).expect("parse");
            println!("== model {inst:08X} 文件级 TEXTURE 段:");
            for section in file.sections_of_type(rw4::SectionType::TEXTURE) {
                let t = file.decode_texture(&data, section.number).expect("tex");
                let px = t.decode_top_mip_rgba().expect("decode");
                let (w, h) = (t.width as usize, t.height as usize);
                let img = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
                    let at = (x as usize + y as usize * w) * 4;
                    image::Rgba([px[at], px[at + 1], px[at + 2], px[at + 3]])
                });
                let path = format!("tmp/filetex_{inst:08X}_{}.png", section.number);
                img.save(&path).expect("save");
                println!("  #{} {}x{} → {}", section.number, w, h, path);
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
