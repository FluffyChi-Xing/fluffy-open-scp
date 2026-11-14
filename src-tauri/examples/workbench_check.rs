//! Compile a workbench document's logic graph and print the resolved behavior.
//! Usage: cargo run -q -p fluffy-open-scp --example workbench_check -- <asset.lot.json>
use fluffy_open_scp_lib::asset_build_m1::{behavior_nodes, workbench_logic};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).expect("read");
    let document: serde_json::Value = serde_json::from_str(&text).expect("parse");
    let compiled = workbench_logic::compile(&document, &|_| false).expect("logic compile failed");
    println!("logic_present: {}", compiled.logic_present);
    println!("menu override: {}", compiled.menu.is_some());
    if let Some(labor) = &compiled.labor {
        println!("labor: {labor}");
    }
    println!(
        "behaviors: {}",
        serde_json::to_string(&compiled.behaviors).unwrap()
    );
    for warning in &compiled.warnings {
        println!("warning: {warning}");
    }
    let labor = compiled.labor.as_ref();
    let resolved = behavior_nodes::parse_labor(labor)
        .and_then(|labor| {
            behavior_nodes::parse_nodes(&compiled.behaviors)
                .and_then(|nodes| behavior_nodes::compile(labor, &nodes))
        })
        .expect("behavior compile failed");
    println!("--- resolved behavior ---");
    println!("ensure_bundles: {:08X?}", resolved.ensure_bundles);
    println!(
        "jobs mounted: {}",
        resolved
            .ensure_bundles
            .iter()
            .any(|b| *b == 0xD07C_5A04 || *b == 0xF236_BB93)
    );
    println!("schedule: {:?}", {
        let mut keys: Vec<_> = resolved.set_int_fields.iter().collect();
        keys.sort_by_key(|(h, _)| **h);
        keys.iter()
            .map(|(h, v)| format!("{h:08X}={v}"))
            .collect::<Vec<_>>()
    });
    let mut floats: Vec<_> = resolved.set_float_fields.iter().collect();
    floats.sort_by_key(|(h, _)| **h);
    for (hash, value) in floats {
        println!("float {hash:08X} = {value}");
    }
}
