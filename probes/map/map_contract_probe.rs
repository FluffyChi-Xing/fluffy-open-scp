//! 地图契约实测：用安装包真实数据复核 PlayArea / eco 格网 / 相机可达幅 / 回环。
//!
//! 用法：
//! `cargo run -p probes --release --example map_contract_probe -- <SimCityData> [regionGroupHex]`
//!
//! 输出：区域边框、flatScale 证据、每个城市的城市局部 PlayArea 与相机可达半幅、
//! 9 点回环误差，以及“扩到 2048 半边”的静态计划与问题清单。
//!
//! 只读：不写包、不碰存档、不启动游戏。

use sc_properties::camera_bounds::CameraConstraintProfile;
use sc_properties::map_contract::{self, RoundTripSample};
use sc_properties::map_frames::{EcoGrid, RegionFrame};
use sc_properties::play_area::{EdgeExpansion, PlayAreaIssue, plan_expansion};

const FLAT_SCALE: u32 = 0xB8F6_B4CF;
const CITY_HALF: f32 = 1024.0;

fn flat_scale(packages: &[dbpf::Package]) -> Option<(f32, String)> {
    for pkg in packages {
        for entry in pkg.entries() {
            if entry.id.type_id != 0x00B1_B104 {
                continue;
            }
            let Ok(bytes) = pkg.read(entry) else { continue };
            let Ok(props) = sc_properties::PropertyFile::parse(&bytes) else {
                continue;
            };
            if let Some(sc_properties::Kind::Scalar(sc_properties::Value::Float(v))) =
                props.get(FLAT_SCALE).map(|p| &p.kind)
            {
                return Some((
                    *v,
                    format!(
                        "{:08X}:{:08X}:{:08X}",
                        entry.id.type_id, entry.id.group, entry.id.instance
                    ),
                ));
            }
        }
    }
    None
}

fn print_round_trip(samples: &[RoundTripSample]) {
    println!("  回环样本（城市局部坐标，米）：");
    for s in samples {
        println!(
            "    {:<14} world=({:>9.2},{:>9.2}) eco=({},{}) s/t=({:.3},{:.3}) 误差={:.4} 盒内={} 格内={}{}",
            s.label,
            s.world[0],
            s.world[1],
            s.eco.x,
            s.eco.y,
            s.eco.s,
            s.eco.t,
            s.error,
            s.inside_play_area,
            s.inside_eco_grid,
            if s.eco.clamped { " [索引夹取]" } else { "" }
        );
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| {
        "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData".into()
    });
    let wanted = args
        .next()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());

    let open = |name: &str| dbpf::Package::open(std::path::Path::new(&dir).join(name)).ok();
    let game = open("SimCity_Game.package");
    let terrain =
        open("SimCity_RegionTerrain0.package").or_else(|| open("SimCity_RegionTerrain1.package"));
    let Some(terrain) = terrain else {
        eprintln!("缺少 SimCity_RegionTerrain 包：{dir}");
        std::process::exit(2);
    };

    let (flat, tgi) = match game
        .as_ref()
        .and_then(|g| flat_scale(std::slice::from_ref(g)))
    {
        Some(v) => v,
        None => (1024.0, "内置默认".to_string()),
    };
    println!("flatScale = {flat}  (来源 {tgi})");
    if (flat - 1024.0).abs() > f32::EPSILON {
        println!(
            "注意：flatScale ≠ 1024，eco 定义域与城市盒半边不再重合（盒外向采样边界变化 {} m）",
            1024.0 - flat
        );
    }

    let regions = sc_properties::region_map::list_regions(&terrain);
    println!("区域数 = {}", regions.len());
    let mut printed = 0;
    for region in &regions {
        if let Some(wanted) = wanted {
            if region.group != wanted {
                continue;
            }
        }
        let Some((uids, positions, _env)) =
            sc_properties::region_3d::plot_table(&terrain, region.group)
        else {
            continue;
        };
        let plots: Vec<_> = uids
            .iter()
            .zip(&positions)
            .map(|(uid, pos)| {
                (
                    uid.clone(),
                    None,
                    [pos.0, pos.1],
                    CITY_HALF,
                    "city".to_string(),
                )
            })
            .collect();
        let grid = EcoGrid::new(128, flat);
        let report = map_contract::report(RegionFrame::retail(), grid, &plots);
        let name = region
            .display_name
            .clone()
            .or_else(|| {
                sc_properties::region_3d::template_name_for_group(region.group).map(String::from)
            })
            .unwrap_or_else(|| "?".into());
        println!(
            "\n=== {:08X} {name}  区域 {:.0} m  {} 城  eco {}² 格 {:.3} m ===",
            region.group,
            report.region.side,
            report.landmarks.len(),
            grid.size,
            grid.cell()
        );
        for city in report.landmarks.iter().take(6) {
            let plan_like =
                CameraConstraintProfile::retail_city(city.play_area).plan_unlock(2048.0);
            println!(
                "  uid={:<6} 区域中心=({:>9.1},{:>9.1}) playCell={:.2}×{:.2} m  modelWorldBound={:.0} m  相机可达半幅={:.0}/{:.0} m  内缩={:.0} m",
                city.uid,
                city.city_origin[0],
                city.city_origin[1],
                city.cell.size_xy()[0],
                city.cell.size_xy()[1],
                city.model_world_bound,
                city.camera_reach_half[0],
                city.camera_reach_half[1],
                city.camera_inset
            );
            let steps: Vec<String> = plan_like
                .steps
                .iter()
                .map(|s| match s {
                    sc_properties::camera_bounds::UnlockStep::ExpandPlayArea {
                        from_half,
                        to_half,
                    } => {
                        format!("扩 PlayArea {from_half:.0}→{to_half:.0}")
                    }
                    sc_properties::camera_bounds::UnlockStep::ReplaceAbsoluteLimit { from, to } => {
                        format!("改绝对常量 {from:.0}→{to:.0}")
                    }
                    sc_properties::camera_bounds::UnlockStep::KeepGroundSample => {
                        "保留防穿地".into()
                    }
                    sc_properties::camera_bounds::UnlockStep::VerifyInputs => "四路输入验收".into(),
                })
                .collect();
            println!("      相机解锁所需步骤：{}", steps.join(" / "));
        }
        if printed == 0 {
            print_round_trip(&report.round_trip);
            // 扩展预览：只看第一个城市，扩到 2048 半边。
            if let Some(city) = report.landmarks.first() {
                let plan = plan_expansion(
                    &city.play_area,
                    EdgeExpansion::to_half_extent(CITY_HALF, 2048.0),
                    report.region,
                    grid,
                );
                println!("  扩展计划（半边 1024→2048，可用于模拟域）：");
                println!(
                    "    新 playCell = {:.2}×{:.2} m，中心位移 = ({:.1}, {:.1})，旧范围仍在新范围内 = {}",
                    plan.cell_after.size_xy()[0],
                    plan.cell_after.size_xy()[1],
                    plan.center_shift[0],
                    plan.center_shift[1],
                    plan.contains_old_range
                );
                for issue in &plan.issues {
                    let text = match issue {
                        PlayAreaIssue::ExceedsEcoGrid {
                            axis,
                            limit,
                            requested,
                        } => format!(
                            "超 eco 定义域 {axis}：|{requested:.0}| > {limit:.0} → 盒外采样夹到格边"
                        ),
                        PlayAreaIssue::ExceedsRegion {
                            axis,
                            requested,
                            limit,
                        } => {
                            format!("超区域可见范围 {axis}：{requested:.0} > {limit:.0}")
                        }
                        PlayAreaIssue::NotCellAligned { edge, value, cell } => {
                            format!("{edge} 边 {value:.1} m 不是 eco 单格 {cell:.3} m 的整数倍")
                        }
                        PlayAreaIssue::CenterShift { dx, dy } => {
                            format!("中心位移 ({dx:.1}, {dy:.1}) m → 旧对象会整体平移")
                        }
                        PlayAreaIssue::UnknownZ => "z 未知（离线包不含 PlayArea 的 z 值）".into(),
                        PlayAreaIssue::DegeneratePlayCell { map_bounds } => {
                            format!("mMapBounds 非法：{map_bounds:?}")
                        }
                        PlayAreaIssue::NegativeExpansion { edge, value } => {
                            format!("{edge} 扩展量为负：{value}")
                        }
                        PlayAreaIssue::Shrinks {
                            current_half,
                            target_half,
                        } => {
                            format!("收缩城市盒：{current_half:.0}→{target_half:.0}")
                        }
                    };
                    println!(
                        "    [{}] {text}",
                        if issue.is_blocking() {
                            "阻塞"
                        } else {
                            "警告"
                        }
                    );
                }
            }
        }
        printed += 1;
        if wanted.is_some() {
            break;
        }
    }
    if printed == 0 {
        eprintln!("没有匹配的区域（可用 --list 另行枚举）");
        std::process::exit(1);
    }
}
