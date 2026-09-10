//! 锚点探针：对带 placement(平移) + 可解 LotMask + 模型的 property，
//! 解出模型 XZ 脚印与 mask 四色区域，用于判定地面定位语义。仅开发用。
//!
//! 用法：cargo run -p sc-properties --example lot_anchor_probe -- <package> [max] [lot_instance]
//! 指定 lot_instance 时进入定向模式：跳过所有过滤，仅输出该 lot。

use dbpf::ResourceId;
use std::collections::HashMap;

fn main() {
    let path = std::env::args().nth(1).expect("usage: lot_anchor_probe <package> [max] [lot_instance]");
    let max: usize = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(8);
    let target_instance: Option<u32> = std::env::args().nth(3)
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
    // 追加参数：额外的 raster 查找包（跨包 mask）
    let extra_packages: Vec<String> = std::env::args().skip(4).collect();
    let package = dbpf::Package::open(&path).expect("open package");

    // raster 索引（主包 + 附加包）
    let mut rasters: HashMap<u32, _> = HashMap::new();
    let mut extra_raster_pkgs: Vec<(dbpf::Package, HashMap<u32, std::path::PathBuf>)> = Vec::new();
    for entry in package.entries() {
        if entry.id.type_id == 0x2F4E_681C {
            rasters.insert(entry.id.instance, entry.clone());
        }
    }
    for extra in &extra_packages {
        if let Ok(extra_pkg) = dbpf::Package::open(extra) {
            let mut index = HashMap::new();
            for entry in extra_pkg.entries() {
                if entry.id.type_id == 0x2F4E_681C {
                    index.insert(entry.id.instance, std::path::PathBuf::from(extra));
                }
            }
            extra_raster_pkgs.push((extra_pkg, index));
        }
    }
    // 跨包读取：本地没有则尝试附加包
    let read_mask_raster = |rasters: &HashMap<u32, dbpf::IndexEntry>,
                            extra: &Vec<(dbpf::Package, HashMap<u32, std::path::PathBuf>)>,
                            instance: u32|
     -> Option<Vec<u8>> {
        if let Some(entry) = rasters.get(&instance) {
            return package.read(entry).ok();
        }
        for (pkg, index) in extra {
            if index.contains_key(&instance) {
                if let Some(entry) = pkg.entry(dbpf::ResourceId { type_id: 0x2F4E_681C, group: 0, instance }) {
                    return pkg.read(&entry.clone()).ok();
                }
            }
        }
        None
    };

    let mut shown = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 { continue; }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = sc_properties::PropertyFile::parse_with_limits(&data, sc_properties::ParseLimits::default()) else { continue };
        let document = sc_properties::LotEditorDocument::from_property_file(file);
        let placement_t = document.placement.as_ref().filter(|t| t.matrix.len() == 12)
            .map(|t| (t.matrix[9], t.matrix[10]));
        // 定向模式：跳过所有过滤，仅输出该 lot
        if let Some(target) = target_instance {
            if entry.id.instance != target { continue; }
        }
        // 有 placement 的只看平移非零的；无 placement 的全要（含 offset=0，验证默认是否居中）
        if target_instance.is_none() && placement_t.is_some() {
            let (tx, ty) = placement_t.unwrap();
            if tx.abs() + ty.abs() < 0.01 { continue; }
        }
        let (tx, ty) = placement_t.unwrap_or((f32::NAN, f32::NAN));
        let trace = target_instance.is_some();
        let Some(mask_key) = document.lot_mask else {
            if trace { eprintln!("skip: no lot_mask (model={:?})", document.model.as_ref().map(|k| k.instance)); }
            continue;
        };
        let Some(lot_size) = document.lot_size else {
            if trace { eprintln!("skip: no lot_size"); }
            continue;
        };
        let bytes = match rasters.get(&mask_key.instance).map(|entry| package.read(entry).ok()) {
            Some(bytes) => bytes,
            None => read_mask_raster(&rasters, &extra_raster_pkgs, mask_key.instance),
        };
        let Some(bytes) = bytes else {
            if trace { eprintln!("skip: mask raster 0x{:08X} unreadable", mask_key.instance); }
            continue;
        };
        let Ok(raster) = rw4::RasterImage::parse(&bytes) else {
            if trace { eprintln!("skip: raster parse failed"); }
            continue;
        };
        let Ok(_rgba) = raster.decode_lot_mask_rgba(&[[0; 4]; 4]) else {
            if trace { eprintln!("skip: mask decode failed (w={} h={} pixFmt={})", raster.width, raster.height, raster.pixel_format); }
            continue;
        };
        let Some(model_key) = document.model else {
            if trace { eprintln!("skip: no model"); }
            continue;
        };

        // 模型 XZ（数据帧 Z-up：脚印 = X/Y 范围，高度 = Z）
        let Some(model_entry) = package.entries().iter().find(|e| {
            e.id.type_id == 0x2F4E_681B && e.id.instance == model_key.instance
        }).map(|e| e.clone()) else {
            if trace { eprintln!("skip: model 0x{:08X} not in package", model_key.instance); }
            continue;
        };
        let Ok(model_bytes) = package.read(&model_entry) else { continue };
        let Ok(rw) = rw4::Rw4File::parse(&model_bytes) else { continue };
        let (mut minx, mut maxx, mut miny, mut maxy, mut minz, mut maxz) =
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let mut verts = 0usize;
        for mesh_sec in rw.sections_of_type(rw4::SectionType::MESH) {
            let Ok(mesh) = rw.decode_mesh(&model_bytes, mesh_sec.number) else { continue };
            for v in &mesh.vertices {
                let Some(p) = v.position() else { continue };
                verts += 1;
                minx = minx.min(p[0]); maxx = maxx.max(p[0]);
                miny = miny.min(p[1]); maxy = maxy.max(p[1]);
                minz = minz.min(p[2]); maxz = maxz.max(p[2]);
            }
        }
        if verts == 0 { continue; }

        // mask 四色区域（黑白红绿蓝量化 → 统计每通道像素 bbox）
        let Some(rgba) = raster.decode_lot_mask_rgba(&[[0, 0, 0, 0], [255, 0, 0, 0], [0, 255, 0, 0], [0, 0, 255, 0]]).ok() else { continue };
        let (w, h) = (raster.width as usize, raster.height as usize);
        let mut regions = [[usize::MAX; 2], [usize::MAX; 2], [usize::MAX; 2], [usize::MAX; 2]];
        let mut regions_max = [[0usize; 2]; 4];
        let mut counts = [0usize; 4];
        for (i, px) in rgba.chunks_exact(4).enumerate() {
            let (x, y) = (i % w, i / w);
            // 最近纯色（量化输出即是精确色）
            let c = match (px[0], px[1], px[2]) {
                (0, 0, 0) => 0,
                (255, 0, 0) => 1,
                (0, 255, 0) => 2,
                (0, 0, 255) => 3,
                _ => continue,
            };
            counts[c] += 1;
            regions[c][0] = regions[c][0].min(x);
            regions[c][1] = regions[c][1].min(y);
            regions_max[c][0] = regions_max[c][0].max(x);
            regions_max[c][1] = regions_max[c][1].max(y);
        }
        // 像素→地块单位
        let sx = lot_size[0] / w as f32;
        let sy = lot_size[1] / h as f32;
        // 0x0CCB7FD0/FD2/FD3 候选锚点属性
        let dump_vec2 = |hash: u32, values: &sc_properties::PropertyFile| -> Option<[f32; 2]> {
            match &values.values.iter().find(|p| p.hash == hash)?.kind {
                sc_properties::Kind::Scalar(sc_properties::Value::Vector2(v)) => Some(*v),
                _ => None,
            }
        };
        let fd0 = dump_vec2(0x0CCB_7FD0, &document.properties);
        println!("lot 0x{:08X} model_inst=0x{:08X} size={:.0}x{:.0} t=({tx},{ty}) offset={:?} fd0={fd0:?} fd2={:?} fd3={:?} mask_inst=0x{:08X} {}x{} model_bbox={:.1}x{:.1}x{:.1} model_center=({:+.1},{:+.1})",
            entry.id.instance, model_key.instance, lot_size[0], lot_size[1], document.lot_offset,
            dump_vec2(0x0CCB_7FD2, &document.properties), dump_vec2(0x0CCB_7FD3, &document.properties),
            mask_key.instance, w, h,
            maxx - minx, maxy - miny, maxz - minz,
            (minx + maxx) / 2.0, (miny + maxy) / 2.0);
        for c in 1..4 {
            if counts[c] == 0 { continue; }
            let bw = (regions_max[c][0] - regions[c][0] + 1) as f32 * sx;
            let bh = (regions_max[c][1] - regions[c][1] + 1) as f32 * sy;
            let cx = ((regions[c][0] + regions_max[c][0]) as f32 / 2.0 - (w as f32 - 1.0) / 2.0) * sx;
            let cy = ((regions[c][1] + regions_max[c][1]) as f32 / 2.0 - (h as f32 - 1.0) / 2.0) * sy;
            println!("    color{c}: px_count={:<6} bbox={:.1}x{:.1} units, center_offset=({cx:+.1},{cy:+.1}) from mask center", counts[c], bw, bh);
        }
        // 对齐评分 v4：4 假设（旋转 ±90° × 平移 ±t）下，把建筑 bbox 映射进
        // mask 像素空间，与主足迹色区 bbox 求 IoU，多数票定约定。
        let full_bleed = |c: usize| -> bool {
            if counts[c] == 0 { return true; }
            (regions_max[c][0] - regions[c][0] + 1) as f64 / w as f64 > 0.95
                && (regions_max[c][1] - regions[c][1] + 1) as f64 / h as f64 > 0.95
        };
        let footprint_color = (1..4)
            .filter(|&c| counts[c] > 0 && !full_bleed(c))
            .max_by_key(|&c| counts[c]);
        // 无 placement 的 lot：轴长检验（0° vs 90°）。建筑 X×Y 与主足迹色
        // bbox 列×行的匹配度，取对数比。
        if let Some(foot_c) = footprint_color {
            if placement_t.is_none() {
                let fw = (regions_max[foot_c][0] - regions[foot_c][0] + 1) as f32 * sx;
                let fh = (regions_max[foot_c][1] - regions[foot_c][1] + 1) as f32 * sy;
                let (bw, bh) = (maxx - minx, maxy - miny);
                let direct = ((bw - fw).abs() + (bh - fh).abs()) / (bw + bh);
                let swapped = ((bw - fh).abs() + (bh - fw).abs()) / (bw + bh);
                let winner = if direct < swapped { "0deg" } else { "90deg" };
                println!("    axes: bbox={bw:.1}x{bh:.1} color={fw:.1}x{fh:.1} direct={direct:.2} swapped={swapped:.2} → {winner}");
            }
        }
        if let (Some(foot_c), Some((tx, ty))) = (footprint_color,
                placement_t.filter(|(x, y)| x.abs() + y.abs() >= 0.01)) {
            // 主足迹色区 bbox（像素）
            let (fx0, fy0) = (regions[foot_c][0] as f32, regions[foot_c][1] as f32);
            let (fx1, fy1) = (regions_max[foot_c][0] as f32, regions_max[foot_c][1] as f32);
            // 假设 (eps, sigma)：mask 内像素 q 对应模型点
            //   p = R(eps*90°)·q + sigma*t（ground = 平移 sigma*t 后旋转 eps*90°）
            // 反解：建筑 bbox 角点 p → q = R(-eps*90°)·(p − sigma·t)
            // q 坐标以 mask 中心为原点（像素），u=列、v=行。
            let iou = |eps: f32, sigma: f32| -> f32 {
                let corners = [
                    (minx, miny), (maxx, miny), (minx, maxy), (maxx, maxy),
                ];
                let (sin, cos) = (eps * 90f32.to_radians().sin(), eps * 90f32.to_radians().cos());
                // 模型坐标（米）→ 像素：先减 sigma*t，旋转 −eps·90°，再换算像素
                let to_px = |x: f32, y: f32| -> (f32, f32) {
                    let dx = x - sigma * tx;
                    let dy = y - sigma * ty;
                    let (u, v) = (cos * dx + sin * dy, -sin * dx + cos * dy);
                    (u / sx + w as f32 / 2.0, v / sy + h as f32 / 2.0)
                };
                let pts: Vec<(f32, f32)> = corners.iter().map(|&(x, y)| to_px(x, y)).collect();
                let (qx0, qx1) = (pts.iter().map(|p| p.0).fold(f32::MAX, f32::min), pts.iter().map(|p| p.0).fold(f32::MIN, f32::max));
                let (qy0, qy1) = (pts.iter().map(|p| p.1).fold(f32::MAX, f32::min), pts.iter().map(|p| p.1).fold(f32::MIN, f32::max));
                let ix = qx0.max(fx0).min(qx1).min(fx1);
                let iy = qy0.max(fy0).min(qy1).min(fy1);
                if ix <= 0.0 && iy <= 0.0 { return 0.0; }
                let iw = (qx1.min(fx1) - qx0.max(fx0)).max(0.0);
                let ih = (qy1.min(fy1) - qy0.max(fy0)).max(0.0);
                let inter = iw * ih;
                let union = (qx1 - qx0) * (qy1 - qy0) + (fx1 - fx0) * (fy1 - fy0) - inter;
                if union <= 0.0 { 0.0 } else { inter / union }
            };
            let hypotheses = [
                (0.0, 1.0, "rot0,inv"),
                (0.0, -1.0, "rot0,fwd"),
                (-1.0, 1.0, "rot-90,inv"),
                (1.0, 1.0, "rot+90,inv"),
                (-1.0, -1.0, "rot-90,fwd"),
                (1.0, -1.0, "rot+90,fwd"),
            ];
            let mut best = (0.0f32, "");
            for &(eps, sigma, label) in &hypotheses {
                let score = iou(eps, sigma);
                if score > best.0 { best = (score, label); }
                print!(" {label}={score:.2}");
            }
            println!("    iou(color{foot_c}):{best_label}", best_label = format!(" → {}", best.1));
        }
        shown += 1;
        if shown >= max { break; }
    }
}
