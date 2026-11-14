# WebGL 上下文与 decal 纹理警告（2026-11-01）

## 1. `Too many active WebGL contexts` / `glGetProgramiv` 报错

原因：`ThreeViewer.dispose()` 只调用了 `renderer.dispose()`。three 的
`dispose()` 释放 GPU 资源，但**不会丢弃 WebGL context**；context 要等 GC，
而 Chromium 同时只允许约 16 个。反复打开 property/package 预览后：

```
WARNING: Too many active WebGL contexts. Oldest context will be lost.
GL_INVALID_OPERATION: glGetProgramiv: Expected a program name, but found a shader name.
GL_INVALID_VALUE: glGetProgramiv: Program object expected.
```

旧 context 被强行丢弃，three 的 program/material 引用随即失效，后续
`useProgram`/`getProgramiv` 就会刷屏。

修复：在 `three-viewer.ts`、`RasterDecalPreview.vue`、`MapViewer3D.vue` 的
dispose 路径调用 `renderer.forceContextLoss()`，并移除 canvas。这样每次关闭
预览都会真正归还 context。

## 2. `[decal] DTO 无 png：Lot Textures ... not found in open packages`

原因：`read_lot_editor_session` 在解析 session 时就要读 decal 的 Lot Textures，
但 EcoGame 包只在之后的 `resolve_prop_models` 才注册。第一次打开 property
时包池里还没有这些包，decal 只能缺 png。

修复：在 `build_lot_editor_session` 开始前调用
`prop_models::ensure_for_session(manager)`，先把 EcoGame 注册进包池；包池
上限已提高到 64，能容纳 26 个 EcoGame 家族 + 用户包。

## 3. 仍需注意

- 如果 decal 的 Lot Textures 不在 EcoGame 目录，而是某个未打开的 graphics
  包，仍会报同样警告；需要把该包加入自动查找或让用户先打开。
- WebGL context 泄漏修复不改变渲染画面，只影响反复开关后的稳定性。
