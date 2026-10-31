# R29 最终对拍报告

## 实机结果

R29 已进入游戏测试，但进入存档阶段仍然崩溃。测试包已从活动目录移除并归档：

`D:\ea-games\simcity_offline\test-archive\000_r29_commercial_lotm.package`

本轮之后不再继续生成测试包。

## 已排除的问题

R29 的主模型经过离线 `nighthouse_dump` 检查：

- 1 个 Mesh；
- 1 个 Material；
- 1 个 CollisionMesh；
- 1 个 BBOX；
- 109587 vertices / 52157 triangles；
- 没有 `decode error`；
- VertexArray、TriangleArray、Mesh 的 section 引用已经重映射到模板编号。

因此 R14/R23 的 105 mesh 孤儿形状、R24 的 CollisionMesh 缺失、R26/R27 的 section 编号错误均已排除。

## Property 对拍

R27 与已验证 R11 的第一处明显分叉：

| 字段 | R27 | R11 |
|---|---:|---:|
| Menu parent | `AF042E9A` | `F970A822` |
| Unit template | `4E01E311` | `565B72B9` |
| Lot template | `4E01E311` | `34CE7E14` |
| Lot Property | 79 字段 / 1391B | 33 字段 / 669B |

R29 已改用 R11 的 Menu parent、Unit template、Lot template，并把 Lot Property 收敛到 30 个当前源包存在的安全字段。R29 的模型 Property 已将 LOD1–LOD4 指向新模型 TGI。

## 尚未解决的差异

R29 仍然没有达到 R11 的完整资源闭包：

- R29：101 entries、75 个 Raster、20 个 RW4；
- R11：105 entries，包含 R11 模板自身的完整配套资源；
- 构建报告仍为 `unit_count=0`、`lowered_units=0`；
- 当前 Lot Property 仍是从 Dalian 参考 Lot 清理而来，不是从已验证的 R11 原生 Lot 叶子逐字段复制；
- R29 使用 reference package 中自动选择的 Dalian RW4 作为骨架，而 R11 使用 NightHouse 原生模板及其完整资源闭包。

这说明当前崩溃点已经从几何格式转移到“原生模板资源闭包 + Lot/Unit/LOD 继承链”的组合差异。仅凭 R29 的离线 RW4 解析不能证明游戏运行时闭包完整。

## 本轮代码整理

已保留的后端改动：

- `src-tauri/src/asset_build_m1/native_lot.rs`：Blender 插件 Lot Property 编码器；
- `src-tauri/src/asset_build_m1/native_model.rs`：LOTM 安全形状、CollisionMesh 模板、RW4 骨架替换和 section 引用重映射；
- `src-tauri/src/asset_build_m1/menu.rs`：R11 菜单模板默认值和 Lot Property 安全字段清理；
- `src-tauri/src/asset_build_m1/package.rs`：自动选择原生 RW4 模板及超过 4 mesh 的保护；
- `crates/rw4/src/writer.rs`：`replace_template_sections`；
- `src-tauri/src/asset_build_m1/nh_collision.bin`：原生 CollisionMesh 模板；
- `docs/re/blender-rust-backend-migration-20261031.md`：Blender 到 Rust 迁移说明。

已删除临时对拍示例 `compare_packages.rs`。R29 schema 和离线产物保留在 `tmp/commercial-r23` 作为失败复现材料；R29 游戏包只保留在 `test-archive`。

## 结论

R29 仍失败，不能宣称 OpenSCP 的 LOTM→RW4→Property 构建链已经达到 Blender 插件的游戏闭环。下一阶段应停止继续试包，直接以 R11/Blender 的完整原生 package 作为基线，做资源级闭包迁移：复制完整模板资源组，再替换模型和纹理引用，最后才重新接入 OpenSCP 的资源编排和绑定能力。
