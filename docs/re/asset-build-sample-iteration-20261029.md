# 构建系统样本迭代：大学与商业建筑资产 mod

日期：2026-10-29

本轮只使用工作台已经创建的两个资产 mod 做构建方案验证，不接入资产工作台，也不修改这两个用户项目。

## 样本事实

| 样本 | 当前内容 | 对构建方案的意义 |
|---|---|---|
| `C:/Users/FluffyCHi_Xing/AppData/Roaming/com.fluffy.openscp/mods/大学-综合实验楼-资产项目` | `package.json`、8 个节点 schema、`asset.lot.json` v1、一个 26.6 MB 的 LOTM；当前文档有 12 个单元但六个 texture input 和 building 节点的 `config.asset/document/property` 仍为空 | 这是“只有模型输入”的最小样本。它可以验证资源快照、LOTM 身份和缺失绑定诊断，但不能直接进入 Property 写回 |
| `C:/Users/FluffyCHi_Xing/AppData/Roaming/com.fluffy.openscp/mods/商业地标资产开发` | `package.json`、`asset.lot.json` v1、一个 45.99 MB LOTM、15 个 PNG 和 5 个参数 JSON；文档有 17 个单元，包含 lot、prop、light、path 等单元 | 这是“有场景语义和六槽候选输入”的复杂样本。它可以验证文档闭合、资源依赖清单和不支持字段阻断，但仍没有参照 package、Unit/Property TGI 或输出包 |

两个样本的 `building-asset-7.json` 都只是 v1 节点配置，`asset.lot.json` 也仍是 v1，尚未成为 M1 所需的 v2 Property schema。当前不能把“存在 LOTM/贴图”误判为可构建的游戏资产。

## 方案调整

M1 的第一个样本目标从“直接构建新建筑”调整为“样本审计 + 原生行为克隆”。每个样本先生成一份外部的 build fixture（放在项目 `tmp`，不写回用户 mod）：

```text
sample-fixture/
  source/                  # 样本原始文件的只读快照和 SHA-256
  asset-v1.json            # 原始工作台文档（若存在）
  binding-report.json      # 节点、槽位、LOTM、语义单元的诊断
  m1-schema.json           # 仅在 reference.package 和 Property TGI 齐全时生成
```

fixture 阶段只做纯读取和诊断：

1. 校验 `package.json` 节点图、节点 schema 引用和六槽连接。
2. 读取 `asset.lot.json`（商业样本）或标记其缺失（大学样本）。
3. 为 LOTM、贴图、参数 JSON 记录路径、内容摘要和识别出的格式，不把它们假设为 RW4/Property 可写资源。
4. 将每个语义单元标为 `cloned`、`modified`、`unresolved` 或 `unsupported`。
5. 只有存在项目内参照 `.package`、明确 Unit/Lot/菜单 TGI，并且字段 patch 可表达时，才生成 M1 v2 schema；否则只生成阻断报告。

## 两个样本的预期结果

- 大学样本第一轮应得到 `missing-reference-package`、`empty-building-asset` 和 `missing-six-slot-inputs` 诊断；它的 `asset.lot.json` 可作为场景文档快照，但不能代替参照包。它适合验证“最小输入不能静默产包”。
- 商业样本第一轮应识别 lot 尺寸、路径点、prop、light 和 LOTM，但对 `scene.model`、六槽贴图写回、Unit/菜单/ER2 依赖报告为 `unsupported` 或 `unresolved`。它适合验证“复杂文档不能被 M1 丢字段”。
- 两个样本都不应调用 `asset_build_m1_build` 产出可安装包，除非另备一个包含参照建筑资源的 package 副本，并补齐目标 Property 的完整 TGI。

## 后续实现顺序

1. 在后端新增独立的 sample fixture/diagnostic 纯函数和测试，输入为项目根目录下的文件快照，输出稳定的 `binding-report.json`；不改工作台入口。
2. 用商业样本覆盖“lot/prop/light/path 被发现但阻断写回”的测试，用大学样本覆盖“仅 LOTM 仍不能构建”的测试。
3. 准备两个临时参照 package（大学和商业各一个），只选一个可确认的 Property 标量字段作为 M1 patch，验证重读和 overlay 报告。
4. 再扩展到 Unit/Lot/菜单依赖闭包；在此之前不实现六槽贴图或 GLB/LOTM → RW4 写回。
5. Raster/RW4 编译器仍排在 Property-only 样本闭环之后，且不把工作台 v1 schema 直接升级为可写契约。

## Property-only fixture（2026-10-29）

已在 `tmp/asset-build-fixtures` 建立临时、可删除的回读 fixture。参照包来自本机
`D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package` 的只读副本，两个样本分别
命中真实 Lot Property：

| 样本 | Lot Property TGI | 原始 LotSize | fixture patch |
|---|---|---:|---:|
| 大学 | `00B1B104:58F0C000:2063CCDD` | `[192, 96]` | `[193, 96]` |
| 商业建筑 | `00B1B104:41F0C000:0483239E` | `[80, 80]` | `[81, 80]` |

两个 patch 都带 `expected` 前值条件，只验证 Property 编码、DBPF overlay、资源重读和
源包不变。测试命令为：

```powershell
$env:SC_ASSET_FIXTURE_ROOT = (Resolve-Path 'tmp/asset-build-fixtures').Path
cargo test -p fluffy-open-scp real_asset_sample_property_fixture_readback --lib -- --ignored
```

fixture 只在 `tmp` 下生成，不写回 AppData mods，也不接入工作台。它证明了两个样本
已经可以进入 Property-only 构建验证；模型替换、六槽写回和 Unit/Menu/ER2 仍未进入
这个 fixture。

已补充 raster-only overlay fixture 测试：每个真实 Lot Property 都生成一个新的
raster instance，修改顶层 RGBA 的一个像素，重新编码为 pixFmt21+mip 链，并把
`0x0CCB7FD5` LotMask 引用指向新 TGI。overlay 重读后，Property 引用和 raster
像素均与预期一致；源 `SimCity_DLC0.package` 的 SHA-256 前后一致。fixture 输出只在
`tmp/raster-overlay-fixtures`，不写入游戏目录或 AppData mods。

处理逻辑已抽到 `asset_build_m1::raster`：它是纯函数式的 raster 编码器，负责解码、
单像素变更、重新编码和语义回读；`ensure_target_free` 在写入前拒绝已有 TGI，避免
覆盖官方资源。包打开、Property patch 和 overlay 写盘仍由上层 fixture 负责。

样本扫描现在同时输出 lot、prop、bin、path point、path 和 path entry 计数，并把
`paths[]` 的缺失或非法 `entry`、缺失 `source` 标为阻断诊断。当前 AppData 中两份
JSON 都声明了一个 `paths[].entry`：大学为 `675B61B1`，商业为 `418A18B1`。这说明
扫描器此前把“有路径点”与“路径语义闭合”混在一起的问题已经拆开。下一步会在
引用包可用时继续校验 `0x0CC8FE7A` 的 per-path Key 和 `0x40E0C400` PathEntry
资源是否实际存在；仅有 JSON 字符串不会被视为合法道路。

跨 `SimCityData` 全部 package 的闭包测试已加入：它按 source TGI 找到路径 Property，
检查 `0x0CC8FE7A` 中是否绑定当前 entry，再检查对应的
`00B1B104:40E0C400:<entry>` 资源。当前两份 AppData 样本均通过这三层检查；如果
商业样本在另一份工作区状态中确实没有 PathEntry，这个阶段会产生
`path-source-property-missing`、`path-entry-not-bound` 或 `path-entry-resource-missing`
并阻止 package 构建。该结果也说明当前文件扫描得到的商业 JSON 与“完全没有 entry”
的描述不一致，需要以实际 schema/package 文件为准继续定位差异。

这套校验已接入 M1 `build_from_schema` 的 staging 前 gate：只要项目内存在
`scene.document`，路径语义会在任何 Property patch、输出目录创建或 package 写盘
之前执行。缺 entry、source Property、per-path Key 或 PathEntry 资源都会直接返回
错误，并保证不会留下 `.build` 产物。没有场景文档的旧 Property-only fixture 会继续
保持兼容，直到它声明了实际场景文件。

大学的真实 package fixture 已进一步验证：DLC0 中的源 path Property 绑定
`675B61B1`，但对应 PathEntry 资源实际位于 `SimCity_Game.package`，TGI 为
`00B1B104:40E1C400:675B61B1`。构建前把这一个资源合并到临时 DLC0 副本后，M1
成功生成 package，并在输出中重读到该 PathEntry。这里的 group 不能硬编码为
`40E0C400`；Property Key 与资源实际 group 需要按 instance 和 `0xC400` 组族解析。

## Raster 依赖闭合（2026-10-29）

Lot 与建筑贴图按游戏原生 raster 处理。两个样本的 `sourceLot` Property 均通过
`0x0CCB7FD5` 引用 `0x2F4E681C` raster，实际资源位于只读的
`D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package`：

| 样本 | LotMask key | raster TGI | 原始大小 | 编码结构 |
|---|---|---|---:|---|
| 大学 | `00000000:00000000:1CF819C1` | `2F4E681C:00000000:1CF819C1` | 174,816 B | pixFmt 21、可解码、含 mip |
| 商业建筑 | `00000000:00000000:DEDC4095` | `2F4E681C:00000000:DEDC4095` | 87,436 B | pixFmt 21、可解码、含 mip |

`asset_build_samples::inspect_project_with_raster` 现在会输出 LotMask key、解析出的
raster TGI、包路径、SHA-256、尺寸、mip 数和 `decode_supported`，并用
`RasterImage::decode_top_mip_rgba` → `build_raw_bgra` → 再解码做像素级 roundtrip
检查。真实样本 ignored 测试已通过；测试只读映射游戏包，不会写入游戏目录或 AppData
mods。建筑六槽中的 PNG/JSON 仍只作为预览输入，不能当作原生 raster TGI。

这一步闭合了 LotMask 的 raster 依赖和编码验证，但没有宣称模型 LOTM/建筑材质已经能
转换为 RW4，也没有把 raster overlay 接入 M1 Property-only 构建器。下一步应在临时
fixture 中复制 raster、做可逆像素变更并 patch LotMask 引用，再验证 overlay 的资源
重读和源包哈希不变。

## 当前明确边界

- 不接入资产工作台，不增加构建按钮，不改变现有 v1 schema。
- 不修改 `C:/Users/FluffyCHi_Xing/AppData/Roaming/com.fluffy.openscp/mods/大学-综合实验楼-资产项目` 或 `C:/Users/FluffyCHi_Xing/AppData/Roaming/com.fluffy.openscp/mods/商业地标资产开发`。
- 不把 LOTM、PNG、参数 JSON 当成已经可写的游戏资源。
- 不生成使用 EA 原生资源的可发布包；样本只用于本地研究和回读验证。

## 构建器三问题修复（2026-10-30）

用户在 Property Viewer 验收构建产物时发现三类失败，全部根因落在 M1 构建器：

1. **模型不完整 + 贴图丢失**：`asset_build_m1::lotm` 把工作台 LOTM v9 容器
   （`magic|version|mesh_count|逐 mesh GLB|材质表|逐 mesh 绑定`）误读成
   「单 GLB 信封」——`mesh_count` 被当 metadata、只编译了第一个 mesh
   （商业样本 105 mesh 只剩 1 个）；材质 PNG 原样写进包但查看器
   `resolve_slot_texture` 只认原生 raster/RW4 TEXTURE，全部贴图解析失败。
2. **手动道路/prop bin 丢失、引用 lot 的 prop/light/pth 回魂**：构建器从未
   把场景文档 `units[]`/`paths[]` 写回 Lot property——输出保留了引用 lot 的
   原始 unit 列。
3. **商业建筑模型/单元全丢**：同 2；且引用 lot 无 LOD 键时不补写 LOD1，
   构建产物继续指向原建筑模型。

修复（全部带测试）：

- `lotm.rs`：重写为 v8/v9/v10 容器解析器（全部 mesh GLB、8/9 张 PNG 材质块、
  逐 mesh 材质/uvKind 绑定、诊断文本）；旧单 GLB 信封保持兼容。
- `native_model.rs`：`compile_lotm` 编译全部 mesh；GLB 展平应用节点变换
  （对齐视口 `normalizeLotModelScene`：未声明剥根旋转、声明 y-up +90°X）；
  顶点格式改为游戏原生 facade 契约（POSITION F3 + NORMAL F3 + COLOR
  D3DCOLOR（G=tint 参数列）+ TEXCOORD0 F2 + TEXCOORD1 F4=facade base/top）；
  围绕 bbox 自动居中落地（复刻 `measureFootprint`）+ 文档 modelTransform
  （WPF 行主序）烘焙；每唯一材质一个 MATERIAL section，MESH_MATERIAL_
  ASSIGNMENT 共享绑定。
- `materials.rs`（新）：材质 PNG → pixFmt21 raster（带 mip 链）、paramsF32 →
  paletteF32 TEXTURE 的独立 RW4；按内容去重；instance 由内容哈希确定性分配。
- `scene.rs`（新）：场景文档 → Lot property 单元列写回（prop 分箱/路径点/
  PATH_PAIRS/灯/效果/生成器/贴花），**先剥离全部原始 unit 列**（引用 lot 的
  资源不再回魂），列在首个缺字段处截断（原生并行数组约定），Transform
  flags 15 ⇔ unknown 规范化，写回后经 assemble_units 回读校验。
- `package.rs`：模型/材质/单元/LotSize/OverlayOffset/Placement/LOD 键统一
  lowering 到 reference.lot（Unit property 仅 retarget LOD）；场景有语义而
  缺 reference.lot 直接报错；路径语义校验前移；修复场景文档不存在时
  `project_file` 无条件报错导致的 3 个既有测试回归。
- 验证：`cargo test --workspace --lib` 全绿（新增容器/材质/单元/端到端
  回归）；真实数据冒烟 `real_commercial_workbench_lotm_builds_full_model`
  （`tmp/asset-build-fixtures/commercial-model`，46MB LOTM）输出
  **105 mesh / 105 绑定 / 98 唯一材质**、17 单元写回、构建 1.5s。

边界：输出包仍为参照包全量克隆（同 TGI 覆盖契约不变，lot property 决定
实际内容）；菜单/ER2/unitOverrides 仍不支持；PathEntry 闭包校验依旧需要
Graphics 包。

## 大学样本导出（2026-10-30，对拍用）

工作台尚未接入构建；本节导出由 opt-in 测试 `real_university_workbench_export_
builds_package` 直接驱动 M1 管线完成（fixture：`tmp/asset-build-fixtures/
university-export/`，含 mod 文档原样拷贝 + 46→26MB LOTM + DLC0 副本）。
`675B61B1` 的 PathEntry（SimCity_Game 独有，group 40E1C400）在构建前合并进
DLC0 副本，路径闭包校验因此通过。

产物 `university-lab.package`（SHA-256 `9170f959…715f6f4`，约 248MB）已放：

- `tmp/大学综合实验楼.package`（仓库 tmp）
- `D:/ea-games/SimCity/SimCityData/大学综合实验楼.package`（游戏对拍）

内容自检：74 mesh/74 绑定全部编译；lot `00B1B104:58F0C000:2063CCDD` 的
LOD1 指向编译模型；12 个作者单元（8 prop + 4 pathPoint）写回、原 lot 单元
列被替换；path_pairs [0,3]；122 个材质资源。注意：同 TGI 全量覆盖意味着
该包在游戏里会替换原版 `58F0C000:2063CCDD` 地块；对拍完成后删除即可还原。

## 对拍结果：三个未解缺陷（2026-10-30 记录，未修复）

大学对拍包（`university-lab.package`，74 mesh 编译全部通过、12 单元写回）
在 Property Viewer 验收时发现三个问题；工作台编辑器渲染（同一 LOTM 走
GLB 链路）一切正常，问题全部落在「编译 RW4 → 查看器解码」与「查看器
prop/贴图解析」两段。按优先级记录：

### D1 · RW4 mesh 解析丢面、模型不完整

现象：查看器里编译模型只剩立面框架+竖梃等部分 mesh，大片面板缺失（截图
2026-10-30）。构建侧 roundtrip 断言 74 mesh 的顶点/三角形数全部一致，即
RW4 字节是完整的——丢失发生在查看器解码/再导出链
（`build_lot_model_payload_for_test`）。

下一步排查入口（按嫌疑排序）：
1. 逐 mesh 统计 payload 导出：`is_exportable()`（`package_service.rs:4163`）
   与 `MESH_GLB_MAX_BYTES` 8MB 上限（`package_service.rs:4238`，
   "[GLB 超限跳过]"）各自丢了多少 mesh——写一个 ignored 测试对
   `university-lab.package` 跑 `build_lot_model_payload_for_test`，对比
   GLB 数 vs 74，并打印 mesh_diag。
2. 若 74 全导出：对比查看器再导出 GLB 与容器原始 GLB 的三角形数
   （`export_glb_with_colors` 路径），定位丢面环节。
3. 怀疑点备注：编译器把容器一个 mesh GLB 合并成一个 RW4 mesh；查看器
   `mesh_uv_kind` 的 facade 判定要求 FLOAT4 全顶点覆盖，混合流会掉到
   kind1（不影响面数，但影响下一步贴图链）。

### D2 · Property Viewer 里 prop bins 不渲染

现象：构建包的 lot 在查看器中只有模型，工作台编辑器里能出的 8 个 prop
（bin 1）不出（灯 spawner 同查）。

已知事实与假设：工作台 prop 的 `resourceId` == `modelTgi.instance`
（直挂 RW4 模型放置），构建器把该值写进 `PROP_ID` 列；而查看器 PE 精细
替换（`prop_models::resolve_all`）按「脚本资源表」语义解析 resourceID →
RW4（`src-tauri/prop_models.rs`，10 个实证 prop id），直挂模型的实例 id
不是脚本资源 id，解析失败 → 只剩锥标记。

下一步：确认 `PROP_ID` 列的运行时 id 语义与脚本表能否覆盖直挂模型；若
不能，给查看器加「PROP_ID 直挂模型兜底」（lot 单元直挂模型目前只有
工作台文档 `modelTgi` 承载，property 格式无对应列——要么查清脚本表合成
规则，要么查看器侧按「ID 无法解析时尝试当作 0x2F4E681B 实例」兜底）。

### D3 · Property Viewer 贴图丢失，仅剩烘焙色

现象：编译模型显示烘焙顶点色（bake 链工作），但 tint/法线/shader 等贴图
细节全无；编辑器同模型贴图正常。

已知事实：材质槽资源在包内（122 个，PNG 已转 pixFmt21 raster、params 为
paletteF32 RW4）；查看器槽位解析 `find_resource_across_packages_named`
先查当前包（`package_service.rs:3439`），不存在跨包遮蔽问题；
`RasterImage::parse` roundtrip 有单测。

下一步：dump `resolve_material_resources` 的 slot 诊断（`MaterialResources.diag`
逐槽记录解析结果，来源 `build_lot_model_payload_for_test` 的材质 diag 块）
对 `university-lab.package` 跑一遍，确认 slot0-5 是「解析失败」还是「解析
成功但前端未消费」；同时核对前端 tint 链消费条件（`meshUvKinds` kind2 +
`LotMaterialTextures` PNG 齐全）与我们导出的 uv_kind/材质 PNG 的匹配情况。
另查 `bake_vertex_colors` 生效说明 slot0/slot1/slot4 链路已通——贴图丢失
更可能出在 slot2/3/5 或前端消费侧。

### D1 补充取证（2026-10-30 深夜）： Blender 源排除，丢失定位到前端

用户提出"Blender 工程本身不是完整建筑，是否为缺面主因"。结论：**不是**。

- Blender 工程的"不完整"是 NY 原生管线的设计：大块零厚度立面平板 +
  着色器投影窗户（不做实体墙建模）。university.lotm 实测 74 mesh /
  17,954 tris，全局 bbox 98.5×68.8×50.9m 覆盖完整 footprint；011/013/
  015/016 等即 44-53m 见方的立面平板，框架/竖梃/屋顶细节齐备。源没有丢。
- 编译侧 roundtrip 断言全过（顶点/三角形逐 mesh 一致）。
- **查看器后端逐 mesh 导出零丢失**（新增诊断测试
  `diag_university_compiled_model_viewer_export`）：74/74 mesh 可解码、
  `is_exportable` 全过、无一触发 8MB GLB 上限，17,954/17,954 tris 全部
  进入 per-mesh GLB。
- tint 贴图 alpha 语义实测：74/74 材质带 tint PNG（256×256 / 512×256，
  RGBA），alpha>0.5 占比 85–100%（游戏 shader 以 tint.A>0.5 做 clip 判定，
  我们的 raster 转换保留 alpha）。

结论：D1（丢面）与 D3（贴图丢失）**合并为同一根因**——查看器前端对编译
payload 的材质/tint 链消费：后端 GLB 数据完整，平板在前端被整体不绘制
（疑似材质链缺失时 shader clip/透明处理）。下一步直接排查前端
`three-gltf.ts`/PE 材质装配对编译 payload 的消费路径，用查看器 info 面板的
payload 诊断文本（后端已输出 uv_kind/材质解析行）对照编辑器（同一 LOTM 走
GLB 链路渲染正常）的差异。

### D1/D3 终局取证（2026-10-30）：后端全链路清白，问题收窄到查看器前端

用户指出 Mesh 面板里编译网格"缺面"并质疑转换。逐 mesh 硬对比结论：

1. **RW4 转换零缺面**：源容器 vs 编译 RW4 逐网格对比（74/74）：顶点数、
   三角形数完全一致（NY_Native_000→#5: 870/290；004→#25: 1824/912；
   073→#370: 48/16……）。Mesh 面板预览**只画当前选中的一个网格**——#5
   本来就是"框架+竖梃"网格（290 tris），整栋建筑是 74 个网格的叠加，
   单网格预览"不完整"是预期行为，不是转换缺陷。
2. **后端材质槽解析全部成功**（新诊断
   `diag_university_material_slot_resolution`）：74 材质每个槽位都能解析
   ——slot0 paletteF32 参数表（1×4，row0-3 齐全）、slot1 tint、slot2
   normal、slot3 shader、slot4 512×16 调色板、slot5 512×512 内景，全部
   生成 PNG；`bake=true`、`params_cols=1`。
3. **uv_kind 分布正确**：27 个 kind2（facade）+ 47 个 kind1（普通），
   与源结构吻合。

结论：编译 RW4 的几何与材质数据在查看器后端**全部就绪**；「整模型视图
缺平板」与「无贴图仅剩烘焙色」都是**前端消费侧**问题（渲染模式/材质装配
未走 refined tint 链，见 PropertyEditorViewport.vue:1330 的
`renderMode === "refined"` 条件——值得第一查）。诊断测试保留为 opt-in：
`diag_university_compiled_model_viewer_export`、
`diag_university_material_slot_resolution`（SC_ASSET_FIXTURE_ROOT）。

## D2/D3 修复（2026-10-30 深夜，已实施）

用户确认精细模式已开、贴图仍缺、prop 不渲染后，定位到三个实锤缺陷并修复：

1. **uv_kind 降级（D3 主因）**：容器声明全部网格 uv_kind=2（tint 链），但
   查看器 `mesh_uv_kind` 从解码网格重推导（要求 facade FLOAT4 幅值 >8）。
   插件导出的 facade UV 幅值可低于 8（如 1.0 的普通图集域）→ 大学 47/74、
   商业 80/105 个网格被降级 kind1 → 前端只给顶点色（=「仅剩烘焙色」）。
   修复：all-FLOAT4 + D3DCOLOR（参数列）即视为 facade（kind2 提升而非仅防
   降级）。**原生安全证明**：SimCity_Graphics 全包 2278 个可导出 mesh 普查
   （`uv_kind_relaxation_census`），提升逻辑翻转 0。
2. **TEXCOORD_1 列/种子丢失（D3 次因）**：插件容器把参数列与内景种子放在
   VEC2 TEXCOORD_1（x=列/255、y=种子/255），编译器此前按「备用基础 UV」
   丢弃。修复：TEXCOORD_0 存在时 VEC2 TEXCOORD_1 = (列, 种子)，写入
   D3DCOLOR.G/B；GLB 再导出按原生约定映射回 TEXCOORD_1.xy。
3. **prop 直挂模型兜底（D2）**：PROP_ID 列存的是 RW4 模型实例 id，不在
   脚本资源表内。`prop_models::resolve_all` 加 `resolve_direct_model` 兜底：
   脚本表失败时按 0x2F4E681B:0:<id> 跨已打开包直查（校验载荷含 MESH 段）。

修复后重导出：university uv_kind 分布 {2:74}（修复前 {2:27, 1:47}）、商业
105 mesh/98 材质不变。交付文件已刷新（SimCityData 旧包被占用，新包为
`大学综合实验楼_v2.package`，请关掉占用后删除旧文件）。新增回归测试：
`vec2_texcoord1_column_and_seed_survive`、
`mesh_uv_kind_accepts_color_as_facade_signature`、
`direct_model_fallback_resolves_rw4_instance`；一次性普查保留为
`uv_kind_relaxation_census`（`SC_UV_CENSUS_PACKAGE`）。

## TANGENT 缺失修复（2026-10-30，v3）

用户以 v2 复测：墙贴图已有、窗户玻璃/空调风扇仍空白、prop 仍缺。与用户指定
的原版对拍对象 `00B1B104:41E1C000:B64CF55B`（模型 0x2F4E681B:B64CF554，
查看器渲染正常）逐字段对比 payload，发现编译模型与原版的最后一个结构差：

**TANGENT 缺失**。原生模型 2267/2267 全部带 TANGENT0；插件容器 GLB 也逐
顶点带 TANGENT:VEC4；v1/v2 编译器的顶点格式（52B）没有切线元素 → 再导出
GLB 无 TANGENT → 前端 tint 着色器的窗户内景/浮雕链（`USE_TANGENT` 分支）
退化为屏幕导数拟合切线架，在窗户 UV 域上退化 → 玻璃/风扇渲染成平色。墙的
基础 tint 链不依赖切线所以幸存——与用户观察完全吻合（墙有贴图、窗户/风扇
空白）。

修复：编译器保留源 TANGENT（节点变换后写入顶点，顶点格式 52→64B，
usage=6/FLOAT3），再导出按原生约定输出 TANGENT:VEC4(w=1)。
`diag_university_uv_stream` 实证再导出 GLB 已带 TANGENT:VEC4，属性流与
原生一致。新增交付 `大学综合实验楼_v3.package`（v2 无切线修复，作废）。

prop（D2）同步加固：`resolve_prop_models` 解析前自动打开注册
Graphics/Game 源包，直挂模型兜底可命中游戏包内的模型实体。

待用户以 v3 复验窗户/风扇/prop；若仍有残余，下一步用 dev 构建
`[building-materials]` 逐网格装配日志定位前端分派。

## Delta 打包 + 菜单录入（2026-10-30，已实施）

### Delta 输出模式（`outputs.mode`，默认 `delta`）

NightHouse 解剖（17 条资源 0 覆盖全新增 + 00B2CCCB 模板继承）证明游戏按 TGI
合并全部包、引用跨包解析，mod 只需携带新增/改写资源。M1 据此重写输出契约：

- `BuildResources`（`asset_build_m1/package.rs`）：全量参照快照 + `touched`
  集合；4 个写入点（patch 任务、slot raster、场景模型/材质、property 语义）
  全部经 inherent `insert` 记录。
- `outputs.mode: "delta"`（默认）只写 touched；`"full"` 保留旧全量克隆契约
  （schema/IR/校验三层都支持）。delta 校验 = 输出包整体读回与 touched 子图
  逐字节比对（`connector::verify`）。
- `semantics.menu` 激活时参照身份不动：same-TGI lot/unit 的场景 lowering
  跳过（场景单元降入克隆出的新 lot 叶子）。
- `M1BuildResult` 新增 `delta_mode`、`menu_instance`；`resource_count` =
  输出包条目数。

### 菜单录入（`semantics.menu`，`asset_build_m1/menu.rs`）

新建筑身份 = 6 件资源（同 instance X，标题内容哈希 + 线性探测）：

| 资源 | 内容 |
|---|---|
| `00B1B104:09878A01:X` | Menu2：parent→公园子根/plop 根、uiCategories→分类容器、uiPosition、title/desc→自有 locale 表 `BF82F7CE`、toolIconKey/iconNormal→PNG、marquee→JPG、menuUnit→X（T/G=0） |
| `00B1B104:40E0C000:X` | cUnit 叶子：parent→模板、lot 链接（0x08E17ED7）→X、可选造价/维护 |
| `00B1B104:40E1C000:X` | Lot 叶子：参照 lot 克隆 + parent→模板 + 场景 lowering（单元列/LotSize/Placement/LOD） |
| `0A98EAF0:02FABF01:BF82F7CE` | locale 表（BOM + 宽松排版 JSON，键 `"0x%08x"`） |
| `2F7D0004:40E02400:X` | 128×128 图标 PNG（可 `iconPng` 指定，缺省程序生成） |
| `3F8662EA:0:X` | 454×263 rollover JPG（程序生成） |

可配置字段（全部可选，除 title）：`categoryInstance`（默认地标
`3F43E038`；公园 `07049250`）、`menuParentInstance`（默认 plop 根
`AF042E9A`；公园子根 `F970A822`）、`unitTemplateInstance`/`
lotTemplateInstance`（默认地标模板对 `4E01E311`；公园测试用 NightHouse
实证对 `565B72B9`/`34CE7E14`）、`uiPosition`、`constructionCost`、`
maintenanceCost`、`iconPng`。字段依据：menu-item-relations-20261018、
willis-tower-landmark-20261027、menu-export.ts（实机闭环）、tools.json
（公园工具 parent/uiCategories 同款）、NightHouse 解剖。

### 干净包交付（tmp/ 与 SimCityData 各一份）

| 包 | 条目数 | 尺寸 | 说明 |
|---|---:|---:|---|
| `大学综合实验楼_clean.package` | 124 | 51.7MB | same-TGI 覆盖 lot + 74 mesh 模型 + 122 材质（原 v3 全量克隆 248MB/1806 条） |
| `商业建筑_clean.package` | 96 | 45.7MB | same-TGI 覆盖 lot + 105 mesh 模型 + 94 材质 |
| `商业公园菜单测试.package` | 101 | 45.7MB | 新 instance `0xBA525BE6`，公园面板（无放置限制），参照 lot 不覆盖 |

测试注意：SimCityData 中的旧 `大学综合实验楼_v3.package`（248MB 全量克隆）
与新干净包存在同 TGI 冲突，进游戏验证前应删除（或移出）。

### 测试

- 单元：`delta_output_writes_only_touched_resources`（delta/full 双契约）、
  `menu_registration_creates_new_identity_resources`（公园档案逐字段断言）。
- 真实（opt-in，`SC_ASSET_FIXTURE_ROOT`）：
  `real_commercial_workbench_lotm_builds_full_model`、
  `real_university_workbench_export_builds_package`（各加 delta/条目数
  断言）、`real_commercial_menu_park_builds_package`（公园菜单全链路）。

## 菜单不可见根因：property flags 缺 0x8000（2026-10-30 晚，已修复）

用户以 `000_test_building.package`（商业公园菜单包改名）进离线版实测：菜单
整条不可见。与同安装内已验证可用的 overlay（`OpenSCP-ui-overlay.package`、
tent-bulldozer 探针）逐字段对拍（`menu_diff.rs`）定位：

- **根因**：`sc-properties::canonical_flags` 对程序新建（flags=0）的属性
  写出标量 `0x0000` / 数组 `0x0030`；游戏加载器要求每条目带 **0x8000**
  有效位，数组为 `0x8000|0x10`（定长元素数组再带 0x80 → Key/数值数组
  `0x809c`，Text/BBox 数组 `0x801c`）。flags=0 的 Menu2 整条被丢弃。
  此前 Rust 编码器只处理过「从游戏文件解析来的属性」（flags 透传），
  从零构建路径从未上过实机；前端 TS 编码器是手写 0x8000 系，所以旧
  overlay 正常。
- **修复**：`canonical_flags` 对 flags==0 输入合成上述规范值（非 0 输入
  仍逐位透传）；`menu.rs` 补齐 NightHouse 实证形状缺失字段（六态图标
  09756955..5A、背景图标 0x0C65D729、解锁三件套 0DE84DC9/D3/DC →
  市政厅 1 级）；`parse_encode_parse` 测试数据改为游戏规范 flags。
- **重交付**：三个包全部重建（商业 96 条 / 大学 124 条 / 公园菜单 101 条，
  instance `0xBA5224C6` 不变），已更新 tmp/、SimCityData/ 与离线版
  `000_test_building_v2.package`（旧 `000_test_building.package` 被游戏
  占用未删，测试前请删旧留新）。
