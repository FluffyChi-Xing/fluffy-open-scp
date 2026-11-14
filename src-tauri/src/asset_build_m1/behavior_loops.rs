//! 行为闭环处理器注册表：闭环与建筑身份（parent / 类别 / 菜单）正交。
//!
//! 一个闭环 = 要挂载的 bundle + 要从 donor 排除表解封的 bundle + 这些
//! bundle 的绑定表（`0x0A6FE749/4A/4B/4C`）与规则专用化实际消费的 Unit 列。
//! 列归属按"谁的 bundle 消费它"判定，而不是按 donor 来源：同一哈希可以被
//! 多个闭环共享（`0AD6A725` 在学校是招工常量、在观光是游客容量），活跃闭环
//! 拥有的列永远不会被残留剥离删除。
//!
//! 基线取自已在游戏里跑通该闭环的原版/金样本叶子（威利斯 8D007745、
//! s31 BE18DA2D、Copan 4DC31F9B）；作者显式值始终优先。

use super::behavior_nodes::ResolvedBehavior;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BehaviorLoop {
    /// 招工上班：岗位 → WorkersNeeded → 城市劳工库存到岗。
    Labor,
    /// 观光客进店：吸引力 → TouristW* → 游客/纪念品容量 → 分档收入。
    Tourism,
    /// 地价：叶子字段 + 全局 LandValue 规则（无需 bundle）。
    LandValue,
    /// 住人：人口 + 幸福 token + 住宅通路/占用 bundle（Copan 栈）。
    Residence,
}

/// 闭环拥有的列；`baseline = None` 表示只由作者写（不自动补默认）。
#[derive(Debug, Clone, Copy)]
pub struct OwnedField {
    pub hash: u32,
    pub baseline: Option<f64>,
}

const fn author(hash: u32) -> OwnedField {
    OwnedField {
        hash,
        baseline: None,
    }
}

const fn base(hash: u32, value: f64) -> OwnedField {
    OwnedField {
        hash,
        baseline: Some(value),
    }
}

#[derive(Debug)]
pub struct LoopSpec {
    pub id: BehaviorLoop,
    pub ensure_bundles: &'static [u32],
    pub unban_bundles: &'static [u32],
    pub fields: &'static [OwnedField],
    pub internal_path_ref: bool,
    pub evidence: &'static str,
}

/// 内部 path 引用列（GetUnitInternalPathEntries / 劳工找门）。
pub const INTERNAL_PATH_REF_FIELD: u32 = 0x0DA6_9D6F;

const LABOR: LoopSpec = LoopSpec {
    id: BehaviorLoop::Labor,
    // D07C5A04/F236BB93：岗位 bundle（s31 A5 同构）。
    ensure_bundles: &[0xD07C_5A04, 0xF236_BB93],
    // 30645625：WorkersNeeded.cap ← MinimumWorkers、MoneyToken.cap ← ×3、
    // 通勤车 bin；公园 donor 的排除表挡住它（威利斯有岗位不排除，Copan 无岗位排除）。
    unban_bundles: &[0x3064_5625],
    fields: &[
        author(0x0A79_17C5), // JobsW1
        author(0x0A79_17C6), // JobsW2
        author(0x0A79_17C7), // JobsW3
        author(0x0A79_17C8), // MinimumWorkersForProduction
        author(0x0A79_17C3), // TotalJobs（labor 接线按 Σ 写）
        // 劳工 bundle 数量绑定：#07F42755←A9、#07F42756←B3、#07F42757←B4
        // （30645625 与 D07C5A04 同源）。B3/B4 曾被学校残留剥离删掉，
        // 必须由劳工闭环拥有；基线 0 与 s31 叶子同值，不表示"关闭招工"。
        base(0x0E3F_65A9, 10.0),
        base(0x0E3F_65B3, 0.0),
        base(0x0E3F_65B4, 0.0),
        author(0x0D00_853A),
        author(0x0D00_853B),
        author(0x0D00_853C),
        author(0x0D00_853D),
        author(0x0D00_853E),
        author(0x0D00_853F),
        author(0x0D00_8540),
        author(0x0D00_8541),
        // 学历分布 6 列：值由 apply_labor_auto_wiring 按 W1/W2/W3 注入，
        // 这里只声明归属。公园 donor 的学校残留剥离把同名哈希列进
        // SCHOOL_RESIDUE_FIELDS——没有归属时"剥了没人补"，希望大厦 /48
        // 就因此全链缺失（2026-11-11 实证：所有能招工样本 s31/Willis/
        // C-zone/HTzhiyao 都带非零学历分布，缺失 = 劳工市场按教育程度
        // 匹配不到人，永远「需要劳工」）。
        author(0x0DC1_DB39),
        author(0x0DC1_DB43),
        author(0x0DC1_DB45),
        author(0x0DC1_DB5B),
        author(0x0DC1_DB5C),
        author(0x0DC1_DB5D),
    ],
    // 找门引用：学校样本有、公园 donor 无。行人/车辆 path 都能进门；
    // 本字段只补 GetUnitInternalPathEntries 的叶子指针，不是"必须有车行 path"。
    internal_path_ref: true,
    evidence: "30645625/D07C bind A9/B3/B4; walking workers use pedestrian path; Willis keeps 30645625 without school sample",
};

const TOURISM: LoopSpec = LoopSpec {
    id: BehaviorLoop::Tourism,
    // 威利斯 A5 游客容器：CF116B9C 财富档、2B5A2BAF 利润、362087BD 购物。
    ensure_bundles: &[0xCF11_6B9C, 0x2B5A_2BAF, 0x3620_87BD],
    unban_bundles: &[],
    fields: &[
        author(0x0E54_2F39),         // 吸引力
        base(0x0AD6_A725, 390.0),    // kPropShopping_TotalShoppers 游客容量
        base(0x0DA3_EE15, 51.0),     // 菜单「每日旅客量」参数
        base(0x0EE2_A50D, 11.0),     // ToyIncomePerResourceW1
        base(0x0EE2_A50E, 11.0),     // ToyIncomePerResourceW2
        base(0x0EE2_A50F, 11.0),     // ToyIncomePerResourceW3
        base(0x0E26_60C6, 3300.0),   // SouvenirsW1/W2/W3 容量
        base(0x0C0F_2290, 48.0),     // 货运补货规则 B019AC5E 周期
        base(0x0C0F_228C, 48.0),     // 同规则专用化 op6
    ],
    internal_path_ref: false,
    evidence: "Willis 8D007745 leaf values; tourist bundle binds Souvenirs.cap <- 0E2660C6",
};

const LAND_VALUE: LoopSpec = LoopSpec {
    id: BehaviorLoop::LandValue,
    ensure_bundles: &[],
    unban_bundles: &[],
    fields: &[
        base(0x0E58_5AE0, 500.0),
        base(0x0E58_5AE1, 3.0),
        base(0x0E58_5B00, 500.0),
        base(0x0E58_5B01, 2.0),
    ],
    internal_path_ref: false,
    evidence: "Willis leaf land-value pairs; menu population row binds 0E585AE0",
};

const RESIDENCE: LoopSpec = LoopSpec {
    id: BehaviorLoop::Residence,
    // 5D5EF1FF 住宅通路/建造 sink；EE66978E 住宅占用。不含地标 8A60B969
    // （它带每城唯一的地标闸门，属于身份而不是住人行为）。
    ensure_bundles: &[0x5D5E_F1FF, 0xEE66_978E],
    unban_bundles: &[],
    fields: &[
        base(0x0A79_17CB, 1100.0), // kPropRCI_TotalPopulation
        base(0x0A79_17C2, 1100.0),
        base(0x0E54_6790, 600.0), // 幸福 token 默认
        base(0x0E54_6791, 6600.0),
        base(0x0E54_6792, 66000.0),
    ],
    internal_path_ref: false,
    evidence: "Copan 4DC31F9B inhabited landmark (no jobs)",
};

pub fn spec(id: BehaviorLoop) -> &'static LoopSpec {
    match id {
        BehaviorLoop::Labor => &LABOR,
        BehaviorLoop::Tourism => &TOURISM,
        BehaviorLoop::LandValue => &LAND_VALUE,
        BehaviorLoop::Residence => &RESIDENCE,
    }
}

/// 活跃闭环拥有的全部列（残留剥离的豁免集）。
pub fn owned_fields(loops: &[BehaviorLoop]) -> Vec<u32> {
    let mut out: Vec<u32> = loops
        .iter()
        .flat_map(|id| spec(*id).fields.iter().map(|field| field.hash))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// 活跃闭环挂载的全部 bundle（残留剥离的豁免集）。
pub fn owned_bundles(loops: &[BehaviorLoop]) -> Vec<u32> {
    let mut out: Vec<u32> = loops
        .iter()
        .flat_map(|id| spec(*id).ensure_bundles.iter().copied())
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// 把一个闭环落到指令集：挂 bundle、解封排除表、补基线、声明列。
/// 作者已写的值不覆盖；与其它节点的 remove 声明冲突时闭环胜出并记录。
pub fn apply(resolved: &mut ResolvedBehavior, id: BehaviorLoop) {
    let loop_spec = spec(id);
    for bundle in loop_spec.ensure_bundles {
        resolved.remove_bundles.retain(|candidate| candidate != bundle);
        if !resolved.ensure_bundles.contains(bundle) {
            resolved.ensure_bundles.push(*bundle);
        }
    }
    for bundle in loop_spec.unban_bundles {
        if !resolved.remove_excluded_bundles.contains(bundle) {
            resolved.remove_excluded_bundles.push(*bundle);
        }
    }
    for field in loop_spec.fields {
        resolved.remove_fields.retain(|candidate| *candidate != field.hash);
        let authored = resolved.set_float_fields.contains_key(&field.hash)
            || resolved.set_int_fields.contains_key(&field.hash);
        if !authored {
            let Some(baseline) = field.baseline else {
                continue;
            };
            resolved.set_float_fields.insert(field.hash, baseline);
        }
        if !resolved.declared_unit_columns.contains(&field.hash) {
            resolved.declared_unit_columns.push(field.hash);
        }
    }
    if loop_spec.internal_path_ref {
        resolved.ensure_internal_path_ref = true;
    }
    resolved.notes.push(format!(
        "loop {id:?}: bundles {:?} unban {:?} ({})",
        loop_spec
            .ensure_bundles
            .iter()
            .map(|bundle| format!("{bundle:08X}"))
            .collect::<Vec<_>>(),
        loop_spec
            .unban_bundles
            .iter()
            .map(|bundle| format!("{bundle:08X}"))
            .collect::<Vec<_>>(),
        loop_spec.evidence
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labor_loop_unbans_economy_and_owns_worker_amount() {
        let mut resolved = ResolvedBehavior::default();
        resolved.remove_fields.push(0x0E3F_65A9);
        apply(&mut resolved, BehaviorLoop::Labor);
        assert!(resolved.remove_excluded_bundles.contains(&0x3064_5625));
        assert!(resolved.ensure_bundles.contains(&0xD07C_5A04));
        assert!(resolved.ensure_bundles.contains(&0xF236_BB93));
        assert!(resolved.ensure_internal_path_ref);
        assert_eq!(resolved.set_float_fields.get(&0x0E3F_65A9), Some(&10.0));
        assert_eq!(resolved.set_float_fields.get(&0x0E3F_65B3), Some(&0.0));
        assert_eq!(resolved.set_float_fields.get(&0x0E3F_65B4), Some(&0.0));
        assert!(!resolved.remove_fields.contains(&0x0E3F_65A9));
        assert!(!resolved.remove_fields.contains(&0x0E3F_65B4));
        // 作者专属列不补默认。
        assert!(!resolved.set_float_fields.contains_key(&0x0A79_17C8));
    }

    #[test]
    fn tourism_loop_owns_souvenir_and_freight_columns() {
        let mut resolved = ResolvedBehavior::default();
        resolved.set_float_fields.insert(0x0AD6_A725, 250.0);
        apply(&mut resolved, BehaviorLoop::Tourism);
        assert_eq!(resolved.set_float_fields.get(&0x0E26_60C6), Some(&3300.0));
        assert_eq!(resolved.set_float_fields.get(&0x0C0F_2290), Some(&48.0));
        assert_eq!(resolved.set_float_fields.get(&0x0AD6_A725), Some(&250.0));
        for bundle in [0xCF11_6B9C, 0x2B5A_2BAF, 0x3620_87BD] {
            assert!(resolved.ensure_bundles.contains(&bundle));
        }
    }

    #[test]
    fn residence_loop_never_carries_landmark_gate() {
        let mut resolved = ResolvedBehavior::default();
        apply(&mut resolved, BehaviorLoop::Residence);
        assert!(!resolved.ensure_bundles.contains(&0x8A60_B969));
        assert_eq!(resolved.set_float_fields.get(&0x0A79_17CB), Some(&1100.0));
    }

    #[test]
    fn owned_fields_union_includes_shared_hashes_once() {
        let fields = owned_fields(&[BehaviorLoop::Labor, BehaviorLoop::Tourism]);
        assert!(fields.contains(&0x0E3F_65A9));
        assert!(fields.contains(&0x0E26_60C6));
        assert_eq!(
            fields.iter().filter(|hash| **hash == 0x0AD6_A725).count(),
            1
        );
    }

    #[test]
    fn labor_loop_owns_education_distribution_columns() {
        // 希望大厦 /48 回归：学历分布 6 列必须有 LABOR 归属，否则公园 donor
        // 的学校残留剥离"剥了没人补"（auto-wiring 注入发生在 strip 之前）。
        let fields = owned_fields(&[BehaviorLoop::Labor]);
        for hash in [
            0x0DC1_DB39u32,
            0x0DC1_DB43,
            0x0DC1_DB45,
            0x0DC1_DB5B,
            0x0DC1_DB5C,
            0x0DC1_DB5D,
        ] {
            assert!(fields.contains(&hash), "{hash:08X} not owned by LABOR");
        }
    }
}
