# ⛔ 冻结区(2026-10-27 用户指令)

本目录(property editor 视口/装配/取景链路)自 2026-10-27 起**冻结**。
两轮「建筑飘在网格上方」回归的代价已证明:此处任何"顺手重构"都会以
回归收场。**后续重构/调整必须在全新的平行目录进行**(用户原话),不得
在本目录内渐进改造。

## 冻结的关键不变量(改动前必读,均有测试钉死)

| 不变量 | 位置 | 守卫测试 |
| --- | --- | --- |
| 网格/取景只锚定内容组(model/lot/dimensions),单元 gizmo 永不参与 | `useEditorViewport.ts` `FRAMING_GROUPS` → `three-viewer.ts frameContent` | `frameInvariants.test.ts` |
| 装配尾部强制 `scene.updateMatrixWorld(true)` 后才放行首帧 | `PropertyEditorViewport.vue` 装配尾部 | 代码注释标记(时序型,无法单测) |
| `frameContent` 软跟随:content 绝对归零 + 相机/轨道等量平移,渐进装配不依赖首帧时序 | `three-viewer.ts` | 代码注释标记 |
| 轴向帧契约唯一权威 `normalizeLotModelScene`(未声明 upAxis 一律剥根旋转) | `src/lib/three-gltf.ts` | `src/lib/three-gltf.test.ts` |
| placement 矩阵 WPF 行向量 → three 列向量转置 + 逆矩阵 | `editorGround.ts placementInverse` | `frameInvariants.test.ts` 黄金用例 |
| 取景取证开关 `localStorage.openscpPeDebug=1`(分诊入口,保留) | `PropertyEditorViewport.vue` 装配尾部 | — |

## 回归史(为什么冻结)

- 2026-10-26 `44e0369` 整场景旋转修正 → 原生建筑沉入地面;
- 2026-10-26 `8dadc93` 剥根旋转修复下沉 → 但量测侧未同步,draft 悬空;
- 2026-10-27 取证定位:单元 gizmo 拖动全内容包围盒 + 软跟随缺失 +
  首帧陈旧矩阵,三层叠加造成 5m~80m 不定幅度悬浮;
- 2026-10-27 修复闭环(`26ecf63`…`8d24d41`),用户确认。

## 分诊工具

任何"飘/沉/错位"复发:控制台执行
`localStorage.setItem("openscpPeDebug", "1")` 后刷新重开 lot,
`[pe-frame]` 表会打印各图层组包围盒、单元位置(⚠️低位标记)、grid.y、
placement 矩阵——先看表,再动手。
