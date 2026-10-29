# 区域/城市坐标同步与道路、建筑放置对齐

本报告针对三个问题：区域视图进入城市时是否改变世界坐标原点；解锁或扩大边界后道路/建筑为何可能偏移；道路和建筑的定位到底使用哪些变换。结论来自 `SimCity2013-source-tree/src/SC` 与 `src/GB` 的反编译源码。源码中的函数体是静态证据，不能单独证明某个存档在运行时一定使用了某组数值。

## 结论

游戏至少维护两套相关但不同的范围：

1. `cTerrainRegion` 的区域大小和可见范围。区域默认大小与可见范围都是 `0x8000`（32768）；正式值可由 RegionTerrain 属性 `0xB0AAB5B1`（反编译显示为有符号常量 `-1330989647`）和 `0x5E3D16DF` 覆盖。
2. `GB::cIEcoGame::PlayArea()` 返回的城市可玩 AABB。道路、分区、建筑放置和大量 UI/碰撞检查都读取这个 AABB。它不是 terrain 的可见范围。

进入城市时没有发现把所有道路或建筑坐标整体平移到另一个坐标系的代码。城市地形的 `mPosInRegion` 直接取城市槽位的区域坐标，区域高度采样再把城市局部输入加回该偏移。因此保持槽位位置、城市数据和 unit/path 的世界坐标不变，只扩大 `PlayArea`，不会产生系统性平移；若编辑器同时改了槽位位置或对坐标重复加偏移，才会造成整体错位。

## 区域到城市的坐标链

`cTerrainGfx::Init`（`src/SC/cTerrainGfx/Init.c`）在区域模式下从环境属性 `0xF01DE4B1` 读取 `mPosInRegion`，并读取角度；在城市模式下读取城市索引，再通过 `cTerrainExporterCity::Init` 将 `city.mPosition` 和 `city.mDegreeAngle` 复制到 `mPosInRegion`/`mAngleInRegion`。这里没有额外的 region→city 平移。

`cTerrainRegion::Init`（`src/SC/cTerrainRegion/Init.c`）把 RegionTerrain 的尺寸转换成：

```text
halfRegionSize = regionSize / 2
cellSize       = regionSize / (1 << maxSubdivisions)
```

`cTerrainGfx::PointSampleHeightInRegion` 的关系是：

```text
regionPos.x = mPosInRegion.x + worldPos.x
regionPos.y = mPosInRegion.y + worldPos.y
```

随后用 `regionPos` 检查 `±halfRegionSize`，并按高度 tile 的 2048 网格采样。这个函数证明城市地形局部输入与区域坐标之间唯一明确的平移量就是 `mPosInRegion`；编辑器导出时不能再把城市槽位位置加一次。

区域视图的摄像机切换也遵循同一坐标来源。`cRegionCameraController::TransitionToSite` 通过游戏对象按 `GB::cSlotRef` 取得一个三维位置，把它写入摄像机 `mLerpSubject.mTarget` 和 `mRotationPoint`。该函数只改变摄像机过渡目标，没有修改 path、lot 或 terrain 数据。

## 城市盒/存档加载不负责几何平移

`cGameAppMode::LoadBoxesFromRegionInfo` 读取 JSON 的 `boxes[]`，收集每个城市的 `id`、`uid` 和 `creatorId`，随后把区域盒和城市盒交给 `GameManager::LoadBoxState`。代码维护的是 box ID、UID、owner、活动城市和 inactive city 索引；没有读取几何坐标并对城市对象做平移。

因此存档切换的坐标一致性来自盒内保存的 EcoGame/path/unit 数据与对应城市环境属性，而不是 `LoadBoxesFromRegionInfo` 在加载时临时计算的偏移。OpenSCP 若修改城市位置，必须同时更新城市槽位、引用该槽位的区域地形/高度采样关系，以及城市盒中已有 path/unit/lot 的世界坐标。

## 道路使用统一的世界坐标

`cUnitPathCreationLocal::CreatePath` 接收 `cSPVector3 points`，直接传给 `GB::cEcoPathSet::AddPoints` 和 `AddPath`；没有 region/city 偏移参数。道路距离和切线分别在 `UpdateDistanceArray`、`UpdateTangent` 中基于同一 path set 计算。

道路是否属于区域道路由脚本表中的 bitset 决定。`cGraphicsPathNetwork::Init` 从 EcoGame scripts 读取：

- `1776773985` → `mRegionPathEntries`
- `-1186731749` → `mRegionConnectionRoadPathEntries`

`IsRegionPath` 和 `cGraphicsPathPart::SetPathFlags` 将两张表合并判断区域路径。`SetPathFlags` 的 `inCity` 参数只影响渲染 flag（区域/连接道路在城市内外的 flag 值），不改变点坐标。桥和隧道也是额外 flag。

道路的边界判定统一读取 `game->PlayArea()`：

- `cGraphicsPathPart::PathInPlayArea` 对每个 segment 的两端点调用 `SP::LineIntersectsBox`；
- `IntersectionInPlayArea` 检查输入点是否落在 PlayArea AABB；
- `SC::IsSegmentInPlayArea` 对 segment 两端点逐个比较 `mMin/mMax`；
- `cZoningGame::StartGame` 用 PlayArea 的 min/max 初始化 zoning 网格。

这意味着“解锁边框”若只扩大视觉 mesh 或 `mRegionVisibleAreaAsInt`，不会放开道路/分区/建筑的可放置范围；要让新空间可用，必须改变城市 EcoGame 的 PlayArea 及其相关存档/初始化数据，并确保 path 点仍在同一世界坐标系。

## 建筑 lot 的定位链

建筑并不是把一个模型中心点直接加到城市原点。源码给出的链条是：

1. `cRegionCityLots::Init` 按 EcoGame scripts 的 `NumUnitEntries` 建立 `cUnitLotEntry`，并为实际 unit slot 创建 lot graphics。
2. `cUnitLotData::FillFromUnit` 用 unit 的 `mEntryIndex` 找到 entry，并复制 `cUnitLotEntry::GetPlacementTransform(graphicsGroup)`。
3. `cUnitLotData::CalculateUnitLotTransform` 先求 placement transform 的逆，再与 unit 的 `mWorldTransform` 做 `PreTransformBy`。这得到 lot local geometry → unit world transform 的最终变换。
4. `cRegionCityLots::CreateUnitLotGraphics` 对 bounding box、overlay corners、unit lot corners 依次应用 rotation、scale，再加 `mUnitLotTransform.mTranslation`；没有 region/city 偏移。
5. 放置阶段 `cBuildPlacementLotWalker::TestPlacement` 用 frontage/depth 构造 lot-local corners，通过 `cUnitPlacementData::GetUnitTransform` 变换到世界位置，使用 `cZoningLot::PointInLot` 检查，最后把 transform 写入 `cPlacementLotGroup::SetUnitPlacementTransform`。
6. `cZoningGame::GetUnitLotCorners` 再应用 `mUnitLotTransform`，并与 `game->PlayArea()` 比较/夹取，地面高度来自 `GroundHeight_2`。

这条链同时解释了“边框解锁后建筑不偏移”的必要条件：保持 lot local geometry、placement transform、unit world transform、frontage/depth 和 PlayArea 的世界坐标原点一致。扩大 PlayArea 只改变合法性边界，不应重算或加上城市槽位偏移。

## 对 OpenSCP 的实现建议

OpenSCP 的地图/城市编辑流程应把坐标字段分为三类，避免误写：

| 类别 | 代表来源 | 导出约束 |
|---|---|---|
| 区域坐标 | CityList `0xF01DE4B1`、`cTerrainExporterCity::mPosition` | 作为城市槽位的区域世界坐标；只写一次 |
| 城市世界坐标 | EcoPathSet 点、unit `mWorldTransform`、lot placement transform | 保持原值；移动城市时整体变换必须显式且一致 |
| 合法范围 | `GB::cIEcoGame::PlayArea()` AABB | 与可见范围独立校验；扩边不能只改渲染字段 |

建议新增导出校验：

- 检查每条 path segment 的端点、每个 lot corner 和每个 unit world transform 是否落在目标 PlayArea 或预期的跨边界白名单内；
- 检查 CityList 位置与 RegionTerrain 高度 tile 的 regionPos 映射，禁止重复应用 `mPosInRegion`；
- 对 region path/region connection road 的脚本 bitset 做一致性校验；
- 计算修改前后的锚点差值（城市槽位、道路端点、建筑 lot 中心），若出现统一非零平移则拒绝导出；
- 将 PlayArea、RegionSize、VisibleArea 分开显示并分别写入完整性摘要。

## 2026-10-30 新样本勘误：PlayArea、Eco 网格与高度 tile 是三条链

新的 retail PDB/source-tree 样本把“扩大 citybox 后坐标是否会漂移”拆成了三个可以分别验收的量：

1. `GB::cEcoGame::InitGame`（RVA `0x60BDC0`）从 `SimProperties` 的 `0x07E0AC05`
   读取 `mPlayArea`，随后用脚本 `MapBounds()` 计算 `mPlayCell`。这说明 citybox 的逻辑
   范围不是渲染网格推导出来的；边界扩展的首个写入点是 EcoGame 属性。
2. `SC::cTerrainMapSet::WorldToEco`（RVA `0x42A910`）只用
   `smScalars.mFlatScale` 把连续世界坐标映射到 `ecoSize×ecoSize` 网格，并把整数格索引
   夹到 `0..ecoSize-1`，再返回格内 `s/t`。因此“盒外”首先表现为采样网格的边缘夹取，
   不是另一套世界原点。
3. `SC::cTerrainEditor::Save341HeightMaps`（RVA `0x40D350`）导出时以
   `4096 - regionDimension/2` 为起点、每块 `8192` 世界单位，按
   `base_xNN_yNN_mipN` 生成资源名；`MakeHeightMapFileKey` 再把名字映射为
   `0x03E421F0` 高度资源 Key。高度场分块不能复用 citybox 的 PlayArea 尺寸，也不能在
   每个 tile 内重复应用 `mPosInRegion`。

编辑器应在导出前显示并校验 `PlayArea`、`mFlatScale/ecoSize`、`RegionDimension` 三组
   数值；任何统一平移只允许出现在显式的城市槽位变换中。这样可以把“边框扩大后道路/建筑
   偏移”快速归因到重复平移、PlayArea 合法性或高度 tile 分块，而不再把三种问题混为一谈。

## 仍需运行时验证的边界

源码已确认静态坐标链，但以下行为仍需用可解析样本或运行时对照确认：

- 某些版本/地图是否在 EcoGame 初始化阶段把 PlayArea 从固定 2048×2048 城市盒改成自定义 AABB；
- 跨城市连接道路在 region box 与 city box 同时加载时的 endpoint 配对和 z 高度处理；
- 边框解锁 mod 实际修改的是 PlayArea 属性、城市盒数据、还是额外的脚本合法性字段；
- 旧存档中 unit/lot transform 是否已经写成相对城市坐标，或始终是城市世界坐标。
