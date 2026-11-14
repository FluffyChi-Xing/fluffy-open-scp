# property-editor-next / effects

这是 `property-editor/effects` 的**平行实现**，不修改冻结区（见
`property-editor/FROZEN.md`）。目前已经通过 `PropertyEditorViewport.vue` 的一行 import
接入，toolbar 开关与默认关闭逻辑保持不变。

## 与现有实现的差异

1. **单位边界**：`scale.ts` 负责 Swarm 局部单位与场景米的换算。默认
   `SWARM_UNIT_SCALE = 1`（真实样本 size 0.5–30、offset 1–6，与建筑 bbox 同量级）；
   可传 `unitScale` 或 localStorage 覆盖做对照。
2. **解算器拆分**：时间累加与发射累加分离，`advance(0.1)` 与两次 `advance(0.05)`
   结果一致（旧实现共用一个 `fraction`，结果依赖切分方式）。
3. **边界清晰**：`scale.ts` 只做单位换算，`solver.ts` 只做数值解算，`renderer.ts`
   只做 Three 对象与 shader，`frame.ts` 只做图集帧公式。
4. **保留现有正确行为**：真实图集、相邻帧 UV 插值、drawMode 4/5/6 加色、容量预算、
   确定性随机种子。

## 单位比例

默认 **1（米）**。早期按厘米 0.01 换算会让普通效果缩小 100 倍，实测几乎不可见。

对拍时可在 property editor 控制台切换比例，刷新/重开 lot 后生效：

```js
localStorage.setItem("openscpSwarmUnitScale", "0.01"); // 厘米假设
localStorage.setItem("openscpSwarmUnitScale", "1");    // 原始米（默认）
localStorage.removeItem("openscpSwarmUnitScale");       // 回到默认
```

## 接入状态

`PropertyEditorViewport.vue` 的 import 已切到本模块；默认关闭逻辑未变：

- `effectRendering = ref(false)`
- `effectsActive = effectRendering && refined && open`

因此默认仍不渲染任何粒子，只有手动打开 toolbar 开关才创建 `createParticleEffect`。

## 测试

```powershell
pnpm vitest run src/pages/packages/components/property-editor-next/effects
```
