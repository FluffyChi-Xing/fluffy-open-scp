# OpenSCP 能力增量与优化路线

本路线将已有 package、地图、坐标和运行时研究转成可落地的 OpenSCP 能力。设计原则是：离线分析先给出确定性结果，流程引擎只消费经过校验的中间表示，运行时桥接明确只读/主动能力边界。

## P0：坐标与边界安全校验

新增统一 `CoordinateAudit` 模型，至少包含：

- region size、visible area、PlayArea min/max；
- CityList 槽位位置和角度；
- path segment 端点、region/connection flags；
- unit world transform、lot corners、frontage/depth；
- 地形高度 tile 的 regionPos 映射。

导出前计算锚点差值、重复平移风险、PlayArea 越界、lot corner 越界和跨区域道路端点误差。报告必须区分“渲染可见”“城市可玩”“道路合法”“建筑合法”四种范围。

## P0：package 合法性、完整性和依赖闭包

在现有 DBPF 解析、overlay 写出和 SHA-256 基础上增加：

1. package manifest：源文件哈希、DBPF 版本、entry 数量、资源类型和修改原因；
2. overlay 决策记录：目标 key、父资源、优先级、覆盖前后摘要；
3. EcoGame 特殊检查：Property/Ecosystem group/version tag、Preload priority、模板/locale/RegionTerrain 依赖；
4. 完整性策略：写出后重新解析、重新计算哈希，并与预期 entry 集合比较；
5. 失败分级：结构损坏、引用缺失、覆盖冲突、语义不完整、运行时未验证。

流程引擎只接受通过这些检查的 package plan，并把检查结果作为可追踪节点输出。

## P1：地图、城市槽位和资源编辑器的依赖可视化

建立资源图而不是只展示单个 Property：

```text
RegionTerrain
  ├─ CityList → city position/angle/group/strata
  ├─ height/ED tiles → terrain sampling
  ├─ brush/water/visible fields
  └─ template/locale/registry
City box
  ├─ region/city state
  ├─ EcoPathSet
  ├─ unit/lot transforms
  └─ resources/bins/rules
```

地图词条、城市槽位、地图资源、边界字段编辑时显示直接依赖、间接依赖、可选依赖和未知依赖，并提供反向查询：修改某个 package entry 会影响哪些地图、槽位、模板和流程节点。

## P1：确定性 package 转译中间表示

为流程引擎提供稳定 IR：

- `PackageIdentity`：路径、SHA-256、DBPF 版本、资源计数；
- `ResourceKey`：type/group/instance/parent；
- `OverlayOperation`：replace/add/remove、优先级、冲突策略；
- `SemanticPatch`：地图槽位、PlayArea、道路端点、lot transform 等高层意图；
- `ValidationResult`：错误、警告、证据路径和运行时验证状态。

IR 必须可往返：package→IR→package 后，未修改资源的字节摘要和资源键集合应保持一致；语义修改必须能生成审计 diff。

## P1：运行时观察器改进

保留当前 UI bridge 的只读、版本绑定和自动撤回，同时增加：

- 请求/返回配对 ID、page/binding 生命周期和 thread 关联；
- 独立的 mutation、region、template、event、console 配额；
- dropped/unknown binding/decode error 的显式状态；
- JSONL、结构化摘要和原始证据哈希三层输出；
- 动作剧本模板：创建区域、认领城市、修路、消费、保存、重载。

当前 `observer_sync` 只同步 `ReceiveGameEventFromClient`，后续应支持把用户选择的 console/query/result 作为 `runtime.observation` 写入 store，同时保留“有界采样”标记，避免被误认为完整日志。

## P2：mod diagnostics 通道

分两层实现：

### UI/数据 mod

提供约定的日志前缀和 JSON envelope，例如 `openscp.mod.<modId>`，允许 UI script 报告请求、状态快照、资源键和阶段标记。OpenSCP 只接受页面 console 中可验证的来源/page/binding，并限制速率和大小。

### Native diagnostics

为未来受控插件预留 `NativeDiagnosticRecord`：group、level、message、thread、timestamp、build fingerprint、city/box/session。插件可以注册 EA::Trace reporter 或独立本地通道；OpenSCP 负责版本校验、权限、限流、撤回和哈希链。这个能力不应通过任意 package 字段触发，也不应复用现有页面 hook 作为 native 调用器。

## P2：模拟过程关联分析

把运行时记录与离线资源图关联：

- `eventID`/command/request → UI 资源或流程节点；
- city UID/box ID/region ID → RegionTerrain、城市盒和状态文件；
- resource ID/bin/rule → 对应 Property group、脚本和 locale；
- 记录前后 PlayArea、path、lot、资源值和存档文件变化。

输出“可观察链”而不是猜测链：缺少 native trace 时明确标记规则内部步骤未知；发生限流时给出丢弃数量和时间窗口。

## P2：测试与回归门禁

新增三组自动门禁：

1. **离线结构门禁**：DBPF round-trip、父继承、entry/hash、地图依赖闭包；
2. **语义安全门禁**：坐标锚点、PlayArea、道路端点、lot corners、区域/城市 flags；
3. **运行时证据门禁**：hook 指纹、绑定校验、自动撤回、trace JSONL schema、dropped/error 统计。

每个 mod 导出包携带 `validation.json`、`manifest.json`、`dependency-graph.json` 和 `evidence-index.json`，便于复现和回滚。

## 建议交付顺序

先实现 CoordinateAudit、package manifest/完整性和依赖图；再扩展地图/槽位/资源编辑器与确定性 IR；随后完善 UI bridge 的配对和动作剧本；最后再做 native diagnostics。这样即使没有 native 注入能力，OpenSCP 也能先提高 package 编辑和地图流程的安全性、可解释性与回滚能力。
