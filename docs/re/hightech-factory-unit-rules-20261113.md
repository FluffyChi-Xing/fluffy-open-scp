# 高科技机械厂的 GlassBox 规则链

样本为 `I_Gen_W3D1_HiTechMech_01_01_N`。以下结论来自安装版包、AEB 规则与带符号反编译。规则 ID、实际引用和专用化数值可复核。局内频率、收货方分布和长期盈亏仍需运行时数据。

## 从 Graphics 回到实际模拟对象

```text
Graphics 2F4E681B:00000000:70F203FB              正常状态模型
  ← Graphics 00B1B104:43E1C000:70F203FB          正常状态 Lot
  ← Graphics 00B1B104:40B8A10B:6B809EF1          变体表 0x0CA5CF4A
  ← Scripts  00B1B104:40E0C000:6B809EF1          实际模拟 Unit
       → 48E0C000:9FC315E2 → A609EF17 → 1FE7D7A1
         → 5729300E → 9EC474FF → 593B61D2       Property 继承
```

正常 Lot 的 LOD0 直接指向 `70F203FB`，同一变体表还含 `70F203F4`（`_A`）和 `70F203E7`（`_R`）。正常 Lot 中的 `0x0C12EF20` 列引用劳工车辆相关 bin。岗位、生产和火灾逻辑在模拟侧。

沿七张模拟属性表取 `0x0A0209A2..A5` 并集并应用排除列后，得到 54 个 bundle。常态、事件和 sink 三类入口共 288 个不同规则 ID，均可在零售主 AEB 中解析。沿编译的成功/失败 ID 可静态到达 1,404 条记录。可达数包括条件互斥和通用分支，只表示引用闭包。

## 建立、施工和招工

1. **区划决定何时建立。** Unit 继承 `0x092F9CB4=5266AA7B`。主 AEB 的 ZoneRule `373DA44C` 以同一 ID 为目标，读取五张地图（含辐射 `1D716F2E`）及工业 W3 税率 `9796CE25`，时间窗为 Day 10:00–17:00。`cEcoZone::ApplyRule` 路径委托 zone handler 选取有效位置并调用 `StartCreatingUnit/FinishCreatingUnit`。与 ZoneRule 同 ID 的 bundle `5266AA7B` 还通过 `CanCreate` 槽挂 `A412FB26`，检查工业 W3 税率低于 15。见 [ZoneRule 执行](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:693423) 和 [Unit ZoneID 读取](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:700066)。
2. **施工由 UnitRule 推进。** `7424049F` 的 `427E0344` 按 Day 08:00 调度，操作 `ConstructionTruckD1` 和施工状态。施工车辆 sink `7F63B2F3` 处理车辆和外部通路计数，`499DE95A` 写入一个 `ConstructionProgress`。后续 `F1495C98` 检查进度，`BA30F7B9` 在相应分支回送施工车辆并更新状态位。
3. **岗位由属性进入劳工网络。** 继承模板 `9FC315E2` 给出 11 个岗位，W1/W2/W3 为 2/5/4，最低开工人数为 4。`D07C5A04` 的属性绑定初始化岗位与 `WorkersNeeded`，状态变化规则 `05573B56/B9616B79` 等通过 PropOp 把相应岗位和所需劳工数量加入或移出城市统计。通勤 sink 如 `D2460875` 与 `11D06F97` 在邻近通勤资源、本地 `ExtraWorkerW1` 和 `ExtraWorkerW1Paused` 之间转移并沿后续规则恢复状态。相同机制分别存在 W2/W3 记录。生产激活链 `9ED7C534` 用 `0A7917C8` 将 `TotalWorkers` 操作专用化为 `4→4` 检查，随后写 `ActiveBuilding` 并置位 `0x20000000`。该位正是生产入口的条件。

## 水电、工艺、货运和利润

| 环节 | 样本规则与实际参数 |
|---|---|
| 公用事业 | 电力 bundle `19E5DB54` 的 sink `1ACEF6C8` 从配送事务取电。 `830BD003` 从本地 `Power` 扣除 `PowerNeeded`，PropOp 把周期从基础 30 改为 72 tick。水务 bundle `D6068BEA` 的 `AC95E620` 进水，`68615EA7` 按 `WaterNeeded` 扣水，周期由上游模板的 `0C09DA6A=72` 提供。两条消费规则均有短缺分支。 |
| 出厂产量 | `52A37D88` 的 `95E3FC17` 检查活跃位后走 `722BAA7C`。PropOp 用 `0C0F20DB=72` 改写生产周期，用 `0E33438A=3` 把 `SkillLevel` 变为 `3→3` 检查，并用 `0C0F20DC=3` 把本地 `Freight` 和全局 `FreightProducedToday` 的每次应用写入量都改为 3。该规则还按当前 `LowSkill` 数量扣减该 bin。输入、输出容量和 apply count 仍会约束实际产量。已定位的这条产货规则消费的是技能相关状态，水电通过各自的服务链影响开工。 |
| 库存与发货 | `343C68A4` 检查本地 `Freight` 和可用 `freighttruck`，随后 `12AB4615` 派出货车，密度分支决定运输链。D1 对应 `E5A9C2E4`，编译操作为本地 Freight `1→0`、事务侧 Freight `0→1`，失败目标为 `50DEF80A`。货车/货物移动还依赖引擎路径、目标 sink 和交易。 |
| 工厂利润 | `D69DE372` 的货车 sink `50DEF80A` 后进入财富分支 `32602A41 → 32602A42 → 32602A43`。W3 走 `2CBB40EF`，增加本地 `ProfitToken` 与 `HappinessFromShipments`，并检查交易侧的 ProfitToken。 `2B5A2BAF` 用 `0AB985AA=60` 为本地 ProfitToken 绑定初值，其他计数也有 Property 绑定。利润 token 随缺水、缺电或经营扣减变化，可作为升级和废弃条件。 |
| 城市税收 | 税率参与准建和经营规则。 `37892368` 在财富等级为 3 的分支把本地 Industrial 的数量映射到全局 `IncomeFromIW3`，继承的 Taxes bundle `7F04E974` 在销毁/状态变化事件做相应减记。这里确认的是模拟收入 bin 记账，尚未把市政现金结算公式逐项对到这座厂。 |
| 污染 | `C935DD44` 的 `0FBC02CA` 同步写入本地、全局和 AirPollution 地图。该规则的 PropOp 读取 `0C131CC9=8`、`0C131CC4=3328` 与区域参数 `0C47D858=2`，前两项乘 4 改写周期和应用上限。地图侧扩散由 MapRule 处理。 |

`input=output` 表示资源检查时应显示净变化为 0。负 output 编码的是数量区间条件，例如 `input=3, output=-4` 要求当前数量恰好为 3。参见 [BinRuleCount](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:701131)、[OverrideResourceOp](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:663123) 和 [ApplyAmountsFromBins](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:701205)。

## 火灾、停业与废弃

- **起火。** 继承的可燃 bundle `0D19711A` 维护 `FireFuel`、`FireRisk` 和 `Heat`。`3E853BE4/B2AAFDBB/4082DFDD` 分别测试 FireRisk 为 1/2/3，有每日触发及时间抖动，检查本地标志与全局资源，再写 Heat。三条记录的基础调度 chance 为 1%/10%/25%，样本 PropOp 分别将其替换为 **8%/4%/24%**，应用上限来自 `0A7BB620=8`。这些数值是调度条件参数，局内实际起火频率还受准入状态和城市火灾计数约束。
- **燃烧和消防。** 起火后 `AEA457EB` 检查全局火灾计数，相关分支 `DF85FF26` 写 `IsSmoking` 并增加全局火灾计数。`686FC533/50C3850E` 消耗 FireFuel、写 FireLevel，后者还增加空气污染。 `2E57D1A0` 是邻近 Heat 的 sink，后续 `AD8FA0D7` 有写燃烧状态与火灾统计的路径。消防员、报警和灭火也有 sink 或后续规则。被烧毁计数在 `FE945624` 的 Destroy 槽可见，仍需条件才能记账。
- **经营恶化。** `BC818CB4` 的 `3206337D` 在 Day 12:00 开始一条利润扣减链。 `BB3E88FA/37DBCBBC` 处理缺电/缺水导致的扣减，`73D2500A` 用待扣量进一步消耗 ProfitToken。 `CCF91580` 检查利润已经为零及非教程条件，成功后 `7362794B` 在其条件满足时将状态位改为 `0x800`。 `182D2924` 的 OnStateChange 规则 `7B8C1FF3` 读取该位，清理施工/空置/正常 bin，并写入本地 Abandoned 及全局废弃统计。恢复正常或转为空置也各有事件规则 `11B97B74/B4B7A614`。外观则由状态和 Graphics 变体表解析。原生 `FailUnit` 另有失败目标和换 Unit 的路径，应与上述状态位链分别追踪。见 [FailUnit](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:642498) 和 [事件枚举](D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/include/GB/tEventRuleType_ord628.h:8)。

## 对 OpenSCP 的设计约束

这座厂的生命周期需要同时表示 **ZoneRule 选址生长 → UnitRule 施工和经营 → sink/运输交付 → 状态事件 → Graphics 变体**。火灾也提供了与 V 博士不同的随机例子：起火入口本身有专用化的调度概率，V 博士的分支检查周期刷新的位。脚本视图应显示 AEB 基础值和属性专用化后的有效值，例如生产周期 `120→72`、每次应用产量 `1→3`，并把资源库存、利润 token、全局税收 bin 和外观状态呈现为各自的输出。原始工厂 ArgScript 未恢复，反编译视图应以实际规则 ID、作用域、编译操作和来源标注为依据。

## 复核

- Graphics `SimCity_Graphics.package` SHA-256 `EAB5BBBE08B8EF7D5F7E4B0538523A2364C2CD28CA73E9C77A4CE03768E84555`；Game `SimCity_Game.package` SHA-256 `79A804494912D06B565B1EDE1CE183586D5C2E3D1AC9668E3D176518341C59E2`。
- `SimCity-Scripts_287520926.package` SHA-256 `77DDCED4A23A818673E44C19649732390D8771BC5A1434583032E5841BAEDD04`；主库 `08068AEB:40800200:622B9CD7` SHA-256 `A388E3C15080A4721E7D0047DA180F6EE9F38292060C67892FA34B952564C381`。
- 用 `prop_dump` 沿 TGI 复核属性，通过 `tmp/school-dissection/parse_rules.py` 解码主库与 PropOp，以 `tmp/behavior-exemplars/rci/parse_rci.py` 核对 ZoneRule。代表记录字节位置为 `95E3FC17=458945`、`722BAA7C=459665`、`2CBB40EF=463697`、`B2AAFDBB=1089101`、`7B8C1FF3=478433`。GlassBox 模拟组件的背景见 [Andrew Willmott 的 GDC 演讲](https://www.andrewwillmott.com/talks/inside-glassbox)。
