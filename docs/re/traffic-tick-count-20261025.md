# 交通 60 tick 只读计数结果

本轮使用 [traffic_tick_watch.py](../../probes/runtime/traffic_tick_watch.py) 对零售版
SimCity 进程做进程外 `ReadProcessMemory` 采样。探针没有注入 DLL、安装 hook、暂停线程、
调用游戏函数或写入游戏内存，达到 60 个模拟 tick 后自动结束。

## 运行身份

- EXE SHA-256：`2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`
- PID：`30104`（仅对本次进程有效）
- 游戏对象：`0x2A30AAD0`
- 活动车辆 handler：`0x2AD31078`
- 车辆 agent vtable：`0xD21458`
- 窗口：`simTick 55417 → 55476`，60 个 tick，0 个缺失 tick

## 量级

| 指标 | 60 tick 总量 | 每 tick 近似量 |
|---|---:|---:|
| 外部采样次数 | 965 | 16.1 |
| 节点观察次数 | 3,434,517 | 57,242 |
| 有效车辆 agent（单次快照） | 约 3,225–3,825 | — |
| 非零速度车辆峰值 | — | 1,212–1,861 |
| `agent+0x15C` 速度变化 | 1,442,030 | 24,034 |
| 新交易键 | 5,319 | 88.7 |
| 消失交易键 | 1,877 | 31.3 |
| 同交易键地址复用 | 626 | 10.4 |

这里的“节点观察次数”是车辆 hash map 在外部快照中被读到的次数；“速度变化”是相邻
外部快照的数值差异。它们是交通状态活动量级，不是引擎写出的日志条数。按本轮采样频率，
速度变化约为每次快照 1,494 次，不能直接解释为每个游戏 tick 内发生了 1,494 个唯一事件。

读取错误 1,245 次，主要来自游戏线程同时修改 hash map 时的节点/链表读取竞态；探针保留
错误计数并继续采样。`headerStable` 没有出现失败，但这只能说明头部前后相同，不能把整个
容器视为原子快照。

游戏对象的 `+0x264` transport-handler 向量同时包含三个 `cTransportVehicle` 对象；本轮
活动对象是 `0x2AD31078`，另外两个对象在窗口末尾的节点数为 0。源码默认
`mMaximumVehicles=6000` 仍是配置上限证据，不等于本次运行时已用尽 6000，也不能据此推导
行人、管网或垃圾代理池的上限。

原始结果：

- [summary.json](../../tmp/traffic-tick-60-20261025/summary.json)
- [samples.jsonl](../../tmp/traffic-tick-60-20261025/samples.jsonl)
- [status.json](../../tmp/traffic-tick-60-20261025/status.json)

