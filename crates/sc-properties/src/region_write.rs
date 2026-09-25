//! 区域高度图写回：4096² u16 高度场 → 341-tile 金字塔重排 → 未压缩 overlay 包。
//!
//! 读侧见 [`crate::region_map`]（mip0 排布为全游戏共享常量
//! [`SHARED_TILE_GRID`](crate::region_map::SHARED_TILE_GRID)）。本模块实现
//! map-workbench E1 / priorities P0-1 的写回链路：
//!
//! 1. [`read_region_field`]：按 mip0 网格拼装 4096² 高度场（1 px = 8 m，
//!    raw = (z+1024)×32，见 region_map 水位注释）；
//! 2. [`solve_f0_pyramid`]：恢复 5 级 tile 实例排布——父 tile 每象限 =
//!    子 tile 的 2×2 box 降采样（sum/4 截断，`tile_arrange`/`ed_grid_for_region`
//!    同款实证算法），粗层级实例对"由场算出的期望内容"做子采样 L1 最近匹配；
//! 3. [`build_heightmap_overlay`]：逐层由高度场重算 256² tile 内容并编码
//!    （20 B 头 + u16 LE；头优先复用源包原字节，缺失时合成 BE
//!    `[0, 256, 256, 7, 131072]`，region-and-map.md：头部仅 256×256 与格式 7），
//!    经 `dbpf::write_uncompressed_overlay` 产出字节级对齐零售包的 overlay。
use std::collections::{HashMap, HashSet};

use dbpf::{OverlayEntry, Package, ResourceId};

use crate::region_map::SHARED_TILE_GRID;

/// 区域高度场边长（像素，1 px = 8 m → 4096 px = 32768 m 全幅）。
pub const REGION_FIELD_PX: usize = 4096;
/// 单 tile 边长（像素）。
pub const TILE_PX: usize = 256;
/// F0（Terrain Heightmap 16-bit）资源类型。
pub const F0_TYPE_ID: u32 = 0x03E4_21F0;
/// 金字塔层数：level 0 = 最粗（1 张根 tile）… level 4 = 最细（256 张，mip0）。
pub const PYRAMID_LEVELS: usize = 5;
/// 一组区域 tile 总数：16²+8²+4²+2²+1。
pub const TILES_PER_REGION: usize = 341;
/// tile 原始字节数：20 B 头 + 256² × u16 LE。
pub const TILE_BYTES: usize = 20 + TILE_PX * TILE_PX * 2;

/// 5 级 tile 实例排布。`levels[l]` 为 l 层的行优先网格：
/// `levels[0]` = 1×1（根），`levels[4]` = 16×16（== `SHARED_TILE_GRID`）。
#[derive(Debug, Clone)]
pub struct F0Pyramid {
    pub levels: Vec<Vec<Vec<u32>>>,
}

impl F0Pyramid {
    /// 遍历全部 (level, x, y, instance)。
    pub fn iter_tiles(&self) -> impl Iterator<Item = (usize, usize, usize, u32)> + '_ {
        self.levels.iter().enumerate().flat_map(|(l, grid)| {
            grid.iter().enumerate().flat_map(move |(y, row)| {
                row.iter().copied().enumerate().map(move |(x, inst)| (l, x, y, inst))
            })
        })
    }
}

/// 合成 tile 头（源 tile 缺失时的回退）：BE `[0, 256, 256, 7, 131072]`
/// （宽/高/格式 7/声明解压尺寸，与零售 tile 头语义一致）。
fn synthesized_header() -> [u8; 20] {
    let mut h = [0u8; 20];
    for (i, v) in [0u32, 256, 256, 7, (TILE_PX * TILE_PX * 2) as u32]
        .iter()
        .enumerate()
    {
        h[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    h
}

/// 2×2 box 降采样（sum/4 截断——引擎 mip 语义，tile_arrange 实证）。
fn downsample2(src: &[u16], w: usize) -> Vec<u16> {
    assert_eq!(w % 2, 0, "field width must be even");
    let hw = w / 2;
    let mut out = vec![0u16; hw * hw];
    for y in 0..hw {
        for x in 0..hw {
            let i = (2 * y) * w + 2 * x;
            out[y * hw + x] = ((u32::from(src[i])
                + u32::from(src[i + 1])
                + u32::from(src[i + w])
                + u32::from(src[i + w + 1]))
                / 4) as u16;
        }
    }
    out
}

/// 逐级降采样场，按层返回：返回值[l] = l 层的场（l=0 最粗 256² … l=4 最细 4096²）。
fn level_fields(heights: &[u16]) -> Vec<Vec<u16>> {
    let mut finest_first = vec![heights.to_vec()];
    for k in 1..PYRAMID_LEVELS {
        let w = REGION_FIELD_PX >> (k - 1);
        finest_first.push(downsample2(&finest_first[k - 1], w));
    }
    finest_first.into_iter().rev().collect()
}

/// 从源包收集某区域组全部 F0 tile（instance → 256² u16 载荷）。
fn collect_tiles(package: &Package, group: u32) -> Result<HashMap<u32, Vec<u16>>, String> {
    let mut tiles = HashMap::new();
    for e in package.entries() {
        if e.id.type_id != F0_TYPE_ID || e.id.group != group {
            continue;
        }
        let data = package.read(e).map_err(|err| err.to_string())?;
        if data.len() != TILE_BYTES {
            return Err(format!(
                "tile 0x{:08X} 尺寸 {} ≠ {TILE_BYTES}，非标准 F0 tile",
                e.id.instance,
                data.len()
            ));
        }
        tiles.insert(
            e.id.instance,
            data[20..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect(),
        );
    }
    Ok(tiles)
}

/// 按 mip0 网格拼装 4096² 区域高度场（1 px = 8 m，raw 值域）。
///
/// 缺失/非法的 mip0 tile 会报错；粗层级 tile 不参与拼装（其内容可由场导出）。
pub fn read_region_field(package: &Package, group: u32) -> Result<Vec<u16>, String> {
    let tiles = collect_tiles(package, group)?;
    let w = REGION_FIELD_PX;
    let mut field = vec![0u16; w * w];
    for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
        for (tx, &inst) in row.iter().enumerate() {
            let px = tiles.get(&inst).ok_or_else(|| {
                format!("mip0 tile 0x{inst:08X} 缺失，组 0x{group:08X} 不是标准区域地形")
            })?;
            for y in 0..TILE_PX {
                let dst = (ty * TILE_PX + y) * w + tx * TILE_PX;
                field[dst..dst + TILE_PX].copy_from_slice(&px[y * TILE_PX..(y + 1) * TILE_PX]);
            }
        }
    }
    Ok(field)
}

/// 由 4096² 高度场计算 l 层 (x, y) 处 tile 的 256² 内容
/// （l 层场边长 = 4096 >> (4-l)，tile 为其直接 256² 分块）。
fn tile_payload(level_field: &[u16], field_dim: usize, x: usize, y: usize) -> Vec<u16> {
    let mut out = vec![0u16; TILE_PX * TILE_PX];
    for r in 0..TILE_PX {
        let src = (y * TILE_PX + r) * field_dim + x * TILE_PX;
        out[r * TILE_PX..(r + 1) * TILE_PX]
            .copy_from_slice(&level_field[src..src + TILE_PX]);
    }
    out
}

/// 子采样 L1 误差（步长 8，每 tile 采样 32² 个点）——用于粗层级实例定位。
fn subsample_l1(a: &[u16], b: &[u16]) -> f64 {
    let mut acc = 0f64;
    let mut n = 0u32;
    for y in (0..TILE_PX).step_by(8) {
        for x in (0..TILE_PX).step_by(8) {
            acc += (f64::from(a[y * TILE_PX + x]) - f64::from(b[y * TILE_PX + x])).abs();
            n += 1;
        }
    }
    acc / f64::from(n)
}

/// 粗层级定位的误差阈值（均值/采样点）：真实 mip 滤波差异远小于此，
/// 随机错配则高 2–3 个数量级。
const MATCH_ERROR_THRESHOLD: f64 = 64.0;

/// 从源包求解 5 级 tile 实例排布。
///
/// mip0 直接取全局常量 [`SHARED_TILE_GRID`]（须全部在场）；其余 85 张粗层级
/// 实例按"源内容 vs 由 mip0 场逐级降采样的期望内容"做子采样 L1 最近匹配定位，
/// 要求一一对应且误差低于 [`MATCH_ERROR_THRESHOLD`]。
pub fn solve_f0_pyramid(package: &Package, group: u32) -> Result<F0Pyramid, String> {
    let tiles = collect_tiles(package, group)?;
    if tiles.len() != TILES_PER_REGION {
        return Err(format!(
            "组 0x{group:08X} 的 F0 tile 数量 {} ≠ {TILES_PER_REGION}，不是标准区域地形组",
            tiles.len()
        ));
    }
    // mip0 校验 + 拼场
    let mut mip0_set = HashSet::new();
    let mut field = vec![0u16; REGION_FIELD_PX * REGION_FIELD_PX];
    for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
        for (tx, &inst) in row.iter().enumerate() {
            let Some(px) = tiles.get(&inst) else {
                return Err(format!("mip0 tile 0x{inst:08X} 缺失"));
            };
            mip0_set.insert(inst);
            for y in 0..TILE_PX {
                let dst = (ty * TILE_PX + y) * REGION_FIELD_PX + tx * TILE_PX;
                field[dst..dst + TILE_PX].copy_from_slice(&px[y * TILE_PX..(y + 1) * TILE_PX]);
            }
        }
    }
    let fields = level_fields(&field); // fields[l]：l 层场（0=最粗）

    // 粗层级期望内容按 (level, x, y) 枚举
    let mut expected: Vec<(usize, usize, usize, Vec<u16>)> = Vec::new();
    for l in 0..(PYRAMID_LEVELS - 1) {
        let grid_dim = 1usize << l;
        let field_dim = REGION_FIELD_PX >> (PYRAMID_LEVELS - 1 - l);
        for y in 0..grid_dim {
            for x in 0..grid_dim {
                expected.push((l, x, y, tile_payload(&fields[l], field_dim, x, y)));
            }
        }
    }
    debug_assert_eq!(expected.len(), TILES_PER_REGION - 256);

    // 未知实例 → 期望位置 最近匹配
    let unknown: Vec<u32> = tiles
        .keys()
        .copied()
        .filter(|i| !mip0_set.contains(i))
        .collect();
    let mut assigned: HashMap<u32, (usize, usize, usize)> = HashMap::new();
    let mut taken: HashSet<(usize, usize, usize)> = HashSet::new();
    for &inst in &unknown {
        let px = &tiles[&inst];
        let mut best: Option<(usize, usize, usize, f64)> = None;
        for (l, x, y, exp) in &expected {
            if taken.contains(&(*l, *x, *y)) {
                continue;
            }
            let e = subsample_l1(px, exp);
            if e < best.as_ref().map(|b| b.3).unwrap_or(f64::INFINITY) {
                best = Some((*l, *x, *y, e));
            }
        }
        let Some((l, x, y, e)) = best else {
            return Err(format!("tile 0x{inst:08X} 无可分配的粗层级槽位"));
        };
        if e > MATCH_ERROR_THRESHOLD {
            return Err(format!(
                "tile 0x{inst:08X} 与全部粗层级槽位的误差 {e:.2} 超过阈值 {MATCH_ERROR_THRESHOLD}，内容与排布不符"
            ));
        }
        assigned.insert(inst, (l, x, y));
        taken.insert((l, x, y));
    }
    if assigned.len() != expected.len() {
        return Err(format!(
            "粗层级槽位分配不完整：{}/{}",
            assigned.len(),
            expected.len()
        ));
    }

    let mut levels = Vec::with_capacity(PYRAMID_LEVELS);
    for l in 0..PYRAMID_LEVELS {
        let dim = 1usize << l;
        levels.push(vec![vec![0u32; dim]; dim]);
    }
    for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
        for (tx, &inst) in row.iter().enumerate() {
            levels[PYRAMID_LEVELS - 1][ty][tx] = inst;
        }
    }
    for (&inst, &(l, x, y)) in &assigned {
        levels[l][y][x] = inst;
    }
    Ok(F0Pyramid { levels })
}

/// 编码一张 F0 tile：20 B 头（优先复用源包原字节）+ u16 LE 载荷。
fn encode_tile(header: Option<&[u8]>, payload: &[u16]) -> Vec<u8> {
    let mut data = Vec::with_capacity(TILE_BYTES);
    match header {
        Some(h) if h.len() == 20 => data.extend_from_slice(h),
        _ => data.extend_from_slice(&synthesized_header()),
    }
    for v in payload {
        data.extend_from_slice(&v.to_le_bytes());
    }
    data
}

/// 由 4096² 高度场重建整组 341 张 tile 并生成未压缩 overlay 包字节。
///
/// 源包用于求解排布（[`solve_f0_pyramid`]）并复用各 tile 原始 20 B 头；
/// `heights` 为行优先 4096² u16 raw 高度场（raw = (z+1024)×32，1 px = 8 m）。
pub fn build_heightmap_overlay(
    package: &Package,
    group: u32,
    heights: &[u16],
) -> Result<Vec<u8>, String> {
    if heights.len() != REGION_FIELD_PX * REGION_FIELD_PX {
        return Err(format!(
            "高度场 {} px ≠ {}×{}",
            heights.len(),
            REGION_FIELD_PX,
            REGION_FIELD_PX
        ));
    }
    let pyramid = solve_f0_pyramid(package, group)?;
    // 原始头复用表
    let mut headers: HashMap<u32, [u8; 20]> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id != F0_TYPE_ID || e.id.group != group {
            continue;
        }
        if let Ok(data) = package.read(e) {
            if data.len() == TILE_BYTES {
                let mut h = [0u8; 20];
                h.copy_from_slice(&data[..20]);
                headers.insert(e.id.instance, h);
            }
        }
    }
    let fields = level_fields(heights);
    let mut entries = Vec::with_capacity(TILES_PER_REGION);
    for (l, x, y, inst) in pyramid.iter_tiles() {
        let field_dim = REGION_FIELD_PX >> (PYRAMID_LEVELS - 1 - l);
        let payload = tile_payload(&fields[l], field_dim, x, y);
        let data = encode_tile(headers.get(&inst).map(|h| h.as_slice()), &payload);
        entries.push(OverlayEntry::new(
            ResourceId {
                type_id: F0_TYPE_ID,
                group,
                instance: inst,
            },
            data,
        ));
    }
    dbpf::write_uncompressed_overlay(&entries).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region_map::list_regions;

    /// 确定性合成高度场：远海 0 + 平滑丘陵 + 细纹理，覆盖大落差与边界。
    fn synthetic_field() -> Vec<u16> {
        let w = REGION_FIELD_PX;
        let mut f = vec![0u16; w * w];
        for y in 0..w {
            for x in 0..w {
                let v = if x < 512 && y < 512 {
                    0u16 // 远海（void）
                } else {
                    let smooth =
                        ((x as f64 / 512.0).sin() * (y as f64 / 384.0).cos() * 0.5 + 0.5) * 6000.0;
                    let fine = (((x * 7) ^ (y * 13)) % 97) as f64;
                    (smooth + fine).min(65_000.0) as u16
                };
                f[y * w + x] = v;
            }
        }
        f
    }

    /// 与 SHARED_TILE_GRID 不冲突的确定性粗层级实例 id。
    fn coarse_instances() -> Vec<u32> {
        let mut mip0: HashSet<u32> = HashSet::new();
        for row in SHARED_TILE_GRID.iter() {
            for v in row.iter() {
                mip0.insert(*v);
            }
        }
        (0..85u32)
            .map(|i| 0x7A00_0001 + i)
            .filter(|id| !mip0.contains(id))
            .collect()
    }

    fn write_synthetic_region_package(path: &std::path::Path, heights: &[u16]) -> Vec<u32> {
        let coarse = coarse_instances();
        let fields = level_fields(heights); // [0]=最粗
        let mut entries = Vec::new();
        // 粗层级：任意指定排布（与求解结果互证）
        let mut idx = 0usize;
        for l in 0..(PYRAMID_LEVELS - 1) {
            let grid_dim = 1usize << l;
            let field_dim = REGION_FIELD_PX >> (PYRAMID_LEVELS - 1 - l);
            for y in 0..grid_dim {
                for x in 0..grid_dim {
                    let inst = coarse[idx];
                    idx += 1;
                    let payload = tile_payload(&fields[l], field_dim, x, y);
                    entries.push(OverlayEntry::new(
                        ResourceId {
                            type_id: F0_TYPE_ID,
                            group: 0x5A5A_0000,
                            instance: inst,
                        },
                        encode_tile(None, &payload),
                    ));
                }
            }
        }
        // mip0：按全局常量排布
        let field_dim = REGION_FIELD_PX;
        for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
            for (tx, &inst) in row.iter().enumerate() {
                let payload = tile_payload(&fields[PYRAMID_LEVELS - 1], field_dim, tx, ty);
                entries.push(OverlayEntry::new(
                    ResourceId {
                        type_id: F0_TYPE_ID,
                        group: 0x5A5A_0000,
                        instance: inst,
                    },
                    encode_tile(None, &payload),
                ));
            }
        }
        dbpf::write_uncompressed_overlay_to_path(path, &entries).expect("write synthetic");
        coarse
    }

    fn decode_tile(data: &[u8]) -> Vec<u16> {
        data[20..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    #[test]
    fn downsample2_matches_box_average() {
        let src: Vec<u16> = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 5, 6, 7, 8];
        let out = downsample2(&src, 4);
        assert_eq!(out, vec![(10 + 20 + 50 + 60) / 4, (30 + 40 + 70 + 80) / 4, (90 + 100 + 5 + 6) / 4, (110 + 120 + 7 + 8) / 4]);
    }

    /// 合成区域整链路回归：写包 → 求解排布 → 重建 overlay → 回读逐 tile 比对。
    #[test]
    fn synthetic_region_roundtrip() {
        let heights = synthetic_field();
        let fields = level_fields(&heights);
        let dir = std::env::temp_dir().join(format!("openscp-region-write-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src_path = dir.join("synthetic-region.package");
        let coarse = write_synthetic_region_package(&src_path, &heights);
        let package = dbpf::Package::open(&src_path).expect("open synthetic");

        // 求解：排布必须与构造一致
        let pyramid = solve_f0_pyramid(&package, 0x5A5A_0000).expect("solve");
        let mut idx = 0usize;
        for l in 0..(PYRAMID_LEVELS - 1) {
            let grid_dim = 1usize << l;
            for y in 0..grid_dim {
                for x in 0..grid_dim {
                    assert_eq!(pyramid.levels[l][y][x], coarse[idx], "level {l} ({x},{y})");
                    idx += 1;
                }
            }
        }
        for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
            for (tx, &inst) in row.iter().enumerate() {
                assert_eq!(pyramid.levels[4][ty][tx], inst);
            }
        }

        // 重建 overlay 并回读比对
        let started = std::time::Instant::now();
        let bytes = build_heightmap_overlay(&package, 0x5A5A_0000, &heights).expect("build");
        let overlay_path = dir.join("synthetic-overlay.package");
        std::fs::write(&overlay_path, &bytes).unwrap();
        let overlay = dbpf::Package::open(&overlay_path).expect("open overlay");
        assert_eq!(overlay.entries().len(), TILES_PER_REGION);
        let mut checked = 0usize;
        for (l, x, y, inst) in pyramid.iter_tiles() {
            let e = overlay
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && e.id.group == 0x5A5A_0000)
                .expect("overlay entry");
            let data = overlay.read(e).unwrap();
            assert_eq!(data.len(), TILE_BYTES);
            let field_dim = REGION_FIELD_PX >> (PYRAMID_LEVELS - 1 - l);
            assert_eq!(
                decode_tile(&data),
                tile_payload(&fields[l], field_dim, x, y),
                "tile l{l} ({x},{y}) 内容不一致"
            );
            checked += 1;
        }
        assert_eq!(checked, TILES_PER_REGION);
        println!(
            "[perf region_write] 合成区域 341 tile 重建 overlay {} bytes，耗时 {:?}",
            bytes.len(),
            started.elapsed()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 真包回归（env 门控）：设 OPENSCP_REGION_TERRAIN_PACKAGE 指向
    /// SimCity_RegionTerrain0.package 后运行。校验：mip0 拼装 → 重建 overlay
    /// → 回读，最细层与源包逐值一致，粗层级均值误差 < 8（引擎滤波近似）。
    #[test]
    fn real_region_regen_roundtrip() {
        let Some(path) = std::env::var_os("OPENSCP_REGION_TERRAIN_PACKAGE") else {
            eprintln!("skip：未设置 OPENSCP_REGION_TERRAIN_PACKAGE");
            return;
        };
        let package = dbpf::Package::open(std::path::PathBuf::from(&path)).expect("open real");
        let Some(region) = list_regions(&package).first().cloned() else {
            panic!("真包中未找到区域");
        };
        let group = region.group;
        let started = std::time::Instant::now();
        let field = read_region_field(&package, group).expect("read field");
        let bytes = build_heightmap_overlay(&package, group, &field).expect("build");
        println!(
            "[perf region_write] 真实区域 0x{group:08X}（{}）重建 overlay {} bytes，耗时 {:?}",
            region.display_name.unwrap_or_default(),
            bytes.len(),
            started.elapsed()
        );
        let dir = std::env::temp_dir().join(format!("openscp-region-real-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let overlay_path = dir.join("real-overlay.package");
        std::fs::write(&overlay_path, &bytes).unwrap();
        let overlay = dbpf::Package::open(&overlay_path).expect("open overlay");
        let source_tiles = collect_tiles(&package, group).unwrap();
        let mut worst = 0f64;
        for e in overlay.entries() {
            let data = overlay.read(e).unwrap();
            let payload = decode_tile(&data);
            let src = &source_tiles[&e.id.instance];
            let err: f64 = payload
                .iter()
                .zip(src.iter())
                .map(|(a, b)| (f64::from(*a) - f64::from(*b)).abs())
                .sum::<f64>()
                / payload.len() as f64;
            worst = worst.max(err);
        }
        println!("[perf region_write] 真包回读最差均值误差 {worst:.3}");
        assert!(worst < 8.0, "粗层级与源包偏差过大：{worst}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
