//! 拼合 16×16 mosaic，用城市地块（已知世界坐标）窗口比对：验证模型 + 定向。仅开发用。
//!
//! 用法：tile_orient <package> <region-group>
use std::collections::HashMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let group = u32::from_str_radix(&args.next().expect("group").trim_start_matches("0x"), 16).unwrap();
    let package = dbpf::Package::open(&path).expect("open");

    // 1. 读全部 F0（区域组 + 各城市地块组）
    let mut region_tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    let mut city_maps: HashMap<u32, Vec<u16>> = HashMap::new(); // group -> px（单条城市组）
    let mut city_group_size: HashMap<u32, usize> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id != 0x03E4_21F0 {
            continue;
        }
        let data = package.read(e).ok();
        let Some(data) = data else { continue };
        if data.len() < 20 {
            continue;
        }
        let px: Vec<u16> = data[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        if px.len() != 65536 {
            continue;
        }
        if e.id.group == group {
            region_tiles.insert(e.id.instance, px);
        } else {
            *city_group_size.entry(e.id.group).or_default() += 1;
            city_maps.insert(e.id.group, px);
        }
    }
    let cities: Vec<u32> = city_group_size.iter().filter(|(_, n)| **n == 1).map(|(g, _)| *g).collect();
    println!("region tiles: {}, single-tile city groups: {}", region_tiles.len(), cities.len());
    if region_tiles.len() != 341 {
        println!("WARN: expected 341 region tiles");
    }

    // 2. 金字塔匹配（与 tile_arrange 相同逻辑，压缩版）
    let insts: Vec<u32> = region_tiles.keys().copied().collect();
    let _ = insts; // 排布用内嵌表（tile_arrange 的输出），本工具不再重复匹配
    // —— 内嵌 tile_arrange 已还原的排布（BC357A2B）；用于其他区域请先跑 tile_arrange 并替换。
    const GRID: [[u32; 16]; 16] = [
        [0x7ADCBE8C,0x5D292B9B,0xA7C1AE16,0x1E279CE5,0xA12CF3A0,0x6854893F,0xF92D745A,0x5A72E609,0xED4A03E4,0x453A7813,0x2103B675,0x57D87926,0x834E206B,0x04BDD09C,0x82FD4959,0x9BE696EA],
        [0x4766E36B,0xF9197D3C,0x25152B55,0x7BB03F26,0x1C7CC92F,0xC605CDF0,0x410980D9,0xABB3C34A,0xE8112423,0x4A192C94,0xA3B03936,0xFA4FD6E5,0xB6C3FB8C,0x52AF89FB,0x3B213CDA,0x315904A9],
        [0xAD4DAB7A,0x435C67E9,0xAB991ED8,0xD2EF6037,0x38CB831E,0x4C37186D,0xDA61A76C,0x5E21B4BB,0xADFF3472,0x77AB6481,0xA98C6847,0xAF4678E8,0x47307D79,0xB98CBA8A,0xD53C07CB,0x9F0FEAFC],
        [0x0B494079,0xADE8672A,0xC3BEF927,0xD31E3EE8,0x51C249DD,0x1AEB98AE,0x93483F4B,0xC82B5F5C,0xE2133BD1,0x466A5E62,0x91668E78,0xAF179A37,0xE934E87A,0x517B2449,0x1C556FEC,0x1C6DE15B],
        [0x710404B8,0xC4AF6B77,0xEB0FF21A,0x6D637AA9,0x73BBA1B4,0xB7E1AB83,0x89A1B256,0x0B370845,0x0C566190,0x3AE0830F,0x2BAD5D39,0xB41946EA,0xC78B8487,0xB4BB7F88,0x48E8F815,0x3FA5C926],
        [0x8BA44787,0xA9159928,0x45DE5A99,0xD7F10CEA,0x3F70AFB3,0xA3CC4344,0x06F52F95,0x551A8B06,0xC4C4A55F,0xBD164480,0xE6FCEA3A,0x498BB4A9,0xACEB41B8,0xE6734757,0xCB957AD6,0xE21D26E5],
        [0x59890776,0x3C252625,0xCEBE8E2C,0x5878575B,0x59ED4A82,0x8275F331,0x8AFD2718,0xAC59AC17,0x780E784E,0x2A9A3E3D,0x8117B52B,0xB7429AFC,0x1047C5B5,0x8F342646,0x00DDE7E7,0x9713C8E8],
        [0xD46088B5,0x6D6EB766,0x6BDC750B,0xDB1A60FC,0x5BC213A1,0x67AEA052,0xBEEA1F67,0xA6E0F548,0x9287AC0D,0x3E954BFE,0xDE50A54C,0x34A0915B,0x95704476,0x2E7CC485,0xCCF0EF98,0x96E4EA37],
        [0xA6040CC4,0xA1677C33,0x6E8AC84E,0x9834631D,0x471ECCA8,0x896C7E07,0x9FA6AAB2,0xBEB168C1,0xD8423E2C,0xEADE327B,0x9B107CAD,0x58EB6D5E,0xF600B203,0xB1EA8414,0xF9E8EF91,0x9093FEC2],
        [0xBA197503,0xD5B26E34,0x8903FC0D,0x7CC3335E,0x62B89EF7,0x6ECC3B38,0xB7F52711,0xB9AF73A2,0x7560250B,0x9CEC791C,0x8098DBEE,0x745C9D1D,0xE1EB49C4,0xAD0BCF93,0xE19A7332,0xABB57CE1],
        [0x02C34549,0xB0401F16,0x54AABA9B,0x96187398,0xBAAD9CED,0xDD7B162A,0x9D5F5BEF,0xD28C5F4C,0xBEBE1F21,0x0409E9AE,0x40CFF468,0xEF8E39AB,0xDB206CE6,0x2521DD59,0x62993C9C,0x0D7354FF],
        [0x6AD4DB8A,0x2D939C55,0xF09B0C3C,0xCA056BE7,0x89621D2E,0x72EF16E9,0x1F96B060,0x75536F2B,0xBCE95602,0x3555696D,0x40A115B7,0x4CC729CC,0xA9D6DBA5,0xDD45D0DA,0xB08AF5FB,0x5830AAB0],
        [0x03F7AAFB,0xB4178FD8,0x3ADDF6E9,0x94BCFED6,0xD6CB0DBF,0x28AC2C3C,0x8D19169D,0xDB38A43A,0xA26CBB33,0xAF241EF0,0xE961F4A6,0x9A23E1B9,0x16C74EA8,0x77609BCB,0x1768268A,0xF154512D],
        [0xB605F19C,0xCC3D6A27,0xA569F62A,0x12107C15,0x347C5270,0x8CBBDA9B,0x71A7E6DE,0x1FE91739,0xD6B7AD34,0x5172DA3F,0x8BD95265,0x55736EBA,0x326120F7,0xBE7A03EC,0xAF569049,0xD6DCB06E],
        [0xDFC4E585,0xF38E631A,0xBC30FA77,0xE57CF3EC,0xF0EC77B1,0x9D016666,0xBDD8D973,0x85A24878,0xB4BE555D,0x50CCF152,0x48CC167C,0x1802ECC7,0x1B9A1CAA,0xEB0D8C15,0x1296EB88,0x2E456A43],
        [0x407C4746,0x4E5CCB99,0xA0972828,0x9E638BCB,0xD62524D2,0x6BB7D525,0xAC980574,0x9DC82247,0x9BC78E9E,0x6B944431,0xC62A0CDB,0xFFDD12F8,0xB10E1D69,0x6DBA0ED6,0x444EB357,0x35F8B304],
    ];

    // 3. 拼合 4096×4096
    let w = 4096usize;
    let mut mosaic = vec![0u16; w * w];
    for (ty, row) in GRID.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            let Some(px) = region_tiles.get(inst) else {
                println!("missing tile 0x{inst:08X}");
                continue;
            };
            for y in 0..256 {
                for x in 0..256 {
                    mosaic[(ty * 256 + y) * w + tx * 256 + x] = px[y * 256 + x];
                }
            }
        }
    }

    // 4. 地块表（位置）
    let plot_entry = package.entries().iter()
        .find(|e| e.id.type_id == 0x00B1_B104 && e.id.instance == 0x2B9C_480C)
        .expect("plot table");
    let plot_table = sc_properties::PropertyFile::parse(&package.read(plot_entry).unwrap()).unwrap();
    let poss: Vec<(f32, f32)> = match &plot_table.get(0xF01D_E4B1).unwrap().kind {
        sc_properties::Kind::Array(vs) => vs.iter().filter_map(|v| match v {
            sc_properties::Value::Vector2(v) => Some((v[0], v[1])), _ => None }).collect(),
        _ => vec![],
    };
    let ids: Vec<u32> = match &plot_table.get(0x16B7_B1EF).unwrap().kind {
        sc_properties::Kind::Array(vs) => vs.iter().filter_map(|v| match v {
            sc_properties::Value::UInt32(x) => Some(*x), _ => None }).collect(),
        _ => vec![],
    };

    // 5. 每城市：世界→mosaic 窗口（两种 y 约定）比对
    // 候选变换（半幅 H=16384，格 8m）：
    //   A: col=(wx+H)/8, row=(wy+H)/8   （世界 +y = 图向下）
    //   B: col=(wx+H)/8, row=(H-wy)/8   （世界 +y = 图向上）
    // 另试 x/y 交换与镜像共 8 种，取最优。
    for (n, (id, pos)) in ids.iter().zip(&poss).enumerate() {
        let Some(city) = city_maps.get(id) else { println!("city #{n} group 0x{id:08X}: no map"); continue };
        let mut best = (f64::INFINITY, 0u8, 0i64, 0i64, 0i64, 0i64);
        for flip_x in [false, true] {
            for swap in [false, true] {
                for ymode in [0, 1] {
                    let wx = pos.0; let wy = pos.1;
                    let (mut c0f, mut r0f) = ((wx + 16384.0) / 8.0, if ymode == 0 { (wy + 16384.0) / 8.0 } else { (16384.0 - wy) / 8.0 });
                    if swap { std::mem::swap(&mut c0f, &mut r0f); }
                    let c0 = c0f as i64; let r0 = r0f as i64;
                    // 允许 ±4 格搜平移
                    for dr in -4i64..=4 {
                        for dc in -4i64..=4 {
                            let (c, r) = (c0 + dc, r0 + dr);
                            if c < 0 || r < 0 || c + 256 > 4096 || r + 256 > 4096 { continue; }
                            let mut sum = 0f64;
                            for y in 0..256 {
                                for x in 0..256 {
                                    sum += mosaic[(((r + y as i64) * 4096 + (c + x as i64))) as usize] as f64;
                                }
                            }
                            let mean_m = sum / 65536.0;
                            let mean_c: f64 = city.iter().map(|v| f64::from(*v)).sum::<f64>() / 65536.0;
                            let mut e = 0f64;
                            for y in 0..256 {
                                for x in 0..256 {
                                    let mv = mosaic[(((r + y as i64) * 4096 + (c + x as i64))) as usize] as f64 - mean_m;
                                    let cv = city[(y * 256 + x) as usize] as f64 - mean_c;
                                    e += (mv - cv).abs();
                                }
                            }
                            e /= 65536.0;
                            if e < best.0 { best = (e, (ymode * 4 + (swap as u8) * 2 + (flip_x as u8)) as u8, c, r, dc, dr); }
                        }
                    }
                }
            }
        }
        println!("city #{n} group=0x{id:08X} pos=({:.0},{:.0}): best err={:.2} mode={:08b} col={} row={} (dc={},dr={})", pos.0, pos.1, best.0, best.1, best.2, best.3, best.4, best.5);
    }
}
