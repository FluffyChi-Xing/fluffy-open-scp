# Spawner 渲染与组装机制：续研究（名字契约 · Agent 链路 · 外观/动画）

> 取证日期：2026-11-01。承接 `docs/re/props-effects-spawners-paths-engine-flow.md` §3、
> `docs/re/unit-props-effects-spawners-paths.md` §3、`docs/re/spawner-path-agent-er2-analysis-20261024.md`。
> 本文回答四个问题：
> ① 一个建筑怎么"绑定"特定 spawner；② spawner 有没有内部语义/区分方式；
> ③ 市政厅穿西装、工厂穿工装的小人从哪里来；④ 他们怎么渲染、怎么播放动画。
>
> 证据三源：**dev 反编译源码树**（`D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\`，
> 逐函数文件）、**零售包实数据**（`SimCity_Game.package`，通过本轮新增探针）、
> **结构体全表**（`SimCity.unwrapped.exe.dec\...\all_types.h`）。所有 `path:line` 均可复核。

---

## 0. 结论速览

| 问题 | 结论 | 强度 |
| --- | --- | --- |
| 引擎里有 "spawner" 这个对象吗 | **没有**。全 441 个 SC 类、41900 个函数中无任何 Spawner 类；"spawner" 只是 lot 作者对 **UnitLocation 表** 的俗称 | 实证（类目录 + 结构体） |
| spawner 的结构 | `cUnitModel::mUnitLocations` = `vector_map<u32, cSPTransform>`，**只有「名字 id + 局部变换」两个字段**，无类型/角色/数量/agent 字段 | 实证（all_types.h:72143） |
| 建筑怎么绑定 spawner | **不是"建筑→spawner"**，而是「引用方（vignette/UI/相机）按 **名字 id** 查这张表」。绑定的全部内容是 **名字** | 实证（GetWorldLocation / BuildUnitJSValue） |
| spawner 有语义吗 | **语义只存在于名字里**。名字是 FNV-1(lower) 哈希；名字本身由作者定义，引擎不解释 | 实证（哈希复核 + 硬编码 id） |
| 小人（可见角色）从哪来 | **不是 spawner**，是模拟层 **transport entry**（agent 类型）沿 **path/sink 网络** 生成 | 实证（RunRule→CreateTransaction→TransportEntry） |
| 西装 / 工装怎么来的 | 每个 transport entry 的 **non-sim 属性 0x0D8C29C3** → `0x40E02D00` 组的外观表 → 每行 `(body, head, outfit)` 区间。**角色 = outfit 调色板索引** | 实证（179 个外观定义实测） |
| 动画 | 小人无蒙皮：**3 体型 × 2 姿态（idle/walk）+ 20 头 + 8 方位离屏图集**；时间维动画在图集里，同类共享相位 | 实证（见 `_spawner-animation-evidence-20261101.md`） |
| 点击里的"打理市政府整洁/巡逻中" | 来自 **transport entry 的 non-sim 文案列**，不是 spawner、也不是建筑 | 强证据（代码链密闭，文案 id 未落本地表） |

**一句话**：spawner = 命名锚点（纯位置，给演出层用）；"谁、穿什么、在干嘛" 全部由 **agent/transport entry** 决定。
建筑与 spawner 之间唯一的连接是 **一个名字哈希**；建筑与人之间唯一的连接是 **模拟规则里的 transport id**。二者互不相干。

---

## 1. 先纠正名词：引擎里没有 spawner

### 1.1 全类目录无 Spawner

`src\SC\` 下 441 个类目录、`src\GB\` 下 63 个类目录里没有 `cSpawner` / `cSpawn`。
全树文件名匹配 `*Spawn*` 的只有 11 个，且全部是"生成实体"的动作函数：

```
SC\cTransportVehicle\SpawnWaitingVehicles.c     车辆排队出车
SC\cTransportRail\SpawnWaitingTrains.c          列车排队出车
SC\cDecalData\SpawnDecalInstance.c              贴花实例
EA\Swarm\cMetaParticlesEffect\CullInactiveSpawnedEffects.c
GB\cUpdateManager\SetSpawnProcessName.c         进程名（无关）
EA\Process\Spawn.c / EA\StdC\Spawn.c            进程启动（无关）
```

### 1.2 数据侧只有两列、两个字段

`SC\cUnitModel\FillFromProps.c:399-428`（逐字）：

```c
kUnitLocationTable[0].mData = &unitLocationIDs;
kUnitLocationTable[0].mPropID = 236694625;   // 0x0E1BAC61
kUnitLocationTable[0].mTypeID = 32;          // Key
kUnitLocationTable[1].mPropID = 236694626;   // 0x0E1BAC62
kUnitLocationTable[1].mTypeID = 56;          // Transform
...
PropertiesAsTable = SP::GetPropertiesAsTable(props: v56, tablePropInfo: kUnitLocationTable);
...
v60 = ...vector_map<unsigned int,cSPTransform>::operator[](this: &this->mUnitLocations,
                                                            k: &unitLocationIDs[v57].mInstance);
EA::Swarm::cTransform::operator=(this: (EA::Swarm::cTransform *)v60, xform: shaderStateb);
```

结构体（`all_types.h:72114-72148`）：

```c
struct __cppobj SC::cUnitModel {
  ...
  eastl::vector_map<unsigned int,cSPTransform,...> mUnitLocations;   // 72143
  ...
};
```

注意来源是 `v8->mNonSimProps`（**non-sim**，即"表现层属性"），不是 `mProps`（模拟属性）。
这条归属本身就是结论的一半：**这列数据只为表现层存在，模拟层根本不读它。**

### 1.3 引擎侧的全部消费者（穷举）

全树 `mUnitLocations` / `GetUnitLocation*` 的引用点只有：

| 文件 | 作用 |
| --- | --- |
| `SC\cUnitModel\FillFromProps.c` | 装配 |
| `SC\cUnitModel\GetUnitLocation.c` / `GetUnitLocationLocal.c` | 查询（二分查 map，输出 = 模型变换 ∘ 局部变换） |
| `SC\cVignette\GetWorldLocation.c` | **演出（vignette）取点** |
| `SC\cVignette\DestinationIsAnActor.c` | 演出目标判定 |
| `anonymous\BuildUnitJSValue.c` | **UI**（语音气泡偏移） |
| `anonymous\ExecuteZoomToSpeechBalloonCommand.c` | **UI**（镜头飞到气泡） |
| `SC\cCameraControllerBase\RegisterEvent.c`、`SC\cVignette\ActivateVignetteInternal.c`、`SC\cVignette\cVignette.c`、`SC\cVignette\FillFromProps.c` | 相机 / 演出 |

**`src\GB\`（模拟命名空间）里一条都没有。** 这是"spawner 不生成 agent"的硬证据。

---

## 2. 建筑怎么"绑定" spawner：靠名字，不靠指针

### 2.1 名字 = FNV-1(lower) 哈希（本轮复核通过）

`GI_Simm_politician_ma` → `0x713A2388`、`GI_Simm_polofficer_ma` → `0x04BB64BD`，
与 `database_main.s3db` 的 `Instances` 表逐字节一致；算法为 **FNV-1**（先乘后异或，
初值 `0x811C9DC5`，字母折叠为小写），与 `SPIDFromName` / `GB` 侧 `FNV1_String8(..., kCharCaseLower)` 同式。

```python
h = 0x811C9DC5
for ch in name.lower().encode():
    h = ((h * 0x01000193) & 0xffffffff) ^ ch
```

### 2.2 引擎硬编码的名字 id（说明"名字"是契约）

| 位置 | 代码 | 含义 |
| --- | --- | --- |
| `anonymous\BuildUnitJSValue.c:128` | `GetUnitLocationLocal(unitLocationID: 0x141EED53u, ...)` | 该 unit 的 **UI 特效/气泡锚点**；查不到再退 `0xC74A075`（非 sim 属性里的中心点） |
| `anonymous\ExecuteZoomToSpeechBalloonCommand.c:77` | 同上 `0x141EED53` | 镜头缩放到气泡时用同名锚点 |
| `SC\cVignette\GetWorldLocation.c:160-177` | `case 0x141EED53u: case 0x9901C8F9:` → `GetUnitLocation(unitLocationID: v11, ...)` | 演出层把"位置空间 id"解析到本建筑锚点 |
| `SC\cVignette\GetWorldLocation.c:293-308` | 兜底同款查询 | 见 2.3 的解析顺序 |

也就是说：引擎在代码里写死两个"必须存在"的锚点名。资产里没有这两个名字的建筑，
表现层会退化成"用非 sim 属性里的中心点/世界原点"。

### 2.3 演出层如何把「位置空间 id」落到建筑锚点

`SC\cVignette\GetWorldLocation.c` 的解析顺序（逐字归纳）：

1. `case 0x9B8E862F` → 直接用传入的局部坐标；
2. `case 0xBAFEA257` → lot 前沿中心（`cUnitLotEntry::GetUnitLotFrontCenter`）；
3. `case 0x141EED53 / 0x9901C8F9` → **查本建筑 UnitLocation**；查不到退 `0xC74A075` 中心点，再退世界变换；
4. `case 0x4DBB220F` / `0x173B3B8C` → 直接套用建筑/演出世界变换；
5. **default**：先查本次演出的 `mSinkLocations`（sink 位置），命中就用 sink 的旋转/位置；
   仍未命中 → **再查本建筑 UnitLocation（:293-308）** → 最后退演出/建筑变换。

结论：**"spawner 名字" 是演出资产与建筑资产之间的公共命名空间。**
演出资产（vignette）在自己的属性里写 `startPoint` / `endPoint` 名字 id，建筑在自己的 lot 里
提供同名锚点与局部变换，运行时按名对接。这就是所谓"绑定"的全部机制。

### 2.4 演出演员的锚点字段（数据侧）

`SC\cVignetteGame\FillEntryFromProps.c:452-496`（逐字）：

```c
kActorTable[0].mPropID = 224696649;  // 0x0D649949  actorIDs
kActorTable[1].mPropID = 221153889;  // 0x0D2E8A61  modelNames（外观 key）
kActorTable[2].mPropID = 225393633;  // 0x0D6F3BE1  modelScales
kActorTable[3].mPropID = 221759571;  // 0x0D37C853  vignetteType
kActorTable[4].mPropID = 223741520;  // 0x0D560650  speeds
kActorTable[5].mPropID = 226501141;  // 0x0D802215  startPoint
kActorTable[6].mPropID = 228478318;  // 0x0D9E4D6E  endPoint
kActorTable[7].mPropID = 230729104;  // 0x0DC0A590  resourceBinds
kActorTable[8].mPropID = 230729105;  // 0x0DC0A591  resourceBindPercents
...
if ( startPoint != nullptr )  v33->mActorSpawnLocation  = startPoint[v30].mInstance;
if ( endPoint != nullptr )    v33->mActorDestroyLocation = endPoint[v30].mInstance;
```

演员结构体（`all_types.h` `cVignetteActorEntryInfo`）就有 `mActorSpawnLocation`、
`mActorDestroyLocation`、`mVignetteModelKey`、`mVignetteType` ——
**演出演员自带外观 key**，与建筑的 spawner 名字表正交。

### 2.5 实数据：零售包里的 spawner 规模与名字分布

本轮新探针 `probes\lot\spawner_names.rs` 扫 `SimCity_Game.package`：

```
lots_with_spawners = 472
total_spawners     = 7761
distinct name ids  = 257
```

出现频次最高的 10 个名字 id（几乎每个 lot 都有 ⇒ 模板化的通用锚点）：

```
0x980F070E x455   0x141EED53 x454   0x125D19FA x453   0x684C5FCA x451
0x700770E0 x451   0x10B3AC30 x443   0x1909178F x443   0x33A4F764 x443
0xAAB85CBB x443   0x3B8ADFE5 x373（另有 0x3B8ADFE0/E2/E3，同族）
```

`0x141EED53` 正是 2.2 里硬编码的那个 UI 锚点，出现在 454/472 个 lot 上；
反向说明"每个建筑至少有 10 个左右的标准锚点"，而**每个 lot 平均 16 个锚点**（7761/472）。

名字 id 用 exe 字符串（117k 条）+ s3db 名字（11k 条）+ 组合词共 155k 候选做过穷举反查：
**只有 1/257 命中**。结论：名字是资产作者自定义的短串（非引擎词表），
离线无法稳定还原成可读文本——这也解释了为什么"spawner 看起来没有语义"。

### 2.6 名字 id 确实被别的资产引用（跨资产契约的实证）

`probes\lot\spawner_refscan.rs` 拿上面 13 个高频名字 id 反查整包属性，命中列全部在
**group `0x40E1E800`**（演出/交互属性组）上：

```
col 0x0D801143（数组）
col 0x0D802215  = startPoint（见 2.4；样本里出现到 idx=40/41，说明条目很多）
col 0x0D9E4D6E  = endPoint  （见 2.4）
col 0x0D36410D（数组）
```

即：**lot 里那条"spawner"记录的名字，会以 id 形式出现在另一类资产的
"位置空间/起点/终点"列里**。这就是"建筑绑定 spawner"的真实交付物：
一份名字约定，而不是一个可配置的对象引用。

---

## 3. 小人从哪来：模拟层 Agent（与 spawner 无关）

### 3.1 规则（ER2）→ 事务（transaction）→ transport entry

`GB::cEcoUnitRule`（`all_types.h:42017-42046`）：

```c
struct __cppobj GB::cEcoUnitRule : GB::cEcoRuleBase {
  ...
  GB::cStoredArray<GB::cAgentTest> mAgentTests;
  eastl::vector<GB::cTransportOp,...> mTransportOps;   // { mID, cSPVector2 mLife }
  unsigned int mTransportID;
  unsigned int mHandlerID;
  unsigned int mUnitID;
  cSPVector3   mUnitCreateOffset;
  unsigned int mGraphicsState;
};
```

`GB\cTransportOp`（`all_types.h:42162-42166`）只有 `{ unsigned int mID; cSPVector2 mLife; }`。

跑规则 `GB\cEcoUnit\RunRule.c:139-155`：

```c
v18 = rule->mTransportID != 0 && (rule->mFlags.mWord[0] & 0x1000000) == 0;
...
v20 = mpObject->IndexFromTransportEntryID(this: mpObject, a2: rule->mTransportID);
...
v8->mBoundTransaction = GB::cEcoGame::CreateTransaction(this: game, sourceUnitSlot: unitSlot, ruleIndex, rule);
```

建事务 `GB\cEcoGame\CreateTransaction.c:66-83`：

```c
mTransportID = rule->mTransportID;
v7->mEntryIndex = this->mScripts.mpObject->IndexFromTransportEntryID_2(this: ..., a2: mTransportID, ...);
...
v13 = this->mScripts.mpObject->TransportEntryFromIndex(this: ..., a2: mEntryIndex);
```

`GB::cEcoTransportEntry`（`all_types.h:42610-42621`）：

```c
const struct __cppobj GB::cEcoTransportEntry {
  EA::AutoRefCount<SP::cPropertyList> mProps;         // +
  EA::AutoRefCount<SP::cPropertyList> mNonSimProps;   // +4
  unsigned int mID;                                   // +8
  GB::cEcoResourceBins mResourceBins;
  float mSpeed;  float mSpeedVary;
  unsigned int mHandlerID;
  bool mAutoDeliver;
  eastl::bitset<256> mActivePathEntries;
};
```

装配 `GB\cEcoTransportEntry\FillFromProps.c`：
`mID` = 属性表 key instance（:31）；`mSpeed` = `0x0A2F86E4`（:60-70）；
`mHandlerID` = Key `0x0A31A734`（:77）；path filter = `0x0EB22775`（:89）。

**所以：建筑要"出人"，是在自己的规则里写 `mTransportID` + `mTransportOps`，
指向一条 transport entry；transport entry 再指定 handler（行人/车辆/轨道）、速度、可用路径段。**
这一条链上没有任何"位置"字段——**位置来自路径网络**。

### 3.2 位置来自 sink / path，不来自 UnitLocation

`SC\cTransportVehicle\SpawnWaitingVehicles.c:110-114`：

```c
UnitPath = SC::cRoadManager::GetUnitPath(this: this->mRdMgr,
             unitSlot: v12->mUnitSlot, unitEntryIndex: v12->mUnitEntryIndex,
             graphicsState: v12->mUnitGraphicsState);
```

`SC\cTransportVehicle\AddAgentToPathPool`、`cTransportPedestrian\GetPathPoolFromSinkEntryIndex`
等一整族函数以 **sink entry**（`cEcoSinkEntry`，脚本表实体）为锚，
`cEcoUnitEntry` 自带 `mSinks`（`all_types.h:42520`）。
对照第 2 节的 11 个消费者清单：**UnitLocation 不在其中**。

> 也就是说：你看到的小人在门口"出现"，是因为他的 agent 从 sink 进入路径段后
> 第一次被渲染，位置由路径插值给出；spawner 只是个碰巧被摆在同一位置的点。

---

## 4. 外观（西装 vs 工装）：per transport entry 的数据

### 4.1 装配：transport entry 的 non-sim 属性 → 外观表

`SC\cGraphicsGame\FillTransportFromProps.c:44-58`（逐字）：

```c
if ( SP::GetPropertyAsKey(list: v3->mNonSimProps.mpObject, id: 0xD8C29C3u, result: &gfxKey) != 0 ) {
    InstancedModelId = SC::cGraphicsInstancedModels::GetInstancedModelId(this: &this->mInstancedModels, key: &gfxKey);
    v3->mInstancedModelID = InstancedModelId;
    return;
}
if ( v3->mNonSimProps.mpObject->HasProperty(..., 16379835u)
  || v3->mNonSimProps.mpObject->HasProperty(..., 227111273u)
  || v3->mNonSimProps.mpObject->HasProperty(..., 204919565u) )   // 204919565 = 0x0C36D30D
{
    InstancedModelId = SC::cGraphicsInstancedModels::GetInstancedModelId(
                         this: &this->mInstancedModels, key: &v3->mNonSimProps.mpObject->mKey);
}
```

两条路：**显式 `0x0D8C29C3` 指向外观资源**，或**transport entry 自己就是外观定义**
（带 `0x0C36D30D` 时用自己的 key）。`GetInstancedModelId` 实际是
`GetPropertyList(key.instance, key.group)`（属性表当资源加载），然后按内容分派到
`cGraphicsInstancedSim`（typeId `227112196`，见 `GetSimPaletteDataFromTransaction.c:35`）。

### 4.2 外观表的 8 列

`SC\cGraphicsInstanced\FillFromProps.c:255-281`（逐字）：

```c
tableInfo[0].mPropID = 213722562;  // 0x0CBD25C2  bodyMin
tableInfo[1].mPropID = 213722565;  // 0x0CBD25C5  bodyMax
tableInfo[2].mPropID = 213722561;  // 0x0CBD25C1  headMin
tableInfo[3].mPropID = 213722564;  // 0x0CBD25C4  headMax
tableInfo[4].mPropID = 213722563;  // 0x0CBD25C3  outfitMin
tableInfo[5].mPropID = 213722566;  // 0x0CBD25C6  outfitMax
tableInfo[6].mPropID = 213722570;  // 0x0CBD25CA  scaleMin
tableInfo[7].mPropID = 213722571;  // 0x0CBD25CB  scaleMax
PropertiesAsTable = SP::GetPropertiesAsTable(props: v13, tablePropInfo: tableInfo);
```

每行 = 一个"外观选项"：`(bodyMin..bodyMax, headMin..headMax, outfitMin..outfitMax, scaleMin..scaleMax)`。

### 4.3 逐实例抽取：`GetSimData`

`SC\cGraphicsInstancedSim\GetSimData.c:44-107`：

```c
v8 = (randomBits >> 12) % mNumSimOptions;          // 先选"行"
... 行内：v14 = v8 % mNumBodies; v13 = v8 / mNumBodies;
    mBody         = v14 + mpBegin->mBody;
    mOutfitPalette= v13 / mNumHeads + mpBegin->mOutfitPalette;
    mHead         = v13 % mNumHeads + mpBegin->mHead;
...
*modelFlag = (1LL << (head + 9)) | (1LL << (body + (speed² > kSimIdleSpeed2) + 2*body));
*simInfo   = (outfit | ((unsigned __int8)randomBits << 8)) << 8;
```

即：**外观 = 行（随机）+ body/head/outfit 三个索引（随机）**；
`outfit` 是**调色板索引**（贴图 U 坐标），不是模型差异。

### 4.4 实数据：179 个外观定义，角色 = 固定 outfit 索引

`probes\property\simm_appearance_dump.rs` 扫 `SimCity_Game.package` 的
`group 0x40E02D00`，得到 **179 个外观定义**。命名角色（`GI_Simm_*`，s3db 40 条）实测：

| 定义 | body | head | outfit | rows | 语义 |
| --- | --- | --- | --- | --- | --- |
| `GI_Simm_politician_ma` 0x713A2388 | 0..0 | 19..19 | **92** | 1 | 西装男（市政/政客） |
| `GI_Simm_politician_fa` 0x763A2BA9 | 1..1 | 3..3 | **190** | 1 | 西装女 |
| `GI_Simm_business_fa` 0x44AE5F25 | 1..1 | 3..3 | **185** | 1 | 商务装 |
| `GI_Simm_polofficer_ma` 0x04BB64BD | 0..0 | 16..16 | **197** | 1 | 警服 |
| `GI_Simm_sanitation_ma` 0xE56E8D3E | 0..0 | 14..14 | **205** | 1 | 环卫/工装 |
| `GI_Simm_sanitation_fa` 0xEE6E9BEB | 1..1 | 14..14 | **205** | 1 | 环卫/工装（女） |
| `GI_Simm_powerforeman_fa` 0x1352F41A | 1..1 | 7..7 | **168** | 1 | 电力工头 |
| `GI_Simm_techforeman_ma` 0x7042F786 | 0..0 | 0..0 | **69** | 1 | 技术工头 |
| `GI_Simm_resident_ma` 0x06BBC54E | 0..0 | 0..0 | 17 | 1 | 居民 |
| `GI_Simm_librarian_ma` 0x638537D0 | 0..0 | 0..0 | 79 | 1 | 图书管理员 |
| `GI_Simm_lawyer_ma` 0xC912BE3A | 0..0 | 0..0 | 73 | 1 | 律师 |
| `GI_Simm_advisor_ma` 0xD5F6295C | 0..0 | 19..19 | 92 | 1 | 顾问（同政客） |

**这直接回答"西装 vs 消防员服"：不是两套模型，是同一套 body/head 模型 + 不同 outfit 调色板索引。**
西装 = outfit 92 / 185 / 190；工装（消防员观感）= outfit 205（sanitation）/ 168 / 69。

### 4.5 群众多样性来自多行表，角色来自单行表

264 条 transport entry 中有 **116 条**带 `0x0D8C29C3`，共指向 **63 个不同外观定义**。
其中：

* **单行**（1 row）= 定装角色，如 0x524FEB31（body0/head14/outfit226）、0xDD83304D（outfit201）；
* **多行** = 群众随机，例如
  `0xE5F62269` rows=14（outfit 1..31）、`0xE2511988` rows=8（outfit 211..）、
  `0xB4D0B60B` rows=8、`0xAC81906F` rows=4（outfit 249..）、`0xDF4F4EBF` rows=11。

> 复刻结论：**穿着的"语义"是数据里的一行 `(body, head, outfit)`**，
> 由"这条 transport entry 用哪个外观定义"决定；再由 `randomBits` 决定同一定义内的具体一行。

### 4.6 第三条路径：资产直接列举"该出现哪些角色"

`spawner_refscan` 把 `GI_Simm_*` 的角色 id 拿去扫别包，发现：

```
SimCity_Game.package   : 0 命中
SimCity_App.package    : 0 命中
SimCityDataEP1.package : 0 命中
SimCity_DLC0.package   : 命中列 0x0DB5452F ×7
    g=0x46F0E800 i=0x225FD826  col 0x0DB5452F idx=80/81 → 0x713A2388 (GI_Simm_politician_ma)
    g=0x58F0E800 i=0xABC9AF43  col 0x0DB5452F idx=3    → 0x7042F786 (GI_Simm_techforeman_ma)
```

即：除 ①transport entry 的外观定义、②演出演员的 `modelNames` 之外，还有
**③"角色 id 数组"列 `0x0DB5452F`** —— 某个资产可以显式列出它要用的 `GI_Simm_*` 角色集合。
这条列在基础 `SimCity_Game.package` 里没有出现，只在 DLC 包出现，
说明它是**按 DLC 资产启用的可选路径**，不是建筑通用路径。列语义待定（见 §10）。

---

## 5. 动画：2 姿态 + 8 方位 + 离屏图集

（完整取证见 `docs/re/_spawner-animation-evidence-20261101.md`，本节只摘结论）

1. **非蒙皮**。`GetSimData` 常态只置两个 bit：`1<<(head+9)`（头部件）与
   `1<<(3*body + walk)`（身体部件）。bit 0..8 = 3 体型 × 3 姿态槽，bit 9.. = 头。
   数据侧自洽：小人 impostor class `22706EFA` 共 29 part = 9 body + 20 head。
2. **走/站是"换部件"，不是插值**。`kSimIdleSpeed2 = 0.010000001`（比较对象是速度平方和
   ⇒ 阈值 ≈0.1 单位/秒）；超过就用 `walk` 槽位（`3*body+1`），否则 `3*body+0`。
3. **时间维动画在图集里**。`cImpostorRenderer::Update` 每帧把每个部件骨骼动画推进
   `dt*mAnimSpeed`，再按 `mNumAngles`（小人=8）把部件重绘进 512×512 图集；
   场景里只贴公告板 ⇒ **同类小人共享同一动画相位**，逐实例只有"姿态槽 + 方位帧"。
4. **朝向**：交通层 `AlignYToDirWithZUp` 造变换，渲染只取 `mRotation.yAxis`，
   顶点阶段在 `mCosAngleTable` 里 3 步二分出方位帧（表用 sin/cos 建、查询用 y/x，
   等价"0 号方位朝 +X"——复刻时极易搞反）。
5. **实例色**：`simInfo = (outfit | (randomBits & 0xFF) << 8) << 8`
   ⇒ **R = skin、G = outfit、B = 0、A = 0**；着色器用 x/y 当两张调色板的 U，
   **A 恒 0**（透明度只能来自图集 alpha）。
6. **建筑**才是真关键帧：`cUnitModel::UpdateModelAnimations` 用资源 bin 比例
   驱动 `MoveToTime(Relative)` + `cChoreographyAnimations::GetAnimationRange`。

---

## 6. 点击时的"打理市政府整洁 / 巡逻中"从哪来

`SC\cQueryHelper\OnAgentSelected.c:112-208`（逐字归纳）：

```c
happiness = this->mGameScripts->TransportEntryFromIndex(this: this->mGameScripts, a2: *(_DWORD *)(v22 + 52));
transactionSlot = 782826392;                                  // 0x2EA8FB98 缺省
SP::GetPropertyAsKeyInstance(list: *(SP::cPropertyList **)(happiness + 4), id: 0xE27B9AFu, ...);
SC::SetJSValue(num: transactionSlot, value: ...);
v23->SetProperty(v23, L"transportType", v25);                 // ← 传给 UI 的键名
...
if ( SP::GetPropertyAsKeyInstance(list: ..., id: 0xE4F02A9u, ...) != 0 ) goto LABEL_28;
// 按资源存量分支：
if ( (_BYTE)a6 != 0 ) {
    if ( mAmount > 0 && GetPropertyAsKeyInstance(..., id: 0xECB2216u, ...) 
      || v32    > 0 && GetPropertyAsKeyInstance(..., id: 0xECB2204u, ...) ) goto LABEL_28;
    PropertyAsKeyInstance = GetPropertyAsKeyInstance(..., id: 0xE4F02CEu, ...);
} else {
    if ( mAmount > 0 && GetPropertyAsKeyInstance(..., id: 0xECB21EBu, ...)
      || v32    > 0 && GetPropertyAsKeyInstance(..., id: 0xECB21EAu, ...) ) goto LABEL_28;
    PropertyAsKeyInstance = GetPropertyAsKeyInstance(..., id: 0xE4F02C2u, ...);
}
...
this->mGameUI->PostGameEvent(this: this->mGameUI, a2: 236428978u, a3: &eventData);   // 交给 UI(JS)
```

要点：

* `happiness + 4` = `cEcoTransportEntry::mNonSimProps`（见 3.1 结构体布局）——
  **文案列挂在 transport entry 的 non-sim 属性上**；
* 事件名 `236428978`（= `0x0E179EB2`）、键名 `transportType` 都交给 EAWebKit 的 JS UI；
* 文案按「资源是否在身（`mAmount`/`v32`）」在两个 Key 之间切换，
  即同一 agent 会因携带/需求状态显示不同活动文本。

### 6.1 实数据：哪些 entry 带这些文案列

`probes\property\simm_transport_dump.rs`（264 条 transport entry，92 种列）：

```
0x0D8C29C3  外观 key            116 条
0x0E27B9AF  transportType        79 条（车辆=车型 id，如 Bus T1/T2 同为 714721605）
0x0E4F02C2 / 0x0E4F02CE / 0x0ECB2216 / 0x0ECB21EB   各 3 条
```

带活动文案的 3 条正好是**行人**（同一外观 key，三个变体）：

```
entry g=0x40E1C600 i=0x396721D4 / 0x396721D6 / 0x396721D7
  Parent            0x00B2CCCB → 0xF43CA7C0 @48E1C600（Agent 家族根组）
  gfx key           0x0D8C29C3 → 0x524FEB31 @0x40E02D00（body0/head14/outfit226）
  transportType     —（行人没有 0x0E27B9AF）
  活动文案          0x0E4F02C2 → 0xD606A55F
                    0x0E4F02CE → 0xD606A554
                    0x0ECB21EB → 0xD27FACB3
                    0x0ECB2216 → 0xD27FACB8
                    0x0ECB21EA → 0xD606A55F（缺省分支同 A）
                    0x0ECB2204 → 0xD606A554
```

### 6.2 尚未闭合的一环（诚实标注）

把 `0xD606A55F / 0xD606A554 / 0xD27FACB3 / 0xD27FACB8` 拿去
`Locale\zh-tw\Data.package`（356 张字符串表）与 `Locale\en-us` 全部表查，
以及 `database_main.s3db` 的 `Instances` 表，**均未命中**。
⇒ 这四个 id 不是本构建的本地化文本 id，最可能是 **UI 侧（JS/WebKit）映射表的键**
（与 `PostGameEvent 236428978` + `transportType` 的 JS 处理路径一致）。
要拿到"打理市政府整洁"这类具体字符串，下一步应在 `SimCity_App.package` 的 UI JS
里追 `transportType` 的消费代码，而不是继续在 locale 里找。

---

## 7. 资产编辑器：把"角色语义"做成可编排、可校验的层

> 本节回答产品问题：**编辑器里用户只看到蓝色占位锥，怎么保证"这是大学的白大褂、
> 那是写字楼的西装"？**
> 结论：**不要试图给 spawner 加语义**（引擎不认），而是把"角色"从锚点上**拆出来**，
> 做成两个显式概念 + 一个校验器。

### 7.1 先承认一个结构性事实：锚点天然表达不了角色

* `mUnitLocations` 只有 `(名字, 变换)`；`GetSimData` 只从 **transport entry 的外观表** 取外观；
* 二者之间**没有字段级连接**；
* 所以"把这个锥子改成一个白大褂"在数据上无处安放——除非我们引入**第三方语义层**，
  或者落到引擎本来就支持的"演出演员（vignette actor）"上。

编辑器现在的做法（每个 spawner 随机撒 2~5 个示意小人）恰恰是把"示意"当成了"语义"，
用户看到的是噪声而不是约定。

### 7.2 拆成三个概念（对应三份数据）

| 概念 | 数据来源 | 引擎里生效吗 | 编辑器职责 |
| --- | --- | --- | --- |
| **锚点 Anchor** | `0x0E1BAC61/62`（名字 + 变换） | 生效（演出/UI/相机按名取点） | 摆放、命名、显示被谁引用 |
| **在场角色 Cast** | vignette actor：`modelNames`(0x0D2E8A61)、`startPoint`(0x0D802215)、`vignetteType`(0x0D37C853)、`speeds`(0x0D560650)、资源绑定 | 生效（演出触发时在该锚点生成该角色） | 在锚点上挂"角色卡"：角色 + 场景类型 + 速度 |
| **人群档 Crowd** | transport entry 的 `0x0D8C29C3` → `0x40E02D00` 外观表 | 生效（但**只能经模拟规则 `mTransportID` 触达**） | 给建筑选"员工/访客外观档"，并**校验规则是否真的指向它** |

**用户要的两件事，分属后两行：**

* "大学门口站几个白大褂" → **Cast**（锚点绑角色），逐点可控、可预览、可写。
* "这栋写字楼上班的人穿西装" → **Crowd**（外观档），是环境级效果，
  必须同时确认该建筑规则里 `mTransportID` 指向了对应 transport entry，否则**不生效**。

### 7.3 角色库：让 outfit 索引变成"人话 + 缩略图"

外观 = `(body, head, outfit)`。其中：

* `body` 3 型、`head` 20 型 —— **真模型在本地**（RW4，PE 已有 `read_sim_parts` + `simAssets.ts` 加载链）；
* `outfit` 是**调色板索引**（0..255），决定衣服配色/花纹。

已实测的 40 个命名角色（`GI_Simm_*`）可直接做"角色卡"：
politician（西装）/ business（商务）/ polofficer（警服）/ sanitation（工装）/
powerforeman / techforeman / librarian / lawyer / advisor / airtrafficcontrol /
casino baron / drVu / maxisman / resident（群众，多套）。

**尚缺一步**：outfit 索引 ↔ 实际颜色。建议做一个离线"服装色板"任务：
用 `simm_model_probe` 取 3 body + 20 head 的 slot0 纹理，按 `simParticleLightPS` 口径
（R=AO、G=palV、A=alpha，再用实例 `In.color.x/y` 查 body/clothes 调色板）
把 0..255 号 outfit 渲成一条色带，**一次性**产出"第 92 号 = 深色西装 / 第 205 号 = 橙黄工装"
的对照表。有了它，角色库才不是一堆数字。

> 在这张表做出来之前，编辑器只能给"命名角色"（40 个，语义已由名字给出）可信标签，
> 其余 139 个匿名定义为"未知外观"，不得擅自起名——这就是"保证语义不出错"的第一条纪律。

### 7.4 锚点 UI：从蓝锥到"角色卡"

建议的单元表示（替换现有蓝锥 + 随机小人）：

```
[锥体/圆环] + [名字标签] + [徽章]
   名字标签：可见名（未知名显示 0xXXXXXXXX）
   徽章：①锚点（灰） ②被 UI 引用（0x141EED53 → "气泡/镜头锚点"，蓝）
        ③被演出引用（被 N 个 vignette 引用，橙）
        ④已挂角色卡（显示角色缩略图，绿）
```

* 只有第 ④ 种在预览时生成小人；②③ 只画标记 + 引用计数；
* 第 ④ 种生成的小人必须打 **"示意（sample）"** 水印/描边，避免用户再把它当人数；
* 预览采样必须调用**与运行期同式的 `GetSimData`**（`row=(randomBits>>12)%N`、
  行内 `body/head/outfit` 取模），seed 由 `(lot, anchorName, instanceSlot)` 派生，
  保证"预览=运行时"，且改一次参数只影响该锚点。

### 7.5 校验器（这才是"保证语义正确"的正解）

编辑器的价值不在"能放人"，而在**能证明放对了**。建议五条硬校验：

1. **名字唯一性**：同一 lot 内锚点名字 id 重复 → 错误（引擎按 `vector_map` 取，重复会互相覆盖）。
2. **引用可解析**：锚点被演出/UI 引用（引用计数 > 0）但名字在该 lot **不存在** → 错误
   （运行期会静默退化成世界原点，最容易被忽略的 bug）。
3. **Cast 兼容性**：角色卡选的 `GI_Simm_*` 与场景 `vignetteType` / 资源绑定不兼容 → 警告
   （数据可从 `FillEntryFromProps` 的 actor 表逐列校验）。
4. **Crowd 生效性**：选了"人群档"但该建筑的规则没有指向对应 `mTransportID` → **必须报红**
   （否则用户在编辑器里看得到、进游戏看不到，这是最伤信任的一类问题）。
5. **预览/运行一致性**：预览生成器与运行期采样函数共用同一实现（单测锁定 `GetSimData` 等价）。

### 7.6 能力边界（必须对用户诚实标注）

| 想做 | 今天能不能 | 卡在哪 |
| --- | --- | --- |
| 编辑器里预览"白大褂/西装"，并保存为资产自己的语义层 | **能** | 纯我们自己的数据 + 渲染，不依赖引擎 |
| 让**游戏内**在指定锚点出现指定角色 | **能，但走 Cast（定义体定位未闭合）** | actor 列（`0x0D802215/0x0D9E4D6E/0x0D2E8A61`）在 `0x40E1E800` 组的 **237 个属性文件**里实测存在；但 lot 顶层列 `0x0D2E8A55` 里的 279 个 id **与本包这 237 个定义零交集**，且这些 id 在整包其它任何列里**都没有引用**（refscan：只命中 270 个 lot 自己的 `0x0D2E8A55`）。⇒ 定义体不在 `SimCity_Game.package`，或按 `cEcoGameScripts::BundlesForID` 的名字表解析。**这是落地 Cast 的第一优先任务**（下一轮：扫 `SimCity_App`/DLC 的实例，或对 `cVignetteGame::StartGame` 取表处下 hook） |
| 让整栋楼的员工换西装 | **有条件能** | 必须改该建筑规则里的 `mTransportID`（ER2 二进制，`0x08068AEB/AEC`）；只改外观 key 不动规则 = 不生效 |
| 新增一种全新角色（不在 40 个里） | **需要实验** | 外观定义是属性表，理论上可新增；但 outfit→贴图的对应关系未解码，新角色可能显示成噪点 |

### 7.7 落地顺序建议（最小可用 → 完整）

1. **第一步（纯编辑器，无引擎风险）**：锚点 inspector（名字/变换/引用计数）+ 角色库面板
   （40 个命名角色，缩略图用现有 `read_sim_parts`）+ 预览按 `GetSimData` 采样 + "示意"标记。
   语义层写进我们自己的 `asset.lot.json`（`spawners[].cast`）。
2. **第二步（让语义进游戏）**：闭合 bundle 链 → 能生成 vignette actor 属性表 →
   导出时把 Cast 写成 vignette + 把 vignette id 挂进 lot。
3. **第三步（环境级人群）**：做出 outfit 色板对照表 → 提供"人群档"预设 →
   与规则编辑器（ER2）联动，选档即校验 `mTransportID`。

### 7.8 对现有 PE 的具体修改清单（按文件）

* `src-tauri/src/prop_models.rs`（spawner 解析）：补 `name`（hex + 命中表）、`refs`（谁引用）、
  去掉"每锚点随机 2~5 人"的生成逻辑，改为按 `cast` 字段生成。
* `src-tauri/src/pe_schema.rs`：`kind:"spawner"` 增加 `cast?`、`anchorKind`（ui/speaker/vignette/plain）。
* 前端 `propModels.ts` / `property-editor` spawner 页签：锥体 → 角色卡；新增"角色库"选择器。
* 新增后端命令 `resolve_unit_cast`：`lot TGI → {spawners[], cast[], crowd?}`（供 inspector + 预览）。
* 单测：`GetSimData` 采样等价性、名字唯一性、引用可解析性。

### 7.9 已落地（2026-11-01 实现记录）

本节 7.1–7.5 的设计里，**不依赖引擎新写入能力**的部分已实现；Cast / Crowd 待定义体闭合。

**新目录（平行实现，不改冻结区）**
`src/pages/packages/components/unit-inspector/`

| 文件 | 职责 |
| --- | --- |
| `spawnerAnchor.ts` | 纯函数层：FNV-1(lower) 名字换算、锚点语义分类（ui/camera/custom/unnamed）、位置读取、三类写入前校验（未命名 / 重名 / 缺变换）、名字建议与重名检测 |
| `SpawnerUnitPanel.vue` | 生成器属性面板：名字编辑（写回 `{typeId:0,group:0,instance}`）、两种硬编码锚点快切、校验内联提示、历史误标列只读披露、角色/外观说明 |
| `SpawnerLibraryPanel.vue` | 组件库「生成器」页签重做：三预设（标准 / UI 气泡 / 镜头）拖入放置 + 锚点清单（语义徽章、重名告警、点选定位）+ 统计 |
| `UnitInspector.vue` | 检查器平行外壳：四个页签复刻（**不新增页签**），**仅当选中单元是 spawner 时**在「属性」页签顶部插入生成器属性区，其余页签复用冻结目录里的原组件实例（只读引用） |

**接线**
* `ModAssetWorkbench.vue`（资产编辑器）：检查器换成 `UnitInspector`（多传 `units`）；组件库多传 `units` / `selectedUnitId` 并接 `@select`。
* `AssetComponentLibrary.vue`：**生成器页签重做**为 `SpawnerLibraryPanel`；放置载荷新增 `spawnerId`。
* `useWorkbenchAsset.ts`：`PlacePayload.spawnerId`；生成器落点若没有预设名字，用 `suggestAnchorName` 自动生成不重名名字（配合"无名字拒绝写"）。
* `clipboard/useUnitClipboard.ts`：Ctrl+V 粘贴锚点时自动改名（引擎按名字建表，直接复制会撞 id）。
* `property-editor/PropertyEditor.vue`（Property Viewer）：**唯一一处最小改动**——两处
  `<PropertyEditorInspector>` 换成 `<UnitInspector>`（等价 drop-in），使 Viewer 的「属性」
  页签同样在选中 spawner 时显示生成器属性区。选中态只能从编辑器内部拿到，
  旁路插槽方案会退化成"整份锚点清单常驻"，已废弃。

**写入收紧（Rust，权威校验）**
* `pe_schema.rs`：`spawnerId` 输出规范化——**只保留 instance，type/group 恒 0**，instance 为 0 视为未命名。
* `asset_build_m1/scene.rs`：
  * 锚点**必须**带非零 `spawnerId` 与非空 `transform`（否则解析期直接报错，附单元 id）；
  * 文档级**重名检测**（引擎按名字建 `vector_map`，同名互相覆盖）；
  * 只写 `SPAWNER_IDS` / `SPAWNER_TRANSFORMS` 两列——**停止写 count/countRandom/agent**
    （实测属建筑变体系统，且三列缺值会让并行数组错位）；
  * `verify_units` 增加生成器数量回读校验（写回后读回数量不符即失败）。

**测试**
`cargo test -p fluffy-open-scp --lib asset_build_m1::scene`（8 例，含 4 个新锚点契约用例）、
`pe_schema`（3 例，含名字规范化与 schema 输出用例）、
`vitest` 新增 15 例（`spawnerAnchor` 纯函数 10 + `UnitInspector` 属性区 4 + 粘贴改名 1）。

**冻结区例外说明（用户 2026-11-01 指令）**：Property Viewer 的 spawner 属性区必须
「只在选中 spawner 时」出现，而选中单元只存在于 `PropertyEditor.vue` 内部状态，
其 `#tabs` 插槽无作用域、拿不到选中态。因此本目录做了**唯一一处最小改动**：
`PropertyEditor.vue` 的两处 `<PropertyEditorInspector>` 换成 `<UnitInspector>`
（同 props / 同事件，drop-in），视觉与行为不变，仅多出"选中 spawner 时的属性区"。
`PropertyEditorInspector.vue` 本体**未改动**。视口/装配/取景链路零改动。

---

## 8. 与旧结论的差异（勘误表）

| 旧结论 | 本文修正 / 补强 |
| --- | --- |
| "spawner 无渲染消费，是纯逻辑锚点" | 方向对，但**不是无消费**：UI 气泡、镜头缩放、演出（vignette）三处都查它；准确说法是"表现层锚点，模拟层不读" |
| "spawner 没有语义" | **语义只在名字里**：引擎硬编码两个名字 id，演出资产按名字 id 引用；名字本身是作者自定义短串，离线无法还原 |
| "spawner 位置 = 小人刷新点"（用户观察） | **因果颠倒**：小人是 agent，从 sink→path 网络出现；锚点只是"看起来在同一处" |
| "不同建筑小人皮肤不同 = 降级源在本地" | 更强：差异来自 **transport entry → 外观定义 → outfit 调色板索引**，40 个 `GI_Simm_*` 角色定义全部在本地 |
| "0x0E715928/29、0x0F0E2BF1 = spawner 计数/agent" | 维持旧勘误：building variation 系统，与 spawner 无关 |
| （新增）"点击文本来自建筑" | 错：来自 **transport entry non-sim 文案列**，再由 UI 事件 `236428978` + `transportType` 处理 |

---

## 9. 复现命令

```powershell
# 1) spawner 名字 id 全量普查（472 lots / 7761 spawners / 257 distinct）
cargo run -q -p probes --example spawner_names -- `
  "D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Game.package" `
  "tmp\spawner_names_game.json"

# 2) spawner 名字 id 的跨资产引用（命中 0x40E1E800 组的位置空间列）
cargo run -q -p probes --example spawner_refscan -- `
  "D:\...\SimCity_Game.package" "0x980F070E,0x141EED53,0x125D19FA,0x3B8ADFE0"

# 3) transport entry 全量导出（264 条）
cargo run -q -p probes --example simm_transport_dump -- `
  "D:\...\SimCity_Game.package" "tmp\transport_dump.json"

# 4) 小人外观定义全量导出（179 个定义，group 0x40E02D00）
cargo run -q -p probes --example simm_appearance_dump -- `
  "D:\...\SimCity_Game.package" "src-tauri\resources\database_main.s3db" "tmp\simm_appearance.json"

# 5) 文案 id 反查（结论：本地 locale 与 s3db 均不含，属 UI 侧）
cargo run -q -p probes --example locale_lookup -- `
  "D:\...\SimCityData\Locale\zh-tw\Data.package" "0xD606A55F,0xD606A554,0xD27FACB3,0xD27FACB8"

# 6) lot 的 vignette 列表 vs 定义体（结论：本包零交集，定义体在别处）
cargo run -q -p probes --example vignette_resolve_probe -- `
  "D:\...\SimCity_Game.package" 5      # 产物 tmp\vignette_resolve.json
```

产物：`tmp\spawner_names_game.json`、`tmp\transport_dump.json`、`tmp\simm_appearance.json`、
`tmp\fnv1_name_table.json`（155k 候选名字表）。

## 10. 未决问题

1. `0x40E1E800` 组到底是什么资产（演出？交互点？）——需要按 TGI 反查一次名称/引用方。
   **本轮进展**：该组下 237 个属性文件带完整 actor 表（actorIDs / modelNames / startPoint /
   endPoint / vignetteType / speeds / 资源绑定），但**没有任何 lot 引用它们**
   （`0x0D2E8A55` 的 279 个 id 与它们零交集，且这些 id 在整包别处零引用）。
   下一步：扫 `SimCity_App.package`/DLC 的实例表，或对 `cVignetteGame::StartGame`
   取表处下 hook 打印真实 TGI。
2. 活动文案 id（`0xD606A55*` / `0xD27FACB*`）的字符串在 `SimCity_App.package` 的 UI JS 中，
   尚未追。
3. `0x0D801143` 列（长度可达 40+ 的位置空间数组）语义未定——可能是"建筑可交互点集合"。
4. 交通规则（ER2 二进制）中 `mTransportID` 的具体取值分布未统计（需要在
   `cEcoUnitRule::Read` 的 stream 层加探针）。
5. 工厂工装到底走 transport entry 还是演出演员（`GI_Simm_sanitation_*` 在 B1B104 里
   没有被 key 引用）——需要在 `SimCity_App.package` / DLC 包里再扫一遍参照。
