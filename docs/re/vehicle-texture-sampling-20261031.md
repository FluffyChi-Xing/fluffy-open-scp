# 车辆 / 船只贴图采样对拍（2026-10-31）

## 结论

车辆和船只与建筑不是同一条材质链：它们使用**传统 UV 漫反射贴图**（LOTM
slot0 彩色图），不走 building4 的 tint/palette/relief 链。之前 `propModels`
的通用纹理加载把各向异性硬编码成 **4×**，而游戏渲染器在开启各向异性时使用
**8×**（`SimCity-dev-ida.c` :278970-278971，`sEnableAnisotropic ? 8 : 1`；
`D3D9SamplerStateDispatch` 在 LINEAR MINFILTER 时升级为 Anisotropic）。

已把 prop/车辆纹理路径改为 **8×**：

- `propModels.ts` 的 `loadTexture()`（车辆/prop 漫反射、法线）
- `getPropModelObject()` 里 vehicles 的 `loadTintTextures(..., 8, ...)`

树使用**独立的** `getTreeModelObject()` 图集加载（另一条 `anisotropy = 4`
路径），本次没有改它，因此不会回到“树被 alpha/过滤误伤”的回归。

## 为什么近景仍然会软

车辆资产实测为 **128×128** 漫反射（`prop-0AB4DFE1` LOD0..3 全部 128²；
部分车辆 128×256）。在编辑器里把镜头拉到整车占屏 700–900 px 时，
放大倍率约 5–7×，双线性放大本身就会软；游戏内车辆通常只占几十像素，
同样 128² 看起来是清晰的。8× 各向异性只改善**掠射/缩小**方向，不能
凭空恢复 128² 不存在的细节。

如果要在编辑器里得到“比游戏更锐”的预览，可选：

1. 车辆专用 **bicubic 放大**（类似建筑 tint 的 `scSampleFacade`，但只作用于
   vehicle 处理器，避免树/建筑回归）；
2. 载入时 2× 锐化上采样（增加显存，且不是游戏行为）；
3. 找到车辆更高分辨率 LOD/资产（如果存在）。

当前只做了与游戏一致的 8× 各向异性，没有引入锐化滤镜。
