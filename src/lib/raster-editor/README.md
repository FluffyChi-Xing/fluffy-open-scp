# Raster 编辑契约

独立菜单 `pages/studio/panels/raster.vue` 只组合 `components/raster/RasterEditor.vue`。
工作台 `RasterFlowTool.vue` 使用同一编辑器，以 `mode="flow"` 接收 `schema` 事件；
将文档序列化到现有 `FlowSchema.config.document`，由父级 `save-node` 持久化。
`asset` 继续保留原文件引用，编辑草稿优先于原文件参与建筑预览。

## 组件职责

| 模块 | 输入 / 输出与职责 |
| --- | --- |
| RasterProfilePicker | 用途值 → update:modelValue；建筑、Lot、Decal 分组 |
| RasterSourcePanel | profile → create/image/schema/parameters；载入数据，不拥有编辑文档 |
| RasterToolbar | 工具配置 v-model、undo/redo；通道与画笔参数 |
| RasterCanvas | document/history → change/pick-color；像素事务与撤销 |
| RasterParameterTable | 参数表 → update:modelValue；4 行 × N 列 float4 |
| RasterMaterialPanel | lot/decal/sampling v-model → loadLot/loadDecal/changed |
| RasterPreviewPanel | 只读 document/version/channel；原始通道与 Lot 染色示意 |
| RasterPreviewCard | 预览标题、全屏 Sheet、焦点恢复与键盘关闭 |
| RasterLotPreview | 当前原始 RGBA 与 Lot 配色 → PropertyEditorViewport 精细地面渲染；共享游戏地表/法线图集 |
| useRasterSession | 文档、历史、来源、脏标记及 schema 快照/恢复 |
| rasterIO | 保存对话框、原生副本导出、无损 PNG/JSON 下载 |

## 数据约定

`schema.ts` 定义并校验 `format: "openscp.raster"`, `schema_version: 1`。
图像文档内嵌 `image: { width, height, encoding: "rgba8-base64", data }`，
参数表内嵌 `parameters: { cols, values }`，两者互斥。来源保存完整 TGI。
JSON 包含用途、Lot 四组 RGB+tile、Decal 颜色/比例/字典/Entry ID、采样设置。
该格式是可恢复编辑文档，不是已编译游戏材质。

- slot0 为 building4 的 float32 参数表；`values.length === cols * 16`，row-major。
- slot1–5 为颜色控制、法线/AO、Shader Map、调色板、室内图集；不把车辆/其他 shader 自动视为 building4。
- Lot 遮罩原始 A/R/G/B 分别映射 LC4/LC3/LC2/LC1，不能把 A 一概当透明度。
- LotColor 的 Alpha 是 0–15 图案索引，不是覆盖率；地表图集是单独用途。
- Decal 用途是编辑语义，不会重写字典 MaterialId，也不生成 SDF 或配置投影/动画。
- 独立通道绘制在记录历史之前恢复其他三个通道；通道预览不改原始像素。
- PNG 使用 `png.ts` 直接编码；避免 Canvas 预乘 Alpha 抹掉透明像素里的 RGB 数据。
- 不自动重采样图片，数据贴图默认不生成 Mip。Lot 覆盖保存校验原尺寸。

## 输出边界

独立菜单可输出 Raster overlay package、新 Lot（Raster+Property）、Decal 字典新增/替换副本；
PNG 和参数表 JSON 可用于其他资产工具，完整 Raster JSON 可恢复编辑。
覆盖现有 Lot 保留其原始 Property，配色控件只用于新建 Lot。
包内载入目前受后端 RGBA 解码能力限制；压缩/浮点资源不会伪装成可绘制 RGBA8。
slot0 支持从内存包的 RW4 抽取参数表，以及参数 JSON 编辑/导入/导出；尚不写回 RW4。
包内目录按 Lot 属性、Decal 字典材质和建筑六槽引用筛选，并解析跨包依赖。
复制保留源尺寸。RW4 图像可导出 PNG/schema，禁止将 Raster 字节写入 RW4 TGI。

流程节点只保存 schema，不弹出 package 保存框。建筑预览通过 `rasterDraftAsset`
将五张图像和参数表绑定回原六槽。后端 `mod_flow_assets::validate` 阻止含编辑
文档的节点参与构建，直到 Raster/RW4 材质编译链真正接通，避免静默丢掉修改。
会话型 packageId 不作为跨进程稳定定位：恢复后原包未打开时，应重新绑定 Lot/字典。

## 界面与预览

`profileBehavior.ts` 是所有用途的完整行为表，统一声明数据通道显示、层绘制、四色配色和默认通道。
数据贴图画布以不透明 RGB 显示，A 单独灰度预览；显示转换不修改原始像素。
Lot 的层顺序为 A/R/G/B，Decal Color1–4 为 R/G/B/A，绘制与取色使用同一映射。
涂鸦支持覆盖强度；SDF 默认编辑 R 数值且保持其他通道；内景/RGBA 不显示无效四色控件。
各 Decal 类型使用 Property Editor 的同族 shader 和独立说明；SDF 预览为静态，笔刷不重建距离场。

所有界面词条（包括提示、错误、无障碍标签与用途说明）必须走 i18n；
Raster 词条集中在 `src/locales/raster-editor.ts`，同时维护中英文，运行中切换语言生效。
Package 列表来自 `useGamePackagesStore.opened`，自动选择内存中的首个包；
浏览器模式显示明确的桌面环境提示，不隐去资源入口。
Lot 3D 预览直接复用 Property Editor 的精细渲染器，传递原始 RGBA，避免 Alpha 预乘污染。
新建遮罩直接从已打开包读取游戏共享地表/法线图集，无需建筑 Lot Property。
绑定 Lot 优先使用自己的图集；共享图集不可用时继续搜索其他 Lot 会话，跳过空项或失败项。
借用图集不替换当前 Lot 的尺寸与边框；缺少图集时提示打开依赖包，不冒充真实渲染。
绑定已有 Lot 时保留尺寸、平铺周期及边框参数；绘制、配色、图案索引更新会触发地面重建。

依据：`src-tauri/src/package_service.rs` 的 building4 六槽解析、
`docs/blog/raster-lot-rendering.md`、`src/lib/decalEngineMaterials.ts` 及已有编码器测试。
