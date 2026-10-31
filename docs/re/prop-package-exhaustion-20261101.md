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

## 验证

- `cargo check -p fluffy-open-scp --lib` 通过；
- `vue-tsc` / eslint 通过；
- 还未在用户机器上复现“包池耗尽”场景：需要连续打开多栋不同 property，
  确认不再需要重启，且失败时控制台能看到 unresolved / resolve failed。
