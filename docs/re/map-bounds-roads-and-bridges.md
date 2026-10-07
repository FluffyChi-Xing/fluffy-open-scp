# 地图可见边界与道路/桥梁渲染（2026-10-08）

## 本轮修复：裁切和截面

旧 3D 数据窗口从城市/伟工中心包围盒加 2560m 余量计算，但只使用 X 宽度
同时循环 X/Y；`sy1` 没有使用。因此纵向较长的区域被截掉、横向较长时还可能
读取原始场之外的数据。这套推测范围也会显示游戏可见边界外的下坡过渡带，
看起来像斜坡，而不是游戏的垂直截面。

dev 函数 `SC/cTerrainRegion/FillFromProps.c`（0x4261B0）实际读取：

- 属性 `1581061855`：`mRegionVisibleAreaAsInt`；
- 属性 `2963977649`：`mRegionSizeAsInt`（本机区域源场为 32768m）；
- `VisibleRegionContainsXY.c` / `VisibleRegionIntersectsXY.c` 用
  `±visibleArea/2 - mPosInRegion` 判断边界。预览使用区域世界坐标，无城市偏移。
- `AddSkirtNodesToSelectionList.c` 为边界四面单独建立节点，底部为 -1024m，
  使用独立 `skirtDir`，不是把地形边缘渐变到海平面。

现实现直接读取可见边长，居中裁剪。属性缺失、非法或无法按当前网格处理时
回退完整源场，不按地块中心猜测。截面复制当前 LOD 网格的边缘顶点，向下
竖直连接到底部，每一面保留独立水平法线。地形更新 LOD 时截面一起更新，
避免原先 16m 截面与 128m 地形边缘不一致产生接缝。

RT0 的 11 个区域实测均能解码；城市方框和伟工圆形的完整范围均在裁切内：

| 可见边长 | 区域 group |
|---|---|
| 8192m | BEAF0510、C2A9C48F |
| 16384m | C04182E4、BC357A2B、B12DE348、D01FA985、A0B60DDE、9E9B1FF0 |
| 32768m | DB25018C、E41A82B8、E0183D94 |

验证：`region_bounds_probe` 检查实际 PNG 尺寸、居中窗口、全部用地范围；
2 项 Rust 边界测试；4 项截面几何测试覆盖 17/65/257 顶点 LOD 与内部块；
Vue 类型检查、截面模块 ESLint 通过。泰坦峡谷与地平线群岛实际 WebGL 截图
在 `tmp/terrain-color-titan/`、`tmp/terrain-horizon/`，最终待用户目视。

## 道路是路径生成几何，并非地面色带

下列路径相对本地 dev 目录
`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/`。

1. `SC/cGraphicsPath/Init.c`（0x7A9F40）由 `GB::cEcoPathSet` 的路径及
   `mEntryIndex` 取得 PathEntry 配置，读取挤出资源键：

   | 属性 | 用途 |
   |---|---|
   | 0x09532375 | static extrusion |
   | 0x09532377 | interactive/realtime extrusion |
   | 0x75152C4A | invalid preview extrusion |
   | 0x09532376 | hull extrusion |

2. `SC/cGraphicsPath/BuildGeometry.c`（0x7B0950）执行 ReserveBuffers →
   FillBuffers → BuildInstancedModels，再把结果分配到材质绘制桶。
3. `FillBuffers.c`（0x7ADE70）初始化 `cPathInterpolationInfo`，计算路径弧长、
   路口裁剪平面及桥头衔接。`SC/CreateExtrusionGeometry.c` 使用
   `GB::cEcoPathSet::GetSplinesForPath` 的真实样条，输出顶点、索引、材质分段、
   装饰实例和 decal。不是从生态图提取中心线。
4. `SC/cMeshExtrusionInfo/Init.c`（0x6847D0）从属性资源加载组件和材质。
   `anonymous/LoadComponentGeometries.c`（0x688CB0）确认至少有模型组件与
   递归组件组，模型通过 `cMeshExtruder::LoadGeometry` 载入；
   `SC/cMeshExtrusionComponent/Init.c` 还处理 ribbon 与 prop instance。
   因此不能把所有道路概括成一个预制模型：路面可由 ribbon/模型组件沿样条
   生成，附件另行布置。
5. `SC/cMeshExtrusionMaterial/Init.c`（0x67DE00）读取材质键 0x09558821
   与最多四个贴图 override。`cGraphicsPathNetwork/RenderRoads.c`（0x7A6DD0）
   按材质/模式桶和索引范围绘制。

### HLSL 实证

已逆向的 `tmp/ps_fragments.json`：

- `networkRoadBasicMaterialSetup`：baseUV、tileUV、worldPosition、worldTangent。
- `networkRoadBasicBaseColor`：采样 diffuseSampler，`clip(alpha-0.75)`，
  混合重复噪声贴图。车道线、沥青纹理等应从材质纹理恢复，不能用黑色常量替代。
- `networkRoadBasicNormalMapPS`：法线 XY 在贴图 G/A 通道，Z 从平方和重建。
- `networkRoadBasicSpecMapPS`：R 控制 gloss/spec exponent，B 控制 spec strength。
- 对应 VS 有 `networkRoadBasicDefaultVS` / `rwRoadShaderVS` 等路径。

## 桥梁与附件

- `cGraphicsPathNetwork/IsBridge.c`（0x7A62E0）根据 **PathEntry 类型位图**识别桥，
  不是根据“路面下方是否有水”。所以游戏也能识别跨陆地高架。
- `cGraphicsPath/FillBuffers.c` 调用 `CalculateBridgeApproachInfos`，替换两端
  衔接部分的 extrusion，并搜索桥下路径/路口，参与构造与裁剪。
- `cGraphicsPathPart/BuildInstancedModels.c`（0x7AB190）根据挤出过程生成的
  `mPropInstances` 和 transforms 加入模型 LOD 实例集；也处理灯光实例。
- `mRelativeToGround` 决定是否按地面加高。`mStackToGround` 且非相对地面时，
  查询脚下高度，按模型高度向下逐节堆叠，最多 100 节；这是支撑柱可落地的
  实际机制，不能用任意固定间距和固定水深盒子等价替代。
- `cMeshExtrusionComponent/Init.c` 读取 stackToGround 属性 **0x0E8AF8B8**。

### 本机包体交叉验证

`road_render_probe` 扫描零售 `SimCity_Game.package` 得到：

- 94 条属性资源含 staticExtrusion 引用；样例
  `40E1C400:565BCF6C → 09558B7E:565BCF6C`。
- 275 条资源含 extrusion 材质键 0x09558821；此数是候选配置条数，不能解释为
  275 种道路。
- 10 条资源含 stackToGround 字段；样例组 `0D942043`，instance
  `7FEFE810`、`2621E418`，样例值为 true。尚未逐条证明这些都属于桥墩。

## OpenSCP 当前差距与落地顺序

当前 3D 预览已有独立黑色 ribbon 和盒状桥墩，但中心线来自 ED 地下水零值，
宽度固定 28m，桥面只由水位抬升。这是示意几何，不是游戏道路/桥梁模型。
ED 只有 16m 栅格且自然缺水也为零，无法可靠恢复真实路宽、车道数、道路类型、
桥梁类型或精确路径高度。本轮只调研真实渲染链路，未把示意道路宣称为原版模型。

建议按以下依赖顺序实现：

1. 找到并解码区域原始 EcoPathSet：点/段/样条、PathEntry ID、路径高度和路口。
   已找到 `GB/ReadRegion__2.c`（0x643AE0）读取路径 slot/span 索引，使用大端序；
   **该函数不是完整中心线解码器，原始路径资源与区域模板的绑定仍待定位**。
2. 解析 PathEntry → extrusion → component → model/material/texture 的引用链，
   优先复原一种 highway 与一种 bridge，在同一路段对拍。
3. 复用项目现有模型/纹理解码，按路径弧长铺设 UV，并实现路口/桥头裁剪。
4. 按实例配置恢复路灯、护栏、桥墩及 LOD；支撑柱按 stackToGround 落地。

本轮地图修复未提交，遵守用户先目视后提交的要求。
