# 开发任务优先级排期（2026-09-25 定稿）

> 综合四份调研文档定优先级：[capability-census.md](capability-census.md)、
> [full-map-playable.md](full-map-playable.md)、[map-workbench.md](map-workbench.md)、
> [../design/blueprint-panel.md](../design/blueprint-panel.md)，并归并既有
> workspace-panels.md（WP0-WP4）与 linux-packaging.md。
> 排序原则：依赖链底座优先 > 用户可见价值 > de-risk spike 提前 > 单测/验收随后。

---

## 1. 结论速览

| 级 | 任务 | 估时 | 关键依赖 | 来源 |
|---|---|---|---|---|
| **P0-1** | ✅ 高度图回写链路库化（tile_arrange 逆过程 → 库 API + `map_panel_write_heightmap` + 真包回归单测）——见 §4 执行记录 | 2–3 人日 | 无（可逆性已验证） | MW-E1 |
| **P0-2** | ✅ debug 工具 overlay 一键化（enable_debug_tools 探针 → 产品命令）——见 §4 | 0.5–1 人日 | 无（路径已实证） | FM0 |
| **P0-3** | ✅ 蓝图 BP0 spike：@vue-flow/core 引入 + happy-dom 兼容验证——见 §4 | 0.5–1 人日 | 无 | BP0 |
| P1-4 | 地图缩放分级 L1（视口窗口渲染）+ L2（地块原生渲染+格网） | 2–3 人日 | P0-1（L2 读 F0） | MW §3 |
| P1-5 | 水位写回 + 画刷清单编辑器 | ~2 人日 | patch_property_overlay（已有） | E2/E4 |
| P1-6 | 灰度 PNG 导入切割（4096² → u16 场 → 金字塔） | 1–2 人日 | P0-1 | FM1/N2 |
| P1-7 | Linux 打包 CI（GitHub issue 需求，可与上并行） | 2.5–3.5 人日 | 无代码阻断 | linux-packaging |
| P2-8 | 地块表编辑器（单可选地块 + 区域外映射） | 2 人日 | P0-1 | E5/FM2 |
| P2-9 | 柏林噪声生成器 + 新区域骨架打包 | 3–5 人日 | P0-1、P2-8 | N1/N3 |
| P2-10 | 蓝图 BP1：dtype 合法性检验 + FSheet 编辑 + JSON 自动保存 | 3 人日 | P0-3 | BP1 |
| P2-11 | Property 面板实装 | 按 property-editor.md | — | WP |
| P2-12 | Asset 面板 8 槽位材质/LOD 链 | 按 workspace-panels §3.4/3.5 | — | WP |
| P3-13 | 全图模组组装 + 游戏内四项验收（出界/资源/道路/偏移） | 1–2 人日验收 | P0-1、P1-5、P2-8 | FM4 |
| P3-14 | .egb 存档解析（AEB 状态表） | 未估 | er2 深化线 | saves |
| P3-15 | AEB/ERZ 写回重序列化 | 未估 | 同上 | er2-rules |
| P3-16 | 大包（504MB 级）overlay IO 专项 | 1–2 人日 | P0-1 实测触发 | MW §7 |

## 2. 排序理由

1. **P0-1 是唯一卡脖子底座**：地图域的所有写回（改现有地图、新地图生成、全图
   模组三件套）都经过"高度场 → F0 tile → overlay 包"这一条链；`tile_arrange`
   可逆性已实证（region-and-map.md:590-592），差的只是工程化。
2. **P0-2 性价比最高**：debug 菜单 overlay 的 property 门控路径已带阳性对照
   验证（`enable_debug_tools.rs`），产品化只是把探针变成命令/UI——它同时服务
   开发期游戏内验证与最终用户实验。
3. **P0-3 spike 提前**：vue-flow 在 happy-dom 下的测试兼容性是蓝图面板唯一的
   技术不确定点（blueprint-panel.md §2.2），半天验证避免 P2 排期踩坑。
4. **P1-7 唯一的外部需求项**（GitHub issue 请求 Linux 安装包），无代码阻断、
   可与地图线并行排。
5. **P3-13 放最后**：游戏内验收依赖前面所有数据工具就位，提前做只会反复返工。

## 3. 建议节奏（两个冲刺）

- **冲刺 A（P0 全部 + P1-5）**：底座三件 + 画刷编辑——完成后工具侧具备
  "读图、改图、写图、游戏内开 debug" 的最小闭环。
- **冲刺 B（P1-4/6/7 + P2-8/9）**：缩放分级 + 灰度导入 + Linux CI + 地块表/
  噪声生成——完成后具备"新地图从零创建并打包"能力，全图模组进入组装期。

## 4. 执行记录

| 日期 | 任务 | 结果 | 提交 |
|---|---|---|---|
| 2026-09-25 | P0-2 | `sc_properties::debug_tools`（`build_debug_tools_overlay` + `patch_property_refs` 字节级补丁与自校验）；探针改为薄封装、阳性对照实验保留；`debug_tools_write_overlay` 命令（原子落盘、分类/UI 分类可参数化）；缺工具场景整体报错并覆盖单测 | af3d97f |
| 2026-09-25 | P0-1 | `sc_properties::region_write`：`read_region_field`（mip0 拼装 4096² 场）、`solve_f0_pyramid`（5 级实例排布求解，mip0 对齐全局常量 + 粗层级子采样 L1 最近匹配）、`build_heightmap_overlay`（341 tile 重建，复用源 tile 20B 头）；`map_panel_write_heightmap` 命令（16-bit 灰度 PNG → overlay，原子落盘）。合成区域整链路回归：求解排布与重建内容逐 tile 一致，44.7MB overlay 耗时 449ms（debug）；真包回归 env 门控（`OPENSCP_REGION_TERRAIN_PACKAGE`） | b5ee19e |
| 2026-09-25 | P0-3 | `@vue-flow/core` 1.48.2 + `@vue-flow/background` 1.3.2 引入（optimizeDeps 已登记）；happy-dom 下挂载 / 节点 DOM / 响应式 store 断言全过（`flow-spike.test.ts`，仅需 ResizeObserver stub）——组件级测试无需降级方案，结论已回填 blueprint-panel.md §2.2 | 6b15ecf |

**验证**：`cargo test --workspace` 31 套件全绿（0 失败）；`vue-tsc -b --noEmit` 干净；`pnpm test` 48 文件 / 193 用例全过（含 i18n-audit 与新 spike 测试）。

**下一步**（待 review 后开始）：P1-5 水位写回 + 画刷清单编辑器 → P1-4 缩放分级 L1/L2 → P1-6 灰度 PNG 导入切割 UI。
