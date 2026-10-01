# Decal 引擎侧全量重分析——三源互证定谳（2026-10-01）

> 背景：广告投影穿模（正反面都印）、悬空判据无实锤、破洞漏投/悬空
> 等问题反复出现，根因是既有分析基于**被截断的 shader 源码**加猜测。
> 本次推倒重来，三源互证：
> ① shader 容器**无损重解析**（新解析器 `tmp/parse_container_tokens_full.py`）；
> ② Ghidra 反编译引擎源码 `docs/source-code/SC_cVolumeDecalManager.c`；
> ③ 脱壳 exe 常量直读（基址 0x400000 无 ASLR，Ghidra 地址 = 文件偏移可算）。

## 〇、截断根因（方法论勘误）

容器格式 = 扁平 `[u8 len][text]` 串流，**单 token 上限 250B**。旧解析器
`parse_container_tokens.py` 对每个块名只保留最长单个 token——超过 250B
的片段被切成多个连续 token，**其余切片全部被丢弃**。例如
`decalAnimateSDFDisabled` 实际 10388B，旧输出只有 118B。
新解析器按「块名 + 连续 body 串流」重组（`tmp/parse_container_tokens_full.py`），
产物 `tmp/dynamic/decal_full_0.txt`。250B 边界偶有字符缺失（len 字节
校验失败跳字节），但内容量级完整。**此前基于截断源码的结论一律以本文为准。**

## 一、引擎 decal 到底是什么（架构定谳）

三条独立证据链指向同一结论：

**证据 A——Ghidra `SC_cVolumeDecalManager.c`**（引擎唯一 decal 管理器）：

- `FUN_006fe190`（挂载）：遍历 lot 的 decal unit 列表（0x2c 步进池），
  读可见类别 **0xCAAD8CA**（float，`flags&0x30` 选直接值/指针解引用），
  判据只有一条：`if (DAT_00cf1e4c < category)`——exe 实值
  **DAT_00cf1e4c = 0.0f**，即 `category > 0`（可见即可）。
  随后 `FUN_006f7d40(变换, category, 0x10 /*16 槽上限*/, 记录, …)` 建记录。
- `FUN_006fdce0`（置变换，每 decal 记录 0x4c 字节）：
  - 记录 [0xf..0x12] = **0x0DA76A05/06/07** + 管理器默认值（4 个 float
    = shader 的 `decalMaterialData` 光参数；unit 属性存在则覆盖）；
  - 0xCAAD8C9（materialInstance）→ `FUN_006f8bd0(计数, 指针)` 上传材质数据；
  - **矩阵 = 纯数据构造**：基行 = unit 旋转 3×3 × scale(+0x34) ×
    DAT_00da307c(**0.5**) × DAT_00da30b0(**10.0**)；平移 = 位置(+0x28..)
    + 旋转·偏移 × scale。**无射线、无距离比较、无几何查询。**
- `FUN_006fd730`（绘制分发）：`(pass & 0xf)==1` → 前向逐批
  `FUN_006fa4b0`（16 slot 合批）；`==2` → 延迟屏幕空间（`FUN_006fccf0`
  + `FUN_006fac10`，对应 decalClipBack/regionDecal 系）。
- `FUN_006fa160`（设备资源）：顶点格式 `V3F_STR_T4F_T4F_T4F`
  （位置 + texcoord4..6），**36 索引盒体 IB（12 三角形）** +
  8 顶点实例数据（角点 0/1，DAT_00cf1e50 = **1.0f**）——
  **decal 就是按变换摆放的一个贴图盒体几何**。

**证据 B——shader 容器完整源码**（`tmp/dynamic/decal_full_0.txt`）：

- `decalProject`(VS)：`texcoord<t0> = mul(modelToTexture, float4(modelPos,1))`
  ——顶点（盒体角点）经矩阵变换，UV 来自盒体自身局部坐标；
- `decalClip`(PS)：`clip(-textureFloatPosition.z)`（**半空间裁剪，只此一条**）
  + `uv = texpos.xy × -0.5 + 0.5`（U 取负）`× texXform.xy + texXform.zw`
  （**atlas 格窗口**，来自顶点/实例数据）+ `Current.color = tex2D(s0, uv)`
  （直采 + alpha 混合，无四色展开、无超采样、无边界处理）；
- `decalFloatQuad`(VS)：`texcoord<t0>.xyz = indices.yzw × (1/255)`——
  浮空族的 UV 烤在顶点索引字节里，同样是自持几何；
- `clip(1 - abs(texturePosition))` **不属于**招牌/广告族——它属于
  `regionDecalProject`（区域 decal，注释原文 "clip to within the volume"）
  和 `decalSDF`（调试可视化：把盒外像素涂白）。早先文档把它记成
  "decalProject PS" 是截断拼块的张冠李戴。

**证据 C——exe 常量直读**：

| 符号 | 实值 | 语义 |
|---|---|---|
| `DAT_00cf1e4c` | **0.0f** | 可见类别判据阈值（`category > 0` 即渲染） |
| `DAT_00da307c` | **0.5f** | 体积盒基缩放因子 |
| `DAT_00da30b0` | **10.0f** | 体积盒基缩放因子（合计 ×5×scale） |
| `DAT_00cf1e50` | **1.0f** | 盒体角点坐标值 |
| `DAT_00d20d04` / `DAT_00cf4550` | ±FLT_MAX | 空包围盒初始化（0xCAAD8CA 体积效果路径用） |

### 结论（一句话）

**引擎 decal = 按 Unit 变换数据摆放的贴图盒体几何，靠 shader 半空间
裁剪与深度测试与场景合成；从数据到渲染不存在任何"距离判定 / 射线锚定 /
几何求交"的显式测试。"贴墙"是两层机制的结果：摆放期把原点粗略放在
墙面区域（烤数据）+ 渲染期大体积盒罩面（延迟模式 = 最近深度自动落墙）。
不存在"距离 < X 判吸附"的运行时阈值。**

### 与建筑的耦合机制（2026-10-01 扫查实证）

用户追问"如果渲染无条件，decal 与建筑的关系是什么、为何有的贴墙有的
不贴"。扫查探针 `crates/sc-exporter/examples/decal_anchor_sweep.rs`
对 SimCityDataEP1 120 个含 decal 的 lot 量测 origin→建筑最近三角形距离，
按材质族（字典 0xCAAD8C9，SimCity_Game/Graphics.package 中的 7 个
atlas 字典 → 3 族）分组：

```
family 0x4491DE3A(破洞): n=194  dist min=0.03 med=4.51  | bbox内 175/194
family 0x73684EFC(招牌): n=107  dist min=0.28 med=2.75  | bbox内  77/107
family 0xE5390A98(涂鸦): n=756  dist min=0.00 med=2.61  | bbox内 676/756
（直方图详见探针输出；三族共同形态：≈10% 贴面(<0.5m)、主体聚 1~5m、
  少数远抛 5~47m）
```

判读：**数据原点不做厘米级贴墙**（中位 2.5~4.5m，甚至常在墙体内部），
因为引擎体积盒（基 = 旋转×scale×0.5×10 → 全深 5×scale）大到必然罩住
立面，延迟投影把广告落在"相机可见的最近表面"上——吸附是渲染期的
几何结果，不是摆放期的精确数据。为何有的不贴：远抛样本（5~47m，
~10%）= 全息 billboard 等浮空设计的族/实例；少数漏配原点的实例在
游戏里同样悬空（引擎不救）。

**实现修正（第三轮，最终形态）**：`decalProjector` 盒尺寸 = 横向可见窗
`2×scale×aspect` × Z 全深 `5×scale`（`DECAL_BOX_DEPTH_FACTOR=2.5` 半深），
中心恒为变换原点。此前薄盒（depth 钳制 0.5~2m）在"原点在墙内 2.6m"
的主流数据形态下够不着任何面 → 回退浮空 quad → "漂浮在半空"的最后
根源。横向是否同为 5×scale+texXform 窗口（C 侧基系数三轴同）留运行时
VB 对拍——若为真，广告落点在盒 xy 中心而非原点，需窗口重映射。

## 二、四个问题的引擎实锤答案

### Q1 广告穿过招牌模型、正反面都印广告

引擎侧招牌板模型与 decal 盒体是**两个独立对象**：板在 opaque pass 写深度，
decal 延迟路径按**最近深度**投影——每个像素只落在最靠投影仪的表面上，
板背面（背向投影仪）不可能同时着色。
我们的病根：`DecalGeometry` 把盒内**所有**网格无差别切片，招牌板（属于
lot 模型网格，在 `buildingMeshes` 里）前后两面都在薄盒内 → 两面都印。
**已修**：`projectDecal` 增加逐三角形法线过滤（`keepFrontFaces`，仅保留
面法线朝向投影仪的面）——延迟路径"最近深度"的 CPU 廉价等价。

### Q2 悬空/投影的判据：距离是确定的还是变量控制？

**引擎没有任何距离判据。** C 侧（FUN_006fe190/FUN_006fdce0）只有
`category > 0`（可见性）；shader 侧只有 `clip(-z)` 半空间。招牌与墙面
有少量间距的游戏内表现 = **变换数据本身就带这一点间距**，引擎原样渲染
（盒子浮在离墙几厘米处，游戏视角下不可见）。"距离 < 0.5 判吸附"这类
阈值在引擎里**不存在**——我们的 ≤10m 投影路由、60m 射线、[0.5,2]m
厚度钳制全部是发明物，**已删除**：盒心恒为变换原点，半厚 = depth
原值（0x 属性的 origin→平面距离语义，§41.2），只留 0.05m 下限防退化。

### Q3 decal 尺寸与广告牌模型无关

引擎记录矩阵只读 unit 自身的旋转/缩放/位置字段（FUN_006fdce0 不接触任何
模型资源）；盒子尺寸 = 变换数据（基 × scale × 5），与广告牌网格尺寸零耦合。
全息广告大于广告牌 = 数据作者把 scale 写大，天经地义。我们的
`2×scale（半高）× aspect` 口径（OMEGACO 游戏实测校准）保持不变；
渲染路径本来就不读广告牌网格，无代码改动。
（存疑项：引擎基的 ×5 因子与我们 2×scale 观感口径的精确关系，见 §五。）

### Q4 破洞 decal 悬空/漏投

引擎侧破洞（decalInteriorMap 族）属**浮空分支**：`decalVS`/顶点流自带
UV 的**自持几何**，摆在变换处即完成，从不锚定、从不求交。游戏里
"洞在建筑上"的观感来自：**建筑模型本身带洞**（烧毁变体的建模）+
洞位摆放的**内景体积盒**透过洞可见、实墙处被深度遮挡。
我们的两个病根都来自把破洞当投影族处理：
1. 射线锚定在轴 Z 朝上的洞型必然脱靶 → 回退浮空 quad（`translateZ`
   让开 depth）→ **视觉悬空**；
2. 切片盒未触及任何面 → `projectDecal` 返回 null → 同样回退 → **漏投**。
**已修**：破洞族改为 `buildHoleVolumeMesh`——以变换原点为中心的
`BoxGeometry` 体积 + 内景材质**只渲染内壁（BackSide）**，不切片、
永不回退。建筑模型有洞 → 透过洞看房间（假内景）；无洞 → 被墙遮挡
（与引擎同构，不再产生浮空幻影）。

## 三、破洞族完整管线（引擎原文逐字，替代既有拼块版本）

```hlsl
// decalLightInteriorMap（PS，容器完整源码）
const float kSunContributionAmount = decalMaterialData[0].x;   // ← 0x0DA76A05（C 侧实锤）
const float kLightAmount           = decalMaterialData[0].y;   // ← 0x0DA76A06（C 侧实锤）
float3 textureFloatPosition = In.texcoord<t0>.xyz;             // 盒体归一坐标
// 假内景透视：前面全尺寸、后面半尺寸缩向中心
float2 interiorUv = lerp(tp.xy, tp.xy * 0.5, tp.z * 0.5 + 0.5);
interiorUv = interiorUv * -0.5 + 0.5;                          // U/V 取负
interiorUv = interiorUv * texXform.xy + texXform.zw;           // atlas 格窗口
float sunMod = saturate(dot(sunSky.mSunDir.xyz, bumpNormal));
float3 sunColor = sunMod * sunSky.mSunColor.rgb * shadow;
shColorDiff -= sunColor;  shColorDiff *= kLightAmount;
shColorSpec *= kLightAmount;
shColorDiff += sunColor * kSunContributionAmount;
float4 interiorTexture = tex2D(Sampler<s0>, interiorUv);
// kInteriorMapSelfLightMax = 16.0（自亮峰值，alpha 通道=窗灯掩码）
float3 interiorTextureLit = interiorTexture.rgb *
    (shColorDiff + shColorSpec + spec + interiorTexture.a * 16.0);
// 墙面混合：decalTexture.a 映射 [-1,1] 后 saturate 做插值因子
Current.color.rgb = lerp(Current.color.rgb, interiorTextureLit,
                         saturate(decalTexture.a * 2 - 1));
```

- **边框/填充**：无 shader 参与——边框、焦痕、房间图全部烤在贴花
  贴图自身（RGB=内容，A=轮廓+窗灯双用：`a×2-1` 做混合因子、`a×16`
  做自亮）。我们的 `createHoleInteriorMaterial` 已按此逐字实现 ✓。
- **假内景**：同一张贴图的透视重采样（上面 interiorUv 公式），
  **不是**建筑 slot5 房间链（§52.5 勘误过的方向保持）。
- **decalWorldDirection**（无光变体 decalInteriorMap 的假灯球）：
  `materialLightScale = info.x×16+1`、`invMaterialLightRadius = info.y×4`、
  `circleDist = saturate(1 - decalTexture.a×256/200)`、
  `circleZ = (tp.z×0.5+0.5) × |dir| × invRadius`、
  `lightScale = saturate(1 - sqrt(circleDist²+circleZ²))`，
  `lightAmount = saturate(dot(dir, worldNormal)) × materialLightScale`。
  注意 worldNormal 来自 `GetDeferredNormal`（屏幕空间延迟读取）——
  无光变体实际也依赖延迟 G-buffer。

## 四、既往认知勘误表

| 既往结论 | 勘误 | 依据 |
|---|---|---|
| `decalProject PS: clip(1-abs(texcoord))` 单位立方体裁剪（§65.10） | 该句属 `regionDecalProject`（区域 decal）与 `decalSDF`（调试）；招牌族 PS = `decalClip` 只有 `clip(-z)` | 完整源码块边界 |
| ≤10m 投影路由 / 60m 射线 / 厚度 [0.5,2] 钳制 | 全部为发明物，引擎无任何距离量；已删除 | FUN_006fdce0 + DAT_00cf1e4c=0.0 |
| 破洞"锚定失败→浮空回退" | 破洞本就是浮空分支的自持几何，不锚定；回退 quad 才是悬空幻影的来源 | decalFloatQuad/decalVS 顶点流 UV |
| decal 纹理需要"超采样公式" | 引擎无超采样：`tex2D` 双线性 + mip（变体对象 LINEAR×3 实证）。我们的 2× 双线性超采样 = CPU 复刻"对掩码双线性采样后展开调色色"，口径保留但定性为**解码器补偿**而非引擎行为 | decalClip 直采 + 采样器状态 |
| "颜色对齐"由解码负责 | 引擎经 `texXform`（atlas 格窗口，来自实例数据）在**完整 atlas** 上采样；我们按条目独立 raster 解码 = 预裁剪 cell，texXform 退化为恒等。颜色错位若再现，先查 raster 是否实为多 cell atlas（字典 textureSize vs atlasSize），而非解码公式 | decalClip/SetupSHParams 的 texXform 链 |
| 招牌 6 变体/涂鸦 5 变体 = 同族 LOD 变体 | 维持不变，与本次结论无冲突 | — |

## 五、遗留运行时验证项（诚实清单）

1. **体积盒 ×5 因子**：C 侧基 = 旋转 × scale × 0.5 × 10；与我们游戏对拍
   校准的"半高 = scale"（即 ×2）差 2.5 倍。可能：unit 字段 +0x34 的 scale
   语义 ≠ 属性里的 scale；或盒体大、`texXform` 窗口只取其中一部分
   （可见尺寸 = 盒 × atlas cell 占比）。**待 frida 抓 SetTransform/
   SetPixelShaderConstantF 对拍**，不影响现行渲染（我们不走体积盒路径）。
2. **前向（mode 1）vs 延迟（mode 2）**对招牌族的实际分配：静态不可判，
   两模式观感等价（贴图盒 vs 最近深度投影），我们取延迟语义（法线过滤）。
3. ~~量化掩码的四色展开~~ **已定谳证伪（见 §六）**：raster 是真彩图像，
   从不存在四色展开。
4. `0xCAAD8C9`（materialInstance）→ `FUN_006f8bd0` 上传的 decalMaterialData
   行数与字典 Color1-4 的对应（4 行假设与 shader 用法吻合，未逐字节验证）。
5. **混合状态（blend state）**：部分招牌/涂鸦 raster 的 A 通道恒 0
   （标准 alpha 混合下不可见）→ 这些族的真实混合状态必须是加法/忽略
   alpha 的一种（shader-def render state，运行时编译，静态不可读）；
   另一些 raster 的 A 是真实的柔和边缘（高清招牌条目2 A σ=52）。
   **待 frida 抓 SetRenderState 对拍**；影响"恒 0 alpha 纹理在 raw 直采
   后是否可见"。

## 六、Raster 真相：真彩图像，无四色展开（2026-10-01 第二轮取证）

**背景**：四问追查（边缘锯齿/暗部细节/色彩异常）指向同一源头——
"decal raster = 四通道掩码、需 Color1-4 展开"这一从原 SCP 编辑器预览
继承的假设。统计探针 `decal_raster_stats.rs`（通道均值/σ/相关性/直方图）
对三个族取证，**假设死亡**：

| 族 | 字典 | 证据 | 结论 |
|---|---|---|---|
| 招牌 0x73684EFC | b185/aa8b7058（高清 149 条目） | 条目0 R=226/G=108/B=0（**橙色图**）；条目1 R-G corr=0.82（暖色图 σ83）；条目2 R≡255、G=164、B=57、**A σ=52（柔和 alpha）** | 真彩 RGB(A) 图像 |
| 招牌（低清 1651/254 条目） | 同上低清版 | 两通道活跃=单色系图像（黄/蓝），B-A corr 0.99=图像+柔边 | 真彩（单色系） |
| 涂鸦 0xE5390A98 | 1651/eefd390c（385 条目） | 条目1 纯 R+A（**红色涂鸦**）；条目2 B=120/R,G 低（**青蓝涂鸦**，A=0）；条目3 B+A corr −0.64 | 真彩 RGB(A) 图像 |
| 破洞 0x4491DE3A | b185/1813da18 | （RW4 DXT 纹理另行处理）A=光衰减掩码 | 维持原判 |

**关键链条**：
- pixFmt 21 = **D3DFMT_A8R8G8B8 未压缩**（rw4/raster.rs 头注 + D3D9
  语义）→ 引擎**原样上传**，shader `tex2D` 直采即所得——**从数据到屏幕
  不存在任何调色板展开步骤**。字典 Color1-4 ≠ 图像色，它们是
  `decalMaterialData[0..3]` = **霓虹动画/灯光参数**（decalAnimateSDF:
  `lightColor[i] = materialLightScale · dot(decalMaterialData[i], lightScales)`
  ——4 行灯光色，断电半亮等）。
- "青底白字 Rolx"的四色阈值解码当年"看起来对"纯属巧合：涂鸦图近单色
  （纯红/纯青），主通道的调色板色 ≈ 真实色。对多色系招牌（琥珀渐变）
  即产生色彩异常；≥128 阈值 = 锯齿根因；σ>100 的暗部细节被阈值抹杀 =
  暗部细节问题根因。
- "raw 直采几乎不可见"的历史观察 = A 恒 0 的条目在标准 alpha 混合下的
  必然结果——**不是数据错，是我们的混合模型错**（见 §五.5）。

### 边缘平滑的引擎机制（定谳）

1. **纹理自身**：美术授权的柔和 alpha（高清招牌条目2 实测 A 梯度）+
   未压缩直传；
2. **采样**：LINEAR×3 + mip（变体对象实证）——双线性保边缘梯度；
3. **无二值化步骤**：引擎管线没有任何 ≥128 阈值/量化。我们的锯齿
   100% 来自阈值解码这一"编辑器预览"路径；
4. 地表 lot overlay 另有 fwidth 自适应 smoothstep 抗锯齿
   （addOverlay："width of lines (for anti-aliasing)" 原文注释）——
   属地形路径，decal 无此需求（纹理已柔）。
5. MSAA：exe 无 MultiSample 字符串（脱壳剥离，不可定谳），独立于
   以上机制。

### 破洞盒方向（Q4 定谳）

`decalProject` VS 的 `texcoord = mul(modelToTexture, float4(modelPos,1))` +
VB 角点 ∈ {0,1}³ + FUN_006fdce0 平移 = 原点 ⇒ **引擎体积盒从原点沿
+basis 延伸（0..1 角点），不居中**。案例 lot 0x9769DE99 破洞实测：
原点在建筑内 6.8m、axisZ 朝建筑内部（-Y）+Z 射线 9/9 命中（墙在 +Z）
——盒延伸方向 = 墙内 ✓。我们 `buildHoleVolumeMesh` 居中摆放 = 一半
穿出立面 = 用户观察到的"破洞盒子在建筑外"根因。
（修复方向：盒中心 = 原点 + Σ(halfᵢ×axisᵢ)；待批准后动代码。）

## 七、本次落地（代码）

- `src/lib/decalProject.ts`：删除 `measureAnchorDistance` 与全部距离常量；
  锚定恒为变换原点；厚度 = depth 原值（0.05 下限）；新增 `keepFrontFaces`
  逐三角形法线过滤（背面剔除，防正反面印广告）。
- `src/pages/packages/components/property-editor/PropertyEditorViewport.vue`：
  删除 ≤10m 路由；破洞族改走 `buildHoleVolumeMesh`（变换原点体积盒 +
  BackSide 内景材质 + 保留 cookie 光），永不回退浮空 quad；
  `createHoleInteriorMaterial` 增加 `side` 参数。
- 测试：`src/lib/decalProject.test.ts` 按新语义重写（含背面过滤用例）。
  tsc ✓ vitest 209/209 ✓。

## 八、家族分类的修正表述（2026-10-01 第三轮，用户观察驱动）

**修正**：§一"引擎不分广告牌/涂鸦/文字"的表述过度简化。准确表述：

- 引擎**存在数据侧分类**——按 materialInstance → shader-def 决定
  **shader 家族**：decalProject 系（10 变体，含 Lit/Neon/SDF）/
  decalFloatQuad / decalNeonTubeSDF / decalInteriorMap / regionDecal 系 /
  decalClipBack（屏幕空间延迟）。家族决定**机制**（体积投影 vs 自持
  浮空 quad vs 内景盒），全部烤在数据里。
- 引擎**不存在的是运行时距离分类**——没有任何"距离<X 判吸附"的代码；
  同族内贴墙/浮空完全由变换数据的原点位置决定。

**对游戏观测的解释**（用户观察：City Hall 类文字招牌紧贴建筑、科技风
全息招牌漂浮）：
- 文字招牌 = 投影家族（decalProject 系）→ 延迟投影把广告落在**可见
  立面表面本身**——间隙恒等于 0，是构造性质，不是"小间隙被模糊掩盖"。
- 科技全息 = 浮空家族（decalFloatQuad 系）→ 自持 quad 摆在数据位置，
  间隙真实存在且是设计意图。
- 游戏的强制景深模糊 + 相机限制的**真正影响**是方法论层面的：它使
  截图 A/B 对拍不可靠——此前多轮修复被"我们的渲染 vs 模糊游戏截图"
  的对比误导。定位问题的对拍应以机制对齐为准，截图仅作观感参考。

**未决**：materialInstance → 家族的映射表需要一次 frida 运行时捕获
（resolve_shader_def_instance hook，地址已知，§66 原计划）——一次会话
即可定谳全部字典的家族路由。静态路径为已证死路（eco 编译，见
deadend dead-mtt129cw）。
**工程意义**："自持网格 + 深度测试"的目标架构对家族不可知也成立——
近墙 quad 深度测试下观感贴墙、远距 quad 自然漂浮，两类观测同时正确。

## 九、来源


- `docs/source-code/SC_cVolumeDecalManager.c`（Ghidra，FUN_006fe190 /
  006fdce0 / 006fd730 / 006fa160）；`SC_cDecalManager.c`、
  `SC_cGraphicsUnitDecals.c`（辅助，无投影逻辑）
- `tmp/dynamic/decal_full_0.txt`（无损重解析产物）；
  `tmp/parse_container_tokens_full.py`（新解析器）
- exe：`D:\ea-games\simcity_offline\SimCity_dump.exe`（常量直读脚本见
  会话记录；VA→off = raw + (VA - base - section VA)）
- 既有资产：`tmp/dynamic/all_blocks_index.txt`、
  `docs/re/decal-family-routing.md`、`docs/re/hole-interior-pipeline.md`
  （后两篇的冲突处以本文为准）
