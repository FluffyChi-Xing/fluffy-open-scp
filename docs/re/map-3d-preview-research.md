# 地图 3D 预览调研——高度场重建 / 着色 / 名字映射 / 伟大工程定位（2026-10-07）

> **2026-10-08 着色勘误**：ED b1 是森林，草量来自土壤×地下水。
> 本文旧着色推断已由 [通道实证与修正](map-terrain-color-correction.md) 取代。

> 动机：地图面板已有 2D 俯视渲染（341-tile 金字塔 + 水位面 + 地块框），但有四个问题：
> ①着色不对 ②名字映射未解决 ③预览不是 3D ④伟大工程定位找不到。
> 本文对拍 dev 源码（`docs/source-code/SimCity-dev-ida.c`，190 万行 Hex-Rays dump）
> 与本地游戏数据，给出四问的实证结论与 3D 预览实现路线。
> 前置阅读：[region-and-map.md](../overview/glass-box/region-and-map.md)（tile 金字塔/水位面）、
> [map-ground-coloring-and-playable-region.md](map-ground-coloring-and-playable-region.md)（城市绿地着色链）。
> 置信度：**[高]**=字节级/函数体实证，**[中]**=结构推断，**[低]**=推测。

## 0. 结论速览

| 问题 | 结论 | 证据强度 |
|---|---|---|
| ④ 伟大工程定位 | **在本地**：EP1 包区域模板 JSON（type 0A98EAF0, instance 4529F96F）`map.greatWorks[{name,x,y}]`，15 张模板全覆盖 | **[高]**，坐标双侧互证 |
| ② 名字映射 | 注册表（Game 包 JS）模板名→stringID → locale JSON 查表；区域/城市/描述全部解析成功；FNV-1(小写) 算法定案 | **[高]**，闭环验证 |
| ① 着色 | 引擎 = 层栈色调（0.48,0.39,0.27）× 6 面 cube 细节 × ecoMap 三色插值 + **平方**；绿地 = ED b1；水面 = 全局平面 | **[高]**（函数体），cube 图载体待定位 |
| ③ 3D 预览 | 引擎 = 2048m 四叉树节点网格 + SVT 页高度位移（VS：z=raw/32−1024）；数据侧 341 tile 已够 1:1 重建 | **[高]** |

**勘误**：region-and-map.md §4.1「每区域 = N 城地块 + 1 伟工位（多出的单条组）」**有误**——
那"多出的一组"就是**区域组自己**（区域组内也有一条 ≤60B property 回指自己的 desc，被
region_bind 误计为城市）。RT0 静态包内没有任何伟工条目。F0 记账不变（3846 = 11×341 + 95 城市）。

---

## 1. 伟大工程定位——已破案，本地数据（勘误"服务器下发"）

### 1.1 数据源：区域模板 JSON（本地服务器的模板库）

`SimCityDataEP1.package`，type `0x0A98EAF0`（JSON 记录类型，exe L269034 注册）、
**instance `0x4529F96F`**、group 逐模板不同（15 条）。这就是 UI 引用串里的
`AutomatedRegionTemplates.json`（服务器区域模板），离线模式下它是本地数据。

结构（Tutorial 模板实测）：

```json
{
  "boxes": [{ "isClaimed":"false", "isFriend":"false", "isTutorial":"false", "x":1024, "y":0 }, ...],
  "height": 4, "width": 4,
  "key": "244",
  "name": "Tutorial", "localizedName": "Tutorial",
  "status": "TUTORIAL",
  "postcard": "http://s3.amazonaws.com/.../region_templates/...",
  "map": {
    "cities":    [ { "uid":"1026", "x":1024, "y":0, "z":-772.9 }, ... ],
    "greatWorks":[ { "name":"Region_GreatWorks_Empty", "x":..., "y":... }, ... ]
  }
}
```

15 张模板 × 伟工数（与官方图鉴全对上）：

| 模板 name | 城市 | 伟工 | 坐标（世界米） | 对应 RT0 组 |
|---|---|---|---|---|
| Confluence（白水谷） | 5 | 1 | (2684,2773) | BEAF0510（此前手工标注 ≈(2656,2496)，互证 ✓） |
| Horizon（地平线群岛） | 11 | 3 | (608,5861) (-6134,1989) (-3168,-7419) | D01FA985 |
| Oasis（绵延不毛之地） | 14 | 2 | (592,-932) (-2674,317) | 9F735B20 |
| LittleGorge（泰坦峡谷） | 16 | 4 | 见探针输出 | BC357A2B |
| Sawyer（三角洲） | 16 | 4 | 见探针输出 | B12DE348 |
| Gallia（藍綠森林） | 16 | 4 | 见探针输出 | C04182E4 |
| CaspianLake（大理石湖） | 10 | 2 | (-7230,-6721) (-538,6079) | 9E9B1FF0 |
| CapeTrinity（三一岬） | 3 | 1 | (-302,650) | C2A9C48F |
| EdgeWaterBay（愛華特灣） | 7 | 1 | (1832,-1215) | E41A82B8 |
| TwinCities（追日灣） | 2 | 1 | (-2512,-2930) | E0183D94 |
| SC_Desolation（荒涼） | 7 | 1 | (-1940,-3366) | A0B60DDE |
| Tutorial（奋进岛） | 2 | 0 | — | DB25018C |
| Caspiar / Reflection / SC_Hinsara | 6/7/8 | 2/1/2 | （EP1 附加地图，RT0 无对应组） | — |

### 1.2 模板 ↔ 区域组的精确 join：**城市坐标**

城市 uid（"1026"…"1036"）跨区域重合（都是同一段数字），**按 uid join 有歧义**；
按 (x,y) 坐标 join 是精确的——地块表 0xF01DE4B1 与模板 cities[].x/y 逐城相等
（BEAF0510 实测 5/5 全同，1028 差 2m 为模板侧舍入）。探针 `plot_pos_join` /
`region_template_bind`。

### 1.3 与运行时的关系（对拍 dev 源码的修正）

exe 侧伟工站点 = 区域游戏（GlassBox）中带 `0xE6197CB`(bool) 属性的单位，位置 =
单位 `mWorldTransform`（cToolRegion::CreateRegionUnitBorderEffects L1142185、
cRegionCameraController::TransitionToSite L732526）。该 transform 在联机时由服务器
box 状态流下发（kLogActionUnitCreate + SP::ReadTransform L650768）——**但模板
（本节 JSON）就是服务器创建区域时的初始数据源**，离线模式同样从这份数据出发。
城市条目的 `z` = 场地基准面高（海边城市 = −870 正好海平面；认领城市时地形整平到
此高度），伟工条目无 z（3D 预览时从高度图采样即可）。伟工**选址**（Arcology/
太空中心等）是游戏进行中的选择，存于存档/服务器状态，模板里恒为 `Region_GreatWorks_Empty`。

---

## 2. 名字映射——链路全通，可删除硬编码 REGION_NAMES

### 2.1 四级链（全部本地，全部实证）

```
RT0 区域组 (FNV1(数字UI id))
  → 地块表 2B9C480C（names "1026".. = uid + 坐标）     [region-and-map §6]
  → 区域模板 JSON（按坐标 join）→ 模板名 "Confluence"   [§1]
  → 注册表 JS（Game 包 67771F5C:40464200:7CCC548C，3.3MB）:
      Confluence:{ Enabled:1,
        regionName:"AutomatedRegionTemplates.json!0x77cb42ea",
        regionDescription:"AutomatedRegionTemplates.json!0xfbdbe448",
        cities:{1026:{cityName:"...!0xa79638c7", cityDescription:"...!0x0b85715d",
          percentageBuildable:"95", wind:"1", water:"2", coal:"2", rawOre:"3",
          crudeOil:"1", rail:"true", shipping:"true", resources:[]}, ...}}
  → locale JSON（Locale/zh-tw/Data.package，instance = FNV1_lower("automatedregiontemplates")
     = 0x2C1D9BDE，388 条）: { "0x77cb42ea": "白水谷", "0xa79638c7": "貝克曼弗利", ... }
```

### 2.2 验证（zh-tw 实测全命中）

- `0x77cb42ea → 白水谷`（区域名）；`0xfbdbe448 → 区域描述全文`；
- 城市 `0xa79638c7→貝克曼弗利`、`0xae3b915a→白水交地`、`0x42647828→銅灣地`、
  `0x4d7b89b2→榆木林`、`0xf3c76956→河灣`——**逐城命名**，首次拿到。

### 2.3 哈希算法定案（dev 源码函数体）

`EA::StdC::FNV1_String8(s, 0x811C9DC5, kCharCaseLower)`（L1153188）：**FNV-1**（先乘
0x01000193 再异或，非 FNV-1a），逐字节 ASCII 小写化。封装：`SPIDFromName`(L203584)、
`SPGroupIDFromName`(L300665，再 |0x80000000)。locale 文件名→instance 同规：
`FNV1_lower("automatedregiontemplates") = 0x2C1D9BDE` ✓。
注意：运行时**不做**哈希查询——stringID 是离线管线预计算存表；OpenSCP 只需按
`注册表 stringID → locale[key]` 查表，无须自己算哈希（仅新造名字时才需要 FNV-1）。

### 2.4 附带收获：每城资源评分（真数据）

注册表条目含 `percentageBuildable / residentialDesirability / wind / water / coal /
rawOre / crudeOil / tornadoes / earthquakes / rail / shipping`（0–3 档）——
地图面板侧栏与"资源画刷"合成层可用它替换当前的噪声假数据。

---

### 2.5 勘误（2026-10-07）：16 城/7 城区域的旧目验映射有误

真名链落地时发现旧"目验"映射与游戏数据矛盾，用**城市场地整平高逐城判别**
（城市组 256² 高度图均值 vs 模板 z，方法锚点：白水谷=存档 MetaData，真模板
平均|Δ|=37，错误候选 75~395）+ 城市坐标双重验证，定案：

| 组 | 旧目验（"排除法"猜测） | 数据定案（模板名 → zh） |
|---|---|---|
| BC357A2B | 泰坦峽谷 | **三角洲**（Sawyer，pos16+z8，Δ=26.6） |
| B12DE348 | 三角洲 | **藍綠森林**（Gallia，pos16+z10） |
| C04182E4 | 藍綠森林 | **泰坦峽谷**（LittleGorge，pos16+z11，高山场地 -160s 吻合） |
| E41A82B8 | 愛華特灣 | **反影環礁**（Reflection，z=23.8 vs 81.9） |
| A0B60DDE | 荒涼 | **愛華特灣**（EdgeWaterBay） |

- 三张 16 城图布局**不同**（早期"同布局"是误判），坐标本身即可区分；
- 荒涼（SC_Desolation）场地 z 要求 -600~-135m 高地，RT0 无匹配——其地形不在
  RT0（与绵延不毛 Oasis 同在 RT1）；SC_Desolation/Desolation 两条注册表键同指荒涼；
- 内部代号 ≠ 零售名（Sawyer=三角洲、LittleGorge=泰坦峽谷、Gallia=藍綠森林），
  zh 名以注册表 regionName stringID → locale 为准；
- 注册表还有 CliffsideVista(摩崖街)/FlocksRiver(Monument Bay)/NewDenmont/
  Tundra/Caspiar(寧靜群島)/SC_Hinsara(翠綠叢林) 等未上架模板。

## 3. 着色——引擎地面着色链与 OpenSCP 修正清单

### 3.1 引擎侧（dev 源码函数体实证）

地形绘制 = 每地层一个材质实例；地面材质 `cMaterialGround`（变体
`terrainRegionMaterial` / `terrainDataViewBlendColors(Contours)Material`）：

| 要素 | 内容 | 源 |
|---|---|---|
| 纹理槽 | 0=combinedEcoMapRT、1=terrainNoise、2=controlMapRT、7=overlay、8=base、9=baseNRM、10=overlayNRM（地面集 slot4=cubeMap、slot6=combinedEcoMaps） | cGroundTextureSet::Update L252415 |
| 层栈色调 | **0x24025DB1 vec4[地层]**：zeros=(1,1,1,0)、ground=(**0.48,0.39,0.27**,0)、water=(0.7,0.7,0.7,0)，经 `cMtlUpdateData.mColor` 进 shader 常量块 642 第 4 个 vec4 | cTerrainStrata::FillFromProps L238056 / cTerrainStratum::UpdateShaderData L254112 |
| cube 细节 | 6 面：`terrain_dirtdetail/_snow/_grassdetail/_pollution/_cliffx/_beach`(+`_NRM`)，type=65282541；环境属性提供 key 时覆盖（区域 desc `0x2687C033`=diffuse cube、`0xEDB0E3AE`=normal cube——**主包无此 instance**，疑在 GraphicsCache/SimCity.par，遗留） | cMaterialGroundType::Init L252465 |
| ecoMap | 每层 BF76959D key（typed map 索引）→ CPU GB::cEcoMap + GPU 纹理（属性 0xAF3DAC7 image），合成进 combinedEcoMapRT（组合器材质 FNV 0x98BC9B06）；区域视图的 9 张 typed map 由画刷运行时盖章（procedural），**静态侧只有 ED 场** | cTerrainEcoMap::Init L254799 |
| PS 公式 | 三色插值 `lerp(lerp(dry,grass,grassAmt),polluted,ecoMaps.a)` → **平方** → 与 lot 色 lerp；grass 来自 `getGrassAmount(combinedEcoMap,noise,normal)` | shader 容器（map-ground-coloring §一） |
| 常量块 | 642=cShaderDataTerrainInfoPS{mWaterInfo,mReflectionParams,mNormalParams,mColor}；659=terrain 块；658=等高线（数据视图） | cMaterialGround::UpdateShaderData L253114 |
| 环境 | envID=mRegionKey.mGroup → `GetPropertyList(0x51F2BBED, envID)` 读 sea level 0x0E163A5A；`heightMapMinMax`(0xC7394CF1)/`forestMapMinMax`(0xEFE654C7) 分层（区域 desc 旁实测 341 条 vec2） | GetRegionEnvironmentID L207138 等 |

### 3.2 OpenSCP 修正清单（2D 与 3D 共用）

1. **绿地主色**：`grass = ED b1 / 255`（b1=草量，已证），基色用引擎三色公式的
   干地/草/污染三色——取自 shader 材质常量（需从 Game 包材质容器提取默认值，
   或先用官方截图标定），**插值后记得平方**（gamma 型提亮，缺失即"着色不对"主因）。
2. **整体色调**：地面基色乘层栈 tint `(0.48,0.39,0.27)`（ground 层）——当前实现无此步。
3. **岩石**：ED b2 材质权重 < 阈值 → cliff 分支（`terrain_cliffx` 面）；雪线 `snowLevel=+1024m`、
   水面 `seaLevel=−870m`（默认常数已从引擎拿到）。
4. **路面**：ED b0==15 细线（现实现已用 b0==0，语义待复核为 b0==15）。
5. **水面**：保持全局平面 + 深度渐变（现实现已对）；潮汐参数在 tessendorfWater
   property（区域组内，`2FFD7EED`，18 个 float，可后续做波浪动画）。
6. **城市块**：区域视图城市 = 4 顶点 billboard（V3F_N3F_G3F_C4B_T4F_I4B，60B），
   base/overlay 贴图 + 8 组 RGBA 颜色来自 unit NonSimProps（0xBBD5875/0xCCB7FD4/…），
   UV1=ecoMap 内位置、UV2=世界坐标平铺——3D 预览城市块可用单色框 + 名称起步。

---

## 4. 3D 预览——引擎重建方案对拍

### 4.1 引擎怎么做（全部有函数体）

- **网格**：区域 32768m（±16384，mCellSize=regionSize/2^maxSubdiv → 4096 格 @8m）。
  四叉树节点 = **2048m 方块**（`cTerrainGrid::smLocalSpaceSideLength=2048`，L165718），
  每节点 6 张网格：fine `(v−1)²`、coarse 半分辨率、water、两条 2 行 skirt；顶点格式
  **V4F**（x,y∈[−1024,1024]，z=0；奇数列 z=1、i%4==1→w=1、i%4==3→w=−1 为裂缝/裙边
  标志）；**z 全在 VS 里从高度图纹理位移**。
- **高度页**：SVT 256² 页池（20 页，5 mip = 341 tile 的运行时形态），页 min/max 存
  LUT（`bias+minmax×2048`，默认 −1024..+1024）。
- **VS 常量**（shader data 0x293）：seaLevel、网格边长、页 UV scale/offset、
  gridSpacing、`[14]=1<<pageMip`、`[15]=16>>pageMip`（=2 texel×8m 的法线差分步长）。
- **法线**：GPU 全屏四边形 pass（`TerrainNormalsMaterial`，FNV1("TerrainNormalsMaterial")
  L231028），中心差分 + normalize，步长常数 16>>mip，输出 A8R8G8B8。CPU 侧不需要复刻
  其编码，JS 端 `n=normalize(−dhdx,−dhdy,1)`（h 以米计）即可。
- **CPU 高度查询**：`z = raw/65535×2048 − 1024`（=raw/32−1024，编辑器 tile 路径
  L225627 与 VS/shader 三方一致）；世界→格 `(world+16384)/8`，格心双线性。

### 4.2 three.js 重建路线（数据 100% 已有）

1. **几何**：复用 `region_map.rs` 的 341-tile 拼合（4096² u16）→ 建议按 tile 分块：
   16×16 个 2048m 块，每块 128² 或 256² 顶点（LOD 档位），顶点 z=raw/32−1024；
   块间接缝用共享顶点行或 skirt 条（引擎同款思路）。整图缩略可用 256²/512² 单网格。
2. **法线**：中心差分（步长 8m），`geometry.computeVertexNormals()` 亦可但引擎语义是
   差分，建议手算保方向一致。
3. **着色**：顶点色或 Canvas 纹理 = §3.2 公式（ED b1 草量 + b2 材质 + 平方 + tint），
   或将现 2D 渲染器输出直接贴为地形纹理（快路径），公式路径作对拍。
4. **水**：全局平面 y=−870m，半透明 + 深浅渐变（现 2D 的 d 渐变搬过来）。
5. **城市块**：地块表 (x,y) + 模板 z（场地基准面）画 2048m 线框/顶盖；进阶 = 用
   城市 95 张 256² 高度图在城市框内替换区域地形（`cRegionCityLots::Render` 的对等物）。
6. **伟工位**：模板 greatWorks (x,y) + 高度图采样 z 放标记（与城市框同层）。
7. **坐标系**：世界 x→three x、世界 y→three −z（俯视保持北向上需与 2D 渲染一致，
   F0=identity 朝向已证）。

### 4.3 验收对拍

白水谷（BEAF0510）3D 渲染 vs 官方图鉴 45° 等距图：Y 形汇流河、5 城地块、
伟工位 (2684,2773) 落在右下山上——三项同构即通过（与 2D 版同一判据）。

---

## 5. 新增探针（本轮全部，crates/sc-properties/examples/）

| 探针 | 用途 |
|---|---|
| `find_tgi.rs` | 跨包按 instance 搜索（任意 type/group） |
| `dump_one.rs` | 按 TGI 转储单条资源 |
| `pkg_survey.rs` | 包类型直方图 |
| `json_grep.rs` | 扫包内 JSON 资源按关键词过滤+转储（伟工破案主力） |
| `region_template_bind.rs` | 模板 JSON ↔ 地块表配对 + 伟工位输出 |
| `plot_pos_join.rs` | 地块表 names/ids/positions 全字段（坐标 join 实证） |
| `city_z_verify.rs` | 模板 z vs 高度图采样 z（全链路一致性） |
| `strata_env_dump.rs` | 层栈模板族 + 区域 desc 全文 |

## 6. 遗留与下一步

- [ ] **cube 细节图载体**：terrain_dirtdetail 等 6 面 + 区域 desc 两个环境 cube
      instance（0x55ACFBD0/0x176FBADA）主包未命中——查 `SimCityUserData/GraphicsCache.package`
      与 `SimCity/SimCity.par`；
- [ ] **着色三色调色板默认值**：从 Game 包材质容器（terrainRegionMaterial）提取
      dry/grass/polluted 三色常量（或官方截图标定），落地 §3.2 公式；
- [ ] **3D 预览 PoC**（three.js）：按 §4.2，先白水谷单区域；
- [ ] **名字链落地**：region_map.rs 删 REGION_NAMES 硬编码 → 解析注册表 + locale
      （新增 Tauri 命令读 Game/locale 包）；顺带接每城资源评分；
- [ ] 伟工空地显示名 stringID 0x0EAA3965 不在 region-templates locale 文件（在 UI
      locale 另一文件），显示名落地时一并解；
- [ ] 修 region-and-map.md §4.1 的"+1 伟工位"勘误（本文 §0 已记）。
