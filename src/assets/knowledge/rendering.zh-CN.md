# 渲染管线简介

SimCity (2013) 使用 D3D9 级别的延迟着色管线，建筑立面采用数据驱动的
多采样材质系统。本文面向模组作者，介绍与贴图最相关的部分。

## 建筑材质六采样器

建筑 shader 家族（`building4`）一次像素着色最多采样 6 张纹理，与
RW4 材质的 6 个槽位一一对应：

1. **slot0 参数表**：float4 行表——调色板 UV、regionXform（Base/Top 层
   的 UV 变换）、tilePadding、房间尺寸；
2. **slot1 颜色控制图**：RGB=tint 区域选择，B 通道×2 为亮度乘子，
   A=镂空掩码（下向面剔除）；
3. **slot2 法线图**：标准切线空间，alpha=AO；
4. **slot3 shader map**：G/B=specularity（墙/窗），A=窗洞位置（假内景）；
5. **slot4 调色板**：512×16，tint 查表着色 + 表面反射参数；
6. **slot5 内景图**：预渲染房间图集，alpha=逐窗灯亮。

## 两层立面（Base / Top）

- **Base 层**：主体墙面，UV = facade 世界投影 ×regionXform；
- **Top 层**：窗户 motif，UV 来自第二套投影（uv2）×regionXform2，
  按 slot1 的 A 通道混合。
- specularity 通道：墙面用 G、窗玻璃用 B，互补窗洞掩码 A。

## 假内景

`ClipAndReliefMapPS` 按 shader map 的 A 通道确定窗洞，逐窗格用噪声选房，
把 slot5 房间图集做盒体投影投进窗内；夜间 alpha 通道点亮"室内灯"。

## 地面与地块

- LotMask raster 四通道 = 4 个布尔选区（阈值 128）；
- 每通道 `LotColor.RGB` 着色、`.A`（0-15）索引 16 格地面贴图图集；
- 定位由 property（`LotPlacementTransform` 逆）决定，不在 shader 内。

> 完整逆向记录见 `docs/rendering.md` 与 `docs/overview/rendering-pipeline.md`。
