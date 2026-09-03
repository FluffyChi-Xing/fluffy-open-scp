//! SimCity 属性列表（property list，类型 `0x00b1b104`）资源解析。
//!
//! 迁移自 C# 项目 `Simcitypak-v2` 的 `Gibbed.Spore.Properties.PropertyFile`
//! （经 `SimCityPak` 引用）及 `CliRunner.DumpProp` / combine 导出逻辑。
//!
//! 迁移范围：
//! - 属性类型：Float, Bool, Key/TGI, Vector2/3/4, Color, BoundingBox,
//!   Transform, String8/16, 各类数组
//! - hash → 可读属性名解析（对应 `TGIRegistry` / `database_main.s3db` 描述符库，
//!   后续接入注册表 crate 或直接内嵌数据）
//! - 资产聚合（`--combine`）：同 InstanceId 联合 + `Model Details`
//!   （hash `0x0975695f`）目录 prop → 模型/玩法 prop 引用联合
//! - 本地化名称解析（locale 包内名称属性 hash：
//!   0x09FB78CB / 0x0A09F5FA / 0x09B711C3 / 0x0E28B5BC / 0x0E28B5D5）
