# 破洞 decal 与引擎簇 shader 对拍记录（2026-10-24）

## 结论

破洞材质实例 `0x4491DE3A` 使用 `decalInteriorMap` 簇。它和建筑立面 `building4ClipAndReliefMapPS` 的 `interiorMap()` 是两条不同的链：前者消费破洞贴图自己的第二套 UV，后者用于建筑 facade 的窗户/房间假内景。把后者的相机射线盒体求交叠加到破洞簇，会把窗口周围的墙面和贴图边缘一起拉伸，造成破洞方向、内凹范围和内部结构都偏离游戏。

## 引擎原始链

`docs/re/decal-shader-fragments/shader_frags_decalInteriorMap.txt` 给出的 PS 顺序是：

1. `decalProject` 用屏幕深度和 `cameraToTexture` 重建 `texturePosition`，并采样破洞 raster。
2. `decalClip` 执行 `clip(-textureFloatPosition.z)`，只保留盒体的 `z >= 0` 半空间。
3. `decalLightInteriorMap` 使用同一张 raster 的内景相位：

```hlsl
float2 interiorUv = lerp(
    textureFloatPosition.xy,
    textureFloatPosition.xy * 0.5,
    textureFloatPosition.z * 0.5 + 0.5);
interiorUv = interiorUv * -0.5 + 0.5;
```

随后按 `decalTexture.a * 2 - 1` 混合前景焦痕和内景颜色，按 `interiorTexture.a * 16` 加入内景自发光。这里没有 `eyeDir`、房间盒体求交或 `building4 interiorMap()` 调用。

破洞随视角变化的来源是屏幕深度投影和深度测试：视角改变后，洞盒内可见的楼板、内墙和远侧结构改变，`textureFloatPosition.z` 也随实际深度改变，于是第二套 UV 的收缩比例改变。它不是在片元里额外发射一条相机射线。

## Property Editor 对齐

- `hole.vert.glsl` 继续把 `DecalGeometry` 生成的 lot 局部位置映射为 `tfp.z ∈ [-1, 1]`，对应引擎 model-to-texture 的深度坐标。
- `hole.frag.glsl` 改为逐字使用 `decalInteriorMap` 的双 UV 公式，并补上 `z < 0` 丢弃，防止投影盒前半段把洞周围外墙染成内景。
- 删除 `uHoleInvRot`、`uHoleCamLot` 和每帧相机位置更新；这组 uniform 属于错误引入的 facade 视差近似。
- sign、clip、sdf、neon-light 路由不变，只有 `family === "hole"` 走这条修复后的分支。

## 限制

Property Editor 当前使用 `DecalGeometry` 近似游戏的屏幕深度投影，无法像游戏的 deferred decal 一样在 GPU 中直接读取场景 depth buffer。因此真实建筑必须提供洞口/内部几何，才能通过深度测试表现出和游戏一致的内部结构；纯平立面不会被 shader 凭空变成立体洞口。
