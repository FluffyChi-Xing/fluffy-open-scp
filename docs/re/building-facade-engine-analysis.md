# 建筑立面渲染——游戏侧实现全量分析（2026-10-05）

> 任务背景：openscp 建筑贴图近距模糊、框架掏空边缘锯齿、消防局正面对称窗只渲染一半、
> 假内景模糊/亮度低。用户指令：**先不看 openscp 现有实现，纯从游戏侧源码定谳引擎怎么做**。
>
> 证据来源：
> 1. `tmp/dynamic/all_blocks_full.txt`——0x0469A3F7 着色器容器**无损重解析**（2406 块；
>    旧解析器丢 >250B 切片的截断问题已由 `tmp/parse_container_tokens_full.py` 修复），
>    建筑族完整块提取在 `tmp/building_shaders_full.txt`；
> 2. `docs/source-code/SimCity-dev-ida.c`——dev exe IDA 全量导出（189 万行）；
> 3. `docs/rendering.md`——2026-09-08 首轮建筑管线调研（本次用无损块对其勘误/补全）。
>
> 重要：本轮从容器恢复出 **reliefMap() 的完整 cone-step 实现**（rendering.md §2.4 当时
> 只拿到恒等裁剪版，注明"需从其他 cpp 变体找"——现已找到，见 §3）。

---

## 1. 管线总览（building4 家族）

```
InteriorAndVariationSetupVS   VS：9 系 SH 环境光 + 太阳直射（顶点级）
                              + 全部内景/调色板参数打包进 8 组 texcoord
        ↓
building4Clip                 窗洞 alpha 裁剪（tint.a ≥ 0.5 保留）+ relief 视差 UV 计算
        ↓
ClipAndReliefMapPS /          正向路径：完整着色（调色板 + 法线 + 内景 + 高光）
InteriorMapPS (half 精度版)
        ↓
DeferredPS                    延迟路径：G-buffer（法线 best-fit 编码 + 深度打包 + specE）
        ↓
CombinePS                     延迟合成：SimCityLighting(屏幕空间) + 内景/外景 lerp
                              + cubemap 环境反射 + FutureGlow 扫描辉光
        ↓
Impostor Pack/Unpack          远景 LOD：打包深度/法线，平面化 + 随机亮窗
```

关键结构事实：
- **窗户不是几何**。窗洞 = tint 图 alpha 裁剪 + shaderMap.a 窗洞遮罩 + relief 视差 +
  interiorMap 假内景，全部像素级（building4ClipAndReliefMapPS）。
- **G-buffer 法线编码**：`outputDepth` 拆 3×8bit（floor/frac 级联），`color.w = specE/255`，
  法线走 `getBestFitNormal` 八面体压缩（DeferredPS）。
- 画质变体：同一公式有 float（ClipAndReliefMapPS）与 half（InteriorMapPS）两版；
  `building4InteriorMapPS` 另有调试变体 `interiorThresholds = 0 强制全暗`。
- MSAA：`rwg_SetPresent`（ida 358964）按设备参数申请后台缓冲多重采样，
  `CheckDeviceMultiSampleType` 逐级回退（2×起步）——**游戏内边缘平滑的一部分来自
  MSAA**，不是着色器技巧。

## 2. 贴图链与"清晰度"的引擎机制

### 2.1 六采样器（与材质六槽位一一对应）

| 采样器 | 槽位 | 内容 | 消费方式 |
|---|---|---|---|
| tintMapSampler | slot1 | facade 遮罩+tint：rgb=调色板子采样坐标，a=Top 层覆盖率/窗洞裁剪 | `tex2Dgrad` 双域（baseUv / relief_tc） |
| normalMapSampler | slot2 | rgb=切线空间法线，**a=AO（artistAO）** | base/top 双采样按 facadeTint.a lerp |
| shaderMapSampler | slot3 | **x=relief 高度，y=cone 比，b/g=高光强度，a=窗洞遮罩（artistOpacity）** | relief 射线步进 + 双域 lerp |
| tintPaletteSampler | slot4 | 256×8 调色板（每材质一行变体色 + 末行 surface 参数行） | BuildingPaletteSample 子采样 |
| interiorMapSampler | slot5 | 内景房间图集（rgb=房间预渲染图，**a=逐像素灯亮自发光**） | 盒体视差投影后采样 |
| reflectionSampler (s6) | — | 环境 **cubemap** | CombinePS 玻璃反射 |

另有全局：s11 天空 LUT（skyTermTableXXX）、s13 lightScalarSampler（阴影标量）。

### 2.2 清晰度的三个引擎机制（对应"近距模糊"）

1. **显式导数采样**：全部主纹理走 `tex2Dgrad(sampler, tc, uv_ddx, uv_ddy)`，其中
   `uv_ddx = ddx(uv * regionXform.xy)`——**导数在"未 fract 的连续 UV × 区域变换"域
   计算**，而非逐 tile 的 fract 后 UV。fract 会打断隐式导数（tile 边界导数爆炸→
   mip 跳变/模糊），引擎用显式导数彻底回避；同时 relief 偏移后的采样仍用原导数，
   保证视差采样不糊。
2. **采样器状态 = 变体对象下发**：运行时纹理由 24B 记录绑定（ida 516100-516148：
   标志位 bit1 → MIN/MAGFILTER、bit2 → MIPFILTER），建筑贴图口径 =
   LINEAR + mip + 最大各向异性（此前 decal 对拍已对齐此口径，
   blog/decal-rendering-pipeline.md:174）。各向异性对**掠射角立面**（近距离侧看
   墙面）是清晰度主因——纯三线性在该角度必然糊。
3. **调色板子采样去网格化**：`BuildingPaletteSample` 子采样坐标 =
   `tint.rg × (1/512, 1/16) + (1/1024, 1/32)`——半纹素偏移居中采样，避免调色板
   邻近条目串色（kSubsampleScale/Offset，building_shaders_full.txt:79-88）。

### 2.3 双 UV 域：Base 层 vs Top 层（对应"消防局半窗"）

```hlsl
baseUv   = frac(uv)  * regionXform.xy  + regionXform.zw;    // row1：Base 层矩形
relief_tc= reliefMap(fract(uv2)…) * regionXform2.xy + regionXform2.zw;  // row2：Top 层矩形
tintResult = lerp(tintBase, tintTop, facadeTintValues.a);   // a = Top 层覆盖率
```

- **窗户 motif 在 Top 层**：Base 矩形无窗，Top 矩形内是窗户图案，靠第二套 UV（uv2 =
  texcoord0.zw）× row2 矩形逐格平铺成整面窗阵（migration.md §30，探针 0xF8FFC5F8
  实证：Top 矩形内 shaderMap.a<128 占 36~53%）。
- 引擎侧可导致"半面墙无窗"的机制只有两处：
  a) `outsideTile > 0 → facadeTintValues.a = 0`——relief 视差把 UV 推出 [0,1] 时
     **Top 层（窗户）整格失效**，回落 Base 层（无窗墙面）；
  b) uv2/regionXform2 数据本身只覆盖部分立面（美术数据）。
  排查"对称窗只渲染一半"时，这两点是对拍锚点：uv2 是否存在/被正确读取、
  relief 视差是否把一半窗格的 UV 推出界、row2 矩形是否只盖半面。
- 窗洞裁剪（building4Clip，"scissor window"）：`alpha = tint.a ≥ 0.5 ? 1 : 0`，
  硬 alpha-test。边缘锯齿的引擎缓解 = MSAA（§1）+ relief 视差提供的立体遮挡，
  **着色器内无任何边缘柔化**——alpha 裁剪边就是硬边。

## 3. Relief mapping（本次无损重解析的新恢复，rendering.md §2.4 补完）

`reliefMap()` 完整实现（building_shaders_full.txt:898-935）= **Cone Step Mapping**
（shaderMap.x=高度、shaderMap.y=cone 比）：

```hlsl
ds.xy = -ds.xy;  ds.z *= 1/kReliefDepth;  ds *= 1/ds.z;
tc -= ds.xy * flatDepth;
// 8 步 cone search：步长 = (cRat × saturate(h - s.z)) / (dsRat + cRat)
// 2 步二分细化
```

常量：`kReliefDepth = 0.16`、`kMaxConeRatio = 0.5`、`kConeSteps = 8`、
`kBinarySteps = 2`、`kCullDistanceSq = 16000`（**距离² > 16000 ≈ 126m 外跳过视差，
直接平铺**）、`kFlatLevel = 23/255`（高度图 ≤23 视为平面，跳过步进）。

- 窗框/掏空的**立体纵深边缘**来自这里：视线方向在高度场里步进，窗洞边缘产生
  真实的遮挡视差——近距离看窗框有"凹进去"的厚度，而非一张平图。
- `tilePadding` 先把 UV 外扩（`reliefSrc × (1+padding) − padding/2`）防 tile 边界穿帮。
- 这是"框架掏空时大量边缘锯齿"的核心对拍点：引擎的掏空边缘 = alpha-clip 硬边
  **+ relief 视差边**（视差把硬边在掠射角打散成立体轮廓）+ MSAA。缺 relief 时
  掏空 = 纯 2D 硬裁剪，锯齿全部暴露。

## 4. 假内景（Interior Map）——完整公式

### 4.1 房间栅格化与逐格随机

```hlsl
interiorUv    = uv * regionXform.xy * interiorRoomInvSize;  // 按房间尺寸栅格化立面
interiorElem  = int2(floor(interiorUv));                    // 每窗格一个整数单元
interiorSrcUv = frac(interiorUv);
roomId        = FastNoise(float3(interiorElem, interiorRandomSeed));  // 逐格伪随机
roomVariation = floor(roomId * 4);                          // 4 种房型
interior_edge = float4(step(interiorThresholds, roomId.xxx), roomVariation*4);
interior_offset = dot(interior_edge, float4(1,1,1,1));      // 0..3 图集列选择
interior_tc.x += interior_offset * interiorScale;           // atlas 分格
```

`interiorThresholds`（VS 顶点流，floor(x×255)+0.5 量化防插值漂移）= 三档亮灯阈值：
roomId 超过第 i 档 → 使用第 i 列房型图；**z 分量被复用为供电开关**（powered=1 /
断电·废弃=0，源码自注释 "hack"）——一票关掉全部窗户自发光与外景 emissive。

### 4.2 盒体视差投影（房间进深的来源）

```hlsl
interiorMap(eye, tc, invMapDepth=0.5, backSize=0.5, dilation=0.9):
    eyeDir.z *= invMapDepth;                 // 房间进深 = 半宽
    pos = tc×-2+1, pos.z -= 1;               // 入口平面 → 单位盒前墙
    k = (sign(eyeDir)-pos)/eyeDir; t = min(k.x,k.y,k.z);   // 射线-盒求交
    target = pos + t×eyeDir;
    target.xy *= lerp(dilation, backSize, target.z);       // 后墙收缩 = 透视纵深
    return target.xy×-0.5+0.5;
```

- 同一份图集，不同视线角命中盒内不同面 → **不同角度看窗户内容不同**（真视差）。
- `kPadShrink = 0.9`：内缩防角部渗色；`kInteriorEdgeBlock`（本编译版 = false）
  用 `ddx/ddy(interiorElem)` 在窗格边界把 interiorMap.a 衰减到 0 防拉伸。
- 图集 cell：`interiorScale/interiorOffset` = 参数表 row0.zw（或顶点色 B 通道
  4bit 尺寸 + 4bit 索引解码，ImpostorPackVS 注释）。

### 4.3 内景着色与"亮度"（对应"假内景模糊/亮度低"）

```hlsl
interiorColor = interiorMap.rgb * (shColorDiff + interiorMap.aaa * kBuildingInteriorMapSelfLightMax + shColorSpec);
// kBuildingInteriorMapSelfLightMax = 16（HDR 值）
```

- interiorMap.**a = 逐像素灯亮通道**：a×16 加进亮度——夜间亮窗的引擎机制，
  16 是 HDR 强度，靠 hejl/filmic tonemap 软肩回收（PE 无 HDR 管线时直乘会过曝，
  不乘则暗——这是"亮度低"的首要对拍点）。
- 白日内景同样吃 `shColorDiff`（环境天空项）——内景不是纯黑贴图，是
  "环境光 × 房间图 + 灯亮自发光"。
- 远景 impostor：`shaderMap.a ≤ 0.8` 的窗像素按
  `FastNoise(interiorElem, 0.2134) > interiorLightTime` 随机压黑 = 夜晚城市
  窗户逐格亮灯。
- 内景采样是普通 `tex2D`（隐式导数）——interior_tc 在窗格内连续，mip 正常；
  **模糊的对拍点 = 盒体投影公式与 dilation/backSize 参数是否逐字**。

## 5. 玻璃幕墙

引擎**没有玻璃材质分支**（全部 26 个 building4 文件无 metalness/glass 分支），幕墙 =
参数化叠加：

1. **透明感**：`shaderMap.a ≈ 0` → `lerp(interiorColor, exteriorColor, artistOpacity)`
   偏向内景——幕墙 = 大面积窗洞 + 内景；
2. **高光**：`specStrength = shaderMap.b × kBuildingSpecOverdrive(2)`（资产实证作者
   把高光画在 **G 通道**，rendering.md §2.1 修正）、`specE = tintResult.a³ × 2048 + 1`
   （调色板 surface 行 a）、`gloss = saturate(tintResult.a × specStrength)`；
3. **天空镜面**：EnvLighting 按 gloss 把采样方向从法线偏向反射方向
   （`normal×(1-s) + reflect×s`，s = gloss×0.75），查天空 LUT——掠射角玻璃反天光；
4. **cubemap 环境反射**（CombinePS）：
   `reflectedView = view − 2·dot(n,view)·n; rgb += texCUBE(reflectionSampler, reflectedView) × specResult.y × specStrength`
   ——幕墙的"反射城市/天空"观感来源；
5. 掠射角软化：`bentViewDirection.z += 2×saturate(−z)×(1−saturate(bumpNormal.z))`
   ——低角度高光不炸。

## 6. 光照强度链（对应"光照强度对拍"）

- **VS（顶点级）**：9 系 SH（kSH_A1=2/3、A2=1/4，shCoeffs CPU 每帧上传）+
  太阳直射 `saturate(dot(sunDir, n)) × sunColor` → shDiffuse/shSpec 插值到 PS。
- **PS（延迟版 SimCityLighting）**：
  `EnvLighting(天空 LUT×阴影环境项) + 太阳漫反射×directShadow +
   Blinn-Phong-Schlick 高光((specE+2)/8 能量归一 + Schlick 菲涅尔 exp2(−8.656·cosLH))
   + GetDeferredLight(screenUV)（延迟灯光累积缓冲：lot 点光/聚光/线光）`。
- **自发光**：外景 `artistEmissive = specResult.r² × interiorThresholds.z`，
  `exteriorColor = tint × (shDiff + emissive×256 + shSpec) × AO + extraEmissive`
  ——**×256 过压**（HDR，tonemap 回收）；内景 a×16（§4.3）；
  FutureGlow 扫描带 kFutureGlowBrightness=50。
- **废弃**：去饱和 0.75、暗化 0.55、AO⁸、高光 ×0.05（cap 0.05）。
- **收尾**：hejlToneMap / filmicMap / gammaOnlyToneMap（mZenith.w 黑阶）+ 大气雾
  （双层指数 Perez 散射）。

## 7. 引擎侧结论 → openscp 症状的对拍锚点

| 症状 | 引擎机制 | 对拍锚点（openscp 侧待查项） |
|---|---|---|
| 近距贴图模糊 | tex2Dgrad 显式导数（未 fract 域）+ LINEAR/mip/最大各向异性 + 调色板半纹素居中 | 采样是否 fract 后隐式导数；anisotropy 是否 = 渲染器上限；调色板子采样偏移 |
| 掏空边缘锯齿 | alpha-clip 硬边 + relief cone-step 视差（8+2 步）+ MSAA 后台缓冲 | relief 是否实现/距离剔除 126m；alphaTest 阈值；渲染器 MSAA（antialias）开关 |
| 消防局对称窗半渲染 | 窗户在 Top 层（uv2 × row2）；outsideTile>0 → Top 层整格失效回退 Base | uv2 读取、row2 矩形覆盖域、relief 视差是否把半面窗格 UV 推出 [0,1] |
| 假内景模糊 | 盒体投影 invDepth=0.5/backSize=0.5/dilation=0.9 + 逐格 FastNoise 选房 | 投影公式逐字对拍；房间图集 cell 选择（row0.zw）；窗格栅格 roomInvSize（row3） |
| 假内景亮度低 | interiorMap.a×16 HDR 自发光 + shColorDiff 环境项 + tonemap | a 通道是否消费、自发光系数、无 HDR 管线时的等效曝光 |
| 幕墙不像玻璃 | shaderMap.a≈0 内景 + specStrength/specE/gloss + 天空 LUT 反射向 + cubemap s6 | 高光通道（b/g）、cubemap 反射是否缺失、gloss 环境项 |

## 7.5 官方材质制作文档对拍（2026-10-06，Discord Danny50205 提供）

Maxis 官方材质参数文档（设计期）逐项对照发行版二进制与本项目实现：

### 7.5.1 全局常量（文档 vs 发行版着色器 vs 项目）

| 常量 | 文档（设计期） | 发行版着色器 | 项目 | 判定 |
| --- | --- | --- | --- | --- |
| kBuildingInteriorMapInvDepth | 0.5 | 0.5 | 0.5 | ✓ 一致 |
| kInteriorMapBackSize | 50% | 0.5 | 0.5 | ✓ |
| kBuildingInteriorMapSelfLightMax | **20** | **16.0** | uInteriorGlow=16 | 发行版调低，**从发行版** |
| kBuildingFlatLevel | 23 | 23（reliefPng 平面钳制） | — | ✓ |
| kBuildingSpecOverdrive | **12** | **2.0** | ×2 | 发行版大调低，**从发行版** |
| ReliefMapConeSteps/BinarySteps | 8→20 / 4→10 | relief 恒等被裁剪 | 未实现 | 印证 cone-step 编码 |
| kAbandonedDesaturate/Darkening/AOExp | 85%/70%/8 | 同 | 未实现 | ✓ |
| kAbandonedSpecScale/Cap | 25%/0.05 | 同 | 未实现 | ✓ |

（文档=设计期参数，发行版两处大调低——SelfLightMax 20→16、SpecOverdrive
12→2。对拍基准=发行版二进制。）

### 7.5.2 调色板布局（文档给出官方语义——本文新增）

256 条目 × 2 列（512 宽），**四逻辑行带**（文档 4 行；资产 512×16 = 每行
2px × 2 子采样）：

```
RGB:  行 1-2 = Tint 色        行 3-4 = Emissive（实际 R=G=B）
A:    行 1-2 = Specular Power  行 3-4 = Roughness（反射清晰度）
```

对照发行版着色器：`colorValues`（RGB tint，乘 tint.b×2）+ `surfaceValues`
（kSurfacePalV 底行：R=emissive 强度、A=reflectance）——**一致**。新增认知：
①palette 含**独立 Emissive 带**（实际 R=G=B → 白色×强度近似成立）；
②alpha 双语义 = Specular Power（上带）/ **Roughness（下带，反射清晰度
——发行版未消费，记录备查）**；PaletteSize 恒 256 ✓。

### 7.5.3 材质参数（Maya 编码为顶点数据——语义命名对照）

| 文档参数名 | 项目对应 | 新增认知 |
| --- | --- | --- |
| PaletteIndex / PaletteIndexBase | palU / palU2（base & overlay 双 pass） | ✓ 命名实证 |
| TileSizeBase/OffsetBase / TileSize/Offset | regionXform row1 / row2 | ✓ |
| **TilePadding（实为 VisibleTileSize）** | row3.xy | **0-hack：0=默认=TileSize**；>TileSize 露 base；<TileSize 重复子域且 reliefmap 可视外扩 |
| TileU / TileV（开关） | — | **未勾 ≈ TilePadding 1000**——实证发现的大 padding 值（15999/8e4）机制即此（Top 层关闭）|
| UseSpecifiedRandomSeed / MaterialRandomSeed / ForceSpecificVariation | vSeed（内景随机种子） | **同 seed 的多材质对同一房号选出同一内景**——楼内一致性由 seed 保证（项目 per-model seed 已满足）|
| InteriorMapSize | interiorScale（行数 4/8/16） | 图集行数可变 |
| InteriorMapSelection | interior_offset | 行号选择；房间点亮 = 模拟入住率（A→B→C→D 调试参数）|
| InteriorMapRoomSize/Back/ForeRelative | roomInvSize / interiorOffset | fore/back 相对缩放按 TileSizeBase/TilePadding |

内景图集：每个 interior 占图集 **50% = 后墙**，墙面深度**线性**（非透视）
——与盒体投影参数互证。facadeAtlas/Palette/Interior 三纹理全场景统一。

### 7.5.4 结论

文档 = 设计期官方参数（无实现错误），但两处核心常量被发行版调低；布局/机
制描述与本项目逆向**全面互证**（TilePadding 语义、TileU/V 关闭、内景 50%
后墙、palette 256 条目）。项目无需改动；Roughness 带/入住率点亮记录备查。

- `tmp/dynamic/all_blocks_full.txt`（无损 2406 块）、`tmp/building_shaders_full.txt`
  （建筑族 5 个完整块：ClipAndReliefMapPS 23.8KB / InteriorAndVariationSetupVS 6.5KB /
  UnpackDeferredPS 4.7KB / facadeTint 2.0KB / SamplersPS 27.4KB）。
- **reliefMap() 完整 cone-step 实现**（rendering.md §2.4 "需从其他 cpp 变体找"结案）。
- cubemap 反射项（rendering.md §2.3 只记了 vehicle 路径的 cubemap ×0.3；建筑
  CombinePS 的 `texCUBE(reflectionSampler)` 本次首次入档）。

---

## 9. openscp 实现对拍（2026-10-05，`refinedRender.ts` 逐行对照）

公式层（调色板子采样/双域 lerp/窗洞 discard/盒体投影/FastNoise/高光链）与引擎
逐字一致，分歧集中在**采样口径与两个历史回滚**：

### 9.1【主因候选】tint 图被强制 NearestFilter + 全纹理禁 mip（refinedRender.ts:243-301）

- 引擎：`tex2Dgrad` + **LINEAR** + mip + 最大各向异性（§2.2）。tint.rg 是调色板
  **连续子采样坐标**（两条目间线性渐变是设计行为），不是离散索引。
- openscp：tint/palette 强制 NearestFilter（:286、:297，2026-09-19"门窗被平均成
  平墙条目"的误治）；tint/normal/shader `generateMipmaps=false`（:262-264，
  2026-09 接缝修复：fract 边界隐式导数爆炸 → mip 错乱亮线，于是禁 mip）。
- 后果链：近距放大时 tint Nearest = 马赛克方块（"锯齿感"），normal/shader
  Linear 无 mip = 放大糊（"模糊"）；中远距无 mip = shimmer/闪烁。
  `anisotropy` 设了上限但无 mip 不生效（:266 注释自承）。
- **正解（引擎同构）**：恢复 mip + LINEAR + 各向异性，采样改 `textureGrad`
  （WebGL2 内建；GLSL1 用 EXT_shader_texture_lod），导数 = 引擎同款
  `ddx(vTintUv × xform.xy)`（未 fract 域）——这同时是当年接缝问题的正解
  （导数连续 → 硬件 mip  footprint 不再爆炸），半 texel 内缩（:683）保留。

### 9.2【掏空立体感】relief 视差回滚的两个前提均已被新证据推翻（:703-710）

- 回滚理由①"reliefPng 是 slot5 alpha、高度通道语义不可靠"——用错了通道。
  容器无损恢复定谳：**高度 = shaderMap.x、cone 比 = shaderMap.y**（§3，
  `tex2Dgrad(shaderMapSampler, …).x/.y`），shaderPng 已在 GPU（:747 已采
  scShaderMap），数据零成本可得。
- 回滚理由②"游戏本编译版 reliefMap 本就是恒等"——那是旧解析器截断的
  编译变体；完整 cone-step 实现（8+2 步、kReliefDepth=0.16、
  kMaxConeRatio=0.5、kCullDistanceSq=16000、kFlatLevel=23/255）现已恢复（§3）。
- 实装后窗框/掏空获得视线视差的立体边，正是"框架掏空边缘锯齿/不像游戏"
  的引擎对照差。

### 9.3【消防局半窗】逐顶点调色板列选择的硬切换（:663-670）

- PE：参数表 Nearest 采样，"跨列三角形在列边界中线切换"；:695 注释定谳
  消防局窗户 = Base 层逐顶点选列（D3DCOLOR.G）。
- 引擎：palU 作为顶点属性**插值**后进 LINEAR 调色板采样，跨列三角形两列
  渐变混色，不会"一半消失"。
- 头号嫌疑：vMatUV 列地址对齐（(col+0.5)/cols 半列偏移）——Nearest 下偏半列
  = 一半窗格选到相邻素墙列 = "对称窗只渲染一半"。9.1 改回 LINEAR 后此症
  可能同愈；若不愈，下一步取证该模型逐顶点 D3DCOLOR.G 与列地址。

### 9.4【幕墙反射】cubemap 环境反射项缺失（:876-885）

- 引擎 CombinePS：`rgb += texCUBE(reflectionSampler, reflectedView) ×
  specResult.y × specStrength`（§5.4）；PE 只有 EnvLighting 天空近似，且
  scSurface.g（= specResult.y）已采样未消费（:768-771 只取 .a）。
- 低成本近似：`reflectedLight.indirectSpecular += scEnv × scSurface.g ×
  scSpecStrength`（程序天空代 cubemap，方向同源）。

### 9.5 内景亮度链基本对齐（:824-836），残留两点

- scSelfLight = a × uInteriorGlow(2.5~16) × powered ✓；缺引擎的 `+ shColorSpec`
  镜面项（小）；盒体投影 eyeDir 用 dFdx 拟合切线架（:798-809），GLB 带
  TANGENT 时可换真实切线流（:846-849 法线已这么做）——内景视角畸变的
  候选改善点。
- "模糊"若指窗格内容：interiorMap 采样是普通隐式导数 + mip 保留（:305
  seamless=false），口径没问题；更可能是 9.1 的全局观感连带。

### 9.6 修复顺序建议（按症状收益/回归风险排序）

1. **9.1 采样口径**（textureGrad + mip + aniso + tint 回 LINEAR）——一条链
   同时治"近距模糊/锯齿/远景 shimmer"，但触动接缝历史雷区，需金样本对拍
   （消防局 0x4DE9912B / 0xF8F776BF、玻璃楼 0xCFEC0F84）。
2. **9.2 relief cone-step**——数据已在手，独立增量，治掏空立体感。
3. **9.4 cubemap 近似**——两行改动，治幕墙质感。
4. **9.3 半窗取证**——若 1 之后仍在，再开探针。

### 9.7【调色偏差】building4 链 sRGB 色彩空间缺失（2026-10-05 消防局实证定谳）

**症状**：消防局前脸饱和红砖渲染成品红 (182,5,80)，上层砖洗白偏粉 (150,105,98)，游戏对应为深红 (153,0,6) / 暗砖红 (125,45,28)。

**取证链**：
1. `building_material_probe` 平面合成（UV 域，逐字对齐 building4 公式）产出正确砖红 (163,77,65)——资产数据无问题，偏差在 GPU 采样路径。
2. 逐点模拟：palette 值 (0.639,0.302,0.255) 以 NoColorSpace 采样（sRGB 值被当 linear）→ 光照 ×~0.55 → 输出端 three 默认 outputColorSpace=SRGB 再做 linear→sRGB 编码 → (161,115,106)，与截图实测 (150,105,98) 逐通道吻合。饱和红经双重编码后 R 维持高位、G 贴近 0、B 被相对抬升 → 色相红→品红，与实测 (182,5,80) 方向、量级一致。
3. 代码对拍：`refinedRender.ts` simple diffuse 链 `baseColorPng` 已标 `SRGBColorSpace`（:423 附近），但 building4 tint 链 `loadTex` 从未设 `colorSpace`——**不对称即病根**。

**语义判定**（数据驱动）：
- palettePng RGB = 颜色（sRGB）→ 必须 `SRGBColorSpace`；alpha = specA（spec 指数链数据源），WebGL sRGB 解码不动 alpha → spec 链不受影响。
- interiorPng RGB = 房间图集颜色（sRGB）→ 必须 `SRGBColorSpace`；alpha = HDR 灯亮强度（×16 链），同样不受影响。
- tintPng（rg=调色板索引、b=亮度倍率、a=覆盖掩码）、normalPng、shaderPng、paramsF32 = 纯数据 → 维持 NoColorSpace。

**修复**：`refinedRender.ts` 调色板与内景加载分支各加 `t.colorSpace = THREE.SRGBColorSpace`（已实装）。引擎侧对应 D3D9 `D3DSAMP_SRGBTEXTURE`/`D3DRS_SRGBWRITEENABLE` 语义——颜色纹理采样解码、数据纹理直通。

### 9.8【半窗定谳】镜像区域（负 scale）双重绝杀 + 窗列 Top 层语义修正（2026-10-05）

**症状**：消防局上层正面对称双窗只渲染一半（左），右半为素砖。

**取证链**（全部数据驱动，工具：`building_vertex_cols` + `building_material_probe` 全列 row0-3 导出）：
1. 立面马赛克：上层前脸 = 壁板大矩形拼合，中央双窗 = 两块窗板（col7 x∈[2.52,4.64]、col6 x∈[4.64,7.75]，z∈[14.6,22.87]）+ 尖拱盖帽（col13/14，z∈[23,24.3]）。每块窗板承载一扇完整拱窗 motif；两板并置 = 双窗。
2. 窗列 Top 层语义（全列参数表首次导出）：col5-8 palU2≠palU（0.18359 vs 0.07031）、interior=0.125、pad.y≈2497——**窗户走 Top 层**，旧结论「大 padding = Top 关闭、消防局窗户来自 Base 层选列」对窗列不成立。pad.y 刀带配合**退化 uv2**（窗板 uv2.y≈0.5±0.0003 常数线、uv2.x 横跨板面）把 motif 钳进板面中段 ~67% 竖带，上下回退 Base——引擎原意。
3. 病根一：PS 判据 `xform2.x > 0` 把 col7（row2 scale.x=**−0.26913**，镜像区域）整块跳过 → scFacade=0 → 回退砖墙。引擎 `reliefEyeDir.xy /= abs(regionXform2.xy)` 证明镜像是一等公民。
4. 病根二：内缩公式 `max(xform2.xy − uTintTexel, 0)` 对负 scale 输出 0 → 采样坐标塌缩为常数竖线（与病根一叠加；Base 层同公式同样坑了 row1 col1=−0.0476 的镜像隅石板）。
5. 离线逐像素模拟（复刻着色器公式 + 镜像修正）产出：双窗板 + 双拱盖帽 + 隅石全貌，与游戏结构一致；未修正版本（无 Top 路径）全砖无窗，与 openscp 现状一致——修正方向确证。

**修复**（`refinedRender.ts`）：
- Top 层判据 `xform2.x > 0 && xform2.y > 0` → `abs(xform2.x) > 1e-6 && abs(xform2.y) > 1e-6`（0 才是禁用标记）。
- Base/Top 两处内缩统一改为 fract 域比例内缩：`inset = clamp(texel/|scale|, 0, 1)`，`f' = fract·(1−inset) + inset/2`，`tUv = f'·scale + off`——正 scale 与旧公式逐值等价（可验证：r·(s−t)+o+t/2 ≡ (r·(1−t/s)+t/2s)·s+o），负 scale 正确覆盖镜像区域。
- Rust 烘焙链（`MaterialBake`）天然免疫（无 max 钳制，`fract·xform+off` 直乘），无需改动。

**已知余项**：pad.y 刀带使窗 motif 仅渲染中段竖带（上下回退砖）；游戏内 reliefMap() 视差步进在越界检查**之前**修订 relief_tc，可能扩大有效带——待 9.2 relief 实装后复测窗台/窗眉衔接。

## 9.9 假内景"正视漆黑/亮度低"定谳（2026-10-05，已实装待目视）

用户观测：窗户假内景只有侧视可见，正视一片漆黑；白天亮度低；调时段到晚上内景漆黑。
离线仿真（消防局 0x4DE9912B col6 真实参数 + slot5 真实图集，Python 复刻
scInteriorMap 盒体投影链）证明：**投影数学本身在正视下输出有效内容**
（正视采样格心 [0.25,0.75]²，仿真亮度 0.14 linear），漆黑来自渲染通路的两个实病：

1. **浏览器 PNG 预乘 alpha 摧毁 rgb**（主因）：`<img>` 解码路径把 rgb 预乘
   alpha 存储、WebGL 上传时再除回——a=0 处 rgb 归零、a≈0 处量化失真。
   DXT5 原生 rgb/alpha 独立存储，a=0 处 rgb 完好（slot5 取证：全图 rgb
   均值 (46,37,26)，a≡0 的图集格同样带有效 rgb）。内景图集内容大量靠
   a×16 自发光项点亮，rgb 被毁后正视（mip0 点采样受损区）→ 漆黑；斜视
   靠 mip 混合邻域幸存像素才可见——正视/斜视不对称的成因。
   **修复**：全部 5 张材质 PNG 改 ImageBitmapLoader +
   `premultiplyAlpha:'none', colorSpaceConversion:'none'`（refinedRender.ts
   loadTex），浏览器交出文件原始值 = 引擎采样 DXT5 原始数据同口径。
2. **自发光系数白天压 2.5 vs 引擎恒 ×16**：仿真实测引擎白天 0.372 vs
   openscp 0.140（38%）。引擎 kBuildingInteriorMapSelfLightMax=16 无昼夜
   插值，昼夜差全在 shColorDiff 环境项。**修复**：env.glow 恒 16，着色器
   内 1.0 以上软肩（c>1 → 2−1/c）替代引擎 hejl tonemap。
3. **漏 `roomId = frac(roomId*4)` 再归一化**（引擎 OCR 行 14）：取过
   variation 后须重新归一化再与 interiorThresholds 比较。已补。

夜间口径：引擎夜间内景 = rgb×(shColorDiff_night + a×16)——环境项小、
灯亮项恒定，亮灯窗夜间**不**随 daylight 压暗；openscp 现状（scNightAmb
下限 0.10-0.18 + a×16）已与此同构，本次只是把 glow 从插值改恒定。

## 9.10 无内景窗户"死黑无玻璃质感"定谳（2026-10-05，已实装待目视）

用户观测：无假内景的窗户（格窗玻璃）直接纯黑，无玻璃质感；同楼门玻璃
内景（暖黄）正常 → 内景链本身已通，缺口在**玻璃反射层**。

病根：环境天空镜面项 `indirectSpecular += scEnv × scEnvS × diffuseColor.rgb`
被**反照率调制**——窗像素经内景 lerp 后反照率≈0 → 反射归零 → 正视
（太阳 Blinn 高光≈0 的角度）玻璃窗 = 纯黑。

引擎 CombinePS 原文：`rgb += texCUBE(reflectionSampler, reflectedView)
× specResult.y × specStrength`——环境反射**加法叠加、不过反照率**，
黑玻璃照样反射天空/城市，这正是"玻璃质感"的来源（doc §5.4 早有记录，
本次才接上）。

修复：反射项改为 `scEnv × scEnvS × scReflectance × scSpecStrength`
（specResult.y 取 scReflectance 口径）。自调节安全性：墙面 specA 低 →
gloss≈0 → scEnvS≈0 → 墙面不泛光；玻璃 specA/strength 双高 → 反射强。
