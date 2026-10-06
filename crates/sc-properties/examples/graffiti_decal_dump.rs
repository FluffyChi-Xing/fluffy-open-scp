//! 涂鸦族（材质 0xE5390A98）贴图取证：权重掩码 vs 真彩图 定谳。
//!
//! 背景矛盾（2026-10-05 涂鸦融合对拍）：
//! - §六（decal_raster_stats，2026-10-01）：涂鸦 raster = 真彩 RGB(A)，
//!   引擎 tex2D 直采即所得，不存在四色展开；
//! - §十二（2026-10-04 用户对拍）：直采 = 彩色模糊涂抹，0.5 阈值多通道
//!   合成才是"清晰图案"来源。
//! 两者必有一个看错。判别量：
//! 1) 通道值直方图——量化掩码 = 少数离散级别（如 0/74/153/255 聚类）；
//!    真彩 = 连续分布；
//! 2) 单通道独占率——掩码 = 像素恰一个通道 ≥128；真彩 = 通道连续共变；
//! 3) alpha 语义——掩码 A = 覆盖 0/1；真彩 A = 柔和边缘（连续梯度）。
//!
//! 用法：cargo run -p sc-properties --release --example graffiti_decal_dump -- <outdir> <max_entries> <package...>

use dbpf::Package;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;
const RASTER_TYPE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: graffiti_decal_dump <outdir> <max_entries> <package...>");
        std::process::exit(2);
    }
    let outdir = &args[0];
    let max_entries: usize = args[1].parse().expect("max_entries");
    std::fs::create_dir_all(outdir).expect("create outdir");

    let mut packages = Vec::new();
    for path in &args[2..] {
        match Package::open(path) {
            Ok(p) => packages.push(p),
            Err(e) => eprintln!("跳过 {path}: {e}"),
        }
    }
    println!("已打开 {} 个包", packages.len());

    let mut dumped = 0usize;
    for package in &packages {
        for entry_ref in package.entries().iter() {
            let id = entry_ref.id;
            if id.type_id != PROPERTY_TYPE || !sc_properties::is_decal_dictionary_group(id.group)
            {
                continue;
            }
            let Ok(data) = package.read(entry_ref) else {
                continue;
            };
            let Ok(file) = sc_properties::PropertyFile::parse_with_limits(
                &data,
                sc_properties::ParseLimits::default(),
            ) else {
                continue;
            };
            if !sc_properties::decal::looks_like_dictionary(&file) {
                continue;
            }
            let dict = sc_properties::DecalDictionary::from_file(&file);
            let mat = dict.material.as_ref().map(|k| k.instance).unwrap_or(0);
            if mat != 0xE539_0A98 {
                continue;
            }
            println!(
                "涂鸦字典 g{:06x}/0x{:08X}: 条目 {}",
                id.group,
                id.instance,
                dict.entries.len()
            );
            for entry in dict.entries.iter().take(max_entries) {
                let Some(raster_key) = &entry.raster else { continue };
                let mut found: Option<Vec<u8>> = None;
                'pkgs: for p in &packages {
                    for e in p.entries() {
                        if (e.id.type_id == RW4_MODEL_TYPE || e.id.type_id == RASTER_TYPE)
                            && e.id.instance == raster_key.instance
                        {
                            if let Ok(bytes) = p.read(e) {
                                found = Some(bytes);
                            }
                            break 'pkgs;
                        }
                    }
                }
                let Some(bytes) = found else {
                    println!("  条目[{}] raster 0x{:08X}: 资源未找到", entry.index, raster_key.instance);
                    continue;
                };
                // 两种容器都试：RW4 文件（TEXTURE 节）或裸 RasterImage；
                // 统一取出 (width, height, rgba)
                let decoded_px: Option<(u32, u32, Vec<u8>)> = (|| {
                    if let Ok(rw4file) = rw4::Rw4File::parse(&bytes) {
                        if let Some(section) = rw4file
                            .sections_of_type(rw4::SectionType::TEXTURE)
                            .next()
                            .map(|s| s.number)
                        {
                            if let Ok(t) = rw4file.decode_texture(&bytes, section) {
                                if let Ok(rgba) = t.decode_top_mip_rgba() {
                                    return Some((u32::from(t.width), u32::from(t.height), rgba));
                                }
                            }
                        }
                    }
                    let r = rw4::RasterImage::parse(&bytes).ok()?;
                    let rgba = r.decode_top_mip_rgba().ok()?;
                    Some((u32::from(r.width), u32::from(r.height), rgba))
                })();
                let Some((w, h, rgba)) = decoded_px else {
                    println!("  条目[{}] raster 0x{:08X}: 纹理解码失败", entry.index, raster_key.instance);
                    continue;
                };
                let total = (w * h) as usize;

                // 判别量 1：每通道 16 桶直方图（量化掩码 = 尖锐峰；真彩 = 连续）
                let mut hist = [[0usize; 16]; 4];
                // 判别量 2：独占通道计数（≥128 的通道数 0/1/2/3/4）
                let mut on_count = [0usize; 5];
                // 判别量 3：alpha 中间值占比（0<A<255 = 柔和边缘证据）
                let mut a_mid = 0usize;
                let mut a_hi = 0usize;
                for px in rgba.chunks_exact(4) {
                    for ch in 0..4 {
                        hist[ch][(px[ch] as usize) / 16] += 1;
                    }
                    let on = px.iter().filter(|&&v| v >= 128).count();
                    on_count[on] += 1;
                    if px[3] > 0 && px[3] < 255 {
                        a_mid += 1;
                    }
                    if px[3] >= 128 {
                        a_hi += 1;
                    }
                }
                let pct = |n: usize| n as f64 / total as f64 * 100.0;
                println!(
                    "  条目[{}] raster 0x{:08X} {}x{}: A≥128 {:.0}% A中间值 {:.0}% | ≥128通道数 0:{:.0}% 1:{:.0}% 2:{:.0}% 3:{:.0}% 4:{:.0}%",
                    entry.index, raster_key.instance, w, h,
                    pct(a_hi), pct(a_mid),
                    pct(on_count[0]), pct(on_count[1]), pct(on_count[2]), pct(on_count[3]), pct(on_count[4]),
                );
                for (ch, name) in ['R', 'G', 'B', 'A'].iter().enumerate() {
                    let dist: Vec<String> = hist[ch]
                        .iter()
                        .enumerate()
                        .filter(|(_, n)| *n > &0)
                        .map(|(b, &n)| format!("{}-{:.0}%", b * 16, pct(n)))
                        .collect();
                    println!("    {name}: {}", dist.join(" "));
                }
                // colors 行（供对照）
                if !entry.colors.is_empty() {
                    let cs: Vec<String> = entry
                        .colors
                        .iter()
                        .map(|c| match c {
                            Some(v) => format!("({},{},{},{})", v[0], v[1], v[2], v[3]),
                            None => "-".to_string(),
                        })
                        .collect();
                    println!("    colors: {}", cs.join(" "));
                }

                let base = format!("{outdir}/graffiti_{:08X}_idx{}", raster_key.instance, entry.index);
                let img: image::RgbaImage =
                    image::ImageBuffer::from_raw(w, h, rgba.clone()).expect("rgba buffer");
                let _ = img.save(format!("{base}.png"));
                for ch in 0..4 {
                    let gray: image::GrayImage = image::ImageBuffer::from_raw(
                        w,
                        h,
                        rgba.chunks_exact(4).map(|p| p[ch]).collect::<Vec<u8>>(),
                    )
                    .expect("gray buffer");
                    let _ = gray.save(format!("{base}_ch{ch}.png"));
                }
                dumped += 1;
            }
        }
    }
    println!("导出 {dumped} 张涂鸦贴图 → {outdir}");
}
