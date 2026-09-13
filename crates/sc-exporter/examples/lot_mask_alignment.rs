//! 冒烟工具：裁决 lot mask 与模型/prop 的坐标绑定方向（镜像取证）。
//!
//! 用法：cargo run -p sc-exporter --release --example lot_mask_alignment --
//!          <package> <model_instance_hex> [extra_package...]
//!
//! 原理：同一 raster 通道语义下，正确朝向时所有树 prop 采样到的 mask
//! 像素值应高度一致（都是草地通道）；错误镜像下采样值离散。
//! 同时输出建筑原点（placement 平移）在 mask 上的落点供人工比对。

use dbpf::Package;
use sc_properties::{assemble_units, LotEditorDocument, LotUnit, PropertyFile};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RASTER_TYPE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, rest) = args
        .split_first()
        .expect("usage: lot_mask_alignment <package> <model_instance> [extra...]");
    let model = rest.first().expect("need model instance");
    let extras = &rest[1..];
    let model_instance =
        u32::from_str_radix(model.trim_start_matches("0x"), 16).expect("instance");

    let package = Package::open(path).expect("open package");
    // 找到 LOD1 指向该模型的 property
    let mut found: Option<(dbpf::IndexEntry, LotEditorDocument)> = None;
    let mut fallback: Option<(dbpf::IndexEntry, LotEditorDocument)> = None;
    for entry in package.entries() {
        if entry.id.type_id != PROPERTY_TYPE {
            continue;
        }
        let raw = package.read(entry).expect("read property");
        let Ok(file) = PropertyFile::parse(&raw) else { continue };
        let doc = LotEditorDocument::from_property_file(file);
        if doc.model_lods.iter().flatten().any(|k| k.instance == model_instance) {
            // 同一模型可有多个 property 引用（含空壳变体），优先取带 mask 的
            if doc.lot_mask.is_some() || fallback.is_none() {
                if doc.lot_mask.is_some() {
                    found = Some((entry.clone(), doc));
                    break;
                }
                fallback = Some((entry.clone(), doc));
            }
        }
    }
    let Some((entry, _raw_doc)) = found.or(fallback) else {
        eprintln!("no property references model 0x{model_instance:08x}");
        std::process::exit(1);
    };
    println!(
        "lot property type={:08x} group={:08x} instance={:08x}",
        entry.id.type_id, entry.id.group, entry.id.instance
    );

    // Parent(0x00B2CCCB) 展平：78% 的 lot 变体把 LotSize / placement / Lot Textures
    // 挂在父级，只读本级会得到「空壳」——本探针此前正是如此，会误报 lot_size=None
    // 并退回 mask×0.75 的猜测尺寸，使 H0/H1/H2 判据建立在错的地块尺寸上。
    let extra_pkgs: Vec<Package> = extras
        .iter()
        .filter_map(|path| Package::open(path).ok())
        .collect();
    let raw_file = PropertyFile::parse(&package.read(&entry).expect("read property"))
        .expect("parse property");
    let resolve = |key: &sc_properties::Key| -> Option<PropertyFile> {
        for source in std::iter::once(&package).chain(extra_pkgs.iter()) {
            let mut hit = None;
            for candidate in source.entries() {
                if candidate.id.instance == key.instance
                    && (key.type_id == 0 || candidate.id.type_id == key.type_id)
                {
                    hit = Some(candidate.clone());
                    break;
                }
            }
            let Some(hit) = hit else { continue };
            let Ok(bytes) = source.read(&hit) else { continue };
            if let Ok(file) = PropertyFile::parse(&bytes) {
                return Some(file);
            }
        }
        None
    };
    let flattened = sc_properties::inherit::flatten_parent_inheritance(raw_file, resolve);
    let units = assemble_units(&flattened);
    let doc = LotEditorDocument::from_property_file(flattened);

    let mut size = doc.lot_size;
    println!(
        "lot_size = {size:?} m; placement = {:?}; lot_offset(0x0CCB7FC9) = {:?}",
        doc.placement.as_ref().map(|t| &t.matrix),
        doc.lot_offset
    );

    // 读 LotMask raw RGBA（本包 → 附加包）
    let Some(mask_key) = doc.lot_mask else {
        println!("no LotMask in property; props only");
        return;
    };
    println!("mask key = {:?}", mask_key);
    if let Some(tex) = doc.lot_textures {
        println!("lot_textures key = 0x{:08x}", tex.instance);
    } else {
        println!("lot_textures key = None");
    }
    let mut mask: Option<(Vec<u8>, usize)> = None;
    let search = |source: &Package, label: &str| -> Option<(Vec<u8>, usize)> {
        let hit = source
            .entries()
            .iter()
            .find(|e| e.id.instance == mask_key.instance && (mask_key.type_id == 0 || e.id.type_id == mask_key.type_id))
            .cloned()?;
        let raw = source.read(&hit).expect("read mask");
        let raster = rw4::RasterImage::parse(&raw).expect("parse mask raster");
        if !raster.is_raw_rgba() {
            eprintln!("mask pixel format {} not raw rgba", raster.pixel_format);
            std::process::exit(1);
        }
        let pixels = raster.decode_top_mip_rgba().expect("decode mask");
        let (w, h) = (raster.width as usize, raster.height as usize);
        println!("mask 0x{:08x} in {label}: {w}×{h} raw rgba", mask_key.instance);
        Some((pixels, w.max(h)))
    };
    mask = search(&package, "self");
    if mask.is_none() {
        for extra in extras {
            let source = Package::open(extra).expect("open extra package");
            mask = search(&source, "extra");
            if mask.is_some() {
                break;
            }
        }
    }
    let Some((mask, dims)) = mask else {
        eprintln!("mask raster not found");
        std::process::exit(1);
    };
    if size.is_none() {
        // EP1 系 lot 无 LotSize：mask 尺寸 × 0.75 m/px 回退（同后端逻辑）
        size = Some([dims as f32 * 0.75, mask.len() as f32 / 4.0 / dims as f32 * 0.75]);
        println!("lot_size fallback from mask: {:?}", size);
    }
    let size = size.unwrap();

    // 树 prop 的 lot 空间坐标（行主序平移 = 索引 9/10/11）
    let props: Vec<(f32, f32)> = units
        .units
        .iter()
        .filter_map(|unit| match unit {
            LotUnit::Prop { transform: Some(t), .. } => Some((t.matrix[9], t.matrix[10])),
            _ => None,
        })
        .collect();
    println!("props = {} (positions sampled)", props.len());
    for (index, &(x, y)) in props.iter().enumerate() {
        println!("  prop[{index:02}] lot-space ({x:8.3}, {y:8.3});");
    }

    let (mask_w, mask_h) = {
        let pixels = &mask;
        // dims 存的是 max 边；重新精确取宽高：用 RasterImage 再解析一次太贵，
        // 直接在查找闭包里返回 (w, h)。这里临时用 dims×dims 近似并修正：
        // —— 实际上方闭包返回了 (pixels, w) 而 h 需要单独带出。
        let _ = pixels;
        (dims, mask.len() / 4 / dims)
    };
    let sample = |x: f32, y: f32| -> [u8; 4] {
        let u = ((x + size[0] / 2.0) / size[0]).clamp(0.0, 0.999) * mask_w as f32;
        let v = ((y + size[1] / 2.0) / size[1]).clamp(0.0, 0.999) * mask_h as f32;
        let px = (u as usize + v as usize * mask_w) * 4;
        [mask[px], mask[px + 1], mask[px + 2], mask[px + 3]]
    };

    // ---- 客观判据：unitOffset(0x0CCB7FC9) 是否参与 mask 采样 ----
    // 引擎 FUN_006ea950（RVA 0x2EAAD4 簇，migration.md §42）：
    //   uv = (pos - unitOffset) / LotSize + 0.5
    // unitOffset 优先取 property 0x0CCB7FC9，缺失时回落该 lot 的 overlay box
    // 中心（运行时数据，离线不可得）。
    // 判据（沿用本探针既有思路）：正确的一侧应让所有 prop 落在**同一通道**，
    // 最大占比越高越对。
    let sample_with = |x: f32, y: f32, ox: f32, oy: f32, sx: f32, sy: f32| -> Option<[u8; 4]> {
        let fu = ((x - ox) + sx / 2.0) / sx;
        let fv = ((y - oy) + sy / 2.0) / sy;
        // 落在 mask 之外的不参与统计（此前 clamp 到边界会伪造出"全都有覆盖"
        // 的假象，把外溢的 prop 掺进直方图）。
        if !(0.0..1.0).contains(&fu) || !(0.0..1.0).contains(&fv) {
            return None;
        }
        let u = fu * mask_w as f32;
        let v = fv * mask_h as f32;
        let px = (u as usize + v as usize * mask_w) * 4;
        Some([mask[px], mask[px + 1], mask[px + 2], mask[px + 3]])
    };
    // 引擎优先级 w→z→y→x（A>B>G>R）→ 颜色下标 3>2>1>0
    let dominant = |px: [u8; 4]| -> Option<u8> {
        for (byte, color) in [(3usize, 3u8), (2, 2), (1, 1), (0, 0)] {
            if px[byte] > 127 {
                return Some(color);
            }
        }
        None
    };
    let report = |label: &str, ox: f32, oy: f32, sx: f32, sy: f32| {
        let mut hist = [0usize; 4];
        let mut outside = 0usize;
        let mut uncovered = 0usize;
        for &(x, y) in &props {
            match sample_with(x, y, ox, oy, sx, sy) {
                None => outside += 1,
                Some(px) => match dominant(px) {
                    Some(c) => hist[c as usize] += 1,
                    None => uncovered += 1,
                },
            }
        }
        let total = props.len().max(1);
        let inside = total - outside;
        let top = hist.iter().copied().max().unwrap_or(0);
        println!(
            "  [{label}] 出界={outside}/{total} 界内={inside} | LC1={} LC2={} LC3={} LC4={} 界内未覆盖={uncovered} → 界内最大占比 {:.0}%",
            hist[0],
            hist[1],
            hist[2],
            hist[3],
            top as f32 / inside.max(1) as f32 * 100.0
        );
    };
    println!("== unitOffset 客观判据（lot_offset = {:?}）", doc.lot_offset);
    report("H0 居中（现状）", 0.0, 0.0, size[0], size[1]);
    if let Some(off) = doc.lot_offset {
        report("H1 引擎 (pos - off)", off[0], off[1], size[0], size[1]);
        report("H2 反向 (pos + off)", -off[0], -off[1], size[0], size[1]);
    }
    // H4：地面尺度改用 mask 原生分辨率（1 m/px）而非 LotSize。若 LotSize 只是
    // UV 缩放遗产而真实地面取自更大的 overlay box，1 m/px 是它的常见近似。
    report(
        "H4 尺度=1 m/px",
        0.0,
        0.0,
        mask_w as f32,
        mask_h as f32,
    );

    // ---- 决定性数据：mask 覆盖区映射回模型空间 vs prop 实际范围 ----
    // 两者质心之差 = 要让 prop 落进覆盖区所需的平移量，同时直接给出「地面矩形该
    // 以什么为中心」。此前只统计 prop 侧，缺了 mask 侧的参照。
    {
        let (mw, mh) = (mask_w, mask_h);
        let (mut u0, mut u1, mut v0, mut v1) = (mw, 0usize, mh, 0usize);
        let (mut su, mut sv, mut n) = (0f64, 0f64, 0usize);
        for py in 0..mh {
            for px in 0..mw {
                let at = (py * mw + px) * 4;
                let covered = mask[at] > 127
                    || mask[at + 1] > 127
                    || mask[at + 2] > 127
                    || mask[at + 3] > 127;
                if !covered {
                    continue;
                }
                u0 = u0.min(px);
                u1 = u1.max(px);
                v0 = v0.min(py);
                v1 = v1.max(py);
                su += px as f64;
                sv += py as f64;
                n += 1;
            }
        }
        if n > 0 {
            // 像素 → 模型空间（H0 映射的逆）：x = (u/mw − 0.5) · LotSize.x
            let to_x = |px: f64| (px / mw as f64 - 0.5) * size[0] as f64;
            let to_y = |py: f64| (py / mh as f64 - 0.5) * size[1] as f64;
            let (mcx, mcy) = (to_x(su / n as f64), to_y(sv / n as f64));
            println!(
                "  mask 覆盖区 {n} px，像素 x[{u0},{u1}] y[{v0},{v1}] → 模型空间 x[{:.1},{:.1}] y[{:.1},{:.1}] 质心({mcx:.1},{mcy:.1})",
                to_x(u0 as f64),
                to_x(u1 as f64),
                to_y(v0 as f64),
                to_y(v1 as f64)
            );
            let cnt = props.len().max(1) as f64;
            let pcx = props.iter().map(|p| f64::from(p.0)).sum::<f64>() / cnt;
            let pcy = props.iter().map(|p| f64::from(p.1)).sum::<f64>() / cnt;
            println!(
                "  prop 质心({pcx:.1},{pcy:.1}) → 使 prop 对齐覆盖区所需 unitOffset ≈ ({:.1},{:.1})",
                pcx - mcx,
                pcy - mcy
            );
        } else {
            println!("  mask 无覆盖像素");
        }
    }

    // H3：把地面矩形改以「模型足迹中心」为中心 —— 引擎 unitOffset 缺失时回落
    // overlay box 中心的离线近似。若 prop 落回同一通道，说明修复方向是
    // 「地面矩形对齐模型足迹，而非以模型原点为中心」。
    {
        let model_key = doc.model_lods.iter().flatten().next().copied();
        let mut center: Option<(f32, f32)> = None;
        if let Some(key) = model_key {
            for source in std::iter::once(&package).chain(extra_pkgs.iter()) {
                let mut hit = None;
                for cand in source.entries() {
                    if cand.id.type_id == 0x2F4E_681B && cand.id.instance == key.instance {
                        hit = Some(cand.clone());
                        break;
                    }
                }
                let Some(hit) = hit else { continue };
                let Ok(bytes) = source.read(&hit) else { continue };
                let Ok(rw) = rw4::Rw4File::parse(&bytes) else { continue };
                let (mut minx, mut maxx, mut miny, mut maxy) =
                    (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
                let mut verts = 0usize;
                for sec in rw.sections_of_type(rw4::SectionType::MESH) {
                    let Ok(mesh) = rw.decode_mesh(&bytes, sec.number) else {
                        continue;
                    };
                    for v in &mesh.vertices {
                        let Some(p) = v.position() else { continue };
                        verts += 1;
                        minx = minx.min(p[0]);
                        maxx = maxx.max(p[0]);
                        miny = miny.min(p[1]);
                        maxy = maxy.max(p[1]);
                    }
                }
                if verts > 0 {
                    let c = ((minx + maxx) * 0.5, (miny + maxy) * 0.5);
                    println!(
                        "  model 0x{:08X}: verts={verts} footprint x[{minx:.1},{maxx:.1}] y[{miny:.1},{maxy:.1}] center=({:.1},{:.1})",
                        key.instance, c.0, c.1
                    );
                    center = Some(c);
                }
                break;
            }
        }
        match center {
            Some((cx, cy)) => report("H3 模型足迹中心", cx, cy, size[0], size[1]),
            None => println!("  [H3] 模型未在已开包中找到，跳过"),
        }
    }

    // 导出 mask 量化 PNG + prop 点标注（目视裁决方向用）
    {
        let (mw, mh) = (dims, mask.len() / 4 / dims);
        let mut pixels = vec![0u8; mw * mh * 4];
        for y in 0..mh {
            for x in 0..mw {
                let at = (y * mw + x) * 4;
                let a = mask[at + 3];
                let (r, g, b) = if a < 16 {
                    (40, 44, 52)
                } else if mask[at] > 127 {
                    (220, 60, 60)
                } else if mask[at + 1] > 127 {
                    (60, 200, 60)
                } else if mask[at + 2] > 127 {
                    (60, 60, 220)
                } else {
                    (200, 180, 60)
                };
                pixels[at..at + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
        for (px, py) in props.iter().map(|(x, y)| {
            (
                (((x + size[0] / 2.0) / size[0]).clamp(0.0, 0.999) * mw as f32) as u32,
                (((y + size[1] / 2.0) / size[1]).clamp(0.0, 0.999) * mh as f32) as u32,
            )
        }) {
            for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    let xx = px as i32 + dx;
                    let yy = py as i32 + dy;
                    if xx < 0 || yy < 0 || xx >= mw as i32 || yy >= mh as i32 {
                        continue;
                    }
                    let at = ((yy as usize) * mw + xx as usize) * 4;
                    pixels[at..at + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
        let image = image::RgbaImage::from_raw(mw as u32, mh as u32, pixels).unwrap();
        let out = format!("lot_mask_0x{:08x}.png", mask_key.instance);
        image.save(&out).expect("save mask png");
        println!("mask visualization saved: {out} (white dots = props; red=R green=G blue=B)");
    }

    // 地块四象限 + 中心采样（识别停车场地块在哪一侧）
    for (label, x, y) in [
        ("center", 0.0f32, 0.0f32),
        ("north (+Y)", 0.0, size[1] * 0.35),
        ("south (-Y)", 0.0, -size[1] * 0.35),
        ("east (+X)", size[0] * 0.35, 0.0),
        ("west (-X)", -size[0] * 0.35, 0.0),
    ] {
        let s0 = sample(x, y);
        let s1 = sample(-x, y);
        let s2 = sample(x, -y);
        println!(
            "  {label:10} id={:02x}{:02x}{:02x} flipX={:02x}{:02x}{:02x} flipY={:02x}{:02x}{:02x}",
            s0[0], s0[1], s0[2], s1[0], s1[1], s1[2], s2[0], s2[1], s2[2]
        );
    }

    // 4 种朝向：prop→mask 像素映射的镜像组合
    for (name, fx, fy) in [
        ("identity ", 1, 1),
        ("flipX    ", -1, 1),
        ("flipY    ", 1, -1),
        ("flipXY   ", -1, -1),
    ] {
        let mut samples = Vec::new();
        for &(x, y) in &props {
            samples.push(sample(fx as f32 * x, fy as f32 * y));
        }
        // 一致性得分：每通道的极差（max-min）之和，越小越一致
        let spread: u32 = (0..4)
            .map(|c| {
                let vals: Vec<u8> = samples.iter().map(|s| s[c]).collect();
                (*vals.iter().max().unwrap() as u32) - (*vals.iter().min().unwrap() as u32)
            })
            .sum();
        let preview: Vec<String> = samples
            .iter()
            .take(8)
            .map(|s| format!("{:02x}{:02x}{:02x}{:02x}", s[0], s[1], s[2], s[3]))
            .collect();
        // 东列（树）与西列（密集点）分组通道统计
        let east: Vec<&[u8; 4]> = samples.iter().skip(55).collect();
        let west: Vec<&[u8; 4]> = samples.iter().take(15).collect();
        let dominant = |set: &[&[u8; 4]]| -> String {
            let names = ["R", "G", "B", "A"];
            (0..4)
                .map(|c| {
                    let hit = set.iter().filter(|s| s[c] > 0x80).count();
                    format!("{}:{}/{}", names[c], hit, set.len())
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!("[{name}] spread={spread:4}  head={:?}", preview);
        println!("         west(col0-14) [{}]  east(col55+) [{}]", dominant(&west), dominant(&east));
    }

    // 建筑原点在 mask 上的落点（4 种朝向）
    if let Some(placement) = &doc.placement {
        let (px, py) = (placement.matrix[9], placement.matrix[10]);
        println!("building origin lot-space = ({px:.1}, {py:.1})");
        for (name, fx, fy) in [
            ("identity", 1, 1),
            ("flipX", -1, 1),
            ("flipY", 1, -1),
            ("flipXY", -1, -1),
        ] {
            let s = sample(fx as f32 * px, fy as f32 * py);
            println!("  [{name}] mask at building = {:02x}{:02x}{:02x}{:02x}", s[0], s[1], s[2], s[3]);
        }
    }
}
