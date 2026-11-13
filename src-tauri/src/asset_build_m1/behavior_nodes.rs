//! 逻辑编排行为节点的编译（2026-11-07）。
//!
//! `semantics.labor` + `semantics.behaviors`（由工作台逻辑图编译而来）在这里
//! 合并成一份 `ResolvedBehavior`——behavior.rs 只消费这份解析结果。
//!
//! 设计约束（docs/re/asset-behavior-census-and-gating-20261106.md §4.2）：
//! 具名 hash 只是看板/规则参数；真正的行为开关是 bundle/bin/sink 链。
//! 因此：
//! - `utilities-core` 的 `inherit` 档不写任何东西（沿用 donor 闭环）；
//! - `explicit` 档写数值 **并确保对应 utility bundle**；
//! - 岗位（W1/W2/W3）只写叶子数值，不伪造岗位 bundle（donor 的岗位 chain
//!   负责闭环；没有它时这些只是看板数字，节点提示里写明）。
//!
//! 冲突一律 fail-closed：同一 hash 两个节点写不同值、ensure/remove 同一
//! bundle、两个不同父模板——都点名节点 id 并拒绝构建。

use super::menu::ELEMENTARY_TEMPLATE;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// 基础行为聚合的 utility 键。
const POWER_RATE: u32 = 0x0AFB_9882;
const POWER_AMOUNT: u32 = 0x0B88_904E;
const POWER_CAPACITY: u32 = 0x0C09_DAA0;
const POWER_CAPACITY_COMPANION: u32 = 0x0C09_DAB2;
const WATER_RATE: u32 = 0x0C09_DA6A;
const WATER_AMOUNT: u32 = 0x0C09_DA65;
const WATER_CAPACITY: u32 = 0x0C09_DA5A;
const WATER_CAPACITY_COMPANION: u32 = 0x0C09_DA6E;
const GARBAGE_AMOUNT: u32 = 0x0C21_6562;

const POWER_BUNDLE: u32 = 0x4893_9D96;
const WATER_BUNDLE: u32 = 0x825B_4CA4;
const GARBAGE_BUNDLE: u32 = 0xD7C6_2954;

/// 岗位（原版小学/大学实证；docs/re/elementary-school-full-dissection-20261106.md:74）。
const JOBS_W1: u32 = 0x0A79_17C5;
const JOBS_W2: u32 = 0x0A79_17C6;
const JOBS_W3: u32 = 0x0A79_17C7;
const MIN_WORKERS: u32 = 0x0A79_17C8;

/// 教育叶子（复用 r7–r10 的最小教育闭环）。
const STUDENT_CAPACITY: u32 = 0x0AD7_FAC9;
const EDUCATION_BOARD: u32 = 0x6A17_85B4;
const EDUCATION_CATEGORY: u32 = 0x95A9_D6FD;
const EDUCATION_BUNDLE: u32 = 0x6C04_2367;
const PARENT_TEMPLATE: u32 = 0x00B2_CCCB;
const BEHAVIOR_CATEGORY: u32 = 0x0AFE_27B1;

/// 观光/地标。
const ATTRACTION: u32 = 0x0E54_2F39;
const TOURISM_INCOME_W1: u32 = 0x0EE2_A50D;
const TOURISM_INCOME_W2: u32 = 0x0EE2_A50E;
const TOURISM_INCOME_W3: u32 = 0x0EE2_A50F;
/// 游客/顾客容量（kPropShopping_TotalShoppers）；前端细调预留参数 shoppers。
const TOURISM_SHOPPERS: u32 = 0x0AD6_A725;
const LANDMARK_TEMPLATE: u32 = 0x4E01_E311;

/// 地价两对（值 + 财富档）。
const LAND_VALUE_A: u32 = 0x0E58_5AE0;
const LAND_WEALTH_A: u32 = 0x0E58_5AE1;
const LAND_VALUE_B: u32 = 0x0E58_5B00;
const LAND_WEALTH_B: u32 = 0x0E58_5B01;

/// 住人（kPropRCI_TotalPopulation + 镜像列）。
const POPULATION: u32 = 0x0A79_17CB;
const POPULATION_MIRROR: u32 = 0x0A79_17C2;

/// 树 bundle（共享；bin draw 在场景层）。
const TREE_BUNDLE: u32 = 0xE769_0450;

/// 岗位声明（W1/W2/W3 + 最低生产人数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LaborSpec {
    pub w1: i32,
    pub w2: i32,
    pub w3: i32,
    pub min_workers: i32,
    /// 用户显式排班（8 班位规模，0=关班）；None = 由后端自动接线补齐。
    pub schedule: Option<[i32; 8]>,
    /// 工人槽 [A9,B3,B4]（0x0E3F65A9/B3/B4，劳工 bundle #07F42755/56/57 的
    /// 数量绑定——各财富档在场工人规模的"extra"杠杆，Willis 量级 75/75/15）。
    /// None = 沿用闭环基线 A9=10/B3=0/B4=0。
    pub slots: Option<[i32; 3]>,
}

/// 表现层请求（教育叶子的 donor 克隆）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PresentationRequest {
    pub package: String,
    /// None = 旧文档缺 presentationUnit（行为阶段明确报错，不猜身份）。
    pub unit_instance: Option<u32>,
    pub adopt_ui_categories: bool,
    pub adopt_stat_rows: bool,
    pub adopt_icon: bool,
}

/// 合并后的行为指令集：behavior.rs 的唯一输入。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ResolvedBehavior {
    pub ensure_bundles: Vec<u32>,
    pub remove_bundles: Vec<u32>,
    pub remove_fields: Vec<u32>,
    pub remove_excluded_bundles: Vec<u32>,
    pub set_float_fields: BTreeMap<u32, f64>,
    pub set_int_fields: BTreeMap<u32, i32>,
    pub parent_template: Option<u32>,
    pub behavior_category: Option<u32>,
    pub student_capacity: Option<u32>,
    pub lot_board: Option<u32>,
    pub ensure_walk_path: bool,
    pub ensure_service_path: bool,
    pub ensure_pedestrian_path: bool,
    pub ensure_internal_path_ref: bool,
    pub presentation: Option<PresentationRequest>,
    pub title_override: Option<String>,
    /// 教育闭环不变量是否适用（legacy / education 叶子）。
    pub education: bool,
    #[serde(skip)]
    pub(super) education_signatures: Vec<u32>,
    /// 树 bundle 不变量是否适用（tree 叶子）。
    pub tree_bundle: bool,
    /// 本次行为声明会改写的 Lot 列（delta-audit 允许集）。
    pub declared_lot_columns: Vec<u32>,
    /// 本次行为声明会改写的 Unit 列（台账/审计用）。
    pub declared_unit_columns: Vec<u32>,
    /// 节点级诊断（进入构建台账，不影响结果）。
    pub notes: Vec<String>,
    /// 行为家族（适配器解析结果；供 menu/donor/labor 下游消费）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<super::behavior_family::BehaviorFamily>,
    /// 适配器选择的 Unit donor 名（如 `generic-park`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_donor: Option<String>,
    /// 适配器选择的 Menu2 骨架（`generic` / `complete-building`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_skeleton: Option<String>,
    /// 岗位自动接线策略。
    #[serde(default)]
    pub labor_profile: super::behavior_family::LaborProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) native_tourism: Option<super::behavior_tourism::NativeTourism>,
    #[serde(skip_serializing_if = "super::behavior_templates::Templates::is_empty")]
    pub(super) templates: super::behavior_templates::Templates,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(super) author_tourism: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) hotel: Option<super::behavior_hotel::Hotel>,
}

impl ResolvedBehavior {
    pub(super) fn record_float(&mut self, hash: u32, value: f64, node: &str) -> Result<(), String> {
        if let Some(previous) = self.set_float_fields.get(&hash) {
            if (previous - value).abs() > 1e-4 {
                return Err(format!(
                    "behavior conflict: nodes write different values to 0x{hash:08X} \
                     ({previous} vs {value}, last writer {node})"
                ));
            }
        }
        self.set_float_fields.insert(hash, value);
        if !self.declared_unit_columns.contains(&hash) {
            self.declared_unit_columns.push(hash);
        }
        Ok(())
    }

    fn ensure_bundle(&mut self, bundle: u32, node: &str) -> Result<(), String> {
        if self.remove_bundles.contains(&bundle) {
            return Err(format!(
                "behavior conflict: node {node} ensures bundle 0x{bundle:08X} but another node removes it"
            ));
        }
        if !self.ensure_bundles.contains(&bundle) {
            self.ensure_bundles.push(bundle);
        }
        Ok(())
    }
}

/// 解析 `semantics.behaviors` 数组（结构 + 类型校验；语义在 compile 内做）。
pub fn parse_nodes(values: &[Value]) -> Result<Vec<Value>, String> {
    let mut ids = BTreeSet::new();
    let mut out = Vec::new();
    for (index, node) in values.iter().enumerate() {
        let Some(object) = node.as_object() else {
            return Err(format!("semantics.behaviors[{index}] must be an object"));
        };
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| format!("semantics.behaviors[{index}]: missing id"))?;
        if !ids.insert(id.to_string()) {
            return Err(format!("semantics.behaviors: duplicate node id {id}"));
        }
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|kind| !kind.is_empty())
            .ok_or_else(|| format!("behaviors node {id}: missing type"))?;
        if !matches!(
            kind,
            "utilities-core" | "education" | "tourism" | "park" | "tree" | "landvalue" | "residence" | "hotel"
        ) {
            return Err(format!("behaviors node {id}: unknown type {kind:?}"));
        }
        out.push(node.clone());
    }
    Ok(out)
}

/// 解析岗位声明（`semantics.labor`）。
///
/// 交互契约（2026-11-11 用户指令）：作者看到 `w1/w2/w3/minWorkers` 的直觉是
/// "这就是我这栋楼需要的劳工数"，但引擎语义是
/// `JobsWx→bin 容量/初值`、`工人槽→WorkersNeededWx`、`minWorkers→红字阈值`。
/// 语义缺失由本函数吸收：**主输入只有 `workers`（这栋楼需要多少劳工）**，
/// 分档、工人槽、开工阈值全部按原版比例自动推导；作者显式字段始终优先。
pub fn parse_labor(value: Option<&Value>) -> Result<Option<LaborSpec>, String> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let object = value
        .as_object()
        .ok_or("semantics.labor must be an object")?;
    let optional = |name: &str| -> Result<Option<i32>, String> {
        match object.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(raw) => {
                let number = raw
                    .as_i64()
                    .or_else(|| raw.as_f64().map(|v| v as i64))
                    .ok_or_else(|| format!("semantics.labor.{name} must be a number"))?;
                i32::try_from(number)
                    .ok()
                    .filter(|v| *v >= 0)
                    .map(Some)
                    .ok_or_else(|| {
                        format!("semantics.labor.{name} must be a non-negative integer")
                    })
            }
        }
    };
    // 主输入：workers = 这栋楼需要的劳工总数。
    let workers = optional("workers")?;
    let tiers = (optional("w1")?, optional("w2")?, optional("w3")?);
    let (w1, w2, w3) = match workers {
        // 显式给了分档 → 用分档（总数以三档之和为准，workers 仅作一致性参考）。
        Some(_) if matches!(tiers, (Some(_), Some(_), Some(_))) => {
            let (a, b, c) = tiers;
            (a.unwrap(), b.unwrap(), c.unwrap())
        }
        // 只给总数 → 全部计入最低收入档（测试城/主城劳动力主力的保守默认）。
        Some(total) => (total, 0, 0),
        None => match tiers {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => {
                return Err(
                    "semantics.labor requires \"workers\" (这栋楼需要的劳工总数) or \
                     w1/w2/w3 tier split"
                        .into(),
                )
            }
        },
    };
    let total = w1.saturating_add(w2).saturating_add(w3);
    // 开工阈值：作者显式优先；缺省按原版地标比例（Willis minWorkers 60 / jobs 360
    // = 1/6；s31 大学 12 / 64 ≈ 1/5.3）取 1/6，保证开工后红字自然消失。
    let min_workers = match optional("minWorkers")? {
        Some(explicit) => explicit,
        None => {
            if total > 0 {
                (total / 6).max(1)
            } else {
                0
            }
        }
    };
    let schedule = match object.get("schedule") {
        None | Some(Value::Null) => None,
        Some(Value::Array(values)) => {
            if values.len() != 8 {
                return Err("semantics.labor.schedule must contain exactly 8 values".into());
            }
            let mut parsed = [0i32; 8];
            for (slot, raw) in values.iter().enumerate() {
                let number = raw
                    .as_i64()
                    .or_else(|| raw.as_f64().map(|v| v as i64))
                    .ok_or_else(|| format!("semantics.labor.schedule[{slot}] must be a number"))?;
                parsed[slot] = i32::try_from(number)
                    .ok()
                    .filter(|v| *v >= 0)
                    .ok_or_else(|| {
                        format!("semantics.labor.schedule[{slot}] must be a non-negative integer")
                    })?;
            }
            Some(parsed)
        }
        Some(_) => return Err("semantics.labor.schedule must be an array of 8 integers".into()),
    };
    let slots = match object.get("workerSlots") {
        None | Some(Value::Null) => None,
        Some(Value::Array(values)) => {
            if values.len() != 3 {
                return Err(
                    "semantics.labor.workerSlots must contain exactly 3 values [A9,B3,B4]".into(),
                );
            }
            let mut parsed = [0i32; 3];
            for (slot, raw) in values.iter().enumerate() {
                let number = raw
                    .as_i64()
                    .or_else(|| raw.as_f64().map(|v| v as i64))
                    .ok_or_else(|| {
                        format!("semantics.labor.workerSlots[{slot}] must be a number")
                    })?;
                parsed[slot] = i32::try_from(number)
                    .ok()
                    .filter(|v| *v >= 0)
                    .ok_or_else(|| {
                        format!(
                            "semantics.labor.workerSlots[{slot}] must be a non-negative integer"
                        )
                    })?;
            }
            Some(parsed)
        }
        Some(_) => {
            return Err(
                "semantics.labor.workerSlots must be an array of 3 integers [A9,B3,B4]".into(),
            )
        }
    };
    Ok(Some(LaborSpec {
        w1,
        w2,
        w3,
        min_workers,
        schedule,
        slots,
    }))
}

pub(super) fn hex(value: &Value, field: &str) -> Result<u32, String> {
    if let Some(number) = value.as_u64() {
        return u32::try_from(number).map_err(|_| format!("{field}: out of range"));
    }
    let text = value
        .as_str()
        .ok_or_else(|| format!("{field} must be a hex string"))?
        .trim();
    let text = text.trim_start_matches("0x");
    u32::from_str_radix(text, 16).map_err(|error| format!("{field}: {error}"))
}

fn hex_list(value: &Value, field: &str) -> Result<Vec<u32>, String> {
    let Some(text) = value.as_str() else {
        if value.is_null() {
            return Ok(Vec::new());
        }
        return Err(format!("{field} must be a comma-separated hex string"));
    };
    let mut out = Vec::new();
    for part in text
        .split([',', ' '])
        .filter(|part| !part.trim().is_empty())
    {
        let part = part.trim().trim_start_matches("0x");
        let id = u32::from_str_radix(part, 16)
            .map_err(|error| format!("{field}: invalid hex {part}: {error}"))?;
        if !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

fn number(value: &Value, field: &str) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| format!("{field} must be a finite non-negative number"))
}

fn flag(value: &Value) -> bool {
    value.as_bool().unwrap_or(false)
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or("").trim().to_string()
}

/// 学历分布 6 列（0DC1DB39/43/45 + 5B/5C/5D，W1/W2/W3 各档需求）。
/// s31 实证配方：W1/W2 减半、W3 等值。2026-11-11 实证所有能吸收劳工的
/// 样本（s31 大学 / Willis 观光 / C-zone 商店 / HTzhiyao）Unit 都带非零
/// 学历分布；缺失时劳工市场按教育程度匹配不到人 → 永远「需要劳工」。
/// 作者显式值优先（只补缺省）。
fn set_education_distribution(resolved: &mut ResolvedBehavior, w1: f64, w2: f64, w3: f64) {
    let mut set_default = |hash: u32, value: f64| {
        if !resolved.set_float_fields.contains_key(&hash) {
            resolved.set_float_fields.insert(hash, value);
            if !resolved.declared_unit_columns.contains(&hash) {
                resolved.declared_unit_columns.push(hash);
            }
        }
    };
    set_default(0x0DC1_DB39, w1 / 2.0);
    set_default(0x0DC1_DB43, w2 / 2.0);
    set_default(0x0DC1_DB45, w3);
    set_default(0x0DC1_DB5B, w1 / 2.0);
    set_default(0x0DC1_DB5C, w2 / 2.0);
    set_default(0x0DC1_DB5D, w3);
}

/// 合并 `labor` + `behaviors` 为可执行指令集（纯函数）。
/// 招工自动接线（2026-11-07 s31 实证配方固化）：前端只填 W1/W2/W3
/// （0A7917C5/C6/C7），后端自动补齐——jobs bundle 挂载、岗位合计叶子
/// （C3=Σ）、minWorkers、八班排班表、学历分布与招工调参常量。
/// 样本来源：vanilla 招工 unit B9217049（w1=20，28 属性最小接线）。
/// 手填值优先：仅当用户未显式写入时才注入默认。
/// legacy（from_legacy）与逻辑图（compile）两条路径共用。
pub(crate) fn apply_labor_auto_wiring(resolved: &mut ResolvedBehavior) {
    use super::behavior_family::LaborProfile;
    const JOB_W1: u32 = 0x0A79_17C5;
    const JOB_W2: u32 = 0x0A79_17C6;
    const JOB_W3: u32 = 0x0A79_17C7;
    const JOB_TOTAL: u32 = 0x0A79_17C3;
    const JOB_MIN_WORKERS: u32 = 0x0A79_17C8;
    const JOBS_BUNDLE_MAIN: u32 = 0xD07C_5A04;
    const JOBS_BUNDLE_AUX: u32 = 0xF236_BB93;
    match resolved.labor_profile {
        LaborProfile::None => return,
        LaborProfile::AuthorOnly => {
            // 只补岗位合计与学历分布；bundle / 排除表解封 / 内部 path 由
            // labor 闭环（behavior_loops::LABOR）统一落地。
            let job_w1 = resolved.set_float_fields.get(&JOB_W1).copied();
            let job_w2 = resolved.set_float_fields.get(&JOB_W2).copied().unwrap_or(0.0);
            let job_w3 = resolved.set_float_fields.get(&JOB_W3).copied().unwrap_or(0.0);
            let Some(w1) = job_w1.filter(|w| *w > 0.0) else {
                return;
            };
            if !resolved.set_float_fields.contains_key(&JOB_TOTAL) {
                resolved
                    .set_float_fields
                    .insert(JOB_TOTAL, w1 + job_w2 + job_w3);
                if !resolved.declared_unit_columns.contains(&JOB_TOTAL) {
                    resolved.declared_unit_columns.push(JOB_TOTAL);
                }
            }
            // 学历分布缺失 = 劳工市场按教育程度匹配不到人（2026-11-11
            // 希望大厦 /48 实证）；紧随其后的家族 strip 靠 LABOR 闭环的
            // 列归属豁免这 6 列，注入必须在 strip 之前。
            set_education_distribution(resolved, w1, job_w2, job_w3);
            // 工人槽默认 = 各财富档岗位数（引擎实证 2026-11-11）：A9/B3/B4
            // 经劳工 bundle 映射表写成 WorkersNeededW1/W2/W3 bin，工人 sink
            // 容量 = 该 bin amount。固定基线 10/0/0 会让任何岗位声明的资产
            // 都只进 ~10 人（"招不满"的真因）；按岗位缩放才是 Willis 形态
            // （Willis 360 岗 / 槽 75/75/15）。作者显式 workerSlots 已在
            // compile 开头写入，or_insert 保证不覆盖。
            {
                const SLOT_A9: u32 = 0x0E3F_65A9;
                const SLOT_B3: u32 = 0x0E3F_65B3;
                const SLOT_B4: u32 = 0x0E3F_65B4;
                for (hash, value) in [(SLOT_A9, w1), (SLOT_B3, job_w2), (SLOT_B4, job_w3)] {
                    resolved.set_float_fields.entry(hash).or_insert(value);
                    if !resolved.declared_unit_columns.contains(&hash) {
                        resolved.declared_unit_columns.push(hash);
                    }
                }
            }
            // 排班缺省 = 原版地标 116 族（Willis 8 班全非零）：作者不需要
            // 知道这 8 列存在，但没有非零班次就没有班次规则 → 工人不循环。
            // 作者显式 schedule 已在 compile 开头写入 set_int_fields。
            if resolved.set_int_fields.is_empty() {
                const LANDMARK_SHIFT: i32 = 116;
                for offset in 0..8u32 {
                    let hash = 0x0D00_853A + offset;
                    // 116/117 交替是原版地标实证值；这里统一 116 略保守。
                    resolved.set_int_fields.insert(hash, LANDMARK_SHIFT);
                    if !resolved.declared_unit_columns.contains(&hash) {
                        resolved.declared_unit_columns.push(hash);
                    }
                }
            }
            return;
        }
        LaborProfile::SchoolSample => {}
    }
    let job_w1 = resolved.set_float_fields.get(&JOB_W1).copied();
    let job_w2 = resolved.set_float_fields.get(&JOB_W2).copied();
    let job_w3 = resolved.set_float_fields.get(&JOB_W3).copied();
    if !job_w1.map_or(false, |w| w > 0.0) {
        return;
    }
    for bundle in [JOBS_BUNDLE_MAIN, JOBS_BUNDLE_AUX] {
        if !resolved.ensure_bundles.contains(&bundle) {
            resolved.ensure_bundles.push(bundle);
        }
    }
    let mut set_default = |hash: u32, value: f64| {
        if !resolved.set_float_fields.contains_key(&hash) {
            resolved.set_float_fields.insert(hash, value);
            if !resolved.declared_unit_columns.contains(&hash) {
                resolved.declared_unit_columns.push(hash);
            }
        }
    };
    let w1 = job_w1.unwrap_or(0.0);
    let w2 = job_w2.unwrap_or(0.0);
    let w3 = job_w3.unwrap_or(0.0);
    set_default(JOB_TOTAL, w1 + w2 + w3);
    set_default(JOB_MIN_WORKERS, 12.0);
    // 招工调参常量（vanilla 学校样本）
    set_default(0x0AD6_A725, 51.0);
    set_default(0x0DA3_EE15, 51.0);
    set_default(0x0C0F_2290, 144.0);
    set_default(0x0E26_60C6, 17.0);
    set_default(0x0E3F_65A9, 10.0);
    set_default(0x0E3F_65B4, 0.0);
    // 学历分布（vanilla 学校样本：W1/W2 减半、W3 等值）
    set_education_distribution(resolved, w1, w2, w3);
    // 八班排班表（vanilla 学校样本；vanilla 为 Int32 标量）
    if resolved.set_int_fields.is_empty() {
        let schedule = [6i32, 6, 7, 7, 6, 6, 7, 6];
        for (offset, value) in schedule.iter().enumerate() {
            let hash = 0x0D00_853A + offset as u32;
            resolved.set_int_fields.insert(hash, *value);
            if !resolved.declared_unit_columns.contains(&hash) {
                resolved.declared_unit_columns.push(hash);
            }
        }
    }
}

pub fn compile(labor: Option<LaborSpec>, nodes: &[Value]) -> Result<ResolvedBehavior, String> {
    compile_with_profile(labor, nodes, false)
}

pub(super) fn compile_with_profile(
    labor: Option<LaborSpec>,
    nodes: &[Value],
    author_tourism: bool,
) -> Result<ResolvedBehavior, String> {
    super::behavior_policy::validate(nodes)?;
    let mut resolved = ResolvedBehavior::default();
    resolved.author_tourism = author_tourism;
    let hotel = super::behavior_hotel::select(nodes)?;
    let labor = hotel.as_ref().map(|recipe| recipe.labor(labor)).or(labor);
    let mut intent =
        super::behavior_family::leaf_intent_from_nodes(nodes, labor.is_some());
    let templates = super::behavior_templates::select(nodes)?;
    if templates.residence() {
        intent.residence = false;
    }
    if templates.land_value() {
        intent.landvalue = false;
    }
    let native_tourism = super::behavior_tourism::select(nodes)?;
    if native_tourism.is_some() && !author_tourism {
        // This recipe installs the visitor bundles explicitly, independent of
        // the identity chosen for education, park and labor.
        intent.tourism = false;
    }
    let family = super::behavior_family::resolve(intent);
    // 先钉家族策略，再写叶子——labor 接线与 strip 都读 labor_profile/family。
    resolved.labor_profile = family.labor_profile;
    resolved.family = Some(family.family);

    if let Some(labor) = labor {
        resolved.record_float(JOBS_W1, f64::from(labor.w1), "labor")?;
        resolved.record_float(JOBS_W2, f64::from(labor.w2), "labor")?;
        resolved.record_float(JOBS_W3, f64::from(labor.w3), "labor")?;
        resolved.record_float(MIN_WORKERS, f64::from(labor.min_workers), "labor")?;
        resolved.declared_unit_columns.push(JOBS_W1);
        resolved.declared_unit_columns.push(JOBS_W2);
        resolved.declared_unit_columns.push(JOBS_W3);
        resolved.declared_unit_columns.push(MIN_WORKERS);
        // 工人槽 A9/B3/B4：作者显式值覆盖 LABOR 闭环基线(10/0/0)——
        // 在场工人规模的 extra 杠杆（Willis 量级 75/75/15）。
        const WORKER_SLOTS: [u32; 3] = [0x0E3F_65A9, 0x0E3F_65B3, 0x0E3F_65B4];
        if let Some(slots) = labor.slots {
            for (hash, value) in WORKER_SLOTS.iter().zip(slots) {
                resolved.record_float(*hash, f64::from(value), "labor")?;
                resolved.declared_unit_columns.push(*hash);
            }
        }
        // 用户显式排班优先；缺省由 apply_labor_auto_wiring 补齐。
        if let Some(schedule) = labor.schedule {
            for (offset, value) in schedule.iter().enumerate() {
                let hash = 0x0D00_853A + offset as u32;
                resolved.set_int_fields.insert(hash, *value);
                if !resolved.declared_unit_columns.contains(&hash) {
                    resolved.declared_unit_columns.push(hash);
                }
            }
            resolved
                .notes
                .push("labor: explicit shift schedule applied".into());
        }
    }

    let mut parent_template: Option<(u32, String)> = None;
    for node in nodes {
        let object = node.as_object().expect("parse_nodes validated objects");
        let id = object["id"].as_str().unwrap_or("behavior").to_string();
        let kind = object["type"].as_str().unwrap_or("");
        let mut params = object.get("params").cloned().unwrap_or(Value::Null);
        if kind == "park" && native_tourism.is_some() {
            if let Some(params) = params.as_object_mut() {
                params.remove("attraction");
            }
        }
        match kind {
            "utilities-core" => compile_utilities(&mut resolved, &id, &params)?,
            "education" => {
                compile_education(&mut resolved, &id, &params)?;
                set_template(&mut parent_template, ELEMENTARY_TEMPLATE, &id)?;
            }
            "tourism" if native_tourism.is_some() => {}
            "tourism" => {
                compile_tourism(&mut resolved, &id, &params)?;
                if author_tourism {
                    continue;
                }
                let template = params
                    .get("templateInstance")
                    .map(|value| hex(value, "tourism.templateInstance"))
                    .transpose()?
                    .unwrap_or(LANDMARK_TEMPLATE);
                // 身份由 park / education 叶子决定时，观光只是嫁接闭环。
                if intent.park || intent.education {
                    resolved.notes.push(format!(
                        "{id}: templateInstance 0x{template:08X} ignored; identity owned by park/education leaf"
                    ));
                } else {
                    set_template(&mut parent_template, template, &id)?;
                }
            }
            "residence" if templates.residence() => {}
            "residence" => compile_residence(&mut resolved, &id, &params)?,
            "hotel" => {}
            "park" => compile_land_values(&mut resolved, &id, &params, true)?,
            "landvalue" if templates.land_value() => {}
            "landvalue" => compile_land_values(&mut resolved, &id, &params, false)?,
            "tree" => {
                if flag(params.get("enabled").unwrap_or(&Value::Bool(true))) {
                    resolved.ensure_bundle(TREE_BUNDLE, &id)?;
                    resolved.tree_bundle = true;
                }
            }
            other => return Err(format!("behaviors node {id}: unknown type {other:?}")),
        }
    }

    if let Some((template, node)) = parent_template {
        resolved.parent_template = Some(template);
        resolved.declared_unit_columns.push(PARENT_TEMPLATE);
        resolved
            .notes
            .push(format!("parent template 0x{template:08X} ({node})"));
    }
    if author_tourism {
        resolved.notes.push("tourism: SC5 Asset Packager native landmark profile".into());
        if templates.residence() && family.loops.contains(&super::behavior_loops::BehaviorLoop::Labor) {
            apply_labor_auto_wiring(&mut resolved);
            super::behavior_loops::apply(&mut resolved, super::behavior_loops::BehaviorLoop::Labor);
        }
    } else {
        apply_labor_auto_wiring(&mut resolved);
        // 叶子写完后再应用家族适配器（strip 学校残留 / 补 donor·menu 元数据）。
        super::behavior_family::apply_to_behavior(&mut resolved, &family);
    }
    if let Some(recipe) = native_tourism {
        recipe.apply(&mut resolved)?;
        resolved.native_tourism = Some(recipe);
    }
    templates.reconcile(&mut resolved);
    resolved.templates = templates;
    resolved.hotel = hotel;
    resolved.declared_lot_columns = lot_columns(
        resolved.lot_board.is_some(),
        resolved.ensure_walk_path
            || resolved.ensure_service_path
            || resolved.ensure_pedestrian_path,
    );
    Ok(resolved)
}

fn set_template(slot: &mut Option<(u32, String)>, template: u32, node: &str) -> Result<(), String> {
    match slot {
        Some((existing, owner)) if *existing != template => Err(format!(
            "behavior conflict: nodes {owner} and {node} require different parent templates \
             (0x{existing:08X} vs 0x{template:08X})"
        )),
        Some(_) => Ok(()),
        None => {
            *slot = Some((template, node.to_string()));
            Ok(())
        }
    }
}

fn compile_utilities(
    resolved: &mut ResolvedBehavior,
    node: &str,
    params: &Value,
) -> Result<(), String> {
    let Some(object) = params.as_object() else {
        return Err(format!(
            "behaviors node {node}: utilities params must be an object"
        ));
    };
    let mut any_explicit = false;
    for (name, bundle, rate, amount, capacity, companion) in [
        (
            "power",
            POWER_BUNDLE,
            Some(POWER_RATE),
            POWER_AMOUNT,
            POWER_CAPACITY,
            POWER_CAPACITY_COMPANION,
        ),
        (
            "water",
            WATER_BUNDLE,
            Some(WATER_RATE),
            WATER_AMOUNT,
            WATER_CAPACITY,
            WATER_CAPACITY_COMPANION,
        ),
        ("garbage", GARBAGE_BUNDLE, None, GARBAGE_AMOUNT, 0, 0),
    ] {
        let Some(utility) = object.get(name) else {
            continue;
        };
        let mode = utility
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("inherit");
        if mode != "explicit" {
            continue;
        }
        any_explicit = true;
        let amount_value = number(
            utility.get("amount").ok_or_else(|| {
                format!("behaviors node {node}: {name}.amount is required for explicit mode")
            })?,
            &format!("{name}.amount"),
        )?;
        if amount_value <= 0.0 {
            return Err(format!(
                "behaviors node {node}: {name}.amount must be greater than 0"
            ));
        }
        resolved.ensure_bundle(bundle, node)?;
        resolved.record_float(amount, amount_value, node)?;
        if let Some(rate_key) = rate {
            if let Some(rate_value) = utility.get("rate").filter(|value| !value.is_null()) {
                let rate_value = number(rate_value, &format!("{name}.rate"))?;
                resolved.record_float(rate_key, rate_value, node)?;
            }
        }
        if capacity != 0 {
            resolved.record_float(capacity, amount_value * 3.0, node)?;
        }
        if companion != 0 {
            resolved.record_float(companion, amount_value * 3.0, node)?;
        }
    }
    if !any_explicit {
        resolved
            .notes
            .push(format!("{node}: utilities inherit the donor loop"));
    }
    Ok(())
}

fn compile_education(
    resolved: &mut ResolvedBehavior,
    node: &str,
    params: &Value,
) -> Result<(), String> {
    resolved.education = true;
    resolved.behavior_category = Some(EDUCATION_CATEGORY);
    resolved.declared_unit_columns.push(BEHAVIOR_CATEGORY);
    // 教育签名可覆盖（默认=小学 6C042367；社區大學身份传 6C042366——
    // 与 s24+ 配方一致，避免 elementary/college 双签名同时挂载）。
    let signature_bundle = params
        .get("signatureBundle")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            hex(
                &Value::String(value.to_string()),
                "education.signatureBundle",
            )
        })
        .transpose()?
        .unwrap_or(EDUCATION_BUNDLE);
    resolved.ensure_bundle(signature_bundle, node)?;
    if !resolved.education_signatures.contains(&signature_bundle) {
        resolved.education_signatures.push(signature_bundle);
    }
    if resolved.lot_board.is_none() {
        resolved.lot_board = Some(EDUCATION_BOARD);
    }

    if let Some(capacity) = params.get("capacity").filter(|value| !value.is_null()) {
        let capacity = number(capacity, "education.capacity")?;
        if capacity <= 0.0 {
            return Err(format!(
                "behaviors node {node}: education.capacity must be > 0"
            ));
        }
        resolved.student_capacity = Some(capacity as u32);
        resolved.declared_unit_columns.push(STUDENT_CAPACITY);
    }
    resolved.ensure_walk_path |= flag(params.get("walkPath").unwrap_or(&Value::Bool(false)));
    resolved.ensure_service_path |= flag(params.get("servicePath").unwrap_or(&Value::Bool(false)));
    resolved.ensure_pedestrian_path |=
        flag(params.get("pedestrianPath").unwrap_or(&Value::Bool(false)));

    if let Some(board) = params
        .get("boardInstance")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|board| !board.is_empty())
    {
        resolved.lot_board = Some(hex(
            &Value::String(board.to_string()),
            "education.boardInstance",
        )?);
    }

    for bundle in hex_list(
        params.get("signatureBundles").unwrap_or(&Value::Null),
        "education.signatureBundles",
    )? {
        resolved.ensure_bundle(bundle, node)?;
    }
    for bundle in hex_list(
        params.get("removeBundles").unwrap_or(&Value::Null),
        "education.removeBundles",
    )? {
        if resolved.ensure_bundles.contains(&bundle) {
            return Err(format!(
                "behavior conflict: node {node} removes bundle 0x{bundle:08X} that another node ensures"
            ));
        }
        resolved.remove_bundles.push(bundle);
    }

    if let Some(title) = params
        .get("titleOverride")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        resolved.title_override = Some(title.to_string());
    }

    let package = text(params.get("presentationPackage").unwrap_or(&Value::Null));
    if !package.is_empty() {
        let unit = hex(
            params.get("presentationUnit").ok_or_else(|| {
                format!(
                    "behaviors node {node}: presentationUnit is required with presentationPackage"
                )
            })?,
            "education.presentationUnit",
        )?;
        resolved.presentation = Some(PresentationRequest {
            package,
            unit_instance: Some(unit),
            adopt_ui_categories: true,
            adopt_stat_rows: true,
            adopt_icon: true,
        });
    }
    Ok(())
}

fn compile_tourism(
    resolved: &mut ResolvedBehavior,
    node: &str,
    params: &Value,
) -> Result<(), String> {
    if let Some(attraction) = params.get("attraction").filter(|value| !value.is_null()) {
        resolved.record_float(ATTRACTION, number(attraction, "tourism.attraction")?, node)?;
    }
    for (key, hash) in [
        ("incomeW1", TOURISM_INCOME_W1),
        ("incomeW2", TOURISM_INCOME_W2),
        ("incomeW3", TOURISM_INCOME_W3),
    ] {
        if let Some(value) = params.get(key).filter(|value| !value.is_null()) {
            resolved.record_float(hash, number(value, &format!("tourism.{key}"))?, node)?;
        }
    }
    // Tourism assets frequently reuse an education donor. Remove the donor's
    // school/jobs bundle set explicitly; otherwise the landmark tourism parent
    // is present but the Unit still runs the school economy. Removing from
    // `ensure_bundles` too is required because behavior.rs unions that list
    // after applying remove_bundles.
    if let Some(value) = params
        .get("removeBundles")
        .filter(|value| !value.is_null())
    {
        let bundles = match value.as_array() {
            Some(items) => items
                .iter()
                .map(|item| hex(item, "tourism.removeBundles"))
                .collect::<Result<Vec<_>, _>>()?,
            None => hex_list(value, "tourism.removeBundles")?,
        };
        for bundle in bundles {
            resolved.ensure_bundles.retain(|candidate| *candidate != bundle);
            if !resolved.remove_bundles.contains(&bundle) {
                resolved.remove_bundles.push(bundle);
            }
        }
    }
    // The shared "complete building" Unit donor is still the s31 university
    // leaf: student counts, grade distributions, jobs and shift schedules make
    // the game render a school overlay ("186 学生") even when the parent is the
    // landmark tourism template. Remove those declared fields explicitly.
    if let Some(value) = params
        .get("removeFields")
        .filter(|value| !value.is_null())
    {
        let fields = match value.as_array() {
            Some(items) => items
                .iter()
                .map(|item| hex(item, "tourism.removeFields"))
                .collect::<Result<Vec<_>, _>>()?,
            None => hex_list(value, "tourism.removeFields")?,
        };
        for field in fields {
            if !resolved.remove_fields.contains(&field) {
                resolved.remove_fields.push(field);
            }
        }
    }
    // 预留：前端观光叶细调的「消费者数量」。shoppers = 游客/顾客容量
    // （kPropShopping_TotalShoppers，0AD6A725——地标语境的游客容量；教育
    // 语境的招工常量是同哈希跨族复用）。前端尚未提供输入时，构建器按
    // 威利斯叶基线自动填充（见 behavior_family）。
    if let Some(value) = params.get("shoppers").filter(|value| !value.is_null()) {
        resolved.record_float(TOURISM_SHOPPERS, number(value, "tourism.shoppers")?, node)?;
    }
    Ok(())
}

/// 住人闭环参数：`population` 覆盖 Copan 基线人口（主列 + 镜像列）。
fn compile_residence(
    resolved: &mut ResolvedBehavior,
    node: &str,
    params: &Value,
) -> Result<(), String> {
    if let Some(value) = params.get("population").filter(|value| !value.is_null()) {
        let population = number(value, "residence.population")?;
        resolved.record_float(POPULATION, population, node)?;
        resolved.record_float(POPULATION_MIRROR, population, node)?;
    }
    Ok(())
}

fn compile_land_values(
    resolved: &mut ResolvedBehavior,
    node: &str,
    params: &Value,
    with_attraction: bool,
) -> Result<(), String> {
    for (key, hash) in [
        ("landValue", LAND_VALUE_A),
        ("landWealth", LAND_WEALTH_A),
        ("landValue2", LAND_VALUE_B),
        ("landWealth2", LAND_WEALTH_B),
        ("value", LAND_VALUE_A),
        ("wealth", LAND_WEALTH_A),
        ("value2", LAND_VALUE_B),
        ("wealth2", LAND_WEALTH_B),
    ] {
        if let Some(value) = params.get(key).filter(|value| !value.is_null()) {
            resolved.record_float(hash, number(value, key)?, node)?;
        }
    }
    if with_attraction {
        if let Some(attraction) = params.get("attraction").filter(|value| !value.is_null()) {
            resolved.record_float(ATTRACTION, number(attraction, "park.attraction")?, node)?;
        }
    }
    Ok(())
}

/// legacy `semantics.behavior` → 同一份指令集（保持 r7–r10 字节行为）。
///
/// `menu_mode`：目标身份是菜单阶段新分配的 Unit/Lot（Menu2/Unit/Lot 由
/// `menu::generate` 生成）。树的 "frozen look" 不变量只对全克隆 donor 身份
/// 成立，菜单身份由菜单生成器负责其 bundle 集，因此此处不再强制 E7690450。
pub fn from_legacy(spec: &super::behavior::BehaviorSpec, menu_mode: bool) -> ResolvedBehavior {
    let mut resolved = ResolvedBehavior {
        ensure_bundles: if spec.ensure_bundles.is_empty() {
            vec![EDUCATION_BUNDLE]
        } else {
            spec.ensure_bundles.clone()
        },
        remove_bundles: spec.remove_bundles.clone(),
        remove_fields: spec.remove_fields.clone(),
        remove_excluded_bundles: spec.remove_excluded_bundles.clone(),
        parent_template: Some(
            spec.simulation_template_instance
                .unwrap_or(ELEMENTARY_TEMPLATE),
        ),
        behavior_category: Some(
            spec.behavior_category_instance
                .unwrap_or(EDUCATION_CATEGORY),
        ),
        student_capacity: spec.student_capacity,
        lot_board: Some(spec.lot_board_instance.unwrap_or(EDUCATION_BOARD)),
        ensure_walk_path: spec.ensure_walk_path,
        ensure_service_path: spec.ensure_service_path,
        ensure_pedestrian_path: spec.ensure_pedestrian_path,
        ensure_internal_path_ref: spec.ensure_internal_path_ref,
        title_override: spec.title_override.clone(),
        education: true,
        tree_bundle: !menu_mode,
        ..Default::default()
    };
    for (hash, value) in &spec.set_float_fields {
        if let Ok(hash) = u32::from_str_radix(hash.trim_start_matches("0x"), 16) {
            resolved.set_float_fields.insert(hash, *value);
            resolved.declared_unit_columns.push(hash);
        }
    }
    for (hash, value) in &spec.set_int_fields {
        if let Ok(hash) = u32::from_str_radix(hash.trim_start_matches("0x"), 16) {
            resolved.set_int_fields.insert(hash, *value as i32);
            resolved.declared_unit_columns.push(hash);
        }
    }
    // 招工自动接线（2026-11-07 s31 实证配方固化）：前端只填 W1/W2/W3
    // （0A7917C5/C6/C7），后端自动补齐——jobs bundle 挂载、岗位合计叶子
    // （C3=Σ）、minWorkers、八班排班表、学历分布与招工调参常量。
    // 样本来源：vanilla 招工 unit B9217049（w1=20，28 属性最小接线）。
    // 手填值优先：仅当用户未显式写入时才注入默认。
    apply_labor_auto_wiring(&mut resolved);
    for hash in [
        PARENT_TEMPLATE,
        BEHAVIOR_CATEGORY,
        STUDENT_CAPACITY,
        JOBS_W1,
        JOBS_W2,
        JOBS_W3,
        MIN_WORKERS,
    ] {
        if !resolved.declared_unit_columns.contains(&hash) {
            resolved.declared_unit_columns.push(hash);
        }
    }
    if let Some(package) = &spec.presentation_package {
        resolved.presentation = Some(PresentationRequest {
            package: package.clone(),
            unit_instance: spec.presentation_unit_instance,
            adopt_ui_categories: spec.adopt_ui_categories,
            adopt_stat_rows: spec.adopt_stat_rows,
            adopt_icon: spec.adopt_icon,
        });
    }
    resolved.declared_lot_columns = lot_columns(
        resolved.lot_board.is_some(),
        resolved.ensure_walk_path
            || resolved.ensure_service_path
            || resolved.ensure_pedestrian_path,
    );
    resolved
}

pub fn lot_columns(board: bool, paths: bool) -> Vec<u32> {
    let mut out = Vec::new();
    if board {
        out.push(0x0D79_70ED);
    }
    if paths {
        out.extend([
            0x0CAA_680D,
            0x0CB0_0ED8,
            0x0CAA_6832,
            0x0CAA_6841,
            0x0CC8_FE7A,
            0x0D9A_4E54,
            0x0D9A_A2BB,
        ]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: &str, kind: &str, params: Value) -> Value {
        json!({"id": id, "type": kind, "params": params})
    }

    #[test]
    fn labor_maps_to_verified_job_hashes() {
        let labor = parse_labor(Some(&json!({"w1": 20, "w2": 5, "w3": 0, "minWorkers": 5})))
            .unwrap()
            .unwrap();
        let resolved = compile(Some(labor), &[]).unwrap();
        assert_eq!(resolved.set_float_fields[&JOBS_W1], 20.0);
        assert_eq!(resolved.set_float_fields[&JOBS_W3], 0.0);
        assert_eq!(resolved.set_float_fields[&MIN_WORKERS], 5.0);
    }

    #[test]
    fn workers_alone_expands_to_all_engine_columns() {
        // 直觉输入：作者只填「这栋楼需要多少劳工」。后端负责翻译成引擎列：
        // 全部计入最低收入档、开工阈值按原版比例 1/6、工人槽按岗位缩放。
        let labor = parse_labor(Some(&json!({"workers": 30})))
            .unwrap()
            .unwrap();
        assert_eq!((labor.w1, labor.w2, labor.w3), (30, 0, 0));
        assert_eq!(labor.min_workers, 5); // 30/6，原版地标比例（Willis 60/360）
        assert!(labor.slots.is_none(), "槽由 auto-wiring 按岗位缩放");
        let resolved = compile(Some(labor), &[]).unwrap();
        assert_eq!(resolved.set_float_fields[&0x0A79_17C3], 30.0);
        assert_eq!(resolved.set_float_fields[&0x0A79_17C8], 5.0);
        assert_eq!(resolved.set_float_fields[&0x0E3F_65A9], 30.0); // 工人槽 = 岗位
        assert_eq!(resolved.set_int_fields[&0x0D00_853A], 116); // 原版地标排班
        assert_eq!(resolved.set_int_fields[&0x0D00_8541], 116);
    }

    #[test]
    fn workers_with_explicit_tiers_and_min_workers_wins() {
        let labor = parse_labor(Some(&json!({
            "workers": 30, "w1": 20, "w2": 10, "w3": 0, "minWorkers": 3
        })))
        .unwrap()
        .unwrap();
        assert_eq!((labor.w1, labor.w2, labor.w3), (20, 10, 0));
        assert_eq!(labor.min_workers, 3);
        let resolved = compile(Some(labor), &[]).unwrap();
        assert_eq!(resolved.set_float_fields[&0x0A79_17C3], 30.0);
        assert_eq!(resolved.set_float_fields[&0x0E3F_65B3], 10.0);
    }

    #[test]
    fn labor_without_workers_or_tiers_is_rejected_with_guidance() {
        let error = parse_labor(Some(&json!({"minWorkers": 5}))).unwrap_err();
        assert!(error.contains("workers"), "{error}");
    }

    #[test]
    fn author_only_labor_scales_worker_slots_with_jobs_by_default() {
        // 引擎实证（2026-11-11）：工人槽 A9/B3/B4 决定实际到岗数（写入
        // WorkersNeededWx bin，sink 容量 = 该 bin amount）。缺省必须按岗位
        // 缩放，否则固定基线 10 让任何规模的资产都只进 ~10 人。
        let mut resolved = ResolvedBehavior::default();
        resolved.labor_profile = crate::asset_build_m1::behavior_family::LaborProfile::AuthorOnly;
        resolved.set_float_fields.insert(JOBS_W1, 30.0);
        resolved.set_float_fields.insert(JOBS_W2, 0.0);
        resolved.set_float_fields.insert(JOBS_W3, 0.0);
        apply_labor_auto_wiring(&mut resolved);
        assert_eq!(resolved.set_float_fields[&0x0E3F_65A9], 30.0);
        assert_eq!(resolved.set_float_fields[&0x0E3F_65B3], 0.0);
        assert_eq!(resolved.set_float_fields[&0x0E3F_65B4], 0.0);
        // 作者显式值不被覆盖（compile 已先写入显式 workerSlots）。
        resolved.set_float_fields.insert(0x0E3F_65A9, 75.0);
        apply_labor_auto_wiring(&mut resolved);
        assert_eq!(resolved.set_float_fields[&0x0E3F_65A9], 75.0);
    }

    #[test]
    fn labor_worker_slots_override_loop_baselines() {
        // Willis 量级：ploppable 资产配到百级在场工人的 extra 杠杆。
        let labor = parse_labor(Some(&json!({
            "w1": 150, "w2": 150, "w3": 60, "minWorkers": 60,
            "workerSlots": [75, 75, 15]
        })))
        .unwrap()
        .unwrap();
        assert_eq!(labor.slots, Some([75, 75, 15]));
        let resolved = compile(Some(labor), &[]).unwrap();
        assert_eq!(resolved.set_float_fields[&0x0E3F_65A9], 75.0);
        assert_eq!(resolved.set_float_fields[&0x0E3F_65B3], 75.0);
        assert_eq!(resolved.set_float_fields[&0x0E3F_65B4], 15.0);
    }

    #[test]
    fn author_only_labor_writes_education_distribution() {
        // 希望大厦 /48 回归（2026-11-11）：AuthorOnly 只补岗位合计导致学历
        // 分布全链缺失——所有能招工样本（s31/Willis/C-zone/HTzhiyao）都带
        // 非零学历分布，缺失 = 劳工市场匹配不到人，永远「需要劳工」。
        let mut resolved = ResolvedBehavior::default();
        resolved.labor_profile = crate::asset_build_m1::behavior_family::LaborProfile::AuthorOnly;
        resolved.set_float_fields.insert(JOBS_W1, 90.0);
        resolved.set_float_fields.insert(JOBS_W2, 0.0);
        resolved.set_float_fields.insert(JOBS_W3, 20.0);
        apply_labor_auto_wiring(&mut resolved);
        assert_eq!(resolved.set_float_fields[&0x0DC1_DB39], 45.0); // W1/2
        assert_eq!(resolved.set_float_fields[&0x0DC1_DB43], 0.0); // W2/2
        assert_eq!(resolved.set_float_fields[&0x0DC1_DB45], 20.0); // W3
        assert_eq!(resolved.set_float_fields[&0x0DC1_DB5B], 45.0);
        assert_eq!(resolved.set_float_fields[&0x0DC1_DB5C], 0.0);
        assert_eq!(resolved.set_float_fields[&0x0DC1_DB5D], 20.0);
        // 作者显式值优先：已写的列不覆盖。
        resolved.set_float_fields.insert(0x0DC1_DB39, 7.0);
        apply_labor_auto_wiring(&mut resolved);
        assert_eq!(resolved.set_float_fields[&0x0DC1_DB39], 7.0);
    }

    #[test]
    fn explicit_power_ensures_bundle_and_derives_capacity() {
        let nodes = vec![node(
            "core-1",
            "utilities-core",
            json!({
                "power": {"mode": "explicit", "rate": 72, "amount": 90},
                "water": {"mode": "inherit", "rate": null, "amount": null}
            }),
        )];
        let resolved = compile(None, &nodes).unwrap();
        assert!(resolved.ensure_bundles.contains(&POWER_BUNDLE));
        assert!(!resolved.ensure_bundles.contains(&WATER_BUNDLE));
        assert_eq!(resolved.set_float_fields[&POWER_AMOUNT], 90.0);
        assert_eq!(resolved.set_float_fields[&POWER_CAPACITY], 270.0);
        assert_eq!(resolved.set_float_fields[&POWER_CAPACITY_COMPANION], 270.0);
        assert_eq!(resolved.set_float_fields[&POWER_RATE], 72.0);
    }

    #[test]
    fn conflicting_float_writers_are_rejected_with_node_ids() {
        let nodes = vec![
            node("leaf-1", "tourism", json!({"attraction": 400})),
            node("leaf-2", "park", json!({"attraction": 500})),
        ];
        let error = compile(None, &nodes).unwrap_err();
        assert!(error.contains("0E542F39"), "{error}");
        assert!(error.contains("leaf-2"), "{error}");
    }

    #[test]
    fn education_leaf_carries_the_verified_recipe() {
        let nodes = vec![node(
            "leaf-1",
            "education",
            json!({
                "capacity": 250,
                "walkPath": true,
                "servicePath": true,
                "pedestrianPath": false,
                "boardInstance": "0x6A1785B5",
                "presentationPackage": "build/donor.package",
                "presentationUnit": "0xA826FA3E"
            }),
        )];
        let resolved = compile(None, &nodes).unwrap();
        assert!(resolved.education);
        assert_eq!(resolved.student_capacity, Some(250));
        assert_eq!(resolved.lot_board, Some(0x6A17_85B5));
        assert!(resolved.ensure_walk_path);
        assert!(resolved.ensure_service_path);
        assert!(!resolved.ensure_pedestrian_path);
        assert!(resolved.ensure_bundles.contains(&EDUCATION_BUNDLE));
        assert_eq!(
            resolved.presentation.as_ref().and_then(|p| p.unit_instance),
            Some(0xA826_FA3E)
        );
        assert!(resolved.declared_lot_columns.contains(&0x0CAA_680D));
    }

    #[test]
    fn tree_leaf_declares_bundle_and_invariant() {
        let nodes = vec![node("leaf-1", "tree", json!({"enabled": true}))];
        let resolved = compile(None, &nodes).unwrap();
        assert!(resolved.tree_bundle);
        assert!(resolved.ensure_bundles.contains(&TREE_BUNDLE));
    }

    #[test]
    fn identity_leaf_owns_parent_over_grafted_tourism_template() {
        let nodes = vec![
            node("leaf-1", "education", json!({"capacity": 100})),
            node(
                "leaf-2",
                "tourism",
                json!({"templateInstance": "0x565B72B9"}),
            ),
        ];
        let resolved = compile(None, &nodes).unwrap();
        assert_eq!(resolved.parent_template, Some(ELEMENTARY_TEMPLATE));
        assert!(resolved.ensure_bundles.contains(&0xCF11_6B9C));
        assert!(resolved
            .notes
            .iter()
            .any(|note| note.contains("565B72B9") && note.contains("ignored")));
    }

    #[test]
    fn ensure_and_remove_same_bundle_is_rejected() {
        let nodes = vec![node(
            "leaf-1",
            "education",
            json!({"capacity": 100, "removeBundles": "0x6C042367"}),
        )];
        let error = compile(None, &nodes).unwrap_err();
        assert!(error.contains("6C042367"), "{error}");
    }

    #[test]
    fn unknown_node_type_is_rejected_at_parse() {
        let values = vec![node("x", "teleport", json!({}))];
        assert!(parse_nodes(&values).is_err());
    }

    #[test]
    fn park_tourism_family_strips_school_and_keeps_park_identity() {
        let labor = parse_labor(Some(&json!({
            "w1": 30, "w2": 20, "w3": 0, "minWorkers": 10
        })))
        .unwrap()
        .unwrap();
        let nodes = vec![
            node(
                "leaf-park",
                "park",
                json!({"landValue": 500, "landWealth": 3}),
            ),
            node(
                "leaf-tourism",
                "tourism",
                json!({
                    "attraction": 400,
                    "incomeW1": 11,
                    "templateInstance": "0x4E01E311"
                }),
            ),
        ];
        let resolved = compile(Some(labor), &nodes).unwrap();
        assert_eq!(
            resolved.family,
            Some(crate::asset_build_m1::behavior_family::BehaviorFamily::ParkTourism)
        );
        assert_eq!(resolved.unit_donor.as_deref(), Some("generic-park"));
        assert_eq!(resolved.menu_skeleton.as_deref(), Some("generic"));
        // 实测：parent=7E4B99E5（教育模板）时夜景亮灯；地标模板会熄灯。
        assert_eq!(resolved.parent_template, Some(ELEMENTARY_TEMPLATE));
        assert!(resolved.remove_bundles.contains(&0x6C04_2366));
        // labor 多闭环：教育签名剥掉，学校劳工包保留（实测可招工组合）。
        assert!(resolved.ensure_bundles.contains(&0xD07C_5A04));
        assert!(!resolved.remove_fields.contains(&0x0DA3_EE15));
        assert_eq!(resolved.set_float_fields[&0x0DA3_EE15], 51.0);
        assert_eq!(resolved.set_float_fields[&JOBS_W1], 30.0);
    }

    #[test]
    fn author_only_labor_composes_hiring_loop_like_s31() {
        // 公园+观光+labor 多闭环（对拍可放置 ce2ee50a）：解封 30645625 +
        // 保留/ensure jobs bundle + 内部 path；教育签名仍剥离。
        let labor = parse_labor(Some(&json!({
            "w1": 30, "w2": 20, "w3": 0, "minWorkers": 30
        })))
        .unwrap()
        .unwrap();
        let nodes = vec![
            node("leaf-park", "park", json!({})),
            node(
                "leaf-tourism",
                "tourism",
                json!({"attraction": 400, "templateInstance": "0x4E01E311"}),
            ),
        ];
        let resolved = compile(Some(labor), &nodes).unwrap();
        assert_eq!(
            resolved.labor_profile,
            crate::asset_build_m1::behavior_family::LaborProfile::AuthorOnly
        );
        assert!(resolved.remove_excluded_bundles.contains(&0x3064_5625));
        assert!(resolved.ensure_bundles.contains(&0xD07C_5A04));
        assert!(resolved.ensure_bundles.contains(&0xF236_BB93));
        assert!(resolved.remove_bundles.contains(&0x6C04_2366));
        assert!(resolved.remove_bundles.contains(&0x6C04_2367));
        assert!(!resolved.ensure_bundles.contains(&0x6C04_2366));
        assert!(
            resolved.ensure_internal_path_ref,
            "AuthorOnly labor must carry s31 door-finding path ref"
        );
    }

    #[test]
    fn overwide_tourism_remove_fields_cannot_strip_authored_labor() {
        // /39 事故：schema removeFields 含 0A7917C3–C8 / 0D00853A–41，
        // 作者 labor 写回后必须从 remove 列表豁免并保留 set 值。
        let labor = parse_labor(Some(&json!({
            "w1": 30, "w2": 20, "w3": 0, "minWorkers": 30,
            "schedule": [0, 0, 7, 7, 6, 6, 0, 0]
        })))
        .unwrap()
        .unwrap();
        let nodes = vec![node(
            "leaf-tourism",
            "tourism",
            json!({
                "attraction": 400,
                "removeFields": [
                    "0AD6A725", "0A7917C3", "0A7917C5", "0A7917C6", "0A7917C7", "0A7917C8",
                    "0D00853A", "0D00853B", "0D00853C", "0D00853D",
                    "0D00853E", "0D00853F", "0D008540", "0D008541",
                    "0AD7FAC9"
                ]
            }),
        )];
        let resolved = compile(Some(labor), &nodes).unwrap();
        assert_eq!(resolved.set_float_fields[&JOBS_W1], 30.0);
        assert_eq!(resolved.set_float_fields[&MIN_WORKERS], 30.0);
        assert_eq!(resolved.set_int_fields[&0x0D00_853A], 0);
        assert_eq!(resolved.set_int_fields[&0x0D00_853C], 7);
        assert!(!resolved.remove_fields.contains(&JOBS_W1));
        assert!(!resolved.remove_fields.contains(&0x0A79_17C3));
        assert!(!resolved.remove_fields.contains(&0x0D00_853A));
        assert!(resolved.remove_fields.contains(&0x0AD7_FAC9));
        assert_eq!(resolved.set_float_fields[&TOURISM_SHOPPERS], 390.0);
        assert_eq!(resolved.set_float_fields[&LAND_VALUE_A], 500.0);
    }

    #[test]
    fn education_family_keeps_school_sample_labor() {
        let labor = parse_labor(Some(&json!({"w1": 20, "w2": 5, "w3": 0, "minWorkers": 5})))
            .unwrap()
            .unwrap();
        let nodes = vec![node("leaf-1", "education", json!({"capacity": 100}))];
        let resolved = compile(Some(labor), &nodes).unwrap();
        assert_eq!(
            resolved.family,
            Some(crate::asset_build_m1::behavior_family::BehaviorFamily::Education)
        );
        assert_eq!(resolved.unit_donor.as_deref(), Some("elementary-school"));
        assert!(resolved.ensure_bundles.contains(&0xD07C_5A04));
        assert_eq!(resolved.set_float_fields[&0x0DA3_EE15], 51.0);
        assert!(!resolved.remove_fields.contains(&0x0DA3_EE15));
    }
}
