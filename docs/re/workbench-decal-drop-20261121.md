# 资产编辑拖入 decal 后不可见（2026-11-21）

## 两处独立缺陷

1. `useWorkbenchAsset` 已通过 `writeDecalSources` 向场景 schema 写入 `decalId/decalSource`，且 `texturesFor` 正确绑定 category/index。但冻结的 `SchemaPropertyViewport` 调用 `unitsFromSchema` 重建单元，其 decal 白名单只有 category、scale、depth、materialData，导致进入实际 renderer 的 decal 丢失来源。`buildDecalBody` 仅为带来源的作者放置提供未命中接收面时的编辑预览；丢失来源后会返回没有子节点的 group。
2. hole 的未附着预览调用旧 hole shader。没有 holeBox 时 VS 得到恒定 Z=-1，旧 PS 又丢弃所有 Z<0 的像素。仅修复来源也不能让该族显示。

## 修复

- 在资产编辑目录新增平行 `WorkbenchSchemaViewport.vue`，原样保留冻结适配器的地块、可见性和事件透传逻辑；在 schema 解码后、分组前调用既有 `restoreDecalSources`。
- `ModAssetWorkbench.vue` 仅切换该适配器的 import。
- `decalBody.ts` 为作者放置且未命中接收面的 hole 平面配置专属双坐标 shader。该平面采用自身局部盒坐标，mesh 的抬升由 modelMatrix 处理，避免重复计算偏移。
- 不修改冻结 property-editor 目录，不改变原生未命中 decal 的投影专用行为。

## 验证与边界

- `WorkbenchSchemaViewport.test.ts` 将作者 decal 写入场景 JSON，挂载真正的工作台适配器，在视口边界读取单元并交给真实 `buildDecalBody`，覆盖 clip、sign/graffiti、SDF plane、hole 四条路径、场景更新及 hole 局部坐标。
- 共 5 文件、14 项定向测试通过；`vue-tsc -b --noEmit` 和改动文件 ESLint 通过。
- `tmp/decal-drop-check/index.html` 使用合成 RGBA 纹理和真实 schema/source/material 装配，在 Chrome headless 中实际 WebGL 绘制。clip、graffiti、hole 均生成一个预览对象且中心像素非空，shader errors=[]。
- 该 GPU 检查证明未附着拖入预览可绘制，不代替真实游戏纹理、建筑接收面位置与最终美术效果的交互验收。未启动或部署游戏。
