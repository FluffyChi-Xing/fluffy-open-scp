# 医院资源索引确认与 OmegaCo 联动观察（2026-10-23）

本轮先使用运行中的医院单位确认资源表索引，再对 OmegaCo 工厂做串行观察。目标进程 PID 为 `28160`，游戏指针为 `0x264c47c0`，EXE SHA-256 为 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。

## 1. 医院单位和索引

普通医院的静态 UI rollover `unitID` 与运行时 UnitEntry 对应为 `0x30236D71`。当前城市中观察到 9 个实例：slots `279、300、338、447、558、734、786、838、850`。每个实例有 213 个本地 bin。医院治疗规则的直接运行时证据来自 slot 838：规则 `0x581CE1E0` 在 60 秒捕获中两次将 `SickHealing` 从 `0→1`、`3→4`，资源索引均为 `2432`，无丢弃、无错误。

资源表确认结果如下。`resourceIndex` 来自本次进程的运行时资源表；“医院本地出现”表示该字段是否在 `0x30236D71` 的 213 个 bin 中出现。

| 字段 | Resource ID | resourceIndex | 医院本地出现 |
|---|---:|---:|---|
| Germs | `0x7B5637C5` | `1398` | 否 |
| Sick | `0x2AFEAA77` | `3482` | 否 |
| SickW1 | `0xCB9E50B7` | `5317` | 否 |
| SickW2 | `0xCB9E50B4` | `5313` | 否 |
| SickW3 | `0xCB9E50B5` | `5315` | 否 |
| SickHealing | `0xA3D2837F` | `2432` | 是 |
| InjuryHealing | `0xA625?` / `0xA644B38E` | `2140` | 是 |
| HospitalRooms | `0x0F3D5B01` | `4067` | 否（医院建筑统计字段） |
| WaitingRoom | `0x2903C3D3` | `5756` | 是 |
| TotalWorkers | `0x1FD156B6` | `2293` | 否 |
| WorkerW1 | `0x6CCC3A0F` | `1910` | 是（医院服务人员状态） |
| Water | `0x9E3C3DFA` | `1117` | 是 |
| Power | `0x8E8B4E10` | `3573` | 是 |
| Oil | `0xBF75DC04` | `4582` | 否 |
| Ore | `0x96CF3B07` | `500` | 否 |
| Processor | `0xBEE1C73E` | `2460` | 否 |

`InjuryHealing` 的完整静态 ID 是 `2788082702`（十六进制 `0xA644B38E`）；上表中的 `0xA625?` 只是上一轮摘要中的未闭合占位，不能继续使用。完整机器可读结果见 [`hospital-index-confirmation-20261023.json`](evidence/hospital-index-confirmation-20261023.json)。

## 2. 医院治疗证据

`tmp/hospital-30236d71-sickhealing-20261023/trace.jsonl` 记录了规则 `0x581CE1E0` 的本地资源写入。两次事件都包含：`SickHealing`（index `2432`）`input=0/output=1`，净增 `+1`；伴随其它医院状态字段同步更新。这确认 `2432` 是治疗量槽位，而不是仅由 UI 推导的显示值。`WaitingRoom`（index `5756`）在医院实例中以容量 10 的本地 bin 出现。

## 3. OmegaCo 120 秒观察

原始快照：`tmp/omegaco-hospital-indexed-watch-20261023/`（45 个样本，ticks `53006→53096`，无错误）。原始 effect：`tmp/omegaco-hospital-indexed-effects-20261023/`（8 条，`errors=0,dropped=0`）。

- 六个 OmegaCo slots 仍为 `63、70、170、418、485、561`。`Germs` 的 index 是 `1398`；slot 63/70/170 保持 `14/99/93`，slot 418 从 `93→100`，slot 561 从 `93→100`，slot 485 保持 `100`。
- 这一窗口首次在工厂本地快照观察到病患：slot 485 的 `Sick=0→2`，其中 `SickW1=0→1、SickW2=0→1`；slot 561 的 `Sick=0→2、SickW1=0→2`。slot 70 的 `TotalWorkers=183→181`，但其它工厂的总劳工在该窗口保持不变，不能将变化直接归因于生病离岗。
- `SickW1` 的 effect 捕获给出两类联动：规则 `0xB5A0D2DC` 在 slot 63/170 将 `SickW1`、`Sick` 和 `TotalWorkers` 同步扣减（例如 `3→0`、`Sick 3→0`、劳工 `-3`）；规则 `0x99A66118` 在 slot 63/170/485 将 `SickW1` 与 `Sick` 增加，并同步扣减未知资源 `0x6106B1CB`。这支持“病患状态和劳工槽位存在同步迁移”，但未知资源的语义以及招聘补充规则尚未闭合。
- `Germs` 的 effect 捕获（45 条，`errors=0,dropped=0`）在所有六个工厂上观察到：规则 `0xCF317CDC` 清除当前 germs（例如 `93→0`、`100→0`）；规则 `0x474C0A4A` 与资源 `0x67344592` 同步把 germs 写回 `0→100`（或 `0→43/68`）；规则 `0x4CAF41F5` 对 germs 做 `input=10/output=10, delta=0` 的检查。当前能确认“细菌会周期性被清除并重新生成/填充”，不能把 `0x67344592` 命名为污染源，也不能仅凭此窗口证明医院清除导致某一个工厂的 germs 下降。
- 原油 index `4582`、原矿 index `500`、处理器 index `2460` 已在同一快照中同步记录。slot 63 的原油/原矿各 `58000→57700`；slot 170 原油 `54100→51850`、原矿在补货后仍在 `49200..59200`；slot 418 原油 `54550→49600`、原矿 `49700→54750`；slot 485 原油/原矿各 `52000→50200`；slot 561 原油 `54900→48150`、原矿 `49250→52500`。处理器只在 slot 70 观察到 `58180→58045`，其它槽位为 0；这仍然不足以推出“1 处理器=1 无人机”。
- 水 index `1117`、电 index `3573` 的本地库存反复出现固定消费与补充：水容量从 `3024` 短暂降至 `2016`，电从 `28485` 降至 `18990`。当前 120 秒 effect 采样恰好只命中 slot 485 的电规则 `0xFFD3003E`，记录了 `+9495` 的补充写入；这与上一轮确认的公用事业消费链一致。

## 4. 当前结论和下一步

已经满足“先通过医院确认索引，再开始观察”的前置条件。现在可以可靠使用以下索引做后续联动序列：`Germs=1398`、`Sick=3482`、`SickW1=5317`、`SickW2=5313`、`SickW3=5315`、`SickHealing=2432`、`HospitalRooms=4067`、`WaitingRoom=5756`、`TotalWorkers=2293`、`WorkerW1=1910`。

仍未闭合的部分是：医院对某一 OmegaCo 实例的空间/服务归属、`0x6106B1CB` 的语义、招聘补充字段、Omega 成品字段及处理器到无人机的比例。下一轮应把医院 `SickHealing`、OmegaCo `Germs/SickW1/TotalWorkers` 和资源消费放在同一时间基准下，并扩大窗口到跨多个 germs 周期，才能检验“细菌增加→病患劳工→劳工减少→招聘补充”的完整链条。
