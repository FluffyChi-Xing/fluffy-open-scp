//! 探针：对一栋建筑的材质链逐槽导出，并按 building4 公式做**平面（UV 域）合成预览**。
//!
//! 用法：
//! ```text
//! cargo run -p sc-exporter --release --example building_material_probe -- \
//!   <model_package> <model_instance_hex> --out=<dir> [--col=N] [lookup_package...]
//! ```
//!
//! 链路公式逐字对齐前端 `refinedRender.ts` 注入的 fragment（building4）：
//! ```text
//! baseUv   = fract(vTintUv) * regionXform.xy + regionXform.zw        // regionXform = row1
//! topUv    = fract(vTintUv) * regionXform2.xy + regionXform2.zw      // regionXform2 = row2
//! scFacade = tint@topUv.a                                            // Top 层覆盖率
//! scSub    = tint@baseUv.rg * (1/512, 1/16) + (1/1024, 1/32)
//! palColor = mix(palette@(palU  + scSub), palette@(palU2 + scSubTop), scFacade)
//! tintMul  = mix(tint.b, tintTop.b, scFacade) * 2
//! albedo   = palColor.rgb * tintMul * normalMap.a                    // AO
//! interior = interiorMap(uv * regionXform.xy * roomInvSize)          // 简化：省略盒体投影
//! final    = mix(interior, albedo, shaderMap.a)                      // shaderMap.a = 窗洞
//! ```
//! 平面预览取 UV ∈ [0,1]²（整数 UV 经 fract 收敛到单个 facade cell）。
use dbpf::Package;
use rw4::{Rw4File, SectionType};

const RASTER_TYPE: u32 = 0x2F4E_681C;
const RW4_TYPE: u32 = 0x2F4E_681B;

const OUT_SIZE: usize = 512;

/// 解码后的槽位内容。
enum SlotContent {
    /// 8bit RGBA。
    Rgba {
        width: usize,
        height: usize,
        pixels: Vec<u8>,
    },
    /// f32×4 网格（参数表 / paletteF32）。
    F32 {
        width: usize,
        height: usize,
        values: Vec<[f32; 4]>,
    },
    Failed(String),
}

impl SlotContent {
    fn size(&self) -> (usize, usize) {
        match self {
            SlotContent::Rgba { width, height, .. } | SlotContent::F32 { width, height, .. } => {
                (*width, *height)
            }
            SlotContent::Failed(_) => (0, 0),
        }
    }

    /// 双线性采样（clamp 到边缘），u/v ∈ [0,1]。
    fn sample(&self, u: f32, v: f32) -> Option<[f32; 4]> {
        match self {
            SlotContent::Rgba {
                width,
                height,
                pixels,
            } => {
                let (x, y, tx, ty) = bilinear_coords(*width, *height, u, v)?;
                let read = |px: usize, py: usize| -> [f32; 4] {
                    let at = (py * *width + px) * 4;
                    [
                        f32::from(pixels[at]) / 255.0,
                        f32::from(pixels[at + 1]) / 255.0,
                        f32::from(pixels[at + 2]) / 255.0,
                        f32::from(pixels[at + 3]) / 255.0,
                    ]
                };
                Some(lerp4(read(x, y), read(x + 1, y), read(x, y + 1), read(x + 1, y + 1), tx, ty))
            }
            SlotContent::F32 {
                width,
                height,
                values,
            } => {
                let (x, y, tx, ty) = bilinear_coords(*width, *height, u, v)?;
                let read = |px: usize, py: usize| values[py * *width + px];
                Some(lerp4(read(x, y), read(x + 1, y), read(x, y + 1), read(x + 1, y + 1), tx, ty))
            }
            SlotContent::Failed(_) => None,
        }
    }

    /// 最近邻采样（palette 查表用，语义上就是点采样）。
    fn fetch(&self, u: f32, v: f32) -> [f32; 4] {
        let (width, height) = self.size();
        if width == 0 || height == 0 {
            return [1.0, 1.0, 1.0, 1.0];
        }
        let x = ((u.clamp(0.0, 1.0) * width as f32) as usize).min(width - 1);
        let y = ((v.clamp(0.0, 1.0) * height as f32) as usize).min(height - 1);
        match self {
            SlotContent::Rgba { pixels, .. } => {
                let at = (y * width + x) * 4;
                [
                    f32::from(pixels[at]) / 255.0,
                    f32::from(pixels[at + 1]) / 255.0,
                    f32::from(pixels[at + 2]) / 255.0,
                    f32::from(pixels[at + 3]) / 255.0,
                ]
            }
            SlotContent::F32 { values, .. } => values[y * width + x],
            SlotContent::Failed(_) => [1.0, 1.0, 1.0, 1.0],
        }
    }
}

fn bilinear_coords(
    width: usize,
    height: usize,
    u: f32,
    v: f32,
) -> Option<(usize, usize, f32, f32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let fx = (u.clamp(0.0, 1.0) * (width - 1) as f32).max(0.0);
    let fy = (v.clamp(0.0, 1.0) * (height - 1) as f32).max(0.0);
    let x = (fx.floor() as usize).min(width.saturating_sub(2));
    let y = (fy.floor() as usize).min(height.saturating_sub(2));
    Some((x, y, fx - x as f32, fy - y as f32))
}

fn lerp4(a: [f32; 4], b: [f32; 4], c: [f32; 4], d: [f32; 4], tx: f32, ty: f32) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    for i in 0..4 {
        let top = a[i] + (b[i] - a[i]) * tx;
        let bottom = c[i] + (d[i] - c[i]) * tx;
        out[i] = top + (bottom - top) * ty;
    }
    out
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = raw.iter().filter(|arg| !arg.starts_with("--")).collect();
    let flag = |name: &str| -> Option<String> {
        raw.iter()
            .find_map(|arg| arg.strip_prefix(&format!("--{name}=")).map(str::to_owned))
    };
    if positional.len() < 2 {
        eprintln!(
            "usage: building_material_probe <model_package> <model_instance_hex> \
             --out=<dir> [--col=N] [lookup_package...]"
        );
        std::process::exit(2);
    }
    let model_path = positional[0].clone();
    let instance = u32::from_str_radix(positional[1].trim_start_matches("0x"), 16)
        .unwrap_or_else(|error| panic!("bad instance: {error}"));
    let out_dir = flag("out").unwrap_or_else(|| "tmp/building_material".to_owned());
    let column: usize = flag("col")
        .map(|value| value.parse().unwrap_or_else(|e| panic!("bad --col: {e}")))
        .unwrap_or(0);
    std::fs::create_dir_all(&out_dir).unwrap_or_else(|error| panic!("create {out_dir}: {error}"));

    let mut paths = vec![model_path.clone()];
    paths.extend(positional[2..].iter().map(|value| (*value).clone()));
    let packages: Vec<Package> = paths
        .iter()
        .map(|path| Package::open(path).unwrap_or_else(|error| panic!("open {path}: {error}")))
        .collect();

    // ---- 模型 → 材质绑定 ----
    let (owner, entry) = find_entry(&packages, RW4_TYPE, instance, None)
        .unwrap_or_else(|| panic!("model 0x{instance:08X} not found"));
    println!(
        "model 0x{:08X}-0x{:08X}-0x{:08X}  pkg={}",
        entry.id.type_id, entry.id.group, entry.id.instance, paths[owner]
    );
    let data = packages[owner].read(&entry).unwrap();
    let file = Rw4File::parse(&data).expect("parse model");
    let bindings = file.decode_mesh_material_bindings(&data);
    println!("MeshMaterialAssignment 绑定数 = {}", bindings.len());
    for binding in &bindings {
        println!(
            "  mesh#{:<4} -> material#{:<4} (unknown={})",
            binding.mesh_section, binding.material_section, binding.unknown2
        );
    }
    let binding = bindings.first().copied().expect("no mesh→material binding");
    let material = file
        .decode_material(&data, binding.material_section)
        .expect("decode material");
    let refs: Vec<_> = material
        .texture_refs()
        .iter()
        .map(|r| (r.slot, r.texture_instance))
        .collect();
    println!("\nmaterial#{}：{} 个槽", binding.material_section, refs.len());

    // ---- 逐槽解析 ----
    let mut slots: Vec<(u32, SlotContent)> = Vec::new();
    for (slot, texture_instance) in refs {
        if slot == rw4::SHADER_DEF_MARKER {
            continue;
        }
        let content = resolve_slot(&packages, &paths, texture_instance);
        let (width, height) = content.size();
        match &content {
            SlotContent::Rgba { .. } => println!(
                "  slot{slot:<2} 0x{texture_instance:08X}  RGBA {width}x{height}"
            ),
            SlotContent::F32 { .. } => println!(
                "  slot{slot:<2} 0x{texture_instance:08X}  F32  {width}x{height}"
            ),
            SlotContent::Failed(reason) => {
                println!("  slot{slot:<2} 0x{texture_instance:08X}  失败: {reason}")
            }
        }
        // 导出：RGBA 直出；F32 参数表按 0..1 截断成图。
        match &content {
            SlotContent::Rgba {
                width,
                height,
                pixels,
            } => save_png(&out_dir, &format!("slot{slot}.png"), *width, *height, pixels),
            SlotContent::F32 {
                width,
                height,
                values,
            } => {
                let pixels: Vec<u8> = values
                    .iter()
                    .flat_map(|value| {
                        [
                            (value[0].clamp(0.0, 1.0) * 255.0) as u8,
                            (value[1].clamp(0.0, 1.0) * 255.0) as u8,
                            (value[2].clamp(0.0, 1.0) * 255.0) as u8,
                            255,
                        ]
                    })
                    .collect();
                save_png(&out_dir, &format!("slot{slot}_f32.png"), *width, *height, &pixels);
            }
            SlotContent::Failed(_) => {}
        }
        slots.push((slot, content));
    }
    let slot_of = |slot: u32| slots.iter().find(|(s, _)| *s == slot).map(|(_, c)| c);

    // ---- 参数表（slot0）----
    let params = match slot_of(0) {
        Some(SlotContent::F32 {
            width,
            height,
            values,
        }) => Some((*width, *height, values.clone())),
        _ => None,
    };
    let Some((param_w, param_h, param_values)) = params else {
        println!("\nslot0 不是 paletteF32 参数表，跳过合成预览。");
        return;
    };
    let col = column.min(param_w.saturating_sub(1));
    let row = |r: usize| -> [f32; 4] {
        param_values
            .get(r * param_w + col)
            .copied()
            .unwrap_or([0.0; 4])
    };
    println!(
        "\n参数表 {param_w}x{param_h}，取列 {col}：\n  row0(palU/palU2/interior) = {:?}\n  row1(regionXform)        = {:?}\n  row2(regionXform2/top)   = {:?}\n  row3(padding/roomInv)    = {:?}",
        row(0),
        row(1),
        row(2),
        row(3)
    );
    let pal_origin = row(0);
    let xform = row(1);
    let xform2 = row(2);
    let room = row(3);

    // ---- 平面（UV 域）合成 ----
    let tint = slot_of(1);
    let normal = slot_of(2);
    let shader = slot_of(3);
    let palette = slot_of(4);
    let interior = slot_of(5);

    let mut albedo = vec![0u8; OUT_SIZE * OUT_SIZE * 4];
    let mut ao_out = vec![0u8; OUT_SIZE * OUT_SIZE * 4];
    let mut win_out = vec![0u8; OUT_SIZE * OUT_SIZE * 4];
    let mut tint_b_out = vec![0u8; OUT_SIZE * OUT_SIZE * 4];

    for y in 0..OUT_SIZE {
        for x in 0..OUT_SIZE {
            let at = (y * OUT_SIZE + x) * 4;
            let u = (x as f32 + 0.5) / OUT_SIZE as f32;
            let v = (y as f32 + 0.5) / OUT_SIZE as f32;
            let base_uv = [fract(u) * xform[0] + xform[2], fract(v) * xform[1] + xform[3]];
            let top_uv = [fract(u) * xform2[0] + xform2[2], fract(v) * xform2[1] + xform2[3]];

            let tint_base = tint
                .and_then(|c| c.sample(base_uv[0], base_uv[1]))
                .unwrap_or([0.0; 4]);
            let has_top = xform2[0] > 0.0 && xform2[1] > 0.0;
            let tint_top = if has_top {
                tint.and_then(|c| c.sample(top_uv[0], top_uv[1]))
                    .unwrap_or([0.0; 4])
            } else {
                tint_base
            };
            let sc_facade = if has_top { tint_top[3] } else { 0.0 };

            // 调色板查表：U 以 palU/palU2 为基列，V 为色行（+子采样偏移）。
            let sub_base = [tint_base[0] / 512.0 + 1.0 / 1024.0, tint_base[1] / 16.0 + 1.0 / 32.0];
            let sub_top = [tint_top[0] / 512.0 + 1.0 / 1024.0, tint_top[1] / 16.0 + 1.0 / 32.0];
            let pal_color = match palette {
                Some(pal) => {
                    let base = pal.fetch(pal_origin[0] + sub_base[0], sub_base[1]);
                    let top = pal.fetch(pal_origin[1] + sub_top[0], sub_top[1]);
                    lerp4(base, top, base, top, sc_facade, 0.0)
                }
                None => [1.0, 1.0, 1.0, 1.0],
            };
            let tint_mul = (tint_base[2] + (tint_top[2] - tint_base[2]) * sc_facade) * 2.0;

            let ao = normal
                .and_then(|c| c.sample(base_uv[0], base_uv[1]))
                .map(|px| px[3])
                .unwrap_or(1.0);
            let shader_px = shader
                .and_then(|c| c.sample(base_uv[0], base_uv[1]))
                .unwrap_or([1.0; 4]);
            // 简化内景：省略盒体投影，只按房间栅格取一格（白天近暗）。
            let interior_rgb = match interior {
                Some(map) => {
                    let iu = [u * xform[0] * room[2], v * xform[1] * room[3]];
                    let cell = [iu[0].fract(), iu[1].fract()];
                    let tc = [cell[0] * pal_origin[2], pal_origin[3] + cell[1]];
                    let px = map.fetch(tc[0], tc[1]);
                    [px[0] * 0.12, px[1] * 0.12, px[2] * 0.12]
                }
                None => [0.0, 0.0, 0.0],
            };
            let opacity = shader_px[3];
            let mut rgb = [0.0f32; 3];
            for ch in 0..3 {
                let facade = pal_color[ch] * tint_mul * ao;
                rgb[ch] = interior_rgb[ch] + (facade - interior_rgb[ch]) * opacity;
            }
            for ch in 0..3 {
                albedo[at + ch] = (rgb[ch].clamp(0.0, 1.0) * 255.0) as u8;
            }
            albedo[at + 3] = 255;
            let gray = (ao.clamp(0.0, 1.0) * 255.0) as u8;
            ao_out[at] = gray;
            ao_out[at + 1] = gray;
            ao_out[at + 2] = gray;
            ao_out[at + 3] = 255;
            let win = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
            win_out[at] = win;
            win_out[at + 1] = win;
            win_out[at + 2] = win;
            win_out[at + 3] = 255;
            let tb = ((tint_mul / 2.0).clamp(0.0, 1.0) * 255.0) as u8;
            tint_b_out[at] = tb;
            tint_b_out[at + 1] = tb;
            tint_b_out[at + 2] = tb;
            tint_b_out[at + 3] = 255;
        }
    }
    save_png(&out_dir, "compose_albedo.png", OUT_SIZE, OUT_SIZE, &albedo);
    save_png(&out_dir, "compose_ao.png", OUT_SIZE, OUT_SIZE, &ao_out);
    save_png(&out_dir, "compose_windows_a.png", OUT_SIZE, OUT_SIZE, &win_out);
    save_png(&out_dir, "compose_tint_b.png", OUT_SIZE, OUT_SIZE, &tint_b_out);

    println!("\n输出目录 {out_dir}：");
    println!("  slot*.png            逐槽原图（slot0 为参数表，slot4 为调色板）");
    println!("  compose_albedo.png   building4 链的 UV 域合成（含 AO、窗洞、简化内景）");
    println!("  compose_ao.png       normalMap.a（AO 通道）");
    println!("  compose_windows_a.png shaderMap.a（窗洞掩码，1=外观 0=透内景）");
    println!("  compose_tint_b.png   tint.b 亮度（×2 后）");
}

/// 跨包解析一个槽位实例：raster → RGBA；RW4 → 首个 TEXTURE（paletteF32 或 RGBA）。
fn resolve_slot(packages: &[Package], paths: &[String], instance: u32) -> SlotContent {
    let Some((owner, entry)) = find_entry(packages, RASTER_TYPE, instance, None)
        .or_else(|| find_entry(packages, RW4_TYPE, instance, None))
    else {
        return SlotContent::Failed("instance not found in open packages".into());
    };
    let Ok(bytes) = packages[owner].read(&entry) else {
        return SlotContent::Failed("read failed".into());
    };
    let source = paths[owner]
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_owned();
    if entry.id.type_id == RASTER_TYPE {
        let Ok(raster) = rw4::RasterImage::parse(&bytes) else {
            return SlotContent::Failed("raster parse failed".into());
        };
        return match raster.decode_top_mip_rgba() {
            Ok(pixels) => SlotContent::Rgba {
                width: raster.width as usize,
                height: raster.height as usize,
                pixels,
            },
            Err(error) => SlotContent::Failed(format!("raster decode failed: {error}")),
        };
    }
    let Ok(file) = Rw4File::parse(&bytes) else {
        return SlotContent::Failed("rw4 parse failed".into());
    };
    let Some(section) = file.sections_of_type(SectionType::TEXTURE).next() else {
        return SlotContent::Failed("rw4 has no texture section".into());
    };
    let Ok(texture) = file.decode_texture(&bytes, section.number) else {
        return SlotContent::Failed("texture decode failed".into());
    };
    let width = texture.width as usize;
    let height = texture.height as usize;
    if texture.texture_type == rw4::TEXTURE_TYPE_PALETTE_F32 {
        return match texture.decode_palette_f32() {
            Ok(values) => SlotContent::F32 {
                width,
                height,
                values,
            },
            Err(error) => SlotContent::Failed(format!("paletteF32 decode failed: {error}")),
        };
    }
    match texture.decode_top_mip_rgba() {
        Ok(pixels) => SlotContent::Rgba {
            width,
            height,
            pixels,
        },
        Err(error) => SlotContent::Failed(format!("{source}: texture pixels failed: {error}")),
    }
}

fn find_entry(
    packages: &[Package],
    type_id: u32,
    instance: u32,
    group: Option<u32>,
) -> Option<(usize, dbpf::IndexEntry)> {
    for (index, package) in packages.iter().enumerate() {
        for entry in package.entries() {
            if entry.id.type_id == type_id
                && entry.id.instance == instance
                && group.is_none_or(|value| entry.id.group == value)
            {
                return Some((index, entry.clone()));
            }
        }
    }
    None
}

fn fract(value: f32) -> f32 {
    value - value.floor()
}

fn save_png(dir: &str, name: &str, width: usize, height: usize, rgba: &[u8]) {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return;
    }
    let Some(image) = image::RgbaImage::from_raw(width as u32, height as u32, rgba.to_vec()) else {
        return;
    };
    image
        .save(format!("{dir}/{name}"))
        .unwrap_or_else(|error| panic!("save {name}: {error}"));
}
