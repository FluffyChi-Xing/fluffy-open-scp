# 资产流程初步接入与程序化建筑路线（2026-10-10）

## 本轮落地

- Property Editor 解锁后经过 `openscp.lot-asset/1` JSON → `SchemaPropertyViewport` → 既有渲染器。变换、字段、增删、隐藏与图层状态均进入完整文档。撤销/重做仍使用现有命令层，JSON 是渲染和持久化契约，不存 Three.js 对象。
- `SchemaPropertyViewport.vue` 可嵌入详细编辑器；`AssetModelPreview.vue` 提供轻量模型卡片，支持 LOTM 游戏材质和 GLB 几何。卡片只预览模型，完整 lot/props/decal 在展开的 Property Editor 中编辑。
- outliner 与组件库共用 `assetCategories.ts`：树木、车辆、垃圾桶/垃圾箱、街道家具、建筑、其他。分类依赖已解析的树资源、已核实 ID 和名称规则，不把未知模型强行分类。目录范围仍是已加载 package，并保留后端命名目录 400 条上限。
- 修复放置矩阵 14 → 12 元素，克隆引用的树 prop 时保留 resourceId、原型缩放及基向量。原始树模型使用树图集解码，不再按 PNG 文件大小猜漫反射。编辑态启用真实资产渲染。
- 模型目录与缓存区分 package + 完整 TGI；新增原始模型在 JSON 中保留源包路径，以便重开时解析新的 packageId。路径依赖仍需在当前机器可访问；跨机器分发需后续依赖打包。
- 地面预览法线图不再把数据通道 Alpha 当透明度，地面材质法线强度调整为 1.8，仍由实际太阳动态照明。用户之前对比图的左右差异主要来自灯光方向；本轮同灯光比较只验证细节增强，不宣称获得原纹理不存在的更高分辨率。
- 关闭/重新打开编辑 Sheet 时，GPU 释放等待在飞 shader 编译结束，避免 Three.js `currentProgram.isReady` 异步轮询访问已释放材质。

## 资产流程使用方式

新建资产类开发项目后生成 6 个贴图输入节点、1 个建筑节点、1 个输出节点。建筑节点上传 `.package`、`.lotm` 或 `.glb`；原生包可选择 Property 并展开编辑器。上传文件复制到项目 `assets/models` 或 `assets/textures`，按内容摘要命名，不修改来源文件。

六个输入 handle 绑定对应贴图节点。空输入表示保留原始材质；非空输入应用到选中的材质索引。上传、槽位连接、材质选择及编辑文档均保存到节点 schema。详细编辑器使用相同的六槽预览材质。用户需点击“保存节点与编辑 Schema”持久化 Sheet 中的草稿。

| 槽位 | building4 语义 | 当前上传格式 |
|---|---|---|
| slot0 | paletteF32 参数表，4 行 float4 | JSON `{cols, values}`，values 长度为 cols × 16 |
| slot1 | 颜色控制图 | PNG |
| slot2 | 法线 / AO 数据 | PNG |
| slot3 | shader map（高光、窗洞、内景控制） | PNG |
| slot4 | 调色板 | PNG |
| slot5 | 室内图集 | PNG |

这套定义适用于已逆向的 building4 材质，不代表全部 RW4 家族都有同样的六槽含义。slot0 是参数数据，不能当普通图片上传；GLB 的普通 UV/PBR 材质也不能自动变成游戏 façade 参数。

### 可构建范围

本轮完成初步预览/编辑/保存接入和原生包保真构建；**没有实现任意 GLB → RW4 编译器或编辑后 Property/RW4 写回器**。

- 未修改的原生 package：保留所有资源的 TGI 和解压内容，写出 DBPF overlay。
- 已保存 Property 编辑文档、上传贴图覆盖、GLB/LOTM 草稿：可预览、可保存，但构建验证会报告尚未接通写回，不能静默输出未修改原包。
- DBPF 重打包不保证文件字节或原压缩方式相同；保证的是逐资源身份与解压内容。未进行游戏内安装/运行验证。
- 自定义贴图不是简单的 DDS 替换：需对应 raster/RW4 编码、材质绑定、完整 TGI、正确颜色空间和 mip 链。

## 两个社区样本

`D:/open-scp-mods/大气的中央火车站/Oppie_TrainStation.package`：5 个资源，1 个 Property、4 个 RW4。Property 为 `00B1B104:40E1C000:8357B87F`；RW4 instance 为 `8357B87F`、`89435924`、`89435926`、`89435927`。

`D:/open-scp-mods/地铁mod/OFFLINE-ONLY_MaglevAsSubway.package`：46 个资源，包含 Property、RW4 及 `2F7D0004`、`3F8662EA` 类型。它不仅是建筑几何与图片，还包含脚本/布局相关依赖。

这决定了资产流程不能把社区包简化为“一个模型 + 六张图片”：专业模型节点只是其中一部分；保真构建必须保留其余资源，后续编译应只替换被编辑的资源。

## 程序化路线前瞻

结论：可以提供两条前端路线，但应汇入同一个资产 schema、验证器和游戏资源编译器。程序化几何预览可较早接入，能在游戏中使用的生成建筑仍依赖尚缺的编译环节。

### 参考项目的实际能力

核查 `D:/source-map/ProceduralBuildingsThreeJS`：

- `src/generator.ts`：`generateBuilding(BuildingParams)` 返回部件实例列表、屋顶 cap 和尺寸。实例包含 kit key、Z-up 的 4×4 矩阵以及 room 元数据。法式建筑尺寸为 `3 × Bays + 2`，模块化生成可确定性复现。
- `src/kit.ts`：从 `public/assets/kit.glb` 加载部件，以“模块 × 材质”构造 `InstancedMesh`。它依赖已有部件几何与材质，不是无需资产输入的通用建筑生成器。
- `src/nyc/gn.ts`、`src/nyc/flatten.ts`、`src/nyc/shadergraph.ts`：提供几何图解释、实例展平和材质图着色路径。Three.js 程序化材质不能直接写入 SimCity shader。
- `package.json` 使用 Three.js 0.185 系列，与当前项目版本相近；两边建筑资产采用 Z-up，有利于预览整合，仍需检查米制比例、朝向与地块锚点。
- 根 LICENSE 为 MIT（2026 mohamedachrefelouafi）。若复用源码，保留许可和版权说明；发布 kit、Blender 文件或纹理前仍需确认相关素材来源。此次未复制其代码或素材。

### 两条路线的分工

| 项目 | 专业开发者路线 | 普通用户路线 |
|---|---|---|
| 几何 | 上传已制备网格、LOD、法线、UV | 楼层/开间/屋顶/样式/seed 驱动 kit 生成 |
| 材质 | 明确六槽参数、贴图与 mesh → material 绑定 | 选择经过验证的游戏材质预设，由适配器生成 UV/参数表 |
| 地块 | 手工挑选/编辑 lot、props、decal、路径 | 选择游戏 lot 模板，按可建范围限制建筑体量 |
| 编辑 | 全量 Property Editor | 简化参数面板，必要时展开同一编辑器 |
| 输出 | 同一 `openscp.lot-asset/1` 与资源依赖清单 | 同一契约，并记录生成参数/seed/kit 版本 |

### 必须补齐的转换

1. **实例烘焙与几何预算**：实例矩阵乘入顶点；逆转置变换法线；正确处理镜像绕序；生成切线、包围盒和多级 LOD。预览的低 draw-call 不代表游戏导出后的三角形成本低。
2. **游戏材质适配**：普通 kit UV 不携带 façade 分区/参数列属性。先选择一种已验证 shader 家族，再实现“立面区域 → 参数列 → 六槽”的适配；不应承诺任意游戏纹理一键贴合任意生成器。
3. **纹理编码**：明确 sRGB/线性数据通道、alpha 语义、调色板与室内房间布局，生成正确 mip 和 raster/RW4 资源。GLSL/Blender graph 需要烘焙或替换为受支持的游戏 shader。
4. **资源写回**：RW4 网格/材质 writer、Property 二进制回写、新 TGI 分配与依赖重定位，保留源包内未编辑资源。
5. **场景语义**：lot 尺寸、建筑锚点、道路入口/路径、碰撞/占用和游戏能力参数不能从外观几何自动推断。此前两例 lot 偏移仍未完全解释，不能把边界框居中当成通用解法。
6. **验收**：先固定一个 lot 模板、一套材质和一个生成 kit；对照编辑器、重读写出包、游戏实际运行三方结果，再扩大预设目录。

推荐迭代顺序：原生包小范围材质/Property 写回 → 一种受支持网格格式的 RW4 编译 → 可复现的简化生成节点 → 经过验证的 lot/材质预设目录。两条路线可以共享已经接入的卡片与 Sheet，不需要各做一套渲染器。

## 验证记录

- `pnpm check`：通过。
- Property Editor 与 mod-flow 两个目录 Vitest：原 51 项通过；增加 shader 家族检查后相关 8 项再次通过（现共 52 项）。
- `cargo test -p fluffy-open-scp pe_schema --lib`：通过；修复既有测试的资源 ID、矩阵和 JSON 浮点样本，新增 spawner 稳定 ID 断言。
- `cargo test -p fluffy-open-scp mod_flow --lib`：11 项通过；地图样本和社区包样本默认忽略。
- 额外分别设置 `SC_FLOW_ASSET_PACKAGE` 为上述两个社区包，运行 `community_asset_package_roundtrip -- --ignored`：两者通过，逐 TGI 比较所有解压资源内容。
- Chromium + WebGL 真实 fixture：schema 视口加载，地面 2458 像素法线纹理、GL error 0；同光照法线 1/1.8 对比已查看。
- 完整编辑器交互：解锁、打开分类库、拖入树、检查真实 MeshStandardMaterial/map/落点、保存 schema 后重新挂载，均通过；无 pageerror。
- 流程图 IPC mock + 真实 LOTM：8 卡片、6 输入 handle、7 条连线，上传触发保存并显示 canvas，无 pageerror。原生文件选择对话框及游戏内表现仍需桌面人工验收。

可复验截图在 `tmp/asset-editor-placed.png`、`tmp/asset-flow-ui.png`、`tmp/property-render/ground-detail-{before,current}.png`；tmp 为本地验证产物，不作为源码依赖。
