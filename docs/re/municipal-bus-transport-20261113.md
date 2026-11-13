# 市区公交的发车、上下客和回库

样本是市区公车车库和普通公车站牌。安装版属性、零售 AEB 字节和反编译函数共同用于确认这条链。道路上的实际站序与乘客等待分布尚未做局内采样。

## 设施身份

| 角色 | 资源 |
|---|---|
| 市区公车车库 | Game 菜单 `00B1B104:09878A01:7985FB1C` 的 `menuUnit` 指向 Scripts `40E0C000:E608D9DA`。行为 bundle `40E0C700:9DDB15A8`。 |
| 普通公车站牌 | Game 菜单 `00B1B104:09878A01:F2F6645E` 指向 Scripts `40E0C000:FE043FDC`。`377AA3B3` 处理候车流，`42F3AD5E` 处理 T2 车辆到站。另有 `990C53C8/98A3F491` 对应 T1 的站台规则。 |
| 公车总站 | Scripts Unit `4722AB8D` 也挂 `377AA3B3/42F3AD5E`，同时拥有区域运输相关 bundle。下面的往返链以车库和街边站为例。 |
| 市区公交车辆 | Scripts Transport `00B1B104:40E0C600:DF88E6C3`。乘客总量资源 `F9B01777`（`BusPassenger`）的容量为 **80**。乘客类别与附带状态另有 bin。 |

## 1. 发车和回库

车库 `E608D9DA` 的 `0DC372BB=3` 与 `0DC372BC=3` 经 bundle 的初值/容量绑定列进入车辆库存。bundle `9DDB15A8` 把 `86E7374A` 初始化为 0 并绑定容量 3。该 bin 与出车、回库形成对称变化。

- `4C1B0C89` 的编译时间窗为 06:00–20:00、周期 30 tick。 `F9542A40` 的周期为 20 tick，失败可进入 `7A88C4EA`。它们有不同的标志条件，属于可尝试发车的调度入口。
- 发车规则从车库本地减 `BusT2` 和 `Bus`，增加在外计数 `86E7374A`，并给新车辆事务写入 `BusT2`。记录尾部的 `mTransportID=DF88E6C3`，`mHandlerID=54ABDF8C`。引擎用 TransportEntry 初始化事务 bin，再将事务交给运输处理器。见 [CreateTransaction](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:647687)、[SendTransaction](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:633854) 和 [RunRule](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:697504)。
- 回库 sink `9DDB15A8` 的 key resource 是 `86E7374A`。绑定规则 `7EE2D725 → C915104E` 检查 T2 事务，消耗事务中的 `BusT2`，减车库在外计数，恢复本地 `BusT2` 与 `Bus` 库存。其后续链还有把事务残留乘客转入站内资源的记录，例如 `65B7721A`。目前只确认回库目标类型及库存回收，是否总回同一发车建筑仍需对拍多车库场景。

## 2. 下一站如何选

发车规则的 TransportOp 序列是 `F5830EFB / B2FC5BFF / 377AA3B3 / 9DDB15A8`，四项的 life 向量均为 `(-1,-1)`。这些 ID 在 `40E0C500` 是 sink entry。各条目的 `0A810F28` 分别关联 `BusPickUp`、`TradeBusPickUp`、`E773C1BA` 和 `86E7374A`。前两项是本地和区域接客需求，第三项有对应的到站处理规则，第四项是车库接收入口。同一 sink 类型可由多个站实例提供。

引擎由 `GetInitialDestinations` 将开头和后续 life.x 为负的记录纳入同一候选组。车辆处理器按 sink 类型取得道路路径池，要求池有有效容量，沿道路距离与方向选择路段，实际停靠时还会检查道路位置、方向和剩余可分配容量。站台条件或库存变化会更新 sink 容量。 `FindNextDestination` 可以切换后续候选组。现有证据支持**按可服务的需求与道路状态动态停靠**，具体候选之间的最终权重和局内站序还需验证。见 [初始候选组](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:1790737)、[下一组候选](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:1088975)、[路段选择](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:1109106)、[停靠容量检查](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:890001) 和 [SinkCapacity](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:653616)。

T2 到站处理链中，`03D4180A` 可以给当前事务提供新一轮候选，包含接客、站台、车库和 `9584AE26`。 `ED032509` 与 `8FBE0801` 的候选只含车库类型 `9DDB15A8` 与备用 sink `9584AE26`。它们由不同条件边到达，所以一辆车可以在站台之间反复派送，也可以转入回收流程。备用 sink 的完整业务身份待确认。

## 3. 乘客如何进入和离开车辆

| 步骤 | AEB 证据 |
|---|---|
| 市民先到站 | `377AA3B3` 将乘客到达 sink 映射到进站规则，例如 `FCD3EEF9 → 2F4D3977`。W2 通勤事务减一单位 `SimWorkerW2`，站台增加一单位该资源，并增加 `PickUp`、`BusPickUp` 和类别状态。 `814C7ACB` 的周期为 10 tick，检查候车 bin，并写本地与全局等待分钟统计。 |
| 公交到站 | 站牌的 `42F3AD5E` 把接客 sink `F5830EFB` 等绑定到 `B28D5831`，检查事务中的 `BusT2=1` 后进入 `288A753D → D1354C91 → 92BB2EF8`。道路车辆停靠后调用 `DeliverTransaction`，引擎把事务绑定到站台规则状态。该链的事务作用域就是这辆车的 bin。 |
| 上车 | T2 分支 `CDBA856E` 从本地扣 `SimWorkerW2`、`PickUp`、`BusPickUp` 和附带状态，在车上增加 `SimWorkerW2`、类别标记 `D83629A2` 及 `BusPassenger`，同时更新接客统计。车辆 TransportEntry 的 80 容量对总乘客 bin 生效。 |
| 下车 | 同一 T2 到站树中的 `8091AE4C` 是另一类 W2 下客示例。它要求目的站的本地接收资源 `8B8F46F2`，从车上扣 `SimWorkerW2`、分类标记 `C738D1D0` 和 `BusPassenger`，写回站内的乘客与状态，并累计 `BusTripsToday`。这条记录说明下车条件按类别处理，示例中的类别 ID 与上一行的上客示例不同。 |
| 接续出行 | 同一站牌的可达规则含从站内取出下车标记再创建后续乘客事务的记录。例如 `37D51996` 消耗站内 `C738D1D0`，输出新的 `SimWorkerW2` 事务。已确认站内交接，最终会到哪个工作地及该分支的步行路段还需逐事务观察。 |

道路处理器与模拟规则的交接见 [车辆交付](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:889660) 和 [GlassBox 绑定事务并执行站台规则](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:638844)。Maxis 的设计说明也提到站台会识别附近的工作、住宅或换乘目的地。 [Guillaume Pierre 的公共交通说明](https://community.simtropolis.com/news/owning-the-streets-of-simcity-r254/)

## 4. ArgScript 证据与设施编辑面

伦敦 DLC 的原始 AEC `2382DACC` 引入 `TransitGarage.er2` 和 `TradeTransitGarage.er2`，实例化 `TransitGarage_SendVehicle`、`TransitGarage_Return` 和 `TransitGarage_ReceiveVehicle`，实参含 `BusT2`、`LondonBusT2`、`CarTraffic`。 `223372BF` 引入 `TransitStop.er2` 并实例化 `TransitStop_EnableVehicle` 和 `TransitStop_AddToTripsToday`。它们确认原作者用宏组合发车、回收、站台兼容与统计。原版市区公交的核心 include 正文目前以编译记录为依据。

OpenSCP 可以把设施显示成 **车库库存 → 发车规则 → TransportEntry → 候选 sink 类型 → 道路上的可达站台 → 上下客规则 → 回库或接续出行**。编辑时需要同时处理站台道路接入、车型兼容、sink 条目与 bundle 绑定、车辆容量和乘客类别。固定线路功能需要额外验证引擎是否支持用指定站点实例约束目的地。现有规则中的 sink ID 能表达站点服务类型。

## 复核位置

- `SimCity_Game.package` SHA-256 `79A804494912D06B565B1EDE1CE183586D5C2E3D1AC9668E3D176518341C59E2`。
- `SimCity-Scripts_287520926.package` SHA-256 `77DDCED4A23A818673E44C19649732390D8771BC5A1434583032E5841BAEDD04`。主 AEB `08068AEB:40800200:622B9CD7` SHA-256 `A388E3C15080A4721E7D0047DA180F6EE9F38292060C67892FA34B952564C381`。
- AEB 记录起点按字节计，`4C1B0C89=3156389`、`7EE2D725=3157109`、`2F4D3977=3220421`、`CDBA856E=3371201`、`03D4180A=3372857`、`8091AE4C=3392057`。零售记录的 TransportOp 从起点加 136 读取，每项为大端 `u32 ID + f32 life.x + f32 life.y`。两个尾 ID 从最后一项之后读取，避免把候选 ID 或 life 误读成车型/处理器。
