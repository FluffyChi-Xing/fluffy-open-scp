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

### 1.4 PE 树渲染落地方案（2026-10-06 已实现——真图集公告板）

1. **形状/颜色 = 游戏真图集**：Graphics 包 **0x835D64F3**（256×512，绿
   占比 95%，find_foliage_textures 探针命中）= 树公告板图集静态源——
   上半 256×256 = **2×2 四树格（128×256/格）**，下半为地面纹理。后端
   `resolve_prop_models` 对树 prop 下发 `treeAtlasPng`（解顶层 mip → 裁
   上半 → PNG → base64）；前端 Sprite 按 randomBits 选格渲染。
   **图集 alpha 通道 = 树剪影遮罩**（实测 min=0/max=255，背景 alpha=0）
   → SpriteMaterial alphaTest 0.5 干净抠像（首版漏开透明渲染成绿底
   矩形，已修）。
2. **图集族**：Graphics 包存在多个 64×128 树图集（0x7B500736 绿 98%/
   0x983213DD 绿 67% 等约 25 个 = 不同树种/尺寸的 impostor 图集族），
   可按树种 descriptor 关联接入更多树种。
3. **尺寸**：Transform.Unknown（半宽）×16 为冠宽，clamp 1.2–12m。
4. **LOD 位掩码/次 key**：PE 无距离渲染，可忽略。

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

**树渲染全链闭合 + 引擎显式回退（source-tree 提取版实证）**：lot 树 prop
→ cGraphicsInstancedImpostor{descriptor C602CD31, LOD 位} → 季节 Update
驱动 cImpostorRenderer → descriptor 的 34 变体（权重表）按 randomBits 选
条目 → 离屏渲染进 512×512 图集 → 公告板粒子（V3FN3FC4BT2F）+ HSV 随机色。

**引擎对缺失变体有显式回退**（`Season::FillFromProps` source-tree 提取版
逐字）：34 变体 key 逐一查季节日数据 map，**查不到 → `v43 = 782826392`
（= 0x2EA8FB98，本地 @Game 09878A01 存在的那条）**——即幻影变体统一回落
到默认树 2EA8FB98 的季节环境。这解释了破解版离线有树：引擎设计就容忍
变体/季节目录缺席，回退到本地默认树 + 图集原色（HSV 恒等）。

**SIGGRAPH2007 Alpha Tested Magnification**（Chris Green，new-cource 目录
PDF）= 贴花文字描边/发光技术的源头：距离场文字 + alphaTest 任意缩放放大
+ outline/glow/dropshadow 全在像素着色器（Figure 6/7/9）——与项目贴花族
的"四通道厚度场/SDF"定谳互证（graffiti 连续喷漆厚度场即距离场应用）。

### 1.6 模型树路线评估（2026-10-06 定谳——树 3D 模型存在且可直出，路线可行）

> 背景：真图集公告板（§1.4）每格仅 128×128/128×256，用户反馈近景抠像精度
> 不足。本轮从 source-tree 源码 + 属性表双向钉死 impostor 离屏渲染源，
> **四棵真 3D 树模型在本地数据中完整存在**，模型树路线全面可行。

**勘误**：52538f5 提交信息所称"BB2760C9 族/D61B3400 族/8A0D18F7 树模型
候选族"**全部错误**——那些是带 facade shader（38869BDA）的窄小建筑
（棚屋/亭子类）。形状探针（高宽比>1.3）恰好把真树排除：树冠宽 39-46m，
高宽比仅 1.1-1.4。教训：树模型定性不能靠形状，要靠 impostor 配置表反查。

**① 配置链（属性表逐字，App 包 group 40002D00）**：

```
树种 descriptor C602CD31（1630B，9 键）
 ├─ 00B2CCCB Parent → 5F804D7E @40002D00（impostor class 表，Parent 继承合并）
 ├─ 0BD62576 NumAngles = 1
 ├─ 0DDE0508 = true
 ├─ 0E0B99FD [34] 变体权重表：2EA8FB98×8、5517D7F7×8、43352C8C×5、
 │             CBFF65DD×4、DA928B73×4、CD984F66、C683C0D5、BFC2F36A、
 │             64030073、86CFD463（即 §1.5 的 34 key，全部 G=0 幻影/季节节点）
 ├─ 0E0B99FE [34] i32 旋转/朝向参数（31,41,120,130,253…）
 ├─ 0E0B99FF [34] Vector3 HSV 随机域下界（H -31..90, S 0.5..1.2, V 0.6..1.2）
 ├─ 0E0B9A00 [34] Vector3 HSV 随机域上界（H -20..115, S 1..3, V 0.8..1.8）
 ├─ 0E0B9A02 bool
 └─ 0E0B9A03 [34] f32 叶量参数（0.35..1，= cGraphicsSeason mLeafAmount 对位）

impostor class 表 5F804D7E（=1776EABC 同表，8 键，4 part/行）：
 ├─ 0BD62577 模型 key [4] = 4DF43690 / C2FBD178 / 1113B131 / 89D658DF
 │             （全在 Graphics 包 T 2F4E681B G 0——真模型实体！）
 ├─ 0BD62578 每部分 BBox：39×39×47 / 28×28×44 / 46×46×41 / 34×34×30.5 (m)
 ├─ 0BD62579 动画 key [4] = null
 ├─ 0BD6257A/7B 图集格宽×高 [4] = 128×128（引擎公告板原生精度！）
 ├─ 0BD6257C ZBias [4] = 0
 ├─ 0BD6257E 离屏材质 [4] = CE5BBF5E（×4 同一材质）
 └─ 0E0B9A01 季节索引 key [4] = CD984F66 / 2EA8FB98 / 2EA8FB98 / 2EA8FB98
```

**② 引擎消费链（source-tree 提取版逐字，tmp/dynamic/source-tree-impostor/）**：

`cImpostorRenderer::InitImpostorClass`（0x693490）：配置属性表
（GetPropertiesAsTable，propID 198583671=0x0BD62577 表，列 198583672-681）
逐行 → `SP::CreateModelInstance(instanceID, groupID)` 建模型实例 →
`cImpostorClassPart{mModel, mBox, mOffscreenMaterial, mZBias, mRectID…}` →
`RenderOffscreenForModel`（0x691920）：按 mNumAngles 个方位角投影，
`DrawBuffersInstanced` 离屏画进 `cRectAllocator(512×512)` 分配的 **128×128**
格 → 场景内按格贴公告板粒子（V3FN3FC4BT2F）。

→ **引擎世界中树从来只有 128×128 公告板精度**——现有抠像公告板已达
引擎同档。要超越游戏观感，唯一路径就是把模型直接 3D 渲染，而模型就在本地。

**③ 四模型实体定性（tree_model_dump 探针 + GLB 光栅化目验，全部通过）**：

| part key | 大小 | verts | tris | 形态（光栅化目验） | 自带纹理 |
| --- | --- | --- | --- | --- | --- |
| 4DF43690 | 270KB | 732 | 370 | 针叶树（层叠枝叶圆锥形） | tex#0 256×512 树抠像图集 a[0..255] 44% 透明 + 128×256 树皮 + 128×128 地面 |
| C2FBD178 | 768KB | 4654 | 2661 | 阔叶树（椭圆冠） | tex#0 256×512（30% 透明）+ 128×256 + 256×512 |
| 1113B131 | 624KB | 2935 | 1774 | 阔叶树（圆冠） | 同布局（38% 透明） |
| 89D658DF | 624KB | 2651 | 1646 | 阔叶树（开张宽冠） | 同布局（31% 透明） |

每模型单 MESH、真 3D 体积几何（冠层为叶团卡片拼合出的立体轮廓），
自带文件级 TEXTURE section（材质段 Raw，同垃圾桶 0x903A704C 形态，
LOTM 构建器 v10 已支持）——tex#0 即树公告板抠像图集同源内容
（4DF43690 的 tex#0 与 0x835D64F3 上半一致：2×2 四视角抠像+下半地面纹理）。

**④ 孤立图集反证（foliage_ref_scan）**：四包 12133 个 RW4 资源无任何
材质引用 23 个 foliage 图集实例——图集是引擎按 key 直取的 impostor 系统
约定资源，不存在引用它们的静态模型；树模型贴图走自身文件级纹理段。

**⑤ PE 落地方案（模型树路线，已实现——真图集公告板降级为回落通道）**：

已实现（本节为实施记录）：后端 `resolve_prop_models` 树签名分支改发
`source="tree_model"` + 4 个 part 模型 TGI（图集 base64 仍随发供回落，
模型全缺席时回落 source="tree"）；前端 session 旁路加载 4 形状 LOTM 载荷
（全部树 prop 共享、只载一次）→ `propModels.ts getTreeModelObject` 真 3D
渲染：几何/图集纹理跨实例共享（模板缓存），每实例 seed 选形状 + descriptor
34 变体 HSV 域 tint（0E0B99FF/0E0B9A00 逐字表，8 离散步 GetImpostorInfo
同构，onBeforeCompile 注入色相旋转+S/V 乘法）+ 随机 yaw + alphaTest 0.5
叶卡。关键技术定谳：

1. **纹理 V 语义**：树模型 UV 为 RW4 v=0 底部语义，LOTM GLB 直读会采错
   半张图（四模型冠层绿占比交叉验证：`flipY=true` 命中 48-87%，直读
   14-69% 且树干误中绿区）——纹理必须 `flipY=true`。
2. **单材质即整树**：mesh UV 只采 tex#0（树视图图集既是叶卡材质库、也含
   树干像素区）；tex#1（树皮 128×256）为 LOTM normal 通道伴生下发，前端
   不用；tex#2（地面）模型内无对应 quad，不消费。
3. **尺寸连续性**：冠宽沿用公告板口径 `clamp(半宽×16, 1.2, 12)`，模型按
   自身冠幅（bbox x/y 跨度 39-47m）等比缩放——观感与公告板时代连续。
4. 叶卡不投影（alpha 不进深度 pass，全四边形影子是假影；公告板同款无影）。
5. **颜色 = 图集原色（真机勘误）**：首版把 34 变体 HSV 域误实现为逐像素
   色相旋转 → 负色相域变体把绿树染成暗红（用户真机截图对拍游戏绿树）。
   引擎口径定谳：GetImpostorInfo 返回的是 **RRRGGGBBB 位域实例色**
   （HSV 先取样成 RGB tint 色再乘纹理，非逐像素旋转）；且离线路径季节
   "Tree" 配置缺席 → 回落 kDefaultTreeEnvironemnt → 调制恒等。故 PE 取
   图集原色（白乘），变体表留档于 §1.6 ①，接入季节系统时按 tint 色口径
   实现。

原方案记录（供设计追溯）：

1. 后端 `resolve_prop_models`：树 prop 下发 4 个 GLB（含文件级纹理 →
   slot0_png/normal_png）+ descriptor 参数（34 变体权重/HSV 域/叶量）。
   复用 LOTM v3 容器与车辆/杂件同一条 GLB 通道，无需新格式。
2. 前端 InstancedMesh 实例化渲染（370-2661 tri/棵，千级实例无压力）；
   per-instance：randomBits 选变体 → 34 变体表映射 4 part（0E0B9A01 提示
   part0↔CD984F66，其余映射待实现时定）→ HSV 域随机 tint（instanceColor）
   + 0E0B99FE 旋转 + 0E0B9A03 叶量（可选 discard 模拟落叶）。
3. 树叶卡片材质 = alphaTest（tex#0 alpha 通道即剪影，同公告板抠像）。
4. 图集公告板保留为远景 LOD（可回落），近景换真 3D——超过引擎原生观感。

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
2. **树**：模型树路线升级（§1.6）——4 棵真 3D 模型 GLB 直出 +
   InstancedMesh + 变体 HSV tint；图集公告板降级为远景 LOD。
   （§1.4 公告板方案已落地，作为引擎同档精度保底。）
3. **实例色**：0xFBA612 读取加入 prop 解析（ ColorRGBA → material.color）。
4. Effects/Spawners/Paths：维持既有方案（标记 + 语义），无渲染缺口。

## 6. 与旧结论的差异表

| 旧结论 | 本文修正 |
| --- | --- |
| 树变体模型为服务器流式 3D 内容 | 树 = **impostor 公告板**（cGraphicsInstancedImpostor），无 3D 模型；34 key = 变体权重链表节点，非模型 key |
| （§1.6 再修正）树 3D 模型不存在于本地 | 公告板离屏渲染源 **4 棵真 3D 树模型在本地**（Graphics 包 4DF43690/C2FBD178/1113B131/89D658DF，由 descriptor C602CD31→Parent 5F804D7E 配置表 0BD62577 列直指） |
| （52538f5）BB2760C9/D61B3400 族=树模型候选 | **错误**——facade shader 窄小建筑；形状探针高宽比>1.3 恰好排除真树（冠宽 39-46m） |
| 槽位资源不可解则无法渲染 | 引擎同样只认资源表；不可解 = 引擎也不渲染（我们的 phantom 集合与引擎空白一致） |
| 实例色来源未知 | 包装属性 0xFBA612（ColorRGBA）→ mModelColor → 实例 tint |
| spawner 三列为生成数量/agent | buildingVariation 系统（维持 unit-props 文档 §3.2 勘误） |
