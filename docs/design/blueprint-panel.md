# 蓝图面板（Vue Flow）实现方案

> 2026-09-25 设计调研，未改任何代码。目标：在开发工作台新增"蓝图式"集成开发
> 范式，与现有菜单式专业面板**并存不互斥**（workspace-panels.md §0 的 Name-First
> 面板体系保留），让新手开发者以流程图方式完成 code mod / assets mod / 地图类
> 模组。事实底座见 [../roadmap/capability-census.md](../roadmap/capability-census.md)。

---

## 1. 结论速览

- 技术选型：**@vue-flow/core**（Vue 3 原生节点图引擎），配套 background/controls/
  minimap。已对比 Rete.js v2（§2.1），结论维持 Vue Flow。项目当前**零** node-graph
  依赖（全库 grep 零命中），需要从零引入，但"节点内容"大量复用现有资产：FSheet
  详情、RasterCanvas 绘制、PE 精细渲染、overlay 保存链。
- 最大口径分歧需先定：**"8 贴图槽位"与代码事实不符**——官方 Material Set 实际
  是 **slot0–5 共 6 槽位、9 张贴图 + 1 张参数表**（§4）。蓝图 handle 设计以代码
  事实为准（9 handle），对宣传口径再统一。
- 保存即产出：蓝图文档（JSON）随时自动保存到项目，"生成 overlay 包"仍是显式
  动作，沿用 raster 面板"编辑基于副本、保存写 overlay"的安全模型
  （`src/pages/studio/panels/raster.vue:24-26`）。

## 2. 技术选型：Rete.js v2 vs Vue Flow（2026-09 调研）

两者均为活跃维护的 TS-first 节点图库。**结论：维持 Vue Flow**——不是笼统的
"谁更灵活"，而是两者的灵活区间不同，我们的复杂度落点恰好全在 Vue Flow 的
区间内（论证见 2.1）。

| 维度 | Rete.js v2 | Vue Flow（@vue-flow/core 1.48.x） |
|---|---|---|
| 架构范式 | **框架级**：核心 NodeEditor + signal/pipe 插件系统；渲染层独立成官方插件（react/vue/svelte/angular/lit），可整体替换 | **组件级**：`<VueFlow>` 声明式组件 + `useVueFlow()` 响应式 store；节点/边就是 Vue 组件与普通数组 |
| 官方插件面 | 极全：area、connection（含 reroute/path）、**history（撤销）**、**dock（工具箱）**、minimap、context-menu、auto-arrange（elk）、comment、readonly、**engine（dataflow/control flow 执行）**、3D、LOD | 四件套：core / background / controls / minimap；其余自建或社区方案 |
| 图执行引擎 | 有（rete-engine：数据流/控制流/混合，另含 codegen） | 无（纯 UI 图） |
| 超大图性能 | 强（官方 LOD 与 LOD-GPU/Pixi 示例，数千节点） | 中（数百至低千节点无压力） |
| 嵌套分组/子图 | scopes 插件 | 原生 parentNode |
| **许可证** | 多数 MIT；**rete-structures 与 rete-scopes-plugin 为 CC-BY-NC-SA-4.0（禁止商用）** | 全家 MIT |
| Vue 3 集成深度 | 官方 vue-plugin，但节点组件须经 preset 注册、编辑器命令式装配（多一层渲染管线抽象） | 节点即插槽模板组件，`<Handle>` 即连线锚点；与 pinia/FSheet/RasterCanvas 零隔阂 |
| 上手成本 | 高（signals/pipes 心智 + preset + area/connection/render 三件套装配） | 低（React Flow 同构心智，文档/示例海量） |

### 2.1 为什么"复杂场景"反而选 Vue Flow

我们的复杂度在**节点内容**（节点内嵌 RasterCanvas 像素画布、PE 三维预览、
属性表单）与**域校验**（dtype 类型系统、必填槽位），不在图引擎的"重武器"上：

1. **Rete 的核心优势我们用不上**：rete-engine 执行引擎——我们的"执行"是 Tauri
   后端产出 overlay 包，不是前端数据流计算；LOD-GPU——蓝图规模是几十个节点；
   3D/协作场景不存在。
2. **许可证红线**：OpenSCP 是 MIT 项目，Rete 恰好最有价值的两个插件
   （structures 图算法 / scopes 分组子图）是 NC 许可，不可依赖。其 MIT 的
   history/dock 插件，我们自建成本低：history = 蓝图文档快照（§5，复用
   RasterHistory 模式）、dock = HTML5 拖放工具箱（vue-flow 官方 DnD 模式）。
3. **节点内容嵌入**：Vue Flow 下 RasterCanvas 直接写进自定义节点模板（与 raster
   面板复用同一组件）；Rete 下重型交互组件要穿过 vue-plugin 的渲染管线注册，
   调试面显著更大——这是我们与"标准节点编辑器"差异最大的地方。
4. **何时应选 Rete**：若未来把蓝图做成通用可视化编程产品（图即程序、前端执行、
   千节点级），Rete.js 的插件架构与引擎是更正确的底座；届时迁移成本主要在
   节点渲染层（节点组件本身是框架无关的设计）。

> 调研依据：Rete.js 官方文档站（retejs.org，llms.txt 插件清单 + licensing 页，
> 2018-2026 活跃维护）；Vue Flow 仓库（bcakmakoglu/vue-flow，1.48.x 持续发版，
> React Flow 同构 API）。

### 2.2 落地注意
- `package.json` 新增 `@vue-flow/core` `@vue-flow/background` `@vue-flow/controls`
  `@vue-flow/minimap`；`vite.config.ts:50-61` 的 optimizeDeps 按先例登记
  （现有 three/echarts/shiki 均登记）。
- vitest 跑在 happy-dom（`vitest.config.ts:74-79`），vue-flow 依赖 DOM 尺寸
  测量，组件测试需 mock `ResizeObserver`/`getBoundingClientRect`——先做一次
  spike 验证，失败则节点逻辑纯函数化、组件测试降级为 E2E 手测清单。
- **P0-3 spike 结论（2026-09-25，已完成）**：`@vue-flow/core` 1.48.2 +
  `@vue-flow/background` 1.3.2 已引入，optimizeDeps 已登记；happy-dom 下
  挂载、节点 DOM 渲染（`.vue-flow__node` ×2）、响应式 store 断言全部通过
  （`src/components/blueprint/flow-spike.test.ts`，仅需 ResizeObserver stub，
  容器尺寸 0 只有良性警告）——组件级测试可按原计划进行，无需降级方案。

## 3. 节点体系（三类模组模板）

蓝图面板入口挂在 `/studio` 工作台（`studio.ts` 已有 tabs 骨架，
workspace-panels.md §4"面板注册为 workspace 页面类型"）。按模组类型给三套
**起点模板**，节点均可混搭：

### 3.1 节点类型清单（首批）

| 节点 | 类别 | 输入 handle | 输出 handle | 后端能力（已具备） |
|---|---|---|---|---|
| RW4/Lot 模型 | assets | — | 9 个贴图槽 handle + model | `read_lot_model_meshes`、`LotModelPayload`（`tauri.ts:634-645`） |
| 贴图绘制 | assets | png | png | RasterCanvas 组件嵌入（sheet 内） |
| 贴图上传 | assets | — | png | `read_image_rgba`（`raster_edit.rs:113`） |
| 资产预览 | assets | model | — | PE 精细渲染 `refinedRender.ts`（`loadTintTextures:218-296`） |
| Property 覆写 | code | — | overlay 包 | `patch_property_overlay`（`package_service.rs:3961`） |
| Locale 覆写 | code | — | overlay 包 | `write_locale_overlay` |
| ER2/ERZ 规则 | code | — | 规则包 | `read_erz_preview`（编辑/写回为远期，er2-rules.md:215-224） |
| 区域高度图 | map | png | region 包 | E1 回写链（map-workbench.md §4） |
| 高度图生成 | map | — | png | N1 Perlin（同上 §5） |
| 资源画刷 | map | — | region 包 | E4 画刷清单编辑 |
| 包导出 | 公共 | overlay 包 | 文件 | `write_export_file`、atomic_fs |

### 3.2 连线类型系统（合法性检验的根基）

handle 携带 **dtype**：`png` / `params`(f32 表) / `model` / `overlay` / `region`。
连线时 dtype 不匹配直接拒绝（vue-flow `isValidConnection`）；同 dtype 再做
子校验（如 png→贴图槽位时校验尺寸/通道，参照 raster 设计文档的 LotSurface
约束）。合法性检验器实现为纯函数（输入 nodes/edges，输出错误列表），便于
vitest 单测与未来"蓝图静态检查"复用。检验项：

1. dtype 匹配与子校验（§上）；
2. DAG 无环（vue-flow 有环检测可关，自己留错误清单而非直接拒绝，允许"先搭后修"）;
3. 必填槽位：模型节点 9 个贴图 handle 未全部连通时预览节点降级为白模
   （复用 `exportLotModel mode:"white"` 语义，`Rw4Preview.vue:21-34`）；
4. 一个蓝图至多一个"包导出"汇聚节点；overlay 目标包一致性告警。

## 4. "8 贴图槽位 → 8 handle"的口径校准与映射

代码事实（`src/api/tauri.ts:595-623`，官方 Material Set 通道拆分，§27 源码
实证）：

| slot | 内容 | handle dtype |
|---|---|---|
| slot0 | 参数表 f32（palU/interiorScale/regionXform/tilePadding…） | params |
| slot1 | 漫反射 baseColor + color control（tint） | png ×2 |
| slot2 | 法线（A=spec）+ AO（alpha） | png ×2 |
| slot3 | shader map（B=spec，A=窗洞）+ roughness | png ×2 |
| slot4 | 256×8 tint palette | png ×1 |
| slot5 | interior 房间图集（alpha=逐窗灯）+ relief | png ×2 |

即 **6 槽位 / 9 张贴图 / 1 张参数表**；容器 v8 每材质下发"9 张 PNG + 参数表"
（`tauri.ts:624-632`），PE 已跑通三.js 多贴图渲染链。workspace-panels.md §3.4
的"8 槽位"是规划口径，落地前统一为：
**模型节点 = 1 个 model 出口 + 9 个 png handle + 1 个 params handle**
（共 10 个右侧 handle，面板上按 slot 分组着色）。若产品仍需"8"的说法，可把
slot2+slot3 的四张灰度/辅助图折叠为"材质增强组"子面板，避免虚构槽位。

## 5. 交互与编辑

- **节点详情 sheet**：双击/右键节点 → FSheet（`src/components/ui/FSheet.vue`，
  仅 25 行 Teleport 抽屉）承载完整编辑器——绘制类节点内嵌 RasterCanvas，
  属性类节点内嵌表单。蓝图画布保持轻，重编辑进 sheet（对齐 raster 面板
  "左画布右来源"的双栏经验）。
- **添加节点**：左侧工具箱按三类模板分组，HTML5 拖放拖入画布（当前全库无
  dragstart/@drop，需新做；vue-flow 官方 DnD helper 可用）。
- **随时保存**：蓝图 = JSON 文档（nodes/edges/位置/参数），pinia store
  （参照 `uiWorkbench.ts` 的 edits/added 模式）+ debounce 自动保存到
  ModProject 目录（`mod_project_*` 命令已有）；**"生成 overlay 包"仍是显式
  按钮**——自动保存只落蓝图文档，绝不自动写游戏目录（安全模型同 raster）。
- **撤销**：推广 `RasterHistory` 区域快照模式（`src/lib/raster-editor/history.ts:18-65`）
  到蓝图文档级快照。

## 6. i18n 与质量

- 新命名空间 `studio.blueprint.*`：zh/en 双语块镜像维护（`src/locales/index.ts`
  单文件结构），提交前 `pnpm test` 的 i18n-audit 强制把关——**所有节点标题/
  错误文案必须过 audit**，这是本仓库既有红线。
- 单测：合法性检验器（纯函数）+ 蓝图文档序列化往返 + store 保存/恢复；
  组件层 vue-flow 渲染测试视 §2 spike 结果决定深度。
- `vue-tsc -b --noEmit` 与 eslint 全绿为合入门槛。

## 7. 里程碑

| 阶段 | 内容 | 验收 |
|---|---|---|
| BP0 | 依赖引入 + 画布骨架 + 资产模板最小闭环（模型节点→贴图上传→预览） | 连线连通后预览节点显示 GLB 白模/贴图 |
| BP1 | dtype 合法性检验 + FSheet 详情编辑 + JSON 自动保存/撤销 | 断线/环/缺槽位三场景告警正确 |
| BP2 | code mod 模板（Property/Locale 覆写节点）+ 包导出节点 | 蓝图产出可用 overlay 包并被游戏加载 |
| BP3 | map 模板（高度图生成/回写/画刷节点，依赖 map-workbench E1/N1）+ 工具箱拖放/小地图/快捷键 | 地图类蓝图走通全流程 |

## 8. 风险与开放问题

1. **vue-flow bundle 体积**：core + 生态约 +100KB gzip 级；预打包登记后评估
   首屏影响，必要时动态 import（蓝图面板独立 chunk）。
2. **happy-dom 测试兼容**（§2 spike）。
3. **PE 与蓝图的渲染重复建设**：资产预览节点复用 `refinedRender`/
   `useEditorViewport`（`registerTextureUrl` 贴图 URL 回收已有），不另起炉灶。
4. **8 槽位口径**（§4）需产品拍板统一。
5. **ER2 写回缺位**：规则节点首版只能"读 + 生成模板"，完整编辑等 AEB
   重序列化器（er2-rules.md:215-224）。
