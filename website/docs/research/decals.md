---
title: 一张贴花，如何抵达建筑表面
description: 从 Property 字典与纹理载体，到投影盒与材质通道。
research: true
---
# 一张贴花，如何抵达建筑表面

> 渲染笔记 · 文档摘要 · 整理于 2026-10-15。依据已有探针、shader 分析与游戏对拍记录；本文不新增实测结论。

招牌、涂鸦、破洞和废墟墙块可以沿着同一条问题链研究：它在资源中如何描述、纹理在哪里、几何如何投影、最终使用什么材质语义？

## 字典不是一张图片

项目研究指出，decal atlas 是普通 **Property 资源**，包含材质引用、纹理尺寸与条目字段。条目以并行数组存储，同一个下标的元素共同组成一个贴花条目。

这使“找到 atlas”与“找到实际纹理”成为两个步骤。

## 纹理有不止一种载体

纹理可能是裸 Raster（`0x2F4E681C`），也可能嵌在 RW4（`0x2F4E681B`）中。某些引用跨越游戏包与扩展内容包，因此仅打开一个 package 未必足以得到完整贴花。

## 从变换到投影盒

归档的 shader 分析显示，顶点位置先变换到纹理空间，再通过投影盒边界做体积裁剪。原文记录的关键表达式是：

```glsl
clip(1 - abs(texturePosition));
```

这里展示的是原始 shader 语义片段，不是可直接运行的完整 GLSL 程序。UV 从盒内归一化坐标推导，因此错误的轴向、矩阵约定或尺寸语义都会造成明显错位。

## 不能只靠纹理外观判断语义

商业招牌、涂鸦与破损墙块在材质与通道处理上存在差异。低分辨率纹理被投影到数米宽的表面时，也可能自然出现像素化；这不能单独证明解码错误。

一个更可靠的验证顺序是：检查条目引用，核对纹理载体，再比较变换与投影尺寸，最后核对通道和材质。

## 依据与边界

- 原始研究：`docs/blog/decal-rendering-pipeline.md`。
- 实现线索：`crates/sc-properties/src/decal.rs`、`crates/rw4/src/raster.rs`、`src/lib/decalProject.ts`。
- 原文结合真实包探针、shader 片段与游戏画面对比；本文不将特定样本结果推广为全部贴花家族的规则。
- [阅读归档原文](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/blog/decal-rendering-pipeline.md)。
