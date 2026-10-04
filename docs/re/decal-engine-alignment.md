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

> **2026-10-04 RL 带名代码定谳 + 修复落地**：RL
> `SC::cDecalData::SpawnDecalInstance` 的 model→texture 矩阵证明
> **只有 z 轴需要从原点起算**——xy 居中（±1，x 含 1/aspect），
> z ∈ [0, depth] → 纹理 z [−1,1]（z 轴 ×2/depth、平移 −depth/2）。
> 上文"Σ(halfᵢ×axisᵢ) 三轴同移"据此修正为**仅 z 轴移动 depth/2**；
> depth 语义 = **全长**（修正 §41.2 半厚旧判）。已实施：
> `buildHoleVolumeMesh` 盒 z 尺寸 = depth、盒心 = 原点+axisZ×depth/2；
> 投影族吸附未命中**删除原点回退 quad**（引擎无回退：延迟模式盒内
> 无深度即不渲染）， decalStats 新增 skipped 计数。vue-tsc ✓
> vitest 208/208 ✓。同批勘误：RL 证实 decal 存在代码级次分类
> （cDecalManager 三 decal set normal/vacant/extractor +
> renderGroup/machineSpec 行属性），详见会话提取产物
> `tmp/dev-sample-analysis/out/`。

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

## 十、2026-10-04 二轮勘误（PE 渲染侧三处与引擎/色彩管线脱节）

1. **破洞"完全黑色"= 色彩空间断裂，非纹理缺失**。破洞 raster 是无
   Color1-4 的 sRGB 颜色纹理（GPU 采样自动转线性），而 ShaderMaterial
   不做输出色彩空间编码——线性值直出 sRGB 画布，整体变暗 ~2.2 gamma，
   焦痕灰（≈0.15）压到 ≈0.02 即"纯黑"。修复：sc-shader 组合器为 hole
   家族 PS 加 sRGB 收尾（`pow(rgb, 1/2.2)`）；sign/holo 纹理走
   NoColorSpace 直采直出，无需补偿。（用户报告"破洞纹理与假内景未
   实装"中"纹理"一项的根因。）
2. **"假内景未实装"= 体积盒错用 quad VS**。组合器给全部家族共用 quad
   VS（`vTexcoord0.z` 恒 0），体积盒拿不到真实进深 →
   `decalLightInteriorMap` 的视差收缩（lerp 因子 = z×0.5+0.5）退化为
   常量 0.5，内景成平面。修复：hole 家族改用体积盒 VS（盒局部坐标经
   uBoxHalf 归一化，z = 前 −1 → 后 +1 真实进深）。
3. **全 decal 夜间"自发光"= 平涂 shader 无光照响应**。sign 链仅
   `decalQuantComposite`（层色 × uNightBoost，恒 1），昼夜同亮；引擎
   decal 走延迟光照，夜间只剩环境项。修复：uNightBoost 挂 env 昼夜
   共享 uniform（白天 1 / 夜间 0.15 ≈ uAmbientDiff 水平），热切换免
   重建。霓虹/跑马灯族的夜间自亮属 SDF 动画链（decalAnimateSDF +
   decalLightNeonTube），仍留待后续任务。
4. **破洞盒前缘内壁恰好是"焦痕环画在墙上"的正解**：BackSide 盒的前
   缘面（z=0，与墙共面，polygonOffset 赢 z-fight）承担墙表焦痕的显示；
   盒后缘内壁透过真实模型洞提供进深内景。平坦化前缘在**曲面立面**
   四角外露（2026-10-04 用户图2），以及**阶梯立面**上 quad/前缘被
   凸出结构切掉（用户图3~6，原游戏同病）——两者的共同正解是投影化
   （DecalGeometry 盒裁剪贴面 / 引擎延迟投影语义），列为下一轮任务，
   需金样本 A/B 对拍后定稿。

验证基线：vue-tsc 干净、vitest 208/208、cargo test -p sc-shader 12/12。

## 十一、2026-10-04 二轮勘误之二（用户目视对拍后的根因修正）

1. **§十.1 的 gamma 解释要收窄**：hole PS 缺 sRGB 收尾是真实 bug（影响
   破洞内景亮度链），但用户图中**墙面纯黑斑块的根因不是它**。像素级
   分析（tmp/hole_*_raw.png：焦痕区 alpha≈1 处 RGB 均值 0.19~0.36，
   纹理中不存在"高 alpha + 黑 RGB"像素组合）证明：破洞族纹理经现有
   shader 数学不可能输出不透明纯黑。纯黑斑块实为**焦痕/烧灼 decal
   （非 hole 变体、带 Color1-4）被一律走 sign 量化合成链**——`m.a≥0.5
   → col=layerColors[3]`，层色近黑 → 整块纯黑。引擎按材质把这类
   decal 路由到 decalProject **直采链**（raster RGB 即美术内容）。
2. **修复 = 家族路由落地**：sc-shader 新增 clip 族（decalClipQuad =
   vUv 直采 + uNightBoost 昼夜因子，PE quad 适配引擎 decalClip 链）；
   视口按 materialInstance 字典路由——`0x73684EFC` → sign（量化），
   其余（含涂鸦 `0xE5390A98`、焦痕、未知兜底）→ clip（直采）。
   附带收益：quantized 预览纹理（四色量化已合成 png）直采即正确，
   不再被量化链二次上色。
3. **单射线吸附 quad 废弃，DecalGeometry 盒裁剪投影上线**：弧面四角
   外露、阶梯立面被凸出结构截断（用户图3~6，原游戏同病）的共同正解。
   投影盒 = 引擎体积盒（原点为中心、Z 全深 5×scale）；深层被投面由
   深度测试自然隐藏（= 引擎延迟模式最近深度语义）；盒内无建筑面 =
   不渲染（无回退口径不变）。UV 翻转口径与 quad 路径一致（几何 uv
   1-x，文字正读已核对）。包围球粗筛控制逐三角形裁剪成本。
4. **本轮未经目视对拍的残留风险**：DecalGeometry 在锐角折边处的拉伸
   （three.js 已知 issue #21187）可能在高曲率立面出现条纹；clip 族
   直采未做 N·L 方向光响应（仅昼夜因子），侧光面可能偏平；广告牌的
   远抛实例若因盒内无面被跳过，需金样本确认与游戏一致。

验证基线：vue-tsc 干净、vitest 208/208、cargo test -p sc-shader 13/13。

## 十二、2026-10-04 三轮勘误（DecalGeometry 投影化后的三个回归/残留）

1. **涂鸦回归量化链**：§十一.2 把涂鸦一并路由 clip 直采是错的——涂鸦
   raster 的四通道是**层权重掩码**，0.5 阈值多通道合成才是"清晰图案"
   的来源（用户对拍：直采 = 权重通道当颜色 → 彩色模糊涂抹）。焦痕/
   烧灼 raster 的 RGB 才是美术内容（直采正确，屋顶焦痕对拍通过）。
   定谳路由：量化族字典 = {0x73684EFC 招牌, 0xE5390A98 涂鸦} → sign
   链；其余（焦痕/未知）→ clip 链。
2. **背墙投影 = 盒子双侧居中的错**：引擎 decalClip 的
   `clip(-texcoord.z)` 是**半空间裁剪**——贴花只落在原点单侧半盒内
   的面上。居中盒（±2.5×scale）把 CASINO 招牌同时投到建筑背面玻璃
   幕墙（镜像字）。修复：±axisZ 射线取最近命中面所在侧（复用吸附
   证据），投影盒改为原点沿命中侧延伸 2.5×scale 的**单侧盒**。
3. **立面线性拉丝 = 盒边界切向三角形的 UV 拉伸**（DecalGeometry 的
   已知缺陷，three #21187）：垂直于投影轴的面（侧墙/屋面/地面，
   N·轴≈0）被盒裁剪后极度拉伸。修复：投影几何按面法线过滤，
   |N·axisZ| < 0.5 的三角形整面丢弃（60° 以内曲面绕折保留）。引擎
   延迟投影逐像素取深度、无三角形拉伸，故游戏无此瑕疵。
4. **焦痕"模糊"的来源分解**：(a) 主要是拉丝涂抹的观感，法线过滤后
   收敛；(b) 量化族的锐利来自阈值链（非采样器），直采族的焦痕边缘
   = 美术授权的软衰减，引擎同为 LINEAR+mip（采样器口径已对齐，含
   最大各向异性）；(c) 若对拍后仍觉糊，嫌疑是损害系 decal 的盒窗
   scale 语义（与招牌 2×scale 可能不同族不同值）——需金样本量尺寸。

验证基线：vue-tsc 干净、vitest 208/208。

## 十三、2026-10-05 四轮勘误（盒深 = depth×scale 实锤 + 浮空分支上线）

1. **盒深语义定谳：世界盒深 = unit.depth × scale**（casino 探针，
   `cargo run -p sc-exporter --release --example decal_unit_dump --
   SimCity_Game.package 0x457EA9DB` + `decal_projection_probe` 复测）：
   lot 0x457EA9DB 全部 7 个 decal，两招牌 scale 13.27/5.63 而
   **depth×scale 恒 = 1.327m**（0.1×13.267 = 0.2355×5.635），五涂鸦
   depth×scale 恒 ≈ 17.95m（3.453×5.199 ≈ 2.137×8.392 ≈ … ≈ 1.599×
   11.232）——不同 scale 下乘积严格守恒 ⇒ depth 是按 scale 归一化的
   盒进深。盒 = xy 居中于变换原点、**z∈[0, depth×scale] 沿 +axisZ**
   （63/63 足迹射线全命中 +局部Z，墙恒在 +axisZ 单侧），与 RL
   `SC::cDecalData::SpawnDecalInstance` 的 model→texture z∈[0,depth]
   映射互证。招牌盒仅 1.33m 深 ⇒ 背墙/对侧玻璃**天然在盒外**，四轮
   图1 的背墙回归两个根因（~2.5×scale≈33m 深盒吞背墙；三轮"最近
   命中侧"把原点偏玻璃侧的招牌定错侧）一并删除——**不再选侧，固定
   +axisZ**。涂鸦盒 18m 深是设计值：曲面/退台立面的绕折全靠深盒
   罩住（§十一"大盒吸附"判断由此获得数据支撑，但深度来自 depth
   字段而非固定系数）。
2. **浮空分支（decalFloatQuad）实装**：+axisZ 射线在数据盒深内无
   命中 → 在**数据位姿**画浮空 quad（组局部恒等姿态，材质
   DoubleSide）——这是 holo 全息广告/远抛实例的引擎语义（四轮
   图3/4：holo 被错误投影贴墙、绕管道弯折 = 此前"万物皆投影、
   未命中即跳过"缺失浮空分支的回归根因）。法线过滤后无剩余几何
   的极端情形同走浮空分支（宁可画在数据位姿也不凭空消失）。
   已知残留：贴墙近处（盒深内）的 holo 与墙招牌静态不可区分
   （material 只到字典级，同字典混编），仍会被投影——次优可接受，
   彻底分离需 frida 运行时捕获 shader-def 选择。
3. **破洞族路径不变**：buildHoleVolumeMesh 维持 depth 原值全长
   （RL SpawnDecalInstance 实锤，破洞 unit 的 scale 语义待普查——
   若破洞 scale≠1 则其盒深同样应乘 scale，列入待办测量）。
4. **material_data 初读**：招牌 = [0.4, 0.0, 0.1]、涂鸦 = [0.5, 0, 0]
   （0x0D109080+cat，Vector3）——疑似 decalMaterialData 光照三元组
   （日光/灯光系数？），族间差异稳定，可供后续昼夜/发光对拍。

验证基线：vue-tsc 干净、vitest 208/208。

## 十四、2026-10-05 五轮勘误（穿透镜像字根除 + FloatQuad 族判据定谳）

1. **穿透投影 = 远侧面未剔除**（五轮图1~2，DIRTY FACTORY 镜像字）：盒深
   = depth×scale 实锤后，深盒（该招牌 2.088×4.40 ≈ 9.2m）会穿过薄板
   结构把招牌同时投到背坡——法线过滤从 |N·axisZ|≥0.5（双侧保留）收紧
   为 **N·axisZ ≤ -0.5（只留面朝贴花原点的面）**，投影材质从
   DoubleSide 改 **FrontSide**（薄单面墙背后看 decal 三角形是背面，
   剔除即无"隔楼见镜像字"）。引擎延迟投影逐像素取最近深度天然只画
   最近面，CPU 几何投影以此同构。
2. **FloatQuad 族判据定谳：sign 字典且 materialData[1] ≥ 0.9 → 浮空**。
   取证链：579 张招牌条目缩略图墙目视定位 → `decal_find_lots` 反查 →
   高塔 lot 0x9401CB7A（竖幅 STORE 0x23D05B09，原点离墙 3.76m、盒深
   4.27m 恰吻墙面，53m 高竖幅）用户确认游戏内浮空。四组对拍一致：
   casino 墙招牌 md[1]=0 → 投影；高塔竖幅 md[1]=0.95/1.0 → 浮空；
   DIRTY FACTORY md[1]=0.06 → 投影；涂鸦 md[1] 恒 0 → 投影。
   **几何判据全部证伪**：离墙距离（涂鸦 10m 仍投影）、盒深余量
   （0.5m~16m 两族重叠）、命中面朝向（高塔墙面与招牌平行）。
   md[1] 疑似引擎自发光/灯箱变体参数（与"霓虹/跑马灯"待办同源），
   FloatQuad 族选择与之绑定。安全网：贴墙 sign 浮空 quad 与投影观感
   近乎一致，误判代价低； graffiti/焦痕不走此判据。
3. **残留诚实备注**：0xBBF5B017（老虎纹商业楼）21 个 sign 的 md[1] 在
   0.2~0.85 之间，按判据全部投影——若游戏内其中部分实为浮空刀旗，
   阈值需下调（0.9 是保守取值）；img4 的淡绿色半透明 quad 疑为浮空
   路径缺少引擎 ×2 增亮/emissive 链（decalFloatQuadNoClip，待办）。
4. **工具沉淀**：`decal_unit_dump`（lot decal 静态字段）、
   `decal_find_lots`（条目→lot 反查，带父链继承）、`decal_float_scan`
   （400 lot 命中/material_data 聚合）、`decal_sign_thumbs`（招牌条目
   贴图导出）均在 crates/sc-exporter/examples/。

验证基线：vue-tsc 干净、vitest 208/208。

## 十五、2026-10-05 六轮（招牌自发光 + 霓虹跑马灯实装，md 三元组语义定谳）

1. **lot 侧 material_data 三元组 = 引擎 decalMaterialInfo.xyz**（SDF 霓虹
   链源码定谳，tmp/dynamic/decal_full_0.txt line 3736-4117；勘误 §十四.2
   "疑似自发光参数"的猜测）：
   - `x` → `materialLightScale = x*16 + 0.25`（灯强，SDF 系口径）；
   - `y` = **animSpeed 跑马灯速度**（`animTime = frac(gameInfo.time ×
     animSpeed + 0.9999)`，decalLightBackground line 4103）；
   - `z` → `materialTubeLightFactor = z*8 + 1`（灯管亮度）；
   - `w` = 供电（断电半亮：`powerFactor = lerp(0.5, lightFactor, w)`，
     decalAnimateSDFDisabled line 3736）。
   对拍自洽：casino [0.4,0,0.1] 静态 ✓、高塔 STORE [0.8,1.0,0.1] 动画 ✓、
   涂鸦 [x,0,0] 静态 ✓。§十四的浮空判据（md[1]≥0.9）经验规则不变，
   语义应读作"高速动画招牌 = 引擎浮空灯箱族"。
2. **字典条目 colors 四行对动画招牌不是调色板而是参数表**
   （decalMaterialData[0..3]）：行 0~2 = 三根灯管颜色；行 3 = 动画参数
   （分量符号选 UV 轴：>0 用 uv.x 否则 uv.y；abs 后整数 = 分块数
   animChunks、小数 = 相位 animOffsets）。注意 DTO colors 统一做过
   线性×2（量化链口径），喂 SDF 链须 /2 还原——动画参数行被 ×2 会
   直接破坏 chunks/offsets 编码。
3. **引擎霓虹链五段实装**（sc-shader 管线，sdf.frag.glsl）：
   `decalLightBackground`（uTime 驱动跑马灯比较量 animResults/useV，
   末行原文截断按 Darken 消费语义修复为 uvCompare − compares）→
   `decalAnimateSDFDisabled`（灯管调光：未扫到 0.1 / 扫到全亮；原文
   向量条件三目改 step+mix 等价）→ `decalAnimateSDFDarken`（SDF 球面
   衰减合成灯色；本次原文再核对修三处：lightScales 实为 float4、
   `animEdge²×animation×32` 的 animation 系早前误读为 animRatio 占位、
   decalNUS 顶点流改 uniform uDecalNUS）→ `decalLightSDF`（场景光叠加）
   → `decalLightNeonTube`（供电开关）。收尾 alpha = 亮部 max 分量
   （SDF 纹理 alpha 实为第四路距离场，非覆盖率），**不吃 nightBoost**
   ——霓虹夜间保持自亮是该族语义（与六轮前"所有 decal 夜间自发光"
   的 bug 是对偶：只有霓虹族该亮）。
4. **PE 路由升级**：sign 字典且 md[1] > 0 → sdf 族；md[1] = 0 → 维持
   sign 量化链。浮空判据同步覆盖 sign/sdf 两族（否则 md[1]≥0.9 的高塔
   竖幅在分流后会被错误投影——路由升级的直接回归点，已在同轮堵上）。
   uTime 经 env 共享对象（SunEnvRefs.time）注入全部 SDF 材质，装配层
   仅在场景含动画 decal 时启动 rAF 推进 + invalidate（按需渲染底座
   零常驻开销）。uDecalNUS 暂取 (sizeX, sizeY, sizeY)，sphereHeight
   分量待高塔 STORE（53m 竖幅）目视校准。
5. **对拍锚点**：高塔 0x9401CB7A 竖幅 STORE 应出跑马灯动画且浮空；
   casino 墙招牌应**不动**（md[1]=0）；夜间场景霓虹族保持自亮、其余
   decal 维持夜间压暗。风险：DIRTY FACTORY md[1]=0.06 会进 SDF 链
   （极慢动画），若其 raster 是权重掩码而非 SDF 距离场观感可能退化
   ——备选判据是把 SDF 路由阈值从 >0 提到 ≥0.3。

验证基线：vue-tsc 干净、vitest 208/208、cargo test -p sc-shader 14/14
（新增 sdf_chain_full_neon_pipeline 组合断言）。

## 十六、2026-10-05 六轮补丁（霓虹色块根因 = colors 矩阵未转置）

1. **症状**：霓虹招牌变成闪烁的纯色块、看不出字体图案；部分此前锐利的
   广告牌 decal 退化为半透明色块。
2. **根因**：`decalMaterialData` 在引擎 VS `decalMaterialData4`
   （容器 line 4139）中被**转置**——`Current.color = (row0.x, row1.x,
   row2.x, row3.x)` 等，即 PS 侧消费的是字典 colors 的**列**而非行：
   - 列 0~2 = 输出 R/G/B 对四个掩码通道的**权重列**（Darken 的
     `dot(data[i], lightScales)` = 输出通道 i 的四通道加权和）；
   - 列 3（w 列）= 四路**独立**动画参数。
   §十五.2"行 0~2 = 三根灯管颜色、行 3 = 动画参数表"的读法**作废**。
3. **实锤数据**（条目 0x23D05B09，PROBE_DEBUG 直出）：四行 colors 的
   w 分量 = −60.0 / −60.3 / +60.6 / −60.9 —— 四路各 60 分块、相位
   0/.3/.6/.9 错开的追逐灯，符号混合 = 三列沿 uv.y、一列沿 uv.x。
   未转置时 ±60 的 w 直接进 dot → 输出被 ±60×lsA 撑爆/清零 → 整牌
   饱和纯色块，随 animTime 闪烁。
4. **修复**：`createEngineDecalMaterial` 对 sdf 族按列重组
   uDecalMaterialData（data[i] = 四行的第 i 分量），shader 侧零改动
   ——decalLightBackground 读 data[3] 恰得 w 列动画参数，Darken 的
   dot(data[0..2], lightScales) 恰为输出 RGB 权重和。贴图通道约定同
   轮摸清：四通道 = 四级量化掩码（如 STORE：R 背景 0.2/字体 0.82，
   G 背景 0.8/字体 0.76，A = 覆盖 0/1），kMaskCenter=0.5 的球面衰减
   让 ≥0.75 的笔画全亮、~0.2 的底板压到 ~16%——"亮字 + 暗底"的
   灯箱观感由此而来，**不是真 SDF 距离场**。
5. **路由维持 md[1]>0 → sdf**：色块是矩阵未转置所致，非路由过宽；
   md[1] 0.2~0.85 的慢速动画招牌在转置修复后应恢复字形。若对拍仍
   有个别条目异常再考虑阈值。

验证基线：vue-tsc 干净、vitest 208/208、cargo test -p sc-shader 14/14。

## 十七、2026-10-05 六轮补丁之二（"运动色块"根因 = PE 硬钳毁色相，非公式/路由错误）

1. **排查路径**：对拍同一加油站（SELF SERVE GAS 广告牌 0x090C71D6）
   动画前后——转置修复后仍是运动色块。取证：该条目与 STORE 同字典
   （1651/aa8b7058，material 0x73684EFC）、colors 行 w = (−70,+70,+70,4)
   = 四路追逐参数、raw 贴图 64×32 为 4~5 级量化掩码——**数据侧完全
   符合 SDF 族特征，路由无误**。
2. **离线模拟定谳**：用真实 colors/md 逐式复算 SDF 链（Python），输出
   与 PE 截图同态（大片白/粉/青）——证明 shader 公式与 uniform 接线
   正确，**引擎公式在这些输入下的原始输出本来就是 25~55 倍 HDR**。
   文字/面板的全部区分度在色相比里（黄字 (13.3,9.3,5.4) vs 红面板
   (24.6,5.7,3.2)），引擎靠 hejl tonemap 软肩回收，PE 无 HDR 曝光
   管线、硬钳 [0,1] → 全部压成 (1,1,1) 白块。
3. **修复**（compose.rs Sdf 收尾）：**保色相 Reinhard**
   `rgb /= 1 + max(rgb)`——单调、不破坏色相比、保留亮暗扫描对比；
   alpha 改取覆盖率通道（前奏暂存 coverageA，A = 0/1 覆盖掩码），
   暗态图案不再被墙面底色冲淡。同式复算验证：SELF SERVE GAS 文字/
   油泵图标/追逐扫描带全部可辨，STORE 字形保持。
4. **诚实备注**：暗态（0.1 调光区）接近全黑是公式本色（lit/dim
   对比 ~25×）；扫动方向（uvOrig 正号口径）与原作是否一致仍待目视；
   sphereHeight = sizeY 近似未校准。若对拍发现暗态死黑过重，候选是
   给 Disabled 段 0.1 地板上调或 Reinhard 加曝光系数。

验证基线：vue-tsc 干净、vitest 208/208、cargo test -p sc-shader 14/14
（sdf_chain_full_neon_pipeline 断言同步更新为 Reinhard/覆盖率收尾）。

## 十八、2026-10-05 六轮补丁之三（"字体半透明+看不清" = 掩码未二值化）

1. **根因**：SDF 族贴图是 4~5 级**量化掩码**（非平滑距离场），64×32
   拉伸到数十米时双线性把 0/1 级别插成连续中间值 → 球面衰减 ls 全场
   半亮 → 文字糊成水洗渐变、轮廓 alpha 随插值半透明。量化链（sign 族）
   在同样采样条件下锐利，靠的是 uLayerColors 上色前的 **0.5 硬阈值**
   二值化——SDF 链缺的就是这个等价物（用户："没有保持原来的采样
   操作"）。
2. **修复**（compose.rs Sdf 前奏）：采样后立即按 0.5 轮廓做屏幕导数
   抗锯齿的 smoothstep 二值化（`fwidth` 给过渡带，近 1:1 时 ≈ 硬阈值、
   无锯齿）——级别设计意图（≥0.6 = on / ≤0.3 = off）被恢复，字形
   边缘锐利；远景 mip 平均出的中间值同样被推回 0/1。coverageA 取
   二值化后的 A 通道，轮廓半透明同愈。
3. **同式复算验证**：加油站"SELF SERVE GAS"恢复硬边黄/青字 + 红面板
   + 油泵图标，追逐扫描带边界清晰；STORE 竖幅字形保持。彩虹渐变
   （此前误认为模糊的一部分）实为追逐扫描过渡带，属公式本色。
4. **诚实备注**：二值化把中间级别（如面板 R 0.29）压到 0，丢弃三级
   层次——这是按"掩码两级设计"的判读；若对拍发现某些招牌丢了灰度
   层次，候选是把单阈值改为按级别聚类的多阈值量化。

验证基线：vue-tsc 干净、vitest 208/208、cargo test -p sc-shader 14/14
（sdf_chain 断言新增 smoothstep 二值化复发点检查）。

## 十九、2026-10-05 七轮补丁（"只剩字体" = 二值化丢中间级别；静态招牌自发光）

1. **根因一（只剩字体/字体不全）**：§十八的 0.5 单阈值二值化按"掩码
   两级设计"判读，但引擎对拍（加油站 0x090C71D6 原图：青面板 + 黄
   描边字 + 紫色油泵）证明中间级别承载实体内容——面板 G 0.6/B 0.65
   （青底）、油泵 R 0.29（二级暗区）。二值化把小于 0.5 的级别全压
   成 0 → 背景面板与油泵全灭，只剩字体；细笔画落在 0.4~0.5 区间的
   招牌字体也残缺。
2. **修复一（sharp-bilinear 保级锐化，compose.rs Sdf 前奏）**：放弃
   值域阈值，改在 **UV 域**锐化——把双线性过渡带压缩到约 1 屏幕
   像素（斜率 = fwidth(uv)×texSize = 每纹素屏幕像素数，封 [1,32]），
   平台级别原样保留（0.29 仍是 0.29），与级别取值无关。新 uniform
   uSdfTexSize（decalEngineMaterials 从 map.image 取像素尺寸，缺省
   64×64）。缩小时钳回 1 退化为普通双线性。
3. **同式复算验证**（tmp/sharp_bilinear_check.py，8 倍放大）：双线性
   482 个唯一值（糊）；二值化 2 个值、面板级 0.29 死亡；sharp-
   bilinear **恰 4 个唯一值 = 原图 4 级别**，0.29/1.0 俱存活，过渡带
   实测 0 像素。拼图目视：青色面板 + 紫油泵 + 黄字三者俱在，与引擎
   原图一致。
4. **根因二（静态招牌平涂无灯箱感）**：sign 量化链此前只有平涂上色
   ×uNightBoost，无发光项。引擎 decalMaterialInfo.x →
   materialLightScale = x×16+0.25（casino md 0.4 → 6.65）对静态招牌
   同样适用（灯箱自发光件）。
5. **修复二（compose.rs Sign 收尾）**：signGain = clamp(x×16+0.25,
   1, 4)——无 lot 数据（x=0，涂鸦同链条目）钳到 1 保持平涂不误亮；
   夜间 mix(1, nightBoost, 0.4) = 60% 豁免（灯箱夜间保持亮）；保色相
   软肩（峰值 ≤1 不动、超出等比压到 1）防 ×4 饱和成白块。夜压从
   decalQuantComposite 片段移到收尾（避免与豁免叠加成双重压暗）。
   decalEngineMaterials 缺省 materialInfo 按族分派：sign → x=0，
   sdf 保持 x=1。
6. **诚实备注**：signGain 上限 4 与夜间豁免 0.4 是初值，等用户对拍
   校准；casino 实测 6.65 被封到 4，若对拍偏暗可上调封顶。

验证基线：vue-tsc 干净、vitest 208/208、cargo test -p sc-shader 15/15
（新增 sign_tail_lightbox_glow；sdf_chain 断言改查 sharp-bilinear 并
禁 smoothstep 二值化回潮）。
