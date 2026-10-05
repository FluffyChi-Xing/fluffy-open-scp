# 模型↔lot 定位全链 + 脚本资源表解析（dev 完整 dump 终局取证）

> 来源：`D:\ea-games\simcity_dev\SimCity.unwrapped.exe.c`（dev beta v253660365 全量 Ghidra 反编译，
> 1,897,901 行，与 `docs/source-code/SimCity-dev-ida.c` 同一份；样本另含 SimCity.pdb）。
> SPID 哈希 = FNV-1 32 位小写折叠（`SPIDFromName` → `EA::StdC::FNV1_String8(s, 0x811C9DC5, kCharCaseLower)`，203583 行）。
> 取证日期：2026-10-05。全部行号可直接在该 dump 中复核。
> 本文关闭 migration.md §42.7-A3（placement 与 unit 的 frame 关系），并复核通过
> docs/re/unit-props-effects-spawners-paths.md 的全部锚点（该文行号即本 dump 行号）。

## 一、变换数学约定（PreTransformBy 复合方向定谳）

`cSPTransform` = { 旋转矩阵 S（三个行向量 xAxis/yAxis/zAxis），scale，translation，flags，modCount }。

- **点作用**：`p_world = t + s·Sᵀ·p`（`ApplyTransformToPoints`，1056596 行起：out.x = R.xAxis.x·p.x + R.yAxis.x·p.y + R.zAxis.x·p.z，再乘 scale 加平移）。即**存储行的列**是本地基向量在世界系的像。
- **`EA::Swarm::cTransform::PreTransformBy(this, xform)` = this ∘ xform**（xform 先作用）。数值验证（216639 行）：
  t_new = this.t + s_this·S_thisᵀ·xform.t；S_new = S_x·S_this（`rw::math::fpu::Mult`，216311 行，S 记法下标准乘积）；s_new = s_x·s_this。
  注意 Mult 的参数顺序在「字段即列」误读下会得出矛盾结论，必须用「字段=行、作用取转置」的 S 记法才能自洽。
- **scale 作用在旋转之后**（p_world = t + s·Sᵀ·p，不是先缩放）。
- 世界系 **Z-up**（`GetUnitTransform` 平移 z = GroundHeight；kSPUpVector = (0,0,1)）。

## 二、模型↔lot 定位全链（三变换 P / U / W）

### 2.1 数据层：cUnitLotEntry（96 字节/条，按 unitEntryIndex 索引）

`SC::cZoningGame::StartGame`（806045 行起）为每个 `GB::cEcoUnitEntry` 建一个 `cUnitLotEntry`，
`cUnitLotEntry::FillFromProps`（1058382 行，__userpurge）填充：

| 字段 | 来源 |
| --- | --- |
| mBBox | `GB::GetUnitEntryBoundingBox`（653427 行）→ `GetUnitBoundingBoxInternal`（653168 行）：**prop 0x0F9EFBA（BoundingBox，默认 ±1m）× prop 16492049（Float 均匀缩放）——声明值，非网格几何** |
| mFrontageMin | prop **196857325 (0x0BBBCDED)**（Float），取 max(属性, bbox.x) **− 1.0** |
| mFrontageMax | 2 × mFrontageMin（相邻 lot 合并容许到两倍） |
| mDepthMin | prop **196857386 (0x0BBBCE2A)**（Float），取 max(属性, bbox.y)，**无 −1** |
| mPlacementTransforms | `hash_map<graphicsGroup, cSPTransform>`：默认组 key=0 从 unit nonSimProps 读；**每个模型组**从 `PropertyManager::GetPropertyList(unitPropsInstance, modelGroup)` 的 per-group 列表读 |
| mModelKeys | `GB::FindUnitGraphicsModels`（653460 行）：filter = { instance=unit nonSimProps instance, group=graphicsGroup, type=0xB1B104（属性表资源类型）, groupMask=0xC0FFFF00 } |

`readPlacementTransform`（1056412 行）：prop **0x0DB7FB17**（Transform）→ 读后 **强制 scale=1.0**、
旋转压平为纯 yaw（z 列强制 (0,0,1)，x/y 列 z 分量清零）。即放置变换是 2D 的。

### 2.2 placement 变换 P 的方向（§42.7-A3 答案）

**P: model-local → lot-local**。推导链（全部函数体级证据）：

1. `cUnitLotData` = { P（首成员，本地），mUnitLotTransform（次成员，世界 W） }（构造器 780250 行；`CalculateUnitLotTransform` 内 `++v2` 按 cTransform 尺寸递增到次成员）。
2. `SC::cUnitLotData::CalculateUnitLotTransform(this, unitWorldTransform)`（1057120 行）：
   复制 P → 求逆（scale⁻¹、转置、−Rᵀt/s）→ W=U → `PreTransformBy(W, P⁻¹)` = **W = U ∘ P⁻¹**。
3. W 的用法（783060 `CreateSubstituteDefaultPath`、1000459 行 location-space 0xBAFEA257、1057224 `TestPlacement`）：
   把 **lot 本地点**（`GetUnitLotCorners` 角点、`GetUnitLotFrontCenter` 前缘中心）经 W 映到世界找道路/判 PointInLot
   ⇒ W: lot-local → world ⇒ P⁻¹: lot-local → unit-local ⇒ **P: unit-local → lot-local**。

放置正向（lot→unit）：`SC::cUnitPlacementData::GetUnitTransform`（1056549 行）直接在世界上构造 W：

- 从 zoning lot 凸包前缘（`GetConvexInteriorPoint` 0→1）取 f̂（单位向量）；
- 旋转列：localX→(−f̂.x, f̂.y, 0)，localY→(−f̂.y, −f̂.x, 0)，localZ→(0,0,1)，右手系；
- 平移 = mid(前缘内点0, lot bbox 中心) + R·(unitLotCenter − unitLotFrontRightCorner)（TestPlacement 中
  即前缘中点后退 depth/2 进地皮、沿 frontage 居中），z 吸附 `game->GroundHeight`；
- 单位世界变换 **U = W ∘ P**。

### 2.3 unit-lot 本地系约定

`SC::cUnitLotEntry::GetUnitLotCorners`（1056165 行）：矩形以原点为中心，
x ∈ ±frontageMin/2（frontage 轴），y ∈ ±depthMin/2（depth 轴）。
`GetUnitLotFrontCenter`（1056430 行）= mid(corners[1], corners[2]) 都在 **+depth/2** 侧
⇒ **+Y = 前缘（临街侧）**，+Z 上。**临街边锚定假设成立**：建筑前缘贴 zoning lot 前缘、
沿 frontage 居中、深度向 lot 内推进。

### 2.4 agent 位置空间

行人图 location space **0xBAFEA257**（1000459 行）= unit-lot 本地空间，走同一条
FillFromUnit + CalculateUnitLotTransform 链把 lot 本地点映到世界。

## 三、脚本资源表解析（离线配方，openscp prop 落地的硬前置）

### 3.1 主表定位

`GB::cEcoGameScripts::Init(gameID, numAddOns, addOnIDs)`（662313 行）：

- sim 主表 = `PropertyManager->GetPropertyList(gameID, group **0x40700000**)`；
- 属性 **0x0C947ABC**（LocalKey）→ nonSim 主表（`GetPropertyList(key.instance, key.group)`）；
- addon 组 **0x40700001**（逐 addOnID 同构）。

### 3.2 九张子表 = 主表上的 KeyArray 属性（每项是一个 group id）

（loader 函数 660380–662313 行；sim 与 nonSim 各读一遍）

| 哈希 | 表 |
| --- | --- |
| 0x07E0AC00 | unitGroups |
| 0x07E0AC01 | resourceGroups |
| 0x07E0AC03 | mapGroups（map rules） |
| 0x09138113 | zoneGroups |
| 0x09138114 | pathGroups |
| 0x09138115 | pointGroups |
| 0x0A30A557 | sinkGroups |
| 0x0A30A558 | transportGroups |
| 0x0A30A559 | bundleGroups |

### 3.3 表项枚举与 ID 语义

对每个 group：`PropertyManager->GetPropertyListIDs(groupKey)` 枚举全部 instance →
`GetPropertyList(instance, group)` 取属性列表 → 一条表项。
**instance id 即 resourceID / unitEntryID / pathEntryID**（`mResourceEntryIDToIndexMap[instance] = index`，
660810 行；`ResourceFromIndex(i)` = `&mResourceEntries[i]`，659129 行）。
**sim/nonSim 配对规则**：同名 instance 在对应 NS group 里的属性列表 = 该条目的 nonSimProps
（`FindNonSimProps`）。别名：表项属性 0x0E61772E（Key 数组）→ 别名 ID 再入索引。

### 3.4 prop 模型解析（prop id → RW4 模型 key）

`SC::cGraphicsResource::FillFromProps(game, props, resourceID)`（952181 行）：

1. prop **0x0D8C29C3**（Key）→ 显式模型 key；
2. 否则若资源属性表含 **16379835 (0x00F9EFBB) / 227111273 (0x0D897169) / 204919565 (0x0C36D30D)** 任一
   → 用资源自身 key（props->mKey）；
3. → `cGraphicsGame::GetInstancedModelId(gfxKey)` → mGfxId。另有 `SC::CreatePlumpInfo`（plump 缩放动画参数）。

消费端 `SC::cUnitBinDrawResources::FillFromProps`（957518 行）：遍历 **全局** `scripts->NumResources/ResourceFromIndex`
建 `mResourceIDToGraphics: resourceID → {mGfxId, mPlumpScale, mPlumpInfo, mUnitRefs}`（props 参数在函数体内未使用）。
lot 的 prop 列（0x0C12EF20 族）里的 id 即 resourceID，经此表得模型。

### 3.5 unit 附加属性（cUnitModel 装配段，958480–958620 行）

| 哈希 | 语义 |
| --- | --- |
| 0x0E1BAC61 / 0x0E1BAC62 | unit 位置表（GetPropertiesAsTable 两列：Key 数组 + Transform 数组）→ `mUnitLocations: vector_map<instance, transform>`（门口/车位/停机点等命名位置，agent 生成点只是用途之一） |
| 0x0E715928 | mBuildingVariation 基值（unit nonSimProps） |
| 0x0E715929 | 变体数：`mBuildingVariation += unitSlot % count` |
| 0x0F0E2BF1 | variation 影响资源（KeyInstance）→ `scripts->IndexFromResourceID` → `GlobalBins()->find` → mVariationInfluenceBinIndex（资源存量驱动变体选择） |
| 0x0FBA612 | 模型整体色（ColorRGBA，从模型属性表）→ mModelColor |

### 3.6 effects（SP::cModelWorld::AddAttachments，437000 行）

0x02A907B5 keys（主 id，HasVisualEffect 判定+创建）/ 0x02A907B6 transforms / 0x02A907BB externalIDs
（运行时 EffectControl 开关用）/ 0x02A907BC active bools / 0x02A907B9 **effectWorlds**（Key 数组）。
与 unit-props-effects-spawners-paths.md §2 完全一致。

### 3.7 paths（SC::CreatePathsFromProps，1024975 行）

- 0x0CAA6841 Int32 对 (start,end) 含端点，奇数个=数据错误返回 1；
- 0x0CAA680D 点 / 0x0CB00ED8 切线（**必须等长**，否则返回 1）；
- 0x0CAA6832 点索引间接寻址；
- 0x0CC8FE7A path entry id（表列优先、KeyArray 兜底，同哈希双通道）→ `IndexFromPathEntryID` → -1 时路径跳过；
- 0x0D9A4E54 scope（缺省 kPathScopeLocal，AcceptedScope 过滤，被拒路径只推进切线索引）；
- 0x0D9AA2BB road connect（缺省 kRoadConnectNone）；
- 三者均 **per-path** 平行数组（下标=路径序号 j，非点序号）；切线索引按 `currTangentIndex += 段点数` 推进。

## 四、openscp 实现对照检视（只读，未改任何文件）

| # | 位置 | 现状 | dump 判定 |
| --- | --- | --- | --- |
| F1 | `lot_unit.rs:57-60` PROP_ID_BASE 注释「离线不可解析；引擎运行时哈希表 FUN_00787870→FUN_0058ec70 解析」 | 注释过时 | **可解析**。§3.4 配方完整：resourceID=脚本资源 instance id；`cUnitBinDrawResources::FillFromProps`（957518）遍历全局资源表，两分支取模型 key。prop 渲染落地前置 = 脚本资源表解析（§3.1–3.3） |
| F2 | `lot_unit.rs:44` `EFFECT_ALWAYS_ZERO = 0x02A9_07B9` | 命名「恒零」 | 语义为 **effectWorlds**（Key 数组），改名并按 Key 列展示 |
| F3 | `lot_unit.rs:437` `effect_id: unit_key(file, EFFECT_REF_IDS, index)` | effect_id 绑 REF_IDS(0x2A907BB) | 引擎视觉挂载用 **EFFECT_IDS(0x2A907B5)**；REF_IDS 是运行时开关 external id。两者都应透出，渲染语义以 0x2A907B5 为准 |
| F4 | `lot_unit.rs:77-86` Spawner 三列 count/count_random/agent（0x0E715928/29、0x0F0E2BF1）+ 注释「生成数量」「agent 引用」 | 语义错位 | 三哈希唯一读取点在 **unit 属性表**上下文（958480 段）= **building variation 基值/变体数/影响 bin**；lot 上不出现（发行包 472 spawner lot 0 命中）。lot spawner 列只有 0x0E1BAC61/62 = 命名位置表 |
| F5 | `lot_unit.rs` 路径仅解析点/切线/索引/pairs | 缺 3 列 | 0x0CC8FE7A（path entry，**路径类型/语义必需**，-1 即跳过）、0x0D9A4E54（scope）、0x0D9AA2BB（road connect）未解析；均为 per-path 平行数组 |
| F6 | `editorGround.ts:59 placementInverse` | 数学正确 | 12 floats 行主序转置进 three 列主序再 invert = 精确的 P⁻¹（P: model→lot，dump §2.2 定谳）。与引擎一致 |
| F7 | `PropertyEditorViewport.vue:798-813` 地面中心覆盖 | `overlayOffset ?? bboxCenter ?? 0`，且偏移经 **A（P 的旋转）** 左乘 | 两处疑点：(a) 引擎栅格映射公式 `uv=(pos−unitOffset)/LotSize+0.5`（migration §42.8）中地面中心=unitOffset（缺省 0），**没有 bbox 中心机制**——bboxCenter fallback 是五样本经验拟合，残差（图书馆 −0.41、EP1 房 +2.89）可能源于此；(b) 偏移若属 lot 系应 postmultiply（`P⁻¹·T(c)`），现 premultiply `T(A·c)·P⁻¹` 把偏移按 P 正向旋转——θ≠0 时方向相反（axis 统计 741:48 支持 0°，故大多数据不触发）；(c) 引擎 readPlacementTransform 强制 scale=1、yaw-only，openscp 未做同样归一化。P⁻¹ 与 unitOffset 疑似同一偏移的两种编码，**叠加使用可能双重计入**（94.3% lot unitOffset=0 故不显） |

### 建议优先级（不动手，仅供目视测试后决策）

1. F4/F2/F3 是纯标注/字段语义修正（属性面板文案），零渲染风险；
2. F5 补三列解析即可让路径显示类型（需先落脚本资源表解析才能解 path entry 语义）；
3. F7 建议用「地面中心 = unitOffset（缺省 0）+ P⁻¹、去掉 bboxCenter fallback」的引擎公式重验五样本，
   并在 θ≠0 的 lot 上核对偏移旋转方向；
4. prop 渲染落地按 §3.1–3.4 配方实现脚本资源表解析（unit → 主表 → resourceGroups → instance 枚举 →
   cGraphicsResource 两分支 → DBPF 查 RW4）。

## 五、复核记录

- unit-props-effects-spawners-paths.md 全部锚点逐条在 952181 / 956948 / 958480 / 437000 / 1024975 行复核通过。
- `.par` 样本为 142 字节 stub（`Original Files/SimCity.par`），属性名反查需真实数据包；
  本文本体不依赖属性名，全部结论以哈希+函数行为锚。
- `PreTransformBy` 复合方向、`Mult` 乘法顺序、Z-up、+Y 临街、P 方向：均经函数体数值推演双验证。
