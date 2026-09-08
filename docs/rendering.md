# SimCity (2013/CoT) 渲染器源码调研

> 2026-09-08 整理。素材来源：`tmp/shaders/` 下 8 个 cpp(0x0469A3F7) 资源转储
> （4×`cpp_0x0000000N_worldToClip-facade.txt` ≈8MB 编译字节码+CTAB 常量表，4×`cpp_0x0000000N_frac-worldToClip-atlas-facade.txt` ≈1.2MB HLSL 源码文本段），
> 以及已复原的 26 个 `tmp/shaders/src/building4*.hlsl`。argscript 片段库格式为
> `[片段名][二进制元数据][HLSL 代码][uniform 声明]` 串联，同名片段因变体组合会重复 2–3 次，
> 两个 62KB 大文件结尾被截断。编译器 `Microsoft (R) HLSL Shader Compiler 9.29.952.3111`，目标 vs_3_0/ps_3_0。

---

## 0. 管线总览

建筑本体管线（building4 家族）是多 pass 结构，每个 pass 由 argscript 片段拼装：

```
SetupVS / InteriorAndVariationSetupVS / MorphRotationAnimVS   顶点准备（调色板、内景、建造动画）
        ↓
Clip + ClipAndReliefMapPS     裁剪窗 alpha（slot1 tint A>0.5）+ Top 层 relief 视差 UV
        ↓
DeferredPS / InteriorMapPS / DefaultPS + FutureGlowPS        正面着色（内景混合 / 深度+法线 GBuffer / 未来辉光）
        ↓
CombinePS + CombineFinalPS    延迟合成（DataViewLightingModified 半兰伯特 + Schlick Blinn）
        ↓
ImpostorSetup/Pack/Unpack     远景 LOD（打包深度/法线 → 平面简化色）
```

另有 `building5` 家族（结构简化版：materialIndex 直查 materialDataSampler，无参数表行绑定）、
`generic_static/_skinning/_morphed`（props/静态件）、`streetprop`（街道小物）、`vehicle`（车）、
`simModel`（市民）等。DataView（数据视图）不是运行时分支，而是**整条管线切换 shader 变体**
（`*DataViewPS` / `*DataViewBlendPS`），着色收敛到固定参数的 `DataViewLightingModified`。

---

## 1. 窗户/假内景渲染链（Q1）

窗户不是几何，而是**着色器在 facade 像素级混合"预渲染房间图"（Interior Map）**。
参与要素：slot1 tint（facade 遮罩）、slot3 shader map、interiorMap 纹理、slot0 参数表、顶点 D3DCOLOR。

### 1.1 窗洞遮罩 = shaderMap.A

`building4InteriorMapPS` / `building4DeferredPS`：

```hlsl
float artistOpacity = shaderMapSampled.a;          // Base/Top 按 facadeTintValues.a 插值
float3 finalColor = lerp(interiorColor, exteriorColor, artistOpacity);
```

- `shaderMap.a = 0` → 透出内景（窗）；`= 1` → 外立面。即 SUGC Materials 章"Shader map A=窗户/Interior 位置"的源码实证。
- 窗户处（A≈0）仍有玻璃高光：shaderMap.B → specStrength 照常生效，玻璃的"质感"由 spec 参数表达（见 §2）。

### 1.2 假内景：逐窗格随机房间 + 盒体视差投影

`building4ClipAndReliefMapPS`（房间单元划分与图集选择）：

```hlsl
float2 interiorUv   = uv * regionXform.xy * interiorRoomInvSize;   // 按"房间尺寸"栅格化 facade
int2   interiorElem = int2(floor(interiorUv));                     // 每个窗格一个整数单元
float2 interiorSrcUv= frac(interiorUv);
float3 interiorEyeDir = eyeDir;  interiorEyeDir *= interiorRoomInvSize.xyx;
float2 interior_result_tc = interiorMap(interiorEyeDir, interiorSrcUv,
                             kBuildingInteriorMapInvDepth, kBuildingInteriorMapBackSize,
                             kBuildingInteriorDilation);
float2 interior_tc = interior_result_tc * interiorScale + float2(0, interiorOffset);
float roomId = FastNoise(float3(interiorElem, interiorRandomSeed));  // 逐窗格伪随机
float roomVariation = floor(roomId * 4);                             // 4 种房型
float4 interior_edge = float4(step(interiorThresholds, float3(roomId, roomId, roomId)), roomVariation * 4);
float interior_offset = dot(interior_edge, float4(1,1,1,1));
interior_tc.x += interior_offset * interiorScale;                    // 图集分格选择
```

`interiorMap()`（盒体裁剪 + 透视投影，模拟房间进深；vehicle/建筑共用）：

```hlsl
float2 interiorMap(float3 eye, float2 tc, float invMapDepth, float backSize, float dilation)
{
    const float kPadShrink = 0.9;
    float3 eyeDir = eye;  eyeDir.z *= invMapDepth;
    float3 pos = float3(tc, 0) * -2 + 1;   pos.z -= 1;        // 入口平面 → 单位盒
    float3 k = (sign(eyeDir) - pos) / eyeDir;
    float t = min(k.x, min(k.y, k.z));                        // 裁剪到盒体
    float3 target = pos + t*eyeDir;
    target.xy *= lerp(dilation, backSize, target.z);          // 前后墙不同缩放=透视
    return target.xy*-0.5 + 0.5;
}
static const float kBuildingInteriorMapInvDepth = 0.500000;
static const float kBuildingInteriorMapBackSize = 0.500000;
static const float kBuildingInteriorDilation    = 0.900000;
```

### 1.3 interiorScale/interiorOffset 的真实来源（顶点 D3DCOLOR.B）

`building4ImpostorPackVS` 注释直接写明（解开"B 1~80"之谜的最终出处）：

```hlsl
// texture location: 4 bits size (1 to 16), 4 bits index (0 to size-1)
float interiorTexData = In.color.b * (16 * 255.0 / 256.0);
int interiorSize      = int(floor(interiorTexData)) + 1;             // 1..16 格
int interiorSelection = int(floor(frac(interiorTexData) * 16));
float interiorScale   = 1.0f / interiorSize;                          // → slot0 row0.z
float interiorOffset  = interiorSelection * interiorScale;            // → slot0 row0.w
```

（若行内数据已烘焙进 slot0 参数表 row0=(palU,palU2,interiorScale,interiorOffset)，PS 直接取用——两条来源一致。）

### 1.4 内景着色 / 夜间亮窗 / 断电

```hlsl
half3 interiorColor = interiorMap.rgb * (shColorDiff + interiorMap.aaa * kBuildingInteriorMapSelfLightMax + shColorSpec);
// kBuildingInteriorMapSelfLightMax = 16 —— interiorMap.a 是逐窗"灯亮"通道，夜间自发光
half artistEmissive = specResult.x * specResult.x * interiorThresholds.z;  // 调色板 surface R²=艺术家自发光
// interiorThresholds.z：powered=1 / 断电&废弃=0，一票关掉全部自发光（源码自注释 "hack"）
```

废弃建筑（DeferredPS）：`specStrength×0.05`（cap 0.05）、`AO^8`、去饱和 0.75、暗化 0.55；
InteriorMapPS 里则是整体灰度化（`dot(rgb,(.3,.59,.11))`）。
`kInteriorEdgeBlock`（本编译版=false）用 `ddx/ddy(interiorElem)` 在窗格边界衰减 interiorMap.a 防拉伸。
远景 impostor（ImpostorUnpackPS）：`shaderMap.a≤0.8` 的窗像素按
`FastNoise(interiorElem, 0.2134) > interiorLightTime` 决定是否压黑（`lerp(tint, 0, interior)`）。

### 1.5 我们 app 缺什么

当前 LOTM v5 只有 Base 层 tint+法线。窗户链需要：
1. slot3 shader map 的 **A 通道**（窗洞遮罩）——已入库但未消费；
2. interiorMap 纹理（slot 之外的独立贴图，随材质/资产而来）+ interiorRoomInvSize（顶点数据 texcoord3.zw）；
3. uv 格栅化（regionXform × roomInvSize）+ FastNoise 逐格选房 + 盒体投影；
4. 供电状态 uniform（interiorThresholds.z）。

---

## 2. 材质质感：金属/玻璃/砖石如何渲染（Q2）

### 2.1 结论：没有材质分支，全部是 4 个标量参数

全 26 个 building4 文件 + 62KB BlendPS 中**不存在** metalness/玻璃/砖石分支。质感 =
`(specE 指数, gloss, specStrength, reflectance)` 四标量 + 调色板 tint + 法线贴图 + AO 的参数化：

| 来源 | specE（指数） | gloss | specStrength | reflectance |
|---|---|---|---|---|
| 建筑（DeferredPS） | `tintResult.a³ × 2048 + 1`（region palette a 通道） | `saturate(tintResult.a × specStrength)` | `shaderMap.b × kBuildingSpecOverdrive(=2)` | `specResult.a`（调色板 surface 行 A） |
| 车辆（specSampler） | `specMap.r³ × 1024 + 1/1024` | `specMap.r` | `specMap.b` | `specMap.g`（名为 roughness，实作菲涅尔下限） |
| DataView | 硬编码 10 | — | — | 硬编码 0.1 |

`specE` 编码统一走**立方曲线**（低值区分辨率高），上限 kSpecExponentMaxValue=60…1024（视路径）。

> **⚠ 资产实证修正（2026-09-08，migration.md §28.4）**：源码字面 `shaderMap.b` 在资产数据里
> 接近全零（玻璃楼 0xCFEC0F84 窗口区 B 均值 6.8/20.2，金样本墙面 14.5）——SUGC PDF
> "B=Specularity" 与实际数据不符。`shader_map_stats --dump` 目视确认**作者把 specularity
> 画在 G 通道**（玻璃材质窗口 G=159~186 且带对角高光笔触；通用材质 G=168 楼层带结构；
> 金样本窗洞 G=11）。open-scp 用 `uSpecG` uniform 取 G（默认），0 可回源 B 对照——
> 对源码的第二处有意偏离。

### 2.2 光照主链（建筑 DeferredPS → SimCityLighting）

```hlsl
specStrength = shaderMapSampled.b * 2;
specE        = artistSpecExponent³ * 2048 + 1;
float3 bentViewDirection = worldViewDirection;
bentViewDirection.z += 2*saturate(-z)*(1-saturate(bumpNormal.z));   // 掠射角视线软化
gloss = saturate(artistSpecExponent * specStrength);
SimCityLighting(screenUV, bumpNormal, bentViewDirection, gloss, artistReflectance, specE, specStrength, ...);

void SimCityLighting(normal, viewVector, glossyStrength, reflectance, specE, specStrength, ...)
{
    EnvLighting(normal, viewVector, glossyStrength, shColorDiff, shColorSpec);  // 天空环境项
    half sunMod = saturate(dot(sunSky.mSunDir.xyz, normal));
    half spec = CalcBlinnPhongSchlick(sunSky.mSunDir.xyz, viewVector, normal, specE, reflectance);
    specHighlight = spec * specStrength * sunMod * sunSky.mSunColor.rgb;
    sunMod = pow(sunMod, (1+glossyStrength));      // gloss 越高太阳漫反射越收窄
    shColorDiff += sunMod * sunSky.mSunColor.rgb;
}
```

`CalcBlinnPhongSchlick`：Blinn 半角向量 + 能量归一化 `(specE+2)/8` +
Schlick 菲涅尔快速式 `specF + (1-specF)*exp2(-8.656170·cosLH)`（8.656170=1/ln2×6）。

### 2.3 环境光 = 解析天空 LUT，不是 cubemap

```hlsl
void EnvLighting(normal, viewVector, s)   // s = gloss × 0.75
{
    half3 sampleDir = normal*(1-s) + reflect(viewVector, normal)*s;  // 法线→反射方向插值
    shColorDiff = SkyColorConv(sunSky, sampleDir, 1-s);              // 查天空预积分表 (s11)
    shColorSpec = shColorDiff;  shColorDiff *= 1-s;  shColorSpec *= s; // 能量劈分
}
```

三种质感的表现机制：
- **玻璃/金属**（高 specE、高 shaderMap.b、平滑法线）：锐利 Blinn 高光 + EnvLighting 采样贴近反射方向（强环境镜面）+（vehicle 路径）cubemap 反射 `specStrength×0.3`。
- **砖石/混凝土**（低 specE、低 gloss）：高光被 `(specE+2)/8` 摊平，EnvLighting 几乎全进 diffuse。
- **自发光**：调色板 surface R² × 供电状态（§1.4），×256 过压进 `exteriorColor`。

AO = normalMap.a（`artistAO`），最终 `exteriorColor = tint × (shDiff + emissive×256 + shSpec) × AO + extraEmissive`。

### 2.4 Top 层 relief（slot5，质感几何）

`building4Clip`：

```hlsl
float3 reliefEyeDir = eyeDir;  reliefEyeDir.xy /= abs(regionXform2.xy);
float2 reliefSrc = frac(uv2);  reliefSrc = reliefSrc*(1+tilePadding) - tilePadding*.5;
if (distFromPixel > kCullDistanceSq) relief_tc = reliefSrc;          // 400m 外不跑视差
else relief_tc = reliefMap(reliefSrc, reliefEyeDir, regionXform2, kFlatLevel, kConeSteps, kBinarySteps, ...);
float outsideTile = dot(saturate(float4(-relief_tc, relief_tc-1)), 1);  // 超出[0,1] → Top 层失效
relief_tc = relief_tc * regionXform2.xy + regionXform2.zw;
```

常量：`kReliefDepth=0.1`、`kMaxConeRatio=0.5`、`kConeSteps=8`、`kBinarySteps=2`、
`kCullDistanceSq=160000`、`kFlatLevel=23/255`（slot5 DXT5 高度图 ≤23 视为平面）。
注意：本编译版 `reliefMap()` 是恒等返回 `tc`（relief 被裁掉），仅保留参数—— relief 实现需从其他 cpp 变体找。

### 2.5 未来城市辉光（CoT）

`building4CombinePS`：沿 UV 的扫描光带 `glowLine`，×`wireFactor=(1-normalMap.a)²`（线框像素）×
`poweredAmount`，`kFutureGlowBrightness=50`、`kFutureGlowSpeed=-0.25`、`kFutureGlowInvLength=.12`、
`kFutureGlowSharp=4`、波幅 0.05/频率 0.5/动画 3。

---

## 3. raster（地块贴图）与建筑的对位 + 渲染模式色块（Q3）

### 3.1 对位关系：不在着色器侧

8 个转储中 `footprint / lotFootprint / lotRaster / groundPlane / tileSim` **零命中**；
lot 地面渲染常量 `lotInstanceInfo` 只有颜色/边框/图集 UV 字段，**无位置或纹理-模型锚点**：

```hlsl
struct cLotInstanceInfo {
    float4 colorsR, colorsG, colorsB, colorNormalsIdx;      // 4 通道遮罩 × RGB 调色板
    float4 borderWidthXYZW, borderColorsR, borderColorsG, borderColorsB, borderNormalsIdx;
    float4 baseTileUVMinMax, overlayTileUVMinMax;           // 4×4 图集（16 tile）UV 裁剪
    float4 colorHeights, borderHeights;
} lotInstanceInfo[14];
```

**结论（证据链）**：
1. 地块贴图的最终对位由 **lot 数据（property）+ 引擎 C++ 侧**决定：
   `LotPlacementTransform 0x0DB7FB17`（模型→地块矩阵，脚印在 +t）、`LotSize 0x0CCB7FC8`、
   `LotMask 0x0CCB7FD5`（raster 引用）、`LotOverlayBoxOffset 0x0CCB7FC9`。
2. 游戏内地面是**城市地形高度图 tile**：`matchTerrainHeight` VS 用
   `topDownUV = position.xy/2048+0.5` 采 HeightMap（高程域 ±1024）把 lot 顶点贴到地形，
   `lotsIndexWall!=0` 时再压到 `height-2`。建筑与地面的贴合由引擎喂 `modelToClip/modelToWorld` 完成。
3. raster LotMask 的游戏用途对应 `lotInstanceInfo` 的颜色/边框覆盖（RCI 分区色），
   **不是几何定位数据**；decal 家族的 `regionDecalInfo.transform/projMat/texTransform`
   也全部是运行时 CPU 常量。
4. 本项目观感偏移 = SCP 原版同款行为（migration.md §19.3：用户已确认原版 SCP 偏移量一致），
   SCP 作为浏览器不重建地形系统。遗留可查：0x0CCB7FC9 offset 已解析未消费；
   前端地面贴图未设 `flipY=false`（模型贴图设了，PropertyEditorViewport.vue:276 vs 425-441）；
   `lot_anchor_probe.rs` 的 0x0CCB7FD0/FD2/FD3 候选锚点未定论。

### 3.3 原版瑕疵：仰视建筑地板穿透（镂空模板继承）

**症状**（游戏与 open-scp 同现）：白模从下往上看正常；渲染后部分地板面消失、可透视模型内部。

**根因**（`tint_underface_probe` 取证，金样本 0x63D180B9）：
building4Clip 的 alpha 镂空（`tint.a<0.5` discard）是**逐材质全面片元测试**，镂空模板
（Color Control Map A=形状剪影/窗洞）只按外立面 UV 创作；地板/底面与立面**共用 facade
世界投影 UV**（探针实测下向面 UV `(272.8, 24.7)` 与墙面同坐标系），于是底面继承立面的
窗洞镂空。tint 图窗口矩形内不透明率仅 64.0%（整体 72.8%）；下向面顶点 37.1% 落在镂空区
（上向 50.0%、侧向 46.3%——各朝向采同一张带洞模板）。游戏正常游玩相机永在地面/屋顶
之上（建筑贴地），仰视不可达，Maxis 从未处理——**数据/管线固有瑕疵，非移植错误**。
共享镂空图集（如 0x1188B12E，18+ 模型共用）解释"某些建筑"才可见。

**open-scp 处理**：观察器缓解——前端 tint shader 中 object 法线 Z<-0.3（下向面，模型
Z-up 帧）豁免镂空并跳过调色（保持白模观感）；墙面窗洞（法线水平）零影响。这是对源码的
唯一已知偏离，源码其余链路逐字一致。用户补充观察：部分位置在**白模（几何）阶段就已被
开发者预剔除**——"玩家不可见"假设同时贯穿几何与贴图两侧，白模完整≠原始资产完整。

### 3.2 渲染模式（DataView）色块替换机制

数据视图下建筑/props 整体换成纯色，**没有纹理采样**，三个来源：

```hlsl
// A. 顶点色透传（主路径，vehicleDataViewPS / streetPropSetupColorVS）
float3 baseColor = pow(abs(In.texcoord1.rgb), 2.2.xxx);      // gamma 解码
finalColor = DataViewLightingModified(screenUV, normal, viewDir, baseColor, baseColor);

// B. CPU uniform（Pylons 等无顶点色回退）
float3 baseColor = pow(abs(globalDataView.baseColor.rgb), 2.2.xxx);   // struct {float4 baseColor; float4 fadeInfo;}

// C. 轮廓调制
Current.color *= contourColor.a;  Current.color.a = contourColor.a;
```

光照固定参数（`DataViewLightingModified`）：**半兰伯特** `n_dot_l = dot(sunDir,normal)*0.5+0.5`（不 saturate）+
Blinn-Phong-Schlick（specE=10, reflectance=0.1）+ 5% 环境项。即"红绿蓝色块"= 材质
DataView 颜色 × 半兰伯特太阳 shading。

地块级多色区块走 **overlay SDF 四通道**路径（`overlayBlend4Chan/SDF`）：
overlay 纹理 RGBA=距离场，`masks = greaterThan(pixelRGBA, maskCenter−borderWidth)` 四通道掩码，
最终色 `dot(colorsR/G/B, masks) + dot(bordersR/G/B, borderMask)` —— 一个地块最多 4 种色块+各自边框
（这就是 RCI 分区地图的经典实现）。视差遮挡用 `globalDataView.fadeInfo`、相机淡入淡出用 `zoneFill.y/z`。

---

## 4. 天空盒 / 环境光照参数（Q4）

### 4.1 cSunSkyInfo —— 全引擎唯一的大气/太阳 uniform

```hlsl
struct cSunSkyInfo {
    float4 mPerezA, mPerezB, mPerezC, mPerezD, mPerezE;  // Perez 分布系数 A–E（xyz 三通道矢量化）
    float4 mInvDensity;      // Perez 输出 xyY 整体缩放（大气密度反比）
    float4 mSunDir;          // 太阳方向（归一化）——全引擎主光方向
    float4 mZenith;          // .w = 色调映射黑阶（gammaOnlyToneMap: sqrt(max(c - mZenith.w, 0))；lensBloom: clamp(c,0,mZenith.w)+bloom）
    float4 mSunColor;        // 太阳颜色/辐照度
    float4 mSkyColorTuning;  // .x=亮度乘数(Y)  .y=色度向白点(0.3333,0.3333)收敛系数（去饱和）
    float4 mDynamicWeather;  // 地面大气雾双层参数：x=density y=falloffZ z=density2 w=falloffZ2
} sunSky;                    // 一次上传 11×float4
```

**.w 通道复用**：`mPerezA.w`=scatterStrength（散射强度）、`mPerezB.w`=baseDensity、
`mPerezC.w`=densityStart（非体雾路径 `SimCityDensity` 的参数）。
`mZenith.xyz` 在着色器内无使用（仅 CPU 语义）。

### 4.2 天空色计算：Preetham/Perez 解析式 → CIE xyY → RGB

```hlsl
float Perez(lambdas, cosTheta, gammaS, cosGamma)
{ return (1 + lambdas.x*exp(lambdas.y/cosTheta)) * (1 + lambdas.z*exp(gammaS) + lambdas.w*cosGamma²); }

// 实际启用的是压缩 LUT 版（kUseSkyTable=true，s11 寄存器）：
float3 SkyColorTableCompressedWithTuning(table, info, cosTheta, cosGamma, s)
{
    s = (0.5 + 3*s) / 8;                          // 第二坐标=8 档粗糙度/卷积级别
    float c = 0.5*(cosTheta+1);
    float g = sqrt(0.5*(1-cosGamma));
    thetaTerm = lerp(thetaMin, thetaMax, tex2D(table, float2(c, s)).xyz);   // 表上半
    gammaTerm = lerp(gammaMin, gammaMax, tex2D(table, float2(g, 0.5+s)).xyz); // 表下半
    xyY = (1+thetaTerm)*(1+gammaTerm) * mInvDensity;
    xyY.xy = 白点 + (xyY.xy − 白点) * mSkyColorTuning.y;   xyY.z *= mSkyColorTuning.x;
    → xyYToXYZ → dot(kXYZToR/G/B, cieXYZ)                 // CIE XYZ → 线性 RGB
}
```

`kXYZToR=(2.80298,-1.18735,-0.437286)`、`kXYZToG=(-1.07909,1.97927,0.0423073)`、
`kXYZToB=(0.0746015,-0.248513,1.08196)`；`kAvoidDesaturation=true`（Y saturate）。
`sunSkyRadianceScales.thetaGammaMinMax[4]` 提供 theta/gamma 项的 min/max 反缩放。
解析式 `SkyColorTG` 仅作 `kUseSkyTable=false` 回退。**没有独立 skydome mesh**——
天空=全屏路径解析计算；`effectSimpleSkyParticleShader`（云/天粒子）、`skybridgePointShader`
（点膨胀四边形、`skybridgePointAlphaPS` alpha×0.4）是叠加层。

### 4.3 大气/雾（deferred 雾 PS）

```hlsl
kGroundScale=1024; kGroundZero=-870; kGroundAtmosphereStart=-870;
kUseQuadraticDensity=true;    // tau = exp(-(density·density))
kScatterStrength=1.5;         // mPerezA.w 同源
双层指数：density/falloffZ + density2/falloffZ2（farDepthZ=0.99 换层）
散射色 = ScatterColor(sunSky, viewDir)：cosGamma→acos，cosTheta=lerp(|1-cosGamma|*0.2, 1, |v.z|) 查同一 LUT
体雾 kUseVolume=false（Crytek 式 VolumetricDensity 代码保留但关闭）
```

地形对位常量：高程域 ±1024（`height*2048-1024`）、`worldUV = worldXY/768`（体积雾噪声）、
雾盒 `boxSize=3*1024`。`mGammaAndFogInfo[3]`：[0].yzw=fogStrengthInfo、[1].xyz=fogColor、[1].w=fogQuality（ray-march 步数）。

---

## 5. 光源种类与特性（Q5）

### 5.1 总表

| 光源 | 数量 | 参数结构 | 衰减/形状 | 阴影 |
|---|---|---|---|---|
| **太阳（方向光）** | 1（SH+deferred 全局） | `sunSky.mSunDir/mSunColor` | saturate(n·l)，太阳漫反射 `pow(sunMod,1+gloss)` | `parallelLight0ShadowApply`（3 档 PCF） |
| **平行光组** | 最多 4 | `parallel[4]{lightDir,color,level,fillColor,fillLevel}` | 每盏主色/强度+填充色/填充强度 | 前 3 盏独立阴影开关 |
| **SH 环境球谐** | 9 或 16 系 | `shCoeffs[16]`（CPU 每帧上传） | kSH_A1=2/3、A2=1/4；16 系镜面版含 `(5z²−1)` 等项 | 无 |
| **方向光（SH 前向）** | 1 | `dirLightsWorld[1]{mDir,mColor}` | 专供镜面热点 | — |
| **球形区域光** | 4 | `sphereLights{mColors4(4x3),mA2,mPosition[4]}` | `t=r/sqrt(a²+r²)` 查 BRDF LUT（brdfZT），`(1-gloss, gloss)` 混合 diff/spec | — |
| **点光（deferred）** | 逐灯 pass | `deferredLight{diffColor,params,position,endPosition}` | `atten=saturate(falloffBias−d²·1/(r²−inner_r²))`，atten²×ndotl（doubleSided 用 abs） | 可选 gel 投影 |
| **聚光（deferred）** | 逐灯 pass | 同上（position.w=外锥 cos，endPosition.w=内锥 cos，params.zw=falloffStart/Dist） | 轴向衰减 × `coneAtten²`；锥角是**半角余弦**（`radius*0.97` 低细分补偿） | `spotShadowPF` |
| **线光/胶囊光** | 逐灯 pass | 同上（p0/p1+双半径） | 径向同点光 + 端帽 `pow(saturate(...),1.5)`；`intersectRayCylinder` | — |
| **投影光 gel** | 点光附加 | `worldToModel` + `texCUBE(gelSampler)`（双 sampler 可 lerp） | cubemap 图案遮片 | — |
| **体积光雾** | 点/锥/柱附加 | `mGammaAndFogInfo` + `lightFogSampler`（风吹噪声：`worldXY/768 − windDir·time`） | ray-march 累积；`LINE_LIGHT_FOG_STRENGTH=1.5` | — |
| **云影** | 全局 | `cloudShadowShader`：3 层云图 | 高度 {200,230,350}、速度 {(-13,-60),(14,-49.5),(28,-88)}、尺寸 {5900,6148,11096}；`shadow=0.45+0.55·clamp(g+b−0.5r)` 最低 0.45 | — |
| **自发光（交通灯）** | 常量 | `kRed=(50,0,0) kYellow=(50,50,0) kGreen=(0,50,0) kBlue=(40,40,50)` | streetpropIllumPS | — |
| **调试光** | — | debugPoint/Spot/LineShader | `Current.color=In.color`；lightHalfIntensityPF ×0.5 | — |

### 5.2 延迟光源统一结构（字段按光型复用）

```hlsl
struct { float4 diffColor; float4 params; float4 position; float4 endPosition; } deferredLight;
// 点光: position.xyz=中心 w=radius；endPosition.w=innerRadius(全亮内径) x=volStrength y=doubleSided；params.x=Kd y=Ks
// 聚光: position.w=外锥半角cos；endPosition.w=内锥半角cos；params.z=falloffStart w=falloffDist（+Kd/Ks/volStrength/doubleSided）
// 线光: position.xyz=p0 w=radius；endPosition.xyz=p1 w=innerRadius；params.z=volStrength w=doubleSided
```

点光：`Current.color.rgb = diffLightColor.rgb * (ndotl * atten * atten)`；
镜面 `pointSpecPF`（Blinn，alpha 通道承载 spec）。聚光：
`atten = saturate(falloffBias − distance·denomFalloff)`（>1 视为 0），总衰减 `ndotl·atten·coneAtten²`。
几何 pass：`pointQuadVF/spotQuadVF/lineVF(+Instanced)` 膨胀体 + `*StencilFront/BackFace` 模板双面剔除。
阴影 3 档：style6=硬件 PCF；style7=4-tap 加权（offset 0.5/0.25）；style8=receiver-plane bias（源码注释 doesn't work）。
最终 `shadow *= shadow;  // plump, smooth, and darken shadow (important!)`。

### 5.3 对 PE 视口的可用结论

PE 真光源直接用 `lot_unit.rs` 的 Light color/radius/diffuse/length 字段，对应游戏
deferredLight 结构：点光=position.w(radius)，线光=双端点+端帽衰减，聚光=内外锥 cos——
即可在 three.js 里以 `PointLight/SpotLight` + 手工衰减曲线近似。

---

## 6. 对本项目（fluffy-open-scp）的落地建议

1. **窗户（下一阶段 slot3+interior）**：消费 slot3 shader map A 通道 → 窗洞；interiorMap 纹理
   + `interiorScale/Offset`（slot0 row0.zw 或顶点 B 通道解码 4bit size+4bit index）+
   `interiorRoomInvSize`（顶点流）→ 逐窗格 FastNoise 选房 + 盒体投影。夜间亮窗
   `interiorMap.a×16`，供电/废弃状态做 uniform。
2. **质感**：把 tint 亮度链升级为完整链——specE=`tintResult.a³×2048+1`、
   specStrength=`shaderMap.b×2`、reflectance=`specResult.a`、gloss=`saturate(a×specStrength)`；
   环境项可先用常数天空色近似 EnvLighting（法线/反射方向插值 + diff/spec 劈分）。
3. **UV 公式疑点（待验证）**：源码 `uv = texcoord0.xy / abs(tileSize)` 后才
   `frac(uv)·regionXform.xy + regionXform.zw`，而 regionXform.xy≡tileSize（clip 窗 W,H）。
   我们当前实现是 `frac(texcoord0.xy)` 直取。若金样本 tileSize≈1 则二者等价；若非 1，
   facade 平铺比例会差 tileSize 倍——建议用 info 浮层复制 tileSize 实测。
4. **relief（slot5）**：本编译版 reliefMap 为恒等（被裁剪），只有常量留痕
   （kFlatLevel=23/255、kConeSteps=8、kBinarySteps=2、kReliefDepth=0.1）；
   完整 cone/binary step 实现需找其它 cpp 变体或按标准 relief mapping 补写。
5. **DataView 模式**（若做"渲染模式"预览）：纯色 `pow(2.2)` + 半兰伯特，无需纹理；
   分区色块可复刻 overlay 四通道 SDF 方案。
6. **天空/环境**：PE 若要氛围光，可用 cSunSkyInfo 结构 + Perez LUT 近似，
   或简化为 `sunDir/sunColor + 9 系 SH`（shCoeffs 是 CPU 上传，游戏从天空预计算）。

## 附：着色器家族清单（转储 0 头部 0–200KB 家族表，字母序分组摘录）

- **建筑/模型**：building4Shader、building4ColorOverlayShader、building5Shader、generic_static(=_nolightnofog(noshadow)(_occlude)…)、generic_skinning、generic_morphed、generic2D、generic_dataview_lit、generic_lot 系（genericLot4ChanOverlay/SDFOverlay/AlphaOverlay/Lawn…）、generic_zone、impostorSimCameraFacing、sim_impostor、simModel、impostorTree、NonImpostorTree、streetprop、VehicleV3Shader、VehicleV3PylonShader
- **贴花**：decalProject(Front/Lit/SDF/Neon…)、decalInteriorMap、decalNeonTubeSDF、regionDecalProjectLit
- **光照**：globalLightsShader、globalShadowShader、pointShader(+Fog)、spotShader(+Fog/HQ)、lineShader(+Fog)、point/spot/lineStencil(Back/Front)FaceShader、cloudShadowShader、applyLightingFogShader、atmospherics
- **天空/特效**：effectSimpleSkyParticleShader、skybridgePoint(Alpha)Shader、postLensFlare、applyBloom/applyGlow/applyDoF、hejlToneMap、filmicMap、gammaOnlyToneMap、Gauss{3x3..13x13}_{X,Y}、motionBlur、volumeFire
- **地形/水**：TerrainHeightShader(Rel/Normals/Thickness)、terrainRegion(Skirt)Shader、hdWaterShader、WaterHeight/Choppy/NormalsShader、grassShader2、grassSlice/Volume、ecoMapCombiner、TerrainBrush 系、TerrainFilterErosion
- **道路**：networkRoad(Basic/BasicInteractive/Emissive/FutureDirectional/FutureGlow/Transition)SH、rwRoadShader、roadMaskShader(Bridge/NoLerp/Old/Tunnel)、road_demolish_tile(_outline/_textured)、roadGuideDashedShader、selection/outline 系列
- **UI/工具**：lotToolPlopPreview(TopDown/BorderPreview)、lotToolPlopBorderPreview、radialVis(Stencil/Color)、uiQuadShader、TexturePreview、Movie*、WebView*、CircleShader(ZoomFade/ZoomOccludeFade)、bargraph_occlude
- **调试**：debugPoint/Spot/LineShader、debugRM、showAlphaAsDither、vsAOVolumeDebug

关键采样器寄存器：s11 skyTermTableXXX（天空 LUT）、s12 lightColorSampler、s13 lightDepthSampler(sampler2DShadow)/lightScalarSampler、s14 normalSampler、s15 depthSampler、gel s0/s1（texCUBE）、lotTextureSampler s1、overlayTextureSampler s0。
