# 破洞（Interior）decal 渲染管线全解 + 悬空案例诊断（2026-10-01）

> **⚠ 2026-10-01 勘误**：本文"体积中心在 transform 原点、引擎无射线锚定"
> 的方向正确，但"投影体积/交集印洞"的表述与根因链已被
> `docs/re/decal-engine-alignment.md` 修正：破洞属**浮空分支的自持几何**
> （不投影、不求交），正确实现 = 变换原点处 BackSide 内景体积盒
> （`buildHoleVolumeMesh`，已落地）。§四的 anchorOverride 方案已被其取代。

> 案例：LOTM v9 | SimCityDataEP1.package 0x9769DE99 的破洞 decal
> （ID 0xB241B64B）渲染成"悬在半空的破洞"。本文档理清破洞族的完整管线、
> 该案例的根因与修复。

## 一、族识别与数据

- 字典条目**无 Color1-4** → `variant = "hole"`（decalInteriorMap 族）；
  raster 为 RW4 纹理，alpha = 光衰减掩码（非不透明度）；
- Unit 属性：`0x0DA76A05/06`（decalLight scaleFactor/radiusFactor，照明用）、
  `0x0DA76A07` 缺席。

## 二、引擎侧机制（证据合成）

- **族**：`decalLightInteriorMap` + `decalLightBackground`
  （decalInteriorMap 系，浮空分支三族之一）；
- **量化公式**（容器原文）：`kSunContributionAmount = decalMaterialData[0].x`、
  `kLightAmount = decalMaterialData[0].y`、`materialLightScale = x×16+1`
  （注意：SDF 系是 ×16+0.25，两族不同）；
- **投影体积**：引擎 `decalProject` VS 将几何经 `modelToTexture` 投影进
  归一化盒，PS `clip(1-abs(texcoord))` 裁剪——**体积以 transform 原点为
  中心、按基向量定向，引擎无任何射线锚定**（§66.7：城内钩子静默的
  反向推论 + decalProject VS 语义）；
- 洞的表现 = 体积与建筑几何的**交集**被内景材质渲染（穿透式，前后墙
  皆洞），洞内可见 `decalLightInteriorMap` 的房间内景（建筑同源 slot5
  房间链）。

## 三、案例诊断（0xB241B64B 悬空根因）

transform 解析：

| 要素 | 值 | 含义 |
|---|---|---|
| 原点 | (11, -33, **41**) | 41 米高 |
| X 轴 | (-1, 0, 0) | 世界 -X |
| Y 轴 | (0, 0, 1) | 世界水平 |
| **Z 轴（投影轴）** | **(0, 1, 0) ≈ 世界向上** | **洞口朝上=水平洞**（楼顶/天井） |
| depth | 1.056 | 原点→平面 1.06m |
| scale | 19.18 | quad 38×38m |

**根因链**：OpenSCP 的定位 = 沿 ±Z 轴射线锚定（§45.3）→ 本例 Z 轴朝上，
垂直射线从 41m 高处向上打永远打不到垂直立面 → 锚定失败 → 回退浮空
quad（内景材质）→ **破洞悬空**。

**引擎为何不悬空**：体积以原点为中心、无锚定——41 米高度处一块
38×38m 水平板（厚 ±depth），建筑立面**垂直穿过**这块板 → 交集即洞，
洞"印"在立面上（高度 41m 处的水平洞带）。

## 四、修复（已实现，待目视）

- `projectDecal` 新增 `anchorOverride` 参数：给定则跳过射线锚定；
- 破洞族传 `anchorOverride = 0`：体积以 transform 原点为中心（引擎同构）
  ——洞印在立面/屋顶上，原点处无几何则不渲染（优于悬空幻影）；
- 彩色 decal（招牌/涂鸦）维持射线锚定（近墙多数正常，§45.2 实测
  0.86~8.36m 均在锚定范围内）。

## 五、开放项

1. 体积 Z 半厚的引擎真值：depth（§41.2 语义）还是 scale——待运行时
   对拍（本例 depth 1.056 vs scale 19.18 差异大）；
2. 穿透性：盒内前后墙皆洞（引擎 clip 无背面判定）是否符合游戏观感；
3. `decalInteriorMap` 房间链的纹理来源（slot5 DXT5）与 OpenSCP
   interior 贴图的分辨率对齐。
