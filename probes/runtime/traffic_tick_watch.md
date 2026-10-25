# 交通 tick 计数探针

`traffic_tick_watch.py` 是一份有界的进程外只读探针。它使用
`ReadProcessMemory`，不注入 DLL、不安装 Frida hook、不暂停线程、不调用游戏函数，
并在达到 tick 跨度或墙钟上限后自动结束。

探针只接受已经确认的当前进程地址：

- `--game`：游戏对象地址；也可以使用已有 `trace.jsonl` 的唯一 `game` 字段作为 `--seed`。
- `--vehicle-manager`：当前进程的 `cTransportVehicle` handler 地址；参数可重复以覆盖多个 handler。堆地址不能跨进程或重启复用。
- `--transport-entry-index`：可重复；只把 transaction 中 `+0x34` 等于指定 TransportEntry 索引的 agent 纳入聚合。该索引必须先由同一进程的脚本表闭合，不能跨进程复用。当前进程的警车候选是 `PatrolCar=50`、`PoliceDetectiveVan=52`、`PoliceOutreachVan=179`、`FrenchPatrolCar=424`；这些数字不能跨进程重用。
- `--per-agent`：额外写出 `agent-samples.jsonl`，保留每个交易 key 的逐 tick 细节。该模式会读取 Agent 的细节窗口和 transaction 细节，适合观察 sink/路径切换；普通聚合模式不会增加这些读取。
- `--trigger-sink`：进入事件等待模式；发现任意 Agent 的**当前 sink**等于该值后记录触发 tick，并继续观察 `--post-trigger-ticks`。必须与 `--per-agent` 一起使用。未触发时运行到 `--seconds` 上限。
- `--post-trigger-ticks`：触发后继续保留逐车上下文的 tick 数，默认 30。
- `--vehicle-vtable`：默认是模块基址加 `0x921458`，运行时会用 EXE 指纹重新验证。

示例（把地址替换为当前 PID 的值）：

```powershell
python probes/runtime/traffic_tick_watch.py `
  --pid 7424 `
  --game 0x12345678 `
  --vehicle-manager 0x2AD31078 `
  --vehicle-manager 0x2AD32910 `
  --vehicle-manager 0x2AD33000 `
  --transport-entry-index 50 `
  --transport-entry-index 52 `
  --transport-entry-index 179 `
  --transport-entry-index 424 `
  --per-agent `
  --out tmp/traffic-tick-watch-01 `
  --ticks 60
```

等待一次真实出警目标的示例。`323` 是当前进程脚本表中已闭合到
`CrimeInProgress` 的 sink；命令会持续观察，直到某辆已过滤的警车实际选中该
sink，随后再抓 30 tick：

```powershell
python probes/runtime/traffic_tick_watch.py `
  --pid 30104 `
  --game 0x2a30aad0 `
  --vehicle-manager 0x2ad31078 `
  --vehicle-manager 0x2ad32910 `
  --vehicle-manager 0x2ad33000 `
  --transport-entry-index 50 `
  --transport-entry-index 52 `
  --transport-entry-index 179 `
  --transport-entry-index 424 `
  --per-agent `
  --trigger-sink 323 `
  --post-trigger-ticks 30 `
  --seconds 900 `
  --interval-ms 50 `
  --out tmp/traffic-police-event-watch
```

`status.json` 的 `trigger` 字段会记录首个触发 tick、Agent key、Agent 地址和当时的
候选列表；`summary.json` 也会复制该字段。若只看到候选列表包含 323 而 `trigger`
为空，说明窗口仍未捕获到实际出警选择。

输出文件：

- `status.json`：PID、模块指纹、窗口边界、停止原因和读取错误。
- `samples.jsonl`：每个已观察 `simTick` 一行，包含所有配置 handler 的合计节点观察次数、有效车辆数、速度变化、路径重算剩余时间变化、停止时间变化、出现/消失和地址复用计数。
- `agent-samples.jsonl`（仅 `--per-agent`）：每个已观察 `simTick` 一行，包含逐 Agent 的 `key`、地址、速度、TransportEntry、transaction 的当前 `sinkEntryIndex`、完整 `sinkEntryIndices` 候选列表、`groupIndex/position/velocity`、4 个 `pathSegments` 和 Agent 字段候选值。`sinkEntryIndices` 来自 `cTransactionInfo` 的 EASTL fixed vector，最多 4 项；它表示该交易当前保留的目标候选，当前 sink 是其中被选中的一项。`cAgentCandidates` 保留 `_candidate` 后缀，避免把尚未完成零售字段闭合的近邻槽位误当成确定语义。
- `summary.json`：60 tick 跨度内的总量、均值、缺失 tick 和限制说明。

探针在同一 `simTick` 内的重复外部轮询会被丢弃，因此 `samples` 是 tick 行数；
`handlerSnapshots` 记录该行实际读取了多少个 handler。`nodeObservations` 是“采样看到的节点次数”；`speedChangeCount` 是相邻外部快照之间
`agent+0x15C` 数值变化的次数；路径和停止字段分别来自 `agent+0x180` 与 `agent+0x184`。它们是可复核的交通状态观察量，不是引擎写出的日志事件，
也不代表一次原子全城快照。hash map 在游戏线程更新时可能处于过渡状态，探针会记录
`headerStable=false` 和读取错误，不会把不稳定快照伪装成完整计数。

当前零售布局中，hash 节点的 `node+4` 是内嵌 `cVehicleAgent` 的起始位置，
该位置的首字为车辆 vtable；探针不会把它当成二级指针解引用。

当前实现只覆盖显式传入且已验证的车辆 handler。行人、管道、水/电/污水、垃圾和信号网络需要各自
完成零售对象地址与布局校验后再添加，不能用车辆节点数量替代它们。

逐 Agent 模式的布局依据：`cAgent` 的候选 sink/unit/destination 槽位来自提取的 `cAgent` 布局，
`+0x88` 是 `cTransportAudioInstance.mPosition` 候选，`+0x9C` 起是四个 `cPath` 的
`mSegmentAndDir`，transaction 的 `+0x30/+0x38/+0x3C/+0x48/+0x54/+0x58` 分别对应
`mSinkEntryIndex/mGroupIndex/mPosition/mVelocity/mSpeed/mAge`。这些读取仍是非原子快照；
只有在同一进程中通过受控事件对拍后，才可把 sink/path 转换成“出警”或“押送”标签。
