//! 统计检验：建筑中心是否显著落在 mask 空腔（未覆盖区）内（偏心假设）。
//!
//! 用法：cargo run -p sc-exporter --release --example lot_cavity_stats -- <package> [more...]
//!
//! 对每个带 LotSize + LotMask 的 lot property：
//!   d0 = |空腔质心 - lot 中心|        （"建筑居中"假设的误差）
//!   d1 = |空腔质心 - 建筑 bbox 中心|  （"建筑锚定空腔"假设的误差）
//! 汇总配对差 d0-d1 的均值/中位数/t 统计量；并输出覆盖占比分布。

use dbpf::Package;
use sc_properties::{LotEditorDocument, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const BBOX_HASH: u32 = 0x00F9_EFBA;
const MAX_SAMPLES: usize = 400;

fn main() {
    let packages: Vec<String> = std::env::args().skip(1).collect();
    assert!(!packages.is_empty(), "usage: lot_cavity_stats <package> [more...]");
    let mut opened = Vec::new();
    for path in &packages {
        opened.push(Package::open(path).expect("open package"));
    }

    let mut samples: Vec<(f64, f64, f64)> = Vec::new(); // (d0, d1, coverage)
    let LOT_COLOR_HASHES = [0x0D02_D586u32, 0x0D02_D587, 0x0D02_D588, 0x0D02_D589];
    let mut tile_histogram = [[0usize; 16]; 4];
    let mut skipped = 0usize;

    'outer: for package in &opened {
        for entry in package.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            if samples.len() >= MAX_SAMPLES {
                break 'outer;
            }
            let Ok(raw) = package.read(entry) else { continue };
            let Ok(file) = sc_properties::PropertyFile::parse(&raw) else { continue };
            let doc = LotEditorDocument::from_property_file(file);
            let (Some(size), Some(mask_key)) = (doc.lot_size, doc.lot_mask) else {
                skipped += 1;
                continue;
            };
            // 跨包找 mask raster
            let mut raw_mask: Option<(Vec<u8>, usize, usize)> = None;
            let search = |source: &Package| -> Option<(Vec<u8>, usize, usize)> {
                let hit = source
                    .entries()
                    .iter()
                    .find(|e| e.id.type_id == RASTER_TYPE && e.id.instance == mask_key.instance)
                    .cloned()?;
                let data = source.read(&hit).ok()?;
                let raster = rw4::RasterImage::parse(&data).ok()?;
                if !raster.is_raw_rgba() {
                    return None;
                }
                let pixels = raster.decode_top_mip_rgba().ok()?;
                Some((pixels, raster.width as usize, raster.height as usize))
            };
            let Some((pixels, w, h)) = search(package).or_else(|| {
                opened.iter().find_map(|source| search(source))
            }) else {
                skipped += 1;
                continue;
            };
            // 空腔 = 四通道权重都接近 0（行序不影响质心对称量的正确性：
            // 翻转只改 y 的符号，|质心距离| 不变，故此处直接计算）
            let mut sum_x = 0f64;
            let mut sum_y = 0f64;
            let mut count = 0usize;
            for y in 0..h {
                for x in 0..w {
                    let at = (y * w + x) * 4;
                    let covered = pixels[at] > 16 || pixels[at + 1] > 16 || pixels[at + 2] > 16 || pixels[at + 3] > 16;
                    if !covered {
                        sum_x += x as f64;
                        sum_y += y as f64;
                        count += 1;
                    }
                }
            }
            let total = (w * h) as f64;
            let coverage = 1.0 - count as f64 / total;
            if (count as f64) < total * 0.01 || count as f64 > total * 0.6 {
                continue; // 无有效空腔
            }
            let cx = (sum_x / count as f64 / w as f64 - 0.5) * size[0] as f64;
            let cy = (sum_y / count as f64 / h as f64 - 0.5) * size[1] as f64;
            let d0 = (cx * cx + cy * cy).sqrt();
            // 建筑 bbox 中心（LOD1 bbox，lot 空间）
            let bbox_center = doc
                .properties
                .get(BBOX_HASH)
                .and_then(|p| p.array())
                .and_then(|values| values.first())
                .and_then(|value| match value {
                    Value::BoundingBox { min, max } => Some([
                        (min[0] + max[0]) / 2.0,
                        (min[1] + max[1]) / 2.0,
                    ]),
                    _ => None,
                });
            let d1 = match bbox_center {
                Some(center) => ((cx - center[0] as f64).powi(2) + (cy - center[1] as f64).powi(2)).sqrt(),
                None => d0, // 无 bbox 的 lot：模型即居中锚点
            };
            // LotColor1-4 的 A（tile 索引）分布直方图数据
            for (hash, slot) in LOT_COLOR_HASHES.iter().zip(0..4) {
                if let Some(sc_properties::Value::ColorRgba { a, .. }) = doc
                    .properties
                    .get(*hash)
                    .and_then(|p| p.scalar())
                    .or_else(|| {
                        doc.properties
                            .get(*hash)
                            .and_then(|p| p.array())
                            .and_then(|v| v.first())
                    })
                {
                    let index = a.clamp(0.0, 15.0).round() as usize;
                    tile_histogram[slot][index] += 1;
                }
            }
            samples.push((d0, d1, coverage));
        }
    }

    let n = samples.len();
    println!("samples={n} skipped(no size/mask/raster)={skipped}");
    for (slot, counts) in tile_histogram.iter().enumerate() {
        let total: usize = counts.iter().sum();
        let line: String = counts
            .iter()
            .enumerate()
            .filter(|(_, c)| **c > 0)
            .map(|(index, c)| format!("{index}:{c}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("LotColor{} tile histogram ({total}): {line}", slot + 1);
    }
    if n < 5 {
        return;
    }
    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
    let d0s: Vec<f64> = samples.iter().map(|s| s.0).collect();
    let d1s: Vec<f64> = samples.iter().map(|s| s.1).collect();
    let covs: Vec<f64> = samples.iter().map(|s| s.2).collect();
    let (mut d0s_sorted, mut d1s_sorted, mut covs_sorted) = (d0s.clone(), d1s.clone(), covs.clone());
    d0s_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    d1s_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    covs_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = |values: &Vec<f64>| values[values.len() / 2];
    println!(
        "d0(|cavity-lotCenter|): mean={:.2} median={:.2}",
        mean(&d0s),
        median(&d0s_sorted)
    );
    println!(
        "d1(|cavity-bboxCenter|): mean={:.2} median={:.2}",
        mean(&d1s),
        median(&d1s_sorted)
    );
    println!(
        "coverage: median={:.2} p10={:.2} p90={:.2}",
        median(&covs_sorted),
        covs_sorted[(n as f64 * 0.1) as usize],
        covs_sorted[(n as f64 * 0.9) as usize]
    );
    // 配对差 d0-d1 的配对 t 检验：H0 锚定不优于居中
    let diffs: Vec<f64> = samples.iter().map(|s| s.0 - s.1).collect();
    let diff_mean = mean(&diffs);
    let diff_var = diffs.iter().map(|d| (d - diff_mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let t = diff_mean / (diff_var.sqrt() / (n as f64).sqrt());
    let win = diffs.iter().filter(|d| **d > 0.0).count();
    println!(
        "paired d0-d1: mean={diff_mean:.2} t={t:.2} (df={}, |t|>2.58 → p<0.01) anchor-wins {}/{} = {:.0}%",
        n - 1,
        win,
        n,
        100.0 * win as f64 / n as f64
    );
}
