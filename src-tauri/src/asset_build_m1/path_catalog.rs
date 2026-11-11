//! Path 类型目录 + 处理器（2026-11-07）。
//!
//! 作者只选"类型"（人行道/车辆引道），entry/scope/roadConnect 由这里
//! 换算——证据基线见 `docs/design/path-semantic-authoring.md`：
//!
//! - 引擎按 **instance** 解析 path entry（`CreatePathsFromProps` 只把
//!   `mInstance` 交给 `IndexFromPathEntryID`，group 只是名义值）；
//! - 原版已经存在的 entry 直接引用，零新资源、零索引预算风险
//!   （C1/C2 的全部风险都来自"新建 entry"，本版不新建）；
//! - 语义必须显式：pedestrian → Pedestrians；`driveway` → 校车引道
//!   （Global+First，跨城接路网）；`vehicle` → 普通车辆引道（Local，
//!   同 entry `7C9C4FFF` / filter `C37D661D` 通用车辆类，拆除随 unit 消失）。
//!
//! 纯函数模块：不读文件、不碰包，输入 PathSpec → 输出 PathRecipe，
//! 可直接单测。构建器写回 lot 列与校验器共用同一份配方。

use super::native_profile::path_entry_group;

/// 一个可绘制的路径类型 = 一条原版 entry 的语义别名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathTypeSpec {
    /// 稳定类型 id（文档/前端/package.json 共用）。
    pub id: &'static str,
    /// 引用的原版 PathEntry instance（B1B104 资源组）。
    pub entry: u32,
    /// 引擎 scope：行人/普通车辆 Local，校车引道 Global。
    pub scope_local: bool,
    /// 道路连接标记：校车引道 First；行人/普通车辆 None。
    pub road_connect: bool,
    /// 允许通行主体（诊断/清单展示；filter 名来自注册表实测）。
    pub transports: &'static [&'static str],
}

/// 首版类型表：只列已实证的原版 entry（"Reference-first"，不新建资源）。
pub const PATH_TYPES: &[PathTypeSpec] = &[
    PathTypeSpec {
        id: "pedestrian",
        // 威利斯/小学/大学实证：Pedestrians filter（0x0D6498E8 → E0CD3122）。
        entry: 0x418A_18B1,
        scope_local: true,
        road_connect: false,
        transports: &["Pedestrians"],
    },
    PathTypeSpec {
        id: "vehicle",
        // 普通车辆引道：与校车共用原版唯一车辆 entry（沥青挤出 + filter
        // C37D661D——注册表名 School busses，但大量道路 PathEntry 同属此
        // filter，普查定案为通用车辆类）。Local+None → 只进本 unit pathSet，
        // 拆除随建筑消失（Hope Tower Global 残段对照）。
        entry: 0x7C9C_4FFF,
        scope_local: true,
        road_connect: false,
        transports: &["Vehicles"],
    },
    PathTypeSpec {
        id: "driveway",
        // 校车引道：同 entry，但 Global+First 接全城/区域路网（小学实证，
        // 可跨城）。非学校建筑请用 `vehicle`，避免拆除残留路网 stub。
        entry: 0x7C9C_4FFF,
        scope_local: false,
        road_connect: true,
        transports: &["School busses"],
    },
];

/// 路径类型 → 引擎配方（写回 lot 七列的唯一换算出口）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathRecipe {
    pub type_id: &'static str,
    pub entry_instance: u32,
    pub entry_group: u32,
    pub scope_local: bool,
    pub road_connect: bool,
    /// 类型来自目录（`true`）还是高级模式的显式 entry（`false`）。
    pub from_catalog: bool,
}

/// 解析一条路径的类型/entry 覆盖。
///
/// - `kind` 命中目录 → 全部语义来自目录（作者不写 scope/connect）；
/// - `kind` 缺失 + `entry` 显式给出 → 高级模式（保留作者的 scope/connect）；
/// - 两者都缺失 → 错误（引擎会整条跳过，必须在构建期拒绝）。
pub fn resolve(
    kind: Option<&str>,
    entry: Option<&str>,
    scope_local: bool,
    road_connect: bool,
) -> Result<PathRecipe, String> {
    if let Some(kind) = kind.map(str::trim).filter(|kind| !kind.is_empty()) {
        if let Some(spec) = PATH_TYPES.iter().find(|spec| spec.id == kind) {
            return Ok(PathRecipe {
                type_id: spec.id,
                entry_instance: spec.entry,
                entry_group: path_entry_group(spec.entry),
                scope_local: spec.scope_local,
                road_connect: spec.road_connect,
                from_catalog: true,
            });
        }
        // 高级模式：type = "custom" + entry 显式给出。
        if kind != "custom" {
            return Err(format!(
                "unknown path type '{kind}'; expected one of: {}",
                PATH_TYPES
                    .iter()
                    .map(|spec| spec.id)
                    .chain(["custom"])
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    let Some(entry) = entry.map(str::trim).filter(|entry| !entry.is_empty()) else {
        return Err(
            "path has no type — choose one of the catalog types or set an explicit entry".into(),
        );
    };
    let instance = u32::from_str_radix(entry.trim_start_matches("0x"), 16)
        .map_err(|error| format!("path entry '{entry}': {error}"))?;
    Ok(PathRecipe {
        type_id: "custom",
        entry_instance: instance,
        entry_group: path_entry_group(instance),
        scope_local,
        road_connect,
        from_catalog: false,
    })
}

/// 类型目录条目（前端/package.json 同步的只读投影）。
pub fn catalog() -> Vec<serde_json::Value> {
    PATH_TYPES
        .iter()
        .map(|spec| {
            serde_json::json!({
                "type": spec.id,
                "entry": format!("{:08X}", spec.entry),
                "group": format!("{:08X}", path_entry_group(spec.entry)),
                "scope": if spec.scope_local { "local" } else { "global" },
                "roadConnect": spec.road_connect,
                "transports": spec.transports,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pedestrian_resolves_to_vanilla_entry() {
        let recipe = resolve(Some("pedestrian"), None, false, true).unwrap();
        assert_eq!(recipe.entry_instance, 0x418A_18B1);
        assert_eq!(recipe.entry_group, 0x40E0_C400);
        assert!(recipe.scope_local, "pedestrian is Local");
        assert!(!recipe.road_connect, "pedestrian is None");
        assert!(recipe.from_catalog);
    }

    #[test]
    fn catalog_semantics_override_document_flags() {
        // 文档里残留的旧 scope/connect 不得覆盖类型配方（语义由处理器决定）。
        let recipe = resolve(Some("driveway"), None, true, false).unwrap();
        assert_eq!(recipe.entry_instance, 0x7C9C_4FFF);
        assert!(!recipe.scope_local, "driveway is Global");
        assert!(recipe.road_connect, "driveway is First");
    }

    #[test]
    fn vehicle_is_local_lot_driveway() {
        // 普通车辆：同 entry，但 Local+None——作者即使写 global/true 也被覆盖。
        let recipe = resolve(Some("vehicle"), None, false, true).unwrap();
        assert_eq!(recipe.entry_instance, 0x7C9C_4FFF);
        assert!(recipe.scope_local, "vehicle is Local");
        assert!(!recipe.road_connect, "vehicle is None");
        assert!(recipe.from_catalog);
    }

    #[test]
    fn custom_entry_keeps_author_flags() {
        let recipe = resolve(Some("custom"), Some("675B61B1"), true, false).unwrap();
        assert_eq!(recipe.entry_instance, 0x675B_61B1);
        // 大学门 entry 住 40E1C400——按 instance 解析，group 由 handler 修正。
        assert_eq!(recipe.entry_group, 0x40E1_C400);
        assert!(recipe.scope_local);
        assert!(!recipe.from_catalog);
    }

    #[test]
    fn missing_type_and_entry_is_rejected() {
        let error = resolve(None, None, true, false).unwrap_err();
        assert!(error.contains("no type"), "{error}");
    }

    #[test]
    fn unknown_type_names_the_alternatives() {
        let error = resolve(Some("sidewalk"), Some("418A18B1"), true, false).unwrap_err();
        assert!(error.contains("pedestrian"), "{error}");
    }
}
