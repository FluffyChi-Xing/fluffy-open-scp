# 警车目标跟踪：第五阶段报告（2026-10-25）

## 结论

上一版把 `PoliceOutreachVan` 的 TransportEntry index 179 当成了全部警车，导致“警车为 0”的结论失效。运行时 `cEcoTransportEntry.mID` 是对象名称的小写 FNV1 哈希；按同一规则闭合后，当前进程中的警车候选为：

| 车辆名称 | mID | TransportEntry index |
|---|---:|---:|
| `PatrolCar` | `0x5788808B` | 50 |
| `PoliceDetectiveVan` | `0x78451C57` | 52 |
| `PoliceOutreachVan` | `0x80E8C885` | 179 |
| `FrenchPatrolCar` | `0xB67B1F5B` | 424 |

用四个 index 同时过滤后，在 2026-10-25 的 60 tick 观察窗口（simTick 68258–68317）实际收到 57 个快照。活动样本全部来自 `PatrolCar` index 50：每个观察 tick 有 21–33 个有效 Agent，均值 24.93；其中非零速度 Agent 为 7–22 个，均值 11.96。窗口累计记录 1,421 个 Agent 观察、703 次速度变化，以及原始路径字段 251 次变化和原始停止字段 251 次变化；后两项是字段变化计数，不能直接等同于“重新寻路”或“状态切换”事件。

这与用户观察的“警车事件持续发生”一致，也说明旧样本不是游戏没有警车，而是目标索引选错。当前窗口没有看到 index 52、179、424 的活动样本；这只表示这些车型在该窗口没有被观测到，不能推导它们在城市中不存在。

## 警车身份闭合

静态源码给出了两条可复核关系：

1. GB::cEcoGameScripts::TransportEntryFromIndex 通过 deque iterator 按整数索引返回 cEcoTransportEntry。
2. GB::cEcoTransportEntry::FillFromProps 将 mResourceBins 填入 TransportEntry；运行时每个资源桶元素为 12 字节，首字段是 resource index。

本进程的 cEcoGameScripts 为 0x2EB6C358：

| 项目 | 运行时值 |
|---|---:|
| ResourceEntry vector | 0x2F774C28–0x2F781CA8 |
| ResourceEntry 数量 | 6672 |
| TransportEntry deque 元素数 | 437 |
| TransportEntry 元素步长 | 0xD4 |

已知警车资源 ID 与当前索引表的映射为：

| 资源 | 十进制 ID | resource index |
|---|---:|---:|
| PoliceCarR1 | 1253104808 | 1876 |
| PoliceDetectiveVanR2 | 4169623735 | 4465 |
| PoliceOutreachVan | 2162739333 | 3257 |
| PoliceOutreachVanR2 | 3960689125 | 1726 |

当前脚本表中只有一条 TransportEntry 的资源桶命中这些“警车服务资源”：

- transportEntryIndex = 179
- entry 地址：0x39A8230C
- entry ID：0x80E8C885（与 PoliceOutreachVan 资源 ID 相同）
- resource index：3257
- resource amount/capacity：0/1

资源桶是运输/服务资源，不是完整的车辆模型目录；因此不能用“资源桶只命中 index 179”排除 `PatrolCar`、`PoliceDetectiveVan` 或 `FrenchPatrolCar`。四类车型最终通过 `mID = FNV1(lowercase(name))` 与运行时 entry ID 闭合。

这些身份和地址只对 PID 30104 的当前进程生命周期有效。原始资源桶闭合结果见 [police-transport-inventory-20261025.json](../../tmp/police-transport-inventory-20261025.json)；名称哈希与四类候选的校验记录见 [police-transport-id-map-20261025.json](../../tmp/police-transport-id-map-20261025.json)。

## 运行时采样

### 警车专用窗口（修正后）

命令使用三个已验证车辆 handler，并同时过滤 `50, 52, 179, 424`。结果：

- simTick 68258 → 68317；观察跨度 60 tick
- 实际快照 57 个，缺失 3 个 tick（外部轮询没有在墙钟窗口内命中）
- `PatrolCar` index 50：1,421 个 Agent 观察，21–33 个/快照，均值 24.93
- 非零速度：7–22 个/快照，均值 11.96
- `PoliceDetectiveVan`、`PoliceOutreachVan`、`FrenchPatrolCar`：本窗口 0 个观察
- 速度变化：703；原始路径字段变化：251；原始停止字段变化：251（不直接等同于游戏事件）
- 读取错误：204；这是进程外 hash-map/transaction 快照竞态，不能按事件计数

原始结果：

- [summary.json](../../tmp/traffic-police-corrected-20261025/summary.json)
- [samples.jsonl](../../tmp/traffic-police-corrected-20261025/samples.jsonl)
- [status.json](../../tmp/traffic-police-corrected-20261025/status.json)

旧的 index 179 样本仍保留在 [traffic-police-filtered-phase5-20261025](../../tmp/traffic-police-filtered-phase5-20261025/) 中，但只能解释 Outreach Van 这一类，不能作为全警车样本。

### 全车辆对照

同一进程的另一段 60 tick 全车辆窗口为 simTick 58227 → 58286：

| 指标 | 结果 |
|---|---:|
| 有效 Agent 观察 | 87,255 |
| 每次快照平均有效 Agent | 1,454.25 |
| 单次快照有效 Agent 峰值 | 3,463 |
| 非零速度峰值 | 929 |
| 速度变化 | 23,769 |
| 新 transaction key | 3,900 |
| 交易键地址复用 | 337 |
| 读取错误 | 25 |

其中 transportEntryIndex = -1 的观察为 5,333 次，占约 6.1%。这些 Agent 的 transaction vector 在对应外部快照中没有成功闭合；修正后的警车窗口仍有 204 次读取错误，因此结果是有效观察的下界。

原始对照结果见 [traffic-tick-count-20261025.md](traffic-tick-count-20261025.md) 及 [summary.json](../../tmp/traffic-police-phase5-20261025/summary.json)。

## 能看到什么，不能看到什么

探针是进程外 ReadProcessMemory 只读快照：

- 不注入 DLL；
- 不安装 hook；
- 不暂停线程；
- 不调用游戏函数；
- 不写入游戏内存。

它读取：

- cVehicleAgent + 0x15C：当前速度；
- cVehicleAgent + 0x180：路径重算剩余时间；
- cVehicleAgent + 0x184：停止时间；
- transaction vector + 0x34：TransportEntry index；
- hash-map 节点地址、transaction key 和车辆 vtable。

这些是可复核的交通状态观察量，不是游戏内部日志记录。当前没有证据表明零售版会为每个警车 Agent 写出独立、可直接订阅的路径日志；若要得到“从 A 到 B 的逐段路径事件”，仍需继续闭合位置/道路段字段或做受控交通事件实验。

探针已增加 --transport-entry-index INDEX 以及路径/停止字段的变化计数，见 [traffic_tick_watch.py](../../probes/runtime/traffic_tick_watch.py)。

## 对对象池和数量上限的含义

源码中的 mMaximumVehicles = 6000 只能作为车辆配置上限证据。它不是当前运行时已分配数量，也不是警车专用上限；旧的 index 179 零样本不能用来推断“警车池为空”或“全城代理池已经耗尽”。

当前能确认的是：

1. 车辆 Agent 通过 transaction key 进入 handler hash map；
2. TransportEntry index 可以把 Agent 关联到资源类别；
3. Agent 地址会发生复用，说明至少存在释放/复用行为；
4. 交通状态规模达到每 tick 约千级有效 Agent、单次快照可达三千级；
5. `PatrolCar` 的可识别 TransportEntry index 50 在本窗口持续有活动样本；其他三类车型未在该窗口出现。

因此，当前可以把“约 25 辆巡逻车处于道路 Agent 活动集合”作为运行时观测量，但不能把它直接当成城市警车总库存。停在车库、尚未派出、正在服务系统内部等待池中的车辆，可能不出现在这个车辆 hash map；要验证用户估计的 80 辆总量，还需要同时读取警察局单位资源桶/停车位和出车等待池。

## 下一步高价值实验

要把警车从“身份闭合”推进到“行为跟踪”，最有价值的是在城市出现一次可控警务需求后重采样：

1. 触发犯罪/警察出勤，分别观察 TransportEntry 50、52、179、424；
2. 记录 transaction key、Agent 地址、速度、+0x180/+0x184 和位置候选字段；
3. 以 1 tick 或更密的间隔观察 50/52/179/424 与 -1 之间的变化，区分任务结束、对象复用和读取竞态；
4. 将警车 Agent 的路径字段与道路/PathEntry 段表对拍；
5. 在车辆接近 6000 配置上限前后比较新 transaction、地址复用和失败计数，才有资格讨论池上限是否会造成运输资源短缺。

本报告中的所有运行时地址、PID 和快照均限定在 2026-10-25 的当前零售进程，不能跨重启复用。

