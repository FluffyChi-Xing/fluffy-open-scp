# Props / Effects / Spawners / Paths 引擎消费渲染全流程（dev 源码取证）

> 来源：dev 完整反编译（SimCity.unwrapped.exe.c，行号可复核）+ 着色器容器
> （tmp/dynamic/all_blocks_full.txt）。取证日期：2026-10-05。
> 本文修正此前"树变体模型为服务器流式 3D 模型"的方向性误判——**树根本不是
> 3D 模型，是 impostor 公告板**，其渲染不依赖任何本地不存在的"树模型"。

## 0. 总链路图

```
lot 属性列（六族）                    脚本资源表（EcoGame 40E0C100 族）
 ┌─────────────┐                     ┌──────────────────────────┐
 │ Props 0x0C12EF20 族       │ → resourceID → │ cGraphicsResource 两分支  │
 │ Effects 0x02A907B5 族     │ → effect key  │ → gfxKey（包装 B1B104）   │
 │ Spawner 0x0E1BAC61/62     │ → 位置名      └────────┬─────────────────┘
 │ Paths 0x0CAA68xx 族       │ → 段定义               ↓
 └─────────────┘                     GetInstancedModelId(gfxKey)
                                             ↓ GetPropertyList(instance,group)
                                     cGraphicsInstanced::FillFromProps（950798）
                              ┌──────────────┼──────────────────┐
                              ↓              ↓                  ↓
                    cGraphicsInstancedModel  cGraphicsInstanced
                    (ModelInstanceLODSet     Impostor（树！）
                     真 3D LOD 集=车辆/杂件）        ↓
                                          cImpostorRenderer
                                          RenderOffscreenForModel
                                          （运行时离屏渲染成公告板）
```

## 1. Props 全流程（逐函数实证）

### 1.1 装配（lot 列 → 运行时结构）

| 步骤 | 函数（行号） | 内容 |
| --- | --- | --- |
| 资源表 | `cEcoGameScripts::Init`（662313）+ loader（660380） | 主表（group 0x40800200，master 622B9CD7@40E0C800）→ 九张子表 group 列表 → 逐 group 枚举属性列表 = 表项（instance=resourceID） |
| 资源→模型 | `cGraphicsResource::FillFromProps`（952181） | 两分支：0x0D8C29C3 显式 key / 0x00F9EFBB·0x0D897169·0x0C36D30D 标记 → 资源自 key；→ `GetInstancedModelId(gfxKey)` |
| 槽位 | `cUnitBinDraw::FillFromProps`（959040） | ID 列=本箱资源清单（→mResources，绑资源 bin 驱动数量）；Transform+Slot 列 → mDrawSlots；RandomizeSlot=洗牌；PercentFill=按存量比例 |
| 实例化 | `cUnitBinDraw::Update`（957107） | mResourceSlots[slot]=resourceID → `mResourceIDToGraphics[resourceID].mGfxId` → `cGraphicsInstanced` 添加实例（`unitModel∘slotTransform`，957230） |

### 1.2 渲染三分类（`cGraphicsInstanced::FillFromProps`，950798——本文核心新证）

`GetInstancedModelId(gfxKey)`（951131）= **GetPropertyList(key.instance, key.group)**
把 gfxKey 当**属性表**加载，再按属性内容分派三类数据（`cSimpleUnion<536,8>`）：

**① cGraphicsInstancedModel（=227112195，`cModelInstanceLODSet`，真 3D）**

```
触发：0x0D897169 KeyArray（Vehicle Models）或 0x00F9EFBB 单 key
数据：≤4 个模型 key（instance+group 成对存 24B 槽）= LOD 集
附加：材质 id = 0xCDCCA5A（SetMaterialID）
      实例色 = 0xFBA612（ColorRGBA——车漆/整体 tint 的引擎来源！）
      LOD 距离 = 0x2E33A81 float 数组，缺省 = bbox 长轴 × scale × 0.125
渲染：cModelInstanceLODSet::AddToRenderer
```

→ **车辆/杂件**：4 key = LOD 集（体积乱序，引擎按距离选）；直引 key（如
0x00F9EFBB→0x903A704C 142KB）= 单模型。**openscp 已实现 ✓**。

**② cGraphicsInstancedImpostor（树！本文最大修正）**

```
触发：0x0C36D30D KeyInstance（树种 descriptor）存在 且 0x0D8C29CF uint32 存在
数据：buffer[0]=descriptor key instance（C602CD31 → App 包 40002D00）
      buffer[8]=LOD 序号（0x0D8C29CF，0..3）
      buffer[10]=1<<LOD 位掩码
      buffer[4]=0x0E701137 key instance（次 descriptor，缺省=主 descriptor）
```

**树不是 3D 模型**——是 impostor 公告板：`cImpostorRenderer::
RenderOffscreenForModel`（11269 行声明）在运行时把 descriptor 引用的表示
**离屏渲染成图集单元**，场景内按 LOD 位掩码贴公告板。这解释了：
- 树"模型"不存在于任何包——它们从未以 3D 网格形式静态存在；
- descriptor 的 34 key 数组（0x0E0B99FD，带权重重复 8/8/5/4/4…）=
  变体权重表，其 key 节点（如 2EA8FB98@09878A01：{self 引用, next, 2 bool}）
  是**变体链表节点**，链在本地数据中不完整（next→幻影）；
- 破解版有树 = **impostor 图集内容在本地**（descriptor 体系本身可及），
  缺的只是"离屏渲染源"——而 impostor 本来就不需要 3D 源也能以已有图集呈现
  （引擎对预渲染图集与实时离屏两态兼容，待实现时验证）。

→ **openscp 落地方案修正**：树 = 公告板渲染。取得 impostor 图集纹理
（见 1.3）后按 descriptor 变体权重铺公告板；无图集时保持标记锥。

**③ cGraphicsInstancedSim（市民外观表）**

```
触发：0x0C36D30D 存在但 0x0D8C29CF 缺失
数据：8 列外观表（0x0CBC0xxx 族：bodies/heads/outfits 及其 Max + scales
      Min/Max）→ cSimGraphicsInfo 数组（body/head/outfit 变体范围 + 缩放）
```

→ 小人群体定义（spawner 的外观提供者）。PE 展示为标记 + 外观参数即可。

### 1.3 未决（树的唯一缺口）

impostor 图集的实际纹理/条目格式：`cImpostorRenderer::Init/GetImpostorClass/
RenderOffscreenForModel`（11249-11274 行声明区，函数体未深挖）。下一步 =
读 `cImpostorRenderer::Init` + `GetImpostorClass` 找图集纹理 key 的来源
（全局或按 descriptor），拿到树图集后即可实现公告板渲染。

## 2. Effects 全流程

| 环节 | 函数（行号） | 内容 |
| --- | --- | --- |
| 挂载 | `cModelWorld::AddAttachments`（437000） | 双挂载点（模型属性表 + lot unit）五列：0x02A907B5 keys（主 id，`HasVisualEffect` 判定）/ B6 transforms / BB externalIds / BC enabled / B9 effectWorlds |
| 实例化 | EffectsManager → Swarm 世界 | effect key → Swarm 效果资源（独立格式，ArgScript 系）→ 粒子/光/音组合体 |
| 运行时 | `EffectControl → SetEffectsRunning(enable, 1, externalId)` | 按 externalId 启停（火情/废弃/夜间开关） |

**渲染本体 = EA Swarm 粒子系统**（非 RW4）。PE 阶段方案不变：挂点标记 +
语义名（key 可在包中查元数据）；真实粒子需 Swarm 解析器（独立工程）。

## 3. Spawners 全流程

| 环节 | 函数（行号） | 内容 |
| --- | --- | --- |
| 装配 | `cUnitModel` 属性段（958480） | 0x0E1BAC61（Key 数组）+ 0x0E1BAC62（Transform 数组）→ `mUnitLocations: vector_map<instance, transform>` |
| 消费 | 位置查询 API（952960 区） | 按名取 transform——**agent 生成/服务点查询**（门口/车位/停机位）；无渲染消费 |
| 勘误 | 同段 | 0x0E715928/29（buildingVariation 基值/变体数）+ 0x0F0E2BF1（影响 bin）= **建筑变体系统**（调色板行选择），与 spawner 无关 |

**无渲染形态**：spawner 列不产生可见物，是游戏逻辑锚点。PE = 语义标记
（按位置名/用途图标），标签需脚本反查 id 语义。

## 4. Paths 全流程

| 环节 | 函数（行号） | 内容 |
| --- | --- | --- |
| 装配 | `SC::CreatePathsFromProps`（1024975） | 七列全解码（点/切线/索引/对/entry/scope/connect）；entry→`IndexFromPathEntryID`（-1 或 scope 拒 → 跳过该路径）；`CreatePath(pathEntryIdx, n, points, scope, connect, segIdx)` |
| 段结构 | `cUnitPath` 族 | Hermite 段进路径网络，agent 沿段行走；UpdateTangent 逐段喂切线 |
| 渲染 | `mPathDistanceTuning.mColors`（cLayer） | **仅数据图层可视化**（路径距离热力图），正常游戏不画路径线 |

PE 方案：点+切线 Hermite 样条管（按 entry 类型着色），类型语义需脚本
pathEntries 表（EcoGame 40E0C400，已实证可读：0x0BD81A2D-31 列族）。

## 5. 对 openscp 的行动清单（按本文修正）

1. **车辆/杂件**：已实现（LOD 集降序取最高细节 + slot0 彩色 diffuse + 车漆
   lerp——注意实例色的引擎来源是包装属性 0xFBA612，可作为色带的替代/
   校准源：**若包装记录带 0xFBA612 则直接用其色，无则色带随机**）。
2. **树**：改方案为 impostor 公告板——先挖 `cImpostorRenderer::Init/
   GetImpostorClass` 找图集来源；有图集 → 按 descriptor 权重铺公告板。
3. **实例色**：0xFBA612 读取加入 prop 解析（ ColorRGBA → material.color）。
4. Effects/Spawners/Paths：维持既有方案（标记 + 语义），无渲染缺口。

## 6. 与旧结论的差异表

| 旧结论 | 本文修正 |
| --- | --- |
| 树变体模型为服务器流式 3D 内容 | 树 = **impostor 公告板**（cGraphicsInstancedImpostor），无 3D 模型；34 key = 变体权重链表节点，非模型 key |
| 槽位资源不可解则无法渲染 | 引擎同样只认资源表；不可解 = 引擎也不渲染（我们的 phantom 集合与引擎空白一致） |
| 实例色来源未知 | 包装属性 0xFBA612（ColorRGBA）→ mModelColor → 实例 tint |
| spawner 三列为生成数量/agent | buildingVariation 系统（维持 unit-props 文档 §3.2 勘误） |
