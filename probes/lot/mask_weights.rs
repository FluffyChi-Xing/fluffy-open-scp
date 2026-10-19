//! 检查 raw mask 指定区域的通道权重（诊断"变绿"根因：区域是否低于 0.5 阈值）。
//! 用法：mask_weights <lot_package> <lot_instance_hex> <x0>,<y0>,<x1>,<y1> [lookup...]

use dbpf::{IndexEntry, Package};
use sc_properties::{Key, Kind, PropertyFile, Value};

const LOT_TYPE: u32 = 0x00B1_B104;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const H_LOT_MASK: u32 = 0x0CCB_7FD5;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let parse_hex = |v: &str| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap();
    let lot_instance = parse_hex(&positional[1]);
    let mut rect = positional[2].split(',');
    let (x0, y0): (usize, usize) = (
        rect.next().unwrap().parse().unwrap(),
        rect.next().unwrap().parse().unwrap(),
    );
    let (x1, y1): (usize, usize) = (
        rect.next().unwrap().parse().unwrap(),
        rect.next().unwrap().parse().unwrap(),
    );

    let mut paths: Vec<String> = vec![positional[0].clone()];
    paths.extend(positional[3..].iter().map(|s| (*s).clone()));
    let packages: Vec<Package> = paths
        .iter()
        .map(|p| Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}")))
        .collect();

    let (owner, entry) = find_entry(&packages, LOT_TYPE, lot_instance, None).unwrap();
    let data = packages[owner].read(&entry).unwrap();
    let file = PropertyFile::parse(&data).unwrap();
    let mask_key = match file.get(H_LOT_MASK).unwrap().kind {
        Kind::Scalar(Value::Key(key)) => key,
        _ => panic!("no LotMask"),
    };
    let (m_owner, m_entry) = find_entry(&packages, RASTER_TYPE, mask_key.instance, None).unwrap();
    let mask_bytes = packages[m_owner].read(&m_entry).unwrap();
    let raster = rw4::RasterImage::parse(&mask_bytes).unwrap();
    let (w, h) = (raster.width as usize, raster.height as usize);
    let mut raw = raster.decode_top_mip_rgba().unwrap();
    for row in 0..h / 2 {
        let top = row * w * 4;
        let bottom = (h - 1 - row) * w * 4;
        for i in 0..w * 4 {
            raw.swap(top + i, bottom + i);
        }
    }
    println!(
        "lot 0x{lot_instance:08X} mask {w}x{h}  region ({x0},{y0})-({x1},{y1})"
    );
    println!("  x\\ch      R=LC1        G=LC2        B=LC3        A=LC4");
    for y in y0..=y1.min(h - 1) {
        for x in x0..=x1.min(w - 1) {
            let at = (y * w + x) * 4;
            println!(
                "  ({x:3},{y:3})  {:4}        {:4}        {:4}        {:4}",
                raw[at],
                raw[at + 1],
                raw[at + 2],
                raw[at + 3]
            );
        }
    }
}

fn find_entry(
    packages: &[Package],
    type_id: u32,
    instance: u32,
    group: Option<u32>,
) -> Option<(usize, IndexEntry)> {
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
