# Decal 18 族路由与片段组成——完整逆向（2026-10-01）

> 来源：shader 容器 0x0469A3F7 命名片段解析（6636 块全库索引，decal 族 288 块）
> + §65.10 源码级量化 + §65.15 变体对象格式。状态：**路由映射完整，
> OpenSCP 的 decal 族路由修复可直接照此实现**。

## 一、结论速览

1. 引擎 decal 渲染 = **命名片段组合系统**：每个最终 shader 族由
   「VS 片段 + 数据装载片段 + PS 片段」链式组合（类似可编程管线拼装）；
2. **decalProject 系 10 变体 = 投影上墙**（`texcoord<t0> = mul(modelToTexture,
   float4(modelPos,1))`）；**浮空分支仅 3 族**（decalFloatQuad / 
   decalNeonTubeSDF / decalInteriorMap）；
3. OpenSCP 修复对应：招牌/广告按其 shader-def 落在哪个组合族路由——
   投影族**必须走投影**（当前实现误挂浮空即"广告离墙"根因）；
   城内静默已证：**加载期接口进城即释放，武装必须在加载窗口完成**。

## 二、片段清单（32 个，全库唯一来源）

### VS/几何片段
| 片段 | 作用 |
|---|---|
| `decalProject` | `Current.texcoord<t0> = mul(modelToTexture, float4(modelPos,1))`——投影进墙体纹理空间 |
| `decalFloatQuad` | `texcoord<t0>.xyz = indices.yzw × (1/255)`——独立 quad，UV 从顶点数据取 |
| `decalVS` | 直通 `texcoord<t0>` |
| `decalNUS` | worldTextureU/V 装载 |

### 数据装载片段（每实例材质数据 → 常量）
| 片段 | 作用 |
|---|---|
| `decalMaterialData1..4` | 装载 1~4 组 `decalMaterialData`（SDF 系用 4 组） |
| `decalMaterialData4SDFSwizzle` | SDF 系的 4 组 swizzle 变体 |
| `decalMaterialInfo` | `decalWorldDirection = texcoord<t0>` |
| `decalMaterialInfoWithObjectData` | 由 modelToTexture 反解方向并取负（−z/−x/−y） |
| `decalBase` / `decalBaseCenter` | building 索引/相机数据装载 |
| `decalOpacity` | `decalMaterialData[1] = In.color` |

### PS 片段（效果链）
| 片段 | 作用 |
|---|---|
| `decalNeonBrighten` | `color.rgb ×= shColorDiff + shColorSpec + spec`——SimCityLighting 场景光响应 |
| `decalLightSDF` | SDF 管灯：`circleDist = saturate(1−sdf/0.5)×hwRatio`、断电=半亮 |
| `decalAnimateSDF`（+Darken/Disabled） | 跑马灯动画：`animEdge²×animRatio×32` 外扩 |
| `decalLightInteriorMap` | 破洞内景：`kSunContributionAmount=kMaterialData[0].x`、`kLightAmount=[0].y` |
| `decalLightNeonTube` | `color.a ×= decalMaterialInfo.x` |
| `decalLightBackground` | 背景光变体 |
| `decalFloatQuadNoClip` | `color.rgb ×= 2`——NoClip 增亮（无体积裁剪） |
| `decalClip` / `decalClipBack` | `clip(−textureFloatPosition.z)`（墙面之后裁掉）/ 背面变体 |
| `decalColorPS` | CSG 贴图生成：`(tex(P1).x, tex(P2).x, 0, 0)` 平面距离场 |
| `decalNormalPS` / `decalDebug` / `decalSDF` / `decalOpacity` / `decalDebug` | 调试/辅助 |

### SDF 量化公式（decalAnimateSDF 全家，引擎原文）
```hlsl
float materialTubeLightFactor = decalMaterialInfo.z * 8 + 1;
float materialLightScale      = decalMaterialInfo.x * 16 + 0.25;   // 注意：InteriorMap 系是 ×16+1
float4 lightFactor = ... powered ? lightFactor : float4(0.5...)       // 断电=半亮
```

## 三、族组成（组合矩阵）

引擎最终族 = 片段链组合（由 shader-def 的 cmp 链分发，.text 静态可读）：

| 最终族 | 组合 |
|---|---|
| decalProject 系（10 变体） | `decalProject`(VS) + [Lit→`decalNeonBrighten`] + [Neon→`decalLightNeonTube`] + [SDF→`decalAnimateSDF`/`decalLightSDF`] |
| decalFloatQuad 系 | `decalFloatQuad`(VS) + `decalFloatQuadNoClip`(PS ×2 增亮) |
| decalInteriorMap 系 | `decalLightInteriorMap` + `decalLightBackground` |
| decalClip 系 | `decalClip`(VS) + `decalColorPS`/CSG |
| regionDecal 系 | `regionDecalProject`/`regionDecalCalcLighting`/`regionDecalClip` |

（每族另有 `decalMaterialData1..4`/`decalNUS`/`decalVS` 数据前置——
**招牌 6 变体 / 涂鸦 5 变体** = 同族不同 LOD/质量档的变体对象预设。）

## 四、OpenSCP 修复映射

| 引擎事实 | OpenSCP 对应修复 |
|---|---|
| 招牌/广告落在 decalProject 系（投影） | **必须走投影路径**（当前误挂浮空 = 广告离墙根因） |
| `decalNeonBrighten` = 场景光响应 | 招牌材质改为响应场景光（Lambert/场景光采样），非纯自发光 |
| `decalAnimateSDF` 管灯因子 `z×8+1`、断电半亮 | 霓虹动画/断电效果按此公式实现 |
| `decalLightInteriorMap` 破洞内景 | 已实现（`kSunContributionAmount`/`kLightAmount` 同款） |
| 浮空分支仅 FloatQuad/NeonTube/InteriorMap | 只有全息 billboards/灯管/破洞走浮空 |

## 五、来源与验证

- 片段库：`tmp/dynamic/all_blocks_index.txt`（6636 块，decal 族 288 块）；
- 完整源码：`tmp/dynamic/shader_container_SimCity_App_g*.bin` +
  `tmp/parse_container_tokens.py`；
- 交叉验证：§65.10（容器源码量化）、§65.15（变体对象格式）、
  `docs/runtime-capture.md` N7 行（字典普查）。
