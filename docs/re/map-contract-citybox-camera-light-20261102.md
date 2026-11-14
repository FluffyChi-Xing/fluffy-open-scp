# 地图契约落地：CityBox 范围、相机钳制与盒外灯对齐（2026-11-02）

> 样本：`D:\ea-games\simcity_dev\new-cource\Simcity Development Files Including PDB Debug files`
> （dev beta 反编译 + PDB + per-function source tree）
> 实测包：`D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData`
> 本轮没有启动游戏、没有注入、没有改任何游戏包或 EXE。所有结论分三层标注：
> **已实测**（跑过真实数据）、**已对拍**（反编译逐行核对）、**待运行验证**。

## 0. 本轮交付物

| 类型 | 位置 | 作用 |
|---|---|---|
| 代码 | `crates/sc-properties/src/map_frames.rs` | World / EcoGrid / RegionTile 三套坐标 + 逐位等价 `WorldToEco` |
| 代码 | `crates/sc-properties/src/play_area.rs` | `PlayArea` / `mPlayCell` / 四边扩展计划与阻塞判定 |
| 代码 | `crates/sc-properties/src/camera_bounds.rs` | 相机两层夹取复现、可达半幅、解锁步骤 |
| 代码 | `crates/sc-properties/src/light_alignment.rs` | 灯光表解析、世界位置、盒外审计、坐标系假设检验 |
| 代码 | `crates/sc-properties/src/map_contract.rs` | 上述四者的只读聚合报告（UI/探针共用） |
| 探针 | `probes/map/map_contract_probe.rs` | 真实区域 → 城市盒/相机/回环/扩展问题清单 |
| 探针 | `probes/map/light_align_probe.rs` | 真实灯光表 → 世界位置 / 盒外 / 坐标系判定 |
| 探针 | `probes/map/map_scalars_probe.rs` | 反查 `flatScale` / 高度缩放与灯光表普查 |
| 探针 | `probes/map/light_table_dump.rs` | 灯光表逐列转储（确认列语义） |
| 测试 | 同目录 `#[cfg(test)]` | 32 个新用例；`cargo test -p sc-properties` 115 通过 |

复现命令：

```powershell
cargo test -p sc-properties
cargo run -p probes --release --example map_contract_probe -- `
  "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData" C2A9C48F
cargo run -p probes --release --example light_align_probe -- `
  ".../SimCityDataEP1.package" 42E1C000 2F0F5C25 1600 -1856 0 1600 -1856
```

## 1. 已实测：`flatScale = 1024`，eco 定义域恰好等于城市盒

`SC::cTerrainGfx::FillFromProps` 从 config 属性 `0xB8F6B4CF` 调
`cTerrainMapScalars::SetFlatScale`。零售包实测（`map_scalars_probe`）：

```text
flatScale  SimCity_Game.package 00B1B104:40232000:A2E2DF52 = 1024
heightScale 同资源 = 2048     heightBias = -1024
```

`WorldToEco`（RVA 0x42A910）把世界坐标映射到 `[-flatScale, +flatScale]²`：

```text
x = int( ecoSize * (p.x + flat) / (2*flat) )      // 高侧夹到 ecoSize-1
cell = 2*flat / ecoSize
s = (p.x + flat - x*cell) / cell
```

因此 **`flatScale` 就是城市盒半边长 1024 m**。这不是巧合，而是“eco 采样域 =
城市盒”这一设计参数的硬证据：城市盒外的坐标在采样阶段就被夹到格边，
不存在第二套世界原点。`map_contract_probe` 的实测回环印证了这一点：

```text
center      (0,0)          → eco=(64,64)     格内 true
max-min     (1024,-1024)   → eco=(127,0) s=1.000 [索引夹取]   ← 盒边正好饱和
outside-west(-1536,0)      → eco=(-32,64)    格内 false        ← 低侧不夹取，索引为负
outside-east( 1536,0)      → eco=(127,64) s=33.0 [索引夹取]
```

两条以前没写清的事实：

1. **正方向边界点会饱和**：`p = +1024` 时 `index = ecoSize`，被夹到 `127`
   且 `s = 1.0`；采样落在最后一格边缘。
2. **负方向不夹取**：`p = -1536` 得到 `x = -32`，引擎直接用它索引。
   这是盒外采样最先会崩的位置，比“夹到格边”更危险。

## 2. 已对拍：CityBox 的约束来源是 `PlayArea`，不是边界网格

`cEcoGame::InitGame`（RVA 0x60BDC0）：

```text
GetPropertyAsBoundingBox(list, 0x07E0AC05, &mPlayArea)
mPlayCell.min = mPlayArea.min
mPlayCell.max.x = (max.x-min.x)/mMapBounds[0] + min.x
mPlayCell.max.y = (max.y-min.y)/mMapBounds[1] + min.y
mPlayCell.max.z = min.z + (max.z-min.z)
```

`cGraphicsCityBoxBoundary::StartGame`（RVA 0x7CB1B0）只按 `PlayArea` 画约 11 m
边界带；`cGraphicsGame::StartGame` 用 `mPlayCell` 宽 × 20 调
`ModelWorld::SetWorldBounds`。所以：

- **主数据是 `mPlayArea`**（`0x07E0AC05`），边界带与可视世界盒都是跟随项；
- 扩盒必须同时看 `mMapBounds`（eco 分格）与 `flatScale`（采样域）；
- 只改网格 = 画面变大、模拟仍判盒外。

`play_area::plan_expansion` 已把这几条变成可判定项。实测（CapeTrinity，半边
1024 → 2048）：

```text
新 playCell = 32.00×32.00 m，中心位移 = (0.0, 0.0)，旧范围仍在新范围内 = true
[警告] 超 eco 定义域 x：|2048| > 1024 → 盒外采样夹到格边
[警告] 超 eco 定义域 y：|2048| > 1024 → 盒外采样夹到格边
[警告] z 未知（离线包不含 PlayArea 的 z 值）
```

即：**扩到 2048 半边后，模拟域仍只有中间 1024 半边是有效的**。要让 2048 真正
可用，必须同时处理 eco 采样域（`SetFlatScale` 的来源属性或
`WorldToEco` 的输入域），否则只能得到“能放、能画、不能算”的假扩展。
这正是 BOC 类方案止步的地方，本轮把它变成了可测量的量而不是猜测。

## 3. 相机：扩 citybox 不会解锁镜头，反而会缩短

2026-11-12 样本更正。本节固定 `±1024/±1020` 公式描述的是开发版分支。当前离线零售 Glide 在已定位的输入路径上以 `0.996` 缩放传入 AABB，再夹取。扩大 AABB 会扩大该 helper 的有效范围，同时影响模拟。见[零售机器码与跨界样本](map-consumers-ui-bridge-camera.md#2026-11-12-零售-glide-对拍)。

`cCameraControllerBase::Update`（RVA 0x395B90）`mRelativeMove != 0` 分支逐行：

```c
// 1) 先按 PlayArea 夹 x/y/z
// 2) 再对 x/y 做绝对判定
if (x < 1024.0) { if (x <= -1024.0) x = -1020.0; } else x = 1020.0;
if (y < 1024.0) { if (y <= -1024.0) y = -1020.0; } else y = 1020.0;
// 3) CheckForGroundFromPoint 修正 z
// 4) ConstrainTarget()：再次按 PlayArea 夹 x/y/z
```

关键点：

- 第 2 层是**绝对坐标常量**，与 `PlayArea` 大小无关；
- 默认城市盒半边正好 1024，所以它看起来“和边界重合”；
- 盒边目标 `x = 1024` 会被写成 `1020`，**比原边界还短 4 m**；
- 扩到 2048 后，盒内 1500 处仍被钉在 1020 —— 相机完全没变宽。

`camera_bounds` 把这条链做成可测断言：

```rust
retail.reachable_half_extent()   == [1024, 1024]
expanded.reachable_half_extent() == [1024, 1024]   // 扩盒零收益
expanded.evaluate([1500,0,5], true).final == 1020  // 反而更短
expanded.evaluate([1500,0,5], false).final == 1500 // 不走相对移动时不触发第二层
```

解锁计划（`plan_unlock`）因此必须同时列两层，且 `ready` 恒为 false：

```text
扩 PlayArea 1024→2048 / 改绝对常量 1024→2048 / 保留防穿地 / 四路输入验收
```

注意：`regionedit`（`kRegionEditCameraID`）只是切控制器并关 dof/motion blur，
共用同一条 `Update`/`ConstrainTarget`，不能当解锁开关。

## 4. 盒外灯：灯链上没有 PlayArea，错位只可能来自坐标系

`cUnitModel::FillFromProps` / `OnMove` → `cGraphicsLights::SetTransform`（RVA
0x7D1750）→ `cLightInfo::SetTransform`（RVA 0x7D0E70）→ `cLightRenderer::SetPos`：

```text
灯世界位置 = unitWorldTransform × lightLocalTranslation   // 仅矩阵乘法
```

链上**没有** `PlayArea` 夹取、也没有 eco 采样。所以“盒外灯不对齐”只可能是：

1. 单位变换与网格/模型不在同一坐标系（差一个 `mPosInRegion`，或重复叠加）；
2. 单位变换没更新（盒外单位被跳过，灯停在旧位置）；
3. 灯光表本身被改错（局部变换列）。

### 已实测的灯光表结构

`FillLightInfoMap`（RVA 0x7D24D0）通过表属性驱动 `cLightInfo`。零售包
（≥1810 处含 `0x0CAA8F10` 的属性）实测列语义：

| 列 | 类型 | 实测值示例 | 含义 |
|---|---|---|---|
| `0x0CAA8F10` | Key | `BC76E6BA` | 灯类（每模型常量） |
| `0x0CAA8F11` | Key | `75D4C8CD` / `2F0FF9FD` / `0820ABAF` | 灯型：point / spot / line |
| `0x0CAA8F12` | Transform(56 B) | 12 float | **局部变换**（3×3 + 平移） |
| `0x0CAA8F13` | ColorRGB | `(1, 0.92, 0.52)` | 颜色 |
| `0x0CAA8F14` | float | `12/4/6/5` | 半径 |
| `0x0CAA8F15` | float | `8/1/2` | 内半径 |
| `0x0CAA8F16` | float | `1/2/5` | 第三参数（长度/Kd 待定） |
| `0x0CAA8F17` | String8 | `overhang` / `F_door` / `F_spot` | 通道名 |

`0x0CAA8F11` 的三个哈希正好等于反编译里的
`136358831 / 789576189 / 1976879309`（= `0x0820ABAF / 0x2F0FF9FD / 0x75D4C8CD`），
这是列定位的硬证据；`0x0CAA8F10` 与它们不同值，说明它是灯类而非灯型。

### 盒外定位的判定方法

`light_align_probe` 把单位平移到区域坐标后再审计，实测：

```text
unit 平移 = [1600, -1856, 0]
盒外灯 23 / 索引夹取 23 / 共 23
坐标系判定 = 区域坐标（应减去城市原点）  修正 = (-1600, 1856)
盒内 0 → 23 / 23  假设命中 [0, 23, 0]
最靠外的灯 #8: 超出盒边 882.28 m
```

三个假设分别是 `0`、`-cityOrigin`、`-2×cityOrigin`。这条输出直接回答
“这批灯的坐标是哪一套帧”，并给出需要施加的平移量；平手时优先“不改动”，
避免把本来就正确的灯误移。

## 5. 现在的工程边界（不要过度承诺）

- 本轮全部是**静态**结论：`mPlayArea` 的 z 值在离线包里读不到，报告中显式标
  `UnknownZ`，不用合成值冒充。
- `flatScale` 已经证明等于盒半边，因此任何“只扩 `PlayArea`”的方案都应先假定
  模拟域不会跟着变大，必须进运行验证。
- 相机方案只到“改哪两层、保留什么”，**没有**任何原生补丁地址被验证可用；
  `CameraUnlockPlan.ready` 恒为 false 是故意的。
- 灯光表解析覆盖了列结构与局部变换，但 `0x0CAA8F16` 的语义（长度 vs Kd）仍待
  用带线光源的模型对拍确认。
- `region_3d` 的 341 tile 是 4096² 场 / 1 px = 8 m / tile 2048 m（`region_write`
  实测）；早期文档里的“8192 单位 tile”表述应以此为准。

## 6. 下一步（按验收收益排序）

1. **盒外灯现场对拍**：在盒外放一座带 `overhang` 灯的建筑，用
   `light_align_probe` 的三种假设分别对照游戏内亮灯位置，确认实际帧。
2. **相机第二层实证**：同一城市把 `PlayArea` 改成 2048（只改逻辑范围），记录
   拖拽/键盘/边缘滚动三种输入的最大 `target`，验证是否停在 1020。
3. **eco 域实验**：只改 `SetFlatScale` 的来源属性（`0xB8F6B4CF`），观察
   `flatScale` 变化后盒外采样是否真的改变，再决定 citybox 扩展的完整资源闭包。
4. **UI 接入**：把 `MapContractReport` 做成地图面板的“范围/相机/灯光”诊断页，
   文本全部走 i18n（zh-CN / en-US）。

## 证据索引

- `crates/sc-properties/src/map_frames.rs`、`play_area.rs`、`camera_bounds.rs`、
  `light_alignment.rs`、`map_contract.rs`（本轮实现与断言）
- `probes/map/{map_contract_probe,light_align_probe,map_scalars_probe,light_table_dump}.rs`
- `tmp/map-contract.log`、`tmp/light-align.log`（本轮真实输出）
- 反编译：`src/GB/cEcoGame/InitGame.c`、`src/SC/cTerrainMapSet/WorldToEco.c`、
  `src/SC/cCameraControllerBase/{Update,ConstrainTarget}.c`、
  `src/SC/{FillLightInfoMap,cGraphicsLights/*,cLightInfo/SetTransform}.c`
- 关联文档：[CityBox 与 PDB 突破](map-editor-pdb-breakthrough-20261030.md)、
  [相机与消费者](map-consumers-ui-bridge-camera.md)、
  [区域坐标同步](region-city-coordinate-sync-and-placement-alignment.md)
