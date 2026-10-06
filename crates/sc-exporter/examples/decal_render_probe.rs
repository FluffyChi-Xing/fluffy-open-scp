//! decal 全量重渲染探针（引擎公式离线软件渲染，2026-10-01）。
//!
//! 背景：商业区招牌 = SDF 霓虹族——raster RGBA 四通道是 4 条灯管的距离场
//! （128=管心），可见图案由 decalAnimateSDFDarken 程序化算出，直接显示
//! 原图只能得到黑底+光球。本探针对全部字典条目做多管线并排渲染：
//!
//!   raw      原图直出（alpha 画到灰底上）
//!   sdf_on   SDF 点亮（decalAnimateSDFDarken：materialData 颜色 × 灯管光）
//!   sdf_off  SDF 熄灭（powerFactor 0.5，animResults=-1）
//!
//! 产物：tmp/dynamic/decal_render/{dict}/{index}_{id}.{variant}.png
//!       tmp/dynamic/decal_render/{dict}/tsv 清单（index/id/尺寸/变体）
//!
//! 用法：cargo run -p sc-exporter --release --example decal_render_probe -- \
//!     <pkg> [pkg...] [--dict group:inst ...] [--max N]

use dbpf::Package;
use sc_properties::decal::{DecalDictionary, DECAL_ATLAS_INSTANCE_TYPES};
use sc_properties::PROPERTY_RESOURCE_TYPE;
use std::collections::BTreeMap;

const RASTER_TYPE: u32 = 0x2F4E_681C;

struct Rgba {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dict_filter: Option<(u16, u32)> = None;
    let mut max_entries = 40usize;
    let mut random_n: Option<usize> = None;
    let mut ss_override: Option<u32> = None;
    let mut out_dir_override: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dict" => {
                i += 1;
                let parts: Vec<&str> = args.get(i).expect("--dict 需要值").split(':').collect();
                dict_filter = Some((
                    u16::from_str_radix(parts[0].trim_start_matches("0x"), 16).expect("group"),
                    u32::from_str_radix(parts[1].trim_start_matches("0x"), 16).expect("inst"),
                ));
            }
            "--max" => {
                i += 1;
                max_entries = args[i].parse().expect("--max N");
            }
            "--random" => {
                i += 1;
                random_n = Some(args[i].parse().expect("--random N"));
            }
            "--ss" => {
                i += 1;
                ss_override = Some(args[i].parse().expect("--ss N"));
            }
            "--out" => {
                i += 1;
                out_dir_override = Some(args[i].clone());
            }
            p => paths.push(p.to_string()),
        }
        i += 1;
    }
    assert!(!paths.is_empty(), "用法: decal_render_probe <pkg> [...] [--dict g:i] [--max N] [--random N] [--out dir]");
    let packages: Vec<Package> = paths.iter().map(|p| Package::open(p).expect("open")).collect();

    let out_dir = std::path::Path::new(
        out_dir_override.as_deref().unwrap_or("tmp/dynamic/decal_render"),
    );
    std::fs::create_dir_all(out_dir).unwrap();

    // 收集全部字典
    let mut dicts: BTreeMap<(u16, u32), DecalDictionary> = BTreeMap::new();
    for pkg in &packages {
        for e in pkg.entries() {
            if e.id.type_id != PROPERTY_RESOURCE_TYPE
                || !DECAL_ATLAS_INSTANCE_TYPES.contains(&(e.id.group as u16))
            {
                continue;
            }
            let key = (e.id.group as u16, e.id.instance);
            if dict_filter.as_ref().is_some_and(|f| f != &key) {
                continue;
            }
            if dicts.contains_key(&key) {
                continue;
            }
            if let Ok(bytes) = pkg.read(e) {
                if let Ok(dict) = DecalDictionary::parse(&bytes) {
                    dicts.insert(key, dict);
                }
            }
        }
    }
    println!("字典 {} 个", dicts.len());

    if let Some(n) = random_n {
        // 随机抽样：全部条目（含 raster 的）收集后洗牌取 n
        let mut pool: Vec<(&DecalDictionary, usize)> = Vec::new();
        for (_key, dict) in &dicts {
            for idx in 0..dict.entries.len() {
                pool.push((dict, idx));
            }
        }
        // LCG 洗牌（时间种子，可复现性不要求）
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as u64
            | 1;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        for i in (1..pool.len()).rev() {
            let j = next() % (i + 1);
            pool.swap(i, j);
        }
        let sample_dir = out_dir.join("sample");
        std::fs::create_dir_all(&sample_dir).unwrap();
        let mut rendered = 0usize;
        let mut seen_tgi = std::collections::HashSet::new();
        for (dict, idx) in pool {
            if rendered >= n {
                break;
            }
            let entry = &dict.entries[idx];
            let Some(raster_key) = entry.raster.clone() else { continue };
            let Some((bytes, _)) = find_raster(&packages, raster_key.instance) else { continue };
            let Ok(raster) = rw4::RasterImage::parse(&bytes) else { continue };
            let Ok(src) = raster.decode_top_mip_rgba() else { continue };
            let img = Rgba { w: raster.width, h: raster.height, px: src };
            let Some(colors) = &entry.colors_rgba8() else { continue };
            let mut rows = [[0f32; 4]; 4];
            for (ri, c) in colors.iter().enumerate() {
                rows[ri] = [
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    c[3] as f32 / 255.0,
                ];
            }
            let id = entry
                .id
                .map(|k| format!("{:08x}_{:08x}", k.group, k.instance))
                .unwrap_or_else(|| format!("idx{}", entry.index));
            if !seen_tgi.insert(id.clone()) {
                continue;
            }
            let ss = ss_override.unwrap_or(SS);
            let day = render_composite_ss(&img, &rows, false, ss);
            let night = render_composite_ss(&img, &rows, true, ss);
            save_png(&sample_dir.join(format!("{id}_day.png")), img.w * ss, img.h * ss, &day);
            save_png(&sample_dir.join(format!("{id}_night.png")), img.w * ss, img.h * ss, &night);
            println!("sample {rendered}: {id} {}x{}", img.w, img.h);
            rendered += 1;
        }
        println!("随机抽样 {rendered}/{n} → {}", sample_dir.display());
        return;
    }

    for ((group, inst), dict) in &dicts {
        let dict_dir = out_dir.join(format!("{group:04x}_{inst:08x}"));
        std::fs::create_dir_all(&dict_dir).unwrap();
        let material = dict.material.as_ref().map(|k| k.instance).unwrap_or(0);
        let mut tsv = format!("index\tid\tw\th\tvariants\n");
        let mut done = 0usize;
        for entry in &dict.entries {
            if done >= max_entries {
                break;
            }
            let Some(raster_key) = entry.raster.clone() else { continue };
            let Some((bytes, _)) = find_raster(&packages, raster_key.instance) else {
                continue;
            };
            let Ok(raster) = rw4::RasterImage::parse(&bytes) else { continue };
            let Ok(src) = raster.decode_top_mip_rgba() else { continue };
            let img = Rgba { w: raster.width, h: raster.height, px: src };
            let tgi = entry
                .id
                .map(|k| format!("{:08x}_{}_{}", k.type_id, k.group, k.instance))
                .unwrap_or_else(|| format!("idx{}", entry.index));
            let id = entry
                .id
                .map(|k| format!("{:08x}", k.instance))
                .unwrap_or_else(|| "none".into());
            let colors = &entry.colors;
            if std::env::var("PROBE_DEBUG")
                .map(|v| v == id)
                .unwrap_or(false)
            {
                println!("== DEBUG {id} ==");
                println!("  colors: {:?}", colors);
                let (cx, cy) = (img.w as usize / 2, img.h as usize / 2);
                for (label, (x, y)) in [
                    ("center", (cx, cy)),
                    ("mid-left", (w_center(img.w as usize, 8), cy)),
                    ("corner", (2, 2)),
                ] {
                    let i = (y * img.w as usize + x) * 4;
                    println!(
                        "  {label} ({x},{y}): R={} G={} B={} A={}",
                        img.px[i],
                        img.px[i + 1],
                        img.px[i + 2],
                        img.px[i + 3]
                    );
                }
            }

            // ---- 变体渲染：仅 tgi_day / tgi_night（清晰量化合成）----
            let mut variants: Vec<(String, Vec<u8>)> = Vec::new();
            {
                // 存储值 = 线性分量的一半（C# ScR/2 口径）→ 线性 = ×2
                let mut rows = [[0f32; 4]; 4];
                for (ri, c) in colors.iter().enumerate() {
                    if let Some(c) = c {
                        rows[ri] = [c[0] * 2.0, c[1] * 2.0, c[2] * 2.0, c[3] * 2.0];
                    }
                }
                variants.push((format!("{tgi}_day"), render_composite(&img, &rows, false)));
                variants.push((format!("{tgi}_night"), render_composite(&img, &rows, true)));
            }
            let mut names: Vec<String> = Vec::new();
            let ss = SS as u32;
            for (vname, px) in &variants {
                let f = dict_dir.join(format!("{}.png", vname));
                save_png(&f, img.w * ss, img.h * ss, px);
                names.push(vname.clone());
            }
            tsv.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                entry.index, id, img.w, img.h, names.join(",")
            ));
            done += 1;
        }
        std::fs::write(dict_dir.join("entries.tsv"), tsv).unwrap();
        println!(
            "字典 {group:04x}/{inst:08x}: material={material:08x} 条目 {done} → {}",
            dict_dir.display()
        );
    }
}

fn w_center(total: usize, inset: usize) -> usize {
    inset.min(total / 2)
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

fn save_png(path: &std::path::Path, w: u32, h: u32, px: &[u8]) {
    let mut img = image::RgbaImage::from_raw(w, h, px.to_vec()).expect("png buffer");
    let flipped = image::imageops::flip_vertical(&img);
    image::save_buffer(path, &flipped, w, h, image::ColorType::Rgba8).unwrap();
    img = flipped;
    let _ = img;
}

// ---------- 管线 1：原图 RGB 直出（alpha 无关——SDF 掩码图 alpha 常全零） ----------
fn render_raw(img: &Rgba) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.w * img.h * 4) as usize);
    for px in img.px.as_chunks::<4>().0 {
        out.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    out
}

// ---------- 管线 4：全合成 v3（印刷层常亮 + 掩码直叠 Color1-4） ----------
//
// 游戏实拍（昼/夜）：背光招牌的设计本体（红环/绿心/白字）昼夜清晰，
// 夜晚仅白色部分带 bloom（引擎后期辉光，逐像素合成不模拟）。
//   day:   base + Σ mask_k × Color(k+1).rgb × 0.3
//   night: base + Σ mask_k × Color(k+1).rgb × 0.9
fn render_composite(img: &Rgba, rows: &[[f32; 4]; 4], night: bool) -> Vec<u8> {
    render_composite_ss(img, rows, night, 4)
}

/// 清晰量化合成（升采样版）：引擎语义 = 双线性采样掩码后逐屏幕像素
/// 过阈值/优先级——阈值后置使边缘落在亚 texel 位置，锐利且平滑
/// （游戏"直线/斜线/曲线都锐利"的来源）；层颜色由优先级裁决，
/// 层间永不混色。
const SS: u32 = 4;

fn sample_bilinear(img: &Rgba, x: f32, y: f32) -> [f32; 4] {
    let w = img.w as i32;
    let h = img.h as i32;
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let clampi = |v: i32, hi: i32| v.clamp(0, hi);
    let xm = clampi(x0, w - 1);
    let xp = clampi(x0 + 1, w - 1);
    let ym = clampi(y0, h - 1);
    let yp = clampi(y0 + 1, h - 1);
    let g = |xx: i32, yy: i32| -> [f32; 4] {
        let i = (yy as usize * img.w as usize + xx as usize) * 4;
        [
            img.px[i] as f32 / 255.0,
            img.px[i + 1] as f32 / 255.0,
            img.px[i + 2] as f32 / 255.0,
            img.px[i + 3] as f32 / 255.0,
        ]
    };
    let c00 = g(xm, ym);
    let c10 = g(xp, ym);
    let c01 = g(xm, yp);
    let c11 = g(xp, yp);
    let mut out = [0f32; 4];
    for k in 0..4 {
        let top = c00[k] * (1.0 - fx) + c10[k] * fx;
        let bot = c01[k] * (1.0 - fx) + c11[k] * fx;
        out[k] = top * (1.0 - fy) + bot * fy;
    }
    out
}

fn render_composite_ss(img: &Rgba, rows: &[[f32; 4]; 4], night: bool, ss: u32) -> Vec<u8> {
    // 清晰量化合成：优先级 A > B > G > R，阈值 ≥128；命中层颜色 = Color 行（×2）
    let boost = if night { 1.25 } else { 1.0 };
    const ORDER: [(usize, usize); 4] = [(3, 3), (2, 2), (1, 1), (0, 0)];
    let ow = (img.w * ss) as usize;
    let oh = (img.h * ss) as usize;
    let mut out = Vec::with_capacity(ow * oh * 4);
    for oy in 0..oh {
        for ox in 0..ow {
            // 输出像素中心 → 源 texel 坐标（半像素对齐）
            let sx = (ox as f32 + 0.5) / ss as f32 - 0.5;
            let sy = (oy as f32 + 0.5) / ss as f32 - 0.5;
            let m = sample_bilinear(img, sx, sy);
            let mut hit = None;
            for (ch, row) in ORDER {
                if m[ch] >= 0.5 {
                    hit = Some(row);
                    break;
                }
            }
            match hit {
                Some(row) => {
                    for c in 0..3 {
                        out.push((rows[row][c] * boost * 255.0).clamp(0.0, 255.0) as u8);
                    }
                    out.push(255);
                }
                None => out.extend_from_slice(&[0, 0, 0, 0]),
            }
        }
    }
    out
}

/// 单通道灰度图（距离场校准用）。
fn render_channel(img: &Rgba, ch: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.w * img.h * 4) as usize);
    for px in img.px.as_chunks::<4>().0 {
        let v = px[ch];
        out.extend_from_slice(&[v, v, v, 255]);
    }
    out
}

// ---------- 管线 2/3：SDF 霓虹（decalAnimateSDFDarken 引擎公式） ----------
//
//   decalNUS          = (w, h, h)        （顶点 NUS：纹理尺寸；z 分量未知，取 h 首猜）
//   materialLightScale = info.x * 16 + 0.25
//   circleDists        = saturate(1 - ch/0.5) * hwRatio   （ch = 四通道距离场）
//   sphereDistsSqr     = circleDists² + circleZ²
//   lightScales        = saturate(1 - sqrt(sphereDistsSqr))²
//   lightColor[i]      = materialLightScale * dot(materialData[i], lightScales)
//
// power：点亮 1.0 / 熄灭 0.1（decalAnimateSDFDisabled 的 lightFactor 语义）
fn render_sdf(img: &Rgba, rows: &[[f32; 4]; 4], power: f32) -> Vec<u8> {
    let (w, h) = (img.w as f32, img.h as f32);
    let material_light_scale = 1.0 * 16.0 + 0.25; // info.x = 1（运行时捕获量级）
    let sdf_texture_length = w.max(h);
    let sphere_height = h;
    let mut hw_ratio = sphere_height * 0.5 / sdf_texture_length;
    let mut z_scale = 1.0f32;
    if hw_ratio < 1.0 {
        hw_ratio = 1.0;
        z_scale = 1.0 / hw_ratio;
    }
    let circle_z = 0.0f32 * z_scale; // 正面 z=0
    let k_mask_center = 0.5f32;

    let mut out = Vec::with_capacity((img.w * img.h * 4) as usize);
    for px in img.px.as_chunks::<4>().0 {
        let sdf = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
            px[3] as f32 / 255.0,
        ];
        // 四通道各自的灯管光标（引擎：lightScales = saturate(1-√(d²+z²))²）
        let mut light_scales = [0f32; 4];
        for (k, ch) in sdf.iter().enumerate() {
            let circle_dist = (1.0 - ch / k_mask_center).clamp(0.0, 1.0) * hw_ratio;
            let sphere_dists_sqr = circle_dist * circle_dist + circle_z * circle_z;
            let light = (1.0 - sphere_dists_sqr.sqrt()).clamp(0.0, 1.0);
            light_scales[k] = light * light;
        }
        // lightColor[i] = materialLightScale · dot(materialData[i], lightScales)
        // i=0..2 → RGB 三分量（颜色行 × 同一组光标的点积）
        let mut rgb = [0f32; 3];
        for ci in 0..3 {
            let dot: f32 = rows[ci].iter().zip(light_scales).map(|(c, l)| c * l).sum();
            rgb[ci] = material_light_scale * dot * power;
        }
        // 光晕区域加 alpha 轮廓（sdf 通道最大值）：便于在白底上判读
        let glow = (1.0 - sdf.iter().map(|v| (v - 0.5).abs()).fold(f32::MAX, f32::min))
            .clamp(0.0, 1.0);
        let a = (glow * 255.0) as u8;
        out.extend_from_slice(&[
            rgb[0].clamp(0.0, 255.0) as u8,
            rgb[1].clamp(0.0, 255.0) as u8,
            rgb[2].clamp(0.0, 255.0) as u8,
            a,
        ]);
    }
    out
}
