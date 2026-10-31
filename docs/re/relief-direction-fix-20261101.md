# 门窗浮雕方向不一致修复（2026-11-01）

## 现象

同一面墙上部分窗/门浮雕向内、部分向外；没有可复现 TGI，只能按坐标系与
Top 层采样域推断。

## 根因

原先 `scReliefEyeDir()` 在 `USE_TANGENT` 分支使用 glTF 导出的 **TANGENT**：

```
tangentU = normalize(vReliefTangent);
tangentV = normalize(cross(tangentU, normalV));
```

问题有两个：

1. glTF TANGENT 是按 **基础 UV（TEXCOORD_0）** 生成的，而浮雕实际采样的是
   **Top/facade UV（TEXCOORD_3）**。同一面墙的不同窗格可能有不同的 Top UV
   朝向/镜像（负 region scale），基础 UV 的切线不能代表 Top UV 的 u/v 方向，
   于是同一面墙上出现“有的内翻、有的外翻”。
2. 镜像 UV island 的 handedness 存在 `tangent.w`，但只取 `xyz` 会丢失符号；
   另外 `cross(T,N)` 与 three 的 `vBitangent = cross(N,T) * tangent.w` 相反。

## 修复

`exec/three/reliefShader.ts` 的 `scReliefEyeDir()` 改为**始终用 Top UV 的屏幕
导数**构造浮雕切线架：

```
dP/du, dP/dv 由 dFdx/dFdy(vTopUv) + dFdx/dFdy(-vViewPosition) 重建
T = normalize(dP/du)
N = normalize(vReliefNormal)（双面材质按 gl_FrontFacing 翻转）
B = normalize(cross(N, T))        // 与 dP/dv 同向
eye = (dot(eyeV,T), dot(eyeV,B), dot(eyeV,N))
```

这样每个窗格的浮雕方向跟随它自己的 Top UV，不再依赖基础 UV 切线；镜像
窗列、双面三角形和 three 的 Y-up 坐标都落在同一套推导里。

保留的改动：

- `vReliefTangentW = tangent.w` 仅用于法线贴图 TBN（`scNormalB`）的 handedness；
- 浮雕本体不再使用 TANGENT，因此不会因 tangent 与 Top UV 不一致而翻转。

## 验证

- `vue-tsc` / 全量 Vitest（605 用例）通过；
- headless harness `property.html?relief=1` 无 `useProgram: program not valid`；
- 仍缺真实 TGI 的目视对拍：需要用户提供一栋能复现“同墙内翻/外翻”的建筑，
  从正面、斜视、掠射三个角度截图确认。
