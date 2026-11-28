//! Building40 coverage is the Base slot1 alpha (building4Clip).
use std::io::Cursor;

use base64::{Engine, engine::general_purpose::STANDARD};
use image::{Rgba, RgbaImage};

use super::{
    dump::{Bundle, PackedCutout},
    recipe::{RecipeOutput, params_table, solid_png},
    resources::{linear_to_srgb, srgb_to_linear},
};

pub(super) fn build(bundle: &Bundle, source: &PackedCutout) -> Result<RecipeOutput, String> {
    let read = |name: &str| -> Result<RgbaImage, String> {
        let encoded = bundle
            .packed_window_images
            .get(name)
            .ok_or_else(|| format!("missing packed cutout image {name}"))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| error.to_string())?;
        image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .map(|image| image.to_rgba8())
            .map_err(|error| format!("cutout image {name}: {error}"))
    };
    let color = read(&source.color)?;
    let mut normal = read(&source.normal)?;
    if color.dimensions() != normal.dimensions() {
        return Err("cutout color and normal dimensions differ".into());
    }
    // The Base palette origin selects the color; slot1 B varies its brightness.
    // Fit that color from covered texels, since transparent pixels can contain
    // arbitrary RGB. Coverage itself is independent of the color conversion.
    let base = color
        .pixels()
        .filter(|pixel| pixel[3] >= 128)
        .map(|pixel| tinted_rgb(*pixel, source.tint))
        .max_by(|a, b| luminance(*a).total_cmp(&luminance(*b)))
        .unwrap_or([0.0; 3]);
    let reference = luminance(base).max(f32::EPSILON);
    let [r, g, b] = base.map(|value| byte(linear_to_srgb(value)));
    let palette = RgbaImage::from_fn(512, 16, |_, y| {
        if y < 14 {
            Rgba([r, g, b, 0])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    let tint = RgbaImage::from_fn(color.width(), color.height(), |x, y| {
        let pixel = *color.get_pixel(x, y);
        let shade = luminance(tinted_rgb(pixel, source.tint)) / reference;
        Rgba([0, 0, byte(shade * 0.5), pixel[3]])
    });
    for pixel in normal.pixels_mut() {
        let xy = [pixel[0], pixel[1]]
            .map(|value| ((f32::from(value) / 255.0) * 2.0 - 1.0) * source.normal_strength);
        let z = (1.0 - (xy[0] * xy[0] + xy[1] * xy[1]).min(1.0)).sqrt();
        *pixel = Rgba([
            byte(xy[0] * 0.5 + 0.5),
            byte(xy[1] * 0.5 + 0.5),
            byte(z * 0.5 + 0.5),
            255,
        ]);
    }
    Ok(RecipeOutput {
        maps: [
            png(&tint)?,
            png(&normal)?,
            solid_png([23, 0, 0, 255])?,
            png(&palette)?,
            solid_png([0, 0, 0, 0])?,
        ],
        params: params_table([0.0; 4], [1.0, 1.0, 0.0, 0.0], [0.0; 4]),
        param_cols: 1,
    })
}

fn tinted_rgb(pixel: Rgba<u8>, tint: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| srgb_to_linear(f32::from(pixel[i]) / 255.0) * tint[i])
}

fn luminance([r, g, b]: [f32; 3]) -> f32 {
    r * 0.2126 + g * 0.7152 + b * 0.0722
}

fn byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round_ties_even() as u8
}

fn png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot1_keeps_source_coverage_and_slot3_stays_exterior() {
        let source = RgbaImage::from_vec(
            4,
            1,
            vec![
                90, 100, 110, 0, 90, 100, 110, 127, 110, 120, 130, 128, 110, 120, 130, 255,
            ],
        )
        .unwrap();
        let normal = RgbaImage::from_pixel(4, 1, Rgba([128, 128, 255, 255]));
        let value = serde_json::json!({
            "version": 1, "unit": 1,
            "packed_window_images": {
                "color": STANDARD.encode(png(&source).unwrap()),
                "normal": STANDARD.encode(png(&normal).unwrap()),
            },
        });
        let bundle = Bundle::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        let recipe = build(
            &bundle,
            &PackedCutout {
                uv: "uv".into(),
                color: "color".into(),
                normal: "normal".into(),
                tint: [1.0; 3],
                normal_strength: 1.0,
            },
        )
        .unwrap();
        let decode = |slot: usize| {
            image::load_from_memory(&recipe.maps[slot])
                .unwrap()
                .to_rgba8()
        };
        assert_eq!(
            decode(0).pixels().map(|p| p[3]).collect::<Vec<_>>(),
            [0, 127, 128, 255]
        );
        assert_eq!(decode(2).get_pixel(0, 0)[3], 255);
        assert_eq!(decode(3).get_pixel(0, 0).0, [110, 120, 130, 0]);
        assert!(decode(0).get_pixel(1, 0)[2] < decode(0).get_pixel(2, 0)[2]);
        assert_eq!(
            &recipe.params[4..12],
            &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
    }
}
