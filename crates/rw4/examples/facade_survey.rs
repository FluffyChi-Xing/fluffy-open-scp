//! 立面窗特征普查（只读）：遍历一个包内全部 RW4 模型容器，对每栋建筑按
//! "实际被顶点使用的材质块"统计 Base(row1)/Top(row2) 矩形内的窗标记占比，
//! 分类为：玻璃/Base 窗型（slot3 Base A<128 明显）、Top 窗型（仅 Top 有）、
//! 无窗型、无 facade 链（无 slot0 参数表）。
//!
//! 判定阈值（来自 0xF8FFC5F8 实证标定）：slot3 矩形内 A<128 占比 ≥2% 记
//! "该域有窗"；tint A<128 ≥2% 记镂空。
//!
//! 用法：cargo run -p rw4 --release --example facade_survey -- <package> [跨包...]

const RW4_IMAGE: u32 = 0x2F4E_681B;
const RASTER_IMAGE: u32 = 0x2F4E_681C;

const THRESH: f32 = 0.02; // 矩形内 A<128 占比阈值

struct Ctx<'a> {
    packages: Vec<(String, &'a dbpf::Package)>,
}

impl Ctx<'_> {
    fn find_rgba(&self, inst: u32) -> Option<(Vec<u8>, u32, u32)> {
        for (_, pkg) in &self.packages {
            let Some(e) = pkg
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && (e.id.type_id == RASTER_IMAGE || e.id.type_id == RW4_IMAGE))
                .cloned()
            else {
                continue;
            };
            let bytes = pkg.read(&e).ok()?;
            if e.id.type_id == RASTER_IMAGE {
                let raster = rw4::RasterImage::parse(&bytes).ok()?;
                return Some((raster.decode_top_mip_rgba().ok()?, raster.width, raster.height));
            }
            let tex_file = rw4::Rw4File::parse(&bytes).ok()?;
            let sec = tex_file.sections_of_type(rw4::SectionType::TEXTURE).next()?.number;
            let tex = tex_file.decode_texture(&bytes, sec).ok()?;
            return Some((tex.decode_top_mip_rgba().ok()?, u32::from(tex.width), u32::from(tex.height)));
        }
        None
    }

    /// slot0 参数表（f32 texel 平铺 + 列数）
    fn find_params(&self, inst: u32) -> Option<(Vec<[f32; 4]>, usize)> {
        for (_, pkg) in &self.packages {
            let Some(e) = pkg.entries().iter().find(|e| e.id.instance == inst && e.id.type_id == RW4_IMAGE).cloned() else {
                continue;
            };
            let bytes = pkg.read(&e).ok()?;
            let tex_file = rw4::Rw4File::parse(&bytes).ok()?;
            let sec = tex_file.sections_of_type(rw4::SectionType::TEXTURE).next()?.number;
            let tex = tex_file.decode_texture(&bytes, sec).ok()?;
            let pixels = tex.decode_palette_f32().ok()?;
            return Some((pixels, usize::from(tex.width)));
        }
        None
    }
}

/// 矩形内 A<128 占比（返回 None = 矩形无效/空）
fn a_low_ratio(rgba: &[u8], w: u32, h: u32, xform: [f32; 4]) -> Option<f32> {
    if !(xform[0] > 0.0 && xform[1] > 0.0) {
        return None;
    }
    let (w, h) = (w as usize, h as usize);
    let x0 = (xform[2].clamp(0.0, 1.0) * (w - 1) as f32) as usize;
    let y0 = (xform[3].clamp(0.0, 1.0) * (h - 1) as f32) as usize;
    let x1 = (((xform[2] + xform[0]).clamp(0.0, 1.0) * (w - 1) as f32) as usize).min(w - 1);
    let y1 = (((xform[3] + xform[1]).clamp(0.0, 1.0) * (h - 1) as f32) as usize).min(h - 1);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (mut lo, mut n) = (0usize, 0usize);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let a = rgba[(y * w + x) * 4 + 3];
            if a < 128 {
                lo += 1;
            }
            n += 1;
        }
    }
    (n > 0).then(|| lo as f32 / n as f32)
}

#[derive(Default)]
struct ModelVerdict {
    /// 有 facade 链（slot0 参数表 + slot1 + slot3）
    facade: bool,
    /// 顶点含 FLOAT4 TexCoord（uv2）
    uv2: bool,
    /// 使用中的材质块数 / Base 有窗 / Top 有窗 / tint Base 镂空 / tint Top 镂空
    blocks: usize,
    base_win: usize,
    top_win: usize,
    base_hole: usize,
    top_hole: usize,
    /// 按顶点数加权：Base 窗块顶点占比 / Top 窗块顶点占比
    base_win_verts: usize,
    top_win_verts: usize,
    used_verts: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, extras) = args.split_first().expect("usage: <package> [跨包...]");
    let package = dbpf::Package::open(path).expect("open package");
    let extra_pkgs: Vec<_> = extras.iter().map(|p| dbpf::Package::open(p).expect("open extra")).collect();
    let mut ctx = Ctx {
        packages: vec![(path.clone(), &package)],
    };
    for (i, p) in extra_pkgs.iter().enumerate() {
        ctx.packages.push((extras[i].clone(), p));
    }

    let models: Vec<_> = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == RW4_IMAGE)
        .cloned()
        .collect();
    eprintln!("{} 个 RW4 资源待扫描", models.len());

    let mut n_facade = 0;
    let mut n_base = 0;
    let mut n_top = 0;
    let mut n_both = 0;
    let mut n_none = 0;
    let mut n_uv2_of_facade = 0;
    let mut n_no_uv2_but_topwin = 0;
    let mut processed = 0;
    let mut rows: Vec<String> = Vec::new();

    for entry in &models {
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
        let mut v = ModelVerdict::default();

        // 收集所有 exportable mesh 的 materialIndex 使用计数 + uv2 存在性
        let mut used: std::collections::HashMap<u8, usize> = Default::default();
        for section in file.sections_of_type(rw4::SectionType::MESH) {
            let Ok(mesh) = file.decode_mesh(&data, section.number) else { continue };
            if !mesh.is_exportable() {
                continue;
            }
            for vtx in &mesh.vertices {
                if let Some(m) = vtx.d3d_color_g() {
                    *used.entry(m).or_default() += 1;
                }
                if vtx.components.iter().any(|(e, val)| {
                    e.usage == rw4::DeclarationUsage::TexCoord && matches!(val, rw4::ComponentValue::Float4(_))
                }) {
                    v.uv2 = true;
                }
            }
            v.used_verts += mesh.vertices.len();
        }

        for material_section in file.sections_of_type(rw4::SectionType::MATERIAL) {
            let Ok(rw4::MaterialSection::Decoded(mat)) = file.decode_material(&data, material_section.number)
            else {
                continue;
            };
            let Some((params, cols)) = mat.slot_texture(0).and_then(|i| ctx.find_params(i)) else {
                continue;
            };
            if cols == 0 || params.len() < cols * 4 {
                continue;
            }
            let Some((tint, tw, th)) = mat.slot_texture(1).and_then(|i| ctx.find_rgba(i)) else {
                continue;
            };
            let Some((shade, sw, sh)) = mat.slot_texture(3).and_then(|i| ctx.find_rgba(i)) else {
                continue;
            };
            v.facade = true;
            // 列布局：材质 m row k = flat[k*cols + m]
            let row = |m: u8, k: usize| params.get(k * cols + m as usize).copied();
            for (&m, &verts) in &used {
                let m = m as usize;
                if m >= cols {
                    continue;
                }
                v.blocks += 1;
                let base = row(m as u8, 1).unwrap_or([0.0; 4]);
                let top = row(m as u8, 2).unwrap_or([0.0; 4]);
                let bw = a_low_ratio(&shade, sw, sh, base).unwrap_or(0.0);
                let tw_ = a_low_ratio(&shade, sw, sh, top).unwrap_or(0.0);
                let bh = a_low_ratio(&tint, tw, th, base).unwrap_or(0.0);
                let th_ = a_low_ratio(&tint, tw, th, top).unwrap_or(0.0);
                if bw >= THRESH {
                    v.base_win += 1;
                    v.base_win_verts += verts;
                }
                if tw_ >= THRESH {
                    v.top_win += 1;
                    v.top_win_verts += verts;
                }
                if bh >= THRESH {
                    v.base_hole += 1;
                }
                if th_ >= THRESH {
                    v.top_hole += 1;
                }
            }
        }

        processed += 1;
        if v.facade {
            n_facade += 1;
            let base = v.base_win > 0;
            let top = v.top_win > 0;
            match (base, top) {
                (true, true) => n_both += 1,
                (true, false) => n_base += 1,
                (false, true) => n_top += 1,
                (false, false) => n_none += 1,
            }
            if v.uv2 {
                n_uv2_of_facade += 1;
            }
            if top && !v.uv2 {
                n_no_uv2_but_topwin += 1;
            }
            rows.push(format!(
                "0x{:08X} blocks={:<3} base窗={:<3} top窗={:<3} base镂={:<3} top镂={:<3} 顶点加权 base={:>3}% top={:>3}% uv2={}",
                entry.id.instance,
                v.blocks,
                v.base_win,
                v.top_win,
                v.base_hole,
                v.top_hole,
                if v.used_verts > 0 { v.base_win_verts * 100 / v.used_verts } else { 0 },
                if v.used_verts > 0 { v.top_win_verts * 100 / v.used_verts } else { 0 },
                v.uv2 as u8,
            ));
        }
        if processed % 25 == 0 {
            eprintln!(".. {}/{}（facade {}）", processed, models.len(), n_facade);
        }
    }

    for r in &rows {
        println!("{r}");
    }
    println!(
        "\n==== 汇总（{processed} 个模型解析，{n_facade} 个含 facade 链）====\n\
         仅 Base 有窗（玻璃/幕墙型）: {n_base}\n\
         仅 Top 有窗（motif 平铺型）: {n_top}\n\
         Base+Top 都有窗:            {n_both}\n\
         两域均无窗:                 {n_none}\n\
         facade 且含 uv2(FLOAT4):    {n_uv2_of_facade}\n\
         Top 有窗但无 uv2（异常）:   {n_no_uv2_but_topwin}"
    );
}
