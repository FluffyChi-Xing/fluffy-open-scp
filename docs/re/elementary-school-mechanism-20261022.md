# 小学菜单与游戏机制纵向研究（2026-10-22）

## 已确认的绑定链

小学菜单条目 `29873FA2`（Menu2，group `09878A01`）位于教育类别 `95A9D6FD`，其 `menuUnit` 为 `C264A268`。建造工具用这个 Unit ID 查找 EcoGame Unit，而不是用菜单标题或图标 ID 判断建筑类型。

```text
Menu2 29873FA2
  └─ menuUnit C264A268
       ├─ EcoGame Unit 40E0C000:C264A268
       │    ├─ parent 48E0C000:7E4B99E5
       │    ├─ child 40E0C000:8C4245A6
       │    └─ rule/resource references (0A0209A5, 0A0209B2)
       └─ Lot/Graphics 40E1C000:C264A268
```

菜单显示的建造价 `16000`、维护费 `400` 是一层子 Unit 求和：主体 `13000/300` 加子 Unit `8C4245A6` 的 `300/100`。因此菜单项对应的费用不是单一资源文件中的单一字段。

## 小学主体 Unit 的行为字段

来自 `SimCity-Scripts_287520926.package` 的 `40E0C000:C264A268`：

| 字段 | 观察值 | 解释边界 |
|---|---:|---|
| `0x0A8A41DA` | 13000 | 主体建造价 |
| `0x09AE19D7` | 300 | 主体维护费绝对值；菜单导出为负值 |
| `0x0AD7FAC9` | 150 | 与菜单“课桌 150”一致，属于容量/统计值 |
| `0x0AFB9882` | 72 | 教育相关数值，具体名称仍需运行时/符号侧表确认 |
| `0x0AFB9886` | 900 | 教育相关数值，具体名称仍需运行时/符号侧表确认 |
| `0x0E585AE0` | 500 | 教育/地价等统计候选值，不能仅凭数值命名 |
| `0x0E585B00` | 500 | 同上 |
| `0x0C216562` | 10 | JS 符号表将其定义为 `kPropGarbageProducer_GarbageOnlyAmount`，因此小学有静态垃圾产生量 10 的字段证据 |
| `0x0A0209A5` | 4 个 Key | Unit bundle/connected-unit ID 数组；开发版 `BuildImportExportJSValue` 用它统计相连 Unit，不是 ER2 Rule ID |
| `0x0A0209B2` | `F90C8A94` | 另一组建筑/资源定义引用；其 `40E0C700` 条目含资源定义数组 |
| `0x0AFE27CA` | `7987C2C3` | addon/container 入口 |
| `0x0AFE27B1` | `95A9D6FD` | 教育菜单类别绑定 |

`kPropertySCUnitProductionInput/Output` 的哈希分别是 `0x09CC6686/0x09CC6687`；主体小学 Unit 没有这两个字段。因此当前证据不支持“学校生产普通资源”的结论。垃圾字段是独立的 `GarbageProducer` 属性，不能由资源输入/输出字段替代。

## 校车机制的定位

小学 Unit 的子 Unit `8C4245A6` 同时是成本表中的菜单子项 `48137100`，名称为“校车停车位”，显示：费用 `3000`、维护 `100`、公车容量 `50 个小孩`、公车数量 `2`。截图显示标准小学放下后停车位计数为 `1/5`，车棚中已经有一辆校车；因此默认车辆属于主体 lot 的初始装配状态，`8C4245A6` 是后续可购买的停车位扩展。UI 的“公车数量 2”是扩展模块统计值，不能直接当作截图中的当前可见车辆数；模型、停车位计数和模拟车辆数量属于三个层次。

教育扩展面板还包含两个独立 Unit：

| 菜单实例 | Unit | 费用/维护 | 容量 | 作用 |
|---|---|---:|---:|---|
| `9DF53908` | `5DE9CD2A` | 10000 / 200 | 200 | 普通教室，增加教师岗位和学生容量 |
| `DDED4F0F` | `5DE9CD2A` 的父级关联 | 10000 / 200 | 200 | 顶楼教室，改变可放置楼层 |

`5DE9CD2A` 的 Unit 属性直接包含 `0x0AD7FAC9=200`、`0x0A8A41DA=10000`、`0x09AE19D7=200`，并引用建筑资源 `AEF52A8E`。顶楼教室的 Unit 属性通过 `0x00B2CCCB` 指向 `5DE9CD2A`，说明两者共享行为/经济主体，差异主要由放置与模型配置表达。

主体 Unit 还包含 `0x0E1BAC61/0x0E1BAC62` 具名位置表，以及 Lot 中相同位置表和道路路径字段。根据 `cUnitModel::mUnitLocations` 的已验证装配语义，这些是建筑锚点；其中哪些锚点供校车生成、停车或接驳使用，仍需把位置 ID 与 transport/agent 资源交叉解析，静态位置表本身不能证明用途。

## ER2 行为规则的当前证据

### 规则是否可解析

可以，但要区分三种层次：

1. **源码层**：`0x08068AEC` 是 ER2 文本规则类型，使用 EA::ArgScript 的行式命令/块命令语法。图中的 `UnitRule goToWork`、`options`、`sendTo`、`switchTo`、`repeatAfter`、`local People in`、`agent People out` 是面向规则作者的语义层，描述 Agent 的目标、交通方式、输入和输出。
2. **编译规则层**：`0x08068AEB` 是游戏实际加载的 ERZ 二进制规则库。当前解析器已经能精确校验版本 `(5,2,14)`、28441 条 UnitRule、573 条 GlobalRule，并解析规则 ID、规则记录、资源操作池和索引段。
3. **运行层**：EcoGame 根据 Unit 的专用化结果选择 UnitRule，再由 `ApplyRule`、TransportOps、成功/失败链和 Agent 测试驱动模拟。开发版 `cEcoUnitRule::Read/Write` 已确认这些字段：`local`、`nearby`、`map`、`global`、`fromBinOps`、`successRuleID`、`failRuleID`、`agentTests`、`TransportOps`、`TransportID`、`HandlerID`。

因此，图例表达的“建筑行为 ↔ 脚本”关系是真实的，但当前本地工具已经完成的是二进制结构解析和资源操作解析；还没有完成把二进制字段反编译回 `goToWork` 等原始命令文本的完整反编译器。规则名和命令名在发行版中通常被哈希化或不随 AEB 保留，不能仅凭 `ruleId` 恢复原始源码。

本轮还发现一个需要修正的实现边界：现有 `crates/dbpf/src/erz.rs` 的摘要解析器使用了通用的 128 字节 RuleRecord 视图，适合验证文件整体布局和索引段，但不能直接当作 `cEcoUnitRule::Read` 的字段解码器。开发版 `Read/Write` 显示 UnitRule 还包含 `unitID`、创建偏移、图形状态、AgentTests、TransportOps、TransportID 和 HandlerID；下一步必须按该专用布局读取，并保留版本/名称开关，不能把通用摘要字段位置误命名为交通字段。

ER2 规则的静态结构已经确认：`unitRule/globalRule` 通过规则集索引解析，GlobalRule 的资源操作是 `(resourceID, input, output)` 三元组，并由 `EcoBinSet.ApplyRule` 执行；动态数量来源还可能通过 `fromBinOps` 改写基础数量。离线规则库 `tmp/rule-main-287520926.bin` 可以精确解析 28441 条 UnitRule 和 573 条 GlobalRule；已知运行探针中的 `unitSlot=392/stateIndex=12` 对应 UnitRule `0xF01B0EB5`，但该日志没有建筑 TGI，`selectedUnits=[30]` 也不是 `C264A268` 的可证明身份。因此不能把它归属到小学。

`0x0A0209A5` 的四个目标是相连 Unit bundle 的成员：`6C042367`、`3A1C3971`、`95573B5D`、`9A1BFB2F`。开发版 `FillFromProps` 明确先把这些 bundle 加入 `cEcoUnitEntry`，再调用 `AddBundle`，因此它们是启用的 connected-unit/bundle 定义，不是 ER2 Rule ID。`0x0A0209B2` 则被同一函数读入 `excludedBundleKeys`，语义是排除列表，不能当作小学的第二组启用规则。小学扩展机制应优先沿 Unit bundle、父级和 addon container 解析。`F90C8A94` 的 `40E0C700` 条目是资源数组容器，而 `40E0C100:F90C8A94` 为空；资源数组同样没有直接携带 ER2 rule ID。

### Bundle 到 UnitRule 的已验证连接

`AddBundle` 并不把 bundle 当成纯图形。它读取每个 bundle 的三组规则属性（属性 ID 为 `0x08080D58 + 0..2`），并把每个 Key 的 instance 交给 `IndexFromUnitRuleID`。`AddBundleEventRules` 还读取 `0x08327D48 + eventRuleIndex`（共 21 个事件槽），再按同一个索引函数加入事件规则。小学 bundle 中的规则 ID 因此可以直接在 AEB 中定位：

| bundle | 规则引用字段 | AEB 命中数 | 结论 |
|---|---|---:|---|
| `6C042367` | `0x08327D49` 等 | 13 个 unit-rule ID | 主 bundle 含资源/服务相关规则候选；不能只按数组数量解释为人或车 |
| `3A1C3971` | `0x08327D49` | 3 个 unit-rule ID | 一个独立规则小组 |
| `95573B5D` | 无 `0x08…` 规则字段 | 0 | 目前只看到资源 bin |
| `9A1BFB2F` | 无 | 0 | 空 bundle |
| `F90C8A94` | `0x08327D52` | 1 个 unit-rule ID | 资源容器关联的单独规则候选 |

在当前离线 AEB（SHA-256 `a388e3…564c381`）中，`6C042367` 的规则候选包括 `0xA44E326F/6C/69/66`、`0xA34E30FE/F9/F4`、`0xA64E35B4`、`0xBA80A329`、`0xE9EE9534`、`0x8466E052`；`3A1C3971` 命中 `0x84F487AC`、`0x4907F5D7`、`0x67B81421`；`F90C8A94` 命中 `0x9CFEB042`。这些是“规则 ID 已被索引”的证据，尚未证明每条规则分别代表校车、学生、教师或教育提升。要完成语义归属，必须继续解码 UnitRule 的 AgentTests、TransportOps 和资源操作，并与运行日志对拍。

## 规则执行日志：已有可捕获通道

规则执行时确实存在稳定的日志通道，但它是 `cEcoGame::mDeltaStream` 中的二进制 action 流，不是普通文本日志。开发版入口和零售版入口均已对拍，当前仓库的 `tools/dynamic/rule-logs/capture.py` 已通过 Frida 被动监听这些入口；它不调用游戏函数，只观察进入日志函数的参数。

### UnitRule 相关 action

| 入口/关键字 | 记录内容 | 二进制结构 |
|---|---|---|
| `LogUnitRule` / `kLogActionUnitRule` | 条件通过并进入单位规则日志 | action header：`unitSlot`、`stateIndex`；随后 `applyCount:int32`、`maxApply:int32`、`maxPossibleMapApply:int32`、`selectedUnits` 的数量和 ID 列表 |
| `LogUnitRuleAttach` / `kLogActionUnitRuleAttach` | 成功/失败链附加下一条规则 | header：`unitSlot`、`success`；随后 `stateIndex:int32` |
| `LogUnitFail` / `kLogActionUnitFail` | 单位规则失败 | header：`unitSlot`；随后 `stateIndex:int32` |
| `LogUnitEventRulesAttach` / `kLogActionEventRuleAttach` | 事件规则槽附加 | header：`unitSlot`、`eventType` |
| `LogAction` / `kLogActionSimTicks` | 规则日志前补齐模拟时间 | header：`simTickDelta` |

零售版还会在部分单位移动场景先写 `kLogActionUnitMove`，然后再写 UnitRule action；因此 action1 的 `data1=0` 不能单独解释为单位 0，必须读取前置的单位上下文。header 使用大端 32 位编码；超过内联范围的 `data1`/`data2` 会写扩展 int32。解析时必须使用对应 delta stream 版本，不能把普通存档版本直接当作 header 版本。

### 日志采集器输出结构

`capture.py --scope unit` 输出 `trace.jsonl`，每行是：

```json
{
  "channel":"simulation.rule", "schema":1, "phase":"log-entry",
  "scope":"unit", "game":"0x...", "unitSlot":827,
  "stateIndex":135, "simTicks":12345,
  "mapping":{"ruleIdHex":"0x...","baseRuleWithinSet":31661},
  "applyCount":3, "maxApply":9, "maxPossibleMapApply":0,
  "selectedUnits":{"values":[30],"count":1,"truncated":false},
  "unitStamp":null, "readErrors":[]
}
```

`mapping.ruleIdHex` 是运行时脚本索引映射到的规则 ID；`baseRuleWithinSet` 是规则集内基础索引。`unitSlot` 不是建筑 TGI，也不是永久身份，换城市、重载存档后不能复用。当前采集器明确保留 `unitStamp:null`，因为尚未闭合单位内部 stamp 与建筑资产的映射。

### 与资源、失败原因相关的其他通道

仅捕获 `simulation.rule` 不能证明水电消耗或垃圾增加。规则执行函数还调用资源 bin 操作，现有采集器提供独立通道：

- `--scope unit-effects`：捕获单位本地 `ApplyBinRule` 前后的 amount/capacity 快照。
- `--scope unit-consumption`：筛选 `input > 0` 的单位本地资源操作，用于观察消费型规则。
- `--scope effects`：捕获全局规则的资源操作。
- `--scope conditions`：捕获 `CanRunRule` 的允许次数、控制旗标、资源槽值和已知阻止原因。

这些通道输出 `simulation.effect` 或 `simulation.condition`，与 `simulation.rule` 的 `applyCount` 分开。条件检查通过不代表规则随后一定执行；规则日志缺失也不能证明规则没有尝试，因为 `LogUnitRule` 只覆盖进入日志的路径。

推荐针对小学候选规则使用短窗口、定向过滤：

```powershell
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope unit --rule-id 0x84f487ac --seconds 10 --out tmp/school-rule
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope unit-effects --rule-id 0x84f487ac --seconds 10 --out tmp/school-effects
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope conditions --rule-id 0x84f487ac --seconds 10 --out tmp/school-conditions
```

目前这些命令仍需要一个已经进入城市的、版本指纹匹配的零售进程；候选规则 ID 来自社区包 AEB，必须先确认安装包中同一规则集和规则 ID 存在，再把结果归属于原版小学。

## 当前安装与运行存档的实测结果

本轮以用户正在运行的进程为准：PID `36688`，路径为 `D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCity\SimCity.exe`，SHA-256 为 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`，与已验证的零售探针版本一致。安装目录的 `SimCity-Scripts_287520926.package` 抽出的 AEB 为 5,947,232 字节，SHA-256 `a388e3c15080a4721e7d0047da180f6ee9f38292060c67892fa34b952564c381`；与此前用于解析候选规则的 AEB 内容一致。因此本轮的规则 ID 基线已经切换为安装目录实际资源，而不是社区包的文件外壳。

已对该正在运行的存档做被动采集：

- 10 秒 `global + unit`：捕获 4 条 `simulation.rule`，读取错误 0，丢弃 0。
- 10 秒定向 `0xE6E75D12`：捕获 2 条 `simulation.effect`。单位本地资源 `0x54B5A6C6` 从 `0` 增加到 `100000`，容量 `1000000`，`applyCount=100000`，实测差值与规则计算一致。
- 20 秒定向 `0x4907F5D7`：0 条命中，不能解释为规则不存在，只能说明该规则在这 20 秒观察窗内没有进入 `LogUnitRule`。
- 15 秒未指定规则的单位采集：捕获 `0x0865D6CC`，单位 slot `932`、entry index 已开始记录；这只是当前存档中活跃的单位规则，尚未证明是学校。

这组结果验证了“安装包规则 → 当前进程规则集 → 规则日志 → 资源前后变化”的链路已经能闭合；小学、中学和社区大学的资产归属还需要把 `unitEntryIndex` 继续映射到对应的 Unit TGI，再进行定向监听。

截图所示的当前状态（11:15、上课时间 6:00–15:00、950/950 学生、8 辆校车、满配扩建）与本轮高命中现象是一致的：规则入口确实在持续运行。30 秒全单位发现采集命中 25,020 次，涉及 177 个运行单位、33 个 `unitEntryIndex`、192 个规则 ID；因上限 1,000 保存了 1,000 条，另外 24,020 条被限额丢弃。这个结果说明“先前只有几条”是采样策略造成的，不是学校没有脚本活动。但由于同一城市同时运行住宅、交通、服务和教育单位，不能把全单位发现结果直接等同于小学行为。

## 放学后的持续资源事件

在用户确认学校已经放学后，又对同一 PID 做了 20 秒 `unit-consumption --all-targets` 采集。仍然捕获到 1,196 次 `ApplyBinRule` 入口，其中 172 条完成了前后快照，读取错误和丢弃均为零；涉及 8 个运行单位、13 个规则 ID。4 个单位 slot（391–394）各出现 36 条记录，另有 slot 35、42、56、141 的低频记录。记录中的资源 ID 是编译哈希（例如 `0x3BBBAC69`、`0xACC3B1F9`、`0x54B5A6C6`），在没有已验证的资源 TGI/名称表前不能命名为水、电或垃圾。

这批数据能证明：放学后仍有持续的单位本地资源消费/转移，且部分规则按固定周期或状态重复执行；它不能证明这些单位属于小学。4 个连续 slot 共享完全相同的 9 条规则，反而提示它们可能是同类系统单位或服务对象，不能凭 slot 数量当作 4 所学校。当前没有从这些日志直接得到城市总人口、建筑总数、车辆总数或小人总数。

### 现有日志能回答与不能回答的数量问题

| 问题 | 当前日志能得到的内容 | 不能直接推出的内容 |
|---|---|---|
| 人口 | 规则 `selectedUnits` 中被某条规则选中的 ID 数量；本轮 1000 条样本只有 12 条非空、23 个不同 ID | 城市总人口、950 名学生的真实人数 |
| 建筑 | 运行中出现过的 `unitSlot`、`unitEntryIndex`、规则状态和资源 bin | 建筑总数、建筑类型、哪一个 slot 是小学 |
| 车辆 | 规则的 `TransportOps` 或交通相关规则命中（目前尚未完成语义映射） | 当前车辆总数、8 辆校车的实体数量 |
| 小人 | `selectedUnits` 是规则选择结果，不是渲染 Actor 列表 | 场景中可见小人总数、教师/学生人数 |
| 资源 | 规则 ID、资源哈希、amount/capacity 前后值、实测 delta | 资源哈希对应的显示名称，除非补齐资源 TGI 侧表 |

因此，950/950、8 辆校车和人口统计应从学校 UI/资产状态或对应的统计 Property 读取；规则日志适合回答“哪个 Unit 在何时执行了什么规则、改变了哪个资源 bin”，不适合替代城市统计接口。

## 全城市规则日志量级基线

为避免详细探针保存上限造成“日志很少”的错觉，新增了项目探针目录下的轻量探针 `probes/runtime/rule_log_volume.py`。它只统计 `LogUnitRule` 与 `LogGlobalRule` 的入口命中，不保存明细、不写 delta、不调用游戏函数。当前运行中的安装版进程在 15 秒窗口内得到：

```text
UnitRule 入口：69,811
GlobalRule 入口：443
去重 unitSlot/stateIndex：29,063
去重 global stateIndex：104
读取错误：0
```

这对应约每秒 4,654 次 UnitRule 日志入口和 29.5 次 GlobalRule 入口。它是整座城市的模拟量级基线，不是小学专属数量；详细探针应先按这个量级设置缓冲和窗口，再用 `unitEntryIndex`、规则 ID、资源 ID 过滤。探针输出保存在 `tmp/rule-log-volume-20261022-15s.json`。

## 下一步验证

1. 解析四个 `0x0A0209A5` 目标的完整 `40E0C700/40E0C100` 条目，寻找资源 bin、容量和 agent/vehicle 引用；规则字段按 `AddBundle` 的实际属性 ID 读取。
2. 解析 `F90C8A94` 的资源数组与它指向的 `175708107` 等资源 ID，并与垃圾、教育和交通资源表交叉比对；`175708107` 当前只确认是 `uint32` 值，不能先命名成资源 ID。
3. 读取小学的 UnitRule slot 和运行日志，将规则 ID 与 AEB 定义中的资源操作对拍。
4. 对照中学 `AF3EAA80`、校车站牌 `9B0D5AAF` 和校车停车位子菜单，确认校车 agent 的生成条件、路径入口与数量扩容方式。

## 实机观察补充：师生 Agent 与路径

用户截图提供了两条运行时证据：小学周围同时出现高个成人和一组矮个儿童，且这些 Agent 分布在建筑入口、绿地和道路边缘；另一张住宅截图显示多个市民沿车道/人行路径移动。由此可以确认：

- `0x0E1BAC61/62` 位置表不等于“一个位置对应一个 Agent”。它是命名锚点表；一个锚点可以被一批次 Agent、一个服务流程或一个车辆停靠过程复用。
- 师生身份来自 Agent 类型/外观和 UnitRule 的选择条件，不能由 spawner 数组长度推断。高个成人很可能是教师/工作人员，矮个儿童很可能是学生，但这需要 Agent/transport 运行日志或外观资源引用确认。
- 建筑内部和周边的移动路线由 Lot 的 PathPoint/Path 段与脚本 path entry 共同决定。Path 是可行走的几何/连接网络；Spawner/UnitLocation 是入口、汇聚点、车位等语义锚点；AgentRule 决定何时生成、去哪里以及使用哪种交通方式。

因此小学专题后续按四层拆解：

```text
Unit bundle / capacity
  → UnitLocation（门口、车位、汇聚点）
  → Path entry（建筑内外步行/车辆路径）
  → UnitRule / AgentTest / TransportOps
  → 学生、教师、校车及其资源变化
```

消费水、电、产生垃圾、教育等级、火灾和废弃属于同一套更大的状态闭环，但证据类型不同：资源消耗/垃圾需要 UnitRule 的资源操作和 bin 前后变化；教育需要容量、学生/教师 Agent 和全局教育统计的联动；火灾/废弃需要状态字段、EffectControl、灾害/服务规则和 Unit 状态迁移。后续不会把它们简化成单个 Property 哈希。

以上“垃圾产生量 10”是属性/符号表静态证据；“校车实际生成、路线和每日运行”仍属于需要运行时观察或更完整 agent 资源解析的未决问题。

## 2026-10-23 上课时段小学运行时复核

- 运行进程：PID 36688，`SimCity.exe`。
- 修正探针身份字段：`cEcoUnit::mID` 位于 Unit 对象 `+0x0C`；此前 `+0x08` 会导致小学过滤错位。相关单元测试 31/31 通过。
- 10 秒 `UnitEntry=0xC264A268` 定向规则采集：命中 11,182 次，接受 27 条，过滤 11,155 条，保存 27 条，丢弃 0，读取错误 0；全部映射到运行 slot 802。这 27 条均为 `0xB52C889D`，stateIndex 429；另四条规则是在后续资源窗口捕获。
- 15 秒资源操作采集：`applyOps` 命中 28,969 次，接受 22 条，保存 22 条，丢弃 0，读取错误 0；全部为 slot 802 的 unit-local 更新。观察到资源 ID `0x05DAADA6` 输入 20、净变化 -18；`0xC02F343B` 输入 3、净变化 -124；`0x2C1DA112`、`0x06EDFbed`、`0xCF0B7832` 各出现输出增加；`0xFF635EE8` 仅零变化。
- 通过安装目录 `SimCity_Game.package` 导出的 `0x40464200` Game UI 脚本静态常量表完成了部分映射：`0x2C1DA112 = kResourceIDStudent`（学生）、`0x06EDFBED = kResourceIDTotalStudents`（累计学生计数）、`0x05DAADA6 = kResourceIDSpotsForKids`（儿童席位）、`0xCF0B7832 = kResourceIDStudentStudyingE2`（E2 在校学习学生）、`0xFF635EE8 = kResourceIDFromTradingConnection`（跨城来源标记）。`0xC02F343B` 在已导出的 UI 常量表中没有同名符号，暂保留为未知教育/单位内部资源。
- 这解释了运行时的“资源消费”形态：`0x05DAADA6` 和 `0xC02F343B` 的 input 是规则计数参数，`0x2C1DA112`、`0x06EDFBED`、`0xCF0B7832` 的 output 是学生状态/统计写入；它们不是水、电或垃圾 bin。小学静态 Property 的 `0x0C216562 = 10`（`kPropGarbageProducer_GarbageOnlyAmount`）仍是垃圾产生量的独立证据，但本轮 22 条资源操作没有出现可命名的水、电资源 ID。


### 静态资源命名核验与采集解释修正

直接从当前安装目录的 `SimCity_Game.package` 提取 `67771F5C:40464200:7CCC548C`，SHA-256 为 `683c66fe0d0146b9b630bbe59e1df89848fc05d907f3028147c01ff4f407431b`。精确符号赋值与行号见 `evidence/elementary-resource-symbols-20261023.json`。

| ID | 安装包 JS 符号 | 本轮实际变化 |
|---|---|---|
| 2C1DA112 | kResourceIDStudent | 835 → 853，+18 |
| 06EDFBED | kResourceIDTotalStudents | 835 → 853，+18；统计周期尚未确认，不能称终身累计 |
| 05DAADA6 | kResourceIDSpotsForKids | 115 → 97，-18 |
| CF0B7832 | kResourceIDStudentStudyingE2 | 835 → 853，+18 |
| FF635EE8 | kResourceIDFromTradingConnection | 0 → 0 |
| C02F343B | 未找到直接符号 | 221 → 221 → 124 → 0 → 97 |

学生数与剩余席位之和在样本中保持 950，吻合用户所述满配容量。B52C889D 的可见本地效果是每应用一次，Student、TotalStudents、StudentStudyingE2 各 +1，SpotsForKids -1。可以确认这是入学计数/席位占用效果；尚未由这些本地操作证明完整规则的 Agent/交通语义。

C02F343B 的四步运行证据显示其与剩余席位有关：先与 SpotsForKids 同作 input=1/output=1 的非消耗检查，随后 applyCount=97 时扣减 97，再以 applyCount=124 清零剩余 124，最后 applyCount=97 写入 97。可作为教育需求/席位协调内部计数的候选解释，不能给它指定官方名称，也不能断言这四步完整定义其功能。

纠正上一轮输入/输出总量表：input/output 是每次应用的系数，需乘以 applyCount；C02F343B 的实际扣减为 221（97+124），写入为 97，净变化 -124。input=output=1 是保持数量的操作，不能当作净消费。上节“输入 3”等数字只是系数之和，不是实际消耗量。

安装包同时明确定义 Power=8E8B4E10、Water=9E3C3DFA、Garbage=7CB23A4E。这三者未出现在这 22 条 unit-local 资源更新中；这只限定本次采样覆盖，不证明学校全天没有水电消费或垃圾生产。此前偏移错误的零命中窗口不能作为昼夜机制对照证据。
