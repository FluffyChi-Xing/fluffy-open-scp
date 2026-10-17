# 模拟诊断第二阶段：索引语义、tick 与采集质量

本轮复用本地开发版反编译源码和此前真实零售采集，不重新启动游戏、不修改存档，不把开发版地址用于零售 hook。当前会话没有可用 REA MCP；CLI doctor 因本地 npm npx 缓存缺失 package.json 失败。本轮证据来自文件读取和本地解析器交叉检查，不宣称取得新的 REA/IDA 反编译结果。

## 新确认的语义

开发版源码根目录：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree`。本轮核对文件的 SHA-256 记录在 [证据摘要](evidence/diagnostics-stage2-20261017.json)。

1. `GB/cEcoBinSet/ApplyRule.c:23` 先取 `mRuleIndices[stateIndex]`，再调用 `GlobalRuleFromIndex`。`stateIndex` 是运行态索引，不是脚本规则 ID。
2. `GB/cEcoGameScripts/GlobalRuleFromIndex.c:19` 把 ruleIndex 拆成 `ruleIndex >> 20`（规则集索引）和 `ruleIndex & 0xFFFFF`（集内索引）。这不是 hash/TGI；解析规则名称还要核对当前加载的规则集和数据存储。
3. `cEcoBinSet::ApplyRule` 只有通过 CanRunRule 且 RunRule 返回真时才调用 LogGlobalRule；OnRuleSuccess 则位于 CanRunRule 的真分支，不以 RunRule 的返回真为前提。因此“日志次数”“尝试次数”和“成功回调次数”不能直接视为相同指标。
4. `GB/cEcoGame/LogGlobalRule.c` 在必要时写 tick 差，再写 stateIndex 和 applyCount。`LogUnitRule.c` 写 unitSlot、stateIndex、applyCount、maxApply、maxPossibleMapApply 及 selectedUnits；不是任意 bin 的前后值快照，也没有直接写入完整 unitRef 的 stamp。
5. `GB/cEcoUnit/ApplyRule.c:24` 同样经过本单位的 mRuleIndices。不同单位的相同 stateIndex 不能直接归并为同一脚本规则。
6. `anonymous/cEcoGameUIEventHandler/UnitOnRuleSuccess.c` 和 `UnitOnRuleFail.c` 都会针对类型 0x84 调用 PostRuleUIEvent，只是来自不同事件列表。PostRuleUIEvent 使用单位消息 199450092 或全局消息 248287250。既有真实零售 trace 中两种消息均存在；但其 payload 没有通用 success/fail 标志。

这里的函数语义来自开发版反编译文件。PostRuleUIEvent 存在明显的寄存器/参数还原伪影，不能把反编译中可疑指针表达式当作零售字段偏移。规则索引及运行态布局仍需要零售版函数、对象和当前加载脚本的验证。

## 已进入 OpenSCP 的能力

“运行时日志 → 诊断会话”新增：

- 可导入 **已有副本** `SLDelta*.mfs`，仅接受已观察的 v4 外层格式；保留源 SHA-256、字节 offset/size、精确的 uint64 delta ID、sim_ticks、kind 和 payload 长度。内部载荷不解码、不套用 action 名称。未知版本、倒退 ID、错误长度和截断文件整体拒绝，保留此前打开的会话。
- “模拟观察”同时包含单位/全局 UI 事件和 delta 外层记录。UI 事件无游戏 tick，delta 外层无 UI 请求关联键；不按时间拼出伪因果链。
- 可关联所选会话的 `status.json`，展示报告记录数、丢弃、解码错误、探针错误、主机错误、detach 及各通道 hits/accepted/dropped/queued。缺失值为未知；顶层 dropped 不与通道 dropped 重复相加。
- 记录数不一致会告警；一致也不是来源同一性的证明。该关联由用户选择，目前未做 trace 哈希身份绑定。主机 marker 不纳入采集记录数对比。
- 导出 JSONL 带版本化 `openscp.session` 元数据头，保留关联状态；重新导入可恢复。MFS 导出保存的是观察字段，不是原始二进制存档；源文件不被修改。

## 实际交叉验证

产品 TypeScript 解析器与既有 Python `delta_container.py` 对两份真实副本的记录数、首条字段、tick 范围和文件 SHA-256 检查一致：

| 文件 | 记录数 | sim_ticks 范围 |
|---|---:|---:|
| delta-live-20261013-SLDelta8_1.mfs | 45 | 109790–109790 |
| delta-live-20261013-SLDelta13_1.mfs | 4265 | 0–0 |

后者有 4265 条但 tick 全零，说明不能直接把外层记录数量当作模拟推进次数，也不能用 delta ID 代替游戏 tick。这里不推测全零的原因。

质量适配器读取三个真实状态文件：完整 UI 142566 条 / dropped 0；旧道路建造窗口 16139 条 / dropped 147557；引擎 384 条 / dropped 0。三者报告记录数均与 JSONL 行数一致，但各自覆盖范围仍取决于采集入口和采样设置。

## 接下来真正需要补的证据

首要任务仍是零售 **ApplyRule / LogGlobalRule / LogUnitRule** 函数及对象布局定位，验证同次采集中的 `tick + unit slot/stamp + stateIndex → ruleIndex → 当前 rule set`。在此之前不提供未经核对的 native 调用或参数偏移，不把 XOR 状态快照中看似合法的 action header 归为规则执行。获得上述映射后，再选择一个规则和少量输入/输出 bin 进行定向前后观察。
