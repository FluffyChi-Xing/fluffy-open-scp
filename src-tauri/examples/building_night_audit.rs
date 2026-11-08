//! Decode one packaged building model's night-interior inputs.
//! Usage: cargo run -p fluffy-open-scp --example building_night_audit -- <package> <model_instance_hex> <output_dir>

use dbpf::{Package, ResourceId};
use image::{Rgba, RgbaImage};
use rw4::{Rw4File, SectionType};
use std::{error::Error, fmt::Write as _, fs, path::Path};

fn resource_bytes(package: &Package, instance: u32) -> Option<(Vec<u8>, u32)> {
    for type_id in [0x2F4E_681B, 0x2F4E_681C] {
        let id = ResourceId {
            type_id,
            group: 0,
            instance,
        };
        if let Some(entry) = package.entry(id) {
            if let Ok(bytes) = package.read(entry) {
                return Some((bytes, type_id));
            }
        }
    }
    None
}

fn decode_rgba(bytes: &[u8], type_id: u32) -> Result<(Vec<u8>, u32, u32), String> {
    if type_id == 0x2F4E_681C {
        let raster = rw4::RasterImage::parse(bytes).map_err(|error| error.to_string())?;
        let rgba = raster
            .decode_top_mip_rgba()
            .map_err(|error| error.to_string())?;
        return Ok((rgba, raster.width, raster.height));
    }
    let file = Rw4File::parse(bytes).map_err(|error| error.to_string())?;
    let section = file
        .sections_of_type(SectionType::TEXTURE)
        .next()
        .ok_or("texture section missing")?;
    let texture = file
        .decode_texture(bytes, section.number)
        .map_err(|error| error.to_string())?;
    let rgba = texture
        .decode_top_mip_rgba()
        .map_err(|error| error.to_string())?;
    Ok((rgba, u32::from(texture.width), u32::from(texture.height)))
}

fn save_image(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn Error>> {
    let image = RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or("decoded RGBA dimensions do not match payload")?;
    image.save(path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let package_path = args.next().ok_or("package argument missing")?;
    let model_instance = u32::from_str_radix(
        args.next()
            .ok_or("model instance argument missing")?
            .trim_start_matches("0x"),
        16,
    )?;
    let output = args.next().ok_or("output directory argument missing")?;
    let output = Path::new(&output);
    fs::create_dir_all(output)?;

    let package = Package::open(&package_path)?;
    let model_id = ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: model_instance,
    };
    let model_entry = package.entry(model_id).ok_or("model resource missing")?;
    let model_bytes = package.read(model_entry)?;
    let model = Rw4File::parse(&model_bytes)?;
    let material_section = model
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .ok_or("material section missing")?;
    let material = model.decode_material(&model_bytes, material_section.number)?;
    let refs = material.texture_refs();
    println!("model={model_instance:08X} material={} refs={refs:?}", material_section.number);

    for slot in 0..=5u32 {
        let Some(reference) = refs.iter().find(|row| row.slot & 0xFF == slot) else {
            println!("slot{slot}: MISSING");
            continue;
        };
        let instance = reference.texture_instance;
        let Some((bytes, type_id)) = resource_bytes(&package, instance) else {
            println!("slot{slot}: {instance:08X} RESOURCE MISSING");
            continue;
        };
        if slot == 0 {
            let file = Rw4File::parse(&bytes)?;
            let section = file
                .sections_of_type(SectionType::TEXTURE)
                .next()
                .ok_or("slot0 texture section missing")?;
            let texture = file.decode_texture(&bytes, section.number)?;
            let params = texture.decode_palette_f32()?;
            let mut text = String::new();
            writeln!(text, "width={} height={}", texture.width, texture.height)?;
            for (index, values) in params.iter().enumerate() {
                writeln!(text, "texel {index:03}: {values:?}")?;
            }
            fs::write(output.join("slot0-params.txt"), text)?;
            println!(
                "slot0: {instance:08X} type={type_id:08X} {}x{} texels={}",
                texture.width,
                texture.height,
                params.len()
            );
            continue;
        }
        if !matches!(slot, 3 | 5) {
            println!("slot{slot}: {instance:08X} type={type_id:08X} bytes={}", bytes.len());
            continue;
        }
        let (rgba, width, height) = decode_rgba(&bytes, type_id).map_err(|error| {
            format!("slot{slot} {instance:08X} decode failed: {error}")
        })?;
        save_image(&output.join(format!("slot{slot}.png")), width, height, &rgba)?;
        let nonzero_alpha = rgba.chunks_exact(4).filter(|pixel| pixel[3] != 0).count();
        let transparent_windows = rgba.chunks_exact(4).filter(|pixel| pixel[3] != 255).count();
        println!(
            "slot{slot}: {instance:08X} type={type_id:08X} {width}x{height} nonzero_alpha={nonzero_alpha} nonopaque={transparent_windows}"
        );
        if slot == 5 {
            let mut alpha = RgbaImage::new(width, height);
            for (index, pixel) in rgba.chunks_exact(4).enumerate() {
                let value = pixel[3];
                alpha.put_pixel(
                    index as u32 % width,
                    index as u32 / width,
                    Rgba([value, value, value, 255]),
                );
            }
            alpha.save(output.join("slot5-alpha.png"))?;
            for state in 0..16u32 {
                let start = state * 64;
                if start >= width {
                    break;
                }
                let end = (start + 64).min(width);
                let mut lit = 0usize;
                let mut alpha_sum = 0u64;
                let mut rgb_sum = [0u64; 3];
                for y in 0..height {
                    for x in start..end {
                        let offset = ((y * width + x) * 4) as usize;
                        let pixel = &rgba[offset..offset + 4];
                        if pixel[3] != 0 {
                            lit += 1;
                            alpha_sum += u64::from(pixel[3]);
                            for channel in 0..3 {
                                rgb_sum[channel] += u64::from(pixel[channel]);
                            }
                        }
                    }
                }
                println!(
                    "  state {state:02}: lit_pixels={lit} alpha_sum={alpha_sum} lit_rgb_sum={rgb_sum:?}"
                );
            }
        }
    }
    Ok(())
}
