# Blender 白模 + 六槽位资产管线前驱简报

> 2026-10-23 整理。用途：供 Blender MCP agent 搭建"白模 + 游戏原生六个贴图槽位"
> 的合成路线，为 agent 驱动的建筑资产开发做前驱探索。
> 范围按需求**只含三项**：模型比例尺、游戏坐标系、六槽位合成与建筑表面映射公式。
> 全部结论出自本项目已定谳的逆向文档与已对拍实现，证据索引见文末。

---

## 1. 模型比例尺

### 1.1 基本单位：1 游戏单位 = 1 米

引擎全链以米为隐含单位，多条独立证据互相锁定：

- lot 包围盒属性 `BoundingBox 0x00F9EFBA` 默认值 **±1**（±1 米，`dev-dump-lot-placement-and-script-resources.md` §2.1）；
- relief 视差距离剔除 `kCullDistanceSq = 16000` → **≈126 米**外跳过视差（`building-facade-engine-analysis.md` §3）；
- 路网/城市边界、地形高度、decal 贴墙中位偏移（2.5~4.5 m）全部同数量级互洽；
- 单位世界变换平移 z = GroundHeight（贴地高度，米）。

**Blender 设置：场景单位制选"公制"，长度 1 Blender 单位 = 1 米，与游戏 1:1，无需缩放。**

### 1.2 尺寸速查表（已实证数值）

| 对象 | 数值 | 出处 |
|---|---|---|
| 区域源场宽度 | 32768 m（±16384 m） | `map-bounds-roads-and-bridges.md`、`region-city-coordinate-sync-and-placement-alignment.md` |
| 区域可见窗口 | 8192 / 16384 / 32768 m 三档 | `map-static-bootstrap.md` |
| 城市模拟域 | 地块中心 ±1024 m，即 **2048 m × 2048 m** | `map-static-bootstrap.md` |
| 城市模拟网格 | 128 × 128 格，**每格 16 m** | `map-static-bootstrap.md` |
| 区域地形高度场 | 4096² u16，8 m/像素，`z = raw/32 − 1024`（值域 ±1024 m） | `map-editor-flow-requirements.md` |
| 城市地形高度图 | `topDownUV = position.xy / 2048 + 0.5`（2048 m 见方） | `rendering.md` §3.1 |
| LotMask 栅格 | **0.75 m/px**（主流换算 64px ↔ 48m；1.0 m/px 试点已被目视反证回退） | `overview/rendering-pipeline.md` §10 |
| relief 视差剔除 | √16000 ≈ 126 m | `building-facade-engine-analysis.md` §3 |
| 内景种子打包域 | x ∈ ±1024、y ∈ ±512（`InteriorSeed` 归一化模型位置） | Maya 源 `std_VS`，`maya-building4-authoring-shader.fx_source.txt:558` |

### 1.3 建筑实例的尺度参照

- 消防局金样本（0x4DE9912B）上层前脸窗板 x ∈ [2.52, 4.64] ∪ [4.64, 7.75]、窗带 z ∈ [14.6, 22.87]——
  即典型服务建筑**临街面约 8 m 宽、檐口约 23 m 高**（`building-facade-engine-analysis.md` §9.8，坐标即米）；
- 建筑前缘贴 zoning lot 前缘、沿 frontage 居中、深度向 lot 内推进；z 吸附 `GroundHeight`；
- 注意 `BoundingBox` 是**声明值**（默认 ±1 m × 均匀缩放属性），不是网格几何包围盒；引擎用它与
  frontage/depth 属性推 lot 尺寸：`frontageMin = max(属性, bbox.x) − 1.0`、`depthMin = max(属性, bbox.y)`。

---

## 2. 游戏坐标系

### 2.1 世界系：右手 + Z-up + 米

```
+X = 东，+Y = 北，+Z = 上（kSPUpVector = (0,0,1)）
```

与 **Blender 默认主轴系完全一致**（Blender = 右手 Z-up 米）。导出时保持 Z-up，
**不要**按 glTF 惯例转成 Y-up——游戏 RW4 模型与引擎变换全链都是 Z-up。

### 2.2 建筑（unit）本地系约定

对白模建模最要紧的一组约定（`dev-dump-lot-placement-and-script-resources.md` §2.3，
函数体级实证）：

| 项 | 约定 |
|---|---|
| 原点 | lot 矩形**中心** |
| +X | frontage 轴（沿街方向），x ∈ ±frontageMin/2 |
| +Y | depth 轴，**+Y = 前缘 = 临街侧**（前缘中心在 +depth/2 侧），y ∈ ±depthMin/2 |
| +Z | 上 |
| 贴地 | 模型 z=0 平面 = 地面；放置时 z 吸附 `GroundHeight` |

### 2.3 变换数学（放置链，影响导出器而非建模本身）

- `cSPTransform = { 旋转 S（三个**行向量** xAxis/yAxis/zAxis），scale，translation }`；
  点作用 `p_world = t + s·Sᵀ·p`——**存储矩阵的列**是本地基向量在世界系的像；scale 作用在旋转**之后**。
- 放置变换读取时**强制 scale = 1、旋转压平为纯 yaw**（z 列强制 (0,0,1)）——建筑放置只有平面旋转。
- 单位世界变换 `U = W ∘ P`（W = lot→world，P = model→lot）；正向放置由 zoning lot 凸包前缘方向
  f̂ 构造：`localX→(−f̂.x, f̂.y, 0)`、`localY→(−f̂.y, −f̂.x, 0)`、`localZ→(0,0,1)`（右手系）。
- 对 Blender agent 的含义：**建模只需保证 §2.2 的本地系约定**；朝向/临街侧由数据层
  placement transform 决定，模型里不要预旋转。

### 2.4 纹理 V 方向与切线

- Maya 工具链导出时对所有 texcoord 执行 `y = 1 − y`（`maya-...fx_source.txt:457-464`，
  "Maya inverts our texcoords"）。Blender 导出脚本需做同款翻转，并**用金样本对拍一次 V 方向**
  （D3D9 采样 V 向下）。
- 切线空间：`binormal = cross(tangent, normal)`，`bumpNormal = (normalMap.rgb×2−1)` 经
  `float3x3(tangent, binormal, normal)` 变换（Maya `std_PS:934`）。白模应带**真实逐顶点切线**；
  缺失时运行时才用 dFdx 拟合（openscp 已验证同构，但作者资产带切线流）。

---

## 3. 白模顶点流（合成路线的输入端）

building4 模型每个顶点必须携带（Maya `vertexInput` + EXPORTED 通道，逐字）：

| 顶点属性 | 内容 | 编码 |
|---|---|---|
| Position | 本地坐标（米，Z-up，原点=lot 中心） | float3 |
| Normal | 面法线 | float3 |
| Tangent | 切线（binormal = cross 推导） | float3 |
| TEXCOORD0.xy | **Base 层 facade UV**（tile 单位，见 §5.2） | float2 |
| TEXCOORD0.zw（=TEXCOORD1） | **Top 层 UV**（窗户 motif 层，见 §5.3） | float2 |
| COLOR0.r | PaletteIndexBase（Base 层调色板列基址 palU） | 0..255 |
| COLOR0.g | PaletteIndex（Top 层调色板列基址 palU2） | 0..255 |
| COLOR0.b | 内景图集打包：**高 4bit = size(1..16)，低 4bit = selection(0..size−1)**；运行时 `d = b×(16×255/256)`，`size = floor(d)+1`，`selection = floor(frac(d)×16)` | u8 |
| COLOR0.a | 内景随机种子 `interiorRandomSeed`（0..1） | u8；`<0.5` 时由模型世界位置经 FastNoise 兜底 |

补充：零售版着色器里这些标量也可烘焙进 slot0 参数表 row0（两来源一致，见 §5.1）；
COLOR0.a/阈值类数据传给 PS 前有 `floor(x×255)+0.5` 量化防插值漂移。

---

## 4. 六个贴图槽位（合成路线的输入端之二）

以 LOTM v10 容器口径为准（`overview/rendering-pipeline.md` §2；逐槽证据见
`building-facade-engine-analysis.md` §2.1、`rendering.md` §1.5）：

| 槽位 | 内容 | 通道语义 | 格式/色彩空间 |
|---|---|---|---|
| **slot0** | **参数表**（f32，非颜色） | 80 列 × 4 行，材质 m 行 k = `flat[k*80+m]`；row0=(palU, palU2, interiorScale, interiorOffset)、row1=regionXform(Base)、row2=regionXform2(Top)、row3=(padX, padY, roomInvSize.x, roomInvSize.y) | R32F 80×4，线性 |
| **slot1** | tint / Color Control Map | **rg = 调色板查表键**（连续子采样坐标，条目间渐变是设计行为）；**b = 亮度倍率**（乘 ×2）；**a = 覆盖掩码/窗洞**（≥0.5 保留） | RGBA8，线性 |
| **slot2** | 法线 | **rgb = 切线空间法线**（×2−1 解码）；**a = AO**（artistAO） | RGBA8，线性 |
| **slot3** | shader map | **x = relief 高度**（Depth）；**y = cone 比**；**g = 高光强度**（实测资产约定画在 G；源码字面读 B，项目已定谳取 G）；**a = 窗洞/Interior 位置**（0=窗/透内景，1=外立面） | RGBA8，线性 |
| **slot4** | tint palette（调色板） | 512×16 = **256 条目 × 2×2 像素块**；V 向 7 个变体行 + 底部 surface 行；查表输出 tint(RGB=颜色, A=高光指数) 与 surface(R=自发光强度, G=反射率/cubemap 强度, A=roughness) | RGBA8，**sRGB**；Maya 采样 POINT |
| **slot5** | interior map（内景房间图集） | rgb = 预渲染房间图（每格后墙占 50%）；**a = 逐像素灯亮**（HDR 自发光 ×16） | RGBA8，**sRGB** |

**家族警告**：以上是**建筑（building4）专用语义**。车辆/prop 是另一族：s0=diffuse、
s1=normal、s2=shader——把建筑槽位语义套到 prop、或反之，是项目已定谳的"灰车/白模"根因。
agent 产资产时先确认目标家族。

`facadeAtlas / Palette / Interior` 三张纹理在官方管线中**全场景统一**（共享图集）；
Base/Top 的 regionXform 就是把模型 UV 映进共享图集的子矩形。首枚测试资产用整幅
（tileSize=(1,1), offset=(0,0)）最简单。

---

## 5. 建筑表面映射与合成公式

以下为 building4 像素链的完整公式，顺序即合成顺序。公式的项目对拍实现：
`src/pages/packages/components/property-editor/refinedRender.ts`（与游戏截图逐症对拍通过），
烘焙端权威布局：`src-tauri/src/package_service.rs:3400-3800`（MaterialBake）。

### 5.1 slot0 参数表 → 每材质的四个 row

```
row0 = (palU, palU2, interiorScale, interiorOffset)
row1 = regionXform  = (tileSizeBase.xy, tileOffsetBase.xy)     # Base 层图集矩形
row2 = regionXform2 = (tileSizeTop.xy,  tileOffsetTop.xy)      # Top 层图集矩形
row3 = (tilePadding.xy, roomInvSize.xy)
```

无参数表时，row0 的四个标量改由顶点色携带（COLOR0.b 解码 interiorScale/Offset，
COLOR0.r/g 解码 palU/palU2）——两条来源等价。

### 5.2 Base 层采样（所有 facade 纹理的公共地址）

```
uv     = TEXCOORD0.xy / abs(tileSize)                          # VS（kWorldToTextureRatio = 1）
baseUv = frac(uv) * regionXform.xy + regionXform.zw            # PS → 图集子矩形
uv_ddx = ddx(uv * regionXform.xy);  uv_ddy = ddy(...)          # 导数在"未 fract 连续域"计算
```

- UV0 是 **tile 计数单位**：整数部分=平铺序号（被 frac 吃掉），小数部分=一张贴图内的位置；
  `|tileSize|` 把它缩放进图集矩形。tileSize=(1,1)、offset=(0,0) 时退化为 `baseUv = frac(UV0)`。
- **负 scale = 镜像子矩形，一等公民**（`reliefEyeDir.xy /= abs(regionXform2.xy)`）；
  内缩/判活逻辑必须用 `abs()`，判活只看"是否为 0"。
- 采样口径：**LINEAR + mip + 最大各向异性**，显式导数（`textureGrad`）——fract 边界隐式导数
  爆炸是"近距模糊/接缝亮线"的已定谳根因。

### 5.3 Top 层采样（窗户/浮雕层，可关闭）

```
# 导出期烘焙（Maya 等价变换）：
padRel = tilePadding / abs(tileSize2) − 1                      # 归一化 padding
uv2_baked = (TEXCOORD0.zw / abs(tileSize2) + padRel*0.5) / (1 + padRel)

# 运行期还原（PS）：
uv2   = frac(uv2_baked)
uv2   = uv2 * (1 + padRel) − padRel * 0.5                      # = reliefSrc
reliefSrc = uv2 沿视线在 slot3 高度场做 cone-step 视差 → relief_tc
outsideTile = any(relief_tc 超出 [0,1]) > 0
relief_tc = relief_tc * regionXform2.xy + regionXform2.zw      # 图集子矩形
```

**tilePadding 编码规则**（0-hack，Maya 源逐字）：

| 输入 | 含义 |
|---|---|
| TilePadding = 0 | 取 `abs(TileSize)`（默认=同 Base） |
| TileU / TileV 未勾选 | 该轴 padding = **1000** → **Top 层关闭**（资产实证值 15999 / 8e4 即此机制） |
| > tileSize | 露出 Base 层边缘；< tileSize | 子域重复，relief 可视外扩 |

### 5.4 Relief 视差（slot3.x 高度 + slot3.y cone 比）

零售版 cone-step 参数（`building-facade-engine-analysis.md` §3）：

```
kReliefDepth = 0.16   kMaxConeRatio = 0.5
kConeSteps = 8        kBinarySteps = 2          # Maya 作者版 32/16
kCullDistanceSq = 16000（≈126 m 外不步进，直接平铺）
kFlatLevel = 23/255   # 高度 ≤23 视为平面，跳过步进
```

cone 比不是画出来的——**烘焙时从高度图预计算**（每纹素最大锥角，官方 P37"高度+锥角打包"）。
`outsideTile > 0` 时 `facadeTintValuesTop.a` 置 0 → 整格回退 Base 层。

### 5.5 双层混合（所有 facade 通道同一 lerp 因子）

```
t = tex(slot1 tint, relief_tc).a          # Top 覆盖率；outsideTile 时 = 0
tintResult   = lerp(tint@baseUv,  tint@relief_tc,  t)
normalSample = lerp(normal@baseUv, normal@relief_tc, t)
specSample   = lerp(shader@baseUv, shader@relief_tc, t)
```

### 5.6 调色板查表（slot1.rg → slot4）

```
# 查表键（连续子采样坐标，允许条目间渐变）：
sub = tint.rg * (1/512, 1/16) + (1/1024, 1/32)          # 半纹素居中，防串色
paletteUv = (palU    + sub.x,  uPaletteRow + sub.y)      # Base 列
paletteUv2= (palU2   + subTop.x, uPaletteRow + subTop.y) # Top 列（row0.y）
# uPaletteRow = buildingVariation（0..6 变体行；REPEAT 回绕）；贴图 REPEAT 寻址

输出两行：
tint    = (RGB 颜色, A = 高光指数)
surface = (R = 自发光强度, G = 反射率/cubemap 强度, A = roughness)
```

- 已对拍实现（`refinedRender.ts:893-912`）与上式逐字一致；slot4 为 **sRGB**。
- palette 布局：512×16，256 条目 × 2×2 像素块；V 向 7 变体行 + 底部 surface 行
  （Maya `kNumPaletteVariations = 7`，`kInvPaletteHeight = 1/8`）；
  文档版行带语义：RGB 上带=Tint 色 / 下带=Emissive(R=G=B)，A 上带=Specular Power / 下带=Roughness。
- 新资产最小配方：单行 palette（变体行 0 + surface 行），tint.rg 画成指向条目的渐变坐标。

### 5.7 镂空与窗洞（两条独立通道，勿混淆）

```
1) 硬裁剪（clip pass）：baseUv 处 tint.a < 0.5 → discard      # 无条件，无朝向豁免
   # 镂空模板只按外立面 UV 创作；下向面继承立面窗洞是原版固有瑕疵（仰视不可达），
   # 项目观察器用"法线 Z<−0.3 豁免"缓解——新资产建议把底面 UV 单独布局规避
2) 内景混合：finalColor = lerp(interiorColor, exteriorColor, shaderMap.a)
   # shaderMap.a = 0 → 透内景（窗）；= 1 → 外立面（含玻璃高光路径）
```

### 5.8 假内景（slot5，可选阶段）

```
interiorUv    = uv * regionXform.xy * roomInvSize        # roomInvSize = row3.zw
interiorElem  = floor(interiorUv)                        # 每窗格一个整数单元
roomId        = FastNoise(float3(interiorElem, interiorRandomSeed))
roomVariation = floor(roomId * 4);  roomId = frac(roomId * 4)
interior_edge = float4(step(thresholds, roomId.xxx), roomVariation * 4)
interior_tc.x += dot(interior_edge, 1) * interiorScale   # 图集分格选择
interior_tc   = interiorMap(eyeDir...) * interiorScale + (0, interiorOffset)

FastNoise(seed): seed *= (78.233, 12.9898, 43758.5453); seed += 0.819*each;
                 return frac(seed.z * frac(seed.x * frac(seed.y)))

interiorMap(eye, tc, invDepth=0.5, backSize=0.5, dilation=0.9):
    eye.z *= invDepth; pos = tc*−2+1; pos.z −= 1
    t = min((sign(eye)−pos)/eye); target = pos + t*eye
    target.xy *= lerp(dilation, backSize, target.z)
    return target.xy*−0.5 + 0.5

interiorColor = interiorMap.rgb * (shDiff + interiorMap.a * 16 + shSpec)
# kBuildingInteriorMapSelfLightMax = 16（HDR；设计文档 20，零售调低 → 从 16）
```

interiorScale/Offset = row0.zw（或顶点 COLOR0.b 解码）；thresholds 三档来自
InteriorThreshold1/2/3，z 分量被复用为供电开关（0 = 一票关掉全部自发光）。

### 5.9 最终合成与材质参数链（零售版权威口径）

```
specStrength  = shaderMap.g * 2                        # kBuildingSpecOverdrive = 2（文档版 12，零售调低）
specE         = tintResult.a³ * 2048 + 1               # 高光指数（立方曲线；Maya 作者版 a×2048+2）
gloss         = saturate(tintResult.a * specStrength)
AO            = normalMap.a
exteriorColor = tintColor * (shDiff + surfaceR²·powered × 256 + shSpec) * AO + extraEmissive
finalColor    = lerp(interiorColor, exteriorColor, shaderMap.a)
finalColor   += tintColor * spec + envColor * surfaceG * specStrength   # cubemap 反射，加法、不过反照率
废弃态：去饱和 0.75、暗化 0.55、AO⁸、高光 ×0.05（cap 0.05）
```

`sRGB` 收尾：颜色类纹理（slot4/slot5）采样时解码、输出端编码；数据类（slot1/2/3/参数表）直通。

### 5.10 常量总表（Blender 端复刻用）

| 常量 | 零售版 | 备注 |
|---|---|---|
| kBuildingInteriorMapInvDepth | 0.5 | 三源一致 |
| kInteriorMapBackSize | 0.5 | 三源一致 |
| kBuildingInteriorDilation | 0.9 | 防图集格间渗色 |
| kBuildingInteriorMapSelfLightMax | **16** | 设计文档 20 → 零售调低 |
| kBuildingFlatLevel | 23 (23/255) | 高度图平面阈值 |
| kBuildingSpecOverdrive | **2** | 设计文档 12 → 零售调低 |
| kReliefDepth / kMaxConeRatio | 0.16 / 0.5 | |
| kConeSteps / kBinarySteps | 8 / 2 | Maya 作者版 32 / 16 |
| kCullDistanceSq | 16000（≈126 m） | |
| kNumPaletteVariations | 7（+1 surface 行） | kInvPaletteHeight = 1/8 |
| MAXIMUM_PALETTE_SIZE | 256 | 贴图 512×16 |

### 5.11 最小可行白模合成检查单（第一枚测试资产）

1. **几何**：米制、Z-up，原点=lot 中心，+X 沿街、+Y 临街、+Z 上；带法线+切线；
   底面 UV 单独布局（避开镂空继承）。
2. **UV0**：Base 层贴图 UV（0..1 一张贴图）；**UV2**：可先与 UV0 相同。
3. **slot0**：4×80 f32——row0=(palU, palU2, 1/图集格数, 格号×scale)、row1=((1,1),(0,0))、
   row2=((1,1),(0,0))、row3=(0,0,roomW,roomH)。
4. **slot1 tint**：rgb = 指向 palette 条目的渐变坐标（0..1）；b = 1（亮度倍率基线）；
   a = 255（不镂空）或窗洞形状（<128 镂空）。
5. **slot2**：rgb = 切线法线，a = AO 灰度。
6. **slot3**：x = 高度（窗框/浮雕凹凸，≤23 视为平面）、y = 由高度图预计算的 cone 比、
   g = 高光强度（玻璃处拉高）、a = 1（全外立面）或窗处 0。
7. **slot4 palette**：512×16 sRGB，行 0 = tint 色带（RGB+A=高光指数），底行 = surface
   (R=自发光, G=反射率, A=roughness)；tint.rg 对准即可。
8. **slot5 interior**：首测可整幅填 a=0、rgb=中性室内色，验证窗洞透色后再加图集分格。
9. 验收对拍：白模几何 → 四槽静态外观 → palette 变体行 → Top 层窗洞+relief → 内景，逐阶段加。

---

## 6. 证据索引

| 主题 | 文档 |
|---|---|
| 建筑管线全量（六采样器↔六槽位、双 UV 域、relief、内景、高光链） | `docs/re/building-facade-engine-analysis.md` |
| 首轮着色器调研（常量表、家族清单、地形对位） | `docs/rendering.md` |
| 六槽位总表 + LOTM v10 口径 + 家族语义警告 | `docs/overview/rendering-pipeline.md` §2 |
| Maya 作者着色器源码（顶点流/0-hack/padding/内景种子） | `docs/re/maya-building4-authoring-shader.fx_source.txt` |
| 官方材质文档对拍（palette 布局、参数命名、常量差异） | `building-facade-engine-analysis.md` §7.5/§7.6 |
| 坐标系/变换/lot 本地系（Z-up、+Y 临街、yaw-only） | `docs/re/dev-dump-lot-placement-and-script-resources.md`、`docs/re/map-coordinate-rendering-facts.md` |
| 城市域/网格/地形尺寸 | `docs/re/map-static-bootstrap.md`、`docs/re/map-editor-flow-requirements.md`、`docs/re/map-3d-preview-research.md` |
| slot0 参数表烘焙权威布局 | `src-tauri/src/package_service.rs:3400-3800`（MaterialBake） |
| 已对拍查表/合成实现 | `src/pages/packages/components/property-editor/refinedRender.ts`（:893-912 palette 查表等） |
