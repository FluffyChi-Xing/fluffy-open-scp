# 破洞 decal：恢复两套坐标（2026-11-21）

## 已确认的源码证据

反编译树根为 `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/`。
用户指定的开发包目录中的 `SimCity.c` 实际只有 42 字节，是反编译失败的 `#error`，不能作为完整源码。

- `SC/cDecalData/SpawnDecalInstance.c:125-138`：逆变换构造 modelToTexture，Z 使用 `2/depth` 并减去半深，映射完整的 `[0,depth]` 到 `[-1,1]`。
- 同文件 `246-310`：生成八个盒体顶点，XY 为 `±aspect/±1`，Z 为 `0/depth`，再应用实例变换。
- `anonymous/cDecalBatch/AllocateIBCube.c:31-32`：写入 36 个 cube 索引。
- `docs/re/decal-shader-fragments/shader_frags_decalInteriorMap.txt` 是已有提取的 HLSL 片段证据：VS [256] decalProject 传递 cameraToTexture；VS [257] decalFloatQuad 另传 `modelToTexture * modelPos`。
- PS [374] 用场景深度重建 **texturePosition**，从它的 XY 采样焦痕/洞口纹理；PS [375] 是 `clip(1 - abs(texturePosition))`，裁剪完整三维盒，绝非裁掉 Z 的半空间。
- PS [380] 的 **textureFloatPosition** 来自 VS 的盒体几何位置，使用 `lerp(tfp.xy, tfp.xy*0.5, tfp.z*0.5+0.5)*-0.5+0.5` 采样同一纹理，最后通过洞口纹理 alpha 混合。

## 旧实现为何失败

2026-10-24 调查把 receiver position 和 cube raster position 合并了，并加入原始 [375] 中不存在的 `z < 0 discard`。
因此即使逐字抄对第二套 UV 公式，两次采样仍被固定在同一建筑接收面；平墙没有额外几何时不产生预期视差。
旧文档“必须给建筑做出内部几何才有假内景”的结论不成立。不要据此给资产添加内部楼板或重复使用 building4 窗户房间算法。

## 实现范围与推断边界

新增 `src/render/exec/three/holeDecal.ts`，只在 `decalBody.ts` 中 engine hole 材质装配后调用。其他族与原始材质工厂保持旧路径。
保留 CPU 裁剪的建筑接收面，以接收面位置替代 deferred depth 重建结果。
沿相机到接收面的视线，解析计算归一化 decal 盒体的远端交点，作为第二套几何坐标，再使用原 HLSL 的双 UV 混合公式。
这是对盒体远端光栅位置的解析重建，不是建筑房间盒算法，也没有改变模型本体。
正交相机使用平行视线；相机通过实际 mesh 世界逆矩阵变换，每次 draw 更新，支持变换父组。

**推断**：用远端盒面作为原引擎该通道的光栅面，是当前适配选择。盒体顶点和双坐标来源有静态证据，但本轮未重新提取破洞材质的完整编译剔除/深度状态，也未运行游戏进行逐像素对拍，因此不声称完整运行时或字节等价。

## 验证

- 定向 Vitest：4 个文件、10 项通过（含新完整体积映射、父变换、正交相机检查）。
- `vue-tsc -b --noEmit`、改动文件 ESLint 通过。
- 本机 Chrome headless 实际 WebGL 编译：1 个 shader program，0 个 shader error。
- `tmp/hole-parity/index.html` 使用附件 PNG、简化接收面和三个视角；`tmp/hole-parity/check.png` 显示视角相关的第二套采样位移。此 PNG 是用户提供的图像，不是经验证的原始游戏 RGBA raster；这项检查只证明 shader 编译和视差路径，不能验收真实纹理 alpha、昼夜光照或建筑曲面上的最终游戏观感。
- 未部署游戏包、未启动游戏、未修改存档或 SimCityData。真实建筑/原始纹理的视觉验收仍待进行。

状态与资源链见 [hole-decal-state-chain-20261121.md](hole-decal-state-chain-20261121.md)。
