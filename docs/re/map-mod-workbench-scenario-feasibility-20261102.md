# 地图 Mod 工作台三场景可行性分析（需求导向）

> 分析日期：2026-11-02
> 对象：`Code 工作台` → `开发流程` 中 `模组类型=地图` 的三个 workflow
> （`original` / `whole-region` / `generated`，见 `src-tauri/src/mod_flow.rs:335-361`）
> 依据：工作台现有代码、`crates/sc-properties` 现有读写能力、本轮之前的
> 反编译与包实测（`map-contract-citybox-camera-light-20261102.md` 等）。

一句话结论：

- **场景 1（原地图编辑）**：三项都可实现，且**不需要动引擎**；两项缺的只是写
  工具（属性/词条/地块表），已有 `PropertyFile::encode_canonical` 与
  `locale::serialize_locale_items` 可直接复用。最大风险是“槽位要三处同步”。
- **场景 2（单一大地图）**：**部分可实现**。“选中 citybox → 屏蔽其他槽位 →
  边界扩展到整区 → 外部坐标对齐 → 资源画刷”里，前两步与坐标换算可做；
  “让模拟真的接受整区”被 **`flatScale` / eco 分辨率 / 相机绝对常量** 三处硬
  约束卡住，必须做运行时验证，不能承诺。
- **场景 3（新地图创建）**：**高度生成已可，完整新地图不可**。差 5 个 writer
  （ED 场、地块表、模板 JSON、注册表、区域注册）与 1 个尺寸链决策；建议先做
  “替换式新地图”（复用现有 group），把“全新 group 注册”放到最后。

## 1. 工作台现在到底能做什么（代码事实）

| 节点 kind | 配置项 | 校验 | 构建产出 |
|---|---|---|---|
| `map-source` | `package`、`group` | 源包含该区域 341 张 F0 tile | 无（只作输入） |
| `height-edit` | `delta_meters` | ±1024 m | 341 张 F0 高度 tile overlay |
| `noise-height` | fBm 参数 / 4096² 灰度图 | 参数范围 + 图尺寸 | 同上（整幅替换高度） |
| `static-resource` | 项目内文件 + TGI | 文件存在、TGI 合法 | 任意资源按 TGI 原样覆盖 |
| `city-slots` | `action` / 城市与伟工 ID | **`action != preserve` 一律阻断** | **无** |
| `resources` | 画刷文件 / target / blend / strength | 同上 | **无** |
| `roads` | — | 同上 | **无** |
| `map-metadata` | 名称/介绍（中英）/ 选择图 | 同上 | **无** |
| `coordinate-alignment` | `action` | **恒阻断**（“需游戏验证”） | **无** |

证据：`mod_flow.rs:842-853`（`city-slots|resources|roads|map-metadata` 只能
`preserve`）、`mod_flow.rs:963-1041`（build 只处理 `building-asset` /
`static-resource` / `height-edit` / `noise-height`，其余 `_ => {}`）。

也就是说：**当前地图 workflow 实际只产出“高度”与“整包资源替换”两种东西**，
其余节点是已经画出来的 UI 与已经定死的 schema，缺执行侧。

已有可复用的写入器（这是三个场景的共同地基）：

| 能力 | 位置 | 说明 |
|---|---|---|
| 任意 property 重编码 | `sc-properties::PropertyFile::encode_canonical` | 改字段后整资源重写 |
| 词条表写回 | `sc-properties::locale::serialize_locale_items` | 0x0A98EAF0 文本表 |
| 341 F0 高度 tile | `sc-properties::region_write::build_heightmap_overlay` | 已在 build 中使用 |
| 区域 EGB/状态 | `sc-properties::region_state::{write_state, write_state_gzip}` | 含 curves / units |
| 覆盖包组装 | `dbpf::OverlayEntry` + `write_uncompressed_overlay` | 已用于所有 mod |
| 区域/地块/资源读取 | `region_3d`、`region_map`、`region_bootstrap`、`region_city` | 读侧齐全 |

## 2. 三场景总矩阵

| # | 用户操作 | 目标资源 | 现状 | 判定 |
|---|---|---|---|---|
| 1a | 选中地图 → 选中城市槽位 → 屏蔽 | 地块表 + 模板 JSON + 注册表 | 读齐，写零 | **可做**（三处同步） |
| 1b | 改单个城市的地下资源 | 区域画刷属性 / 城市 eco map | 读齐，写零 | **可做**（先做区域画刷参数层） |
| 1c | 改城市选择区词条 | locale 0x0A98EAF0 + 注册表引用 | 解析+序列化齐全 | **可做**（最省力） |
| 2a | 选中 citybox → 屏蔽其他槽位 | 同 1a | 同 1a | **可做** |
| 2b | citybox 扩到整个区域 | `PlayArea` 0x07E0AC05 + `mMapBounds` + `flatScale` | 计划器已有，写入未接 | **条件可做**（需 eco 决策） |
| 2c | 自动对齐盒外坐标 | 三帧换算 + 灯/贴花/相机 | 换算与审计已实现 | **可做**（工具侧） |
| 2d | 资源画刷为区域加地下资源 | 画刷属性 + ED/eco 场 | 读齐，写零 | **部分**（ED writer 缺失） |
| 3a | 噪声生成地形 | F0 341 tile | 已实现 | **已可** |
| 3b | 放置区域道路 | EGB curves/units + PathEntry | EGB 可写，编辑器零 | **部分**（需道路编辑器） |
| 3c | 放置城市槽位 | 地块表 + 模板 + 逐城 bootstrap | 读齐，写零 | **可做但链路长** |
| 3d | 绘制资源 | 画刷属性 + ED 场 | 同 2d | **部分** |
| 3e | 编制选择词条/图片 | locale + 选择图 raster | 词条可写，图片写入待接 | **可做** |
| 3f | 全新地图注册（新 group） | 区域 desc + 模板 + 注册 + 引导 | 全部要新建 | **暂不可承诺** |

## 3. 场景 1：原地图编辑

### 1a 槽位屏蔽（最高风险，必须三处同步）

现有证据链（`docs/re/map-consumers-ui-bridge-camera.md:16-24`）：

| 消费者 | 输入 | 只改一处的后果 |
|---|---|---|
| launcher `ShowTemplateDetails` | 模板 JSON `map.cities.length` | 创建页城市数不变 |
| `cMyGamesUIHandler.RemapTemplateToRegion` | 要求 `boxes.length == map.cities.length` | 索引错位/拒绝映射 |
| 游戏内 `ReceiveRegionData` | 引擎返回的 `a.boxes` | 仍显示旧槽位 |
| `GetCityStubDescription` | 注册表 `cities[uid]` → locale | 槽位在但没名字 |

目标资源：

| 资源 | TGI / 属性 | 读侧代码 |
|---|---|---|
| 地块表 | `0x00B1B104 : F0E2E39C : 0x2B9C480C`（表 group ≠ 区域 group，需按“城市组交集”选表） | `region_3d::plot_table` |
| 逐城绑定 | 城市属性 `0xC1949C4D` → `0x51E7A18D` + region group | `region_3d::plot_table` L303 |
| 模板 JSON | `0x0A98EAF0 : <模板组> : 0x4529F96F`（EP1） | `region_3d::parse_region_templates` L401 |
| 注册表 | `0x67771F5C`（launcher `0x696447EA` / 游戏 UI `0x7CCC548C`） | `region_city::registry` |

**地块表实际有 5 条并行数组**（CapeTrinity 实测，3 城各 5 项）：

| 属性 | 类型 | 实测内容 |
|---|---|---|
| `0x0543BC96` | string8[] | uid 串（`1026`/`1027`/`1028`） |
| `0x16B7B1EF` | uint32[] | 城市实例 hash（**不是** uid 数字） |
| `0x9F2F9B65` | Key[] | 地层/地形模板引用（实测同为 `D7EF2862:6F9F6AB3`） |
| `0xC0B13E7E` | float[] | 未命名 float 列（实测全 `0`，疑似朝向/权重，待确认） |
| `0xF01D_E4B1` | vector2[] | 世界坐标（1600,-1856 等） |

实现要点（可直接照做）：

1. **五条并行数组必须同索引删除**；少删一条就是索引错位，比“删不掉”更糟。
   读侧只用了其中三条（uid/id/坐标），这正是容易踩空的地方。
2. 模板 JSON 的 `map.cities` 与 `boxes` 同步删同一元素；`boxes` 由 launcher JS
   消费（`ShowTemplateDetails` / `RemapTemplateToRegion`），**不要只删 `boxes`**。
3. 注册表按 uid 删 `cities[uid]` 条目；两份注册表变体都要处理（历史坑见
   `boc-field-diff-and-map-expansion.md:83-97`）。
4. 验收必须用**新区域选择 + 新存档**：老存档里的城市 EGB 不会因为地块表变了
   而消失（`boc-...:99-100`）。

工作量与风险：写入器只是“数组删除 + property 重编码”，**低**；风险在“三处不同
步”与“旧存档残留”，需要 UI 明确提示。

### 1b 单个城市地下资源

这里有两个不同层，工作台 `resources` 节点目前只指向第二层：

| 层 | 数据 | 生效时机 | 可操作性 |
|---|---|---|---|
| 区域画刷参数 | `0x00B2CCCA`（`coalEcoMapBrushes`…）+ `0x02A907B5` + `0x02A907B6` + `0x0DBA3A9C`（目标 map）+ `0x0DE43899`（typed-map 索引） | 区域 bootstrap 时由引擎程序化生成 stamp | 改参数**只在新建区域**生效 |
| 城市资源场 | 城市 eco map 图像（map entry `0x07E0AE01/05/06` → 图像资源） | 城市认领/读档时初始化 | 直接改数值，**必须写图像** |

关键事实（`region-load-road-resource-lifecycle.md:57`）：**画刷图像不在包里**，
引擎按区域种子和画刷参数生成。所以：

- “给某个城市加一片煤”最可靠的路径是**直接改该城市的资源场图像**，而不是改
  区域画刷（后者会影响所有新区域且不可精确控制形状）；
- 改图像需要 `u16/u8 raster writer` + 正确的 environment 绑定（`cEcoMap::Create`
  先按当前 environment 覆盖 group 查，失败再回退，见 `map-consumers...:43`）。

> 待复核：`map-editor-pdb-breakthrough-20261030.md:82` 说城市 eco map 图像类型是
> `0x03E421F0`/`0x03E421EC`，但 `0x03E421F0` 已实测是 **F0 高度 tile**
> （`region_write::F0_TYPE_ID`，341 张 256² u16）。两者不可能同为一种资源，
> 实现前必须先用真实包确认城市资源场图像的真实 type id。

### 1c 选择区词条

- 词条本体：`0x0A98EAF0` 文本表，`parse_locale_items` / `serialize_locale_items`
  已有，**这是三个场景里最快能闭环的一项**。
- 注册表里的 name/description 是 string ID 引用（`ParseLocaleString`），改词条
  即可，不必改注册表。
- 选择界面图片走 raster 覆盖（`raster_edit.rs` 已有编辑面），但要确认它消费的
  是模板字段还是注册表字段（`map-editor-flow-requirements.md:27`）。

## 4. 场景 2：单一大地图

### 2a/2b：把 citybox 扩到整区——真正的约束不是边界，而是分辨率链

已实测（本轮前一份文档）：`flatScale = 1024` 且 **eco 定义域 = 城市盒半边**；
`PlayArea` 只是 `mPlayArea`（`0x07E0AC05`）的一个 AABB，边界网格与可视世界盒都
跟随它（`cGraphicsCityBoxBoundary::StartGame`、`cGraphicsGame::StartGame`）。

区域尺度事实：区域 32768 m ↔ 4096² 高度场 @ **8 m/px**；城市 2048 m ↔ 256²
城市高度场 @ 8 m/px；eco 2048 m ↔ 128² @ **16 m/px**。三者是同一套 8/16 m
栅格的不同裁剪。于是“单城占满整区”意味着：

| 量 | 现在 | 整区单城 | 倍数 |
|---|---|---|---|
| 城市盒半边 | 1024 m | 16384 m | 16× |
| `flatScale`（必须 ≥ 半边） | 1024 | 16384 | 16× |
| 高度场 | 256²（城市）/ 4096²（区域） | 4096²（城市） | 256× 城市 |
| eco 网格（若保持 16 m） | 128² | 2048² | 256× |
| eco 网格（若保持 128²） | 16 m/格 | 256 m/格 | 精度降 16× |

三种可行组合：

| 方案 | flatScale | mMapBounds | eco 格 | 代价 |
|---|---|---|---|---|
| A 保持精度 | 16384 | 2048 | 16 m | 模拟格 4.19 M（≈256×），载入/存档/寻路未验证 |
| B 保持格数 | 16384 | 128 | 256 m | 精度降为 1/16，道路/分区格网变粗 |
| C 折中 | 16384 | 512 | 64 m | 4× 精度损失、16× 格数 |

**没有任何一个可以只靠改 `PlayArea` 完成**：`flatScale` 是独立属性
（`0xB8F6B4CF`），`mMapBounds` 决定 `mPlayCell` 与路径格网，三者必须一起改，
且都只能在运行时验收。

#### 4.1 `mMapBounds` 的真实来源（本轮新证据）

`GB::cEcoGameScripts::Init`（source-tree `GB/cEcoGameScripts/Init.c:277-301`）：

```c
// 主路径：属性 0x0D4AE7FE 的 int 数组（≥2 项）直接作为 mMapBounds
GetPropertyAsIntArray(list, 0x0D4AE7FE, &count, &cellBounds)
    → mMapBounds[0] = cellBounds[0]; mMapBounds[1] = cellBounds[1]
// 回退：属性 0x086C1E0F 的 int = addOn，mMapBounds = 1 << addOn（addOn 默认 5）
```

实测（`map_scalars_probe` 扫 EcoGame 脚本包）：

```text
cellBounds  SimCity-Scripts_257365692.package 00B1B104:40800200:622B9CD7 = [128,128]
```

即 **`mMapBounds = [128,128]` 来自 EcoGame 的 SimProperties 记录
（instance `622B9CD7`），不是硬编码**；同一 instance 也出现在
`SimCity_Game.package` 的 `40E0C800` 组（早期 dev 记录），脚本包版本按加载
优先级生效。所以场景 2 改 eco 分辨率的**唯一入口是 `0x0D4AE7FE`**，且要改
“游戏实际加载的那一份”。默认值 `1 << 5 = 32` 只在属性缺失时出现。

### 2c：外部坐标对齐——工具侧已经可做

三帧（World / EcoGrid / RegionTile）与灯的三假设判定已实现（
`crates/sc-properties/src/map_frames.rs`、`light_alignment.rs`）。对整区单城：

- 城市局部坐标不再有 ±1024 的天然边界 → `WorldToEco` 的低侧负索引问题会放大
  （实测 `p=-1536 → x=-32`，引擎不夹取）；
- 灯光表里每盏灯的局部变换是**模型空间**，只要 unit 变换正确就不需要“按区对齐”；
  需要对齐的是 **unit 变换用哪一帧**，审计工具已能给结论；
- 贴花/招牌同理（`cGraphicsUnitDecals::SetTransform` 与灯同一 xform）。

### 2d：资源画刷

工作台 `resources` 节点当前只有“brush 文件 + target + blend + strength”，但
引擎消费的是 `0x00B2CCCA` 家族属性（坐标变换 + typed-map 索引），**没有可执行
的写侧**；且 ED 场（`0x03E421ED`，341 张 128² u32）没有 writer。

可做的两条路：

1. **参数路**：改区域画刷属性 → 只影响新建区域，无法精确绘制；
2. **场地路**：直接写 ED/eco 场 tile → 精确，但需要 ED writer（**当前缺**）。

## 5. 场景 3：新地图创建

### 3a 噪声地形：已可（但有边界）

`noise-height` 已支持 fBm v1 与 4096² 灰度图，写出 341 张 F0 tile。缺的是：
生成结果的**地形合法性检查**（海岸线/水面以上比例/坡度）与“新 group”注册。

### 3b 区域道路：EGB 可写，编辑器为零

- EGB 侧：`region_state::write_state` 已能写 curves/units；
- 资源侧：道路 PathEntry / appearance / Unit 属性全部读齐；
- 缺的是**道路绘制 UI 与曲线求解器**（拖拽 → 控制点 → `cSplineSetWalker` 等价
  几何）。这是独立的中等工程，不建议塞进本轮。

### 3c 城市槽位：链路明确但长

地块表（同 1a）+ 模板 JSON + 逐城 bootstrap（EP1 `0x08068AED`）+ 城市属性
`0xC1949C4D` 绑定 + 注册表条目。`region_bootstrap::catalog/load` 已把静态引导
读通，写侧需按同一结构生成。

### 3d/3e 资源与词条：同 1b/1c

### 3f 全新注册：暂不可承诺

新 group 需要：区域 desc property（含 `0x0E16BE1A` 水位等）、341 F0、341 ED、
模板 JSON、注册表、引导 EGB、locale、选择图，且 group 与“区域数字 id 的
FNV1”绑定（`region_map` 实证）。**没有一份资源是“生成器现成”的**，除高度外
都要新写。

## 6. 目标清单（文件 × 资源）

### 前端（可视化操作落点）

| 文件 | 现有能力 | 需要新增 |
|---|---|---|
| `src/pages/studio/panels/map.vue` | 选区域 → 3D 渲染 → 点地块 | 槽位批量选择、屏蔽预览、区域级画刷 |
| `src/components/map/MapPreviewPane.vue` / `MapViewer3D.vue` | `selectPlot` 事件、3D 点击 | citybox 矩形叠加、扩展把手、框外网格提示 |
| `src/pages/studio/components/mod-flow/*` | 节点表单/校验/构建 | `city-slots`、`resources`、`roads`、`coordinate-alignment` 的真实编辑器 |
| `src/pages/studio/components/mod-flow/MapGenerationNode.vue` + `src/lib/map-graph*.ts` | 噪声/笔刷地形图（extent 到 32768） | 资源通道笔刷、道路层 |

### 后端（写入器）

| 文件 | 现状 | 需要新增 |
|---|---|---|
| `src-tauri/src/mod_flow.rs` | 只 build 高度/静态资源 | `city-slots`/`resources`/`roads`/`metadata` 分支 |
| `src-tauri/src/map_panel.rs` | 预览 + 高度写回 | 地块表/画刷/ED 写回命令 |
| `crates/sc-properties/src/region_3d.rs` | 地块/模板读取 | `plot_table::upsert/remove` |
| `crates/sc-properties/src/region_map.rs` | ED/F0 读取、画刷清单 | **ED writer**、eco map writer |
| `crates/sc-properties/src/region_write.rs` | F0 341 写回 | （可复用） |
| `crates/sc-properties/src/locale.rs` | 词条读写 | （可复用） |
| `crates/sc-properties/src/map_contract.rs` / `play_area.rs` / `camera_bounds.rs` / `light_alignment.rs` | 本轮新增 | 接进 `coordinate-alignment` 节点 |

### 资源目标速查

| 用途 | 标识 |
|---|---|
| 逻辑范围 | SimProperties `0x07E0AC05`（`mPlayArea`） |
| eco 分格 | SimProperties `0x0D4AE7FE`（int[2]，零售 `[128,128]`；`cEcoGameScripts::Init` 直接读入） |
| eco 采样域 | config `0xB8F6B4CF` → `mFlatScale`（零售 1024） |
| 高度场 | `0x03E421F0` ×341（256² u16，8 m/px） |
| 地面/生态场 | `0x03E421ED` ×341（128² u32，A8R8G8B8） |
| 地块表 | `0x00B1B104 : F0E2E39C : 0x2B9C480C`（五数组：`0x0543BC96`/`0x16B7B1EF`/`0x9F2F9B65`/`0xC0B13E7E`/`0xF01D_E4B1`） |
| 区域 desc | `0x00B1B104 : <region> : 0x2FFD7EED`（水位 `0x0E16BE1A`） |
| 模板 JSON | `0x0A98EAF0 : <template> : 0x4529F96F` |
| 注册表 | `0x67771F5C`（`0x696447EA` launcher / `0x7CCC548C` 游戏 UI） |
| 引导 EGB | EP1 `0x08068AED`（每城一份，gzip） |
| 资源画刷 | `0x00B2CCCA` + `0x02A907B5` + `0x02A907B6` + `0x0DBA3A9C` + `0x0DE43899` |
| 灯光表 | `0x0CAA8F11` / `0x0CAA8F12`（本轮已解析） |

## 7. 实施顺序（每步都有可演示验收）

### P0：解锁场景 1（不动引擎，2 周量级）

1. `plot_table` 写入器：删/改槽位，**五条数组同步**，重编码 property；
2. `map-metadata`：模板 JSON 城市列表 + 注册表 uid 条目同步；
3. `locale` 词条写入：复用 `serialize_locale_items`；
4. 验收：新区域选择页城市数变化 → 进入区域 → 认领 → 存档重载 → 旧存档不回退。

### P1：场景 2 的“工具侧闭环”

1. `coordinate-alignment` 节点接 `map_contract` + `plan_expansion`：输出
   “扩到 16384 的三种 eco 方案 + 阻塞项”，**仍然阻止构建**（`ready=false`）；
2. 地图面板叠加 citybox 矩形、扩展把手、盒外网格/灯/相机可达幅；
3. 灯与贴花的帧判定做成“点击建筑 → 显示三假设命中数”。
4. 验收：对某个区域点“扩到整区”，面板能回答“哪一层不允许”而不是只报错。

### P2：写侧实验（必须新区域 + 新存档）

1. 只改 `flatScale` → 观察盒外采样是否变化；
2. 只改 `PlayArea` → 观察相机是否被 1020 卡住（本轮静态预测）；
3. 只改 `mMapBounds`（EcoGame SimProperties `0x0D4AE7FE`，如 `[512,512]`）→
   观察路径格网 / `mPlayCell` / 寻路与 UI 是否正常；
4. 三项都通过后，才讨论 ED writer 与资源画刷。

## 8. 不可承诺项（写进产品文案，避免过度承诺）

- 场景 2 的“整区可模拟”没有运行时证据；`flatScale`/`mMapBounds`/相机常量三处
  改动都可能触发存档膨胀、寻路退化或相机卡死。
- 城市资源场图像的真实 type id 与 `0x03E421F0` 的冲突未解决前，不做“单城资源
  写入”的交付承诺。
- 场景 3 的“全新 group 注册”需要 7 类资源同时成立，任何一类缺失都会得到
  “能选、进不去”或“进去是旧地形”的半成品。
- 画刷参数只影响**新建**区域；对已有存档城市无效。

## 相关文档

- [CityBox/相机/盒外灯契约落地](map-contract-citybox-camera-light-20261102.md)
- [地图消费者、UI 桥与相机](map-consumers-ui-bridge-camera.md)
- [区域加载、道路与资源生命周期](region-load-road-resource-lifecycle.md)
- [地图编辑依赖闭包](package-validation-and-map-edit-dependency-closure.md)
- [BOC 字段差异与地图扩展边界](boc-field-diff-and-map-expansion.md)
