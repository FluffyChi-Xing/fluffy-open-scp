//! 锚点探针：对带 placement(平移) + 可解 LotMask + 模型的 property，
//! 解出模型 XZ 脚印与 mask 四色区域，用于判定地面定位语义。仅开发用。
//!
//! 用法：cargo run -p sc-properties --example lot_anchor_probe -- <package> [max]

use dbpf::ResourceId;
use std::collections::HashMap;

fn main() {
    let path = std::env::args().nth(1).expect("usage: lot_anchor_probe <package> [max]");
    let max: usize = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(8);
    let package = dbpf::Package::open(&path).expect("open package");

    // raster 索引
    let mut rasters: HashMap<u32, _> = HashMap::new();
    for entry in package.entries() {
        if entry.id.type_id == 0x2F4E_681C {
            rasters.insert(entry.id.instance, entry.clone());
        }
    }

    let mut shown = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 { continue; }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = sc_properties::PropertyFile::parse_with_limits(&data, sc_properties::ParseLimits::default()) else { continue };
        let document = sc_properties::LotEditorDocument::from_property_file(file);
        let placement_t = document.placement.as_ref().filter(|t| t.matrix.len() == 12)
            .map(|t| (t.matrix[9], t.matrix[10]));
        // 有 placement 的只看平移非零的；无 placement 的全要（含 offset=0，验证默认是否居中）
        if let Some((tx, ty)) = placement_t {
            if tx.abs() + ty.abs() < 0.01 { continue; }
        }
        let (tx, ty) = placement_t.unwrap_or((f32::NAN, f32::NAN));
        let Some(mask_key) = document.lot_mask else { continue };
        let Some(lot_size) = document.lot_size else { continue };
        let Some(raster_entry) = rasters.get(&mask_key.instance) else { continue };
        let Ok(bytes) = package.read(raster_entry) else { continue };
        let Ok(raster) = rw4::RasterImage::parse(&bytes) else { continue };
        let Ok(_rgba) = raster.decode_lot_mask_rgba(&[[0; 3]; 4]) else { continue };
        let Some(model_key) = document.model else { continue };

        // 模型 XZ（数据帧 Z-up：脚印 = X/Y 范围，高度 = Z）
        let Some(model_entry) = package.entries().iter().find(|e| {
            e.id.type_id == 0x2F4E_681B && e.id.instance == model_key.instance
        }).map(|e| e.clone()) else { continue; };
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
        let Some(rgba) = raster.decode_lot_mask_rgba(&[[0, 0, 0], [255, 0, 0], [0, 255, 0], [0, 0, 255]]).ok() else { continue };
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
        println!("lot 0x{:08X} size={:.0}x{:.0} t=({tx},{ty}) offset={:?} fd0={fd0:?} fd2={:?} fd3={:?} mask_inst=0x{:08X} {}x{} model_bbox={:.1}x{:.1}x{:.1} model_center=({:+.1},{:+.1})",
            entry.id.instance, lot_size[0], lot_size[1], document.lot_offset,
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
        shown += 1;
        if shown >= max { break; }
    }
}
