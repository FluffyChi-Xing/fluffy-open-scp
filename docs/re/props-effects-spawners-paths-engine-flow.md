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

**杂件 props（LOD1 直引族）的贴图位置（2026-10-05 实证补充）**：其模型
RW4 文件**自带文件级 TEXTURE section**（垃圾桶 0x903A704C：#11=彩色
diffuse 256×256、#13=法线 256×256），材质段为 Raw（仅参数无引用）——
LOTM 构建器 v10 已实现：无参数表材质 → 文件级纹理段 [0]→slot0_png、
[1]→normal_png（垃圾桶实测 slot0 120KB/normal 176KB 正确下发）。

**③ cGraphicsInstancedSim（市民外观表）**

```
触发：0x0C36D30D 存在但 0x0D8C29CF 缺失
数据：8 列外观表（0x0CBC0xxx 族：bodies/heads/outfits 及其 Max + scales
      Min/Max）→ cSimGraphicsInfo 数组（body/head/outfit 变体范围 + 缩放）
```

→ 小人群体定义（spawner 的外观提供者）。PE 展示为标记 + 外观参数即可。

### 1.2.1 prop 子分类与标志性代表（贴图/模型实证定性，2026-10-05 普查）

6673 个 lot 共 96 个不同 prop id，全分类后子类及代表：

| 子类 | 代表 resourceID | 实物（贴图/模型解码定性） | 渲染路径 | PE |
| --- | --- | --- | --- | --- |
| 轿车/旅行车 | 0AB4DFE1-E3 | 4 LOD 轿车+旅行车（slot0 彩色图集 19.5KB） | 显式 key→Vehicle Models | ✓ 真实模型 |
| 垃圾清运车 | 92BEE95F | C600 载具定义带车灯列 | vehicle_models 直出 | ✓ 真实模型 |
| 混凝土搅拌车 | 54CA89F0 | 搅拌车单模型 | LOD1 直引 | ✓ 真实模型 |
| 大型垃圾箱 | 85271637→903A704C | 连体垃圾箱+垃圾堆（slot0 256×256 图集 120KB） | LOD1 直引 | ✓ 真实模型 |
| 街道家具 | 9375C65E→AD64CB3D | 长椅/野餐桌/停车计费器（256×256 家具图集） | LOD1 直引 | ✓ 真实模型 |
| 乔木 | 14984C68-6B | 4 LOD 资源族 → 树种 descriptor（34 变体权重表） | 显式 key→impostor | 锥体（变体模型=运行时/服务器内容） |

（普查明细见 docs/design/pe-component-replacement.md §4.5 本地稿）

### 1.3 树的颜色与图集机制（cImpostorRenderer 深挖，2026-10-05 续）

**颜色公式（`cGraphicsInstancedImpostor::GetImpostorInfo`，753747 逐字）**：

```
env = pGfx->mSeasonInfo.mColorCache[mModelType]   // mModelType = descriptor
      key instance（C602CD31）索引季节色缓存；越界回落
      kDefaultTreeEnvironemnt
返回色（HSV 域随机，randomBits 每实例不同）：
  H = HsvMin[0] + ((randomBits>>4 )&7)/8 × (HsvMax[0]-HsvMin[0])
  S = HsvMin[1] + ((randomBits>>7 )&7)/8 × (HsvMax[1]-HsvMin[1])
  V = HsvMin[2] + ((randomBits>>10)&7)/8 × (HsvMax[2]-HsvMin[2])
（打包为 RRRGGGBBB 位域）
```

**kDefaultTreeEnvironemnt = {flags:1, HsvMin:(0,0,0), HsvMax:(0,0,0), 1.0}**
（170018 行）——缺省环境无色域（全零），实际色域来自 mSeasonInfo.mColorCache
（按 descriptor 实例索引，每树种一个 cTreeEnvironment{HsvMin[3],HsvMax[3]}，
u8 存储 ×1/255 归一化——`cTerrainForest2::UpdatePixelShaderData` 实证）。

**图集机制（`cImpostorRenderer::Init`，749707）**：atlas = **运行时生成**——
`cRectAllocator::Clear(512, 512)` 动态矩形分配（512×512 图集）；impostor
以粒子公告板批渲染（`RenderOneParticleBatch`，顶点 V3FN3FC4BT2F）；最多
2 个 impostor class（`mClasses[2]`）。图集内容 = `RenderOffscreenForModel`
运行时离屏渲染注册模型所得，**非静态贴图**。

**新发现：`cTerrainForest2` 地形森林系统**（00430150 区）——地图级森林
独立于 lot prop 树：专用材质（shader 0x299）+ 森林渲染目标
（cForestRenderTarget），HSV 色域同源（mSeasonInfo.mColorCache）。用户在
游戏里看到的大片树 = 此系统 + lot prop 树（impostor）两层。

### 1.4 PE 树渲染落地方案（基于本轮机制）

1. **形状**：程序化树形公告板（canvas 生成树冠轮廓 + 树干），或用一张
   通用树剪影贴图——引擎的图集是运行时渲染的，本地无静态树图集可取。
2. **颜色**：按本文公式——绿域 HSV 随机（H≈90-130°、S 中高、V 中），
   每树实例用 randomBits（unit.index 派生）取色，即可复刻"同片树林
   深浅不一"的游戏观感。
3. **LOD 位掩码/次 key**：PE 无距离渲染，可忽略。

### 1.5 树的季节目录与"Tree"配置（impostor 深挖第三轮，2026-10-05）

**季节色缓存来源（`cGraphicsSeason::FillFromProps` 749167 + 调用点
764640 逐字）**：

```c
v53 = EA::StdC::FNV1_String8("Tree", 0x811C9DC5, kCharCaseLower);
     // = 0xE085813D（大小写折叠）
v51->GetPropertyList(v51, v53, kImpostorConfigGroup, &seasonConfig);
SC::cGraphicsSeason::FillFromProps(&mSeasonInfo, seasonConfig, mGame);
```

- **kImpostorConfigGroup = 0x3FFF0000**（170035 行）；树种 descriptor
  C602CD31 的 Parent 正指向该组——impostor 配置体系确认。
- **"Tree" 配置（0xE085813D @0x3FFF0000）在全部本地数据源缺席**（安装包
  全套/离线版全套 84 个 EcoGame 文件/mod 包/Server、Cache 包均扫无）——
  季节 HSV 色域为服务器内容。缺席路径实测推演：`FillFromProps(null)`
  空转 → 色缓存空 → `GetImpostorInfo` 回落 `kDefaultTreeEnvironemnt`
  （全零 HSV）→ **颜色调制恒等，树以图集原色渲染**——破解版不联网也
  有树，即此回落路径。
- `BuildCacheForDay(dayOfYear)`（749009 行）：按年积日在 day-keyframe
  （u8 HSV 对，p_mDay±偏移）间 lerp——叶量（mLeafAmount）随季节渐变
  （落叶机制）。
- **`cGraphicsSeason::Update(pRenderer, impostorClass, pGame)`**（11246
  行声明）：季节系统直接驱动 `cImpostorRenderer`——公告板图集由季节
  更新过程填充。

**树渲染全链闭合**：lot 树 prop → cGraphicsInstancedImpostor{descriptor
C602CD31, LOD 位} → 季节 Update 驱动 cImpostorRenderer → descriptor 的
34 变体（权重表）按 randomBits 选条目 → 离屏渲染进 512×512 图集 →
公告板粒子（V3FN3FC4BT2F）+ HSV 随机色。**变体的 3D 模型源（34 key 的
链尾）为运行时/服务器内容，静态数据只有链表头**——PE 用程序化树形公告
板 + HSV 随机即可获得等价观感。

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
