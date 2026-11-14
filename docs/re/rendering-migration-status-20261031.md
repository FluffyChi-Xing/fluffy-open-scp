# 渲染迁移收尾状态（2026-10-31）

## 已完成

| 域 | 状态 | 位置 |
|---|---|---|
| contract 类型 | ✅ | `src/render/contract/types.ts` |
| 矩阵 / TRS / 地面像素核 | ✅ | `src/render/resolve/` |
| 单元分发 / 地面 / 贴花 / 模型 / 浮雕 / Prop 语义 | ✅ | `src/render/plan/` |
| three 执行器（单元、Props、模型、贴花、地面、覆盖层、浮雕） | ✅ | `src/render/exec/three/` |
| 阶段编排（单元、贴花、地面） | ✅ | `src/render/session/` |
| 旧 renderer / 旧构建器 | ✅ 删除 | `renderers/{prop,tree,spawner}Renderer.ts`、`unitGizmos` 构建器 |
| 迁移期 adapter | ✅ 删除 | 原 `adapters/legacyViewport.ts` 已内联到 `renderers/index.ts` |
| 对拍 / 测试 / 文档 | ✅ | 605 用例、tsc/eslint、`docs/re/*-20261031.md` |

## 仍留在视口（有意保留）

- `assembleScene` 的模型/地面汇合、dims 标注、遥测、首帧矩阵刷新；
- gizmo / TransformControls / 选择 / 悬停；
- 粒子 effects 与真实灯光预算；
- 地面贴图的 worker/canvas 宿主（纯像素核已迁 `resolve/groundCompose`）。

## 后续可选批次

1. `session/renderScene.ts`：收拢 `assembleScene` 剩余编排；
2. 地面资源宿主迁入资源层；
3. effects/particles + 灯光预算独立模块；
4. 性能项 O1/O3/O4（交互分辨率、静态阴影、浮雕 uniform 切换）；
5. `FROZEN.md` 更新与最终提交。

## 当前指标

- `PropertyEditorViewport.vue`：2358 行（迁移前 3128）；
- `src/render`：39 个文件；
- 全量测试：120 文件 / 605 用例；
- 分支：`dev/rendering-optimisation`。
