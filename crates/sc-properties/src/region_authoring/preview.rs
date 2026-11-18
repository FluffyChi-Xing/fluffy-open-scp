use super::*;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

/// Isometric artwork for the stock selection screen. Samples the same field and
/// road position that go into the native region, so previews follow the seed.
pub(crate) fn isometric_png(region: &CoastalRegion) -> Result<Vec<u8>, String> {
    let mut image = RgbaImage::new(1280, 720);
    let sea = ((SEA + 1024) * 32) as f64;
    for (px, py, pixel) in image.enumerate_pixels_mut() {
        let u = (f64::from(px) + 0.5 - 640.0) / 254.0;
        let v = (f64::from(py) + 0.5 - 337.0) / 145.0;
        let east = (v + u + 1.0) * 0.5;
        let north = (u - v + 1.0) * 0.5;
        let shade = if east >= 0.0 && east < 1.0 && north >= 0.0 && north < 1.0 {
            let x = (east * HEIGHT_SIDE as f64) as usize;
            let y = (north * HEIGHT_SIDE as f64) as usize;
            let height = f64::from(region.heights[y * HEIGHT_SIDE + x]);
            let relief = (height - sea) / 32.0;
            let road_offset = (east - 0.75).abs();
            if road_offset < 0.0048 {
                [232, 193, 83]
            } else if road_offset < 0.008 {
                [47, 52, 49]
            } else if relief < 0.0 {
                [
                    35 + (27.0 + relief).max(0.0) as u8,
                    89 + (30.0 + relief).max(0.0) as u8,
                    124 + (35.0 + relief).max(0.0) as u8,
                ]
            } else if relief < 7.0 {
                [200, 177, 126]
            } else {
                let light = ((relief - 7.0) * 1.6) as u8;
                [79 + light, 111 + light, 63 + light / 2]
            }
        } else {
            let front_edge = 1.0 - u.abs();
            if u.abs() >= 1.0 || v < front_edge || v > front_edge + 0.44 {
                continue;
            }
            if u < 0.0 { [97, 75, 56] } else { [126, 99, 72] }
        };
        *pixel = Rgba([shade[0], shade[1], shade[2], 255]);
    }

    png(image)
}

fn png(image: RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|err| err.to_string())?;
    Ok(out.into_inner())
}

// Camera on the east shore, looking north along the road with the sea to the left.
pub(crate) fn landscape_png(region: &CoastalRegion) -> Result<Vec<u8>, String> {
    const SIZE: u32 = 512;
    let mut image = RgbaImage::new(SIZE, SIZE);
    for (column, row, pixel) in image.enumerate_pixels_mut() {
        let vx = (f64::from(column) + 0.5) / f64::from(SIZE) - 0.5;
        let pitch = (f64::from(row) + 0.5) / f64::from(SIZE) - 0.41;
        let heading = 0.07 + vx * 1.25;
        let sky = mix(
            [180., 205., 218.],
            [101., 160., 206.],
            (-pitch * 2.0).clamp(0., 1.),
        );
        let mut color = sky;
        if pitch > 0.0 {
            // March through the actual 8 m field until the view ray hits water or land.
            let camera = [1750.0, -3620.0, f64::from(SEA) + 154.0];
            let mut distance = 1.0;
            while distance < 13500.0 {
                let x = camera[0] + distance * heading;
                let y = camera[1] + distance;
                let z = camera[2] - distance * pitch;
                let ground = elevation(region, x, y);
                if z <= ground.max(f64::from(SEA)) {
                    let samples = perlin(903, x / 38.0, y / 38.0);
                    let surface = if ground < f64::from(SEA) {
                        let ripple = 0.5 + (y * 0.24 + samples * 4.0).sin() * 0.19;
                        mix([39., 102., 142.], [98., 161., 181.], ripple)
                    } else if x.abs() < 115.0 {
                        mix([158., 134., 89.], [208., 185., 132.], 0.5 + samples * 0.3)
                    } else if (x - f64::from(HIGHWAY_X)).abs() < 23.0 {
                        let offset = (x - f64::from(HIGHWAY_X)).abs();
                        if offset < 1.5 {
                            [220., 183., 87.]
                        } else if offset > 21.0
                            || ((offset - 11.).abs() < 0.8 && y.rem_euclid(38.) < 21.)
                        {
                            [221., 222., 207.]
                        } else {
                            [69., 76., 78.]
                        }
                    } else {
                        let relief = (ground - elevation(region, x - 12., y + 10.)) * 0.11;
                        let amount = (0.52 + samples * 0.27 + relief).clamp(0.0, 1.0);
                        mix([63., 87., 49.], [141., 156., 87.], amount)
                    };
                    color = mix(surface, sky, (distance / 10500.0).powf(1.3).min(1.0));
                    break;
                }
                distance += (distance * 0.018).clamp(7.0, 82.0);
            }
        }
        *pixel = Rgba([color[0] as u8, color[1] as u8, color[2] as u8, 255]);
    }
    png(image)
}

fn elevation(region: &CoastalRegion, x: f64, y: f64) -> f64 {
    let pixel = |v| (((v + f64::from(HALF)) / 8.0_f64) as usize).min(HEIGHT_SIDE - 1);
    f64::from(region.heights[pixel(y) * HEIGHT_SIDE + pixel(x)]) / 32.0 - 1024.0
}

fn mix(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
