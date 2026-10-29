# 新样本推动地图编辑器开发的突破

> 样本：`D:\ea-games\simcity_dev\new-cource\Simcity Development Files Including PDB Debug files`
> 分析日期：2026-10-30
> 证据类型：retail PDB、反汇编函数、source-tree 符号和现有资源链

## 结论

车辆/规则链也已闭合：新样本的 `cGameAppMode::Init` 注册 12 个 transport handler，警车四类 TransportEntry 共享 `cartraffic`/`cTransportVehicle`。地图编辑器的资源检查不能只验证模型和纹理，还应验证 `mID → mHandlerID → handler index → UnitRule`，完整表见 [交通 handler 与车辆规则类型闭合](traffic-handler-types-and-police-correlation-20261030.md)。

这个样本对地图编辑器的价值已经超过“多找到几个函数名”：它把四个长期混在一起的问题拆成了可以单独验收的运行链。当前最值得立刻实现的是一个**范围/资源/调试三层诊断管线**：导入时先解析 PlayArea 与脚本 MapBounds，再建立 World、EcoGrid、RegionTile 三种坐标契约，最后用 EcoMap、GraphicsMap 和 341 高度 tile 的资源闭包做 round-trip。这样可以在写回游戏前发现 90% 的“边界变大但道路仍受限”“地图在盒外整体平移”“PNG 已替换但游戏没读到”的错误。

| 问题 | 新样本确认的机制 | 对编辑器的直接突破 | 仍需运行时确认 |
| --- | --- | --- | --- |
| CityBox 大小扩展 | `cEcoGame::InitGame` 从 `SimProperties` 读取 `mPlayArea`；`cGraphicsCityBoxBoundary::StartGame` 只生成 11m 边界带 | 把扩展入口从 mesh 修改提升为逻辑 PlayArea patch；边界 mesh 只作为可视化跟随项 | 额外道路、zoning、建筑合法性是否还受其他脚本上限限制 |
| 地图外坐标对齐 | `WorldToEco` 用 `mFlatScale` 映射并夹取 eco 索引；341 导出另用 8192 单位 tile | 明确三套 frame，禁止重复 `mPosInRegion`；导入/导出可做可逆坐标测试 | 盒外采样、负坐标和非方形 region 的实际样本 |
| 地图资源添加 | EcoGame → EcoMapEntry → EcoMap 图像；GraphicsMap 单独创建 GPU 纹理 | 资源添加变成 entry、模拟图像、图形图像、UI/规则引用的闭包检查 | 新 map-entry 是否能被目标版本脚本枚举、保存和 UI 查询 |
| 地图 debug 工具 | Terrain Editor authoring gate、brush queue、341 导出、debug dump；`DebugModifyMap` action ID=42 | 可以实现可重放的 authoring session，而不只保存最终 PNG | 运行时 action 回放与不同 tick/版本兼容性 |

## 1. CityBox 扩展：修改逻辑范围，边界带随动

### 静态链

```text
SimProperties[0x07E0AC05]
  -> GB::cEcoGame::InitGame (RVA 0x60BDC0)
  -> mPlayArea
  -> script MapBounds() / mPlayCell
  -> 道路、建筑、zoning 的合法性检查

mPlayArea
  -> cGraphicsCityBoxBoundary::StartGame (RVA 0x7CB1B0)
  -> 四条约 11m 边界带，每边 256 段
```

这条链说明 CityBox 的边界 mesh 不是约束来源。编辑器应把 `mPlayArea` 当作主数据，生成一个可审计的 patch：旧值、新值、中心点、四边扩展量，以及脚本 `MapBounds` 的期望网格。生成边界可视化时再按新 PlayArea 重建四条边，避免出现“画面边界扩大了、模拟仍把对象判定在盒外”的假成功。

### 第一阶段验收

1. 读取原始 `mPlayArea` 和脚本 `MapBounds`，检查它们的单位、中心和宽高是否一致。
2. 只扩展一条边，写入测试 patch，重新生成 boundary 预览；预览必须与新范围完全重合。
3. 在新增条带内放置道路、建筑和 zoning probe，分别记录合法性结果。若只有边界可见而 probe 失败，报告“还有逻辑上限”，不要继续扩大 mesh。
4. 保存并重新加载，比较城市槽位锚点和旧范围内对象坐标；旧范围内不允许出现统一平移。

## 2. 坐标契约：WorldFrame、EcoGridFrame、RegionTileFrame

### 三种 frame

| Frame | 原点/尺度 | 主要证据 | 编辑器规则 |
| --- | --- | --- | --- |
| `WorldFrame` | 游戏连续世界坐标 | `cTerrainMapSet::WorldToEco` 输入 | 对象、相机、brush 使用；允许负坐标 |
| `EcoGridFrame` | `ecoSize × ecoSize` 的整数格与格内 `s/t` | `WorldToEco` RVA `0x42A910` | 只在采样边界夹取一次；记录 clamp 标志 |
| `RegionTileFrame` | 341 高度资源，起点 `4096 - regionDimension/2`，tile 边长 8192 | `Save341HeightMaps` RVA `0x40D350` | tile 内只做局部索引；不得再次叠加 `mPosInRegion` |

`WorldToEco` 使用 `smScalars.mFlatScale`，将连续坐标映射到 eco 网格，并把整数索引限制在 `0..ecoSize-1`。因此“盒外”首先是采样夹取，不是存在另一套世界原点。341 导出则按固定世界单位分块，并用 `xNN_yNN_mipN` 生成资源名；它和 PlayArea 的大小不是同一个量。

### 可逆测试

编辑器导入一组点 `p` 时保存三元组：`pWorld`、`(iEco,jEco,s,t)`、`(tileX,tileY,localX,localY)`。导出后反算 `pWorld2`，要求：

- 对未 clamp 的点，`distance(pWorld, pWorld2) <= 0.5 * cellSize`；
- 对 clamp 的点，显式标记 `clamped=true`，禁止静默写回；
- 341 tile 的路径 key 与 tile 坐标互相可解析；
- 任意对象集合的平均位移接近 0，统一非零位移只允许来自显式城市槽位变换。

这组断言能直接定位三类错误：重复平移、错误 flat scale、tile 原点或 y 轴方向错误。

## 3. 地图资源添加：完成资源闭包再宣称“已添加”

### 资源闭包

```text
script map-entry
  -> cEcoGame::InitGame
  -> cEcoMapEntry::FillFromProps
  -> cEcoMap::FillFromProps / Create
  -> 0x03E421F0 (16-bit) 或 0x03E421EC (8-bit) 图像
  -> cGraphicsMap::Create (Property 0x0AF3DAC7)
  -> cGraphicsMap::Update / UI BuildMapsJSValue
```

`FillFromProps` 从 `0x07E0AE01` 读取 `maxValue`、从 `0x07E0AE05` 读取初值、从 `0x07E0AE06` 读取图像 key；`InitFromImage` 优先读 16-bit 图像，失败后读 8-bit 图像，并按 `maxValue` 缩放到 `cellsX × cellsY`。图形侧使用另一个纹理 key 和相同的 cells 尺寸创建 GPU 纹理，不能把预览纹理当成模拟地图。

编辑器为每个新地图生成资源闭包清单：

- entry Property：名称、类型、`MapBounds`、`maxValue`、初值和开关；
- 模拟图像：TGI/key、位深、宽高、缩放规则、校验和；
- GraphicsMap：纹理 key、格式 24、用途 81、尺寸和更新来源；
- UI/规则引用：`BuildMapsJSValue` 能否查询，相关规则是否声明该 map；
- 保存包：save area、命名资源 key、写回后重读结果。

任何一项缺失时，状态应是“资源存在但未注册”，并给出缺失层，而不是显示绿色成功。

## 4. Terrain Editor 与 DebugModifyMap：从最终图片升级为可重放会话

### 已确认的调试闭环

```text
cTerrainSystem::Init (RVA 0x4052E0)
  -> cTerrainEditor + Terrain Editor layer 0x0A20698B
  -> SetTerrainAuthoringToolActive / SetRegionAuthoringActive
  -> ApplyBrush (RVA 0x40D830)
  -> Save341HeightMaps (RVA 0x40D350)
  -> FinalizeGameHeightMap / WriteHeightMapToFile (RVA 0x40A850)
  -> GetDebugDir()/Textures/dump/ + save area
```

`SetRegionAuthoringActive` 会切换 `kRegionEditCameraID` 并关闭 `dof`、`motionBlur`；`ApplyBrush` 把位置、旋转、尺寸、纹理和 brush description 放入 layer queue，尺寸等于 `mRegionDimension` 时代表整场操作。PDB 中 `tLogActionID 42` 是 `DebugModifyMap`，因此修改动作可以与 delta action 解码器结合。

建议定义 `TerrainAuthoringSession` 文档格式，至少包含：

```json
{
  "version": 1,
  "authoring": { "terrain": true, "region": true, "camera": "kRegionEditCameraID" },
  "actions": [
    { "id": 42, "tickDelta": 0, "map": "height", "brush": {}, "world": {} }
  ],
  "exports": [
    { "resourceKey": "heightmap_x00_y00_mip4", "tile": [0, 0], "sha256": "..." }
  ]
}
```

先实现只读 session 录制、341 dump 和差异查看，再开放 overlay 写回。这样出现错位时能区分是 brush 输入、坐标转换、GPU 读回还是资源保存失败。

## 5. OpenSCP 实施顺序

### Sprint A：只读诊断（可立即做）

- 增加 PlayArea/MapBounds/RegionDimension/flatScale 诊断面板；
- 为每个 map entry 生成资源闭包清单；
- 导出三 frame 坐标样本和 clamp 标志；
- 读取 341 dump 与 GraphicsMap 纹理，显示 hash/尺寸/Key。

### Sprint B：离线可逆 round-trip

- 实现 World↔EcoGrid 和 World↔RegionTile 的纯函数转换；
- 用固定点集做无 clamp、边界、负坐标和盒外测试；
- 写入临时 save area，立即重读并比较 hash、尺寸、采样值。

### Sprint C：最小写回实验

- 只扩展一条 PlayArea 边；
- 只添加一个 map-entry；
- 只执行一个可重放 `DebugModifyMap` brush；
- 失败时保留 session、dump 和资源清单，禁止覆盖原始包。

### Sprint D：运行时验收矩阵

| 验收项 | 通过标准 |
| --- | --- |
| CityBox | 新增条带内道路、建筑、zoning 均可合法创建，旧范围对象零平移 |
| 坐标 | 边界/负坐标 round-trip 误差在一个 eco cell 内，clamp 可见且可解释 |
| 资源 | EcoMap 可读、`mMapSum` 更新、GraphicsMap 更新、UI 可查询、保存后可重读 |
| Debug | session 可重放，341 tile hash 稳定，失败能定位到 action/坐标/资源层 |

## 6. 摄像机解除：独立于 PlayArea 的最小安全改造

新样本证明区域编辑相机不是自由镜头。`SetRegionAuthoringActive` 只切换
`kRegionEditCameraID`（字符串 `regionedit` 的 FNV1 hash），并关闭 DOF 与 motion blur；
`cCameraControllerBase::Update` 每帧仍会调用 `ConstrainTarget`。因此把 PlayArea 放大来
“顺便解锁镜头”会同时改变模拟域，风险高且不必要。

相机限制至少有三层：

1. `ConstrainTarget`（RVA `0x391830`）从 `Game()->PlayArea()` 取得 AABB，对目标的
   x/y/z 做夹取；
2. `Update`（RVA `0x395B90`）的 `mRelativeMove` 分支先按 Game bounds 夹取，再用
   `[-1024,1024]` 和 `±1020` 常量二次限制 x/y；
3. `CameraEyeConstraints`（RVA `0x393530`）根据 PlayArea 选择地面采样路径，界外
   使用 `PointSampleHeightInRegion` 并修正高度/俯仰。

建议 OpenSCP 将镜头解除实现为独立的 `CameraConstraintProfile`：

```json
{
  "mode": "regionedit",
  "xyBounds": { "min": [-8192, -8192], "max": [8192, 8192] },
  "zPolicy": "terrainSample",
  "relativeMovePolicy": "same-as-xy-bounds",
  "preserve": ["ground-check", "pitch-correction", "lerp", "camera-restore"]
}
```

第一步只做只读观测：记录每帧约束前后目标、六个 bounds、relative move 和地面采样
结果。第二步只替换相机专用 XY bounds，同时保留 z 和高度检测。第三步才处理
`mRelativeMove` 的 ±1024/±1020 第二道限制。这样能分别判断“主夹取已解除”和“某种
输入仍被硬编码限制”，避免误判为完全自由。

相机 Property 仍可调整 zoom、FOV、near/far clip、min/max pitch 和 orientation；但
这些参数只控制镜头曲线，不能解除 XY 边界。相机切换还必须测试进入城市、区域视图、
跟随对象、城市切换和重载后的恢复，因为 `cCameraDirector` 会保存并恢复前一个 camera ID。

## 7. 当前不能过度承诺的部分

静态样本还不能证明：扩大 PlayArea 后所有 gameplay 系统都会接受新范围；脚本能否枚举任意新增 map-entry；负坐标和非方形 region 的运行时边界行为；以及 `DebugModifyMap` action 在不同版本中的回放兼容性。上述项目必须进入运行时验收矩阵，不能用 PDB 的存在替代测试结果。

## 证据索引

- `GB::cEcoGame::InitGame`：RVA `0x60BDC0`，`SimProperties[0x07E0AC05]` 与 map-entry 初始化。
- `SC::cTerrainMapSet::WorldToEco`：RVA `0x42A910`，`mFlatScale`、eco index clamp 和 `s/t`。
- `SC::cTerrainEditor::Save341HeightMaps`：RVA `0x40D350`，341 tile 起点、8192 单位分块和资源命名。
- `SC::cTerrainSystem::RegisterPackage`：RVA `0x405520`，save area/package 注册。
- `SC::cTerrainEditor::WriteHeightMapToFile`：RVA `0x40A850`，`SaveNamedResource` 写回。
- `SC::cTerrainEditor::ApplyBrush`：RVA `0x40D830`，brush queue。
- `cGraphicsCityBoxBoundary::StartGame`：RVA `0x7CB1B0`，边界带生成。
- PDB `tLogActionID`：`42 = DebugModifyMap`。

相关原有文档：

- [区域/城市坐标同步与放置对齐](region-city-coordinate-sync-and-placement-alignment.md)
- [地图静态启动链](map-static-bootstrap.md)
- [隐藏调试工具与地形笔刷](hidden-debug-tools-and-terrain-brushes.md)
