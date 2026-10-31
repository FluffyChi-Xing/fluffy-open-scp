# 浮雕开关卡顿优化（2026-11-01）

## 现象

切换“浮雕”开关时部分建筑卡顿：旧视口把 `reliefEnabled` 放进重建依赖
（`watch([modelPayload, renderMode, reliefEnabled])`），每次切换都走一次
完整 `assembleScene`：GLB 解析/材质重建/贴图重新绑定/阴影与延迟链刷新。

## 修复

浮雕开关不再触发重建，而是**热更新材质 uniform**：

1. `makeTintMaterial` 把 `uReliefEnabled` 引用与 `reliefAvailable` 挂到
   材质 `userData`；
2. 视口新增 `applyReliefSwitch(enabled)`：遍历 `model` 组内 tint 材质，
   设置 `uReliefEnabled = enabled && reliefAvailable ? 1 : 0`，然后
   `viewer.invalidate()`；
3. 重建 watch 移除 `reliefEnabled`，只有 payload/renderMode 变化才重建。

效果：

- 切换只改 uniform + 重绘一帧，不重新解析模型/上传贴图/重编译材质；
- 平面建筑（`reliefAvailable=false`）保持关闭；
- 与之前修好的“按 Top UV 导数构造 relief 切线架”不冲突。

## 验证

- `vue-tsc` / `eslint` 通过；
- 全量测试通过；
- 尚未做 GPU 计时：需要用户在一栋会卡顿的建筑上切换开关，确认不再触发
  `scene_rebuild` 遥测（旧实现会新增一条 rebuild；新实现只增 draw）。
