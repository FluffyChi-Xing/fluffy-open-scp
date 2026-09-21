# 区域与地图机制 — RegionTerrain / 城市地块 / 可玩边界

> 调查笔记（2026-09-22）。动机：BoC（区域外可建设模组）只移除了可玩边框，
> 但界外建造存在「摆放漂移 / 无地下资源 / 建筑埋地穿模」三类缺陷——本文从
> 数据与引擎两侧厘清地图机制，回答「大地图如何切成小地图」「地图尺寸是
> 引擎写死还是数据驱动」，并给出 OpenSCP 的可行路线。
> 工具：新增探针 `crates/sc-properties/examples/region_probe.rs`
>（survey / dump / props / hstats / tilecheck / extract 六个子命令）。
> 置信度标注沿用 [glass-box/terrain.md](./terrain.md)：**[高]**=字节级实证，
> **[中]**=结构推断，**[低]**=推测。

## 1. 数据源盘点 [高]

| 包 | 内容 |
|---|---|
| `SimCity_RegionTerrain0.package` | F0 3846 条（504 MB）· ED 3751 条（245 MB）· property 282 条 → **基础游戏 11 个区域** |
| `SimCity_RegionTerrain1.package` | F0 1399 条 · ED 1364 条 · property 111 条 → **EP1/DLC 4 个区域** |
| `SimCity_Game.package` | 地块地形层栈模板（group D7EF2862 族）等 |

类型语义（`verified_type_name`）：
- `0x03E421F0` **Terrain Heightmap (16-bit)**：解压恒为 **131,092 B = 20 B 头 + 256×256 × u16 LE**；
- `0x03E421ED` **Terrain Field Map**：≈65,558 B = 头 + 256×256 × u8；
- property（0x00B1B104）承载全部区域/画刷/地块描述。

## 2. 高度图解剖 [高]

头部 20 B：`00 00 00 00 | 00 00 01 00 | 00 00 01 00 | 00 00 00 07`——
65536（=0x10000，格子总数）与 256 出现于定长字段，末尾 u32=7 疑为格式 id。

数值实证（hstats，u16 LE）：

| 地块 | min | max | 特征 |
|---|---|---|---|
| BC357A2B:D6DCB06E | 2155 | 5399 | 海岸城（基准面 ≈2000+） |
| A0B60DDE:EB0FF21B | 3021 | **16218** | 山地 |
| BEAF0510:DB206CE6 | 0 | 0 | **全 0**（未使用/预留地块） |
| BEAF0510:AB991ED8 | 0 | 25573 | 陡崖（nonzero 仅 2719 格） |

垂直比例（u16 → 米）未定（见 §11）；水平方向：地形格边长 8 m（terrain.md §2.6，
LotMask 96 m = 12 格 × 8 m 互证 [中]）。

## 3. 区域组织：group ≈ 区域 [高]

- RT0 的 F0 按统计落为 **11 个"大 group"（各 341 张高度图）+ 95 个"单条 group"**；
  ED 恰为 11 个大 group × 341（无单条）——**每区域 341 对 (F0 高度图 + ED 地面场)**；
- 每个大 group 恰有一条 **`…:51E7A18D` "region" 描述 property**（34 键）：
  常量（1024、-870、32768、40、60、2/5/6、种子 257368037、布尔）+ **~14 个 Key
  引用子资源**——正好对应 [terrain.md](./terrain.md) §1.2 的 **13–14 张 typed map** 槽位；
- 子资源全家福（按 `0x00B2CCCA` 名称字符串）：
  `heightmap`（地形画刷清单）· `watertableheightmap`+`waterTableEcoMapBrushes` ·
  `coalheightmap`+`coalEcoMapBrushes` · `oreheightmap`+… · `oilheightmap`+… ·
  `soilheightmap`+… · `forestheightmap`+… · `desirabilityheightmap`+`desirabilityEcoMapBrushes` ·
  `desirabilityTwo…` · `tessendorfWater`（水面参数）· `terrainEditor` · `grass` ·
  `environment` · **城市地块表**（§6）。
- 95 个单条 group = 各区域城市地块的独立资源 id（§6 的 uint32 表引用它们，
  95 ≈ 11 区域 × 8.6 块/区域，与地块表 11 块/区域量级吻合 [中]）。

## 4. 区域大地形 = 画刷盖章拼图 [高]

地形画刷清单（例 BC357A2B:3BABB8DB）：

```
0x00B2CCCA string8 = brushes          ← 画刷清单
0x02A907B5 Key[2]  = C175ACEA, 282078BF   ← 指向本区域 group 的 F0 高度图块
0x02A907B6 transform[2] = 平移 (813.29, 2168.00) / (-2035.99, -203.31)
0x0DBA3A9C string8 = heightmap        ← 目标 map 名
0x0DC097E3 uint32  = 256              ← 贴图分辨率
```

**边缘连续性检验（tilecheck）**：同区域相邻块 EB0FF21A → EB0FF21B

```
A.right vs B.left   avg|Δ| = 22.2   ← 与 A 自身 right/left 差 24.2 同级 → 连续！
A.left  vs B.right  avg|Δ| = 6520.4 ← 反向/上下缘 514~3019 → 不连续
```

### 4.1 自动拼合（jigsaw）：区域大地图重建成功 [高]

互为最优（mutual best-match）边缘匹配 + BFS 铺放，对区域 DB25018C 的
341 张 F0 tile 自动拼合：**320 块进入 19×17 网格（仅 3 空槽），21 块为
噪声/全零板未匹配**。拼合结果（![区域 DB25018C 拼图](region-db25018c-mosaic.png)）
呈现连贯地形——河谷、山脊、海岸线跨 tile 连续延伸。

结论：**区域大地形 = 19×17 tile 网格（≈341 块 256×256 u16 高度图 + 同数
256×256 u8 地面场图）的马赛克**，引擎渲染端按 `heightmap_x%02d_y%02d_mip%d` /
`ecomap_x%02d_y%02d_mip%d`（exe 字符串，SCY dump 0x99dc58）分块 + mip 缓存。

## 5. 资源分布（EcoMap 画刷）[高]

每区域 9 类资源/覆盖图，全部走**同一套画刷机制**：

```
0x02A907B5 Key[N]    = 128×128 位图（ED/相关资源）
0x02A907B6 transform[N] = 世界摆放（平移 + 旋转 + 强度）
0x0DBA3A9C string8   = 目标 map 名（"coalheightmap" / "watertableheightmap" / …）
0x0DC097E3 uint32    = 128（分辨率，地形为 256）
0x0DE43899 uint32    = 目标 map 索引：watertable=2 · oil=4 · ore=6 ·
                       desirability=8 · desirabilityTwo=9 …
```

- 索引直接对应 [terrain.md](./terrain.md) §1.2 的 typed map 槽位——**13–14 张
  map 的写入路径是"画刷→索引→map 槽"**；
- 水：`tessendorfWater` 参数组（500/20/0.1/5/…）+ watertable 高度图；
- **资源是"画"上去的**：画刷没涂到的格子恒 0——这就是界外无矿/无水的数据根源（§9）。

## 6. 城市地块（city plot）[高]

地块表（例 DB25018C 区引用的 D27E9966:2B9C480C，11 块）：

| 字段 | hash | 实证 |
|---|---|---|
| 名称串 | 0x0543BC96 | "1026", "1027", … "1036" |
| 地块 id | 0x16B7B1EF | 0xC1A84CC8 / 0x9E14B10D / **0xFFED13FB**…（= 单条 group 的 id） |
| 模板 Key | 0x9F2F9B65 | → D7EF2862:6F9F6AB3（全部同键） |
| 旋转 | 0xC0B13E7E | float[11] = 0 |
| **位置** | 0xF01DE4B1 | vec2[11]：(-2288,6656) (-480,3376) (2512,3616) (3744,6208) (-6704,5232) (-3408,1184) (-736,-224) (-352,-5136) … |

- **地块位置是区域世界坐标**（跨度 ±6700 m → 区域世界 ~13 km 见方）；
- 模板 `D7EF2862:6F9F6AB3`（SimCity_Game.package）= 地形层栈：`ground/soil/water`
  三层（层名 "zeros"/"ground"/"water"、贴图引用、颜色 (0.48,0.39,0.27)、启用标志）
  ——**定义了每个地块的地层构成，不含尺寸数字**。

## 7. 城市可玩尺寸与比例尺 [中]

- 城市模拟格 = **256×256 = 65536 格**（引擎常量 0x10000，terrain.md §2 [高]）；
- 格边长 = 8 m（terrain.md 假设 + 本文 LotMask 96 m = 12 格互证 [中]）
  → **单城可玩地面 ≈ 2048 m × 2048 m（≈4 km²）**，与社区公称"2km×2km"一致 [外部]；
- 区域世界（画刷/地块坐标）跨度 ~13 km 见方，即区域 ≈ 40+ 个城市格的地理范围，
  但其中**只有地块表登记的地块可玩**。

## 8. 可玩边界（citybox）在哪一层 [高]

- BoC（Build Outside Citybox）的安装物 = **替换 EcoGame JS 脚本包**
  （SimCity-Scripts_272391411.package，6.3 MB 高熵 bundle，非明文）+ 属性/资源覆盖包；
- 替换脚本即可解除界外放置限制 ⇒ **边界检查位于可替换的 GlassBox 脚本/数据层，
  不是引擎硬约束**；
- 引擎只提供渲染与格网原语：`cRegionCityLots::Render`、`cRegionDecals::Render`、
  `cTerrainLayer::DrawLayer`、`cTerrainHeightMap::UpdateHeightMap/UpdateNormalMap`、
  `bindCurrentHeightMap(AsTarget)`（SCY dump 字符串）。

## 9. 判定表：尺寸与限制，哪些写死、哪些数据驱动

| 层 | 内容 | 载体 | 可改性 |
|---|---|---|---|
| 引擎 | 地形图分辨率 256×256（0x10000 常量） | EXE（FUN_00bdd210） | 二进制级 |
| 引擎 | typed map 槽位数 13–14、格网原语、渲染管线 | EXE | 二进制级 |
| 引擎[中] | 格边长 8 m、垂直比例常量 | EXE float | 二进制级 |
| 数据 | **区域数量/名称/地块位置/地块表/资源分布/水参数** | RegionTerrain property | **纯数据，可覆盖可新增** |
| 数据 | 界外格的资源/高度（当前 = 空白） | 同上（没画就是 0） | **纯数据，可补** |
| 脚本 | 可玩边界检查、放置规则 | EcoGame JS bundle | 脚本层（BoC 已证可改） |

**结论**：单城的地形分辨率（256×256）与格边长是引擎侧常量；「有多少地、地在哪、
地上有什么」全部是数据。做大地图的正确姿势不是改引擎常量，而是**造数据**。

## 10. 用该机制解释 BoC 的三个缺陷

1. **界外无地下资源**：coal/ore/oil/watertable 等 eco map 只在地块范围内被画刷涂过；
   界外格子在这些 map 上恒 0 → 模拟判定"无资源"。BoC 没有界外 eco map 数据。
2. **摆放漂移**：道路/建筑的吸附格网以城市原点为基准定义；界外坐标不在城市格网
   定义域内，吸附/寻址行为未定义 → 漂移。
3. **建筑埋地穿模**：城市内地形 = 城市 256×256 @8m 高度场；界外地形 = 区域级
   拼图 tile（不同分辨率/基准）。建筑落地采样两种高度源不一致 → 视觉穿模。

⇒ **方向判定**：BoC 的路线（拆边框）只解决了"能不能放"，没解决"放上去之后
世界数据是否自洽"。要做成完整的大地图，需要数据工程三件套：
①界外 eco map 补画（矿/水/土/森林）；②界外高度源对齐（区域 tile ↔ 城市高度场
的重采样或整体替换）；③地块表/格网定义扩展。这三件都是 package 数据工程——
正是 OpenSCP 的能力圈（探针 + 画刷机制已可编程生成）。

## 11. 复现命令

```bash
cargo run -p sc-properties --release --example region_probe -- \
  survey  <package>                       # 类型直方图
  dump    <package> <type-hex>            # 条目 TGI/尺寸/头部
  props   <package>                       # 全量 property 键值
  hstats  <package> 03E421F0              # 高度图 min/max/avg
  tilecheck <package> <instA> <instB>     # 边缘连续性（拼图判定）
  extract <package> <instance> <out>      # 提取资源
```

## 12. 遗留与下一步（2026-09-22 更新）

### 12.1 待解问题

- [ ] **垂直比例**（u16 高度 → 米）：需引擎常量。Ghidra 静态分析受限——
  `SimCity.exe` 加壳（.text 熵 8.00，见 file-formats.md §7），须先动态脱壳再
  反编译 `cTerrainHeightMap` / `cShaderDataTerrainRegionVS` 取比例常量；
- [ ] **tile 世界尺寸**（256 格 tile 的米数，4 m/格 vs 8 m/格两说）：同上，或
  通过存档（save）内城市高度图与区域 mosaic 的对比采样间接定标；
- [ ] **citybox 可玩边界逻辑定位**：在 EcoGame 脚本 bundle（§12.2）的 ER2 数据
  或其解包后的 JS 中，找界外放置检查的实现；
- [ ] **ER2/JS bundle 容器格式**：见 §12.2，解开后可 diff BoC 的具体改动；
- [ ] 341 块/区域的构成记账（地形 stamp vs 各 eco map stamp 的条数分布）；
- [ ] `2B9C480C` 的 uint32[11] 与单条 group id 的精确对应关系验证；
- [ ] 资源预览缺失 TGI 时**引导用户打开游戏包**的 UI（note-mublqhrd）。

### 12.2 EcoGame JS bundle 对应文件 [高]

- **游戏原版**：`<游戏目录>/SimCityUserData/EcoGame/SimCity-Scripts_<补丁号>.package`
  （EcoGame 目录现存 13 个补丁版本；当前离线 10.1 = `SimCity-Scripts_272391411.package`）；
- **目标资源 TGI = `08068AEB:40800200:622B9CD7`**（type = ER2 Binary Rule File，
  注册表名；解压 6.3 MB，压缩存储 ≈1.0 MB）；同包另有
  `00B1B104:40800200:622B9CD7` property 伴随表；
- **BoC 模组的替换物** = 同 TGI 的解压版（6,323,736 B，高熵）——即 BoC 的
  「边界解除」是对这份编译后 GlassBox 规则/脚本数据的修改；
- 容器静态无明文（无 ≥40 字符 ASCII 串），解码需按 GB 流处理器
  （docs/source-code/GB_cIEcoStreamHandler.c）或动态脱壳还原；
- 定位命令：`cargo run -p sc-properties --release --example find_instance --
  622B9CD7 <package...>`。

### 12.3 工程待办（OpenSCP 侧）

- [ ] 资源预览缺失 TGI（LOD/ LotMask/Lot Textures）时提示并引导打开游戏包
      （note-mublqhrd，相关诊断已在 lot/LOD 解析链路）；
- [ ] M-CM3：code 工作台文本编辑保存（code_write_text 接 UI）+ mod 打包导出；
- [ ] Raster 绘制 / property 编辑器作为底座能力嵌入模组工作台；
- [ ] 大地图数据工程 PoC（§10 三件套）：界外 eco map 补画 + 高度源对齐 +
      地块表扩展。
