//! DBPF (Database Packed File) — SimCity 2013 `.package` 容器格式。
//!
//! 迁移自 C# 项目 `Simcitypak-v2` 的 `SimCityPak.Packages` 库
//! （`DataBasePackedFile.cs` / `DataBaseIndex.cs` / `DataBaseIndexData.cs`）。
//!
//! 迁移范围：
//! - DBPF 头部与索引表解析（TGI：Type / Group / Instance）
//! - RefPack（QFS）压缩数据的解压（`index.GetIndexData(true)` 对应逻辑）
//! - 资源枚举与按 TGI 查找
//! - （后续）写回 / 修改资源（`ModifiedDataBaseIndex.cs`）
