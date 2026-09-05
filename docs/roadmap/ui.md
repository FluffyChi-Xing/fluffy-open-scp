# OpenSCP 前端 Roadmap — 模板壳基线 → P0/P1 信息架构 → 实施计划

> 上游文档：`docs/roadmap/migration.md`（crate 迁移主线）、`docs/roadmap/modding-suite.md`（P1 Modding Suite 契约）。
> 本文档负责 UI 侧：当前前端真实基线、新导航信息架构、页面与后端能力映射、Vue Flow 边界、异常隔离约束和 UI 里程碑。
> 核心原则：**fluffy-design-pro 应用壳基本不改，业务开发集中在 `src/pages/`，经 Tauri 封装为 Windows EXE。**
> **MVP 交付定义：最小可交付产物 = 前后端 P0 阶段打通后的可运行 EXE**（即原 SCP 核心能力的 Rust + Tauri 迁移版：打开/浏览 package、查看资源、属性表与资产目录，异常不崩溃）。对应本文件 U0–U2 完成 + `pnpm tauri build` 出包；U3 起属于 P1 增强，不在 MVP 范围内。

---

## 1. 当前前端真实基线（盘点，2026-09-04）

### 1.1 应用壳（保持不变）

fluffy-design-pro 已提供完整壳能力，全部保留：

| 组件 | 位置 | 职责 |
|---|---|---|
| `DefaultLayout.vue` | `src/layouts/DefaultLayout.vue` | 壳骨架：Navbar + Sidebar + TabBar + RouterView + 设置面板 + 移动端抽屉 |
| `SidebarNav.vue` | `src/components/layout/` | 渲染 `NavigationGroup[]`，纯展示组件 |
| `TabBar.vue` | `src/components/layout/` | 页面级多页签（对应原版右栏多资源页签） |
| `CommandPalette.vue` | `src/components/navigation/` | Ctrl+K 快速跳转，消费 `leafNavigationItems` |
| 主题/语言/色弱 | `stores/app.ts` | 已实现，不改 |

**边界**：`DefaultLayout` 与 `SidebarNav` 不引入任何 OpenSCP 业务状态（当前 package、资产图、导出任务、workflow 状态都不进壳）；导航变化只通过 route metadata + registry 完成。

### 1.2 当前导航（需重组）

`src/router/registry.ts` 用字符串分组自动生成菜单，当前 `groupOrder`：

```text
navigation.workspace  → 概览/项目/部署（dashboard.ts，模板 mock）
navigation.showcase   → 组件/图表/表格/表单等 9 个演示页（showcase.ts）
navigation.manage     → 设置（management.ts）
navigation.resources  → Vue 文档/嵌入示例（external.ts，外部 iframe）
```

问题：这是「项目/部署控制台」模板语义，与 OpenSCP 的 package/resource/asset 领域不匹配；showcase 占据生产导航；`groupKey` 是自由字符串，无集中常量。

### 1.3 当前页面（正在替换）

页面已按 `src/pages/<group>/index.vue` 组织，`src/pages` 根目录不再保留扁平页面或转发 wrapper。概览已改为 OpenSCP 资源地图，Package 页正在收敛为 JetBrains IDE 式目录树/Package 列表/多 Tab 工作区；文档工作区和设置页已具备基础接入。showcase 页面保留为组件参考但退出生产导航，登录入口已从桌面路由移除。

### 1.4 后端真实进度（决定 P0 菜单边界）

| 能力 | 状态 | 依据 |
|---|---|---|
| DBPF/DBBF 头+索引+RefPack+mmap 读取 | ✅ 已完成 | `crates/dbpf`，Tauri `open_package/list_resources/read_resource_bytes` |
| 属性表解析/combine/locale | ✅ crate 已完成，❌ Tauri command 未接入 | `crates/sc-properties`；后续补 `parse_properties/combine_assets` IPC |
| s3db 描述符库 | ✅ 已完成，❌ 前端真实接入未完成 | `crates/sc-registry`；Tauri `resolve_name` 已可调用 |
| RW4 解析/模型导出 | ✅ 已完成 | `crates/rw4` + `crates/sc-exporter`；OBJ/GLB/纹理导出 command |
| 音视频工具运行时探测 | ✅ 已完成 | `detect_media_tools`；vgmstream bundled 路径、ffmpeg bundled/PATH 回退 |
| 文档工作区 | ✅ 后端已完成 | M5.5 workspace commands、Markdown revision 和安全文件写入 |
| 游戏目录设置/探测 | ✅ 后端已完成 | M6 `settings_get/settings_set_game_directory/game_directory_detect` |
| **Tauri command 层** | ✅ M5/M5.5/M6 后端基础已完成 | `src-tauri/src/lib.rs` |

**关键结论：crate 已实现 ≠ 前端端到端已完成。** 当前前端先通过 `localStorage['openscp:local-key']` 选择版本化 Mock data 完成静态页面和交互，再切换 Tauri adapter 对接真实命令；浏览器不得伪造 Tauri 成功。
---

## 2. 新导航信息架构

### 2.1 设计原则

1. **壳不动**：layout/Sidebar/TabBar/主题/CommandPalette 原样保留；所有新能力 = 新 route module + 新 `src/pages/` 页面。
2. **导航按后端能力组织**：P0 菜单只放已实现（或即将实现）的 dbpf/sc-properties/sc-registry 能力；RW4/导出/工作流明确标 P1，不提前亮出灰入口。
3. **新增「模组/资产制作」一级 group**：区分「资源浏览 = 理解已有 package」与「模组/资产制作 = 组织/编辑/构建资产」，后者是 P1 Modding Suite 的导航落点。
4. **详情路由不进菜单**：资源详情、资产详情用 `hideInMenu: true` + `activeMenu` 指向父级菜单，经 TabBar 开页签。
5. **showcase 退出生产导航**：保留文件，route 加 `hideInMenu` 或仅 debug 构建注册；`LoginPage` 与 permission 守卫在桌面场景移除（`appConfig.permission` 置空）。
6. **后端失败不能升级为应用失败**：见 §5 异常隔离契约。

### 2.2 目标菜单树

```text
概览                                           ← 顶层第一项，无 groupKey，P0

工作区 navigation.workspace
   └── 文档工作区                         P0  Markdown/README 文件夹工作区

资源浏览 navigation.resources
   └── 游戏包                             P0  打开 package、分页资源和详情工作台

管理 navigation.manage
   └── 设置                               P0  游戏目录、文档工作区和工具状态

── 后续阶段，不提前注册生产菜单 ──
模组/资产制作 navigation.modding              P1  资产目录、模组项目、制作工作台、依赖工作流
预览与导出 navigation.export                  P1  导出中心、模型预览、媒体预览
showcase / external                         保留参考页面，hideInMenu
LoginPage                                   桌面场景不作为业务入口
```

概览通过 `topLevelNavigationItems` 独立渲染，按 `order: 0` 置于所有分组之前；分组顺序保留为 `workspace → resources → modding → export → manage`。当前只有具备真实页面和后端基础的组进入生产菜单。

### 2.3 路由模块重组

`src/router/routes/modules/` 按业务域重划（壳的 `import.meta.glob` 机制不变）：

| 文件 | 内容 | 状态 |
|---|---|---|
| `workspace.ts` | 概览（`/`）+ 文档工作区 | ✅ 已注册 |
| `packages.ts` | Package 库 + 浏览器 + 资源详情（隐藏） | 规划中，当前合并于 `resources.ts` |
| `resources.ts` | 源文件解析：目录树、Package 列表、资源工作区与分类 Tabs | ✅ Mock 工作流已接入；真实目录扫描 command 待补 |
| `assets.ts` | 资产目录 + 资产详情（隐藏） | P0 待 `combine_assets` command |
| `modding.ts` | 模组项目/工作台/工作流 | P1，暂不注册 |
| `export.ts` | 导出中心/模型/媒体预览 | P1，暂不注册 |
| `management.ts` | 设置 | ✅ 已注册并接入 M6 settings |
| `showcase.ts` / `external.ts` | 演示与外部页 | ✅ 保留，`hideInMenu` |

P1 重型页面（工作台、Vue Flow、Three.js、导出中心）必须 lazy-load（`defineAsyncComponent` 或动态 import），不进首屏 bundle。

### 2.4 资源工作台页布局（对应原版主窗口，落在单个页面内）

```text
┌────────────┬───────────────────────┬───────────────────────────┐
│ 包列表      │ 资源列表（虚拟表格）   │ 资源详情（页内子页签）     │
│ (已打开的   │  TGI/名称/大小/压缩   │  hex｜文本｜贴图｜属性｜   │
│  .package)  │  排序·筛选·全文过滤   │  模型｜音视频              │
└────────────┴───────────────────────┴───────────────────────────┘
```

- 三栏是**页面内部布局**，可折叠；不与壳全局 Sidebar 冲突。
- 资源详情优先做成页内右栏子页签；深度查看（如全屏模型）再走隐藏路由 + TabBar。
- 原版「Open in new window」→ 壳 TabBar 新页签；右键动作 1:1 保留（复制 T-G-I、Filter by、导出、跳属性搜索）。

### 2.4 源文件解析工作台布局（JetBrains 风格，单页面内）

```text
┌──────────────┬────────────────────┬──────────────────────────┐
│ 游戏文件树    │ 当前目录 package    │ 已打开 package 工作区      │
│ 选择目录后展开 │ 文件列表             │ 多 Tab + 分类 Tabs + 详情  │
└──────────────┴────────────────────┴──────────────────────────┘
```

- 首次进入不自动扫描或打开文件：三栏分别显示 `FEmpty`；顶部导入下拉提供选择文件夹/默认目录。
- 游戏包区域限定最大高度，目录树、package 列表和资源工作区在内部滚动。
- 选择 package 后，每个 package 进入独立 Tab，Tab 必须支持切换和只关闭当前项。
- 分类使用横向 Tabs（如 `全部 (1000)`、`RW4 (200)`），不再使用多个分类下拉框；资源详情优先使用页内子 Tabs。

---

### 2.5 前端 Mock → Tauri 联调顺序

浏览器预览使用 `localStorage['openscp:local-key']` 作为版本化本地演示开关，例如 `{ "version": 1, "mode": "mock" }`。它不是认证凭据，也不进入后端或生产配置。

- Phase A：Mock adapter 填充概览、package/资源分页、详情 tabs（hex/text/property/image/audio/video/model）、文档工作区、设置和所有空/加载/错误状态。
- Phase B：使用 5174 做浏览器 E2E 和响应式验证；5173 为长期预览端口，不得操作；测试完成必须释放 5174。
- Phase C：Tauri adapter 对接真实命令，真实功能测试根路径为 `D:\\ea-games\\SimCity`，打开时仍要求具体 `.package` 文件，不硬编码生产路径。
- 页面只依赖统一 `PackageDataSource`，不得在浏览器伪造 Tauri 成功，也不得直接解析二进制或启动 ffmpeg/vgmstream。

### 2.6 统一格式预览

详情 tabs 共享 preview surface、格式徽标、TGI、状态和工具栏。文本/属性/hex 受后端分页限制，图片使用 contain，音频/视频使用原生控件且不自动播放；Three.js 模型预览 lazy-load，支持左键旋转、滚轮缩放、右键平移/角度调整、光照方位/高度和 reset/wireframe，并在卸载时释放 renderer、geometry、material、texture、controls 和 object URL。完整多 Mesh/多材质跨 package GLB、真实媒体转码和批量导出按 P1/后端能力逐步接入。

---

## 3. 页面与后端能力映射（P0）

每个 P0 页面的验收 = 下表链路全通 + §5 异常隔离用例通过：

| 页面 | Tauri command / Mock data source | Rust crate | DTO 要点 | 当前状态 |
|---|---|---|---|---|
| 概览 | Mock summary；后续补 `workspace_summary` 或 activity 聚合 | sc-store/dbpf | 包数/资源数/资产数/失败任务 | 前端待重写，当前仍为 CLI mock |
| Package 库/资源工作台 | `open_package` / `list_resources`；浏览器先走 Mock adapter | dbpf::Package | 路径/大小/kind/index count/TGI/压缩 | 基础页面已接入，单页 tabs 待完善 |
| 资源详情 | `read_resource_bytes(range)`；Mock 返回稳定 bytes | Package::read | hex 4KB/文本/格式状态 | 基础 hex 已接入，统一 preview tabs 待开发 |
| 属性表 | 后端待补 `parse_properties`；Mock 先填静态 DTO | sc-properties + sc-registry | hash/类型/kind/值/名称 | P0 页面待开发，不伪造真实 IPC |
| 资产目录 | 后端待补 `combine_assets`；Mock 先填组件关系 | sc-properties + locale | 组成员/localized name | P0 页面待开发，不提前注册菜单 |
| 文档工作区 | `workspace_*` commands；浏览器展示空态/Mock 状态 | sc-store/文件服务 | folder/readme 相对路径/revision | 后端已完成，前端基础页已接入 |
| 游戏目录设置 | `settings_*` / `game_directory_detect` | sc-store/settings | path/marker/accessibility | 后端已完成，前端基础页已接入 |

前端不直接解析任何二进制格式。当前先使用 local-key Mock 完成静态流程，再切换 Tauri adapter；`parse_properties`、`combine_assets`、`workspace_summary` 等未存在 command 不得在前端假装调用成功。

---

## 4. Vue Flow 引入评估（P1）

### 4.1 结论

**引入，但仅作为 P1 工作流页面的可视化层**：`DependencyWorkflow` 页用 Vue Flow 表达资产依赖子图与构建流水线。它不进入 P0，不进入全局导航结构，不进入 `DefaultLayout`。

### 4.2 适用与不适用

| 适用 ✅ | 不适用 ❌ |
|---|---|
| 资产依赖图：Property → Model Details → RW4 → material → texture | Package 全量 TGI 树（这是树/虚拟表格问题，几千节点画图是噪声） |
| 构建流水线：manifest → resolve → import OBJ → patch → validate → overlay 输出 | 全量 property graph（节点爆炸） |
| Key / Model Details 引用关系展示（`Property::keys()` 已提供数据源） | 3D 几何编辑（归 Three.js） |
| 失败定位：点击节点查看对应 diagnostic | 工作流执行引擎（执行/持久化/循环检测/取消都在 Rust 后端） |

### 4.3 硬性边界

- 节点 ID 用稳定 TGI 或 workflow step ID，不用数组索引；边标注来源（Key / Model Details / pipeline stage）。
- 默认只渲染筛选/展开后的子图（当前资产 + 直接依赖），大图 lazy expansion。
- **必须提供列表/树 fallback 视图**（无障碍 + 大图降级），图与列表双向选中同步。
- 后端负责 schema 校验、cycle detection、执行、进度事件（`modding://build-progress` 等）；Vue Flow 只做节点/边交互与布局。
- 依赖按 P1 第二阶段安装（`@vue-flow/core` + 必要 nodes/controls 包），与 Three.js 一样 lazy-load。

---

## 5. 异常隔离契约（P0/P1 强制）

原版 SCP「读取/编辑异常 → 整个应用崩溃」是新版禁止回归项。发布形态为 Tauri Windows EXE，任何文件异常都要保证前后端继续运行、页面给出提示。

### 5.1 统一响应

```ts
export interface CommandResult<T> {
  ok: boolean
  value?: T
  diagnostics: Diagnostic[]
}
```

### 5.2 分层责任

- **Rust crate**：精细 error enum（已达标：dbpf/sc-properties 均为 thiserror），边界检查输入，批量任务单项失败不中止整体。
- **Tauri command**：禁止 `unwrap`/`expect`/panic 逃逸；底层错误转为 `CommandResult`；批量任务返回成功项+失败项。重活用 `spawn_blocking`，防 UI 冻结。
- **前端页面**：inline error / toast / 诊断面板展示诊断码、文件/TGI/section、修复建议；提供重试、跳过、返回列表。单资源失败不影响其它资源、已开页签和后台任务。
- **进程兜底**：解析/导出任务隔离在后台线程；兜底记录并通知前端，不传播为应用退出。

### 5.3 写文件安全

优先写临时文件 → 校验 → 替换目标；失败不破坏原文件。

### 5.4 强制验收用例（每个 P0/P1 里程碑跑一遍）

损坏 package / 截断 RW4 / 缺失或非法 OBJ / 无权限输出目录 / 外部工具缺失 / 批量中单个资源失败 / 取消任务。每项通过标准：**稳定诊断码 + 修复建议，页面可操作，Tauri 进程存活，其它页签与任务不受影响。**

---

## 6. UI 里程碑

> **MVP = U0 + U1 + U2 全部验收通过，且 `pnpm tauri build` 产出可运行的 Windows EXE。** U3–U5 为 P1 增强，不阻塞 MVP 交付。

### U0 — 导航重组 + Tauri command 地基（前置）
- [x] route module 重组与生产菜单清理，概览为无分组首项
- [x] `navigation.*` i18n 词条（zh-CN/en-US）与 showcase/external 隐藏
- [x] M5/M5.5/M6 后端 command 基础、typed adapter 与 workspace/settings 页面首版
- [ ] `CommandResult<T>` + Diagnostic wire 类型完整进入 `src/api/contracts.ts`
- [ ] 真实 Tauri runtime 打开 `D:\\ea-games\\SimCity` 下具体 package 并列出 TGI
- 验收：Mock 页面流程已通；真实 Tauri 联调待后续 Phase C

### U1 — `/pages/` 资源工作台（P0 DBPF）
- [ ] 重写 OpenSCP 概览：包数/资源数/资产数/失败任务/最近包，移除 CLI 项目部署 mock
- [ ] Package 库 + 资源浏览器页（三栏骨架、虚拟表格 10 万+条 60fps、TGI 分组树 FTree 惰性展开）；先由 local-key Mock 填充静态流程
- [ ] 筛选/排序/搜索 + StatusBar 信息条 + 右键基础组（复制 T-G-I、Filter by、导出原始字节）
- [ ] 坏包/缺文件/无权限 → 页面错误态（§5 用例）
- 验收：Mock 流程先通；再用 `D:\\ea-games\\SimCity` 下具体 package 进行 Tauri 联调

### U2 — 资源详情与属性/资产页（P0 M2+sc-registry）
- [ ] 单页面页内 tabs：hex（4KB 分页）/文本（Shiki）/属性/图片/音频/视频/模型
- [ ] 属性表独立页 + 资产目录页（需要后端 `parse_properties`/`combine_assets`；Mock 先填充稳定 DTO）
- [ ] 统一预览 surface、格式徽标、状态、复制/导出和诊断面板
- [ ] 诊断面板雏形（severity/code 过滤 + 重试/跳过）
- 验收：Mock 全部 tabs 可操作；真实后端能力接入后，249 属性表、122 资产组真实数据可浏览

### U3 — 模组/资产制作 + 导出（P1 核心，对齐 modding-suite）
- [ ] 模组项目页（`openscp.mod.toml` 列表/向导——向导只生成 manifest）
- [ ] 资产制作工作台（OBJ 导入/LOD 声明/property patch/Validate/Build）
- [ ] 导出中心（批量任务、进度事件、成功/失败分列、可取消、导出历史）
- [ ] BuildReport/diagnostics 页
- 验收：P1 vertical slice（1 资产 + 2~3 LOD + 1 patch）一键构建 overlay package；失败不崩溃

### U4 — Three.js 查看器 + Vue Flow 工作流 + Watch（P1）
- [ ] 模型预览（Three.js + GLB/OBJ：材质/骨骼/动画/LOD 切换/wireframe/bbox/法线 UV 辅助）
- [ ] 右键平移/角度调整、滚轮缩放、光照方位/高度控制、reset view；键盘 fallback 与 reduced-motion
- [ ] Three.js lazy-load；换模型/卸载时释放 renderer/geometry/material/texture/controls/object URL
- [ ] OBJ 快速预览仅作即时反馈；Validate/Build 永远走 Rust importer
- [ ] DependencyWorkflow 页（Vue Flow 依赖子图 + 流水线状态 + 列表 fallback）
- [ ] 文件监听 debounce：OBJ 保存 → 局部重导入 → 预览/诊断刷新
- [ ] 媒体预览（`<video>` + ffmpeg 临时文件；vgmstream→wav）；活动记录页
- 验收：模型预览不泄漏资源；右键/光照控制可用；watch/媒体失败只刷诊断

### U5 — 高级编辑器 + Windows 发布
- [ ] LotEditor/DecalDictionary/Path/Effects 按需裁剪评估
- [ ] 设置页完善（游戏目录/locale/s3db/外部工具/外观）；首启向导；About；a11y
- [ ] `pnpm tauri build` EXE/安装包验收：§5.4 全用例在发布版复跑
- 验收：发布候选可启动、可打开、可提示错误、可继续操作

---

## 7. 专项约定

### 7.1 Three.js 边界

Rust 是格式解析与校验的唯一权威，产出 GLB/Mesh 预览数据；Three.js 只做渲染层。依赖最小化：`three` + `GLTFLoader` + `OBJLoader` + `OrbitControls`，lazy-load。Three.js 对象不进 Pinia；组件卸载/换模型时释放 renderer、geometry、material、texture、object URL。大模型解析交给 Rust/Tauri 或 Worker，不阻塞 Vue 响应式更新。

### 7.2 i18n 与术语

`navigation.*` 组名 + 各页 `titleKey` 双语同步；统一术语：Package/Resource/Property/Asset/Mod/Workflow/Export（中：包/资源/属性/资产/模组/工作流/导出）。TGI、hash、文件名等动态值不进 locale 文件；registry 名称优先，缺失时十六进制回退。增加 locale key 中英一致性检查。

### 7.3 性能

Package 索引不整包载入 UI（分页 + 虚拟滚动）；资源 payload 惰性读取；RefPack 解压结果复用后端缓存；P1 重页面（Three.js/Vue Flow/导出中心）全部 lazy-load；代表包（DLC0 643 条、EP1 数万条）作为性能基线 fixture。

### 7.4 可访问性与状态

每个数据页面必备 loading / empty / error / success 四态；graph 类页面必须有列表 fallback；键盘导航与焦点管理沿用壳既有约定（Ctrl+K、方向键、Escape）。

---

## 8. 验收清单（导航与路由）

- [ ] 菜单只展示 OpenSCP 业务入口，showcase 不在生产导航
- [ ] 「模组/资产制作」一级 group 存在且 P0 仅含资产目录
- [ ] 每个可见菜单项有可访问 route + 双语 titleKey
- [ ] 详情路由 `hideInMenu` + `activeMenu` 高亮正确
- [ ] P1 未实现能力不出现在 P0 菜单
- [ ] `DefaultLayout`/`SidebarNav` 零业务状态；业务只在 `src/pages/` 与页面级 store/composable
- [ ] P1 重型页面不在首屏 bundle
- [ ] §5.4 异常用例全过；Tauri 进程在所有用例中存活
- [ ] `pnpm check` / `pnpm test` / `pnpm build` / `cargo test` 全绿
