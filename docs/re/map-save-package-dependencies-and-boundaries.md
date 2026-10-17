# 地图/存档的 Package 依赖与边界机制

> 分析分支：`dev/rendering-optimisation`
>
> 本文把“区域地图”“城市地图/存档”“可放置边界”分成三个层次，避免把它们混成一个 package。

## 1. 结论先行

一个可加载的地图/城市存档不是单个 `.package`，而是：

```text
区域描述与区域背景地形
  + 城市规则/脚本与 Property
  + 模型、材质、纹理、shader/UI 等共享资源
  + 存档中的规则状态、delta 和元数据
```

边界也不是单一的矩形文件字段：

- 区域背景边界来自 RegionTerrain 的图块/描述数据；
- 城市可放置边界主要由 zoning/lot/unit 的几何与规则检查实现；
- 资源/规则的“允许范围”还受 EcoGame 脚本、Property 和区域 map 采样影响；
- 世界坐标本身是连续的，现有证据不支持存在一个独立的“盒外坐标系”。

## 2. 区域地图依赖

### 2.1 RegionTerrain 包

现有资产普查和渲染报告将区域背景数据归入：

- `SimCity_RegionTerrain0.package`：基础游戏区域，报告统计为 11 个区域；
- `SimCity_RegionTerrain1.package`：EP1/DLC 区域，报告统计为 4 个区域。

这些包提供：

- 高度图；
- 场/生态/资源类图层；
- 区域背景地形的 tile/mip 数据；
- 区域描述中与水位、区域名称和地块位置相关的 property。

渲染报告给出的区域背景规格是 4096×4096、8 m 网格，并切分为 341 个 tile 的 mip 金字塔。`FUN_00BEB730` 负责世界坐标到 tile 的换算。

### 2.2 区域描述与资源图层

区域数量、名称、城市地块位置、资源分布和水参数被报告为 RegionTerrain property 数据，而不是硬编码在单一 C 函数中。地下资源和生态层是区域图层的采样结果；它们是否覆盖某一点取决于区域数据是否有对应图层/范围。

因此，修改区域边界或背景通常至少涉及：

1. 区域描述 property；
2. 高度图/生态/资源图层；
3. tile 索引和尺寸一致性；
4. 可能引用这些区域的 UI/region 元数据。

## 3. 城市/存档依赖

### 3.1 共享资源包

| 包 | 对城市/地图的作用 |
|---|---|
| `SimCity_Game.package` | 地块模板、UI 布局、菜单 property、贴花字典等基础定义 |
| `SimCity_Graphics.package` | 建筑/单位 RW4 模型、全局法线/染色图集 |
| `SimCity_App.package` | UI/脚本资源、shader 源码容器、部分道具模型 |
| `SimCityDataEP1.package` | EP1 模型/材质主体 |
| `SimCity_DLC0.package` | DLC 模型和 mesh |
| `SimCity_RegionTerrain0/1.package` | 区域背景地形、区域图层和描述 |
| `SimCityUserData\EcoGame\*.package` | EcoGame 规则、unit/resource/map/path 等数据表和规则资源 |

这些包提供“解释存档状态所需的类型和资源”。存档本身只保存状态、引用和增量，并不会把所有模型、shader 和规则定义复制进去。

### 3.2 存档文件族

现有逆向报告对离线存档的识别是：

| 文件 | 作用 |
|---|---|
| `misc\MetaData` | JSON 区域/城市清单和元数据 |
| `state_file_<n>_<ts>.egb` | gzip 压缩的城市状态快照；解压头含主规则库 instance `622B9CD7` |
| `SLDelta*.mfs` | 二进制增量/delta 数据 |

因此，存档依赖关系可表示为：

```text
MetaData
  → 选择区域/城市/box
state_file / SLDelta
  → 恢复 EcoGame/ER2 状态
外部 package 资源
  → 解释 TGI、property、模型、材质、规则和地图图层
```

### 3.3 ER2/ERZ 规则链

`glassbox-engine-architecture.md` 和 `dev-dump-lot-placement-and-script-resources.md` 给出的链条是：

```text
EcoGame package
  → ER2 文本规则 / Property
  → ERZ 编译规则库
  → 主规则库 622B9CD7
  → egb 存档状态投影
```

这意味着只复制一个城市存档而缺少匹配版本的 EcoGame/Property 数据，可能出现资源引用无法解析、规则版本不匹配或城市无法正确恢复。

## 4. 城市内部的几何边界

### 4.0 `mPlayArea` 是已确认的城市可玩盒来源

`GB::cEcoGame::InitGame` 从 EcoGame 的 Property `0x7E0AC05` 调用 `SP::GetPropertyAsBoundingBox`，结果写入 `mPlayArea`；`GB::cEcoGame::PlayArea` 只是返回该 AABB。构造函数默认值为 `min=(-10,-10,0)`、`max=(10,10,3)`，但正式初始化会用 Property 覆盖它。

这不是只用于显示的字段：

- `SC::cZoningGame::StartGame` 用 `PlayArea().min/max` 初始化 zoning grid；
- `GetUnitLotCorners` 和 `CreateNewLotAndParcel` 将 lot 点/角点限制到 `PlayArea` 后再取地面高度；
- `cGraphicsCityBoxBoundary::StartGame` 读取 `PlayArea` 的 X/Y 范围生成四段边界网格，说明可见城市盒直接由同一 AABB 绘制；
- `cGraphicsPathPart::PathInPlayArea`、`IntersectionInPlayArea` 和 `SC::IsPointOnEdgeOfBox` 对路径、点和盒边执行 AABB 相交/范围检查。

因此，当前证据支持“城市盒主要是 EcoGame Property 驱动的 AABB，并被多个系统共同消费”。要扩大可玩范围，不能只改边界网格或 `SC::cBoundary` 对象；至少要同步 EcoGame Property、地图栅格尺寸/采样以及存档兼容性。

### 4.1 Lot / Unit 边界

现有 `dev-dump-lot-placement-and-script-resources.md` 对 `SC::cUnitLotEntry` 和 `SC::cUnitPlacementData` 的结论是：

- lot 不是简单的任意网格，而是道路 corner 对之间的带状区域；
- unit lot 的局部矩形由 frontage/depth 属性决定；
- `+Y` 是临街方向；
- 放置变换由 lot → unit 的世界变换和 placement transform 共同构成；
- `TestPlacement` 通过 lot corners、前缘、深度和地面高度进行检查。

关键属性包括：

- `0x0F9EFBA`：BoundingBox；
- `0x0BBBCDED`：frontage min；
- `0x0BBBCE2A`：depth min；
- `0x0DB7FB17`：placement transform；
- `0x00B1B104`：Property 资源类型。

这类边界决定建筑能否放入 lot、道路连接是否成立、模型是否超出地块，而不是决定整个区域的世界坐标范围。

### 4.2 区域外/盒外

已有样本和报告支持以下判断：

- 世界坐标保持统一连续坐标系；
- “城市盒”更像脚本/放置规则层的逻辑边界；
- 盒外能否放置，取决于脚本/Property 的放置检查、资源定义和工具行为；
- 盒外的资源采样是否有效，还取决于区域 map 是否覆盖该坐标。

此前的盒外放置样本使用了特殊脚本/属性组合，不能简单推导为“修改一个矩形边界字段即可解除限制”。

### 4.3 `cBoundary` 的证据状态

源码树中存在 `SC::cBoundary` 类型和生成函数文件，例如：

- `src/SC/cBoundary/operator.c`
- 对应 `include/SC/cBoundary_*.h`

但目前只从 `operator=` 等生成文件直接确认类型存在，尚未完成其构造、contains/test、与 zoning/region/placement 调用方的完整调用链。因此：

- `[已确认]` 引擎存在边界对象/类型；
- `[已确认]` 当前已确认的城市盒实现路径是 `GB::cEcoGame::mPlayArea`，而不是仅凭 `SC::cBoundary` 推导；
- `[已确认]` lot 放置有独立几何检查链；
- `[未确认]` `cBoundary` 是否就是城市盒边界的唯一实现；
- `[未确认]` 边界是矩形、凸多边形还是多个区域对象的组合。

## 5. 对 Mod 与地图编辑的实际含义

### 5.1 想增加一个可用建筑

需要同时考虑：

1. EcoGame resource/unit/property 定义；
2. 模型 TGI 和 LOD；
3. placement transform、frontage/depth 和 lot 规则；
4. 相关 UI/tool/category 引用；
5. 若涉及城市规则，还需要 ER2/ERZ 规则资源。

只放一个 RW4 或只放一个 Property 通常不够。

### 5.2 想改变区域地形或边界

至少要保持以下内容一致：

- 高度图尺寸和编码；
- RegionTerrain 的 tile/mip 索引；
- region 描述 property；
- 资源/生态图层坐标覆盖；
- 存档引用的 region/box 元数据。

OpenSCP 当前的 map panel 可以生成确定性 heightmap overlay，但这证明的是 DBPF 写回链路，不等于游戏会接受任意 region 边界修改。

### 5.3 想修改已有资源

当前最安全的实验顺序是：

1. 先读取原 package 中目标 TGI 和所有引用它的 Property；
2. 复制为新 TGI，修改引用方；
3. 以新增 overlay package 放入 `SimCityUserData\Packages`；
4. 用干净存档验证；
5. 只有在确认同 TGI 替换路径有效后，才考虑覆盖实验。

## 6. 下一步最值得做的验证

1. **包加载路径**：已确认 `SP::SetupResources`、`anonymous::RegisterPackages`、`SP::AddPackageIfValid` 和 `GB::InitGameVersions` 的目录枚举及注册链；剩余工作是把旧样本中的 `FUN_00403FB0` 与这条源码链逐地址对齐。
2. **最小 TGI 冲突实验**：一个原版 TGI、一个同 TGI overlay、一个全新 TGI，分别放入 `SimCityData` 和 `SimCityUserData\Packages`，比较启动日志和最终资源。
3. **地图依赖闭包**：从一个具体 region/box 的 `MetaData` 和 `state_file` 出发，反查所有 TGI，再映射到 RegionTerrain/Game/Graphics/EcoGame package。
4. **边界调用链**：从 `SC::cBoundary` 的构造和测试方法反查调用者，与 `TestPlacement`、`cZoningGame`、region map 采样做交叉验证。

## 证据清单

- `docs/blog/simcity-game-architecture.md`
- `docs/blog/glassbox-engine-architecture.md`
- `docs/re/map-ground-coloring-and-playable-region.md`
- `docs/re/dev-dump-lot-placement-and-script-resources.md`
- `src-tauri/src/package_browser.rs`
- `src-tauri/src/overrides.rs`
- `src-tauri/src/game_launch.rs`
- `src-tauri/src/map_panel.rs`
- `crates/dbpf/src/package.rs`
- `crates/dbpf/src/writer.rs`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\anonymous\AddGamePackage.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\GB\InitGameVersions.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\GB\cEcoGame\InitGame.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\GB\cEcoGame\PlayArea.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\cGraphicsCityBoxBoundary\StartGame.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\cZoningGame\StartGame.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\IsPointOnEdgeOfBox.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\cBoundary\operator.c`
