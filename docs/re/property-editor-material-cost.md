# Property editor 建筑材质修复与成本评估

## 已修复

`refinedRender.ts` 现在在有 `shaderMap` 的建筑材质上使用 building4 同口径的短
cone-step relief：高度取 `shaderMap.r`，最多 8 次 cone step + 2 次二分细化，
平面阈值为 `23/255`，视距平方超过 `16000`（约 126 m）时直接跳过。采样继续使用
未 `fract` 的显式导数，因此不会在窗格重复边界错误选择 mip。原来的实现把该段明确
回滚为恒等映射，这正是窗框、墙面和其他立面缺少浮雕纵深的原因。

当前 relief 还会检查 `shaderMap.a` 的 artist-opacity/window mask：不透明墙面
（`a > 0.90`）直接跳过，只有门窗、栏杆等被材质标记的区域才进入视差步进，避免
把普通墙面高度图噪声放大成条纹。

玻璃/幕墙的反射链仍使用 `shaderMap.b/g` 的高光强度、palette surface 行和天空解析
反射；普通 UV 材质继续绑定 base/normal/roughness/AO 四张贴图。这样建筑窗户、墙面、
金属件共用同一套材质输入，不需要为玻璃增加额外几何体。

## 游戏侧证据

`D:\\ea-games\\simcity_dev\\new-cource\\answer.txt` 与 `docs/re/building-facade-engine-analysis.md`
显示游戏的建筑管线是：tint alpha 裁剪 → relief/cone-step → normal/palette →
interior map → deferred lighting/combine。窗户主要是纹理 alpha、视差和假内景，
不是独立窗户网格；玻璃幕墙的明显反光来自 cubemap/天空环境反射与高光参数叠加。

## 渲染成本

| 路径 | 额外工作 | 适用范围 |
| --- | --- | --- |
| 平面或远距离 | 1 次高度探测，随后退出 | 高度 ≤ 23/255 或距离 ≥ 126 m |
| 近距离 relief | 最多 10 次 `shaderMap` 采样（8+2），复用同一组 UV 导数 | 近景、有高度变化的建筑立面 |
| 玻璃反射 | 现有天空解析反射与高光计算，无新增纹理/几何 pass | 玻璃、幕墙、金属高光 |

按 1080p、立面覆盖 15% 屏幕估算，最坏 relief 路径约增加 1.5M 个纹理采样/帧；
在典型编辑器镜头（覆盖 5%–10%）约为 0.5M–1.0M。距离裁剪和 `flat-level` 快速路径
会使大多数墙面只付一次探测成本。它不增加 draw call、顶点数或显存；主要代价是近景
像素着色器时间，低端 GPU 预计约 0.3–1.2 ms，高端 GPU 通常低于 0.4 ms。若需要更
低成本，可把 cone steps 从 8 降为 6，并保留 2 次二分，画质损失主要出现在极斜视角。

## 验证限制

仓库现有测试全部通过，另有一个与本改动无关的 i18n 审计失败（缺失
`tickHint/chartSource/shareHint/requests/source` 双语键）。实际 GPU 视觉效果仍需在
property editor 中加载带 `shaderMap.r` 高度数据的建筑资源进行对拍。
