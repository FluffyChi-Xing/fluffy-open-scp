# 建筑浮雕（relief）逆向对拍与处理器优化

日期：2026-10-31

## 1. 本次核对的对象

- 发行版 shader 片段：`tmp/building_shaders_full.txt`（两份 building4ClipAndReliefMapPS 片段）
- Maya 作者 shader：`docs/re/maya-building4-authoring-shader.fx_source.txt`
- 既有逆向结论：`docs/re/building-facade-engine-analysis.md`、
  `docs/re/property-editor-reliefPS-analysis-20261024.md`、
  `docs/re/property-editor-material-cost.md`
- 用户提供的开发版目录：
  `D:\ea-games\simcity_dev\new-cource\Simcity Development Files Including PDB Debug files`
  （`SimCity.exe` / `SimCity.pdb` / `SimCity.i64`，**没有 C++ 源码或 shader 源文件**）

## 2. 源码里没有 relief 实现，只有材质名

对 `SimCity.pdb` / `SimCity.exe` 的字符串扫描结果：

- `relief` / `Relief`：0 处
- `shaderMap`：0 处
- 只存在材质名：`Building4RWPackedUV`、`Building4RWPackedUVNoBackfaceCulling`、
  `Building4RWColorOverlayOccluded/NonOccluded`，以及 `kMaterialIdBuilding4Packed`
- 没有 `Building3` / `Building5` 材质名

结论：**开发版目录能证明“哪些材质属于 Building4 家族”，但不能提供 relief
公式**。公式仍以发行版 shader 容器片段为准（两份片段如下）。

## 3. 两份片段不能混用

`tmp/building_shaders_full.txt` 中存在两个版本的 relief 片段：

| 常量 | authoring/调试变体（:803-822） | 发行版完整变体（:888-935） |
| --- | ---: | ---: |
| `kReliefDepth` | 0.1 | **0.16** |
| `kCullDistanceSq` | 160000 | **16000**（≈126 m） |
| `kConeSteps` / `kBinarySteps` | 8 / 2 | 8 / 2 |
| `kFlatLevel` | 23/255 | 23/255 |
| `reliefMap` 实现 | 直接 `return tc`（stub） | 完整 cone-step + binary refine |

当前实现此前采用了 **0.16 + 160000** 的混合值，即“发行版深度 + authoring 剔除
距离”。这就是远景建筑仍有浮雕、近景代价偏高的一个直接原因。

## 4. 逐项对拍（修复前 → 修复后）

| 项 | 发行版片段 | 修复前 | 修复后 |
| --- | --- | --- | --- |
| 距离剔除 | 16000 | 160000 | **16000** |
| 深度 | 0.16 | 0.16 | 0.16 |
| cone / binary 步数 | 8 / 2 | 8 / 2 | 8 / 2 |
| flat 处理 | `tc -= ds.xy * kFlatLevel`，不提前返回 | 额外 `sourceControl.r <= kFlatLevel` 提前返回 | **删除提前返回**，只保留 flat 偏移 |
| 采样域导数 | Base 域 `uv_ddx/uv_ddy` | Base 域（已修） | Base 域 |
| 视线方向 | `ds.xy = -ds.xy` | 一致 | 一致 |
| 切线空间 | `cross(tangent, normal)` | 一致 | 一致 |
| 结果钳制 | 返回未钳制 `s.xy`，由 outsideTile 判界 | 一致 | 一致 |

## 5. “不是所有建筑都有浮雕”

两条独立原因：

1. **shader 家族**：relief 只存在于 `building4ClipAndReliefMapPS`。PDB 的材质名
   证据也只列出 `Building4RW*`。编辑器只有走 building4 tint 链（有 params +
   tint + palette + shaderMap）的材质才可能进入 relief；其他材质（simple
   diffuse / 车辆 / prop / building5 等）本来就没有。
2. **高度场本身是平的**：building4 材质里也有大量 `shaderMap.r <= 23/255` 的
   平面墙。发行版仍会执行 cone-step，只是位移为 0；视觉上没有浮雕。

本轮把第 2 条变成显式的**逐材质高度可用性**：解码 shaderMap PNG，若
R > 23/255 的像素不足 1%，则该材质 `uReliefEnabled = 0`，直接跳过 8+2 次采样。
这样平面建筑不再付浮雕成本，画面与“位移为 0”一致。

## 6. 代码落点

- `src/render/plan/relief.ts`：纯函数 `planRelief` + 发行版常量 +
  `reliefHasHeight`（RGBA 高度占比判定，可单测）
- `src/render/exec/three/reliefShader.ts`：`RELIEF_GLSL`（cone-step 片段，
  常量由 plan 插值，避免魔法数字）
- `refinedRender.ts`：删除 70 行内联 GLSL，改为 `${RELIEF_GLSL}`；
  加载 tint 贴图时解码 shaderMap 计算 `reliefAvailable`；
  `makeTintMaterial` 用 `planRelief` 决定 uniform
- 测试：`plan/relief.test.ts` 5 项（常量、开关、GLSL 不含 160000/0.1、
  平面/1% 高度判定）

## 7. 仍未闭环的部分

- 材质 → shader-def 的 **PS 变体名**未在前端 payload 中暴露，因此“是否为
  building4ClipAndReliefMap”仍以 tint 六槽链路间接判定；如果后续要精确到
  shader 变体，需要在后端 `resolve_material_resources` 增加 shader-def 名称。
- 真实游戏内肉眼对拍尚未进行；当前只能保证公式、常量、分支顺序与发行版
  片段一致。建议在真实 lot 的近景/斜视/远景各截一张，对比窗框 pop-in 与
  126 m 外的平面化。
