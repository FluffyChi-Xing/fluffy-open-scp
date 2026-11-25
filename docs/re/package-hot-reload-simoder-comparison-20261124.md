# Package 热重载对比与首个实机验收

## Simoder 已能验证什么

本地检出的 `main` 为 `b2e5ae4`，并检查了 `v0.2.0-alpha.1`。本次对照的范围是随源码和 tag 可复查的实现。用户提到的实机演示仍缺对应 commit、MOD 样本或日志，无法判定演示覆盖的是以下哪一种重载。

| 路径 | 源码或记录所示结果 |
|---|---|
| Simoder MOD 管理器重载 | `ModManager::reload` 校验待应用指纹、替换 PROP patch registry，再通知受影响的 TGI。可选的 `with_refresh_callback` 在本地源码未见生产调用者。回调失败会记录“registry 已提交，live 刷新失败”。 |
| 实机标量 PROP 取证 | Sanitizer 的运行期记录记录了成本字段 300→345，选择器和实际模拟总值为 545，已放置建筑详情仍为 500。它证明特定既有 PROP 值能变动，并明确了 UI 数据副本可能滞后的情况。 |
| Simoder-Base 热重载 | 监视并交换 Luau 源和 UI applicator。集成测试覆盖成功发布及损坏输入保留旧版本。 |
| Simoder DBPF overlay / raw-resources | overlay builder 生成经检查的 DBPF 字节，在检出的源码中由测试调用。schema 3 可校验原始资源清单，发现路径会报告 `resource.register/resource.replace` 未开启。当前快照没有可直接复用的 DBPF 重挂和缓存刷新的实机调用链。 |

证据：[Manager](../../../Simoder/crates/simoder-mods/src/manager/mod.rs)、[Sanitizer 记录](../../../Simoder/docs/resource-loading.md)、[Base 热重载测试](../../../Simoder/crates/simoder-d3d9/tests/ui_patching_and_base_mods.rs)、[overlay builder](../../../Simoder/crates/simoder-mods/src/overlay.rs)、[raw 资源发现](../../../Simoder/crates/simoder-mods/src/manager/discovery.rs)。

## Base 和 MCP 本地源码复查

- Base `6f98f956` 与 Simoder CI 固定版本相同。截图的主菜单入口对应 `mods-trigger-main`，区域页有单独的入口。宿主监视 `scripts/**/*.luau` 和 `ui/applicator.js`，候选通过后替换 Base VM 和 applicator，经同步包发送 `reloadGen`。EAWebKit 按代际、patch revision 和当前视图更新 DOM。这里的热重载输入是脚本和视图描述。[主菜单入口](../../../Simoder-Base/scripts/services/mods-ui.luau#L709)、[DOM 应用](../../../Simoder-Base/ui/applicator.js#L1545)、[宿主监视器](../../../Simoder/crates/simoder-d3d9/src/base_hot_reload.rs#L57)
- MCP `591eeabb` 的会话路由可观察 UI 的 `reloadGen`，可借鉴作为真实消费者回执。工具目录对启停和 debug-tools overlay 有描述，但此快照的 live backend 将两者转给 fallback，启停实现返回 `synthetic-request`。目录描述和 DLL 显示激活都不足以确认真实游戏数据库发生了变更。这两个新检出均未发现可直接接入的 DBPF 挂载、重读和卸载调用链。[UI 观测](../../../Simoder-MCP/crates/simoder-mcp/src/host/session_router.js#L883)、[live backend](../../../Simoder-MCP/crates/simoder-mcp/src/host/live.rs#L208)、[fallback](../../../Simoder-MCP/crates/simoder-mcp/src/host/backend.rs#L340)

## OpenSCP 可先走通的范围

既有方案直接验证引擎数据库命中和资源重读，目标是迭代我们自己生成的 DBPF。当前仓库已有零售入口的静态证据和离线差量 CLI。首版选一个已通过冷启动验收的独立测试菜单项，只改一处指向既有本地化字符串的 Menu2 标题或描述引用。A/B 共享同一套 TGI、菜单身份和 Unit 索引，不改写已装包。既有库在打开时直接再次调用 `Open` 已有拒绝证据，库命中也不能替代缓存和消费者刷新验证。参见 [零售版证据和试验限制](package-hot-reload-retail-20261115.md)。

本轮在 `package_reload_prepare` 输出中增加解码 payload 的 `baselinePayloadSha256` 与 `candidatePayloadSha256`。全包 SHA-256 用于定位构建产物和部署身份。payload SHA-256 用于确认命中的指定资源确实是本轮内容。overlay 的 SHA-256 另外记录。原有 TGI 和属性限制继续执行。

| 同 PID 的成功证据 | 验收条件 |
|---|---|
| 候选身份 | A/B 来源由 build ledger 和 MANIFEST 核对，实际文件分别验证 SHA-256，正反向差量记录目标 TGI 与 payload 摘要。 |
| 引擎命中 | 观察活体查找命中了预期库。分别记录重载前目标资源内容与候选解码内容。 |
| 对象刷新 | 单独记录资源重读返回值和目标对象的已更新属性。仅收到通知不足以判定刷新成功。 |
| 实际生效与回切 | 游戏界面显示 A→B→A，并核对其他菜单不受影响。重复试验需确认无重复库记录和悬空引用。 |

第一项可离线执行。第二项开始前需要确认同版本零售进程的实际调度线程、读取静止条件以及临时库所有权。用户完成存档备份并启动游戏，在一次性城市或已确认可观测的隔离场景进行。会话中保持 SimCityData 的测试包集合固定，按现有归档脚本和台账做冷启动准备。第一条闭环完成后，再按资源类型增加消费者验证，优先挑能反复测试且容易观察的一类。模型、材质、夜灯、统计柱、现有建筑实例及新菜单的动态注册各需独立证据。

Simoder 可借用的是候选指纹、显式提交、受影响 TGI 集合以及失败保留上一代的做法。OpenSCP 需要额外区分“库已登记”“资源已命中”“对象已重读”“游戏内已观察”。如果拿到用户所指的 DBPF 实机演示对应版本和样本，优先对照它的挂载来源、命中优先级、重读范围和回切行为，再决定是否复用该路径。

## 本次验证

运行 `cargo test --offline --locked -p probes --example package_reload_prepare`。合成包覆盖正反向差量、摘要与解码内容一致、拒绝覆盖输出及拒绝额外资源变化。未启动或附加游戏，未部署包。SDK 待下一次合并 master 后从 master 切分支，UI 编辑器移植搁置。
