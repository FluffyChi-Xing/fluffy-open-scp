# 医院、小学与 OmegaCo 的 ER2 静态脚本分析（2026-10-23）

## 结论先行

可以从 EcoGame 提取 OmegaCo 的 ER2 **编译二进制规则**。当前安装版的 EP1 脚本包有 `0x08068AEB` AEB 资源，最新目标资源是 `SimCityDLCEP1_AEB_1082130945_95409789_0011AAB1.bin`，结构为 `5.2.14`，包含 5,485 条 UnitRule，能够精确解析到文件尾。EP1 本轮没有发现配套的 `0x08068AEC` AEC 文本资源，因此可以静态分析规则索引、资源操作和 `fromBinOps`，但不能恢复 OmegaCo 原始 ArgScript 的规则名和语句。

主游戏规则库 `tmp/rule-main-287520926.bin` 同样是有效的 `5.2.14` AEB，包含 28,441 条 UnitRule。医院和小学的规则来自这个规则库；OmegaCo 的规则来自 EP1 AEB，不能把两个 AEB 的 rule index 直接混用。

完整机器可读证据见 [er2-static-three-unit-analysis-20261023.json](evidence/er2-static-three-unit-analysis-20261023.json)。本轮使用的静态解码器是 [unit_rule_resource_bridge.py](../../probes/runtime/unit_rule_resource_bridge.py)，它只解码 UnitRule 资源前缀和 `fromBinOps`，控制流、AgentTests、TransportOps 和零售版运行时特化仍是未知字段。

## 医院：先确认病患与治疗字段

医院 UnitEntry 是 `0x30236D71`，属性资源为 `00B1B104:40E0C000:30236D71`。主体连接 `0x61C1B2DF` 和空的 `0x9A1BFB2F` bundle。医院 bundle 的规则数组如下：

| 属性 | 内容 |
|---|---|
| `0x08080D58` | UnitRule：`84040876`、`480E93D6`、`F900A7CC` |
| `0x08327D49` | EventRule：`15BCBB80`、`E5A0ED31`、`F08EAF84`、`9D7F0573` |
| `0x08327D4D` | EventRule：`5FC19245`、`DEBED60C`、`75A4D02F`、`6126FF9F` |

静态资源操作能直接读出三类关系：

- `0x84040876` 的 nearby 操作是未知资源 `0xECCD9036`，基础输出 `999999`；`fromBinOps` 从 scope 0 读取 `WaitingRoom (0x2903C3D3)` 的剩余容量，并以 `writeMode=2` 写入目标操作。也就是说，医院服务规则的数量不是固定常量，至少有一条等待室容量来源。
- `0x480E93D6` 对本地未知资源 `0x0DCC49E8` 做 `1 → 0`。
- `0xF900A7CC` 对本地 `0xD374C4A4` 和 `0x01E7D2AE` 做动态替换；其中后者同时作为目标操作 1 的写入来源和目标操作 0 的动态输出来源。

医院锚点的运行时资源索引已经确认：`Germs=1398`、`Sick=3482`、`SickW1=5317`、`SickW2=5313`、`SickW3=5315`、`SickHealing=2432`、`HospitalRooms=4067`、`WaitingRoom=5756`、`TotalWorkers=2293`、`WorkerW1=1910`。运行时规则 `0x581CE1E0` 在医院实例上把 `SickHealing` 从 `0→1`、`3→4`，因此 `2432` 是实际治疗量槽位；`WaitingRoom` 在医院实例本地 bin 中以容量 10 出现。

这一步确认了后续联动观察所需的索引，但静态规则暂时不能把 `0xECCD9036`、`0x0DCC49E8`、`0xD374C4A4` 或 `0x01E7D2AE` 直接命名为病患、招聘或医院容量。命名必须继续用运行时 effect/provenance 对拍。

## 小学：规则定义了学生、席位和学习状态链

小学 UnitEntry 是 `0xC264A268`，属性资源为 `00B1B104:40E0C000:C264A268`。主体连接教育 bundle `0x6C042367`、`0x3A1C3971`、`0x95573B5D`、`0x9A1BFB2F`，排除容器是 `0xF90C8A94`。

已由符号表确认的资源 ID 是：

| 资源 | Resource ID |
|---|---:|
| Student | `0x2C1DA112` |
| TotalStudents | `0x06EDFBED` |
| SpotsForKids | `0x05DAADA6` |
| StudentStudyingE2 | `0xCF0B7832` |
| FromTradingConnection | `0xFF635EE8` |

`0x6C042367` 的 `0x08080D58` 数组包含 10 条本地 UnitRule。静态资源操作显示：

- `0xC193A5BB` 将 `TotalStudents` 写为 `0` 的关闭/清空路径。
- `0x40313FC2`、`0x40313FC1`、`0x40313FC4`、`0x40313FCB`、`0x413141B7`、`0x413141B0`、`0x413141BD`、`0x3E313CFD` 都是 `1→1` 的状态维护操作，同时把未知资源 `0x09AEFD2E` 作为动态来源写入第二个操作。
- `0x23E737BD` 同时增加未知教育资源 `0xC02F343B` 和 `SpotsForKids`，并从 global 的 `0xBE2C0EB3` 读取动态值。

教育 EventRule 数组还包含 `0xBA80A329`/`0xE9EE9534`：它们同时操作 `0x219D6623` 和 `SpotsForKids`，并保留 global 侧的 `0x81BE2AA7`。这些结构与运行时观察到的学生、席位、学习状态同步一致，但不能只根据 `1→1` 把每次规则应用解释成一名学生或一节课。

## OmegaCo：基础产线、扩展产线和资源操作已经分开

OmegaCo UnitEntry 是 `0x071D31E5`，属性资源为 `00B1B104:61EFC000:071D31E5`。属性连接 10 个 bundle，其中核心静态链可以分成四组：

| 功能证据 | Bundle / Rule | 静态操作 |
|---|---|---|
| 基础 Omega 工厂 | `0x6B116B35` / `0x645B10E4` | 本地 `Ore (0x96CF3B07)` 和 `Oil (0xBF75DC04)` 都是 `input=3/output=3` 检查型操作；另有三个 nearby 条件资源 `1→1` |
| 处理器/电子扩展候选 | `0xD4666039` / `0x88DE7AA1` | `0xB935C1D9` `1→0`，`Processor (0xBEE1C73E)` `0→1` |
| 采油/采矿库存桥 | `0xD4666039` / `0xF2C1A0FD`、`0xF776E9FA` | `0x61FFD1B5` `1→0` 后写入 Oil；`0x950D6372` `1→0` 后写入 Ore |
| 无人机/容量扩展候选 | `0x81A602E3` / `0x2EAF95E6` | 操作 `0x41B4C16F` `0→-1`，并对 `0x2CFE923B` 做容量型清理；该 ID 已知是电子贸易容量追踪字段，不等于处理器库存 |
| 隐藏双缓冲/转运 | `0x6B116B35` / `0xCA5B8861`、`0xDE3EC0F9`，以及主体 bundle `0xC935DD44`、`0x28DC2213` | `0x292876D2`/`0x7F26603F`、`0x7727423A`/`0x72FC8C95`/`0x15811FF7`，以及 `0x11A2434E`/`0x8E25D7FB`、`0xF40144D9`/`0xCE415E6A` 之间存在动态搬运和清空关系 |

OmegaCo 属性本身还给出了 `Oil`、`Ore`、`Processor` 的资源引用，以及原油/原矿容量 `60000`。`0x645B10E4` 的 `3→3` 不能解释为消费 3 单位；它是输入输出相等的检查/配额推进操作。真正的库存变化来自其它规则和动态特化。

`0x88DE7AA1` 确实含有 Processor 的 `input=0/output=1`，但这只是一次规则操作的基础系数。它没有给出无人机产量，也没有证明每个 OmegaCo 都装配了该扩展。`0x2EAF95E6` 位于独立 bundle，进一步支持“无人机是扩展产线”的判断。要得到处理器→无人机比例，必须把该 bundle 的目标 Unit、实际 `applyCount`、Processor before/after 和无人机成品资源在同一运行时窗口闭合。

同样，静态规则没有把某个资源 ID直接标成 Omega 成品。原油、原矿与隐藏双缓冲之间存在明确的规则操作，但目前只能确认生产链的结构，不能从静态系数计算 Omega 每小时产量或原油/原矿配比。

## ArgScript 语法节选与二进制解析的对应关系

下面的代码块先说明发行版文本规则通常长什么样，再把本轮 AEB 中已经解码的资源操作翻译成接近作者视角的写法。医院、小学和 OmegaCo 的这些代码块都不是从发行版逐字恢复的原文：本轮只提取到了主规则库和 EP1 的 AEB 二进制，EP1 没有对应 AEC 文本。因此，代码块中的名称是已确认的资源符号或 `0x...` 规则 ID，带“重建”标记的行只表达二进制语义。

### 基本语法

仓库中实际提取到的 AEC 文本使用以下几类结构：

```er2
unitRule SomeUnitRule
    local SomeResource in 1
    local SomeOtherResource out 1
    global CityCounter out !CityCounter@global
    applyCount 1
    onSuccess SomeUnitRule_Next
    onFail SomeUnitRule_Stop
end
```

`unitRule` 是规则块，后面的 `local`、`nearby`、`map`、`global` 是资源查找作用域；`in` 表示输入/消耗条件，`out` 表示写入。`applyCount` 控制一次规则允许应用的次数，`onSuccess`、`onFail` 和 `chain` 连接控制流，`!Name@scope` 表示从另一个作用域读取动态数量。编译后，这些行不再保留原始名称，而是进入 UnitRule 的资源前缀和 `fromBinOps` 数组。

二进制解码结果可以按下面的方式阅读：

```text
local   (resourceId, input, output)
fromBinOps(sourceScope, sourceResourceId,
           targetScope, targetOperation, writeMode)
```

例如 `(0x96CF3B07, 3, 3)` 是“原矿操作的输入和输出字段都为 3”，属于检查/配额推进候选；它不能直接翻译成“消费 3 单位”。`writeMode=2` 表示目标操作的基础输出会被来源 bin 的动态数量替换，这就是 `out !SomeResource@scope` 在编译规则中的一个常见落点。

### 医院：等待室动态数量与治疗写入

医院 bundle `0x61C1B2DF` 的 UnitRule 数组是 `0x84040876`、`0x480E93D6`、`0xF900A7CC`。其中 `0x84040876` 的语义可写成：

```er2
# 重建节选：不是发行版原文
unitRule 0x84040876
    nearby 0xECCD9036 out !WaitingRoom@local
    # fromBinOps: scope0 WaitingRoom(0x2903C3D3)
    #             -> scope1 operation0, writeMode=2
end
```

这里 `WaitingRoom` 的运行时索引是 `5756`，所以静态的 `999999` 不是实际治疗数量。它只是目标操作的基础值，运行时由等待室 bin 覆盖。医院运行时规则 `0x581CE1E0` 还实际写入 `SickHealing`（索引 `2432`）：

```er2
# 由运行时 effect 与静态字段对拍得到的语义节选
unitRule Hospital_Heal
    local SickHealing out 1
end
```

`0x480E93D6` 的 `(0x0DCC49E8, 1, 0)` 和 `0xF900A7CC` 的两个动态本地操作暂时没有正式符号，不能把它们直接改名为“病患清除”或“招聘”。

### 小学：学生、席位和教育状态

小学 `0x6C042367` bundle 的 10 条本地规则包含多个 `1→1` 状态维护操作。已确认的资源符号可以用作者视角表示为：

```er2
# 重建节选：资源名称来自符号表，控制流名称未知
unitRule PrimarySchool_Study
    local Student out 1
    local TotalStudents out 1
    local SpotsForKids in 1
    local StudentStudyingE2 out 1
end
```

本轮直接解码到的 `0x23E737BD` 更精确地是：

```text
local  0xC02F343B: (1, 1)
local  SpotsForKids(0x05DAADA6): (1, 1)
global 0xBE2C0EB3: (999999, 0)
fromBinOps: scope3/global 0xBE2C0EB3 -> local operation0
```

因此上面的 `Student`/`StudentStudyingE2` 是教育链的语义概览，不是声称它们都由同一条 `0x23E737BD` 写入。小学事件规则 `0xBA80A329` 和 `0xE9EE9534` 还会同步操作 `SpotsForKids` 与另一组 global 资源；要把它们命名为“入学”“放学”仍需结合 Agent/Transport 日志。

### OmegaCo：原油、原矿、处理器和扩展产线

OmegaCo 的基础检查规则 `0x645B10E4` 可以翻译成：

```er2
# 重建节选：input=output=3，不能解释成净消费
unitRule OmegaCo_CheckInputs
    local Ore in 3
    local Oil in 3
    nearby 0x44F5CF6C in 1
    nearby 0x7A19A243 in 1
    nearby 0x18867D33 in 1
end
```

对应 AEB 资源前缀是：

```text
local Ore(0x96CF3B07): (3, 3)
local Oil(0xBF75DC04): (3, 3)
nearby 0x44F5CF6C / 0x7A19A243 / 0x18867D33: (1, 1)
```

处理器候选规则 `0x88DE7AA1` 的二进制操作是：

```er2
# 重建节选：位于独立扩展 bundle
unitRule OmegaCo_ProcessorExtension
    local 0xB935C1D9 in 1
    local Processor out 1
end
```

这只能证明规则存在一个 Processor `0→1` 基础操作；`applyCount`、动态特化以及无人机成品资源还没有闭合，所以不能把它解释成“一个处理器生产一架无人机”。该规则所在的 bundle 与基础 OmegaCo bundle 分离，符合无人机线是扩展模块的证据。

OmegaCo 还有两类由 `fromBinOps` 表达的缓冲/转运规则。以 `0x95BA3AA1` 为例：

```text
local  0x11A2434E: (999999, 999999)
local  0x8E25D7FB: (999999, 0)
fromBinOps: 0x11A2434E -> operation0,
            0x8E25D7FB -> operation1(writeMode=2)
```

这类结构说明“基础输出会从另一个 bin 动态取得”，但没有给出资源的人类名称。隐藏资源、Omega 成品、原油/原矿实际消耗量必须继续用运行时的 `applyCount` 和 bin 前后快照确认。

## 与运行时观察的合并结论

目前可以安全写入分析器的结论是：

1. **ER2脚本可提取**：OmegaCo 的 EP1 AEB 已提取并精确解析；医院、小学和 OmegaCo 的属性 → bundle → ruleId → 资源操作链可以复验。
2. **医院是有效索引锚点**：`SickHealing=2432` 已由医院运行时规则确认，医院静态规则还显示 WaitingRoom 容量参与动态数量。
3. **小学和 OmegaCo 都有动态数量来源**：小学的 `SpotsForKids`/教育状态和 OmegaCo 的产线库存都不是单纯固定常量；`fromBinOps` 会替换基础输出。
4. **OmegaCo 的基础和扩展行为是分开的**：处理器/无人机候选规则位于独立 bundle；不能把所有工厂都当作有无人机线。
5. **生产关系仍需运行时闭合**：`input/output` 是脚本配置；要建立比例，必须记录 `applyCount`、动态替换后的操作、具体 Unit/slot 和资源 bin 前后值。

下一轮应把医院 `SickHealing`、OmegaCo `Germs/SickW1/TotalWorkers`、Oil/Ore/Processor，以及候选隐藏双缓冲资源放在同一 simTicks 时间轴上，跨越至少两个 germs 清除/再生周期；只有这样才能检验“细菌增长 → 病患工人 → 劳工减少 → 招聘补充”的因果链。
