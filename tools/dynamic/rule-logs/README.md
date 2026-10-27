# 定向规则日志采集

需要 Python、Frida，以及受支持零售游戏。规则日志模式附加已进入城市的现有 PID；来源模式也支持显式冷启动（见下文）。采集器不关闭游戏、不主动调用游戏函数。Frida Interceptor 会临时改写日志入口指令，结束时卸载；并非零开销观察。

```powershell
python tools/dynamic/rule-logs/capture.py --pid <当前游戏PID> --scope global --state-index 318 --seconds 10 --out tmp/my-global-capture
python tools/dynamic/rule-logs/capture.py --pid <当前游戏PID> --scope unit --unit-slot 827 --state-index 135 --seconds 10 --out tmp/my-unit-capture
```

示例索引来自测试城市，其他城市不保证命中。省略索引时，每个 scope 固定选择第一个匹配的 game/slot/stateIndex，后续只采集同一目标。`both` 同时选择两类目标。限制：1–120 秒、最多 1–2000 条（默认 250）、队列 256 条。输出目录必须不存在。

规则模式 `unit/global/both` 现在也支持 `--all-targets`，取消首个目标锁定，但仍保留显式过滤和记录上限。它用于短时发现目标，不代表完整城市日志；达到上限后的命中计入 dropped。2026-10-22 的 30 秒实测命中 25020 次，保存 1000 条、丢弃 24020 条，不能将这些全城命中归为某所学校。`targets` 在该模式仅保留首个匹配目标，完整身份以各行记录为准。

先读取目标进程实际模块路径并校验完整 SHA-256（仅支持 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`），再校验全部入口的内存字节，然后安装 hook。异常、手动中断与正常结束均进入清理流程。主机错误或字段读取错误使退出码非零；零条记录不能证明游戏没有执行规则。

在 OpenSCP 诊断工作台导入 `trace.jsonl`，再导入同目录 `status.json` 查看采集质量。`hits` 是入口总命中，`filtered` 是主动筛除，`dropped` 是超过容量/队列限制；三者不能混淆。字段读取失败保留部分记录和 `readErrors`。

采集类别按 native 证据分层：`simulation.rule` 是规则入口，`simulation.effect` 是资源 bin 更新，`simulation.condition` 是可执行条件，`provenance` 是资源来源。资源消费应使用 `unit-consumption`，生产/消费资源可进一步用 `--resource-id 0x...` 定向；规则执行可用 `--scope unit`/`global` 加 `--rule-id 0x...`。这两种过滤会在 hook 内尽早丢弃无关目标，避免把全量事件送入工作台。

当前没有已验证的零售版交通模拟入口，也没有可靠字段把 unit 标记为建筑。因此不能提供名义上的“交通日志”或自动“建筑日志”类别；`simulation.rule` 的 unit 记录只有在用户用已知 rule ID/slot 定向后才能作为建筑规则候选，报告中必须保留该语义限制。渲染层 trafficTicks、draw/bind 事件不属于交通寻路证据。

`simulation.rule` schema 1 的 `log-entry` 表示进入 LogGlobalRule / LogUnitRule，包含 tick、状态索引、规则索引和执行计数。它不证明 delta 已写入或所有规则尝试都被覆盖。单位 slot 不是完整身份：stamp 未知，保留 null；脚本名称和输入输出 bin 仍未知。跨城市、加载存档后应重新采集，不复用旧指针。

运行验证见 `docs/re/evidence/rule-logging-runtime-20261017.json`。采集器目前是命令行工具；工作台负责导入分析，尚无一键原生日志触发按钮。

新增身份字段：`mapping.ruleIdHex`、`baseRuleWithinSet` 和 `resourceKey`。后者是运行态抽象资源（AE9），不是磁盘编译文件（AEB）或已验证的 package 来源；UI 显示为 `abstractResourceTgi`。单位特化可能使当前索引与基础索引不同。身份核对证据见 `docs/re/rule-resource-identity-20261017.md`。

离线工具 `resolve_ids.py <导出的AEB文件> 0x规则ID ...` 可在已验证的 5/2/14、无名称布局中查找全部候选记录；拒绝其他布局、截断和尾部残留。它不恢复名称，也不证明包在游戏中胜出。

全局日志新增 `ruleDefinition`：基础资源操作（最多 16 项）、fromBinOps 引用与应用次数约束。离线工具对 global 候选输出同形定义，已与实机 59 条记录逐项对拍。`input/output` 是规则配置，不是实测资源变化；负 output 为范围条件，fromBinOps 可动态替换数量。单位定义暂未展开。详见 `docs/re/rule-definition-fields-20261017.md`。

## 资源更新前后快照

`--resource-id` 筛选包含该资源的整条更新，保留所有配套操作及原顺序，以正确对应原生 bin lookup；输出可能包含其他资源。2026-10-23 之前的实现会提前删除非目标操作，多操作规则可能出现 `extra_lookup` 或错配。见 [垃圾观测与修复](../../../docs/re/garbage-observation-20261023.md)。

`--scope effects --rule-id 0xc2181f55` 定向捕获全局 RunRule 内实际资源更新及其 bin lookup，记录操作前后 amount/capacity、实测差值和原函数返回值。省略 rule-id 则固定首个符合条件的 game/rule。最多 16 项操作，超限主动筛除；不接受 state-index、unit-slot 或 launch-exe。

```powershell
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope effects --rule-id 0xc2181f55 --seconds 15 --out tmp/effects
python tools/dynamic/rule-logs/verify_effects.py tmp/effects
```

返回失败也可能已部分写入；未观察到不等于零变化。工作台导入后以表格展示快照并支持资源 ID 检索高亮。当前没有 UI 一键启动采集。实机 7 次 +1、无读取错误/丢失，证据和限制见 `docs/re/rule-resource-effects-20261017.md`。

## 单位本地资源快照

`--scope unit-effects` 观察 Unit.RunRule 内的本单位资源更新，可用 `--unit-slot N`、`--rule-id 0x...` 定向。省略过滤时固定首个匹配 game/slot/pointer/rule；不接受 state-index 或 launch-exe。仅接受本单位调用点和 bins，排除全局、区域及交易路径。stamp 未知，不能跨会话复用身份。

```powershell
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope unit-effects --seconds 15 --out tmp/unit-effects
python tools/dynamic/rule-logs/verify_effects.py tmp/unit-effects
```

15 秒实测捕获一次资源 586→864（+278），错误/丢失/未完成均为零。高频 hook 有开销，建议短窗口定向采集；详细证据与限制见 `docs/re/unit-resource-effects-20261017.md`。

`--scope unit-consumption` 专门跳过 `input=0` 的生产型规则，接受首个单位本地 `input > 0` 的资源操作，用于观察消费/投入资源。它仍只记录一个目标，不能代表所有建筑或交通代理。

```powershell
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope unit-consumption --seconds 15 --out tmp/unit-consumption
```

若要观察多个单位和规则，可加 `--all-targets`；仍受 `--max-records` 上限约束，建议短窗口使用。实机 5 秒、上限 100 的对拍捕获了 57 个不同单位指针、80 个单位/规则组合；同时因高频调用触达上限而丢弃 13,873 个后续候选，因此这不是全量日志。

## 全局条件检查

`--scope conditions` 观察首个匹配 game 的全局 CanRunRule，`--rule-id` 可进一步限定规则。记录控制条件、原生允许结果、资源允许次数、实际资源槽值和有证据的阻止原因。不接受 state-index/unit-slot，也不冷启动。

```powershell
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope conditions --seconds 10 --max-records 1000 --out tmp/conditions
python tools/dynamic/rule-logs/verify_conditions.py tmp/conditions
```

允许通过不代表已执行；动态路径缺少证据时保留原因未知。最多展开 16 项操作。实机 410 条检查及详细边界见 `docs/re/rule-eligibility-20261017.md`。

## 资源来源采集

`--scope provenance` 监听规则资源的 ResourceManager 请求、缓存检查、数据库 OpenRecord 和工厂 CreateResource，按线程内嵌套请求及实际 Record/Resource 指针关联。`created-from-record` 仅在最终返回对象与成功创建对象一致时出现；若被缓存对象替代，则输出 `cache-hit-origin-unknown`，不沿用新建候选的包路径。非 PackedFile 数据库保留指针和 vtable，不猜路径。此模式不支持单位/状态索引过滤。

```powershell
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope provenance --seconds 20 --out tmp/provenance-attach
# 先自行保存并退出游戏；以下显式启动指定 EXE，恢复运行前安装探针。
python tools/dynamic/rule-logs/capture.py --launch-exe 'D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCity/SimCity.exe' --scope provenance --seconds 120 --max-records 2000 --out tmp/provenance-startup
```

若已有 SimCity 进程，启动模式拒绝执行。成功或失败清理后均不终止游戏；若初始化失败，尝试卸载探针、detach 并恢复新进程。`incompleteRequests` 记录采集结束时尚未返回的目标请求，不能将截断请求算成闭环。零记录只代表观察窗口内没有捕获到目标加载，不能补录探针安装前的缓存来源。

使用 `python tools/dynamic/rule-logs/verify_provenance.py <采集目录>` 独立复核来源链。校验器要求同请求的成功 Record → Create → 最终返回事件完整且指针、键和数据库一致；探针自身的 `created-from-record` 标签不能替代完整日志证据。

2026-10-17 进城实测曾因定时发送来不及执行而丢失 186 条事件，仍保留 21 条可验证完整链。来源探针现改为 256 条满队列立即发送，保留总数硬上限；已通过同步突发模拟测试，尚未重新实机冷启动复测。详见 `docs/re/rule-resource-provenance-20261017.md`。规则日志探针的队列策略未在本次修改。
## 2026-10-23 采集器并发事故修复

规则采集与量级/UnitEntry/渲染采集共用 `probes/runtime/capture_lock.py` 的 Windows 命名互斥量，在 attach 前拒绝第二个宿主，锁覆盖卸载与 detach。不要并行启动多个采集进程；其他未迁移的注入工具仍须手动串行。入口 `entry_mismatch` 只阻止业务 hook 安装，不能阻止此前已发生的 Frida agent 加载。详见 [崩溃取证](../../../docs/re/simcity-frida-crash-20261023.md)。
