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
const LANDMARK_TEMPLATE: u32 = 0x4E01_E311;

/// 地价两对（值 + 财富档）。
const LAND_VALUE_A: u32 = 0x0E58_5AE0;
const LAND_WEALTH_A: u32 = 0x0E58_5AE1;
const LAND_VALUE_B: u32 = 0x0E58_5B00;
const LAND_WEALTH_B: u32 = 0x0E58_5B01;

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
}

impl ResolvedBehavior {
    fn record_float(&mut self, hash: u32, value: f64, node: &str) -> Result<(), String> {
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
            "utilities-core" | "education" | "tourism" | "park" | "tree" | "landvalue"
        ) {
            return Err(format!("behaviors node {id}: unknown type {kind:?}"));
        }
        out.push(node.clone());
    }
    Ok(out)
}

/// 解析岗位声明（`semantics.labor`）。
pub fn parse_labor(value: Option<&Value>) -> Result<Option<LaborSpec>, String> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let object = value
        .as_object()
        .ok_or("semantics.labor must be an object")?;
    let field = |name: &str| -> Result<i32, String> {
        let raw = object
            .get(name)
            .ok_or_else(|| format!("semantics.labor.{name} is required"))?;
        let number = raw
            .as_i64()
            .or_else(|| raw.as_f64().map(|v| v as i64))
            .ok_or_else(|| format!("semantics.labor.{name} must be a number"))?;
        i32::try_from(number)
            .ok()
            .filter(|v| *v >= 0)
            .ok_or_else(|| format!("semantics.labor.{name} must be a non-negative integer"))
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
    Ok(Some(LaborSpec {
        w1: field("w1")?,
        w2: field("w2")?,
        w3: field("w3")?,
        min_workers: field("minWorkers")?,
        schedule,
    }))
}

fn hex(value: &Value, field: &str) -> Result<u32, String> {
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
            // 只补岗位合计；不挂学校 jobs bundle / 学历 / 招工常量。
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
    // 学历分布（vanilla 学校样本：W1/W2 减半、W3 等值）
    set_default(0x0DC1_DB39, w1 / 2.0);
    set_default(0x0DC1_DB43, w2 / 2.0);
    set_default(0x0DC1_DB45, w3);
    set_default(0x0DC1_DB5B, w1 / 2.0);
    set_default(0x0DC1_DB5C, w2 / 2.0);
    set_default(0x0DC1_DB5D, w3);
    // 招工调参常量（vanilla 学校样本）
    set_default(0x0AD6_A725, 51.0);
    set_default(0x0DA3_EE15, 51.0);
    set_default(0x0C0F_2290, 144.0);
    set_default(0x0E26_60C6, 17.0);
    set_default(0x0E3F_65A9, 10.0);
    set_default(0x0E3F_65B4, 0.0);
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
    let mut resolved = ResolvedBehavior::default();
    let intent =
        super::behavior_family::leaf_intent_from_nodes(nodes, labor.is_some());
    let family = super::behavior_family::resolve(intent);
    // 先钉家族策略，再写叶子——labor 接线与 strip 都读 labor_profile。
    resolved.labor_profile = family.labor_profile;

    if let Some(labor) = labor {
        resolved.record_float(JOBS_W1, f64::from(labor.w1), "labor")?;
        resolved.record_float(JOBS_W2, f64::from(labor.w2), "labor")?;
        resolved.record_float(JOBS_W3, f64::from(labor.w3), "labor")?;
        resolved.record_float(MIN_WORKERS, f64::from(labor.min_workers), "labor")?;
        resolved.declared_unit_columns.push(JOBS_W1);
        resolved.declared_unit_columns.push(JOBS_W2);
        resolved.declared_unit_columns.push(JOBS_W3);
        resolved.declared_unit_columns.push(MIN_WORKERS);
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
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        match kind {
            "utilities-core" => compile_utilities(&mut resolved, &id, &params)?,
            "education" => {
                compile_education(&mut resolved, &id, &params)?;
                set_template(&mut parent_template, ELEMENTARY_TEMPLATE, &id)?;
            }
            "tourism" => {
                compile_tourism(&mut resolved, &id, &params)?;
                let template = params
                    .get("templateInstance")
                    .map(|value| hex(value, "tourism.templateInstance"))
                    .transpose()?
                    .unwrap_or(LANDMARK_TEMPLATE);
                set_template(&mut parent_template, template, &id)?;
            }
            "park" => compile_land_values(&mut resolved, &id, &params, true)?,
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
    apply_labor_auto_wiring(&mut resolved);
    // 叶子写完后再应用家族适配器（strip 学校残留 / 补 donor·menu 元数据）。
    super::behavior_family::apply_to_behavior(&mut resolved, &family);
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
    fn conflicting_parent_templates_are_rejected() {
        let nodes = vec![
            node("leaf-1", "education", json!({"capacity": 100})),
            node(
                "leaf-2",
                "tourism",
                json!({"templateInstance": "0x565B72B9"}),
            ),
        ];
        let error = compile(None, &nodes).unwrap_err();
        assert!(error.contains("different parent templates"), "{error}");
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
    fn park_tourism_family_strips_school_and_keeps_landmark_parent() {
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
        assert_eq!(resolved.parent_template, Some(0x4E01_E311));
        assert!(resolved.remove_bundles.contains(&0x6C04_2366));
        assert!(resolved.remove_bundles.contains(&0xD07C_5A04));
        assert!(resolved.remove_fields.contains(&0x0DA3_EE15));
        assert!(!resolved.set_float_fields.contains_key(&0x0DA3_EE15));
        assert_eq!(resolved.set_float_fields[&JOBS_W1], 30.0);
        assert!(!resolved.ensure_bundles.contains(&0xD07C_5A04));
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
