//! Lot label raster -> native smooth selectors (lot_native.py:20-58 contract).
fn distance(seed: &[bool], n: usize, dx: f64, dy: f64) -> Vec<f64> {
    let mut d: Vec<f64> = seed.iter().map(|s| if *s { 0. } else { 1e12 }).collect();
    let diagonal = dx.hypot(dy);
    for forward in [true, false] {
        for row in 0..n {
            let y = if forward { row } else { n - 1 - row };
            let previous = if forward {
                y.checked_sub(1)
            } else if y + 1 < n {
                Some(y + 1)
            } else {
                None
            };
            if let Some(p) = previous {
                for x in 0..n {
                    let mut value = d[y * n + x].min(d[p * n + x] + dy);
                    if x > 0 {
                        value = value.min(d[p * n + x - 1] + diagonal);
                    }
                    if x + 1 < n {
                        value = value.min(d[p * n + x + 1] + diagonal);
                    }
                    d[y * n + x] = value;
                }
            }
            for x in 1..n {
                d[y * n + x] = d[y * n + x].min(d[y * n + x - 1] + dx);
            }
            for x in (0..n - 1).rev() {
                d[y * n + x] = d[y * n + x].min(d[y * n + x + 1] + dx);
            }
        }
    }
    d
}

pub fn selectors(
    labels: &[u8],
    n: usize,
    size: [f64; 2],
    borders: [f64; 4],
) -> (Vec<u8>, [f64; 4]) {
    let dx = size[0] / n as f64;
    let dy = size[1] / n as f64;
    let ramp = 2f64
        .max(4. * dx.max(dy))
        .max(borders.iter().copied().fold(0., f64::max) * 2.1);
    let mut rgba = vec![0; n * n * 4];
    for channel in 0..4 {
        let seed: Vec<bool> = labels.iter().map(|v| *v as usize == channel + 1).collect();
        if !seed.iter().any(|v| *v) {
            continue;
        }
        if seed.iter().all(|v| *v) {
            for pixel in rgba.chunks_mut(4) {
                pixel[channel] = 255;
            }
            continue;
        }
        let inside = distance(&seed.iter().map(|v| !*v).collect::<Vec<_>>(), n, dx, dy);
        let outside = distance(&seed, n, dx, dy);
        for p in 0..n * n {
            let signed = if seed[p] {
                (inside[p] - dx.min(dy) * 0.5).max(0.)
            } else {
                -(outside[p] - dx.min(dy) * 0.5).max(0.)
            };
            rgba[p * 4 + channel] =
                ((0.5 + signed / ramp).clamp(0., 1.) * 255.).round_ties_even() as u8;
        }
    }
    (rgba, borders.map(|v| v / (2. * ramp)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_full_and_half_texel_contour_follow_native_mask_contract() {
        let (mask, widths) = selectors(&[0, 1, 0, 1], 2, [2., 2.], [0.15; 4]);
        assert_eq!([mask[0], mask[4]], [96, 159]);
        assert!((widths[0] - 0.01875).abs() < 1e-12);
        assert_eq!(
            selectors(&[2; 4], 2, [2., 2.], [0.; 4]).0,
            vec![0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0]
        );
    }
}
