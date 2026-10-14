---
title: 工作台使用指南
description: 从资源浏览、模型与地图预览到 overlay 写回。
---
# 探索、编辑，再构建

OpenSCP 的不同面板处理同一条资源链上的不同问题。本页先说明可以做什么，再说明需要留意的边界。

## 资源浏览

打开资源包后，按类型或 TGI 定位资源。文本会以语法高亮展示；未知格式可以使用 hex 视图检查。切换页面时，当前打开的包、筛选与页码会保留。

![资源浏览器](/images/zh-CN/source-parsing.png)

### 资源总览

通过资源总览查看已扫描的包与资源规模，以及类型数量和容量分布，再选择需要深入检查的资源包。

![资源总览与类型分布](/images/zh-CN/resource-overview.png)

## 模型与材质

RW4 视口用于观察模型、LOD 与材质通道。Property 编辑器可查看 Lot 的灯光、贴花、Prop 与路径单位；日夜时间及供电状态帮助比较建筑的不同表现。

模型预览是对游戏表现的持续复现，尚不能将所有预览结果视为与原版游戏完全一致。

![模型与材质预览](/images/zh-CN/rendering.png)

## 地图与 Raster

地图面板将区域地形、城市地块与资源图层放在同一空间中。Raster 绘制提供四通道 LotMask 画布，支持笔刷、填充与停车位标线，并可保存为 overlay package。

![地图预览](/images/zh-CN/map-review.png)

![Raster 四通道绘制工作台](/images/zh-CN/raster-paint.png)

## Mod Studio

| 面板 | 使用方式 | 状态边界 |
| --- | --- | --- |
| TGI 复写检测 | 扫描游戏和 mod 目录，查看同 TGI 的覆盖链及差异 | README 标记可用 |
| Locale 文本编辑 | 通过 stringID 筛选文本、编辑、按行还原并导出 overlay | README 标记可用 |
| UI 预览 | 查看通过游戏 CSS、locale 与采样图装配的游戏 UI | 预览版；完整编辑仍在完善 |

## 一条建议的验证路径

1. 选定目标资源，记录 TGI 和原始值。
2. 在对应面板进行单一、可比较的修改。
3. 导出 overlay，确认写回的资源和预期一致。
4. 使用复写检测检查与其他 mod 的覆盖关系。
5. 在游戏内验证，并保留结果与还原方式。

来源：[项目 README](https://github.com/FluffyChi-Xing/fluffy-open-scp#readme)。本页截图随官网语言切换。
