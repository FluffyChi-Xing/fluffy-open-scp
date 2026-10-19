//! zone 资源 id 反查：BlendColors 活体提取的 16 位 id（zone 资源 instance
//! 低 16 位），在 Game/EP1/App 包中定位资源并检测 0xdca011d/0xdca012d
//! 两套颜色属性（zoning-lot.md §3 的 ToolMetaLotZone 体系）。
use dbpf::Package;
use std::collections::HashSet;

const ZONE_IDS: &[u32] = &[
    0x9c9, 0x9bf, 0x9c0, 0x92f, 0x92e, 0x235, 0x234, 0xd46, 0xd47, 0x93d, 0x948, 0x94d,
    0x94f, 0x94c, 0x93b, 0x91b, 0x91a, 0x912, 0x911, 0x916, 0x915, 0x905, 0x908, 0x986,
    0x989, 0x987, 0x8c0, 0x8c1, 0x90e, 0x8bb, 0x8b8, 0x907, 0x8be, 0x935, 0x934, 0x931,
    0x930, 0x90d, 0x16d0, 0x16d2, 0x93a, 0x939, 0x933, 0x937, 0x946, 0x8ff,
];
const PROP_LO: u32 = 0x0dca011d;
const PROP_HI: u32 = 0x0dca012d;

fn main() -> dbpf::Result<()> {
    let roots = [
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Game.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCityDataEP1.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_App.package",
    ];
    let want: HashSet<u32> = ZONE_IDS.iter().copied().collect();
    for root in roots {
        let name = root.rsplit(std::path::MAIN_SEPARATOR).next().unwrap();
        let pkg = match Package::open(root) {
            Ok(p) => p,
            Err(e) => { eprintln!("{name}: {e:?}"); continue; }
        };
        println!("== {name}（{} 资源）==", pkg.entries().len());
        let mut hits = 0;
        for e in pkg.entries() {
            if e.id.instance > 0xFFFF && want.contains(&(e.id.instance & 0xFFFF)) {
                let body = pkg.read(e)?;
                let has_lo = body.chunks_exact(4).any(|w| u32::from_le_bytes(w.try_into().unwrap()) == PROP_LO);
                let has_hi = body.chunks_exact(4).any(|w| u32::from_le_bytes(w.try_into().unwrap()) == PROP_HI);
                let mark = if has_lo || has_hi { " ★含颜色属性" } else { "" };
                println!("  inst={:08x} type={:08x} grp={:08x} size={}{}", e.id.instance, e.id.type_id, e.id.group, body.len(), mark);
                hits += 1;
                if hits > 50 { println!("  …截断"); break; }
            }
        }
        println!("  命中 {hits}");
    }
    Ok(())
}
