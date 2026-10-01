//! 只读统计：decal raster 的通道形态取证（2026-10-01 边缘/暗部/色彩三问）。
//!
//! 判定 pixFmt21 掩码到底是「4 层独立掩码」（引擎必须做 mask→color 展开）
//! 还是「亮度+覆盖度」（直采即可成像）。输出每通道均值/σ、两两相关系数、
//! 唯一值数（量化检查）。
//!
//! 用法：cargo run -p sc-exporter --release --example decal_raster_stats -- \
//!     <dict_group_hex> <dict_instance_hex> <pkg> [max_entries] [raster_pkg...]

use dbpf::Package;
use sc_properties::decal::DecalDictionary;
use sc_properties::PROPERTY_RESOURCE_TYPE;

const RASTER_TYPE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    eprintln!("args = {args:?}");
    let group = u16::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("group hex");
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("inst hex");
    let packages: Vec<Package> = args
        .iter()
        .skip(2)
        .filter(|p| p.parse::<usize>().is_err()) // 跳过 max 这类纯数字参数
        .map(|p| Package::open(p).expect("open"))
        .collect();
    let max: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(8).max(1);

    // 找字典
    let mut dict = None;
    'outer: for pkg in &packages {
        for e in pkg.entries() {
            if e.id.type_id != PROPERTY_RESOURCE_TYPE {
                continue;
            }
            if (e.id.group as u16) != group || e.id.instance != instance {
                continue;
            }
            eprintln!(
                "命中字典 group=0x{:08x} instance=0x{:08x}",
                e.id.group, e.id.instance
            );
            let data = pkg.read(e).expect("read dict");
            dict = Some(DecalDictionary::parse(&data).expect("parse dict"));
            break 'outer;
        }
    }
    let dict = dict.expect("dict not found");
    println!(
        "字典 {group:08x}/{instance:08x}: material={:?} entries={}",
        dict.material.as_ref().map(|k| k.instance),
        dict.entries.len()
    );

    let mut done = 0usize;
    for entry in &dict.entries {
        if done >= max {
            break;
        }
        let Some(raster_key) = entry.raster.clone() else { continue };
        // 先试裸 raster（0x2F4E681C），失败再试 RW4 纹理（0x2F4E681B，破洞族）
        let rgba_width_height: Option<(Vec<u8>, u32, u32, String)> = find_raster(&packages, raster_key.instance)
            .and_then(|(bytes, _)| {
                let raster = rw4::RasterImage::parse(&bytes).ok()?;
                let rgba = raster.decode_top_mip_rgba().ok()?;
                Some((rgba, raster.width, raster.height, format!("raw pixFmt{}", raster.pixel_format)))
            })
            .or_else(|| find_rw4_texture(&packages, raster_key.instance));
        let Some((rgba, w, h, source)) = rgba_width_height else {
            eprintln!("条目[{}] raster {instance2:08x} 未找到", entry.index, instance2 = raster_key.instance);
            continue;
        };
        let n = (w * h) as usize;
        let id = entry.id.map(|k| format!("{:08x}", k.instance)).unwrap_or_else(|| "?".into());
        println!(
            "\n条目[{}] id={} {}x{} {}",
            entry.index, id, w, h, source
        );
        // 每通道 mean/σ/唯一值
        let mut mean = [0f64; 4];
        let mut uniq = [std::collections::BTreeSet::new(), Default::default(), Default::default(), Default::default()];
        for px in rgba.chunks_exact(4) {
            for c in 0..4 {
                mean[c] += px[c] as f64;
                uniq[c].insert(px[c]);
            }
        }
        for c in 0..4 {
            mean[c] /= n as f64;
        }
        let mut var = [0f64; 4];
        for px in rgba.chunks_exact(4) {
            for c in 0..4 {
                let d = px[c] as f64 - mean[c];
                var[c] += d * d;
            }
        }
        let std: Vec<f64> = var.iter().map(|v| (v / n as f64).sqrt()).collect();
        for (c, name) in ["R", "G", "B", "A"].iter().enumerate() {
            println!(
                "  {name}: mean={:.1} σ={:.1} 唯一值={}",
                mean[c], std[c], uniq[c].len()
            );
        }
        // 两两相关（0 通道 R 与 3 通道 A 等）
        let corr = |a: usize, b: usize| -> f64 {
            let mut sab = 0f64;
            for px in rgba.chunks_exact(4) {
                sab += (px[a] as f64 - mean[a]) * (px[b] as f64 - mean[b]);
            }
            sab / (n as f64 * std[a] * std[b]).max(1e-9)
        };
        println!(
            "  corr R-G={:.3} R-B={:.3} G-B={:.3} | R-A={:.3} G-A={:.3} B-A={:.3}",
            corr(0, 1), corr(0, 2), corr(1, 2), corr(0, 3), corr(1, 3), corr(2, 3)
        );
        // 权重和分布（掩码形态判据：Σc∈[0,255] 窄带=层掩码/覆盖度）
        let mut sum_hist = [0usize; 5]; // 0-16,16-64,64-128,128-240,240+
        for px in rgba.chunks_exact(4) {
            let s = px[0] as u32 + px[1] as u32 + px[2] as u32 + px[3] as u32;
            let bucket = match s { 0..=16 => 0, 17..=64 => 1, 65..=128 => 2, 129..=240 => 3, _ => 4 };
            sum_hist[bucket] += 1;
        }
        println!(
            "  Σ通道直方图: ≤16:{} 17-64:{} 65-128:{} 129-240:{} >240:{}",
            sum_hist[0], sum_hist[1], sum_hist[2], sum_hist[3], sum_hist[4]
        );
        done += 1;
    }
}

fn find_raster(packages: &[Package], instance: u32) -> Option<(Vec<u8>, u32)> {
    for pkg in packages {
        for e in pkg.entries() {
            if e.id.type_id == RASTER_TYPE && e.id.instance == instance {
                return pkg.read(e).ok().map(|b| (b, e.id.type_id));
            }
        }
    }
    None
}

const RW4_TEXTURE_TYPE: u32 = 0x2F4E_681B;

fn find_rw4_texture(packages: &[Package], instance: u32) -> Option<(Vec<u8>, u32, u32, String)> {
    for pkg in packages {
        for e in pkg.entries() {
            if e.id.type_id != RW4_TEXTURE_TYPE || e.id.instance != instance {
                continue;
            }
            let bytes = pkg.read(e).ok()?;
            let file = rw4::Rw4File::parse(&bytes).ok()?;
            let number = file.sections_of_type(rw4::SectionType::TEXTURE).next()?.number;
            let texture = file.decode_texture(&bytes, number).ok()?;
            let kind = match texture.format() {
                rw4::TextureFormat::Dxt1 => "DXT1",
                rw4::TextureFormat::Dxt5 => "DXT5",
                rw4::TextureFormat::Raw => "rawBGRA",
                rw4::TextureFormat::Unknown(_) => "unknown",
            };
            let rgba = texture.decode_top_mip_rgba().ok()?;
            return Some((rgba, u32::from(texture.width), u32::from(texture.height), format!("rw4 {kind}")));
        }
    }
    None
}
