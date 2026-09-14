//! 探针：按 FNV-1(小写去扩展名) = instance 的约定，从 UI 包里导出布局 JS 模块。
//! 与 locale 表同一条命名映射（`gameentry` → 0xB844F811 已验证）。
//!
//! 用法：cargo run -p dbpf --release --example ui_js_dump -- <out-dir> <package...>
//! 导出 <out>/<name>.js（命中清单内的条目）。

use std::collections::BTreeMap;
use std::path::PathBuf;

/// UI 清单 8C5BB5A8 里与 HUD/底栏/建筑面板相关的布局模块（名字去 .js）。
const NAMES: &[&str] = &[
    "globalui",
    "globalui2",
    "speedpanel",
    "speedpanel2",
    "buildingitemui2",
    "genericplop2",
    "newstyle",
    "newstylebutton",
    "maptogglebuttonui",
    "tickertape",
    "modal",
    "pausering",
    "rolloveritemiconmeter2",
    "graphsign",
    "contributenumber",
    "legendsign",
    "xsign",
];

fn fnv1_lower(name: &str) -> u32 {
    let mut hash: u32 = 0x811C_9DC5;
    for byte in name.to_ascii_lowercase().as_bytes() {
        hash = hash.wrapping_mul(0x0100_0193);
        hash ^= u32::from(*byte);
    }
    hash
}

fn main() -> dbpf::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = PathBuf::from(args.remove(0));
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    let want: BTreeMap<u32, &str> = NAMES
        .iter()
        .map(|name| (fnv1_lower(name), *name))
        .collect();

    for path in &args {
        let package = match dbpf::Package::open(path) {
            Ok(p) => p,
            Err(err) => {
                eprintln!("skip {}: {err}", path);
                continue;
            }
        };
        let package_name = path.split(['/', '\\']).next_back().unwrap_or(path);
        for entry in package.entries() {
            let Some(name) = want.get(&entry.id.instance) else {
                continue;
            };
            let data = package.read(entry)?;
            let out = out_dir.join(format!("{name}.js"));
            std::fs::write(&out, &data)?;
            println!(
                "{package_name}: {name}.js  type={:08X} group={:08X} {} bytes",
                entry.id.type_id,
                entry.id.group,
                data.len()
            );
        }
    }
    Ok(())
}
