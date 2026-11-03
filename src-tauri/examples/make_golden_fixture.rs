//! P0 黄金用例源包生成器：从真实 donor 参考包抽取 parent-free 通用模板链
//! （GENERIC_ASSET 9EC474FF 链 / INTERACTION_CHAIN 34CE7E14 / 菜单链
//! 48E1EB00:F970A822 / donor lot 58F0C000:2063CCDD / PathEntry
//! 40E1C400:675B61B1），叠加 fixture 专属件（40E1C000:8D007745 donor lot：
//! Frontage 数值 + 0x0CC8FE7A→675B61B1 绑定键），产出仓库内自包含的
//! golden-source.package。tests.rs 以 include_bytes! 内嵌它作 source.package。
//!
//! 用法：cargo run -p fluffy-open-scp --example make_golden_fixture -- \
//!   <reference.package> <out.package>
//!
//! reference.package 的重生成：工作台大学项目的 parent-free 构建
//! （tmp/openscp-build-v1/university/reference.package），或用
//! make_identity_donor 同款探针从游戏包抽取。

use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sc_properties::{Kind, Property, PropertyFile, PropertyEncoding, PropType, Value};
use std::{env, fs, path::Path};

/// 抽取的 donor 链成员（与 reference.package 的 0x00B1B104 清单一致）。
/// 原生地面图集模型(作者链 require(inventory),~1.4MB each)。
const DONOR_MODELS: [(u32, u32, u32); 3] = [
    (0x2F4E_681B, 0, 0xA0DA_9E6C),
    (0x2F4E_681B, 0, 0x60E7_805D),
    // donor 真实 ground mask raster(resource closure 必需)
    (0x2F4E_681C, 0, 0x1CF8_19C1),
];

const DONOR_B1B104: [(u32, u32); 11] = [
    (0x40E1_C000, 0x2A25_CF66),
    (0x40E1_C000, 0x34CE_7E14),
    (0x40E1_C000, 0x593B_61D2),
    (0x40E1_C000, 0x9EC4_74FF),
    (0x40E1_C000, 0xF1CF_C5B0),
    (0x40E1_C000, 0xF98B_CB35),
    (0x40E1_C400, 0x675B_61B1),
    (0x48E1_EB00, 0xF970_A822),
    (0x48E1_EB00, 0xAF04_2E9A),
    (0x51F0_C000, 0x9D56_EBFD),
    (0x58F0_C000, 0x2063_CCDD),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reference = env::args().nth(1).ok_or("usage: make_golden_fixture <reference.package> <out.package>")?;
    let out = env::args().nth(2).ok_or("usage: make_golden_fixture <reference.package> <out.package>")?;
    let pkg = Package::open(Path::new(&reference))?;

    let mut entries: Vec<OverlayEntry> = Vec::new();
    for (type_id, group, instance) in DONOR_MODELS {
        let id = ResourceId { type_id, group, instance };
        let entry = pkg
            .entry(id)
            .ok_or_else(|| format!("reference package missing {id:08X?}"))?;
        entries.push(OverlayEntry::new(id, pkg.read(entry)?));
    }
    for (group, instance) in DONOR_B1B104 {
        let id = ResourceId { type_id: 0x00B1_B104, group, instance };
        let entry = pkg
            .entry(id)
            .ok_or_else(|| format!("reference package missing {id:08X?}"))?;
        entries.push(OverlayEntry::new(id, pkg.read(entry)?));
    }

    // fixture 专属 ground lot:最小 ground 属性(native_profile 只接受
    // LOT_TYPES 数值 + parent/model/atlas/mask 键),LotSize=192×96。
    let donor_lot = donor_lot_file(vec![
        Property {
            hash: 0x0CCB_7FC8,
            prop_type: PropType::Vector2,
            kind: Kind::Scalar(Value::Vector2([192.0, 96.0])),
            encoding: PropertyEncoding::default(),
        },
        // ground mask key → 包内 raster(closure 会按 candidates 解析;
        // 模板链自带 2C61FE2E 在参考包缺席,故用 donor 实际引用的 1CF819C1)
        Property {
            hash: 0x0CCB_7FD5,
            prop_type: PropType::Key,
            kind: Kind::Scalar(Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: 0x1CF8_19C1,
            })),
            encoding: PropertyEncoding::default(),
        },
    ]);
    entries.push(OverlayEntry::new(
        ResourceId { type_id: 0x00B1_B104, group: 0x40E1_C000, instance: 0x8D00_7745 },
        donor_lot,
    ));

    fs::write(Path::new(&out), write_uncompressed_overlay(&entries)?)?;
    println!("wrote {out} with {} entries", entries.len());
    Ok(())
}

fn donor_lot_file(values: Vec<Property>) -> Vec<u8> {
    PropertyFile { claimed_count: values.len() as u32, values }
        .encode_canonical()
        .unwrap()
}
