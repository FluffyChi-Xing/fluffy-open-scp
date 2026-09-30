# Shader 容器全库普查——6636 命名片段按域分布（2026-10-01）

> 来源：`tmp/parse_container_tokens.py` × 32 容器（SimCity_App.package，
> g40212000..40212015 共 8 group）→ `tmp/dynamic/all_blocks_index.txt`。
> 本普查一次性回答 N2（Prop）/N3（Effect）/N5（载具）的静态部分：
> **引擎渲染族清单与各自的片段组成**。

## 一、总览

| 域 | 命名块数 | 说明 |
|---|---|---|
| Effect/Particle | **131** | 完整粒子/特效系统（含树木粒子、soft particle、体积光、SC/SH/Dir 三种光照变体） |
| Building | **50** | building4/building5 两代管线 + Impostor + ConstructionRubble + InteriorMap |
| Tree/Flora | **45** | tree impostor（远景代理树）+ streetprop 全家族 + 树木粒子 + 地形森林阴影 |
| 载具 Vehicle | **33** | 完整载具管线（Setup/DataView/Deferred/Interior/Reflection/SelfIllum/Pylons） |
| Decal | **32** | 见 decal-family-routing.md |
| Lot/地表 | **35** | textureLotBase/Overlay 全变体（Deferred/NoLighting/TerrainMask/Lawn/Dirt） |
| Point（灯/点） | **17** | point/pointQuad/pointFogging/pointSpec/pointStencil + skybridge |
| Region | **17** | regionCityBuildings（Impostor 全套）+ regionDecal + terrainRegion |
| 其他（云/天/火/调试） | ~20 | cloudShadowPS、volumeFire、skybridge、debug 系列 |

**用途速记**：本文档是 N2/N3/N5/N6 的静态线索地图——每个域的片段组成即该域
的"渲染管线清单"，OpenSCP 可按域逐族复刻。

## 二、载具（N5 静态答案）：33 块 = 完整独立管线

载具拥有**独立的完整管线**（与建筑/人物均不共用）：

- **VS**：`vehicleSetupColorVS` / `SetupColorInstancedVS` / `SetupColorBatchedVS`
  （三档：单实例/实例化/批处理）、`vehicleOutputCombineVS` / `OutputDataViewVS` /
  `OutputDefaultVS` / `OutputDeferredVS` / `OutputPS`
- **PS**：`vehicleDataViewPS`（+Blend/Pylons 变体）、`vehicleDefaultPS`、
  `vehicleDeferredPS`、`vehicleInteriorPS`、`vehicleReflectionPS`、
  `vehicleSelfIllumPS`（+Mask）、`vehicleSpecColorPS`、`vehicleSpecularHotSpotPS`、
  `vehicleCubeSamplersPS`、`vehicleSamplersPS`、`vehicleUnpackCombine/Default/DeferredPS`
- **判读**：载具支持**反射（cubemap）、自发光遮罩、Pylons（警灯?）、内饰**
  ——这四项是 OpenSCP 载具渲染复刻的清单。

## 三、Prop（N2 静态答案）：streetprop 家族

街道道具（红绿灯/路牌等）拥有**独立 streetprop 管线**：
`streetpropDataViewVS/PS`、`streetpropSetupColor(VS/InstancedVS)`、
`streetpropDiffusePS`、`streetpropNormalMap(VS/PS)`、`streetpropNormalPS`、
`streetpropIllumPS`、`streetpropSamplersPS`、`streetpropSetupSpecPS`、
`streetpropSHParamSetup`——即**漫反射+法线+自发光+镜面高光+SH 参数**全套装：
streetprop 是**受光实体**（非 billboard）。另见
`terrainDataViewForest*`（森林阴影）与 `impostorTree*`（树木代理）。

## 四、Effect（N3 静态答案）：131 块 = 大型粒子/特效系统

- 结构：`effect*Input/Setup/Output/Sample/Lighting` 前后缀组合 +
  `effectSimple*`（MinSpec 低配变体）+ `impostorParticle*`（代理粒子）+
  `fastParticleQuad*` + `softParticles` + `unitParticle*`（单位粒子）+
  `simParticle*`（模拟粒子）+ `treeParticle*`（树木粒子子系统，Forest/Standard
  两变体，含 HSV Tweak/Alpha Fade/Contour Cull）
- **光照三变体**：`particleLightingDir` / `SC` / `SH`——粒子接受方向光/SC/SH；
- **判读**：N3 的"能否模拟"答案 = 能。粒子系统是标准 quad-impostor +
  动画图集（`effectAnimateTiles*`/`effectPickTiles*`）+ 三种光照，OpenSCP
  用 sprite + 自定义 shader 可复刻主干。

## 五、Building（N6/N8 关联）：50 块两代管线

- building4：`Clip/ClipAndReliefMapPS/Combine(Final/VS/PS)/Default/Deferred/
  DataView(VS/PS/Blend)/Interior(AndVariation)SetupVS/InteriorMapPS/
  MorphRotationAnimVS/ConstructionRubble.../FutureGlowPS/Impostor*/Samplers`
- building5：`MaterialIndex(VS/BatchedVS)/GetMaterialData/Unpack*` ——
  **building5 = 材质索引+Unpack 新代管线**（与 building4 的直采并存）
- **N8 关联**：`deformAbandonedVS/deformRubbleVS` 亦在全库
  （详见 n8-abandoned-deform.md）；`building4MorphRotationAnimVS` 的
  `animMorphRot` 即建筑倒塌动画（平移/旋转两模式，量化格式见该文档）。

## 六、Lot/地表（与已实现管线对照）

`textureLotBase*`（Default/Deferred/NoLighting/TerrainMask/Lawn/Dirt/
FlattenToEdge/OverlayCombine...共 35 变体）——与 OpenSCP 已实现的
generic_lot 三级来源管线同源，遗漏的变体（如 `FlattenToEdge`）可补。

## 七、Point 灯光（17 块）

`point/pointVF/pointPF/pointQuad/pointFogging/pointSpec/pointStencil
(Front/BackFace)/debugPoint` + `skybridge*`——点灯光系统独立成族，
OpenSCP 的点灯光复刻可对照。

## 八、Region（17 块）

`regionCityBuildings*`（Impostor 全套：ClipSpaceVS/WorldSpaceVS/
WorldSpaceInstancedVS/PackNormalDepthPS/UnpackPS/ClipPS/BaseColorPS）+
`regionDecal*` + `terrainRegion*`——远景城市Impostor 管线
（region 视图的城市渲染代理），OpenSCP 的地图面板远景可参考。

## 九、Water（32 块候选，待去伪）

`hdWater(VS/PS/NormalPS/MinSpecPS)`、`WaterChoppy/Normals/HeightPS`、
`distCameraFacingQuadWave(VS/PS/ZPoleWave)`、`terrainInitializeWaterPSFragment`、
`terrainWaterMapAdd/SubtractPSFragment`——**水面有专用高清管线与
地形水位增减片段**（`terrainWaterMapAdd/Subtract` 直接对应 OpenSCP
地图面板的水位刷）。

## 十、方法与再生产

- 解析器：`tmp/parse_container_tokens.py`（长度前缀字符串流）；
- 索引：`tmp/dynamic/all_blocks_index.txt`（6636 块）；
- 源容器：`tmp/dynamic/shader_container_*.bin`（32 个）；
- 交叉检索：`grep -iE <关键词> tmp/dynamic/all_blocks_index.txt`。
