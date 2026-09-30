//! 法线图案图集（0x60E7805D）边缘列/行解剖 —— 排查 PE 精细地面
//! "图案重复周期处的竖向切割痕"（2026-09-29 用户对拍）。
//!
//! 假设：图集格的边缘 1-2 列/行法线极端（图集打包缝），烘焙坡度明暗时在
//! 每个图案重复边界产生一条暗线（GPU+mip 在游戏里会把它滤掉，烘焙进
//! 反照率则永久保留）。本探针输出每格四边与内部的 nx/ny 均值对照 +
//! 整图 PNG dump，供直接目检。
//!
//! ```bash
//! cargo run -p sc-exporter --release --example normal_atlas_probe [-- out_dir]
//! ```
use dbpf::{IndexEntry, Package};

const RW4_TYPE: u32 = 0x2F4E_681B;
const H_NORMAL_ATLAS: u32 = 0x60E7_805D;
const ATLAS_COLS: usize = 4;

/// 一个像素带的 nx/ny 累计（byte/127.5 − 1）。
#[derive(Default)]
struct BandSum {
    nx: f64,
    ny: f64,
    count: u64,
}

impl BandSum {
    fn add(&mut self, rgba: &[u8], at: usize) {
        self.nx += f64::from(rgba[at]) / 127.5 - 1.0;
        self.ny += f64::from(rgba[at + 1]) / 127.5 - 1.0;
        self.count += 1;
    }
    fn show(&self) -> String {
        if self.count == 0 {
            return "n/a".to_owned();
        }
        format!(
            "nx{:+.3} ny{:+.3}",
            self.nx / self.count as f64,
            self.ny / self.count as f64
        )
    }
}

fn main() -> dbpf::Result<()> {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tmp/dynamic/normal_atlas_probe".to_owned());
    std::fs::create_dir_all(&out_dir)?;

    // 附加模式：对任意 PNG 做竖向异常列扫描（列均亮相对左右邻列的跳变），
    // 用于解剖探针成品图里是否同样存在"切割痕"。
    if let Some(png) = std::env::args().nth(2) {
        let image = image::open(&png).expect("open png").to_rgba8();
        let (w, h) = (image.width() as usize, image.height() as usize);
        let raw = image.into_raw();
        let lum = |x: usize, y: usize| -> f64 {
            let at = (y * w + x) * 4;
            0.2126 * f64::from(raw[at])
                + 0.7152 * f64::from(raw[at + 1])
                + 0.0722 * f64::from(raw[at + 2])
        };
        let mut col_mean = vec![0.0f64; w];
        for x in 0..w {
            let mut sum = 0.0;
            for y in 0..h {
                sum += lum(x, y);
            }
            col_mean[x] = sum / h as f64;
        }
        // 平滑基准（±8 列中位滤波近似）后的负向跳变 = 暗线。
        let mut worst: Vec<(usize, f64)> = Vec::new();
        for x in 8..w - 8 {
            let mut neighbours = Vec::with_capacity(16);
            for d in -8i64..=8i64 {
                if d == 0 {
                    continue;
                }
                neighbours.push(col_mean[(x as i64 + d) as usize]);
            }
            neighbours.sort_by(|a, b| a.total_cmp(b));
            let baseline = neighbours[neighbours.len() / 2];
            let dip = baseline - col_mean[x];
            if dip > 6.0 {
                worst.push((x, dip));
            }
        }
        // 合并相邻列。
        let mut groups: Vec<(usize, f64)> = Vec::new();
        for (x, dip) in worst {
            match groups.last_mut() {
                Some((last_x, last_dip)) if x.saturating_sub(*last_x) <= 2 => {
                    if dip > *last_dip {
                        *last_x = x;
                        *last_dip = dip;
                    }
                }
                _ => groups.push((x, dip)),
            }
        }
        println!(
            "{}: {w}x{h}, 暗列组 {} 个（阈值 6/255）：{:?}",
            png,
            groups.len(),
            &groups[..groups.len().min(40)]
        );
        return Ok(());
    }

    let graphics =
        Package::open(r"D:\ea-games\SimCity\SimCityData\SimCity_Graphics.package")?;
    let mut target: Option<IndexEntry> = None;
    for entry in graphics.entries() {
        if entry.id.type_id == RW4_TYPE && entry.id.instance == H_NORMAL_ATLAS {
            target = Some(entry.clone());
            break;
        }
    }
    let Some(entry) = target else {
        println!("normal atlas 0x{H_NORMAL_ATLAS:08X} not found");
        return Ok(());
    };
    let bytes = graphics.read(&entry)?;
    let Ok(file) = rw4::Rw4File::parse(&bytes) else {
        println!("rw4 parse failed");
        return Ok(());
    };
    let mut decoded = 0usize;
    for section in file.sections_of_type(rw4::SectionType::TEXTURE) {
        let Ok(texture) = file.decode_texture(&bytes, section.number) else {
            continue;
        };
        let Ok(rgba) = texture.decode_top_mip_rgba() else {
            continue;
        };
        let width = texture.width as usize;
        let height = rgba.len() / 4 / width.max(1);
        println!(
            "texture section #{}: {width}x{height}, type {}",
            section.number, texture.texture_type
        );
        let image = image::RgbaImage::from_raw(width as u32, height as u32, rgba.clone())
            .expect("atlas buf");
        let path = format!("{out_dir}/normal_atlas_{H_NORMAL_ATLAS:08X}.png");
        if let Err(error) = image.save(&path) {
            println!("  save failed: {error}");
            continue;
        }
        println!("  -> {path}");

        let cw = width / ATLAS_COLS;
        let ch = height / ATLAS_COLS;
        if cw < 8 || ch < 8 {
            println!("  cell too small: {cw}x{ch}");
            continue;
        }
        for cell in 0..ATLAS_COLS * ATLAS_COLS {
            let ox = (cell % ATLAS_COLS) * cw;
            let oy = (cell / ATLAS_COLS) * ch;
            let mut col_first = BandSum::default();
            let mut col_second = BandSum::default();
            let mut col_last = BandSum::default();
            let mut row_first = BandSum::default();
            let mut row_last = BandSum::default();
            let mut interior = BandSum::default();
            for y in 0..ch {
                for x in 0..cw {
                    let at = ((oy + y) * width + ox + x) * 4;
                    match (x, y) {
                        (0, _) => col_first.add(&rgba, at),
                        (1, _) => col_second.add(&rgba, at),
                        (xx, _) if xx == cw - 1 => col_last.add(&rgba, at),
                        (_, 0) => row_first.add(&rgba, at),
                        (_, yy) if yy == ch - 1 => row_last.add(&rgba, at),
                        _ => interior.add(&rgba, at),
                    }
                }
            }
            println!(
                "cell {cell:2}: col0[{}] col1[{}] colN[{}] row0[{}] rowN[{}] 内部[{}]",
                col_first.show(),
                col_second.show(),
                col_last.show(),
                row_first.show(),
                row_last.show(),
                interior.show(),
            );
        }
        decoded += 1;
    }
    println!("decoded {decoded} texture section(s)");
    Ok(())
}
