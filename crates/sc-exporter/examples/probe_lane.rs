//! 一次性探针：从 SimCity_Graphics.package 解出指定 instance 的 raster，
//! 打印尺寸/像素格式并导出 PNG 供测量（停车位笔刷参数标定用）。
//!
//! ```text
//! cargo run -p sc-exporter --example probe_lane -- [package] [instance-hex] [out.png]
//! ```

use dbpf::Package;
use rw4::RasterImage;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let package_path = args
        .next()
        .unwrap_or_else(|| r"D:\ea-games\SimCity\SimCityData\SimCity_Graphics.package".into());
    let instance_hex = args.next().unwrap_or_else(|| "7BE85E77".into());
    let out_path = args.next().unwrap_or_else(|| "tmp/lane_7BE85E77.png".into());

    let instance = u32::from_str_radix(instance_hex.trim_start_matches("0x"), 16)
        .map_err(|_| format!("invalid instance: {instance_hex}"))?;
    let package = Package::open(&package_path).map_err(|e| e.to_string())?;
    let entry = package
        .entries()
        .iter()
        .find(|entry| entry.id.instance == instance)
        .ok_or_else(|| format!("instance 0x{instance:08X} not found"))?;
    println!(
        "TGI {} stored={} decompressed={}",
        entry.id,
        entry.stored_len(),
        entry.decompressed_size
    );

    let data = package.read(entry).map_err(|e| e.to_string())?;
    let raster = RasterImage::parse(&data).map_err(|e| e.to_string())?;
    println!(
        "raster_type={} {}x{} pixFmt={} mips={} pixelSize={}",
        raster.raster_type,
        raster.width,
        raster.height,
        raster.pixel_format,
        raster.mip_count,
        raster.pixel_size
    );

    let rgba = raster.decode_top_mip_rgba().map_err(|e| e.to_string())?;
    let width = raster.width as u32;
    let height = raster.height as u32;

    // 颜色统计：区分"四色通道语义图"与"普通 RGBA 贴图"
    let mut colors = std::collections::BTreeMap::new();
    for px in rgba.chunks_exact(4) {
        *colors.entry([px[0], px[1], px[2], px[3]]).or_insert(0u32) += 1;
    }
    println!("unique colors: {}", colors.len());
    for (color, count) in colors.iter().take(12) {
        println!("  #{:02X}{:02X}{:02X}{:02X} x{}", color[0], color[1], color[2], color[3], count);
    }

    // 结构摘要：强度（任一通道）≥128 的行占用区间（判断线条位置与粗细）
    let intensity = |x: u32, y: u32| {
        let base = ((y * width + x) * 4) as usize;
        rgba[base].max(rgba[base + 1]).max(rgba[base + 2]).max(rgba[base + 3])
    };
    let occupied = |x: u32, y: u32| intensity(x, y) >= 128;
    for y in (0..height).step_by((height / 16).max(1) as usize) {
        let mut runs: Vec<String> = Vec::new();
        let mut run: Option<(u32, u32)> = None;
        for x in 0..width {
            if occupied(x, y) {
                match run.as_mut() {
                    Some((_, end)) => *end = x,
                    None => run = Some((x, x)),
                }
            } else if let Some((start, end)) = run.take() {
                runs.push(format!("{start}-{end}({})", end - start + 1));
            }
        }
        if let Some((start, end)) = run.take() {
            runs.push(format!("{start}-{end}({})", end - start + 1));
        }
        if !runs.is_empty() {
            println!("row {y:>3}: {}", runs.join(" "));
        }
    }

    // 分通道几何：包围盒 + 每隔 4 行/列的行程（定位线条与图层结构）
    for (name, shift) in [("R", 0usize), ("G", 1), ("B", 2), ("A", 3)] {
        let channel = |x: u32, y: u32| rgba[((y * width + x) * 4) as usize + shift];
        let count = (0..width * height).filter(|i| channel(i % width, i / width) >= 128).count();
        println!("channel {name}: {count}/{} px >=128", width * height);
        if count == 0 {
            continue;
        }
        let mut min_x = width;
        let mut max_x = 0;
        let mut min_y = height;
        let mut max_y = 0;
        for y in 0..height {
            for x in 0..width {
                if channel(x, y) >= 128 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }
        println!(
            "  bbox x[{min_x},{max_x}] y[{min_y},{max_y}] = {}x{} px",
            max_x - min_x + 1,
            max_y - min_y + 1
        );
        for y in (0..height).step_by(1) {
            let mut runs: Vec<String> = Vec::new();
            let mut run: Option<(u32, u32)> = None;
            for x in 0..width {
                if channel(x, y) >= 128 {
                    match run.as_mut() {
                        Some((_, end)) => *end = x,
                        None => run = Some((x, x)),
                    }
                } else if let Some((start, end)) = run.take() {
                    runs.push(format!("{start}-{end}({})", end - start + 1));
                }
            }
            if let Some((start, end)) = run.take() {
                runs.push(format!("{start}-{end}({})", end - start + 1));
            }
            if !runs.is_empty() {
                println!("  {name} r{y:>3}: {}", runs.join(" "));
            }
        }
    }

    // ASCII 结构图（2px 采样）：# = ≥128，+ = ≥32，. = >0
    for y in (0..height).step_by(2) {
        let mut line = String::new();
        for x in (0..width).step_by(2) {
            let v = intensity(x, y);
            line.push(if v >= 128 {
                '#'
            } else if v >= 32 {
                '+'
            } else if v > 0 {
                '.'
            } else {
                ' '
            });
        }
        println!("|{line}|");
    }

    // 归一化可视化：强度 → 灰度不透明，便于肉眼/工具测量
    let mut viz = rgba.clone();
    for px in viz.chunks_exact_mut(4) {
        let v = px[0].max(px[1]).max(px[2]).max(px[3]);
        px[0] = v;
        px[1] = v;
        px[2] = v;
        px[3] = 255;
    }
    image::save_buffer(
        &out_path,
        &viz,
        width,
        height,
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())?;
    println!("saved {out_path}");
    Ok(())
}
