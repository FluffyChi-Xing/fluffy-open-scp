//! 地图机制调查探针（只读）：
//!   survey <package>              —— 按类型统计条目（数量/字节量/代表 TGI）
//!   dump   <package> <type-hex>   —— 打印某类型全部条目（TGI + 尺寸 + 头部 hex）
//!   props  <package>              —— 全量打印包内 0x00B1B104 property 键值摘要
//! 用于 SimCity_RegionTerrain*.package / 模组覆盖包的地图机制取证。

use dbpf::Package;
use sc_properties::{PropertyFile, Value};
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args[0].as_str() {
        "survey" => survey(&args[1]),
        "dump" => dump(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap()),
        "props" => props(&args[1]),
        // hstats <package> <type-hex> —— u16 高度图逐条 min/max/avg（跳 20 字节头）
        "hstats" => hstats(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap()),
        // extract <package> <instance-hex> <out> —— 按 instance 提取第一条资源
        "extract" => extract(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap(), &args[3]),
        // tilecheck <package> <instA> <instB> —— 检验两张 256×256 u16 高度图的边缘连续性
        "tilecheck" => tilecheck(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(&args[3].trim_start_matches("0x"), 16).unwrap()),
        // full <package> <instance> —— 打印 property 的完整数组（不截断）
        "full" => full(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap()),
        // refs <package> <region-group-hex> —— 区域内全部画刷 property 记账
        "refs" => refs(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap()),
        // matchstate <package> <region-group-hex> <state.bin> —— 在城市状态中定位区域 tile 行
        "matchstate" => matchstate(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap(), &args[3]),
        // jigsaw <package> <region-group-hex> <type-hex> <out.bmp> [tile-instance...] —— 边缘匹配自动拼合
        "jigsaw" => {
            let type_hex = u32::from_str_radix(args[3].trim_start_matches("0x"), 16).unwrap();
            jigsaw(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap(), &args[4], type_hex)
        }
        other => panic!("unknown subcommand {other}"),
    }
}

fn jigsaw(path: &str, group: u32, out: &str, type_hex: u32) {
    let package = open(path);
    // 收集该区域 group 的全部 256×256 u16 tile
    let mut tiles: Vec<(u32, Vec<u16>)> = Vec::new();
    for entry in package.entries().iter().filter(|e| e.id.group == group && e.id.type_id == type_hex) {
        if let Ok(data) = package.read(entry) {
            if data.len() >= 20 && (data.len() - 20) % 2 == 0 {
                tiles.push((
                    entry.id.instance,
                    data[20..]
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect(),
                ));
            }
        }
    }
    println!("tiles = {}", tiles.len());
    if tiles.is_empty() { return; }
    let n = (tiles[0].1.len() as f64).sqrt().floor() as usize; // 256
    let edge = |cells: &[u16], side: u8| -> Vec<u16> {
        (0..n)
            .map(|i| match side {
                0 => cells[i * n],              // left
                1 => cells[i * n + n - 1],      // right
                2 => cells[i],                  // top
                _ => cells[(n - 1) * n + i],    // bottom
            })
            .collect()
    };
    let edges: Vec<[Vec<u16>; 4]> = tiles
        .iter()
        .map(|(_, c)| [edge(c, 0), edge(c, 1), edge(c, 2), edge(c, 3)])
        .collect();
    let diff = |x: &[u16], y: &[u16]| -> f64 {
        x.iter()
            .zip(y)
            .map(|(p, q)| (*p as i64 - *q as i64).unsigned_abs() as f64)
            .sum::<f64>()
            / x.len() as f64
    };
    // 每块每侧找最佳邻居，再做互为最优（mutual best-match）过滤抑制噪声
    let mut best_right: Vec<Option<(usize, f64)>> = vec![None; tiles.len()];
    let mut best_bottom: Vec<Option<(usize, f64)>> = vec![None; tiles.len()];
    // 反向索引：left[b] = 差值最小的 a（即 a.right 接 b.left）；top 同理
    let mut best_left: Vec<Option<(usize, f64)>> = vec![None; tiles.len()];
    let mut best_top: Vec<Option<(usize, f64)>> = vec![None; tiles.len()];
    for a in 0..tiles.len() {
        for b in 0..tiles.len() {
            if a == b { continue; }
            let dr = diff(&edges[a][1], &edges[b][0]);
            let db = diff(&edges[a][3], &edges[b][2]);
            if best_right[a].map_or(true, |(_, s)| dr < s) { best_right[a] = Some((b, dr)); }
            if best_left[b].map_or(true, |(_, s)| dr < s) { best_left[b] = Some((a, dr)); }
            if best_bottom[a].map_or(true, |(_, s)| db < s) { best_bottom[a] = Some((b, db)); }
            if best_top[b].map_or(true, |(_, s)| db < s) { best_top[b] = Some((a, db)); }
        }
    }
    let mutual = |a: usize, side: u8, b: usize| -> bool {
        match side {
            1 => best_right[a].map_or(false, |(x, _)| x == b) && best_left[b].map_or(false, |(x, _)| x == a),
            _ => best_bottom[a].map_or(false, |(x, _)| x == b) && best_top[b].map_or(false, |(x, _)| x == a),
        }
    };
    // 从「右邻居缺失」的块（最右列）反推：选被引用为 right 次数最少的种子
    let mut pos: std::collections::HashMap<usize, (i64, i64)> = std::collections::HashMap::new();
    let seed = (0..tiles.len())
        .min_by_key(|&a| {
            let mut cnt = 0i32;
            for b in 0..tiles.len() {
                if let Some((x, _)) = best_right[b] { if x == a { cnt += 1; } }
            }
            cnt
        })
        .unwrap();
    pos.insert(seed, (0, 0));
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(seed);
    while let Some(a) = queue.pop_front() {
        let (ax, ay) = pos[&a];
        for b in 0..tiles.len() {
            if !pos.contains_key(&b) {
                if mutual(a, 1, b) { pos.insert(b, (ax + 1, ay)); queue.push_back(b); }
                else if mutual(a, 3, b) { pos.insert(b, (ax, ay + 1)); queue.push_back(b); }
                else if mutual(b, 1, a) { pos.insert(b, (ax - 1, ay)); queue.push_back(b); }
                else if mutual(b, 3, a) { pos.insert(b, (ax, ay - 1)); queue.push_back(b); }
            }
        }
    }
    let mut xs: Vec<i64> = pos.values().map(|p| p.0).collect();
    let mut ys: Vec<i64> = pos.values().map(|p| p.1).collect();
    xs.sort_unstable(); ys.sort_unstable();
    println!(
        "placed={} grid x[{},{}) y[{},{}])",
        pos.len(), xs[0], xs[xs.len() - 1] + 1, ys[0], ys[ys.len() - 1] + 1
    );
    // 渲染 BMP（24bit BGR，灰度归一化）
    let minx = *xs.first().unwrap(); let maxx = *xs.last().unwrap();
    let miny = *ys.first().unwrap(); let maxy = *ys.last().unwrap();
    let w = ((maxx - minx + 1) as usize) * n;
    let h = ((maxy - miny + 1) as usize) * n;
    let mut canvas = vec![0u16; w * h];
    let mut placed_mask = vec![false; w * h];
    for (idx, (gx, gy)) in &pos {
        let (_, cells) = &tiles[*idx];
        let ox = ((gx - minx) as usize) * n;
        let oy = ((gy - miny) as usize) * n;
        for y in 0..n {
            for x in 0..n {
                canvas[(oy + y) * w + ox + x] = cells[y * n + x];
                placed_mask[(oy + y) * w + ox + x] = true;
            }
        }
    }
    let mut minv = u16::MAX; let mut maxv = 0u16;
    for (v, m) in canvas.iter().zip(&placed_mask) { if *m { minv = minv.min(*v); maxv = maxv.max(*v); } }
    // 打印指定 instance 的网格坐标（世界尺寸标定：std::env args[5..] 传入）
    for arg in std::env::args().skip(5) {
        let Ok(inst) = u32::from_str_radix(arg.trim_start_matches("0x"), 16) else { continue };
        for (i, (gx, gy)) in &pos {
            if tiles[*i].0 == inst {
                println!("TILEPOS 0x{inst:08X} grid=({gx},{gy}) tile_idx={i}");
            }
        }
    }
    let span = (maxv - minv).max(1) as f32;
    let row_pad = (4 - (w * 3) % 4) % 4;
    let data_size = (h * (w * 3 + row_pad)) as u32;
    let mut bmp = Vec::new();
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(54 + data_size).to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(w as i32).to_le_bytes());
    bmp.extend_from_slice(&(h as i32).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&24u16.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 24]);
    for y in (0..h).rev() {
        for x in 0..w {
            let i = y * w + x;
            let v = if placed_mask[i] {
                (((canvas[i] - minv) as f32 / span) * 255.0) as u8
            } else { 40 };
            bmp.extend_from_slice(&[v, v, v]);
        }
        for _ in 0..row_pad { bmp.push(0); }
    }
    std::fs::write(out, &bmp).unwrap();
    println!("mosaic written -> {out} ({}x{}, range {minv}..{maxv})", w, h);
}

fn refs(path: &str, region_group: u32) {
    let package = open(path);
    for entry in package
        .entries()
        .iter()
        .filter(|e| e.id.group == region_group && e.id.type_id == 0x00B1_B104)
    {
        let data = package.read(entry).unwrap_or_default();
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        let mut target = String::new();
        let mut res = 0u32;
        let mut map_index: i64 = -1;
        let mut stamps: Vec<(u32, f32, f32)> = Vec::new();
        for property in &file.values {
            match property.hash {
                0x0DBA3A9C => {
                    if let sc_properties::Kind::Scalar(Value::String8(s)) = &property.kind {
                        target = s.clone();
                    }
                }
                0x0DC097E3 => {
                    if let sc_properties::Kind::Scalar(Value::UInt32(v)) = &property.kind {
                        res = *v;
                    }
                }
                0x0DE43899 => {
                    if let sc_properties::Kind::Scalar(Value::UInt32(v)) = &property.kind {
                        map_index = *v as i64;
                    }
                }
                0x02A907B5 => {
                    if let sc_properties::Kind::Array(values) = &property.kind {
                        for (i, v) in values.iter().enumerate() {
                            if let Value::Key(k) = v {
                                if stamps.len() <= i { stamps.resize(i + 1, (0, 0.0, 0.0)); }
                                stamps[i].0 = k.instance;
                            }
                        }
                    }
                }
                0x02A907B6 => {
                    if let sc_properties::Kind::Array(values) = &property.kind {
                        for (i, v) in values.iter().enumerate() {
                            if let Value::Transform(t) = v {
                                if t.matrix.len() >= 11 {
                                    if stamps.len() <= i { stamps.resize(i + 1, (0, 0.0, 0.0)); }
                                    stamps[i].1 = t.matrix[9];
                                    stamps[i].2 = t.matrix[10];
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if !stamps.is_empty() {
            println!(
                "== prop {:08X} target={target} res={res} mapidx={map_index} stamps={}",
                entry.id.instance, stamps.len()
            );
            for (instance, tx, ty) in &stamps {
                println!("   stamp 0x{instance:08X} at ({tx:.2}, {ty:.2})");
            }
        }
    }
}

fn full(path: &str, instance: u32) {
    let package = open(path);
    let mut found = 0usize;
    for entry in package.entries().iter() {
        if entry.id.instance != instance { continue; }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        found += 1;
        println!("== {:08X}:{:08X}:{:08X} ==", entry.id.type_id, entry.id.group, entry.id.instance);
        for property in &file.values {
            println!("  0x{:08X} {}", property.hash, property.prop_type.name());
            match &property.kind {
                sc_properties::Kind::Scalar(v) => println!("    {} {}", property.prop_type.name(), value_summary(v)),
                sc_properties::Kind::Array(values) => {
                    println!("    [{}] {}", values.len(), property.prop_type.name());
                    for (i, v) in values.iter().enumerate() {
                        println!("      [{i}] {}", value_summary(v));
                    }
                }
                sc_properties::Kind::Empty => println!("    <empty>"),
            }
        }
    }
    if found == 0 { println!("instance {instance:08X} not found"); }
}

fn height_grid(path: &str, instance: u32) -> Option<(dbpf::ResourceId, Vec<u16>)> {
    let package = open(path);
    for entry in package.entries().iter() {
        if entry.id.instance == instance {
            let data = package.read(entry).ok()?;
            let cells: Vec<u16> = data[20..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            return Some((entry.id, cells));
        }
    }
    None
}

/// 相邻 tile 判定：A 右缘 vs B 左缘（及下缘/上缘）平均差显著小于随机边缘差。
fn tilecheck(path: &str, inst_a: u32, inst_b: u32) {
    let (ida, a) = height_grid(path, inst_a).expect("A missing");
    let (idb, b) = height_grid(path, inst_b).expect("B missing");
    let edge = |cells: &[u16], side: &str| -> Vec<u16> {
        (0..256)
            .map(|i| match side {
                "right" => cells[i * 256 + 255],
                "left" => cells[i * 256],
                "bottom" => cells[255 * 256 + i],
                _ => cells[i],
            })
            .collect()
    };
    let diff = |x: &[u16], y: &[u16]| -> f64 {
        x.iter().zip(y).map(|(p, q)| (*p as i64 - *q as i64).unsigned_abs() as u64).sum::<u64>() as f64 / 256.0
    };
    println!("A {:08X}  B {:08X}", ida.instance, idb.instance);
    println!("A.right vs B.left  avg|d| = {:.1}", diff(&edge(&a, "right"), &edge(&b, "left")));
    println!("A.left  vs B.right avg|d| = {:.1}", diff(&edge(&a, "left"), &edge(&b, "right")));
    println!("A.bottom vs B.top   avg|d| = {:.1}", diff(&edge(&a, "bottom"), &edge(&b, "top")));
    println!("A.top vs B.bottom   avg|d| = {:.1}", diff(&edge(&a, "top"), &edge(&b, "bottom")));
    println!("A.right vs A.left(自身对照) avg|d| = {:.1}", diff(&edge(&a, "right"), &edge(&a, "left")));
}

fn hstats(path: &str, type_id: u32) {
    let package = open(path);
    let mut shown = 0usize;
    for entry in package.entries().iter().filter(|e| e.id.type_id == type_id) {
        let Ok(data) = package.read(entry) else { continue };
        if data.len() < 20 || (data.len() - 20) % 2 != 0 { continue; }
        let cells: Vec<u16> = data[20..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let min = cells.iter().min().unwrap();
        let max = cells.iter().max().unwrap();
        let avg = cells.iter().map(|v| *v as u64).sum::<u64>() / cells.len() as u64;
        let nonzero = cells.iter().filter(|v| **v != 0).count();
        println!(
            "{:08X}:{:08X}:{:08X}  cells={} min={min} max={max} avg={avg} nonzero={nonzero}",
            entry.id.type_id, entry.id.group, entry.id.instance, cells.len()
        );
        shown += 1;
        if shown >= 8 { break; }
    }
}

fn extract(path: &str, instance: u32, out: &str) {
    let package = open(path);
    for entry in package.entries().iter() {
        if entry.id.instance == instance {
            let data = package.read(entry).unwrap();
            std::fs::write(out, &data).unwrap();
            println!("extracted {} bytes -> {out}", data.len());
            return;
        }
    }
    panic!("instance {instance:08X} not found");
}

fn open(path: &str) -> Package {
    Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"))
}

fn survey(path: &str) {
    let package = open(path);
    let mut by_type: BTreeMap<u32, (usize, u64, Option<dbpf::ResourceId>)> = BTreeMap::new();
    for entry in package.entries() {
        let slot = by_type.entry(entry.id.type_id).or_insert((0, 0, None));
        slot.0 += 1;
        slot.1 += u64::from(entry.decompressed_size);
        if slot.2.is_none() {
            slot.2 = Some(entry.id);
        }
    }
    println!("== {path} ==");
    let mut rows: Vec<_> = by_type.iter().collect();
    rows.sort_by_key(|(_, (count, size, _))| (*size, *count));
    for (type_id, (count, size, sample)) in rows.iter().rev() {
        println!(
            "type 0x{type_id:08X}  count={count}  bytes={size}  sample={:?}",
            sample.map(|id| format!(
                "{:08X}:{:08X}:{:08X}",
                id.type_id, id.group, id.instance
            )),
        );
    }
}

fn head_hex(data: &[u8], n: usize) -> String {
    data.iter()
        .take(n)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn dump(path: &str, type_id: u32) {
    let package = open(path);
    for entry in package.entries().iter().filter(|e| e.id.type_id == type_id) {
        let data = package.read(entry).unwrap_or_default();
        println!(
            "{:08X}:{:08X}:{:08X}  stored={} size={}  head={}",
            entry.id.type_id,
            entry.id.group,
            entry.id.instance,
            entry.stored_len(),
            entry.decompressed_size,
            head_hex(&data, 16)
        );
    }
}

fn value_summary(value: &Value) -> String {
    match value {
        Value::UInt32(v) => format!("u32 {v} (0x{v:08X})"),
        Value::Int32(v) => format!("i32 {v}"),
        Value::Float(v) => format!("f32 {v:.4}"),
        Value::Key(k) => format!("key {:08X}:{:08X}:{:08X}", k.type_id, k.group, k.instance),
        Value::Vector2(v) => format!("vec2 {:?}", v),
        Value::Vector3(v) => format!("vec3 {:?}", v),
        other => format!("{other}"),
    }
}

fn props(path: &str) {
    let package = open(path);
    for entry in package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x00B1_B104)
    {
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else {
            println!("-- {:08X}:{:08X}:{:08X} parse failed", entry.id.type_id, entry.id.group, entry.id.instance);
            continue;
        };
        println!(
            "== property {:08X}:{:08X}:{:08X} ({} props) ==",
            entry.id.type_id, entry.id.group, entry.id.instance,
            file.values.len()
        );
        for property in &file.values {
            match &property.kind {
                sc_properties::Kind::Scalar(v) => {
                    println!("  0x{:08X} {} = {}", property.hash, property.prop_type.name(), value_summary(v));
                }
                sc_properties::Kind::Array(values) => {
                    let preview: Vec<String> =
                        values.iter().take(8).map(value_summary).collect();
                    println!(
                        "  0x{:08X} {}[{}] = {}{}",
                        property.hash,
                        property.prop_type.name(),
                        values.len(),
                        preview.join(", "),
                        if values.len() > 8 { ", …" } else { "" }
                    );
                }
                sc_properties::Kind::Empty => println!("  0x{:08X} <empty>", property.hash),
            }
        }
    }
}

fn matchstate(package_path: &str, group: u32, state_path: &str) {
    let package = open(package_path);
    // 收集该区域 group 的 F0 tile：instance → 行数据（512B/行）
    let mut rows: std::collections::HashMap<[u8; 8], Vec<(u32, usize)>> = std::collections::HashMap::new();
    let mut tile_rows: std::collections::HashMap<u32, Vec<Vec<u8>>> = std::collections::HashMap::new();
    let mut order: Vec<u32> = Vec::new();
    for entry in package.entries().iter().filter(|e| e.id.group == group && e.id.type_id == 0x03E4_21F0) {
        let Ok(data) = package.read(entry) else { continue };
        if data.len() < 20 { continue; }
        let body = &data[20..];
        let mut rowvecs = Vec::new();
        for r in 0..(body.len() / 512) {
            let row = &body[r * 512..(r + 1) * 512];
            rows.entry(row[..8].try_into().unwrap()).or_default().push((entry.id.instance, r));
            rowvecs.push(row.to_vec());
        }
        tile_rows.insert(entry.id.instance, rowvecs);
        order.push(entry.id.instance);
    }
    let state = std::fs::read(state_path).unwrap();
    println!("state {} bytes, tiles {} rows", state.len(), tile_rows.values().map(|v| v.len()).sum::<usize>());
    let mut hits: std::collections::BTreeMap<u32, Vec<(usize, usize)>> = std::collections::BTreeMap::new();
    let mut off = 0usize;
    while off + 512 <= state.len() {
        let key: [u8; 8] = state[off..off + 8].try_into().unwrap();
        if let Some(list) = rows.get(&key) {
            for (inst, r) in list {
                let row = &tile_rows[inst][*r];
                if &state[off..off + 512] == row.as_slice() {
                    hits.entry(*inst).or_default().push((off, *r));
                    handled_marker(off);
                }
            }
        }
        off += 2; // u16 步进扫描
    }
    let mut total = 0usize;
    for (inst, list) in &hits {
        let min_off = list.iter().map(|h| h.0).min().unwrap();
        let max_off = list.iter().map(|h| h.0).max().unwrap();
        println!("tile {:08X}: {} 行命中, state偏移 {}..{}, 行号 {}..{}",
            inst, list.len(), min_off, max_off,
            list.iter().map(|(_, r)| *r).min().unwrap(),
            list.iter().map(|(_, r)| *r).max().unwrap());
        total += list.len();
    }
    println!("总命中行数: {total}");
}

fn handled_marker(_off: usize) {}
