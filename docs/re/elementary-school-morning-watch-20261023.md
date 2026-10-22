# 小学单建筑早间运行观测（2026-10-23）

本轮使用进程外 `ReadProcessMemory`，没有加载 Frida、没有修改游戏入口，也没有调用游戏函数。用户报告观测开始时游戏时间为 7:22am；探针没有读取 HUD 日历，所以 7:22am 作为外部时间标记保存，不能宣称已完成游戏时钟校准。

## 目标和质量

- PID `58376`，零售版 `SimCity.exe` SHA-256 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。
- 唯一匹配 `UnitEntry=0xC264A268` 的小学：`unitSlot=802`。
- 60 秒、间隔 0.5 秒，120 个快照；模拟 tick `46139 → 46169`。每个快照重验证游戏单位向量、小学 UnitEntry、资源数组边界和脚本对象指针。
- 0 读取错误；目标进程仍存活；未发现新的 SimCity WER 1000 事件。
- 限制：快照不是原子读取，只能得到单位资源的净变化，不能单独归因于某条 ER2 规则；没有读取 UI HUD 时间。

原始证据：`tmp/school-morning-watch-20261023/{snapshots,status,resource-map,resources-summary}.*`。探针：`probes/runtime/school_resource_watch.py`。

## 资源映射和变化

本轮读取 `cEcoGameScripts` 的有序资源映射表（`scripts+0x90`，6672 个 `<resourceId, resourceIndex>` 对），映射摘要见 `resource-map.json`。当前小学资源变化如下：

| 资源 | resourceIndex | 首值 → 末值 | 变化 |
| --- | ---: | ---: | ---: |
| `ExpensesFromMaintenance` | 5501 | 300 → 300 | 0 |
| `WorkersNeeded` | 4512 | 5 → 5 | 0 |
| `WorkerW1` | 1910 | 5 → 5 | 0 |
| `JobsW1/W2/W3` | 3829/3830/3832 | 68/13/0 → 68/13/0 | 0 |
| `Power` | 3573 | 270 → 270 | 中间最低 180 |
| `PowerNeeded` | 431 | 90 → 90 | 0 |
| `Water` | 1117 | 270 → 270 | 中间最低 180 |
| `WaterNeeded` | 650 | 90 → 90 | 0 |
| `Sewage` | 5526 | 0 → 0 | 0 |
| `Garbage` | 675 | 0 → 0 | 0 |
| `Germs` | 1398 | 80 → 77 | -3 |
| `Student` | 303 | 186 → 266 | +80 |
| `TotalStudents` | 729 | 186 → 266 | +80 |
| `SpotsForKids` | 3547 | 764 → 684 | -80 |
| `StudentStudyingE2` | 1757 | 186 → 266 | +80 |
| `C02F343B`（未命名内部资源） | 2653 | 908 → 764 | -144 |

安装包符号和资源 ID 的精确对应见 [elementary-resource-symbols-20261023.json](D:/rust/packages/fluffy-open-scp/docs/re/evidence/elementary-resource-symbols-20261023.json)。学生数与剩余席位始终满足 `Student + SpotsForKids = 950`，说明本轮处于满配小学的入学/占座阶段。

## 对行为和 ER2 的解释

学生增加 80、席位减少 80；此前小学规则 `0xB52C889D` 的运行时对拍已确认每次应用会写入 `Student+1`、`TotalStudents+1`、`StudentStudyingE2+1`、`SpotsForKids-1`。本轮只观察结果，没有把快照差值伪装成规则调用。

小学有 `PowerNeeded=90`、`WaterNeeded=90`，而 `Power` 和 `Water` 容量均为 270。两者短暂从 270 降到 180，又回到 270；首尾库存净变化为0，但这不能排除先消费再补给。后续[规则调度分析](elementary-school-rule-schedule-20261023.md)确认小学引用的水电消费规则以 Needed 资源替换输入量，并由 Property 将两者检查间隔设为72 tick。这支持“消费90后补给”的解释；该窗口未采集规则入口，具体下降的因果归属仍待验证。按安装配置推导，此窗口约为07:59–08:29，尚未同步HUD校准。

`Sewage=0` 和 `Garbage=0` 在整段保持不变；学校静态垃圾属性 `kPropGarbageProducer_GarbageOnlyAmount=10` 仍成立，但垃圾生产不是每个模拟 tick 执行。污水或垃圾可能由其他单位、全局统计或低频规则处理。

`WorkersNeeded=5`、`WorkerW1=5`、`JobsW1=68`、`JobsW2=13`、`JobsW3=0` 和 `ExpensesFromMaintenance=300` 全程稳定。它们确认了当前单位的劳工和维护状态，但没有证明招聘路径或每小时扣费。

本轮建立的桥梁是：

```text
游戏日历时间（用户提供 7:22am，未由探针校准）
  → Unit 802 的定时/条件规则
    → Unit-local bins：学生、席位、劳工、水/电、污水、垃圾、维护费
```

下一步需要在单会话、slot 802 定向的短规则窗口中，把这些已映射资源的变化对拍到具体 ER2 ruleId；外部快照本身不执行规则，也不会引入 Frida 崩溃风险。
