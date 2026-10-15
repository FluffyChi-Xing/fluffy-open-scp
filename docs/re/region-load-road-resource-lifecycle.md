# 区域加载全流程：词条、槽位、资源与道路

## 结论

SimCity 的区域加载不是“从一个地图 package 直接画出全部内容”。它把两类输入合并：

1. **RegionTerrain/Game/EP1 静态资源**：高度、ED 地面场、资源画刷、城市槽位坐标、地层模板、区域与城市词条。
2. **区域级 EcoGame 状态**：初始道路曲线、道路单位、网络拓扑，以及各城市的资源/模拟状态。离线文件通常是 `Games/<account>/<region>/0/state_file_0_<timestamp>.egb`。

所以道路的真实生成必须经过区域状态初始化；ED 只能提供道路控制/森林等视觉场近似，不能替代道路曲线。

## 从区域选择到进入地图

### 1. 地图选择界面

区域列表的显示链是：

```text
RegionTerrain 区域 group
  → region 描述 property 0x51E7A18D（区域数值 ID、水位、资源/地形引用）
  → 城市地块表 0x2B9C480C（uid、世界坐标、地块 group）
  → EP1 AutomatedRegionTemplates JSON（模板名、城市/伟工坐标）
  → Game registry JS（模板/城市 stringID）
  → Locale JSON（最终显示词条）
```

区域模板 JSON 提供布局和伟工初始位置；它不是道路网格文件。城市槽位是否出现在选择界面，还会受地块表、注册表和区域状态的 claimed/box 信息共同影响。

### 2. 区域静态场装配

加载区域后，Terrain 系统按世界坐标查找 16×16 的 mip0 tile：

```text
tileX = (regionHalf + worldX) / 2048
tileY = (regionHalf + worldY) / 2048
```

每个区域有 341 张 F0 高度 tile 和 341 张 ED tile，组成 4096×4096 格、8m/格的背景区域。

- F0 → 高度场，GPU/CPU 高度查询与法线重建。
- ED → 地面/森林/道路控制等场，参与地面材质、植被和道路痕迹的视觉处理。
- Game 的 `D7EF2862:6F9F6AB3` → `zeros/ground/water` 地层模板、贴图和色调。
- 城市地块 group 的 256×256 高度图 → 认领城市时覆盖对应 2048m 城市格网。

### 3. 资源图层生成

区域描述 property 内的资源清单使用以下结构：

```text
0x00B2CCCA = coalEcoMapBrushes / oreEcoMapBrushes / ...
0x02A907B5 = brush key 列表
0x02A907B6 = 世界坐标变换/强度
0x0DBA3A9C = 目标 map 名
0x0DE43899 = typed-map 索引
```

画刷图像本身不在 package 中；引擎按区域种子和画刷参数程序化生成 stamp，然后写入 13 张左右的 typed map（soil、water table、forest、oil、coal、ore、desirability 等）。城市存档中的资源 map 是城市范围内的模拟结果，可作为参考或覆盖。

这解释了为什么资源可以在没有城市存档时存在：区域静态画刷足够生成初始区域资源；存档只增加城市模拟后的字段。

### 4. 区域级状态与道路初始化

区域级 `state_file_0_*.egb` 的读取链在脱壳符号中是：

```text
cLocalSaveManager
  → state_file_0_%lld.egb
  → cEcoGame::Read / cEcoMap::Read / cEcoUnit::Read
  → 区域 environment、curves、regional units
```

OpenSCP 当前解析出的状态结构为：

```text
RegionState.environment  = 区域 group
RegionState.curves       = 道路/路径的四段控制点
RegionState.units        = 区域道路单位/路口模型实例
```

`site == "0"` 的曲线属于区域外层网络；城市目录只提供各城市的资源 map 和城市模拟状态。未开垦城市的 `state_file_0` 仍可能存在，因为它是槽位初始状态，而不是“玩家已经游玩”的证明。

### 5. RoadManager 如何把状态变成道路

进入游戏后，RoadManager 和交通网络子系统消费状态中的道路数据：

```text
curves / units
  → Network 边与节点
  → PathIntersection（求路口臂和连接）
  → GraphicsPath（按 Path appearance 生成道路带状网格）
  → GraphicsPathNetwork（信号灯、路口部件）
```

已确认的引擎职责：

- `SC_cRoadManager` 读取 Network 的道路类型/参数表，并为网络边分配记录。
- `SC_cPathIntersection` 根据道路臂求解路口几何。
- `SC_cGraphicsPath` 遍历道路端点和 Path appearance，生成可视路面顶点；弯道通过曲线/偏移计算生成车道带。
- `SC_cGraphicsPathNetwork` 只补充信号灯等网络部件。

因此，**道路可视网格不是 RegionTerrain 中的一张道路 PNG，也不是由 ED 单独生成的**。ED 可用于预览道路痕迹，但真实道路需要 curves、Path appearance 和 Network 拓扑。

## 对 OpenSCP 的直接含义

### 为什么当前无存档预览看不到真实道路

此前 3D 面板把 `road_assets` 放在 `save_layers` 分支中。即使 Game 包中有道路 Path appearance，没有 `curves` 也无法实例化真实道路。ED 只会产生地形控制痕迹，不能生成道路贴图、车道线或路口，因此预览现在不再把 ED 猜测绘制成伪道路。

对安装包做的只读 census 已确认 `SimCity_Game.package` 中存在大量带有
`0x0CAA680D`（路径点）、`0x0CAA6841`（点范围）和 `0x0CC8FE7A`
（PathEntry）的静态 Unit 资源。例如 `40E1C000:9C4C3711` 有 9 个点、6 个
范围和 3 个 PathEntry。这证明离线初始化并不依赖联网；缺少的是把区域/城市
模板的 `PrecreateUnits` 表解析出来，并将这些 Unit 的局部路径按同一
`cSPTransform` 放回区域坐标。新增的 `path_prop_scan` 探针用于持续验证这条链。

资源链也已在原始安装包上实测：Cape Trinity (`C2A9C48F`) 的静态区域属性
可以读出 `coalEcoMapBrushes`、`oreEcoMapBrushes`、`oilEcoMapBrushes`、
`waterTableEcoMapBrushes`、`forestEcoMapBrushes` 等 8 类画刷，每类至少有
一个带世界坐标的 transform。它们不应依赖城市 EGB；预览层应把这些 brush
作为无存档初始资源覆盖，城市 EGB 只负责覆盖/演化后的 typed map。当前前端
已增强 brush 覆盖层的可见性，下一步是把 brush stamp 合成为与游戏一致的
typed-map 纹理，而不是只画中心圆。

### 正确的真实道路路线

按优先级应实现三层来源：

1. **存在区域 `state_file_0`**：直接解析 region 0 的 curves/units，复用现有 `map_assets` 生成原生道路。
2. **只有城市槽位初始状态**：检查对应城市 `state_file_0` 是否包含区域网络引用；不要把已游玩城市曲线误当成区域外层网络。
3. **完全没有 EGB**：只能运行时复现“模板 → 初始 EcoGame 状态”的生成器，或继续使用 ED 近似；不能从 F0/ED 无损还原道路中心线。

### 对地图 mod 生成的含义

- 修改高度、ED、资源画刷、槽位和词条属于静态 package 覆盖。
- 修改真实道路需要修改/生成区域级 EcoGame 状态，或者找到游戏创建新区域时写入 `state_file_0` 的模板/服务器输入。
- 仅覆盖 RegionTerrain package 可以改变地形和资源显示，但不会自动创造新的 Network curves。

## 证据文件

- `docs/overview/glass-box/region-and-map.md`
- `docs/overview/glass-box/terrain.md`
- `docs/overview/glass-box/transport.md`
- `docs/source-code\\_legacy_ghidra_dump/GB_cLocalSaveManager.c`
- `crates/sc-properties/src/region_state.rs`
