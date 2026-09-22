//! 城市地块高度图 ↔ 区域背景 tile 锚定探针。仅开发用。
//! 用法：tile_anchor <package> <region-group>
// 步骤：
// 1. 找 region 描述（51E7A18D）→ 地块表 Key（0xFB7A85A0）
// 2. 地块表：名称(0543BC96)/地块id(16B7B1EF=单条group)/位置(F01DE4B1)
// 3. 每地块：读单条 group 的 F0 高度图，与区域 group 全部 341 tile 求 avg|Δ|
// 4. 打印每个地块的最佳匹配 tile instance 与次佳
use sc_properties::{Kind, PropertyFile};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let group = u32::from_str_radix(&args.next().expect("group").trim_start_matches("0x"), 16).unwrap();
    let package = dbpf::Package::open(&path).expect("open");

    let read_prop = |g: u32, i: u32| -> Option<PropertyFile> {
        let e = package.entries().iter().find(|e| e.id.type_id == 0x00B1_B104 && e.id.group == g && e.id.instance == i)?;
        PropertyFile::parse(&package.read(e).ok()?).ok()
    };
    let read_f0 = |g: u32, i: u32| -> Option<Vec<u16>> {
        let e = package.entries().iter().find(|e| e.id.type_id == 0x03E4_21F0 && e.id.group == g && e.id.instance == i)?;
        let data = package.read(e).ok()?;
        Some(data[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect())
    };

    // 1. region 描述 → 地块表 key
    let region = read_prop(group, 0x51E7_A18D).expect("region prop");
    let plot_table_inst = region.get(0xFB7A_85A0).and_then(|p| match &p.kind {
        Kind::Scalar(sc_properties::Value::Key(k)) => Some(k.instance),
        _ => None,
    }).expect("plot table key");

    // 2. 地块表在哪个 group？扫全包找该 instance 的 property（可能在本 group 或专门 group）
    let plot_prop_entry = package.entries().iter().find(|e| e.id.type_id == 0x00B1_B104 && e.id.instance == plot_table_inst).expect("plot table prop entry");
    let plot_group = plot_prop_entry.id.group;
    let plot_data = package.read(plot_prop_entry).expect("read plot table");
    let plot_table = PropertyFile::parse(&plot_data).expect("parse plot table");
    let names: Vec<String> = match &plot_table.get(0x0543_BC96).unwrap().kind {
        Kind::Array(vs) => vs.iter().map(|v| match v {
            sc_properties::Value::String8(t) => t.clone(),
            sc_properties::Value::Text(t) => format!("#{:08X}:{:08X}", t.table_id, t.instance_id),
            other => format!("{other:?}"),
        }).collect(),
        Kind::Scalar(sc_properties::Value::String8(t)) => vec![t.clone()],
        _ => vec![],
    };
    let ids: Vec<u32> = match &plot_table.get(0x16B7_B1EF).unwrap().kind {
        Kind::Array(vs) => vs.iter().filter_map(|v| match v { sc_properties::Value::UInt32(x) => Some(*x), _ => None }).collect(),
        Kind::Scalar(sc_properties::Value::UInt32(x)) => vec![*x],
        _ => vec![],
    };
    let poss: Vec<(f32, f32)> = match &plot_table.get(0xF01D_E4B1).unwrap().kind {
        Kind::Array(vs) => vs.iter().filter_map(|v| match v { sc_properties::Value::Vector2(v) => Some((v[0], v[1])), _ => None }).collect(),
        _ => vec![],
    };
    println!("plot table inst=0x{plot_table_inst:08X} group=0x{plot_group:08X}: {} names, {} ids, {} positions", names.len(), ids.len(), poss.len());

    // 3. 区域背景 tile 集合
    let tiles: Vec<u32> = package.entries().iter()
        .filter(|e| e.id.type_id == 0x03E4_21F0 && e.id.group == group)
        .map(|e| e.id.instance).collect();
    println!("background tiles: {}", tiles.len());
    let mut tile_data: Vec<(u32, Vec<u16>)> = Vec::new();
    for t in &tiles {
        if let Some(d) = read_f0(group, *t) { tile_data.push((*t, d)); }
    }

    // 4. 每个地块 vs 全部 tile
    for (n, (id, pos)) in ids.iter().zip(&poss).enumerate() {
        let name = names.get(n).map(|s| s.as_str()).unwrap_or("?");
        let Some(city) = read_f0(*id, 0) else { // 地块 group 的 F0：instance 未知，扫该 group
            // 找该 group 的 F0 条目
            let e = package.entries().iter().find(|e| e.id.type_id == 0x03E4_21F0 && e.id.group == *id);
            let Some(e) = e else { println!("city {name} id=0x{id:08X}: no heightmap"); continue };
            let data = package.read(e).unwrap();
            let city = data[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>();
            anchor_report(&city, &tile_data, name, *id, *pos);
            continue;
        };
        anchor_report(&city, &tile_data, name, *id, *pos);
    }
}

fn anchor_report(city: &[u16], tile_data: &[(u32, Vec<u16>)], name: &str, id: u32, pos: (f32, f32)) {
    if city.len() != 65536 { println!("city {name}: bad size {}", city.len()); return; }
    let mut scored: Vec<(f64, u32)> = tile_data.iter()
        .filter(|(_, d)| d.len() == 65536)
        .map(|(inst, d)| {
            let s: f64 = city.iter().zip(d).map(|(a, b)| (*a as i32 - *b as i32).abs() as f64).sum();
            (s / 65536.0, *inst)
        }).collect();
    scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let best = scored.iter().take(2).map(|(s, i)| format!("0x{i:08X}({s:.1})")).collect::<Vec<_>>().join("  ");
    println!("city {name} id=0x{id:08X} pos=({:.0},{:.0}): top2 = {best}", pos.0, pos.1);
}
