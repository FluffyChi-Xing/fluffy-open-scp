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
    let Some((entry, doc)) = found.or(fallback) else {
        eprintln!("no property references model 0x{model_instance:08x}");
        std::process::exit(1);
    };
    println!(
        "lot property type={:08x} group={:08x} instance={:08x}",
        entry.id.type_id, entry.id.group, entry.id.instance
    );

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
    let units = assemble_units(&PropertyFile::parse(&package.read(&entry).unwrap()).unwrap());
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
