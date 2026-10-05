# Lot Unit 机制分析：props / effects / spawners / paths（dev 源码取证）

> 来源：`docs/source-code/SimCity-dev-ida.c`（dev beta 全量反编译，带 PDB 符号）。
> 对拍基线：openscp `crates/sc-properties/src/lot_unit.rs`（六类 unit 已解析）。
> 取证日期：2026-10-05。全部结论附函数名与属性哈希，可直接复核。

## 0. 总览

Lot 文件（property-list）中的 unit 列族与引擎消费者的对应关系：

| 列族（openscp 常量） | 引擎消费者 | 状态 |
| --- | --- | --- |
| Light（0x0CAA8F10 族） | 灯光系统 | 已解析，本文不展开 |
| Effect（0x02A907B5 族） | `SP::cModelWorld::AddAttachments` | **链全通**（§2） |
| Decal（0x0D109050 族） | decal 渲染链 | 见 decal-engine-alignment.md |
| Prop（0x0C12EF20 族，14 分箱） | `SC::cUnitBinDraw(Resources/Set)` | **链全通**（§1） |
| PathPoint/Path（0x0CAA680D 族） | `SC::CreatePathsFromProps` | **链全通**（§4） |
| Spawner（0x0E1BAC61/62） | `cUnitModel::mUnitLocations` | **语义修正**（§3） |

## 1. Props（树木/车辆/垃圾桶等摆件）

### 1.1 解析链（dev 符号实证，此前「离线不可解析」的缺口闭合）

```
lot prop 列（PROP_ID_BASE+bin）→ resourceID（脚本资源 id，非 DBPF key）
  → cUnitBinDrawResources::FillFromProps（00788D40 附近）
    遍历 unit 的脚本资源列表（scripts->NumResources / ResourceFromIndex）
    → cGraphicsResource::FillFromProps（952181 行）：
        优先读属性 0x0D8C29C3（Key）→ 显式模型 key
        否则若资源带 0x00F9EFBB / 0x0D897169 / 0x0C36D30D 任一属性
          → 以资源自身 key（props->mKey）为模型 key
        → cGraphicsGame::GetInstancedModelId(gfxKey) → mGfxId
    建表 mResourceIDToGraphics: resourceID → { mGfxId, mPlumpScale, mPlumpInfo, mUnitRefs }
  → 渲染：cGraphicsInstanced 实例化（同模型多实例合批）
```

**对 openscp 的含义**：lot 的 prop id 不是 DBPF 资源的直接 key（这正是当年全包扫描
0 命中的原因），而是**建筑脚本资源表的下标级 id**。离线解析需要补上
「unit → 脚本资源列表」这一层（EcoGame scripts），然后按上表规则取模型 key——
两个分支的产物（显式 key / 资源自 key）都是常规 ResourceMan key，可直接在
DBPF 包中查 RW4 模型。**prop 渲染在 openscp 落地的前置 = 脚本资源表解析**。

### 1.2 Bin 驱动语义（cUnitBinDraw::Update，956948 行）

摆件不是静态全显，而是由**模拟状态**驱动数量：

- 每个 BinDrawSet 绑定一个 EcoResourceBin（`unit->mResourceBins`）；
- 可见槽位数 = `bin.mAmount`；若 `mUseBinPercent`（= lot 列
  PROP_PERCENT_FILL_BASE 0x0C12EF60）则 = `Amount/Capacity × 槽位总数`（向上取整）；
- 数量变化时按 `mResourceSlots` 数组逐个填充/清空（`RandomizeSlot`
  = 0x0C12EF50 控制槽位随机化）；
- **这就是为什么消防局截图里红色桶/路障是「垃圾量摆件」**：垃圾桶、煤堆、
  原油罐等随资源存量显隐；树木类静态 prop 对应不绑资源 bin 的集合（全显）。

### 1.3 槽位结构（cBinDrawSlot，757560 行附近赋值序列）

```
mSlot          : i32      槽位号（lot 列 PROP_SLOT_BASE 0x0C12EF40）
mModelSlot     : u32
mTransform     : cSPTransform（flags + 平移 xyz + scale + 3×3 旋转）
```
lot 的 transform 列（0x0C12EF30）即此结构的序列化；flags==15 时 Unknown=Scale
（openscp 已实现的 C# hack 注释与之一致）。

### 1.4 远景 impostor（cUnitBinDrawResources::UpdateStaticImpostors，0078EFB0）

- 远景 prop 走 `cImpostorRenderer`（persistence class 0x0EB9F833）——树木等
  远距离降级为公告板 impostor，近景才是实例化模型；
- DataView 检查：handler 0x0E556418（数据视图开启时跳过 impostor 更新）。
- PE 近景检视场景可先不实现 impostor。

### 1.5 Plump（cPlumpInfo / mPlumpScale）

每个图形资源可带 PlumpInfo（`SC::CreatePlumpInfo(props)` + FillFromProps）——
prop 的「生长/鼓起」缩放动画参数（建筑升级时摆件随动的缩放插值）。PE 静态
检视可忽略，存档回放时需要。

## 2. Effects（EA::Swarm 粒子）

`SP::cModelWorld::AddAttachments`（437000 行）——**同一哈希族同时存在于
lot unit 与模型自身属性表**（双挂载点）：

| 哈希 | openscp 命名 | 引擎语义（AddAttachments 实证） |
| --- | --- | --- |
| 0x02A907B5 | EFFECT_IDS | Swarm 视觉效果的 Key 数组（`HasVisualEffect(instance, group)`） |
| 0x02A907B6 | EFFECT_TRANSFORMS | 挂载点 transform 数组 |
| 0x02A907BB | EFFECT_REF_IDS | external ID（运行时 `EffectControl → SetEffectsRunning(enable, 1, effectId)` 按此开关） |
| 0x02A907BC | EFFECT_ENABLED | bool 数组：初始激活态 |
| 0x02A907B9 | EFFECT_ALWAYS_ZERO（**命名错误**） | **effectWorlds**：效果所属 world 的 Key 数组 |

模型属性表携带的效果 = 建筑本体粒子（烟囱烟、火光）；lot unit 效果 = 场地
放置粒子（喷泉、火情）。effect id → Swarm 效果资源的解析走
`SP::EffectsManager()`（效果资源为独立格式，非 RW4，需另立解析器；
PE 可先用占位图标/标记渲染挂载点）。

## 3. Spawners —— 语义修正（重要）

### 3.1 0x0E1BAC61/62 不是「spawner」专用

dev 源码（958500 行，`cUnitModel` 属性装配段）把 0x0E1BAC61/62 作为
**键+transform 表**读入 `mUnitLocations`（vector_map<instance, transform>）：
这是**unit 上的命名位置表**——agent 生成点只是其用途之一（门口、车位、
停机点等同构）。openscp 命名 SimsSpawner 方向正确但范围偏窄。

### 3.2 三列「spawner 计数」哈希的真实语义（与 migration §42 冲突，需复核）

dev 源码（同段）对 **unit（非 lot）属性表**的读取：

| 哈希 | migration §42 判读（零售 FUN_00786000） | dev 源码实证 |
| --- | --- | --- |
| 0x0E715928 | spawner 生成数量 | **building variation 基值**（mBuildingVariation） |
| 0x0E715929 | 数量随机化上限 | **变体数取模**（`mBuildingVariation += unitSlot % count`） |
| 0x0F0E2BF1 | spawner agent 引用 | **variation 影响 bin**：resource id → `IndexFromResourceID` → `GlobalBins()->find` → mVariationInfluenceBinIndex（资源存量驱动建筑变体选择） |

注意：哈希语义按属性表作用域区分——lot unit 列与 unit 属性表是不同上下文，
两套判读可能各自成立；但发行包 472 个 spawner lot 对这三列 0 命中 +
dev 源码的明确用法，强烈倾向 **migration §42 的 spawner 判读是误标**，
真实语义 = 建筑变体（building variation）系统。这与着色器侧
`buildingVariation = In.texcoord<t2>.z` 进调色板 V 偏移的链路吻合
（变体 → 调色板行/列偏移 → 同模型多配色）。

### 3.3 周边发现（同函数段）

- 0x0FBA612：模型整体着色（ColorRGBA，mModelColor）；
- building variation 直接基值/变体数读取自 mNonSimProps（脚本写入）；
- agent 实际生成：零售 `FUN_00786000` 链（migration C4，已 dump 未读），
  生成位置取 mUnitLocations —— 即 0x0E1BAC61/62 表。

## 4. Paths（CreatePathsFromProps，1024975 行，全解码）

lot 路径列 → 引擎段：

| 哈希 | 语义 |
| --- | --- |
| 0x0CAA6841 | Int32 对数组 `(start,end)`，每对 = 一条路径（点区间，**含端点**；奇数个 = 数据错误返回 1） |
| 0x0CAA680D | Vector3 点数组（Hermite 控制点） |
| 0x0CB00ED8 | Vector3 切线数组（**必须与点等长**，否则返回 1） |
| 0x0CAA6832 | Int32 索引重映射：路径点经 `pPathPointIndices[i]` 间接寻址 |
| 0x0CC8FE7A | 路径条目 Key 数组（表属性首选，缺则 KeyArray 兜底）：每条路径一个 **path entry id** → `scripts->IndexFromPathEntryID(instance)` 解析为脚本定义的路径条目（门口/车库口/行人道等类型与行为在脚本侧） |
| 0x0D9A4E54 | 路径 scope（Key 数组，缺省 `kPathScopeLocal`；`AcceptedScope` 过滤——本地路径 vs 全局连接器） |
| 0x0D9AA2BB | 道路连接标志（缺省 `kRoadConnectNone`） |

装配：`CreatePath(pathEntryIdx, numPoints, points, scope, connectToRoad,
segmentIndices)` → 在路径网络中生成 segment，agent（行人/车辆）沿段行走。
切线索引随路径推进（`currTangentIndex += 点数`）。

**对 openscp**：路径可视化可直接画（点 + 切线 Hermite 样条 + pair 分段）；
path entry id 的语义标签需要脚本资源表（同 §1.1 的前置）。

## 5. 落地优先级建议（openscp）

1. **脚本资源表解析**（prop/路径条目/variation 的共同前置）：unit →
   EcoGame scripts → resource 列表 → 各资源属性表。这是 prop 渲染落地的
   唯一硬前置。
2. **静态 prop 渲染**：树类（无资源 bin 驱动）按 Amount=Capacity 全显 +
   instanced 渲染；资源驱动型（垃圾桶等）先全显或按容量 50% 占位，存档
   回放接入后再读 bin 实况。
3. **路径可视化**：点/切线/分段直接可画，PE 内即可对拍游戏内行人动线。
4. **Effects**：先渲染挂载点标记；Swarm 效果格式另立解析器（烟囱烟、
   失火、霓虹粒子都走这里，工作量独立评估）。
5. **Spawner/UnitLocations**：PE 已有标记渲染；按 §3.2 修正属性面板文案，
   并把 building variation 三列接入 §9 建筑渲染链（变体 → 调色板偏移）。

## 6. 与 openscp 现有注释的冲突清单

| 位置 | 旧判读 | 新证据 |
| --- | --- | --- |
| lot_unit.rs PROP_ID_BASE 注释「离线不可解析」 | 运行时哈希表解析 | §1.1：脚本资源表 + 0x0D8C29C3/自 key 两分支，离线可解 |
| lot_unit.rs EFFECT_ALWAYS_ZERO 命名 | 恒零列 | §2：effectWorlds（效果 world Key 数组） |
| lot_unit.rs SPAWNER_COUNT/COUNT_RANDOM/AGENT 注释 | spawner 计数/agent | §3.2：building variation 基值/变体数/影响 bin（dev 实证，migration §42 待复核） |
