# Property Editor reliefPS 对拍与修复记录

日期：2026-10-24

## 结论

Property Editor 之前的 relief 实现不是游戏的 cone-step mapping，而是固定步长的近似：它把 `eye.xy / abs(eye.z)` 乘固定深度后每步减去，并且对 `shaderMap.a > 0.90` 的像素直接返回原 UV。结果是高度场没有按锥比推进，且大部分不透明门窗区域被提前跳过，因此表面浮雕几乎不可见。

本轮改为对齐 `reliefPS`/`reliefMap` 逻辑：

- `shaderMap.r` 是高度，`shaderMap.g` 是 cone ratio 输入。
- `kReliefDepth = 0.16`。
- `kMaxConeRatio = 0.5`。
- 8 次 cone search，2 次 binary refinement。
- 每一步使用 `step = saturate(height - sampleDepth)`。
- 步长为 `(coneRatio * step) / (dsRatio + coneRatio)`。
- 先应用 `tc -= ds.xy * flatDepth`，再把 `ds.z` 归一为 1。
- 高度不超过 `23/255` 时走平面路径。
- 视差只改变采样 UV，不修改顶点几何。

## 源码对拍

已对拍的来源：

- `docs/re/maya-building4-authoring-shader.fx_source.txt` 中 `ReliefLib`、`reliefMapLinear` 调用和 `std_PS`。
- `tmp/building_shaders_full.txt` 中完整恢复的 `reliefMap` cone-step 片段。
- `tmp/dynamic/all_blocks_full.txt` 中 `building4ClipAndReliefMapPS` 片段族。
- `docs/re/building-facade-engine-analysis.md` 中的通道、步进和距离剔除结论。
- `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree` 的新 source tree。

新 source tree 中没有 `reliefPS` 或 `reliefMap` 的 shader 源实现；它包含渲染类、材质和高度图相关 C++ 接口，但没有导出的 building4 shader 片段。因此 shader 逻辑仍以 SporeModderFX/资源 shader 解包结果和 Maya 作者 shader 对拍为准，不能把 new-source 中的高度图接口误认为 reliefPS 实现。

## 当前修复

修改文件：`src/pages/packages/components/property-editor/refinedRender.ts`。

1. 用游戏 cone-step + binary refinement 替换固定步长近似。
2. 删除 `shaderMap.a > 0.90` 的提前返回；relief 是否生效由高度平坦阈值和 Top 层覆盖率共同决定。
3. 修正 Three.js 视图坐标下的视线坐标链：调用 reliefMap 前先按游戏实现用 `abs(regionXform2.xy)` 缩放切线空间 XY，再保留源码的 `ds.xy = -ds.xy`。同时使用导出的游戏切线和 `cross(tangent, normal)` 得到同向 binormal，避免 Three 默认 `cross(normal, tangent)` 带来的 Y 轴反转。该方向把高度场采样推向墙体内部，门窗应表现为内凹。
4. 距离剔除与发行版 building4 变体保持一致，使用 `kCullDistanceSq = 160000`；
   近景仍执行 relief，远景直接使用原始 Top UV。

### 窗户周围墙面串扰修复

真实建筑对拍发现，窗户上下、左右以及转角墙面出现了 Top 层材质条带。
根因不是 relief 方向，而是两个顺序偏差：

- `reliefMap()` 的结果必须保留超出 `[0,1]` 的坐标，游戏随后构造
  `tcBoundary = float4(-tc, tc - 1)`，由 `outsideTile > 0` 把 Top alpha 置零并回退
  Base 层。此前 Property Editor 在 `scReliefUv()` 内部先 `clamp`，导致
  `outsideTile` 永远为零，越界的窗/门采样被钳到墙面边缘。
- 游戏调用 `reliefMap()` 时传入的是 Base 域 `uv_ddx/uv_ddy`。Property Editor
  此前传入 Top 域导数，在窗框边缘形成过大的高度图采样 footprint，扩大了邻近墙面
  的参与范围。现在恢复为 Base 域导数；Top 层最终采样仍由 `facadeTint.a` 控制。

当前实现保留 `ds.xy = -ds.xy` 以及切线空间变换，因此门窗的内凹方向不变；修复只
   影响越界回退和高度图 footprint，不会把已正确的内凹重新翻转。

### 两个实际建筑实例的取证与窗户材质修复

用游戏数据导出了用户给出的两个 Property 实例对应的真实 LOTM：

| Property | 解析到的模型 | mesh / uvKind | 参数列 | shader map |
|---|---|---:|---:|---|
| `00b1b104:42e1c000:4afd3ded` | `0x4AFD3DF1`（SimCity_Graphics） | `1 / 2`，facade=true | 46 | slot3，256×256 |
| `00b1b104:40a2a165:10daf7bf` | `0x29AAA67A`（SimCity_Game） | `1 / 2`，facade=true | 66 | slot3，256×256 |

两者都具备 facade FLOAT4、Top 参数和 shader map，因此“完全没有浮雕”不是因为
建筑缺少 facade UV。第二个实例的 shader map 的 B 通道平均值约 23/255，而 G
通道约 154/255；B 的高光强度随窗格变化，正是截图中一部分玻璃反光、一部分哑光的
直接来源。游戏发行版 `building4DeferredPS` 明确使用
`shaderMapSampled.b` 作为 `specStrength`，不能用 A 通道把 B/G 自动混合，否则
会把墙面 spec 错当玻璃 spec；Property Editor 继续保持发行版的 B 通道口径。

本轮修复了另一个会放大该差异的采样错误：游戏的 `uv_ddx2/uv_ddy2` 是
`ddx(uv2 * regionXform2.xy)`，不包含 `tilePadding`。Property Editor 之前把
`(1 + tilePadding)` 乘进 Top 导数；窗列中的 padding 常常是数千，导致 GPU 直接
选择整张 atlas 的粗 mip，窗框/玻璃被平均成哑光，且 relief 的 Top alpha 看起来
像是只在部分窗户生效。现在 Top 的 tint、normal、shader map 都使用原始
`uv2 * regionXform2` 导数，padding 只留在 relief tile-domain 和 outsideTile
判定中。

另外修复了 LOTM 导出中“第一个 TEXCOORD 就当 facade”的隐患：当 RW4 同时存在
普通 FLOAT2 与 facade FLOAT4 时，导出器现在选择坐标幅度最大的 FLOAT4，并且只有
所有顶点都具备该流才声明 `uvKind=2`；后端烘焙调色板颜色也使用同一选择规则。这样
不会再把普通 UV 或零填充的顶点送进 Top/relief 链，避免同一栋建筑出现局部窗户失效。

最后移除了 Property Editor 对“大 padding 列跳过 relief”的额外分支。发行版源码对
每个 Top 列都调用 `reliefMap`；padding 只影响 relief 源坐标，最终是否保留 Top
由 `outsideTile` 决定。跳过大 padding 列会把本来应由越界过滤的窗列直接变成无
浮雕，正好对应 `4afd3ded` 的整栋建筑症状。

## 视觉语义

relief 不增加几何，也不是独立的法线贴图。它通过改变 Top 层 tint、normal、shader map 和 palette 的采样坐标，让窗框、门洞和立面 motif 在视角变化时产生 pop-in/pop-out 的遮挡效果。真正的窗洞裁剪仍来自 tint alpha 的 alpha-test，假内景仍由 interior map 的盒体投影负责。

## 验证

- `pnpm exec vitest run src/pages/packages/components/property-editor/refinedRender.test.ts`：5 项通过。
- `pnpm check`（`vue-tsc -b --noEmit`）：通过。
- `pnpm build`：通过。
- `git diff --check`：通过。
- 单元测试和类型检查已通过；仍需要在真实 Property Editor fixture 中从正视、斜视、掠射视角复核门窗凹陷方向，并确认镜像 Top 区域、超大 padding 和远景 LOD 不回归。
- 已用 Chrome headless + SwiftShader 加载 `makeTintMaterial`、带 `TINT_PARAMS/TINT_SHADERMAP` 和 TANGENT 的合成平面，实际完成 WebGL program 编译，`gl.getError() = 0`。当前仓库没有可直接复用的 building4 贴图 fixture，因此尚未把真实门窗像素的凹凸方向自动化截图锁定。
