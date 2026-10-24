//! 实验工具：把三个 debug 工具（Edit Unit Prop Tool / Edit Snap Points Tool /
//! Unit Connecter Tool）的**分类键**重指到可见分类，生成一个未压缩 overlay 包，
//! 放进 `SimCityUserData/Packages/` 由游戏覆盖加载 —— 用于验证「debug 工具是
//! 被数据门控关掉、可重新打开」这一假设。
//!
//! 核心补丁逻辑已产品化为 [`sc_properties::debug_tools`]（priorities P0-2），
//! 本示例保留为 CLI 入口 + 阳性对照实验（把「宿舍」标题改指到「商務學院」的
//! 串 id —— 若游戏里真出现两个「商務學院」，就证明覆盖包确实被加载）。
//!
//! 用法：
//!   cargo run -p probes --release --example enable_debug_tools -- \
//!       <SimCity_Game.package> <out.package> [--cat=0xAC12C904] [--ui=0xAC12C904]
//!       [--action=0x1943CB96] [--start=110] [--dry]

use dbpf::{OverlayEntry, Package};
use sc_properties::debug_tools;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const TOOL_GROUP: u32 = debug_tools::TOOL_GROUP;
/// **阳性对照**：教育栏里可见的「宿舍」（`ui=0x9D2EF585`，与三个 debug 工具同栏）。
/// 把它的标题串 id 改成同栏「商務學院」的串 id —— 若游戏里真出现两个「商務學院」，
/// 就证明覆盖包确实被加载，前几轮「没出现」不是加载失败导致的假阴性。
const CONTROL_TOOL: u32 = 0x75A9_0B66;
const CONTROL_TITLE_OLD: u32 = 0x7C9A_0071; // 「宿舍」
const CONTROL_TITLE_NEW: u32 = 0x64EC_016B; // 「商務學院」
/// 标题键（对照实验用）。
const HASH_TITLE: u32 = 0x0A09_F5FA;
/// 诊断用：工具实现 action。默认不修改；`--action=0x1943CB96` 可把
/// 三个 debug 条目的 action 临时映射到已存在的 cToolPlop，用于区分
/// “菜单覆盖未命中”与“原 action 没有 native factory”。
const HASH_ACTION: u32 = 0x08F6_72C7;
const HASH_UI_POSITION: u32 = 0x0DC1_E3E0;
const ROAD_CATEGORY: u32 = 0xAC12_C904;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let base_path = positional
        .first()
        .expect("usage: enable_debug_tools <base.package> <out.package> [--cat=] [--ui=] [--dry]");
    let out_path = positional.get(1);
    let dry = args.iter().any(|a| a == "--dry");
    let cat = parse_hex(&args, "--cat=").unwrap_or(ROAD_CATEGORY);
    let ui = parse_hex(&args, "--ui=").unwrap_or(ROAD_CATEGORY);
    let action = parse_hex(&args, "--action=");
    let start = parse_i32(&args, "--start=").unwrap_or(110);
    if !dry && out_path.is_none() {
        panic!("需要输出路径，或加 --dry");
    }

    let package = Package::open(base_path).expect("open base package");
    let (entries, patches) =
        debug_tools::build_debug_tools_overlay(&package, cat, ui).expect("build overlay");
    for p in &patches {
        println!(
            "0x{:08X}  Parent {:?} → 0x{:08X}；分类 {:?} → 0x{:08X}；UI 分类 {:?} → 0x{:08X}（{} 字节）",
            p.instance,
            debug_tools::hex_list(&p.old_parents),
            p.new_parent,
            debug_tools::hex_list(&p.old_categories),
            p.new_category,
            debug_tools::hex_list(&p.old_ui_categories),
            p.new_ui_category,
            p.size_bytes
        );
    }

    // ---- 阳性对照：把「宿舍」标题改指到「商務學院」的串 id ----
    let mut entries = entries;
    if let Some(action) = action {
        for entry in entries.iter_mut() {
            if !debug_tools::DEBUG_TOOLS.contains(&entry.id.instance) {
                continue;
            }
            let old_action = entry.id.instance;
            let mut patched = debug_tools::patch_key_property_instances(
                &entry.data,
                HASH_ACTION,
                old_action,
                action,
            )
            .expect("patch diagnostic action");
            let old_position = match old_action {
                0xA2AA_4268 => 100,
                0x53B7_7423 => 200,
                0xB65E_1F2E => 300,
                _ => unreachable!(),
            };
            let index = match old_action {
                0xA2AA_4268 => 0,
                0x53B7_7423 => 1,
                0xB65E_1F2E => 2,
                _ => unreachable!(),
            };
            patched = debug_tools::patch_property_i32(
                &patched,
                HASH_UI_POSITION,
                old_position,
                start + index,
            )
            .expect("patch diagnostic menu position");
            entry.data = patched;
            println!(
                "诊断 action 0x{old_action:08X} -> 0x{action:08X}（cToolPlop 等已有 factory 时用于验证工具创建链）"
            );
        }
    }
    if let Some(entry) = package.entries().iter().find(|e| {
        e.id.type_id == PROPERTY_TYPE && e.id.group == TOOL_GROUP && e.id.instance == CONTROL_TOOL
    }) {
        let data = package.read(entry).expect("read control property");
        let patches = [(HASH_TITLE, CONTROL_TITLE_OLD, CONTROL_TITLE_NEW)];
        match debug_tools::patch_property_refs(&data, &patches, |f, h| {
            debug_tools::text_instances(f, h)
        }) {
            Ok(patched) => {
                println!(
                    "对照 0x{CONTROL_TOOL:08X} 标题串 0x{CONTROL_TITLE_OLD:08X}（宿舍）→ 0x{CONTROL_TITLE_NEW:08X}（商務學院）"
                );
                entries.push(OverlayEntry::new(entry.id, patched));
            }
            Err(error) => println!("对照 0x{CONTROL_TOOL:08X} !! 补丁失败：{error}"),
        }
    } else {
        println!("对照 0x{CONTROL_TOOL:08X} 未找到");
    }

    if dry {
        println!("\n--dry：未写文件。目标分类 0x{cat:08X} / UI 分类 0x{ui:08X}");
        return;
    }
    let out_path = out_path.unwrap();
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    dbpf::write_uncompressed_overlay_to_path(out_path, &entries).expect("write overlay");
    let size = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "\n已写出 {out_path}（{size} 字节，{} 条资源）",
        entries.len()
    );

    // 自检：重新打开并回读条目
    let check = Package::open(out_path).expect("reopen overlay");
    println!("--- 回读校验（{} 条）---", check.entries().len());
    for entry in check.entries() {
        let Ok(data) = check.read(entry) else {
            continue;
        };
        let Ok(file) = sc_properties::PropertyFile::parse(&data) else {
            continue;
        };
        println!(
            "  0x{:08X} g=0x{:08X} 分类=0x{:08X} UI=0x{:08X} 位置={:?}",
            entry.id.instance,
            entry.id.group,
            debug_tools::key_instances(&file, debug_tools::HASH_TOOL_CATEGORY)
                .first()
                .copied()
                .unwrap_or(0),
            debug_tools::key_instances(&file, debug_tools::HASH_UI_TOOL_CATEGORY)
                .first()
                .copied()
                .unwrap_or(0),
            file.get(HASH_UI_POSITION).map(|p| format!("{:?}", p.kind))
        );
    }
}

fn parse_hex(args: &[String], prefix: &str) -> Option<u32> {
    args.iter()
        .find_map(|a| a.strip_prefix(prefix))
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
}

fn parse_i32(args: &[String], prefix: &str) -> Option<i32> {
    args.iter()
        .find_map(|a| a.strip_prefix(prefix))
        .and_then(|v| v.parse().ok())
}
