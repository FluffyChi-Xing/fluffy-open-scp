//! 盒外灯对齐审计：解析真实灯光表，按 unit 变换算世界位置并做坐标系假设检验。
//!
//! 用法：
//! `cargo run -p probes --release --example light_align_probe -- <package> <group> <instance> [tx ty tz] [ox oy]`
//!
//! - `tx ty tz`：unit（建筑）在世界中的平移，默认 0/0/0；
//! - `ox oy`：该城市在区域中的原点（`mPosInRegion`），给出后才做
//!   `CityLocal / Region / DoubleOffset` 三假设检验。
//!
//! 依据见 `docs/re/map-light-alignment-20261102.md`：灯世界位置 =
//! `unitWorldTransform × lightLocalTranslation`，灯链上没有 `PlayArea` 夹取。
//!
//! 只读：不写包、不启动游戏。

use sc_properties::light_alignment::{
    LightFrame, LightKind, Transform34, audit, light_world_positions, parse_light_table,
};
use sc_properties::map_frames::EcoGrid;
use sc_properties::play_area::PlayArea;

fn kind_label(kind: LightKind) -> String {
    match kind {
        LightKind::Line => "line".into(),
        LightKind::Spot => "spot".into(),
        LightKind::Point => "point".into(),
        LightKind::Other(hash) => format!("other({hash:08X})"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("用法: light_align_probe <package> <group> <instance> [tx ty tz] [ox oy]");
        std::process::exit(2);
    };
    let parse_hex = |v: Option<String>, what: &str| -> u32 {
        v.and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or_else(|| {
                eprintln!("缺少 {what}（十六进制）");
                std::process::exit(2)
            })
    };
    let group = parse_hex(args.next(), "group");
    let instance = parse_hex(args.next(), "instance");
    let rest: Vec<f32> = args.filter_map(|v| v.parse::<f32>().ok()).collect();
    let unit_t = if rest.len() >= 3 {
        Some([rest[0], rest[1], rest[2]])
    } else {
        None
    };
    let origin = if rest.len() >= 5 {
        Some([rest[3], rest[4]])
    } else {
        None
    };

    let pkg = dbpf::Package::open(&path).expect("open package");
    let entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.group == group && e.id.instance == instance)
        .unwrap_or_else(|| {
            eprintln!("包内没有 {group:08X}:{instance:08X}");
            std::process::exit(1)
        });
    let props = sc_properties::PropertyFile::parse(&pkg.read(entry).unwrap()).expect("parse props");
    let table = match parse_light_table(&props) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("不是灯光表：{e:?}");
            std::process::exit(1);
        }
    };

    println!(
        "TGI {:08X}:{:08X}:{:08X}  灯 {} 盏",
        entry.id.type_id,
        entry.id.group,
        entry.id.instance,
        table.lights.len()
    );
    let mut by_kind = std::collections::BTreeMap::new();
    let mut by_channel = std::collections::BTreeMap::new();
    for light in &table.lights {
        *by_kind.entry(kind_label(light.kind)).or_insert(0usize) += 1;
        *by_channel.entry(light.channel.clone()).or_insert(0usize) += 1;
    }
    println!("按灯型：{by_kind:?}");
    println!("按通道：{by_channel:?}");

    let unit = Transform34::from_translation(unit_t.unwrap_or([0.0; 3]));
    let area = PlayArea::retail_city([0.0, 0.0]);
    let grid = EcoGrid::retail([128, 128]);
    let report = audit(&table, &unit, &area, grid, origin);
    let positions = light_world_positions(&unit, &table);

    println!(
        "unit 平移 = {:?}  PlayArea = ±1024  eco = {}² ({} m/格)",
        unit.translation,
        grid.size,
        grid.cell()
    );
    println!(
        "盒外灯 {} / 索引夹取 {} / 共 {}",
        report.outside_play_area,
        report.eco_clamped,
        report.lights.len()
    );
    if let Some(frame) = &report.frame {
        let label = match frame.frame {
            LightFrame::CityLocal => "城市局部（无需修正）",
            LightFrame::Region => "区域坐标（应减去城市原点）",
            LightFrame::DoubleOffset => "重复叠加城市原点（应减去 2×原点）",
        };
        println!(
            "坐标系判定 = {label}  修正 = ({:.1}, {:.1})  盒内 {} → {} / {}  假设命中 {:?}",
            frame.offset[0],
            frame.offset[1],
            frame.inside_before,
            frame.inside_after,
            frame.total,
            frame.hypothesis_hits
        );
    }

    let shown = table.lights.len().min(12);
    println!("\n前 {shown} 盏（局部 → 世界）：");
    for (light, world) in table.lights.iter().zip(&positions).take(shown) {
        println!(
            "  #{:<2} {:<12} {:<10} 局部=({:>8.2},{:>8.2},{:>7.2}) 世界=({:>9.2},{:>9.2},{:>7.2}) 盒内={} eco=({},{}) clamped={}",
            light.index,
            light.channel,
            kind_label(light.kind),
            light.local.translation[0],
            light.local.translation[1],
            light.local.translation[2],
            world[0],
            world[1],
            world[2],
            area.contains_xy([world[0], world[1]]),
            grid.sample([world[0], world[1]]).x,
            grid.sample([world[0], world[1]]).y,
            grid.sample([world[0], world[1]]).clamped
        );
    }

    // 最远灯：按到盒边的余量排序，用于区分“整体平移”与“单灯异常”。
    let mut sorted: Vec<_> = report
        .lights
        .iter()
        .map(|l| {
            let margin = 1024.0 - l.world[0].abs().max(l.world[1].abs());
            (margin, l.index, l.world)
        })
        .collect();
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((margin, index, world)) = sorted.first() {
        if *margin < 0.0 {
            println!(
                "\n最靠外的灯 #{index}: 超出盒边 {:.2} m，世界 = ({:.2}, {:.2}, {:.2})",
                -margin, world[0], world[1], world[2]
            );
        } else {
            println!(
                "\n最靠外的灯 #{index}: 距盒边仍有 {margin:.2} m，世界 = ({:.2}, {:.2}, {:.2})",
                world[0], world[1], world[2]
            );
        }
    }
}
