# 新样本交通 handler 与车辆规则类型闭合（2026-10-30）

## 结论

此前资料中的“约 12 种车辆规则类型”现在可以精确化：它指的是 `SC::cGameAppMode::Init` 向 `GB::cEcoGame::RegisterTransportHandler` 注册的 **12 个 transport handler 实例**。这 12 项是运行时交通实现层的注册表，不是资源中的 12 个车型，也不是 `GetAgentTypeFromTransportEntryIndex` 返回的 5 个 Agent type。

新样本直接给出了连续注册代码：`SimCity2013-source-tree/src/SC/cGameAppMode/Init.c`。`RegisterTransportHandler`（RVA `0x201B70`）把 handler ID 映射到 vector index；`IndexFromTransportHandlerID`（RVA `0x206D50`）按 ID 查回 index；`HandlerIDForTransportID`（RVA `0x1FD200`）按规则 transport ID 查 handler ID；`cEcoTransportEntry::FillFromProps`（RVA `0x24AFF0`）把 TransportEntry 的 `mID`、速度、资源桶和 `mHandlerID` 从 Property 填入。

因此地图编辑器或规则编辑器要判断“一个单位能否生成道路车辆”，至少要同时保留 `UnitRule.mTransportID`、`UnitRule.mHandlerID`、TransportEntry `mID` 和 handler 注册表 index。只看车型名或资源桶会丢失这条关系。

## 完整 12 项注册表

第二列是 `RegisterTransportHandler` 的 handler ID，按新样本中的 FNV1 lower-case 规则反解；第三列是创建的实现类，第四列是传给构造函数的 type ID，第五列是 debug name。`0` 表示该 handler 实例没有额外的 transport type ID，而不是“无效 handler”。

| 注册顺序 | handler ID | FNV1 名称 | 实现类 | type ID | debug name |
|---:|---:|---|---|---:|---|
| 0 | `0xBCC56192` | `direct` | `SC::cTransportDirect` | `0` | `Direct` |
| 1 | `0xC086C6EB` | `airplane` | `SC::cTransportAirplane` | `0` | `Airplane` |
| 2 | `0x6DE71541` | `pipe` | `SC::cTransportPipe` | `0x6DE71541` | `Pipe` |
| 3 | `0x8136C3AA` | `helicopter` | `SC::cTransportHelicopter` | `0` | `Helicopter` |
| 4 | `0x34F64DCD` | `signal` | `SC::cTransportSignal` | `0` | `Signal` |
| 5 | `0x1A65986C` | `radial` | `SC::cTransportRadial` | `0` | `Radial` |
| 6 | `0xE6BBD7DE` | `venue` | `SC::cTransportVenue` | `0` | `Venue` |
| 7 | `0xE7565405` | `pedestriantraffic` | `SC::cTransportPedestrian` | `0xE7565405` | `Pedestrian` |
| 8 | `0x54ABDF8C` | `cartraffic` | `SC::cTransportVehicle` | `0x54ABDF8C` | `Vehicle` |
| 9 | `0x6AFE622F` | `boat` | `SC::cTransportVehicle` | `0x6AFE622F` | `Boat` |
| 10 | `0xA63DDC77` | `lightrail` | `SC::cTransportRail` | `0xA63DDC77` | `Light Rail` |
| 11 | `0x4B277752` | `heavyrail` | `SC::cTransportRail` | `0x4B277752` | `Heavy Rail` |

注册顺序有实际意义：`RegisterTransportHandler` 先把当前 vector 长度写入 `mTransportHandlerIDToIndexMap`，再追加对象。因此 handler index 与此表顺序一致，只要没有额外注册插入。`GetTransportHandler` 通过 `IndexFromTransportHandlerID` 再从 vector 取对象，这条链可以直接作为地图 debug 工具的运行时查询入口。

## “12 项”与其他交通分类的边界

新样本同时暴露了三个容易混淆的层级：

1. **12 个 handler 实例**：上表，是 `cEcoGame` 的注册表，决定寻路/模拟实现。
2. **5 个交通 Agent type**：`cTransportTraffic::GetAgentTypeFromTransportEntryIndex` 只返回 `0..4`，是更低层的 Agent 分类，不能替代 12 项注册表。
3. **资源/车型 TransportEntry**：例如四种警车是不同的 `cEcoTransportEntry.mID`，可以共享同一个 `cartraffic` handler。

因此不应把 C++ handler 类数量当作车型数量，也不应把 5 个 Agent type 当作规则作者看到的 12 类。

## 警车对照：四个车型共用 cartraffic

既有运行时证据已经闭合四个警车 TransportEntry：

| 车型 | TransportEntry `mID` | index | handler 层结论 |
|---|---:|---:|---|
| `PatrolCar` | `0x5788808B` | 50 | `cartraffic` → `SC::cTransportVehicle` |
| `PoliceDetectiveVan` | `0x78451C57` | 52 | `cartraffic` → `SC::cTransportVehicle` |
| `PoliceOutreachVan` | `0x80E8C885` | 179 | `cartraffic` → `SC::cTransportVehicle` |
| `FrenchPatrolCar` | `0xB67B1F5B` | 424 | `cartraffic` → `SC::cTransportVehicle` |

这里的 `mID` 是车型/TransportEntry 身份；`0x54ABDF8C` 是道路车辆 handler 身份。两者不是同一个 key，也不能互换。警车日志中过滤的 index 50、52、179、424 仍然正确，但要解释行为，应在同一快照中记录：

```text
TransportEntry index → entry.mID → entry.mHandlerID → cEcoGame handler index
```

旧警车窗口中实际活跃的是 index 50 `PatrolCar`，每快照约 25 个道路 Agent；这与新样本静态注册一致：它应进入 `cTransportVehicle`/`cartraffic` 的道路路径、速度和 sink 更新链。旧日志的 `+0x15C` 速度、`+0x180` 路径重算剩余时间、`+0x184` 停止时间描述车辆 Agent 状态，不是 UnitRule 的 `mTransportOps` 本身。

## 对规则解析和地图编辑器的直接影响

### 规则解析

`cEcoUnitRule::Read` 的 transport 数组和尾部 `mTransportID/mHandlerID` 不能只解析成一个“车辆类型字符串”。建议保留四层字段：

```text
unitRule.transportOps[]
unitRule.transportID
unitRule.handlerID
resolvedHandler = cEcoGame.handlerRegistry[handlerID]
```

解析器遇到 `handlerID = 0x54ABDF8C` 时可以标记为 `cartraffic`；遇到 `transportID = 0x54ABDF8C` 时仍要检查资源规则，因为 transport ID 与 handler ID 在部分默认实例上相同，并非协议保证。

### 地图资源添加

新增车辆资源时，最小闭包不是增加一个模型资源。必须确保：

1. TransportEntry 的 `mID` 与资源/车型名称 hash 一致；
2. TransportEntry 的 `mHandlerID` 能在 `mTransportHandlerIDToIndexMap` 找到 `cartraffic`；
3. UnitRule 的 `mTransportID` 与 `mHandlerID` 组合能被 `HandlerIDForTransportID` 解析；
4. `mActivePathEntries`、速度和资源桶与道路车辆 handler 的期望一致；
5. 运行时 Agent 的 transaction vector 能回指正确 TransportEntry index。

### 地图 debug 工具

新样本支持一个比“只列车型”更有用的调试面板：

```text
handler registry: index, handlerID, name, C++ class, typeID
transport entry:  index, mID, mHandlerID, speed, speedVary, activePathEntries
unit rule:        unitID, transportID, handlerID, transportOps[]
agent:            transaction key, TransportEntry index, handler index, speed, path timer, stop timer, sink
```

这样可以直接区分“资源存在”“规则引用”“handler 注册”“Agent 已运行”四种状态，避免再次把 index 179 的单一资源桶命中误判为全部警车。

## 证据边界与下一步实验

已实锤的是 12 项 handler 注册和警车共享 `cartraffic` 的静态层级；尚未实锤的是四种警车各自的 UnitRule 组合、不同 handler 对 `mTransportOps` 的具体解释，以及零售版是否保留开发版注册顺序。下一次运行时采样应在每个 TransportEntry index 上同时读取 `mHandlerID` 和 handler registry index，并对一个受控警务事件记录 `UnitRule`/transaction/sink 的时间序列。

相关旧证据：

- [traffic-simulation-entry-phase4-20261017.md](traffic-simulation-entry-phase4-20261017.md)
- [traffic-police-target-phase5-20261025.md](traffic-police-target-phase5-20261025.md)
- [traffic-police-return-phase6-20261025.md](traffic-police-return-phase6-20261025.md)
- 新样本源码：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/SC/cGameAppMode/Init.c`
- 新样本源码：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/GB/cEcoGame/RegisterTransportHandler.c`
- 新样本源码：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/GB/cEcoGame/IndexFromTransportHandlerID.c`
- 新样本源码：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/GB/cEcoTransportEntry/FillFromProps.c`
