# 小学 ER2 编译规则、运行日志与模拟时间的第一组闭合证据

2026-10-23。本轮只分析安装包、EXE 和已有日志，没有连接或注入游戏进程。原始 ER2 文本未恢复；以下是编译 ERZ 的字段与语义，不能当作原作者的 ArgScript 源码。

## 结论

已经建立一个直接对应：小学 `23E737BD` 配置为每个 Hour 周期起点触发；旧日志在 tick **44880** 实际记录该规则的资源操作，该 tick 正好是配置推导的 **11:00**。另有学生到达事件和水电周期消费两种不同入口，不能统一解释为“每 tick 执行一次”。

证据来自用户实际安装目录 `D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow`。脚本包为 `SimCityUserData/EcoGame/SimCity-Scripts_287520926.package`，AEB SHA-256 为 `a388e3c15080a4721e7d0047da180f6ee9f38292060c67892fa34b952564c381`。这是已有运行时溯源确认的主规则库；本次没有重新测量当前进程的覆盖包及专用化状态。

完整机器证据见 [schedule JSON](evidence/elementary-school-rule-schedule-20261023.json)，原始属性转储和反汇编位于 `tmp/school-rule-schedule-20261023/`。

## 时间不是墙钟秒数

模拟配置 Property `00B1B104:40800200:622B9CD7` 中：

| 字段 | 安装包值 | 含义 |
|---|---|---|
| `07E0AC04` | 2 | 每 game second 的模拟 tick 数；不是当前实际每秒运行速度 |
| `083950BC` | `12952639`、`54BA27CD`、`9B8BC62E`、`D5A08F93` | 四个周期 ID |
| `083950BD` | 720、720、8640、30 | 对应周期的 game seconds 长度 |
| `083950BE` | 0.29166666、0.29166666、0.024305554、0 | 对应初相位 |

`FNV-1("day")=12952639`，`FNV-1("hour")=D5A08F93`；安装版 UI 也定义了同值的 `kResourceIDHour`。周期 ID 和资源 ID 即使数值相同，也应保存各自的使用语境。

开发版带符号反编译 `SetTimePeriodsFromSimTicks`（VA `005F2BA0`）及本机零售版 `005CF4D0` 的运算相符：

```text
elapsedPeriods = simTicks * periodInvLength + phase
periodCycles = floor(elapsedPeriods)
periodTime = elapsedPeriods - periodCycles
periodInvLength = 1 / (simTickRate * periodLength)
```

零售版读取 game+17A0 的 tick、+18B4 的倒数周期长度数组、+18C4 的相位数组，写 +18E4 周期数和 +18D4 周期内时刻。day 因此是 **1440 tick**，hour 是 **60 tick**。在这套配置下，1 tick 对应 1 个模拟分钟，day 的初相位相当于 07:00。

据此，tick 46139–46169 对应约 **07:59–08:29**，并不是用户先前报时的 07:22。这是安装配置和已有 tick 推导出的时刻，尚未与同一帧 HUD 校准；专用配置覆盖、相位变化和浮点取整仍须保留。工具按最近秒显示，避免单精度初相位让整分钟显示成上一分钟。

## 解码出的调度结构

RuleBase 的第三个大端 int32 是数据池内 `SimInfo` 偏移；-1 表示使用默认结构。数据池本身按小端读取。零售 `cEcoSimInfo` 构造函数 `005FBC10` 与开发版 `00621140` 的字段写入位置一致，结构长度为 68 字节：

| 偏移 | 字段 |
|---|---|
| +00 / +04 / +08 | priority / simRate / simOffset |
| +0C..+14 | 三个时间范围对应的 period ID |
| +18..+2C | 三组 float 起止范围 |
| +30 / +34 / +38 | triggerTime / triggerPeriodID / triggerCount |
| +3C / +40 | triggerVariance / chance |

开发版 `UpdateUnitRulesToRun`、`TimeTrigger`、`SetNextTriggerTime` 显示：周期触发与递减 simCounter 是两种入口；触发后还要检查时间范围、概率、控制状态及资源可执行次数。周期触发的 nextTriggerTime 按一个周期递增；范围还支持跨周期边界。详细调度分支以开发版符号解释为依据，本轮没有逐分支证明零售实现完全相同。

**默认 simRate=16 不代表所有引用默认结构的规则每 16 tick 自动运行。** EventRule、sink 接收规则和成功/失败链还由不同入口调度。必须同时解析 Property 中的绑定类别。

## 小学的三种规则入口

### 1. 入校是 sink 接收事件

教育 bundle `40E0C700:6C042367`：

```text
0A2725CE → sink entry 78B96709
0A2725CF → rule B52C889D
```

开发版 `cEcoUnitEntry::AddBundle` 对这两个属性调用 `GetPropertiesAsTable`，构造 `mSinks` 并解析 sink/rule 索引。结合前轮运行身份确认，这是比“看到一条规则 ID”更完整的绑定证据。

`B52C889D` 每次接收的基础操作是：nearby KidE2 -1，本地 Student +1、TotalStudents +1、StudentStudyingE2 +1、SpotsForKids -1；同时更新全局学生统计。FromTradingConnection 数量由 fromBinOps 替换。

已留存的零售日志提供两个独立窗口：

| 日志窗口 | tick / 推导时刻 | 观测 |
|---|---|---|
| `live-elementary-id-rules-20261023-r2` | 44787–44790 / 09:27–09:30 | 27 次入口，其中同一 tick 44788 有 21 次 |
| `live-elementary-id-effects-20261023-r2` | 44874 / 10:54 | 同一 tick 内 18 次资源更新，学生 835→853、席位 115→97 |

这不是从一个窗口推断另一个窗口的执行次数。它直接说明多人到达可以在同一模拟 tick 批量处理，与 spawner 点位数没有一人对应一点的关系。

### 2. 学校有按日和按小时触发的任务

同一个教育 bundle 的 `08080D58` 引用：

| Rule ID | 基础触发配置 | 资源意义 |
|---|---|---|
| `C193A5BB` | day 的 0.25，即 06:00 | 通过 fromBinOps 读取 TotalStudents，清零此统计；不能解释为驱逐所有在校学生 |
| `40313FC2/C1/C4/CB`、`413141B7/B0/BD`、`3E313CFD` | day 的 0、0.125……0.875 | 分别检查 StudentsNeeded_00/03/06/09/12/15/18/21，并处理 ExamsPassedE2、成功/失败分支 |
| `23E737BD` | hour 的 0，variance=0，triggerCount=0 | 检查 C02F343B 和剩余席位，启动相关资源协调逻辑 |

**八个定时规则不等于小学全天有八次实际开课。** 学生需求 bin、建筑状态与后续分支决定哪些时段有效；原版小学的完整开课/放学链尚未闭合。

最强的时间对拍来自旧 effect 日志：`23E737BD` 在 **tick 44880 = 11:00** 被观察到。该 tick 还执行 `94BD11EA`、`6EAB3D39`；`8D084DB8` 在 44883 = 11:03 返回。这个时序与整点触发一致。后几条的资源流相连，但仅凭先后关系不能宣称已证明全部因果调用链。

### 3. 水电是经 Property 专用化的周期消费

小学父级 `48E0C000:2A25CF66` 的 bundle 引用中包括：

| bundle / rule | 基础 SimInfo | Property 专用化 | 一次消费输入 |
|---|---|---|---|
| `48939D96` / `D51CB94C` | simRate=30 | prop `0AFB9882`，小学本体为 72 | fromBinOps 读取 PowerNeeded，采样值 90 |
| `825B4CA4` / `D54F8B18` | simRate=72 | prop `0C09DA6A`，小学继承值 72 | fromBinOps 读取 WaterNeeded，采样值 90 |

本轮还解出了 ERZ 的 UnitRulePropSet：映射 rule ID→prop set，记录20字节、大端序。零售 `005FF840` 与开发版 `ReadPropSet` 对照确认序列化布局。上述两条 opId=0，表达式都是 `1 * property + 0`；开发版 `SpecialiseRuleBase` 将 opId=0 写到 SimInfo+4，即 simRate。

因此这两个规则在标准小学属性下都得到 **72 tick 的检查间隔**，配置折算约 **72 模拟分钟**。这是检查节奏，不是保证每72分钟一定成功消费；控制状态/资源条件仍可阻止执行。

旧快照在 tick **46164 ≈08:24** 同时看到水电 **270→180**，水同 tick 恢复到270，电在46165恢复。90恰好等于两项 Needed 资源。静态规则与采样吻合，支持“消费后补给”的解释；由于该窗口没有对应规则入口/效果记录，还不能把这次下降最终归因到 `D51CB94C/D54F8B18`。前一报告中“更像供给刷新”的措辞过强，端点净变化为0不能排除消费。

## 对 OpenSCP 的具体意义与下一步

1. **规则检查器**可新增三类入口：周期/时刻、事件/sink、规则链；显示基础值和 Property 专用化值。电力规则若只显示基础30，就会误导资产作者。
2. **运行时工作台**可按配置派生时钟，但必须标注“推导时间/未校准”，并保留 game、tick、slot、ruleId；同 tick 内多次事件不能合并丢失。
3. **资产编辑校验**可追踪间隔属性、Needed资源、容量、fromBinOps、供给入口的闭环；基础消费量999999是动态占位值，不应直接暴露为实际消费量。
4. **下一步验证**优先低频进程外只读：同时读取 period ID/length/phase、专用化后的两条消费规则和 simCounter，覆盖至少两个72 tick周期；再核对每次减少90与计数归零是否一致。必要时才使用单会话、单建筑的短时效果日志补齐因果证据。
5. 继续沿06:00规则的成功/失败链分析开课与放学状态，再补维护扣款、劳工到达/离开、污水生成与排出。维护费300的稳定 bin 不是“这段时间没扣钱”的证明，污水快照为0也不能证明没有短暂生成。

当前探针扫描38个属性节点，汇总198条引用/历史观测/对照规则；它是候选引用图，**不是运行时已启用规则清单**。它没有实现 bundle exclusions、合并覆盖、DLC优先级或所有专用化效果，不能据此直接写回建筑资产。

## 复现

```powershell
python probes/runtime/school_rule_schedule.py --package 'D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityUserData\EcoGame\SimCity-Scripts_287520926.package' --binary tmp/live-school-installed-main.bin --trace tmp/live-elementary-id-rules-20261023-r2/trace.jsonl --trace tmp/live-elementary-id-effects-20261023-r2/trace.jsonl --exe 'D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCity\SimCity.exe' --out tmp/school-rule-schedule-20261023
python -m unittest discover -s probes/runtime -p test_school_rule_schedule.py -v
```

新增探针位于项目 `probes/runtime/`；调用已有 prop_dump 读取指定包，不修改游戏、存档或 UI。
