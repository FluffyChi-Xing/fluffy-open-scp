# Property Editor 大型场景性能审计

日期：2026-11-06

范围：Property Editor（PE）大型/复杂 Lot 场景视角移动、默认/精细渲染切换、地面精细合成、材质与模型缓存生命周期。

## 结论

当前卡顿的主因不是模型面数，而是以下组合：

1. PE 使用 WebView2 + Three.js 前向渲染，每个相机帧都重新执行整张画布的片元着色；Blender 视口使用持久化依赖图、GPU 资源和更激进的交互降级。
2. 精细模式启用 MeshStandard/自定义 tint shader、法线/粗糙度/AO、阴影和地面 Phong 材质。成本随屏幕像素、材质批次、灯光数量和阴影贴图增长，和三角形数量不是线性关系。
3. 默认/精细切换目前是全量 rebuild：清空所有场景组、dispose 材质、重新解析/克隆对象、重新建材质、解码贴图、合成 Lot、预热纹理，并调用同步 `renderer.compile`。这些步骤在主线程串行执行，UI 和输入会一起停顿。
4. 地面精细合成已经放入 Worker，但图像解码、DataTexture 创建、GPU 上传和首轮 shader 编译仍会回到主线程；Worker 不能消除 GPU 上传阻塞。

## 证据链

### 相机帧

- `src/lib/three-viewer.ts` 使用 `WebGLRenderer({ antialias: true, alpha: true, powerPreference: "high-performance" })`。
- PE 以 `maxPixelRatio: 2` 创建视口。高 DPI 屏幕可能渲染到 CSS 像素的 4 倍；交互期间目前只降到 DPR 1，仍然可能是百万级片元。
- render loop 是按需渲染，但相机拖动会每个 pointermove 调用 `invalidate()`，因此拖动期间接近连续 60 FPS。
- hover 在每个 pointermove 对所有登记目标递归 raycast；大型 props、spawner、decal 组会放大 CPU 射线检测成本。

### 精细模式

- `src/render/plan/model.ts`：精细模式启用 tint/deferred 材质策略和阴影。
- `src/render/exec/three/model.ts`：每个 mesh/材质组建立新的 `MeshStandardMaterial` 或自定义 tint 材质；材质不是按场景统一合并。
- `src/lib/three-viewer.ts`：精细模式阴影使用方向光和 `2048 × 2048` shadow map；真实灯光最多保留 48 个。Three.js 前向路径下，受光片元的成本会随灯光数量增加。
- 模型材质普遍使用 `DoubleSide`。对于不需要背面绘制的建筑面，这会增加片元工作量。
- 选中/悬停轮廓会复制目标 mesh 的可见壳体；复杂对象选中时会额外增加绘制批次。

### Lot 地面

- `src/render/session/groundStage.ts` 会并行读取 surface、mask 尺寸和 normal atlas。
- `src/pages/packages/components/property-editor/refinedGround.ts` 对 mask、surface、normal atlas 做像素提取和合成；输出通常按图案原生密度生成较大的 `DataTexture`。
- `src/render/exec/three/groundTextures.ts` 在精细模式建立 `MeshPhongMaterial`，绑定合成 albedo/normal，并在模式切换或缓存未命中时重新合成。
- `src/lib/three-viewer.ts` 在 rebuild 尾部调用 `primeTextures()` 遍历整棵场景并逐纹理 `renderer.initTexture()`，然后调用同步 `renderer.compile()`。这两步都可能长时间占用主线程或驱动线程。
- 地面合成缓存容量为 2；模式、Lot、材质颜色或图集键变化后可能淘汰旧结果。缓存淘汰会 dispose GPU 纹理，下一次切换重新付出上传和合成成本。

### 模式切换和 UI 卡死

`useEditorViewport.rebuild()` 的实际顺序是：

```text
clearGroup + dispose
→ assembleScene
→ model/units/ground/decal 全量装配
→ scene.updateMatrixWorld
→ frameContent
→ primeTextures（同步 GPU 上传）
→ renderer.compile（同步 shader 编译）
→ 放行首帧
```

`scheduleRebuild()` 已经把同一帧请求合并并串行化，这解决了重复 rebuild 和 shader poll 竞态，但没有改变单次 rebuild 的同步工作量。因此切换模式时出现“UI 卡死”是当前架构的自然结果，而不是单纯缓存失效。

## 为什么 Blender 同场景更流畅

Blender 的材质节点可能更复杂，但交互视口默认采用一组专门的交互策略：

- 场景对象、几何、图片和 shader 程序长期驻留，视角变化只更新 camera matrix；
- 依赖图只重算受影响部分，不因切换视图着色而销毁整棵场景；
- 交互时使用视口降级、遮挡/视锥裁剪、LOD、纹理 mip 和 GPU 缓存；
- shader 编译和纹理上传通常发生在资源准备阶段，拖动相机时不会重新建材质；
- 视口绘制重点是保持输入响应，完整质量可以延后到停下后。

PE 当前即使几何只有几千面，也可能同时付出：高 DPR 片元、法线/粗糙度/AO、多个材质批次、48 灯前向光、2048 阴影、透明贴花、轮廓壳和地面大纹理上传。瓶颈自然会落在 GPU 片元和主线程同步，而不是三角形吞吐。

## 推荐目标架构

### P0：先保证视角移动始终可交互

1. **交互质量状态机**
   - pointerdown/相机开始移动：DPR 降到 0.75–1、关闭阴影、normalMap、DOF、实时 decal 投影和粒子更新；保留基础 albedo。
   - pointermove 只更新相机，不触发场景 rebuild。
   - pointerup 后等待 120–200 ms，再恢复质量并请求一次渲染。
   - 当前已有 `interactionPixelRatio`，应扩展为材质/阴影质量开关；只降 DPR 对高光栅化和阴影场景不够。

2. **hover 节流和拾取代理**
   - hover raycast 限制为 30 Hz，并只对 unit anchor/proxy 做射线检测；不要递归扫描真实高面数模型。
   - 选中轮廓使用单个低模 hull 或屏幕空间 outline，避免复制复杂 mesh 子树。

3. **精细模式限制实时灯光**
   - 地面和建筑只保留太阳 + 环境光；单元灯默认使用 emissive marker，近距离明确开启时才创建真实 Light。
   - 将 48 灯硬上限改为按质量档动态预算，近景高档也建议先控制在 8–16 个。

### P1：默认模式立即可见，精细模式渐进替换

1. 把模型对象、Lot 几何、unit 对象与材质策略分离。模式切换只替换材质/灯光/地面贴图，不清空 `model`、`lot`、`props` 等组。
2. 默认模式首帧沿用现有白模；精细模式在后台准备 tint/deferred 贴图和程序，完成一组材质就原地绑定，最后再打开阴影和高级效果。
3. 将 `renderer.compile()` 和 `renderer.initTexture()` 从“放行首帧前全部完成”改为小预算队列：每帧最多处理固定数量的纹理/材质，超过预算留给 `requestIdleCallback` 或下一帧。
4. 保留旧材质直到新材质 GPU 就绪；新资源失败时不清空旧场景，避免半成品帧和 UI 锁死。

### P1：缓存按资源身份长期复用

1. 模型缓存、材质缓存、地面合成缓存使用统一 `payload + render inputs` key，并在同一 Property 会话内保留默认与精细两套结果。
2. 不要在普通 render-mode 切换时 dispose 共享纹理；只在 payload、图集或 WebGL context 变化时释放。
3. 将地面 surface/normal 的解码结果从“合成缓存内部”提升为会话级 `ImageData`/ImageBitmap 缓存；合成只缓存输出纹理。
4. 高分辨率地面采用两级结果：先生成 mask×1/低分辨率 albedo，随后生成完整密度的 albedo/normal 并原地替换。

### P2：降低首载成本

1. GLB parse、PNG decode、Lot mask decode 分离到 Worker；只把最终可传输的几何/像素结果交回主线程。之前的 GLB Worker 实验出现过纹理乱码，需以带校验的二进制传输和回归样本重新验证，不能直接恢复旧实现。
2. 复用材质对象和 sampler 配置；避免每个 mesh 都新建相同的 `MeshStandardMaterial`。
3. 合并兼容材质/几何批次，减少 draw call。应优先统计 `renderer.info.render.calls`、`triangles`、`textures` 和 shader program 数量，再决定是否合批。
4. 建筑远处使用低 LOD 或简化材质；贴花和透明对象按屏幕面积阈值启用。

## “秒加载”应如何定义

不能把“所有精细效果准备完成”作为首帧条件。建议定义三个时间点：

| 时间点 | 内容 | 目标 |
|---|---|---|
| T0 | 旧场景或白模 + Lot 基础色 + 相机可操作 | 100–200 ms 内恢复输入 |
| T1 | 模型精细 albedo/tint、普通法线、主要地面纹理 | 500 ms 内渐进出现 |
| T2 | 阴影、完整 normal/AO、decal 投影、粒子和 DOF | 后台完成，不阻塞输入 |

如果没有旧场景可复用，T0 至少应该显示低分辨率占位地面和建筑包围盒；用户可以立即旋转、缩放和关闭窗口。

## 建议增加的测量

现有 `renderTelemetry` 只覆盖异步装配阶段，不能解释拖动时的 GPU/CPU 帧卡顿。建议在开发模式增加：

- `renderer.info.render.calls`, `triangles`, `points`, `lines`；
- 每帧 CPU `updateMatrixWorld`、raycast、overlay 投影耗时；
- GPU timer query（可用时）分别测 shadow pass、main pass、post/DOF；
- 纹理总像素、显存估算、shader program 数量和编译耗时；
- rebuild 中 clear/dispose、PNG decode、Worker compose、DataTexture 创建、initTexture、compile 各自耗时；
- 相机交互时的 p50/p95 frame time 和 pointer-to-present 延迟。

没有这些数据时，单纯把“面数限制到 5,000”不会解决问题，甚至可能把优化方向带偏。

## 优先实施顺序

1. 交互质量状态机（动态 DPR、阴影、normal、DOF、粒子和 decal 降级）。
2. 模式切换改为原地材质替换，默认场景立即保留，精细资源后台就绪后替换。
3. 取消首帧前的全量 `primeTextures + renderer.compile` 阻塞，改为预算化渐进预热。
4. 会话级双模式材质/地面缓存，模式切换不 dispose 共享资源。
5. 用 renderer.info 和 GPU/CPU 分阶段数据验证灯光、draw call、纹理与 shader 数量，再进行合批和 LOD。

## 风险与边界

- 阴影、透明贴花和精细地面关闭后，交互画面会暂时低质量；必须在停止操作后恢复，并提供质量状态提示。
- 原地替换材质需要严格的 generation token 和资源所有权，否则旧异步任务可能把过期材质绑定到新场景。
- WebGL 资源无法保证“立即释放”；应通过引用计数和 context 生命周期管理，避免为了省显存频繁 dispose/re-upload。
- 真实游戏材质的精细合成仍依赖 `lotSurfacePng`/normal atlas 可用；资源缺失应显示明确诊断，而不是把纯色回退误认为性能优化。
