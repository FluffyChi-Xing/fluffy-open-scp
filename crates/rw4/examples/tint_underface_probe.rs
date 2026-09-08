//! 下向面（地板/底面）tint alpha 取证探针（2026-09-08，只读不改）：
//! 症状：白模从下往上看正常，渲染后部分地板面消失可透视模型内部（游戏同症状）。
//! 假设：building4Clip 的 alpha 镂空（tint.a<0.5 discard）在下向面的 UV 落点上成立。
//! 方法：逐三角形按面法线分桶（Z-up：down<-0.5 / up>0.5 / side），
//!       顶点级采样 baseUv=frac(uv4.xy)*xform.xy+xform.zw（xform=params row1）→ slot1 tint alpha。
//!
//! 用法：cargo run -p rw4 --release --example tint_underface_probe -- <package> 0xMODEL [跨包...]

const RW4_IMAGE: u32 = 0x2F4E_681B;
const RASTER_IMAGE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, rest) = args.split_first().expect("usage: <package> 0xMODEL [extra...]");
    let (model, extra) = rest.split_first().expect("missing 0xMODEL");
    let instance = u32::from_str_radix(model.trim_start_matches("0x"), 16).expect("instance");

    let package = dbpf::Package::open(path).expect("open package");
    let extras: Vec<_> = extra
        .iter()
        .map(|p| dbpf::Package::open(p).expect("open extra package"))
        .collect();
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == RW4_IMAGE && e.id.instance == instance)
        .cloned()
        .expect("model not found in main package");
    let data = package.read(&entry).expect("read model");
    let file = rw4::Rw4File::parse(&data).expect("parse rw4");
    let bindings = file.decode_mesh_material_bindings(&data);

    let find_rgba = |inst: u32| -> Option<(Vec<u8>, u32, u32)> {
        for pkg in std::iter::once(&package).chain(extras.iter()) {
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
                let rgba = raster.decode_top_mip_rgba().ok()?;
                return Some((rgba, raster.width, raster.height));
            }
            let tex_file = rw4::Rw4File::parse(&bytes).ok()?;
            let sec = tex_file.sections_of_type(rw4::SectionType::TEXTURE).next()?.number;
            let tex = tex_file.decode_texture(&bytes, sec).ok()?;
            let rgba = tex.decode_top_mip_rgba().ok()?;
            return Some((rgba, u32::from(tex.width), u32::from(tex.height)));
        }
        None
    };
    let find_params = |inst: u32| -> Option<Vec<[f32; 4]>> {
        for pkg in std::iter::once(&package).chain(extras.iter()) {
            let Some(e) = pkg
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && e.id.type_id == RW4_IMAGE)
                .cloned()
            else {
                continue;
            };
            let bytes = pkg.read(&e).ok()?;
            let tex_file = rw4::Rw4File::parse(&bytes).ok()?;
            let Some(sec) = tex_file.sections_of_type(rw4::SectionType::TEXTURE).next() else {
                continue;
            };
            let tex = tex_file.decode_texture(&bytes, sec.number).ok()?;
            if let Ok(pixels) = tex.decode_palette_f32() {
                return Some(pixels);
            }
        }
        None
    };

    struct Mat {
        xform: [f32; 4],
        tint: Option<(Vec<u8>, u32, u32)>,
    }
    let mut mats: std::collections::BTreeMap<u32, Mat> = std::collections::BTreeMap::new();
    for b in &bindings {
        if mats.contains_key(&b.material_section) {
            continue;
        }
        let mut xform = [1.0f32, 1.0, 0.0, 0.0];
        let mut tint = None;
        if let Ok(rw4::MaterialSection::Decoded(mat)) = file.decode_material(&data, b.material_section) {
            if let Some(inst) = mat.slot_texture(0) {
                if let Some(rows) = find_params(inst) {
                    // 源码 row1=regionXform（row0=palU/palU2/intScale/intOffset）
                    if let Some(r) = rows.get(1) {
                        xform = [r[0], r[1], r[2], r[3]];
                    }
                }
            }
            if let Some(inst) = mat.slot_texture(1) {
                tint = find_rgba(inst);
            }
        }
        mats.insert(b.material_section, Mat { xform, tint });
    }

    #[derive(Clone, Copy, Default)]
    struct Bucket {
        verts: usize,
        discarded: usize,
        tris: usize,
        tris_full_discard: usize,
        /// 三角形内部 >80% 像素被丢弃（视觉上整面消失）
        tris_vanished: usize,
        /// 三角形内部可见率 <20%
        tris_gone: usize,
    }
    let mut buckets: [Bucket; 3] = [Bucket::default(); 3]; // 0=down 1=up 2=side
    let mut uv_discarded_down: Vec<[f32; 4]> = Vec::new(); // 原始 Float4.xy
    let mut sample_discarded_down: Vec<[f32; 2]> = Vec::new(); // baseUv 落点

    // tint 图整体 alpha 覆盖率（含窗口矩形内）
    let mut tex_alpha_cov = 0.0f32;
    let mut win_alpha_cov = 0.0f32;
    {
        let mut any_tex = false;
        for mat in mats.values() {
            let Some((rgba, w, h)) = mat.tint.as_ref() else { continue };
            let (w, h) = (*w as usize, *h as usize);
            let mut op = 0usize;
            for iy in 0..h {
                for ix in 0..w {
                    if rgba[(iy * w + ix) * 4 + 3] >= 128 {
                        op += 1;
                    }
                }
            }
            tex_alpha_cov = op as f32 / (w * h) as f32;
            // 窗口矩形 [zw, zw+xy]
            let (x0, y0) = ((mat.xform[2].clamp(0.0, 1.0) * (w - 1) as f32) as usize, (mat.xform[3].clamp(0.0, 1.0) * (h - 1) as f32) as usize);
            let (x1, y1) = ((mat.xform[2] + mat.xform[0]).clamp(0.0, 1.0) * (w - 1) as f32, (mat.xform[3] + mat.xform[1]).clamp(0.0, 1.0) * (h - 1) as f32);
            let (x1, y1) = ((x1 as usize).min(w - 1), (y1 as usize).min(h - 1));
            let mut opw = 0usize;
            for iy in y0..=y1 {
                for ix in x0..=x1 {
                    if rgba[(iy * w + ix) * 4 + 3] >= 128 {
                        opw += 1;
                    }
                }
            }
            win_alpha_cov = opw as f32 / ((x1 - x0 + 1) * (y1 - y0 + 1)) as f32;
            any_tex = true;
            break; // 单材质模型足够；多材质取首个有 tint 的
        }
        if !any_tex {
            println!("（无 slot1 tint 纹理，跳过覆盖率统计）");
        }
    }

    println!("model 0x{instance:08X}  meshes={} bindings={}", {
        file.sections_of_type(rw4::SectionType::MESH).count()
    }, bindings.len());

    for b in &bindings {
        let Some(mat) = mats.get(&b.material_section) else { continue };
        let Some(mesh) = file.decode_mesh(&data, b.mesh_section).ok() else { continue };
        if !mesh.is_exportable() {
            continue;
        }
        let sample_alpha = |uv: [f32; 2]| -> Option<f32> {
            let (rgba, w, h) = mat.tint.as_ref()?;
            let (w, h) = (*w as usize, *h as usize);
            if w == 0 || h == 0 {
                return None;
            }
            let fx = uv[0].clamp(0.0, 1.0) * (w - 1) as f32;
            let fy = uv[1].clamp(0.0, 1.0) * (h - 1) as f32;
            let ix = (fx as usize).min(w - 1);
            let iy = (fy as usize).min(h - 1);
            Some(rgba[(iy * w + ix) * 4 + 3] as f32 / 255.0)
        };
        let pos = |i: usize| -> Option<[f32; 3]> { mesh.vertices.get(i)?.position() };
        let uv4 = |i: usize| -> Option<[f32; 4]> {
            mesh.vertices.get(i)?.components.iter().find_map(|(e, v)| {
                (e.usage == rw4::DeclarationUsage::TexCoord)
                    .then_some(())
                    .and_then(|_| match v {
                        rw4::ComponentValue::Float4(f) => Some(*f),
                        _ => None,
                    })
            })
        };

        for tri in &mesh.triangles {
            let (Some(p0), Some(p1), Some(p2)) = (pos(tri[0] as usize), pos(tri[1] as usize), pos(tri[2] as usize))
            else {
                continue;
            };
            let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let nz = if len > 1e-9 { n[2] / len } else { 0.0 };
            let bi = if nz < -0.5 {
                0
            } else if nz > 0.5 {
                1
            } else {
                2
            };
            buckets[bi].tris += 1;
            let mut tri_discard = 0usize;
            // 三角形内部重心网格采样：模拟逐像素 discard 的可见率
            let uvs: Vec<Option<[f32; 4]>> = (0..3).map(|k| uv4(tri[k] as usize)).collect();
            if uvs.iter().all(|u| u.is_some()) {
                let (ua, ub, uc) = (uvs[0].unwrap(), uvs[1].unwrap(), uvs[2].unwrap());
                const N: usize = 12;
                let mut pts = 0usize;
                let mut hit = 0usize;
                for iy in 0..N {
                    for ix in 0..N {
                        let b0 = (ix as f32 + 0.5) / N as f32;
                        let b1 = (iy as f32 + 0.5) / N as f32;
                        if b0 + b1 > 1.0 {
                            continue;
                        }
                        let b2 = 1.0 - b0 - b1;
                        let fu = [
                            ua[0] * b0 + ub[0] * b1 + uc[0] * b2,
                            ua[1] * b0 + ub[1] * b1 + uc[1] * b2,
                        ];
                        let base = [
                            fu[0].fract().rem_euclid(1.0) * mat.xform[0] + mat.xform[2],
                            fu[1].fract().rem_euclid(1.0) * mat.xform[1] + mat.xform[3],
                        ];
                        pts += 1;
                        if let Some(a) = sample_alpha(base) {
                            if a < 0.5 {
                                hit += 1;
                            }
                        }
                    }
                }
                if pts > 0 {
                    let cov = hit as f32 / pts as f32;
                    if cov > 0.8 {
                        buckets[bi].tris_vanished += 1;
                    }
                    if 1.0 - cov < 0.2 {
                        buckets[bi].tris_gone += 1;
                    }
                }
            }
            for k in 0..3 {
                let vi = tri[k] as usize;
                buckets[bi].verts += 1;
                if let Some(f) = uv4(vi) {
                    let base = [
                        f[0].fract().rem_euclid(1.0) * mat.xform[0] + mat.xform[2],
                        f[1].fract().rem_euclid(1.0) * mat.xform[1] + mat.xform[3],
                    ];
                    if let Some(a) = sample_alpha(base) {
                        if a < 0.5 {
                            buckets[bi].discarded += 1;
                            tri_discard += 1;
                            if bi == 0 && uv_discarded_down.len() < 24 {
                                uv_discarded_down.push([f[0], f[1], mat.xform[0], mat.xform[1]]);
                                sample_discarded_down.push(base);
                            }
                        }
                    }
                }
            }
            if tri_discard == 3 {
                buckets[bi].tris_full_discard += 1;
            }
        }
    }

    let names = ["down(法线z<-0.5)", "up  (法线z> 0.5)", "side"];
    println!(
        "tint 图整体 alpha≥0.5 覆盖率 {:.1}% ｜ 窗口矩形内 {:.1}%",
        tex_alpha_cov * 100.0,
        win_alpha_cov * 100.0
    );
    println!("\n=== 顶点级 tint alpha<0.5 丢弃率（按面朝向）===");
    for (i, b) in buckets.iter().enumerate() {
        let pct = if b.verts > 0 { b.discarded as f32 / b.verts as f32 * 100.0 } else { 0.0 };
        println!(
            "{}: 顶点 {}/{} ({pct:.1}%)  三角 {}（全丢弃 {}，内部>80%丢弃 {}）",
            names[i], b.discarded, b.verts, b.tris, b.tris_full_discard, b.tris_vanished
        );
    }
    println!("\n=== 被丢弃的 down 顶点样例（uv.xy | xform.xy | baseUv）===");
    for (i, uv) in uv_discarded_down.iter().enumerate() {
        let s = sample_discarded_down[i];
        println!("  uv=({:>9.3},{:>9.3})  xform=({:.3},{:.3})  base=({:.3},{:.3})", uv[0], uv[1], uv[2], uv[3], s[0], s[1]);
    }
}
