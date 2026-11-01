//! 探针：验证「建筑 → vignette → actor 起点/终点 → 本 lot 的 spawner 名字」这条绑定链。
//! 对每个含 spawner 的 lot：读 vignette 列表(0x0D2E8A55)，再读每个 vignette 的
//! actorIDs(0x0D649949) / startPoint(0x0D802215) / endPoint(0x0D9E4D6E)，
//! 统计这些位置 id 与本 lot spawner 名字 id 的交集。仅开发用。
//!
//! 用法：cargo run -p probes --example vignette_bind_probe -- <package> [max-lots]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::{BTreeMap, BTreeSet};

const SPAWNER_IDS: u32 = 0x0E1B_AC61;
const VIGNETTE_IDS: u32 = 0x0D2E_8A55;
const ACTOR_IDS: u32 = 0x0D64_9949;
const START_POINT: u32 = 0x0D80_2215;
const END_POINT: u32 = 0x0D9E_4D6E;
const MODEL_NAMES: u32 = 0x0D2E_8A61;

fn keys_of(file: &PropertyFile, col: u32) -> Vec<u32> {
    let Some(p) = file.get(col) else {
        return Vec::new();
    };
    match &p.kind {
        Kind::Array(items) => items
            .iter()
            .filter_map(|v| match v {
                Value::Key(k) => Some(k.instance),
                Value::UInt32(u) => Some(*u),
                _ => None,
            })
            .collect(),
        Kind::Scalar(Value::Key(k)) => vec![k.instance],
        _ => Vec::new(),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: vignette_bind_probe <package> [max-lots]");
    let max_lots: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(20);
    let pkg = Package::open(&path).expect("open package");

    // 第一遍：索引所有属性文件（instance → file），以及 lot 的 spawner 名集合
    let mut by_instance: BTreeMap<u32, (u32, PropertyFile)> = BTreeMap::new();
    let mut vig_defs: BTreeMap<u32, PropertyFile> = BTreeMap::new();
    let mut lots: Vec<(u32, BTreeSet<u32>)> = Vec::new();
    for entry in pkg.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let Ok(data) = pkg.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        if file.get(SPAWNER_IDS).is_some() {
            let ids: BTreeSet<u32> = keys_of(&file, SPAWNER_IDS).into_iter().collect();
            if !ids.is_empty() {
                lots.push((entry.id.instance, ids));
            }
        }
        if entry.id.group == 0x40E1_E800 {
            vig_defs.insert(entry.id.instance, file.clone());
        }
        by_instance.insert(entry.id.instance, (entry.id.group, file));
    }
    println!("lots with spawners: {}  vig_defs(g=0x40E1E800): {}", lots.len(), vig_defs.len());

    let mut total_lots_with_vignette = 0usize;
    let mut total_lots_with_hit = 0usize;
    let mut shown = 0usize;
    for (lot, spawners) in &lots {
        let Some((_g, file)) = by_instance.get(lot) else { continue };
        let vigs = keys_of(file, VIGNETTE_IDS);
        if vigs.is_empty() {
            continue;
        }
        total_lots_with_vignette += 1;
        let mut starts: BTreeSet<u32> = BTreeSet::new();
        let mut ends: BTreeSet<u32> = BTreeSet::new();
        let mut actors: BTreeSet<u32> = BTreeSet::new();
        let mut models: BTreeSet<u32> = BTreeSet::new();
        for v in &vigs {
            let Some(vf) = vig_defs.get(v).or_else(|| by_instance.get(v).map(|(_, f)| f)) else { continue };
            starts.extend(keys_of(vf, START_POINT));
            ends.extend(keys_of(vf, END_POINT));
            actors.extend(keys_of(vf, ACTOR_IDS));
            models.extend(keys_of(vf, MODEL_NAMES));
        }
        let hit_start: Vec<u32> = starts.intersection(spawners).copied().collect();
        let hit_end: Vec<u32> = ends.intersection(spawners).copied().collect();
        if !hit_start.is_empty() || !hit_end.is_empty() {
            total_lots_with_hit += 1;
        }
        if shown < max_lots {
            shown += 1;
            println!(
                "\nlot 0x{lot:08X} spawners={} vignettes={} actors={} models={}",
                spawners.len(),
                vigs.len(),
                actors.len(),
                models.len()
            );
            println!(
                "   startPoint ids={:?}\n   endPoint ids={:?}",
                starts.iter().take(8).map(|v| format!("0x{v:08X}")).collect::<Vec<_>>(),
                ends.iter().take(8).map(|v| format!("0x{v:08X}")).collect::<Vec<_>>()
            );
            println!(
                "   ∩ spawner(start)={:?}  ∩ spawner(end)={:?}",
                hit_start.iter().map(|v| format!("0x{v:08X}")).collect::<Vec<_>>(),
                hit_end.iter().map(|v| format!("0x{v:08X}")).collect::<Vec<_>>()
            );
        }
    }
    println!(
        "\n--- lots_with_vignette={total_lots_with_vignette} with_location_hit={total_lots_with_hit}"
    );
}
