//! 只读取证：lot 的地面定位三要素（P=0xDB7FB17 / unitOffset=0x0CCB7FC9 /
//! bbox=0xF9EFBA）与 prop 槽位变换（0x0C12EF30 族）的空间关系。
//!
//! 判定问题：openscp 里 prop 标记（模型空间原样）与 raster 绿地
//! （P⁻¹·T(unitOffset) 摆放）出现一致性偏移——本探针输出两侧的候选
//! 锚点值，用数据定谳引擎的栅格映射口径（pos 在 model 系还是 lot 系）。
//!
//! 用法：lot_prop_offset_probe <lot_instance_hex> <lot_group_hex> <package>

use dbpf::Package;
use rw4::RasterImage;
use sc_properties::{Kind, PropertyFile, ParseLimits, Value};
use sc_properties::PROPERTY_RESOURCE_TYPE;

fn values_of<'a>(file: &'a PropertyFile, hash: u32) -> Vec<&'a Value> {
    file.values
        .iter()
        .filter(|p| p.hash == hash)
        .flat_map(|p| match &p.kind {
            Kind::Scalar(v) => vec![v],
            Kind::Array(vs) => vs.iter().collect(),
            Kind::Empty => vec![],
        })
        .collect()
}

/// prop 平移列表（bin, x, y）
fn props_of(file: &PropertyFile) -> Vec<(usize, f32, f32)> {
    let mut out = Vec::new();
    for bin in 0..14usize {
        for v in values_of(file, 0x0C12_EF30 + bin as u32) {
            if let Value::Transform(t) = v {
                if t.matrix.len() >= 12 {
                    out.push((bin, t.matrix[9], t.matrix[10]));
                }
            }
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: lot_prop_offset_probe <instance_hex> <group_hex> <package> [extra_package...]");
        std::process::exit(2);
    }
    let instance = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("instance");
    let group = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("group");
    let packages: Vec<Package> = args[2..]
        .iter()
        .map(|p| Package::open(p).expect("open package"))
        .collect();

    let mut found = false;
    for entry in packages[0].entries().iter().filter(|e| {
        e.id.type_id == PROPERTY_RESOURCE_TYPE && e.id.instance == instance && e.id.group == group
    }) {
        found = true;
        let data = packages[0].read(entry).expect("read lot");
        let file =
            PropertyFile::parse_with_limits(&data, ParseLimits::default()).expect("parse lot");
        println!("== lot {:08X}/{:08X}", entry.id.instance, entry.id.group);

        // mask raster（0x0CCB7FD5 key → 裸 raw RGBA）
        let mask_pixels: Option<(Vec<u8>, usize, usize)> = values_of(&file, 0x0CCB_7FD5)
            .first()
            .and_then(|v| match v {
                Value::Key(k) => Some(k.instance),
                _ => None,
            })
            .and_then(|mask_instance| {
                for pkg in &packages {
                    for e in pkg.entries().iter() {
                        if e.id.instance != mask_instance {
                            continue;
                        }
                        let Ok(raw) = pkg.read(e) else { continue };
                        let Ok(raster) = RasterImage::parse(&raw) else {
                            continue;
                        };
                        if !raster.is_raw_rgba() {
                            continue;
                        }
                        let px = raster.decode_top_mip_rgba().expect("decode mask");
                        return Some((px, raster.width as usize, raster.height as usize));
                    }
                }
                None
            });
        let lot_size: [f32; 2] = match values_of(&file, 0x0CCB_7FC8).first() {
            Some(Value::Vector2(v)) => *v,
            _ => {
                // 后端口径：mask 尺寸 × 0.75 m/px
                match &mask_pixels {
                    Some((_, w, h)) => [*w as f32 * 0.75, *h as f32 * 0.75],
                    None => [96.0, 96.0],
                }
            }
        };
        println!("lotSize={lot_size:?}");

        // 数值定谳：绿通道连通域质心 vs prop 位置（两种尺度的最小残差对比）
        let props = props_of(&file);
        if let Some((px, mw, mh)) = &mask_pixels {
            let (w, h) = (*mw, *mh);
            let mut label = vec![0u32; w * h];
            let mut blobs: Vec<(u64, i64, i64)> = Vec::new();
            for start in 0..w * h {
                if px[start * 4 + 1] <= 127 || label[start] != 0 {
                    continue;
                }
                let id = (blobs.len() + 1) as u32;
                let mut stack = vec![start];
                label[start] = id;
                let (mut area, mut sx, mut sy) = (0u64, 0i64, 0i64);
                while let Some(cur) = stack.pop() {
                    let cx = (cur % w) as i64;
                    let cy = (cur / w) as i64;
                    area += 1;
                    sx += cx;
                    sy += cy;
                    for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                            continue;
                        }
                        let n = nx as usize + ny as usize * w;
                        if label[n] == 0 && px[n * 4 + 1] > 127 {
                            label[n] = id;
                            stack.push(n);
                        }
                    }
                }
                blobs.push((area, sx, sy));
            }
            blobs.sort_by(|a, b| b.0.cmp(&a.0));
            // PNG 可视化：绿通道对比拉伸灰度 ×4，prop 红点=0.75 映射、蓝点=1.0 映射
            {
                let scale = 4usize;
                let mut img = image::RgbImage::new(w as u32 * scale as u32, h as u32 * scale as u32);
                for y in 0..h {
                    for x in 0..w {
                        let g = px[(x + y * w) * 4 + 1];
                        let v = ((g as f32 / 255.0).powf(0.5) * 255.0) as u8;
                        for dy in 0..scale {
                            for dx in 0..scale {
                                img.put_pixel(
                                    (x * scale + dx) as u32,
                                    (y * scale + dy) as u32,
                                    image::Rgb([v, v, v]),
                                );
                            }
                        }
                    }
                }
                let dot = |img: &mut image::RgbImage, x: f32, y: f32, mpp: f32, c: [u8; 3]| {
                    let u = ((x / (w as f32 * mpp) + 1.0) * 0.5 * w as f32) as usize;
                    let v = ((y / (h as f32 * mpp) + 1.0) * 0.5 * h as f32) as usize;
                    for dy in -1..=1i64 {
                        for dx in -1..=1i64 {
                            let px_i = u as i64 + dx;
                            let py_i = v as i64 + dy;
                            if px_i < 0 || py_i < 0 || px_i >= w as i64 || py_i >= h as i64 {
                                continue;
                            }
                            for sy in 0..scale {
                                for sx in 0..scale {
                                    img.put_pixel(
                                        ((px_i as usize) * scale + sx) as u32,
                                        ((py_i as usize) * scale + sy) as u32,
                                        image::Rgb(c),
                                    );
                                }
                            }
                        }
                    }
                };
                for (_, x, y) in &props {
                    dot(&mut img, *x, *y, 0.75, [255, 40, 40]);
                    dot(&mut img, *x, *y, 1.0, [60, 120, 255]);
                }
                img.save("tmp/lot_mask_prop_scales.png").expect("save png");
                println!("可视化已存 tmp/lot_mask_prop_scales.png（红=0.75m/px，蓝=1.0m/px）");
            }
            // 互配打分：扫描（尺度 s, 偏移 dx, dy），score = Σ prop 的 B 通道采样值
            {
                let mut best = (0f32, 0f32, 0f32, 0f32); // (score, s, dx, dy)
                let mut s_ = 0.60f32;
                while s_ <= 1.45 {
                    let mut dx = -10.0f32;
                    while dx <= 10.0 {
                        let mut dy = -10.0f32;
                        while dy <= 10.0 {
                            let half = w as f32 * s_ / 2.0;
                            let half_y = h as f32 * s_ / 2.0;
                            let mut score = 0f32;
                            for (_, x, y) in &props {
                                let u = (((x + dx) / half + 1.0) * 0.5).clamp(0.0, 0.999);
                                let v = (((y + dy) / half_y + 1.0) * 0.5).clamp(0.0, 0.999);
                                let at = ((u * w as f32) as usize + (v * h as f32) as usize * w) * 4;
                                score += px[at + 2] as f32;
                            }
                            if score > best.0 {
                                best = (score, s_, dx, dy);
                            }
                            dy += 0.5;
                        }
                        dx += 0.5;
                    }
                    s_ += 0.02;
                }
                println!(
                    "B 通道互配最优: score={:.0} (均值 {:.1}/17) 尺度={:.2}m/px 偏移=({:+.1},{:+.1})m",
                    best.0,
                    best.0 / props.len() as f32,
                    best.1,
                    best.2,
                    best.3
                );
                println!("（现状口径: 尺度=0.75 偏移=(0,0)；H4: 尺度=1.0 偏移=(0,0)）");
            }

            println!("绿通道连通域 top10（px 质心 → 两尺度世界坐标；bin12 prop y≈-36..-38, x=-31.8..+27.2）:");
            for (area, sx, sy) in blobs.iter().take(10) {
                let cx = *sx as f32 / *area as f32;
                let cy = *sy as f32 / *area as f32;
                let wx075 = (cx / w as f32 - 0.5) * w as f32 * 0.75;
                let wy075 = (cy / h as f32 - 0.5) * h as f32 * 0.75;
                let wx100 = (cx / w as f32 - 0.5) * w as f32;
                let wy100 = (cy / h as f32 - 0.5) * h as f32;
                println!(
                    "  area={area:>5} px=({cx:6.1},{cy:6.1}) -> 0.75:({wx075:7.2},{wy075:7.2})  1.0:({wx100:7.2},{wy100:7.2})"
                );
            }
        } else {
            println!("mask raster 未找到（跳过采样判据）");
        }

        // CCB7 家族属性全览（找 raster 范围/overlay box 尺寸）
        let mut fam: Vec<_> = file.values.iter().filter(|p| (p.hash >> 8) == (0x0CCB_7F00 >> 8)).map(|p| p.hash).collect();
        fam.sort();
        fam.dedup();
        for h in fam {
            let vals = values_of(&file, h);
            let brief: Vec<String> = vals.iter().take(2).map(|v| format!("{v:?}")).collect();
            println!("0x{h:08X}: {} 值 {}", vals.len(), brief.join(" | "));
        }

        // placement P（0xDB7FB17）
        for (i, v) in values_of(&file, 0x0DB7_FB17).iter().enumerate() {
            if let Value::Transform(t) = v {
                println!(
                    "P[{i}] flags={} unknown={:?} matrix={:?}",
                    t.flags, t.unknown, t.matrix
                );
            }
        }
        // unitOffset（0x0CCB7FC9）
        println!("unitOffset: {:?}", values_of(&file, 0x0CCB_7FC9));
        // bbox（0xF9EFBA）
        println!("bbox: {:?}", values_of(&file, 0x00F9_EFBA));
        // LotSize（0x0CCB7FC8）
        println!("lotSize: {:?}", values_of(&file, 0x0CCB_7FC8));
        // prop 14 分箱：id 列与 transform 列
        for bin in 0..14usize {
            let ids = values_of(&file, 0x0C12_EF20 + bin as u32);
            let tfs = values_of(&file, 0x0C12_EF30 + bin as u32);
            let slots = values_of(&file, 0x0C12_EF40 + bin as u32);
            if ids.is_empty() && tfs.is_empty() {
                continue;
            }
            println!(
                "-- bin {bin}: ids={} transforms={} slots={}",
                ids.len(),
                tfs.len(),
                slots.len()
            );
            for (i, v) in tfs.iter().enumerate() {
                if let Value::Transform(t) = v {
                    let m = &t.matrix;
                    // 12 floats：行主序（引擎 cSPTransform 三个行向量 + 平移行）
                    let f = |k: usize| m.get(k).copied().unwrap_or(f32::NAN);
                    println!(
                        "   tf[{i}] flags={} unknown={:?} t=({:.3},{:.3},{:.3}) row0=({:.3},{:.3},{:.3})",
                        t.flags,
                        t.unknown,
                        f(9),
                        f(10),
                        f(11),
                        f(0),
                        f(1),
                        f(2),
                    );
                }
            }
        }
    }
    if !found {
        eprintln!("lot not found: {instance:08X}/{group:08X}");
        std::process::exit(1);
    }
}
