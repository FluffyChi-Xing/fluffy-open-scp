# Prop 解析失败后全局失效：包池耗尽（2026-11-01）

## 现象

- 一栋建筑的 prop 解析失败后，后续打开任何 property 的 prop 都只出标记锥；
- `[prop-models] unresolved resource ids` 不打印；
- 卸载/重开包也无法恢复，只能重启软件。

## 根因

`resolve_all` 会为 prop 解析自动注册 `SimCityUserData/EcoGame` 下的包，并把
id 记进进程级 `REGISTERED_ECOGAME`。`PackageManager` 有 32 包上限，
`ensure_ecogame_registered` 在预算不足时**静默返回空/部分包**：

```rust
let budget = MAX_OPEN_PACKAGES - (open_count + 2);
if registered.len() >= budget { return Ok(registered); }
```

一旦之前的 property 把包池推到接近 32：

- 后续 `resolve_all` 注册不到 EcoGame 包，`resolve_one` 找不到记录；
- 后端返回空 resolutions，前端没有 unresolved 记录（旧日志只遍历已有
  resolutions），所以“什么也不打印”；
- 自动注册包不在普通包 UI 里，手动卸载普通包清不掉它们；只有重启进程
  清空 `REGISTERED_ECOGAME` 和 PackageManager 才恢复。

## 修复

1. 前端 `load()` 打开新 property 前调用 `releasePropModelPackages()`，先释放
   上一栋自动注册的 EcoGame 包；
2. 后端 `resolve_all` 在包池接近上限时再兜底 `release_registered()`，避免
   残留包继续占位；
3. 前端换 property 时清空 `propModels` / `propTreeIds` / `treeModelPayloads`
   / `treeAtlasPng`，避免跨包 resourceId 碰撞和旧树 id 误判；
4. 日志修正：先按请求 id 计算 `missingIds`，即使后端返回**空 resolutions**
   也会打印 unresolved；`resolvePropModels` 整体抛错时 DEV 打印
   `[prop-models] resolve failed`。

## 更深一层：26 个 EcoGame 家族本来就接近旧上限

当前安装的 EcoGame 目录有 82 个 `.package`，按家族取最新后是 **26 个**。
旧上限 32、且要为手动包预留 2 个，一旦用户已经开了几个包，预算就会低于
26，晚注册的 DLC 脚本包缺失——表现为“以前能渲染的某些 prop bin 又出不来”，
而不是整批失败。

因此把 `MAX_OPEN_PACKAGES` 提到 **64**，并只在**关闭 property**时释放自动包
（不再每次 load 后释放，避免反复开关 26 个包和读取竞态）。打开普通包只在
真正命中包上限时重试一次。

## 另一个可观测症状

prop 解析失败后再打开普通包，UI 只显示 “Package 无法解析”。真实原因通常是
`PackageManager` 已满（`PackageLimitExceeded`），但 `gamePackages.openFile`
把所有失败都映射成同一句 toast。打开包前也会先释放自动注册的 EcoGame 包，
并保留 `console.warn("[packages] open failed:", error)` 便于确认。

## 验证

- `cargo check -p fluffy-open-scp --lib` 通过；
- `vue-tsc` / eslint 通过；
- 还未在用户机器上复现“包池耗尽”场景：需要连续打开多栋不同 property，
  确认不再需要重启，且失败时控制台能看到 unresolved / resolve failed。
