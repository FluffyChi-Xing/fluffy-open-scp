//! 只读取证：把建筑物模型的**地面脚印多边形**投影到其所属 lot 的合成图上。
//!
//! 用途：诊断「建筑在 lot 内的偏移」——游戏里建筑贴 lot 一角，而 app 渲染
//! 在中心压路。本探针把建筑网格的贴地三角形经 LotPlacementTransform
//! （模型→地块）变换后画到 lot 合成图上，与游戏截图对照即知偏移来源。
//!
//! 同时打印：模型网格 AABB、0xF9EFBA "Model Bounding Box" 属性值、
//! LotPlacementTransform 矩阵、LotOverlayBoxOffset——四者互证定位差异。
//!
//! 用法：cargo run -p sc-exporter --release --example footprint_project -- \
//!   <lot package> <model instance hex> [--out dir] [lookup package...]
use dbpf::Package;
use rw4::{Rw4File, SectionType};
use sc_properties::{assemble_units, LotEditorDocument};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const MODEL_TYPE: u32 = 0x2F4E_681B;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const H_MODEL_BBOX: u32 = 0x00F9_EFBA;
const H_OVERLAY_OFFSET: u32 = 0x0CCB_7FC9;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut out_dir = "tmp/footprint".to_owned();
    args.retain(|a| {
        if let Some(value) = a.strip_prefix("--out=") {
            out_dir = value.to_owned();
            false
        } else {
            a != "--out"
        }
    });
    std::fs::create_dir_all(&out_dir).unwrap();
    let (lot_path, model_hex) = (
        args[0].clone(),
        u32::from_str_radix(args[1].trim_start_matches("0x"), 16)
            .expect("model instance hex"),
    );
    let mut paths = vec![lot_path.clone()];
    paths.extend(args[2..].iter().cloned());
    let packages: Vec<Package> = paths
        .iter()
        .map(|p| Package::open(p).expect("open package"))
        .collect();

    // ---- 1. 找引用该模型的 lot property ----
    let mut found: Option<(usize, dbpf::IndexEntry, LotEditorDocument)> = None;
    'outer: for (owner, package) in packages.iter().enumerate() {
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE {
                continue;
            }
            let entry = entry.clone();
            let Ok(data) = package.read(&entry) else { continue };
            let Ok(properties) =
                sc_properties::PropertyFile::parse(&data)
            else {
                continue;
            };
            let document = LotEditorDocument::from_property_file(properties);
            if document
                .model_lods
                .iter()
                .flatten()
                .any(|key| key.instance == model_hex)
            {
                found = Some((owner, entry, document));
                break 'outer;
            }
        }
    }
    let (lot_owner, lot_entry, doc) =
        found.unwrap_or_else(|| panic!("no lot property references 0x{model_hex:08X}"));
    println!(
        "model 0x{model_hex:08X} <- lot property 0x{:08X} (pkg={})",
        lot_entry.id.instance, paths[lot_owner]
    );

    let lot_size = doc.lot_size.unwrap_or([48.0, 48.0]);
    let placement = doc
        .placement
        .as_ref()
        .map(|t| t.matrix.clone())
        .unwrap_or_else(|| vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
    let overlay_offset = doc.lot_offset.unwrap_or([0.0, 0.0]);
    let mask_key = doc.lot_mask.expect("lot has no LotMask");
    println!(
        "LotSize = {lot_size:?}  LotOverlayBoxOffset = {overlay_offset:?}"
    );
    println!(
        "LotPlacementTransform = {:?}",
        placement.iter().map(|v| (*v * 1000.0).round() / 1000.0).collect::<Vec<_>>()
    );

    // 0xF9EFBA Model Bounding Box（属性授权的建筑包围盒）+ xy 中心
    let mut bbox_center: [f32; 2] = [0.0, 0.0];
    if let Some(prop) = doc.properties.get(H_MODEL_BBOX) {
        let value = prop
            .scalar()
            .or_else(|| prop.array().and_then(|values| values.first()));
        println!("ModelBoundingBox (0xF9EFBA) = {value:?}");
        if let Some(sc_properties::Value::BoundingBox { min, max }) = value {
            bbox_center = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
            println!(
                "  bbox center (model space) = ({:.3}, {:.3})",
                bbox_center[0], bbox_center[1]
            );
        }
    } else {
        println!("ModelBoundingBox (0xF9EFBA) = (absent)");
    }

    // ---- 2. LotMask 栅格 ----
    let mask_entry = packages
        .iter()
        .find_map(|p| {
            p.entries()
                .iter()
                .find(|e| {
                    e.id.type_id == RASTER_TYPE && e.id.instance == mask_key.instance
                })
                .cloned()
        })
        .expect("LotMask raster not found");
    let owner = packages
        .iter()
        .position(|p| p.entries().iter().any(|e| e.id == mask_entry.id))
        .unwrap();
    let mask_bytes = packages[owner].read(&mask_entry).unwrap();
    let raster = rw4::RasterImage::parse(&mask_bytes).expect("parse mask");
    let (mw, mh) = (raster.width as usize, raster.height as usize);
    let mask = raster.decode_top_mip_rgba().expect("decode mask");
    println!("LotMask {mw}x{mh}");

    // ---- 3. 建筑网格：位置 + 贴地三角形 ----
    let model_entry = packages
        .iter()
        .find_map(|p| {
            p.entries()
                .iter()
                .find(|e| e.id.type_id == MODEL_TYPE && e.id.instance == model_hex)
                .cloned()
        })
        .expect("model not found");
    let model_owner = packages
        .iter()
        .position(|p| p.entries().iter().any(|e| e.id == model_entry.id))
        .unwrap();
    let model_bytes = packages[model_owner].read(&model_entry).unwrap();
    let file = Rw4File::parse(&model_bytes).expect("parse model");
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut ground_tris: Vec<[[f32; 3]; 3]> = Vec::new();
    for section in file.sections_of_type(SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(&model_bytes, section.number) else {
            continue;
        };
        let verts: Vec<[f32; 3]> = mesh
            .vertices
            .iter()
            .filter_map(|v| v.position())
            .collect();
        if verts.is_empty() {
            continue;
        }
        let ground = 0.5f32; // 触地判定（绝对）：塔墙止于 z=0、广场埋至 -1 // 触地判定（绝对）：塔墙止于 z=0、广场埋至 -1，取 z<=0.5 均触地
        let base = positions.len() as u32;
        for v in &verts {
            positions.push(*v);
        }
        for t in &mesh.triangles {
            let (ia, ib, ic) = (base + u32::from(t[0]), base + u32::from(t[1]), base + u32::from(t[2]));
            let (Some(a), Some(b), Some(c)) = (
                positions.get(ia as usize),
                positions.get(ib as usize),
                positions.get(ic as usize),
            ) else {
                continue;
            };
            let min_vz = a[2].min(b[2]).min(c[2]);
            if min_vz <= ground {
                ground_tris.push([*a, *b, *c]);
            }
        }
        println!(
            "mesh #{}: {} verts, ground-touching z<={ground:.2}",
            section.number,
            verts.len()
        );
    }
    let (min_x, max_x) = positions.iter().fold((f32::MAX, f32::MIN), |acc, p| {
        (acc.0.min(p[0]), acc.1.max(p[0]))
    });
    let (min_y, max_y) = positions.iter().fold((f32::MAX, f32::MIN), |acc, p| {
        (acc.0.min(p[1]), acc.1.max(p[1]))
    });
    println!(
        "model AABB (mesh): x[{min_x:.2},{max_x:.2}] y[{min_y:.2},{max_y:.2}] ground_tris={}",
        ground_tris.len()
    );

    // ---- 4. 变换到 lot 空间 ----
    // 列主序 12 floats：前三列 = 旋转基，m[9..11] = 平移（模型→地块）。
    let xform = |p: &[f32; 3]| -> [f32; 3] {
        [
            placement[0] * p[0] + placement[3] * p[1] + placement[6] * p[2] + placement[9],
            placement[1] * p[0] + placement[4] * p[1] + placement[7] * p[2] + placement[10],
            placement[2] * p[0] + placement[5] * p[1] + placement[8] * p[2] + placement[11],
        ]
    };
    let ground_lot: Vec<[[f32; 3]; 3]> = ground_tris
        .iter()
        .map(|t| [xform(&t[0]), xform(&t[1]), xform(&t[2])])
        .collect();
    let (lmin_x, lmax_x) = ground_lot.iter().fold((f32::MAX, f32::MIN), |acc, t| {
        (
            acc.0.min(t[0][0]).min(t[1][0]).min(t[2][0]),
            acc.1.max(t[0][0]).max(t[1][0]).max(t[2][0]),
        )
    });
    let (lmin_y, lmax_y) = ground_lot.iter().fold((f32::MAX, f32::MIN), |acc, t| {
        (
            acc.0.min(t[0][1]).min(t[1][1]).min(t[2][1]),
            acc.1.max(t[0][1]).max(t[1][1]).max(t[2][1]),
        )
    });
    println!(
        "footprint AABB (lot space): x[{lmin_x:.2},{lmax_x:.2}] y[{lmin_y:.2},{lmax_y:.2}]  center=(({:.2}), ({:.2}))  (lot {lot_size:?})",
        (lmin_x + lmax_x) / 2.0,
        (lmin_y + lmax_y) / 2.0,
    );

    // ---- 4b. Unit 锚点位置（引擎地面 quad 以锚点包围盒为中心的候选基准）----
    let lot_units = assemble_units(&doc.properties);
    let mut unit_pts: Vec<[f32; 2]> = Vec::new();
    for unit in &lot_units.units {
        let matrix = match unit {
            sc_properties::LotUnit::Light { transform, .. }
            | sc_properties::LotUnit::Effect { transform, .. }
            | sc_properties::LotUnit::Decal { transform, .. }
            | sc_properties::LotUnit::Prop { transform, .. }
            | sc_properties::LotUnit::Spawner { transform, .. } => {
                transform.as_ref().map(|t| t.matrix)
            }
            _ => None,
        };
        if let Some(m) = matrix {
            unit_pts.push([
                m[0] * 0.0 + m[3] * 0.0 + m[6] * 0.0 + m[9],
                m[1] * 0.0 + m[4] * 0.0 + m[7] * 0.0 + m[10],
            ]);
        }
    }
    if !unit_pts.is_empty() {
        let (ux0, ux1) = unit_pts.iter().fold((f32::MAX, f32::MIN), |acc, p| {
            (acc.0.min(p[0]), acc.1.max(p[0]))
        });
        let (uy0, uy1) = unit_pts.iter().fold((f32::MAX, f32::MIN), |acc, p| {
            (acc.0.min(p[1]), acc.1.max(p[1]))
        });
        println!(
            "units: {} 个, AABB x[{ux0:.2},{ux1:.2}] y[{uy0:.2},{uy1:.2}] 中心=(({:.2}),({:.2}))",
            unit_pts.len(),
            (ux0 + ux1) / 2.0,
            (uy0 + uy1) / 2.0,
        );
    }

    // ---- 地面中心三方案对照（米，lot 空间）----
    // R = I 时：引擎 = bboxC + t；app 旧 = −t；app d313e11 = units 中心。
    let t = [placement[9], placement[10]];
    let engine_ground = [bbox_center[0] + t[0], bbox_center[1] + t[1]];
    let app_legacy = [-t[0], -t[1]];
    let units_center: Option<[f32; 2]> = if unit_pts.is_empty() {
        None
    } else {
        let (ux0, ux1) = unit_pts.iter().fold((f32::MAX, f32::MIN), |acc, p| {
            (acc.0.min(p[0]), acc.1.max(p[0]))
        });
        let (uy0, uy1) = unit_pts.iter().fold((f32::MAX, f32::MIN), |acc, p| {
            (acc.0.min(p[1]), acc.1.max(p[1]))
        });
        Some([(ux0 + ux1) / 2.0, (uy0 + uy1) / 2.0])
    };
    println!("== 地面中心对照 ==");
    println!(
        "  引擎   = ({:.2}, {:.2})   [M(bboxC), bboxC=({:.2},{:.2}) t=({:.2},{:.2})]",
        engine_ground[0], engine_ground[1], bbox_center[0], bbox_center[1], t[0], t[1]
    );
    println!(
        "  app 旧 = ({:.2}, {:.2})   [M^-1(0) = -t]",
        app_legacy[0], app_legacy[1]
    );
    match units_center {
        Some(c) => println!(
            "  app 新 = ({:.2}, {:.2})   [units 中心, d313e11]",
            c[0], c[1]
        ),
        None => println!("  app 新 = (无单元, 回退原点)"),
    }
    let err = [
        engine_ground[0] - app_legacy[0],
        engine_ground[1] - app_legacy[1],
    ];
    println!(
        "  app 旧误差 = ({:.2}, {:.2}) m",
        err[0], err[1]
    );
    if let Some(c) = units_center {
        println!(
            "  app 新误差 = ({:.2}, {:.2}) m",
            engine_ground[0] - c[0],
            engine_ground[1] - c[1]
        );
    }

    // ---- 5. 渲染：lot 平色合成 + 脚印叠加 ----
    let (rw, rh) = (mw * 4, mh * 4);
    let mut img = vec![255u8; rw * rh * 3];
    // lot 平色底：按 0.5 阈值的四色平涂（R/G/B/A = LotColor1-4 sRGB 近似）
    let palette: [[u8; 3]; 4] = [
        [96, 106, 130],   // R 沥青 蓝灰
        [166, 150, 120],  // G 铺装 棕
        [96, 148, 84],    // B 草坪 绿
        [226, 214, 96],   // A 标线 黄
    ];
    for py in 0..rh {
        for px in 0..rw {
            let mx = (px as f32 / rw as f32 * mw as f32) as usize % mw;
            let my = (py as f32 / rh as f32 * mh as f32) as usize % mh;
            let base = (my * mw + mx) * 4;
            let (mut r, mut g, mut b) = (255u8, 255u8, 255u8);
            for (c, color) in palette.iter().enumerate() {
                if mask[base + c] > 127 {
                    (r, g, b) = (color[0], color[1], color[2]);
                    break;
                }
            }
            let o = (py * rw + px) * 3;
            img[o] = r;
            img[o + 1] = g;
            img[o + 2] = b;
        }
    }
    // 脚印多边形：半透明品红填充 + 亮红描边
    let to_px = |lx: f32, ly: f32| -> (f32, f32) {
        (
            (lx + lot_size[0] / 2.0) / lot_size[0] * rw as f32,
            (ly + lot_size[1] / 2.0) / lot_size[1] * rh as f32,
        )
    };
    let mut coverage = vec![0u32; rw * rh];
    for t in &ground_lot {
        let (ax, ay) = to_px(t[0][0], t[0][1]);
        let (bx, by) = to_px(t[1][0], t[1][1]);
        let (cx, cy) = to_px(t[2][0], t[2][1]);
        let det = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay);
        if det.abs() < 1e-9 {
            continue;
        }
        let minx = max_i(0, (ax.min(bx).min(cx)) as i32);
        let maxx = min_i(rw as i32 - 1, (ax.max(bx).max(cx)) as i32);
        let miny = max_i(0, (ay.min(by).min(cy)) as i32);
        let maxy = min_i(rh as i32 - 1, (ay.max(by).max(cy)) as i32);
        for py in miny..=maxy {
            for px in minx..=maxx {
                let fx = px as f32 + 0.5;
                let fy = py as f32 + 0.5;
                let w0 = ((bx - fx) * (cy - fy) - (cx - fx) * (by - fy)) / det;
                let w1 = ((cx - fx) * (ay - fy) - (ax - fx) * (cy - fy)) / det;
                let w2 = 1.0 - w0 - w1;
                if w0 >= -0.001 && w1 >= -0.001 && w2 >= -0.001 {
                    coverage[(py as usize) * rw + px as usize] += 1;
                }
            }
        }
    }
    for (i, cover) in coverage.iter().enumerate() {
        if *cover > 0 {
            let o = i * 3;
            // 55% 品红混合
            img[o] = ((img[o] as u32 * 45 + 255u32 * 55) / 100) as u8;
            img[o + 1] = (img[o + 1] as u32 * 45 / 100) as u8;
            img[o + 2] = ((img[o + 2] as u32 * 45 + 255u32 * 55) / 100) as u8;
        }
    }
    // 描边：覆盖与非覆盖边界
    let mut outline = img.clone();
    // 地面 quad（引擎：中心 = 锚点 AABB 中心 ± LotSize/2）黄框
    if !unit_pts.is_empty() {
        let (ux0, ux1) = unit_pts.iter().fold((f32::MAX, f32::MIN), |acc, p| {
            (acc.0.min(p[0]), acc.1.max(p[0]))
        });
        let (uy0, uy1) = unit_pts.iter().fold((f32::MAX, f32::MIN), |acc, p| {
            (acc.0.min(p[1]), acc.1.max(p[1]))
        });
        let gcx = (ux0 + ux1) / 2.0 + overlay_offset[0];
        let gcy = (uy0 + uy1) / 2.0 + overlay_offset[1];
        let (gx0, gy0) = to_px(gcx - lot_size[0] / 2.0, gcy - lot_size[1] / 2.0);
        let (gx1, gy1) = to_px(gcx + lot_size[0] / 2.0, gcy + lot_size[1] / 2.0);
        let (gx0, gy0, gx1, gy1) =
            (gx0 as i32, gy0 as i32, gx1 as i32, gy1 as i32);
        for t in 0..3 {
            for x in (gx0 + t)..=(gx1 - t) {
                for y in [gy0 + t, gy1 - t] {
                    if x < 0 || y < 0 || x >= rw as i32 || y >= rh as i32 {
                        continue;
                    }
                    let o = ((y as usize) * rw + x as usize) * 3;
                    outline[o] = 250;
                    outline[o + 1] = 210;
                    outline[o + 2] = 60;
                }
            }
            for y in (gy0 + t)..=(gy1 - t) {
                for x in [gx0 + t, gx1 - t] {
                    if x < 0 || y < 0 || x >= rw as i32 || y >= rh as i32 {
                        continue;
                    }
                    let o = ((y as usize) * rw + x as usize) * 3;
                    outline[o] = 250;
                    outline[o + 1] = 210;
                    outline[o + 2] = 60;
                }
            }
        }
    }

    // Unit 锚点：青色 3px 方块 + AABB 青框
    for p in &unit_pts {
        let (cx, cy) = to_px(p[0], p[1]);
        let cx = cx as i32;
        let cy = cy as i32;
        for dy in -2..=2 {
            for dx in -2..=2 {
                let x = cx + dx;
                let y = cy + dy;
                if x < 0 || y < 0 || x >= rw as i32 || y >= rh as i32 {
                    continue;
                }
                let o = ((y as usize) * rw + x as usize) * 3;
                outline[o] = 0;
                outline[o + 1] = 230;
                outline[o + 2] = 230;
            }
        }
    }
    if !unit_pts.is_empty() {
        let (ux0, ux1) = unit_pts.iter().fold((f32::MAX, f32::MIN), |acc, p| {
            (acc.0.min(p[0]), acc.1.max(p[0]))
        });
        let (uy0, uy1) = unit_pts.iter().fold((f32::MAX, f32::MIN), |acc, p| {
            (acc.0.min(p[1]), acc.1.max(p[1]))
        });
        let (x0, y0) = to_px(ux0, uy0);
        let (x1, y1) = to_px(ux1, uy1);
        let (x0, y0, x1, y1) = (x0 as i32, y0 as i32, x1 as i32, y1 as i32);
        for t in 0..2 {
            for x in (x0 + t)..=(x1 + t) {
                for y in [y0 + t, y1 + t] {
                    if x < 0 || y < 0 || x >= rw as i32 || y >= rh as i32 {
                        continue;
                    }
                    let o = ((y as usize) * rw + x as usize) * 3;
                    outline[o] = 0;
                    outline[o + 1] = 200;
                    outline[o + 2] = 200;
                }
            }
            for y in (y0 + t)..=(y1 + t) {
                for x in [x0 + t, x1 + t] {
                    if x < 0 || y < 0 || x >= rw as i32 || y >= rh as i32 {
                        continue;
                    }
                    let o = ((y as usize) * rw + x as usize) * 3;
                    outline[o] = 0;
                    outline[o + 1] = 200;
                    outline[o + 2] = 200;
                }
            }
        }
    }

    for py in 1..rh - 1 {
        for px in 1..rw - 1 {
            let i = py * rw + px;
            if coverage[i] > 0
                && (coverage[i - 1] == 0
                    || coverage[i + 1] == 0
                    || coverage[i - rw] == 0
                    || coverage[i + rw] == 0)
            {
                let o = i * 3;
                outline[o] = 255;
                outline[o + 1] = 40;
                outline[o + 2] = 40;
            }
        }
    }
    image::save_buffer(
        format!("{out_dir}/footprint_overlay.png"),
        &outline,
        rw as u32,
        rh as u32,
        image::ColorType::Rgb8,
    )
    .expect("save overlay");
    println!("\nfootprint overlay -> {out_dir}/footprint_overlay.png ({rw}x{rh})");
}

fn max_i(a: i32, b: i32) -> i32 {
    a.max(b)
}
fn min_i(a: i32, b: i32) -> i32 {
    a.min(b)
}
