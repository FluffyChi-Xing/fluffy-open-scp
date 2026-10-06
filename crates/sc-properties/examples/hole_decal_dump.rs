//! 破洞族（decalInteriorMap = 无 Color1-4 的 atlas 条目）贴图取证：
//! 导出每个破洞条目的 RGBA PNG + alpha 通道灰度图 + alpha 分布统计。
//!
//! 回答的问题（2026-10-05 破洞"黑色平斑"对拍）：
//! 1) 内景内容是否在这张贴图里、在哪个区域；
//! 2) alpha 如何编码（引擎公式：lerp 因子 = saturate(a×2−1)，a>0.5 才显内景）；
//! 3) 高 alpha 区的 RGB 亮度（决定 interiorLit 的基础色）。
//!
//! 用法：cargo run -p sc-properties --release --example hole_decal_dump -- <outdir> <package...>

use dbpf::Package;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: hole_decal_dump <outdir> <package...>");
        std::process::exit(2);
    }
    let outdir = &args[0];
    std::fs::create_dir_all(outdir).expect("create outdir");

    let mut packages = Vec::new();
    for path in &args[1..] {
        match Package::open(path) {
            Ok(p) => packages.push(p),
            Err(e) => eprintln!("跳过 {path}: {e}"),
        }
    }
    println!("已打开 {} 个包", packages.len());

    let mut dumped = 0usize;
    let mut dicts = 0usize;
    let mut entries_total = 0usize;
    let mut entries_hole = 0usize;
    for package in &packages {
        for entry_ref in package.entries().iter() {
            let id = entry_ref.id;
            if id.type_id != PROPERTY_TYPE || !sc_properties::is_decal_dictionary_group(id.group) {
                continue;
            }
            let Ok(data) = package.read(entry_ref) else { continue };
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
            dicts += 1;
            let mat = dict.material.as_ref().map(|k| k.instance).unwrap_or(0);
            println!(
                "字典 0x{:08X}: material=0x{:08X} 条目 {}",
                id.instance, mat, dict.entries.len()
            );
            // 破洞族判定（2026-10-05 §11 定谳）：字典材质 = 0x4491DE3A
            // （decalInteriorMap / shader 0x700000D2）。早前"无 Color1-4"
            // 口径在 1694 条全量扫描中 0 命中 = 该判据不成立。
            if mat != 0x4491_DE3A {
                continue;
            }
            for entry in &dict.entries {
                entries_total += 1;
                entries_hole += 1;
                let Some(raster_key) = &entry.raster else { continue };
                // 跨包按 instance 找 RW4 纹理资源
                let mut found: Option<Vec<u8>> = None;
                for p in &packages {
                    for e in p.entries() {
                        if e.id.type_id == RW4_MODEL_TYPE && e.id.instance == raster_key.instance {
                            if let Ok(bytes) = p.read(e) {
                                found = Some(bytes);
                            }
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
                let Some(bytes) = found else {
                    println!(
                        "条目[{}] raster 0x{:08X}: 资源未找到",
                        entry.index, raster_key.instance
                    );
                    continue;
                };
                let Ok(rw4file) = rw4::Rw4File::parse(&bytes) else {
                    println!("条目[{}] raster 0x{:08X}: RW4 解析失败", entry.index, raster_key.instance);
                    continue;
                };
                let Some(section) = rw4file
                    .sections_of_type(rw4::SectionType::TEXTURE)
                    .next()
                    .map(|s| s.number)
                else {
                    println!("条目[{}] raster 0x{:08X}: 无 TEXTURE 节", entry.index, raster_key.instance);
                    continue;
                };
                let Ok(texture) = rw4file.decode_texture(&bytes, section) else {
                    println!("条目[{}] raster 0x{:08X}: 纹理解码失败", entry.index, raster_key.instance);
                    continue;
                };
                let Ok(rgba) = texture.decode_top_mip_rgba() else {
                    println!("条目[{}] raster 0x{:08X}: 像素解码失败", entry.index, raster_key.instance);
                    continue;
                };
                let w = u32::from(texture.width);
                let h = u32::from(texture.height);

                // alpha 分布统计
                let total = (w * h) as usize;
                let mut buckets = [0usize; 4]; // [0,0.25) [0.25,0.5) [0.5,0.75) [0.75,1]
                let mut hi_rgb_sum = [0u64; 3];
                let mut hi_count = 0usize;
                let mut hi_cx = 0u64;
                let mut hi_cy = 0u64;
                for y in 0..h {
                    for x in 0..w {
                        let i = ((y * w + x) * 4) as usize;
                        let a = rgba[i + 3];
                        let b = match a {
                            0..=63 => 0,
                            64..=127 => 1,
                            128..=191 => 2,
                            _ => 3,
                        };
                        buckets[b] += 1;
                        if a > 127 {
                            hi_count += 1;
                            hi_rgb_sum[0] += u64::from(rgba[i]);
                            hi_rgb_sum[1] += u64::from(rgba[i + 1]);
                            hi_rgb_sum[2] += u64::from(rgba[i + 2]);
                            hi_cx += u64::from(x);
                            hi_cy += u64::from(y);
                        }
                    }
                }
                let (mean_rgb, centroid) = if hi_count > 0 {
                    (
                        format!(
                            "({},{},{})",
                            hi_rgb_sum[0] / hi_count as u64,
                            hi_rgb_sum[1] / hi_count as u64,
                            hi_rgb_sum[2] / hi_count as u64
                        ),
                        format!(
                            "({:.2},{:.2})",
                            hi_cx as f64 / hi_count as f64 / f64::from(w),
                            hi_cy as f64 / hi_count as f64 / f64::from(h)
                        ),
                    )
                } else {
                    ("-".into(), "-".into())
                };
                println!(
                    "条目[{}] raster 0x{:08X} {}x{}: a[0,.25)={:.0}% [.25,.5)={:.0}% [.5,.75)={:.0}% [.75,1]={:.0}% | a>0.5 区 RGB均值 {} 质心 {}",
                    entry.index,
                    raster_key.instance,
                    w,
                    h,
                    buckets[0] as f64 / total as f64 * 100.0,
                    buckets[1] as f64 / total as f64 * 100.0,
                    buckets[2] as f64 / total as f64 * 100.0,
                    buckets[3] as f64 / total as f64 * 100.0,
                    mean_rgb,
                    centroid,
                );

                // 导出 RGBA PNG + alpha 灰度图
                let base = format!("{outdir}/hole_{:08X}_idx{}", raster_key.instance, entry.index);
                let rgba_img: image::RgbaImage =
                    image::ImageBuffer::from_raw(w, h, rgba.clone()).expect("rgba buffer");
                if let Err(e) = rgba_img.save(format!("{base}.png")) {
                    eprintln!("  PNG 保存失败: {e}");
                }
                let alpha_gray: image::GrayImage = image::ImageBuffer::from_raw(
                    w,
                    h,
                    rgba.chunks_exact(4).map(|p| p[3]).collect::<Vec<u8>>(),
                )
                .expect("alpha buffer");
                let _ = alpha_gray.save(format!("{base}_alpha.png"));
                dumped += 1;
            }
        }
    }
    println!("字典 {dicts} 个 / 条目 {entries_total} / 破洞族 {entries_hole} / 导出 {dumped} → {outdir}");
}
