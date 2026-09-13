//! 探针：扫描含 Spawner 列的 lot，dump spawner 全列值并尝试解析 agent 引用
//! （0x0F0E2BF1）与数量列（0x0E715928/9）是否指向真实 DBPF 资源。仅开发用。
//!
//! 用法：cargo run -p sc-properties --release --example spawner_probe -- <package> [max]
use dbpf::Package;
use sc_properties::{PropertyFile, Value};

const SPAWNER_IDS: u32 = 0x0E1B_AC61;
const SPAWNER_TRANSFORMS: u32 = 0x0E1B_AC62;
const SPAWNER_COUNT: u32 = 0x0E71_5928;
const SPAWNER_COUNT_RANDOM: u32 = 0x0E71_5929;
const SPAWNER_AGENT: u32 = 0x0F0E_2BF1;

fn key_of(file: &PropertyFile, hash: u32, index: usize) -> Option<(u32, u32, u32)> {
    match file.get(hash).and_then(|p| p.array())?.get(index)? {
        Value::Key(k) => Some((k.type_id, k.group, k.instance)),
        _ => None,
    }
}

fn i32_of(file: &PropertyFile, hash: u32, index: usize) -> Option<i32> {
    match file.get(hash).and_then(|p| p.array())?.get(index)? {
        Value::Int32(v) => Some(*v),
        _ => None,
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: spawner_probe <package> [max]");
    let max: usize = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(6);
    let package = Package::open(&path).expect("open package");

    let mut lots_with_spawner = 0usize;
    let mut shown = 0usize;
    let mut id_kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut agent_keys: Vec<(u32, u32)> = Vec::new();
    let mut with_count = 0usize;
    let mut with_random = 0usize;
    let mut with_agent = 0usize;


    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        let Some(ids) = file.get(SPAWNER_IDS).and_then(|p| p.array()) else { continue };
        if ids.is_empty() {
            continue;
        }
        lots_with_spawner += 1;
        if file.get(SPAWNER_COUNT).is_some() { with_count += 1; }
        if file.get(SPAWNER_COUNT_RANDOM).is_some() { with_random += 1; }
        if file.get(SPAWNER_AGENT).is_some() { with_agent += 1; }
        for value in ids.iter() {
            let Value::Key(k) = value else { continue };
            *id_kinds.entry(format!("t=0x{:08X} g=0x{:08X}", k.type_id, k.group))
                .or_default() += 1;
        }
        // agent 引用列（若存在）单独收集
        if let Some(agents) = file.get(SPAWNER_AGENT).and_then(|p| p.array()) {
            for (index, value) in agents.iter().enumerate() {
                if let Value::Key(k) = value {
                    agent_keys.push((entry.id.instance, k.instance));
                    if index < 3 {
                        println!("   agent[{index}] lot 0x{:08X} -> 0x{:08X}", entry.id.instance, k.instance);
                    }
                }
            }
        }
        if shown < max {
            shown += 1;
            let tf = file.get(SPAWNER_TRANSFORMS).and_then(|p| p.array());
            println!("lot 0x{:08X} spawners={} transform_col={:?}",
                entry.id.instance, ids.len(), tf.map(|t| t.len()));
            for index in 0..ids.len().min(4) {
                println!("   #{index} id={:?} count={:?} countRand={:?} agent={:?}",
                    key_of(&file, SPAWNER_IDS, index),
                    i32_of(&file, SPAWNER_COUNT, index),
                    i32_of(&file, SPAWNER_COUNT_RANDOM, index),
                    key_of(&file, SPAWNER_AGENT, index));
            }
        }
    }

    println!("\n--- lots_with_spawner={lots_with_spawner} count={with_count} countRand={with_random} agent={with_agent}");
    println!("--- 0x0E1BAC61 key (type,group) 分布：");
    for (k, v) in &id_kinds {
        println!("    {k} × {v}");
    }
    // agent 引用是否在包内存在
    let mut resolved = 0usize;
    for (lot, inst) in &agent_keys {
        let hit = package.entries().iter().any(|e| e.id.instance == *inst);
        if hit { resolved += 1; }
        println!("    lot 0x{lot:08X} agent 0x{inst:08X} in_package={hit}");
    }
    println!("--- agent 采样 {}/{} 在包内命中", resolved, agent_keys.len());
}
