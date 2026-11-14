> **总入口**：`docs/re/engine-contracts-and-port-status-20261111.md`（2026-11-11 汇总，结论以它为准；本文为过程与逐包数据）。
>
> **行为复核见 §12**。重新验哈希并解包后确认，`23fafa95` 的地标 parent 未把观光 bundle 带进有效属性表。§9、§10 保留当时的推断，实际有效 bundle、招工 sink 和 UI 判据以 §12 为准。
>
> **作者实现对照见 §13**。用户提供的缓存含新一代观光配方，§12.4 的版本边界专指仓库 vendored 0.8.0。§7.6 对 HTzhiyao 只数了 A5，完整引用需按 §13.3 阅读。

# 引擎侧劳工机制定案 + 希望大厦历次填工对照分析（2026-11-11）

本文沉淀两件事：

1. **引擎侧劳工机制的源码级定案**——直接读 SimCity 2013 Dev Beta 反编译源码
   （`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree`，约 4.2 万个
   CodeDumper 函数文件）逐函数核对，闭合"package 列 → bin → sink 容量 → 到岗人
   数 → UI 红字"的完整链路。此前多轮基于 package 对拍的推断有**三处被推翻**
   （见 §4）。
2. **希望大厦历次构建的填工数据对照**——把 test-archive 里 5 个历史包逐字节解出
   真实 labor 列，与游戏内观察逐项对照。

本轮**未改引擎外的东西之外**：代码侧只改 `behavior_nodes.rs` 的 AuthorOnly 工人槽
缺省策略（§5）；数据侧改了项目 schema 的 labor 段。

---

## 0. 结论速览

| # | 结论 | 证据强度 |
|---|---|---|
| 1 | 「需要劳工」红字 = `TotalWorkers` bin 的 amount < `MinimumWorkersForProduction` 属性值。**与岗位填充率无关**。威利斯 105 人 / 360 岗（29%）不红，只因 105 > 它的 minWorkers 60 | 源码 + UI 数据双证 |
| 2 | `kPropWork_TotalJobs`（`0x0A7917C3`）在引擎里**零引用**，纯 UI/统计列 | 全树 grep |
| 3 | 岗位属性 → bin 的桥是**被引用 bundle 上的属性→bin 映射表**；写在 Unit/Lot 自身（core bundle）会被静默跳过 | `AddBundle.c` 0x6490B0 |
| 4 | 工人槽 `0x0E3F65A9/B3/B4` 初始化 `WorkersNeededW1/W2/W3` bin。具体接收容量取决于各 sink 的有效 key，见 §12.3 | bundle 映射表 + sink 条目 + `SinkCapacity.c` |
| 5 | 非 connected sink 通过测试后，缺 key bin 时返回 10。历史样本是否走了这个分支仍待运行取证 | `SinkCapacity.c` 0x616BF0:157 |
| 6 | 引擎**没有**财富档过滤、没有"按距离/吸引力选楼"；工人目的地来自住宅规则 `mTransportOps` 的 sink entry 模板列表，按数据顺序逐个试 | `GetInitialDestinations` 0xB6D3E0 |
| 7 | 排班 8 列 `0x0D00853A..41` 在 C++ 里**零读者**，由数据侧 labor 规则消费以控制班次 | 全树 grep |
| 8 | 历史 5 个包的到岗人数 ≈ **W1 岗位数 × 0.83~0.87**（26/30、25/30、6/7），而非被工人槽 10 卡死 | §3 对照表 |

---

## 1. 引擎数据结构（源码位置）

| 结构 | 头文件 | 关键字段 |
|---|---|---|
| `GB::cEcoResourceBin` | `include/GB/cEcoResourceBin_ord5252.h` | `mResourceIndex / mAmount / mCapacity` |
| `GB::cEcoSinkEntry` | `cEcoSinkEntry_ord5304.h` | `mResourceIndex`（key 资源）、`mResourceInvert`、`mAmountModifier`、`mResourceConnected`、单元/全局控制位测试、资源测试表 |
| `GB::cEcoUnitEntrySinkInfo` | `cEcoUnitEntrySinkInfo_ord5288.h` | `mEntryIndex / mRuleIndex / mKeyBinIndex`、`mUnitTests`、`mConnectedTests` |
| `GB::cEcoSinkInfo` | `cEcoSinkInfo_ord5408.h` | `mCapacity / mMaxCapacity / mPosition`（运行时） |
| `GB::cEcoTransportEntry` | `cEcoTransportEntry_ord5305.h` | `mResourceBins / mSpeed / mAutoDeliver / mActivePathEntries` |

---

## 2. 数据通路：package 列 → 引擎值

### 2.1 岗位与工人槽（P0）

```
Unit 叶子属性                        bundle 映射表                        运行时 bin
────────────────────────────────────────────────────────────────────────────────
kPropWork_JobsW1/C6/C7              容量表 0x0A6FE749/4A/4D              JobsW1/W2/W3
  (0x0A7917C5/C6/C7)    ──────────► 初值表 0x0A6FE74B/4C/4E/0AE26620 ─► bin.capacity / bin.amount
工人槽 0x0E3F65A9/B3/B4   ─────────► 同上                               WorkersNeededW1/W2/W3
                                                                        (07F42755/56/57)
```

- 映射表实现：`src/GB/cEcoUnitEntry/AddBundlePropOverrides.c`（0x644F20，63–100 容量 /
  115–153 初值）。**只对"被引用 bundle"执行**——`AddBundle.c:196-198, 726-729` 有
  `if (!isCoreBundle) AddBundlePropOverrides(...)` 闸门。映射表写在 Unit/Lot 自身的 core
  bundle 上会被**静默跳过**。
- 实测：地标族经济 bundle `40E0C700:3A70FAA2` 的容量表把 `47BEDF8B/88/89` 写成
  `JobsW1/W2/W3`，初值表把 `D3A424B9/BA/BB` 写成 `JobsW1/W2/W3`；劳工 bundle
  `D07C5A04` 把 `WorkersNeeded`(5378D6F9) ← `0x0A7917C8`、`WorkersNeededWx` ←
  `0x0E3F65A9/B3/B4`。

### 2.2 sink 容量公式（P0）

`src/GB/SinkCapacity.c`（0x616BF0）：

```text
if !unit->mIsSimulating                          → 0
if 单元/全局控制位测试失败                        → 0
for 本地测试 t:  if bins[t].amount == 阻挡值      → 0
for 连接测试 c:  if 汇总 amount == 阻挡值         → 0
keyBin = sink->mKeyBinIndex
if keyBin >= 0:  a = bins[key].amount; k = bins[key].capacity
else if !connected:                              → 10      // ← 硬编码 fallback
if amountModifier > 0:  a = invert ? max(a-mod,0) : min(a+mod,k)
return invert ? a : (k - a)
```

`SinkMaxCapacity.c`（0x616E50）：`max = keyBin>=0 ? max(capacity,1) : 10`，connected 时累加。

本地测试的阻挡值是 0 或初始化时的 capacity（inverted），连接测试用汇总 capacity。通过前置测试后，非 connected sink 缺 key bin 才走 10 的分支，connected sink 继续计算汇总值。

### 2.3 工人 agent 的路由（P1）

- 住宅规则 `RunRule` → `CreateTransaction`（0x60DD50）→ `SendTransaction`（0x5FDA00）
  → `DoAddTransaction`（0x5F7530）。
- 目的地候选 = 规则数据 `mTransportOps` 里的 **sink entry 模板 ID 列表**，
  `GetInitialDestinations`（0xB6D3E0）按数据顺序转换，**没有全城距离/吸引力排序**。
- 上路选池：`AddAgentToPathPool`（0x8337C0）要求 `pool.mTotalSinkCapacity > 0`；
  车辆另有 `SetInitialDestination`（0x834390）。
- 到达后进哪栋楼：`CanDeliverPathAgent`（0x757290）→ 该路段 attached sinks 逐个
  `AgentAtSink` + `mCapacity > 0` + `TakePathToSink`。容量在多个 path attachment 间
  **整数拆分**（`AddSink` 0x836C30：`capacity/n + (capacity%n > i)`）。

### 2.4 占用/释放（P1）

- 占用：`DeliverTransaction`（0x602E10）→ bin amount 上升 → `UpdateUnitSinkCapacities`
  （0x5F7730）重算 → sink 容量下降，归零后交通层不再派人。
- **引擎没有"到点自动下班"**：释放由数据侧规则（班次结束）减/清 bin。

### 2.5 UI 三套判定（P0）

| 显示 | 条件 | 值来源 |
|---|---|---|
| 悬浮卡红字 `N/M 個勞工` | `TotalWorkers.amount < kPropWork_MinimumWorkersForProduction` | `template_registry.js:5855` |
| `已關閉。需要勞工…` | `ShiftLength.amount > 0` 且 `WorkersNeeded.amount != WorkersNeeded.capacity` | `template_registry.js:6499` |
| `需要勞工！` 告警图标 | 数据告警 `0x0c77219e` / `icoAlertWorkers.png` | `Alerts.json` |

城市劳工面板读的是**全局资源**（`TotalWorkersW1/W2/W3`、`ExcessWorkers*` 等），
与单栋楼的 bin 无关——所以"全城待业 2912 人"与"这栋楼招不满"可以同时成立。

---

## 3. 希望大厦历次构建的数据对照

### 3.1 逐包真实列（test-archive 解包，sc-properties 权威解析）

| build | TotalJobs | W1 | W2 | W3 | MinWorkers | A9 | B3 | B4 | 学历分布 | 排班 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| `125ed5c7`（/48 旧基准） | 110 | 90 | 20 | 0 | 10 | 10 | 0 | 0 | **缺失** | 0/0/7/7/6/6/0/0 |
| `204c0738`（学历分布修复） | 50 | 30 | 20 | 0 | 30 | 10 | 0 | 0 | 6 列 | 0/0/7/7/6/6/0/0 |
| `f2be3587`（minW 30→10 + 8 班） | 50 | 30 | 20 | 0 | 10 | 10 | 0 | 0 | 6 列 | 6/6/7/7/6/6/7/6 |
| `bc066263`（HT 14 坑） | 14 | 7 | 5 | 2 | 0 | 10 | 0 | 0 | 6 列 | 全 0 |
| `7ce0cd77`（全 W1 14） | 14 | 14 | 0 | 0 | 0 | 10 | 0 | 0 | 6 列 | 全 0 |
| `d034f4e1`（引擎对拍修正，本版） | 30 | 30 | 0 | 0 | 0 | **30** | 0 | 0 | 6 列 | 116 族全 8 班 |

### 3.2 游戏内观察记录

| build | 观察到的到岗 | 到岗 / W1 | 面板状态 | 其它 |
|---|---:|---:|---|---|
| `125ed5c7` | 0 | 0 | 需要劳工！ | 游客 0/0/0 |
| `204c0738` | **26** | 87% | 需要劳工！ | 游客 0；26 < minW 30 → **红字条件成立** |
| `f2be3587` | **10 → 25** | 33% → 83% | 仍需劳工 | 10 = minW 边界；25 > minW 10 却仍红 → 见 §4.3 |
| `bc066263` | **6** | 86% | 缺人 | 同城工厂 514 劳工（未受教育档）；气泡「高收入消費者不足」 |
| `7ce0cd77` | **6** | 43% | 需要劳工！ | 门口街道工人 agent 密集；全城 §9208 / $$9730（2912 待业）/ $$$201 |
| 对照组 威利斯 | **105** | 70%（W1 150） | **不红** | 观光客 369/390；minW 60，105 > 60 |

### 3.3 从对照表读出的三条事实

1. **到岗数跟 W1 走，不跟 TotalJobs 走**：`204c0738`（W1 30）26 人、`bc066263`（W1 7）
   6 人、`f2be3587`（W1 30）25 人。TotalJobs 在 50 / 14 / 50 之间摆动，到岗数只随
   W1 变动，与结论 2（C3 不进模拟）一致。
2. **到岗数 ≈ W1 × 0.83~0.87**（26/30=0.867、25/30=0.833、6/7=0.857）。三个样本
   高度一致地停在 7/8 附近，说明每个班次/每条 path 通路有稳定比例的槽位没被填满，
   而不是被工人槽 10 卡死（26 > 10）。
3. **红字与 minWorkers 对得上班表**：`204c0738` 26 < 30 → 红，符合引擎判定；
   `d034f4e1` 把 minW 设为 0 后该条件恒不成立。

**一处数据质量说明**：`7ce0cd77` 记录的 6 人 / W1 14 = 43% 与上面 7/8 的规律不符。
该次观察发生在包集替换之后，城市里那座楼很可能是**旧包集下放置的实例**（每次
SimCityData 包集变更都会让已放置测试楼的 unit entry 索引漂移）。按存档隔离死命令，
该样本不能作为机制反证，只作为"改包后必须重放"的例证。

**一处反直觉但已解释的样本**：`f2be3587` 在 25 人（> minW 10）时用户仍看到
「需要劳工」。可能有两种来源：①红字在工人下班归零的时段回显（引擎无自动下班，
班次结束由规则清 bin）；②显示的是数据告警（`需要勞工！` 图标），其触发来自
`WorkersNeeded.amount != capacity` 而非 minWorkers。两者都不推翻结论 1——
它们读的是**另外两个**引擎值。

### 3.4 与威利斯的差距到底在哪（不再是"全都对不上"）

| 维度 | 威利斯 | 我们（`7ce0cd77` 前） | 判定 |
|---|---|---|---|
| W1 岗位 | 150 | 7~30 | 量级差 5~20 倍 → 到岗上限差同倍 |
| 工人槽 A9/B3/B4 | 75/75/15 | 10/0/0 | **关键杠杆**，见 §5 |
| 排班 | 116 族 × 8（全非零） | 全 0 / 半零 | 班次规则不跑 → 交付窗口少 |
| minWorkers | 60（105 人时已过） | 30 / 10 / 0 | 决定红字，不决定人数 |
| 观光客 | 369/390 | 0 | 另一条独立链路（§6） |

---

## 4. 本轮**推翻**的三个旧结论

| 旧结论（本会话早前） | 推翻依据 |
|---|---|
| 「需要劳工」= 岗位没填满 | 红字只比 `TotalWorkers` 与 `minWorkers`；威利斯 29% 填充不红 |
| 财富/教育档位不匹配是主因 | 引擎无财富档过滤；`bc066263` 把 W2/W3 清零后到岗比例没变（86%） |
| 工人槽 10 是硬上限 | 26 > 10，历史数据直接反证；10 是 fallback 与初值，不是 lifetime cap |

**仍未定的一项**：到岗卡在 W1×7/8 的机制（班次覆盖 / path attachment 整数拆分 /
规则重装窗口）——需要 `d034f4e1` 的实测数据来区分（§6）。

---

## 5. 已实施的修复

### 5.1 代码（`src-tauri/src/asset_build_m1/behavior_nodes.rs`）

`apply_labor_auto_wiring` 的 `LaborProfile::AuthorOnly` 分支：

- 原有：只补 `TotalJobs = ΣW`。
- 新增：**工人槽缺省按岗位缩放**——`A9←W1`、`B3←W2`、`B4←W3`，用
  `set_float_fields.entry(..).or_insert(..)` 保证作者显式值（`compile` 开头已写入的
  `semantics.labor.workerSlots`）不被覆盖。
- 回归测试：`author_only_labor_scales_worker_slots_with_jobs_by_default`、
  `labor_worker_slots_override_loop_baselines`。

依据：映射表把 A9/B3/B4 写成 `WorkersNeededWx` bin，sink 容量 = 该 bin amount；
固定 10/0/0 让任何规模的资产都只按 10 的预算招人。

### 5.2 数据（`D:/openscp/mods/大连希望大厦/schemas/assets/m1.hope-tower.json`）

作者侧只剩一个字段：

```json
"labor": { "workers": 30 }
```

后端 `parse_labor` 把它展开成引擎需要的全部列（推导规则 = §2 的引擎事实）：

| 作者输入 | 推导结果 | 依据 |
|---|---|---|
| `workers: 30` | `TotalJobs=30`、`W1=30/W2=W3=0` | 总数全部计入最低收入档（保守默认） |
| 缺省 `minWorkers` | `30/6 = 5` | 原版地标比例（Willis 60/360；s31 12/64≈1/5.3） |
| 缺省工人槽 | `A9=30 / B3=0 / B4=0` | 按岗位缩放（§2.1 映射表） |
| 缺省排班 | 116 族 × 8 班 | 原版地标形态 |
| 缺省学历分布 | 15/0/0 系（W1/2、W2/2、W3） | s31 实证配方 |

作者仍可显式覆盖任意一项（`w1/w2/w3` 分档、`minWorkers`、`workerSlots`、`schedule`）。

包内抽查（`03b056b3`，workers-only 构建）：`W1=30 / Total=30 / minW=5 / A9=30 /
B3=B4=0 / 排班 116 / 学历分布 6/6`——全部由后端生成，无手填列。

前端（`LogicNodeCard.vue` + `logicGraph.ts`）：

- 主输入：「需要的劳工数」（`workers`），带一句说明"分档、工人槽、开工阈值与班次由构建器自动推导"。
- 高级折叠：「按收入档拆分」三项 + 「开工所需最低人数」（占位符"留空自动，约为总数的 1/6"）。
- 旧图迁移：缺 `workers` 的图按 `w1+w2+w3` 自动补总额，分档保留为高级覆盖。

---

## 6. 未决项与下一步验证

1. **`d034f4e1` 到岗数**：若跳到 ~30 → 工人槽确认是杠杆；若仍 ~25 → 槽不是杠杆，
   瓶颈在 path/sink 交付通路，下一步查 `AddSink` 的 attachment 拆分与
   `mMaxSinkToPathDistance`。
2. **W1×7/8 的稳定缺口**：用同一栋楼改 `schedule`（全 116 vs 全 0）做 A/B，确认
   班次数是否线性影响交付。
3. **观光客到访待测**。以地标观光查询的 `TouristW1–3` 和对应 sink 的送达记录为准。
   公园面板「每日旅客人数」的居民、孩童、游民统计走公园查询。有效 bundle
   复核与实机读数见 §12 和 §19。
4. **夜景灯光**：`cGraphicsUnitInteriors::Update`（0x7D4720）用
   `WorkersNeeded.amount/capacity` 算亮灯比例 `0.7*ratio+0.15`——灯不亮与到岗不足
   同源，`d034f4e1` 后应一并复查。

---

## 7. 为什么 s31 是"歪打正着"，照它调希望大厦必然失败

### 7.1 s31 的真实列（`000_OpenSCP_University_s31.package` 解包实测）

| 列 | s31 值 | 说明 |
|---|---|---|
| TotalJobs `0A7917C3` | 34 | 引擎不读（§0 结论 2） |
| JobsW1/W2/W3 `0A7917C5/C6/C7` | 20 / 10 / 4 | 真正的岗位 bin 输入 |
| MinWorkers `0A7917C8` | **12** | **只做红字阈值**，占总岗位 35% |
| 工人槽 A9/B3/B4 | 10 / 0 / 0 | 与希望大厦旧基线完全相同 |
| 学历分布 `0DC1DB39/43/45` | 10 / 5 / 4 | 恰为 W1/2、W2/2、W3 |
| 排班 `0D00853A..41` | 6/6/7/7/6/6/7/6 | 全是非零班次 |
| parent `00B2CCCB` | `7E4B99E5` 教育模板 | 容器身份 |
| 类别 `0AFE27B1` | `95A9D6FD` 教育类别 | 容器身份 |
| 启用 bundle | 265FB8DD, C0672F57, **6C042366, 487CDA43, BA823893, 472644E9, 53BA360B, CF341CAC, 9A1BFB2F**, D07C5A04, F236BB93 | 教育族 7 个 + 劳工 2 个 |
| 排除 bundle | 1B946073, E0C02E1F | |

而本仓库的 auto-wiring 注释早已写明来源：**vanilla 招工 unit B9217049（w1=20，28 属性最小接线）**。
即 s31 的劳工数值不是设计出来的，是**逐字抄自一个已跑通的原版学校**。

### 7.2 三层偶然

| 层 | 偶然点 | 后果 |
|---|---|---|
| ① 来源 | 数值抄自原版同类建筑（同一身份、同一量级 w1=20） | 所有派生 bin 落在原版规则预期区间，偶然自洽 |
| ② 判据 | minWorkers=12 只占总岗位 **35%**，而实际到岗 ≈ W1×0.86 ≈ 17 | 17 ≥ 12，红字恒不出现——**从未填满，只是门槛低** |
| ③ 观测 | 教育建筑的主流量是**学生**（教育 bundle 家族），学生 sink 让学生流动、建筑看起来在运转 | 劳工链路的任何缺陷都被学生链路掩盖，从未单独验证在岗人数 |

**这就是"歪打正着"的完整定义**：s31 证明的是"教育身份 + 原版数值副本 + 低门槛"能跑，
**不是**"我们理解了劳工链路"。同理，威利斯 105/360 = 29% 填充也不红（minW 60 占 17%）。

### 7.3 为什么照 s31 调希望大厦复现不了

| # | 失效点 | 证据 |
|---|---|---|
| 1 | **抄了叶子数值，没抄容器**。s31 的数值由「教育模板 + 教育类别 + 教育 bundle 家族 + D07C5A04 + 6/7 排班」整套消费；希望大厦是公园/旅游身份（park 类别 + CF116B9C/2B5A2BAF/362087BD），教育专属 bundle **一个都没有**（实测 bundle 差集：s31 独有 6 个教育 bundle，hope 独有 E7690450+3 个旅游 bundle） | §7.1 表 + `willis_unit/hope_unit` dump |
| 2 | **把 minWorkers 当"目标"而不是"阈值"**。照着 s31 的思路给希望大厦写 minWorkers=30（占 50 岗的 **60%**），实际到岗 26（= W1 30 × 0.86，与预测完全吻合）→ 26 < 30 → 红字常驻 | `204c0738` 实测 26 人 |
| 3 | **量级错配**。s31 是 34 岗学校；同一套比例搬到 50 岗商业地标后，sink 容量/排班/学历分布不再自洽 | §3.1 vs §7.1 |
| 4 | **一直在调引擎不读的列**。C3（TotalJobs）引擎零引用；C5/C6/C7 只有在 bundle 映射表存在时才落成 bin；真正决定到岗的是 sink key bin 容量（无 key bin → fallback 10）+ 工人槽 + 排班 | §0 结论 2/3/4/5 |

### 7.4 红线判据对照（引擎语义下的唯一正确读法）

红字条件 = `TotalWorkers ≥ minWorkers`，与"填满率"无关。用可达填充 ≈ W1×0.86 代入：

| 样本 | W1 | minW | minW 占总岗位 | 预期可达 | 判定 |
|---|---:|---:|---:|---:|---|
| s31 大学 | 20 | 12 | 35% | 17 | **通过** |
| 原版威利斯 | 150 | 60 | 17% | 105（实测） | **通过** |
| 希望 `204c0738` | 30 | 30 | 60% | 26（实测） | **红字常驻** |
| 希望 `f2be3587` | 30 | 10 | 20% | 26（实测） | 卡片判据通过；`需要勞工`告警仍未消（读的是 `WorkersNeeded.amount != capacity`，另一个值） |

**结论**：希望大厦的招工链路**一直是通的**（26 = W1×0.86 与 s31/威利斯的规律一致），
失败的是"判据设置"与"容器身份"两件事。以 s31 为样板时必须同时搬三件套——
**容器（模板/类别/bundle 家族）、比例关系（岗位:槽:学历:排班）、判据（minWorkers 与可达填充的关系）**，
只搬数字必然复现不了。

### 7.5 现在的直觉输入层如何吸收这层语义

`semantics.labor.workers` 一个字段 → 后端给齐三件套：

- **比例**：`W1=workers`，学历 = W1/2、W2/2、W3，槽 = 岗位数（§5.2）；
- **判据**：`minWorkers` 缺省 = 总数/6（= 原版地标 17% 与希望实测可达 86% 之间的安全值）；
- **容器**：仍由家族适配器决定（教育族沿用 s31 配方；park/tourism 族按旅游 bundle + 地标形态）。

### 7.6 第三方样本 HTzhiyao：反向佐证（2026-11-11 解包实测）

`SC5_Asset-HTzhiyao_0.1.0.package`（SC5 Asset Packager 作者的观光资产）是第三个关键样本。
它被当作"能正常吸收劳工"的参照，但解包后的事实是：

| 维度 | HTzhiyao 实测 |
|---|---|
| TotalJobs / JobsW1/W2/W3 | 7 / **5 / 2 / 0** |
| MinWorkers | **0** |
| 工人槽 A9/B3/B4 | **0 / 0 / 0** |
| 学历分布 | 5 / 2 / 0 |
| 排班 8 列 | **全 0** |
| parent | **无**（不继承任何模板） |
| 类别 | `899BEA70`（公园容器） |
| 启用 bundle | 265FB8DE、C0672F54、8BA6D35D（共 3 个） |
| 其中 bin 声明 | **1 个**（资源 779278001，容量 INT_MAX，初值 0） |
| 其中 sink 声明 | 4 个（265FB8DE 3 个 + C0672F54 1 个） |
| 属性→bin 映射表 `0A6FE749..4E` | **完全没有** |
| Lot 上的 sink 表 `0A2725CE` | 无（sink 只来自 bundle） |

对照 s31/hope 使用的劳工 bundle `D07C5A04`：**49 个 bin 声明 + 39 个 sink + 完整映射表**。
HT 只有 1 个 bin、零映射表 → **它的 JobsWx 属性永远不会变成 bin**，即"7 个岗位"
在引擎里只是一组展示数字（与 §0 结论 2 的 TotalJobs 同性质）。

**结论：HT 不是"招工链路更简洁的成功案例"，而是"退出了劳工系统的样本"**：

1. `minWorkers = 0` → 红字判据 `TotalWorkers(0) < 0` **恒不成立**，它**永远不会抱怨缺人**；
2. 零映射表 → 岗位数不落 bin，sink 只能吃到引擎的 fallback 容量（`SinkCapacity` 无 key bin → 10）；
3. 公园身份（`899BEA70`）本就不需要劳工 sink 三元组，靠地价/吸引力规则运转。

所以用户看到的"HT 正常吸收劳工"，按引擎事实应改写为**"HT 从不抱怨 + 门口有零星 agent 经过"**。
它非但不能作为"可以少写列"的依据，反而证明：**不写映射表 = 岗位根本不参与模拟**。

### 7.7 四样本判据合表（第三方交叉验证）

| 样本 | 真劳工 wiring | W1 | minW | minW 占比 | 实测/预期到岗 | 面板是否抱怨 |
|---|---|---:|---:|---:|---:|---|
| s31 大学 | ✅ 完整（D07C5A04） | 20 | 12 | 35% | 17 | 否 |
| 原版威利斯 | ✅ 完整（地标 bundle） | 150 | 60 | 17% | 105 | 否 |
| **HTzhiyao** | ❌ 几乎为零（1 bin/0 映射） | 5 | **0** | 0% | ≈0–fallback | **否** |
| 希望 204c0738 | ✅ 完整 | 30 | **30** | **60%** | 26 | **是（唯一抱怨者）** |

**这张表是整个结论最强的交叉验证**：四个样本里，"是否抱怨"只由 `minW / 可达到岗` 决定，
与 wiring 质量无关；而"到岗多少"才由 wiring 决定。**唯一真正接通并交付了可预测人数
（26 = W1×0.86）的，恰恰是唯一被抱怨的那一个**——因为我们给它的阈值设在了自己的
可达上限之上。

---

## 8. 证据索引

- 引擎源码：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/GB/`
  - `SinkCapacity.c` 0x616BF0 / `SinkMaxCapacity.c` 0x616E50 / `SumConnectedCapacities.c` 0x616B20
  - `cEcoUnitEntry/AddBundle.c` 0x6490B0 / `cEcoUnitEntry/AddBundlePropOverrides.c` 0x644F20
  - `cEcoUnitEntry/FillFromProps.c` 0x64A3A0 / `cEcoSinkEntry/FillFromProps.c` 0x64B2C0
  - `cEcoGame/AddUnitSinks.c` 0x6031E0 / `UpdateUnitSinkCapacities.c` 0x5F7730
  - `cEcoGame/DeliverTransaction.c` 0x602E10 / `CanDeliverTransaction.c` 0x5F8C80
  - `GetInitialDestinations__2.c` 0xB6D3E0 / `GetNextDestinations.c` 0xB6D540
  - `src/SC/cTransportTraffic/AddSink.c` 0x836C30 / `AddAgentToPathPool.c` 0x8337C0
  - `src/SC/cTransportPedestrian/CanDeliverPathAgent.c` 0x757290 / `AgentAtSink.c` 0x74E7B0
  - `src/SC/cGraphicsUnitInteriors/Update.c` 0x7D4720（亮灯比例）
  - `src/global/GetBinIndices.c` 0x7D4610（WorkersNeeded=1400428281 / TotalResidents=70888836）
- UI 数据：`tmp/js/template_registry.js:5855,6499`；`tmp/hud-assets/locale/2F32503C.json`
- 包内证据：`target/labor-audit/*.unit.bin` + `*.txt`（本会话从 test-archive 解包，
  sc-properties 权威解析；可用 `cargo run -p sc-properties --example dump_unit` 复现）
- 部署台账：`D:/ea-games/simcity_offline/test-archive/MANIFEST.md`
- 前置研究：`docs/re/willis-tower-landmark-mechanism-20261027.md`、
  `docs/re/proven-student-behavior-and-tourism-gap-20261109.md`

---

## 9. 观光客 / 消费者机制（引擎 + 19 地标普查定案，2026-11-11）

### 9.1 引擎侧：根本没有 tourist/shopper 代码

全树 grep 17 个旅游/商业资源常量（十进制与十六进制）命中 **0**。游客与消费者完全是
**数据驱动事务**：规则 → `CreateTransaction` → 交通层 agent → sink 交付 → sink 规则记账。
引擎只提供通用机器（`CanRunRule` 0x648470 / `CreateTransaction` 0x60DD50 /
`SendTransaction` 0x5FDA00 / `DeliverTransaction` 0x602E10 / `ApplyBinRule` 0x64BBA0）。

关键资源 ID（FNV-1 小写名，已三重校验）：

| 名称 | 资源 ID | 作用 |
|---|---|---|
| `AttractionLevel` | `54FC7890` | 城市吸引力存量 |
| `TouristsAllowedW1/W2/W3` | `96F3554C/4F/4E` | 每日可吸引配额 |
| `TouristW1/W2/W3` | `1A7B1745/46/47` | 游客 agent 携带资源（财富档） |
| `TotalShoppers` | `C1F569B9` | 顾客/游客**容量** |
| `OpenShoppers` | `CE6E3D2D` | 剩余槽位（初值 ← `kPropShopping_TotalShoppers` 属性） |
| `SouvenirsW1/W2/W3` | `6AE939A5/A6/A7` | 消费额（低/中/高价） |

**触发链**：规则带 `mTransportID` → `CanRunRule` 算 `applyCount`（上限 = `mApplyResource`
bin 的 amount）→ 最后一道闸 `cTransportTraffic::CanDeliverTransaction`（0x8341E0）要求
该规则 `mTransportOps` 里**任一 sink entry 的路段池 `mTotalSinkCapacity > 0`**；全为 0
则一个 agent 都不发。

**建筑"可参观"四要件**：① unit entry 的 sink entry ID 必须与游客规则 `mTransportOps`
里的 ID 完全一致；② key bin 存在且有容量（无 key bin → fallback 10）；③ 控制位/资源测试
通过；④ 临路且 `AddSink` 成功（否则池容量 0）。**不做"就近找景点"**。

**结算**：`DeliverTransaction` → sink 规则 → `TouristW*` 进 bin、`SouvenirsW*` ++、
乘 `kPropTaxes_ToyIncomePerResourceW1/W2/W3`（`0x0EE2A50D/E/F`，引擎零直接读者，
必须先被 bundle 映射成 bin）→`IncomeFromLandMarks`。

### 9.2 面板数字的映射

- 悬浮卡 `光觀客 369/390`：`390` = `kPropShopping_TotalShoppers` 属性（`0x0AD6A725`）→
  `OpenShoppers` bin 初值；`369` = `ShopperW1+ShopperW2+ShopperW3 = 143+11+215`（实测吻合）。
- 「每日旅客人数」那几行 = Menu2 的 `unitEffectTitle 0x0EB1FC05` + `unitEffectParameter
  0x0EB1FC22` 直引用 **Unit 属性**（不是 bin）。威利斯绑定 `[0x0E542F39, 0x0E585AE0]`。
  希望大厦显示"居民/孩童/游民"是**公园 donor 的统计行**，属于菜单层错误绑定。

### 9.3 19 个原版地标普查（文化/地标子栏 3F43E038，逐资源解析）

| 层 | 内容 | 覆盖 |
|---|---|---|
| Menu2 | `09878A01`，`0DB9FC63` 含 `3F43E038` | 19/19 |
| Unit parent | `48E0C000:4E01E311` | **19/19** |
| 模板 A4（12 bundle） | 含 `E15369A2` + `75D22FD0/1/2` + `3A70FAA2` + `C14044C4` | 19/19 |
| 叶层 A5 | 0 个=10 地标、1 个=3、5 个=3、15 个=3 | **A5=0 也能观光** |

- 游客 bin（`AttractionLevel`/`TouristsAllowedW*`/`TouristW*`/`SouvenirsW*`）由
  `E15369A2` + `75D22FD0/1/2`（16/19）或 `13A59DB6`（3/19 演出型）声明，**与叶层无关**。
- 叶层 `CF116B9C` 2/19、`2B5A2BAF` 3/19、`362087BD` 3/19、`908B3CEC` 3/19 等**都不是共性**。
- 属性共性：吸引力 ∈ {400, 800}；`TotalShoppers` ≥ 50（中位 275）；`AmountOfGoods` ≥ 30；
  收入系数 10–15 且三档相等；`Souvenirs` ≥ 100；地价两点恒 500（低档 < 高档）。

### 9.4 希望大厦的游客 0：根因是 parent 选错

| 项 | 原版 19/19 | 希望大厦（03b056b3） | 后果 |
|---|---|---|---|
| parent | `4E01E311` | `7E4B99E5`（教育） | 丢 10/12 模板 bundle |
| `AttractionLevel` bin | 16/19 | ✗ | 吸引力 400 无处承接 |
| `TouristsAllowedW1-3` | 19/19 | ✗ | 无每日配额 |
| `TouristW1-3` | 19/19 | ✗ | 无游客货架 |
| `SouvenirsW1-3` | 19/19 | ✗ | 3300 列无 bin |
| `TotalShoppers/OpenShoppers` | 19/19 | ✓（公园顾客包 `6EA8E701`） | **有"顾客"无"游客"**（购物 8 的来源） |

**结论：2026-11-10 为修劳工把 parent 从地标模板换成教育模板（7E4B99E5），
正是游客恒 0 的根因。** 地标模板的 `3A70FAA2` 本来就提供劳工 bin（`.0A6FE749/74B`
把 `JobsWx` 映射进 bin；Willis 105 人即此路径），一个 parent 同时满足劳工与观光。

**（历史实现，复核见 §12）** `behavior_family.rs`：当时的 `ParkTourism` 族在 Labor 闭环存在时
parent = `LANDMARK_TEMPLATE (4E01E311)`、不注入教育类别、剔除学校劳工包
`D07C5A04/F236BB93`；教育族仍走 `7E4B99E5`。构建验证：parent=4E01E311、
`7E4B99E5` 消失、JobsW1=30/minW=5/A9=30、旅游包保留。包 `23fafa95` 已部署。

### 9.5 剩余待办

1. 菜单统计行改绑真游客字段（`0x0E542F39` 吸引力 / `TouristW*`），把"居民/孩童/游民"换掉。
2. 地价两点顺序反了（希望 `+3/+2`，原版恒 `低<高`）——`behavior_family` 的 land value 基线。
3. HT 的 `AmountOfGoods=48 / Souvenirs=48 / 吸引力=1` 说明它可以有"顾客"但无游客，与本次结论一致。

---

## 10. 学生（student）机制：引擎 + 6 个教育建筑 + s31 对拍（2026-11-11）

### 10.1 引擎侧结论

| # | 结论 | 证据 |
|---|---|---|
| 1 | 引擎里**没有"学生"概念**：16 个教育/技能资源与属性 ID 全树 grep = 0 命中 | 全树 grep |
| 2 | 学生 agent 携带的是 **`KidE2`(0x1A3C152E) / `KidE3`(0x1A3C152F) + `KnowledgeToken`(0x884BA59E)**，**不存在 `StudentW*`**（只有 `TradeStudentW1-3`） | transport entry `0D19BDC8/9` 的 bin 表 |
| 3 | 目的地来自**发起 transaction 的那条住宅派发规则**（`mTransportOps`，实例 `9CFEB042`: `[78B96709, 5770FEDD, 1A32F13D]`），不是学校自己的规则；不做就近搜索 | `GetInitialDestinations` 0xB6D3E0 |
| 4 | 学校接收闸门 = sink entry **`78B96709` 的 key bin `SpotsForKids`(0x05DAADA6) 的 amount**（`invert=true`） | `SinkCapacity` 0x616BF0 |
| 5 | **`0x0AD7FAC9` 不是学生容量**：它只喂 8 个 `StudentsNeeded_00..21` 槽的 capacity | `6C042367` 的 `0x0A6FE749/4A` 表 |
| 6 | `mSkills` 技能子系统在本版**休眠**：1214 unit / 385 模板 / 1308 bundle 中 **0 个**声明 `SkillID/SkillThreshold`（`0x07E0AD04/05`）；`AddSkillForUnit` 只在建单位时 +1，规则无法写 | 全库频次统计 |
| 7 | 到校结算规则 `B52C889D`：`Student +1 / TotalStudents +1 / SpotsForKids −1 / StudentStudyingE2 +1`，`nearby: KidE2 −1` | 规则转储 |
| 8 | 引擎**无毕业/辍学判定**；席位与统计的清理是规则 | 全树 grep + 规则表 |

### 10.2 原版教育建筑普查（6 条菜单）

| Menu | Unit | 建筑 | parent | 容量列 | 学生 sink |
|---|---|---|---|---|---|
| 29873FA2 | C264A268 | 小学 | 7E4B99E5 | `0AD7FAC9=150` | `78B96709`(B52C889D) |
| AF3EAA80 | 64F750E2 | 中学 | 7E4B99E5 | `0AD7FAC9=800` | `78B96709` |
| B01CEDE4 | A826FA3E | 社区大学 | 7E4B99E5 | `0AD93A77=500` | 4 个（`D10F7B4E` 等） |
| 4A8E26DF | 4250AE35 | 大学 | 7E4B99E5 | `0AD93A77=800` | 4+1（含 `StudentToLeave` `7CD6A9FA` invert=false） |
| E2832F99 | 0BFE06AB | 图书馆 | DEFA792F | — | **无** `05DAADA6`（走工人/技能流） |
| 9B0D5AAF | 9182FA79 | 校车站牌 | — | — | — |

**关键配对规则（写错即 0 席位，且完全静默）**：

```
小学/中学族  bundle 6C042367  → 容量列 0AD7FAC9 + 初值列 0B98804E/51/54/57/5A/5D/60/63
社区/大学族  bundle 6C042366 / 3256C0A2 / AF195AFC → 容量列 0AD93A77 + 初值列 0B9880C6/C9/CC/CF/D2/D5/D8/DB
```

全库 1214 unit 中：8 个带 `0AD7FAC9`、9 个带 `0AD93A77`，**无一同时具备**。

**教育模板 `7E4B99E5` 不承载学生**：其 A4 只有 4 个事件/通用底座 bundle（`C1F210B4/F0D5FBAE/3C8CF2EC/5D7F8D92`），学生能力**全在叶子 bundle**。这与地标族（游客在模板层）恰好相反。

### 10.3 s31 为什么能收学生（与社区大学逐字同集）

s31 的 7 个教育 bundle（`6C042366/487CDA43/BA823893/472644E9/53BA360B/CF341CAC/9A1BFB2F`）
**与社区大学 `A826FA3E` 完全同集**；其中只有 `6C042366` 承载学生 sink。
s31 同时具备 `0AD93A77=500` + 初值 `0B9880C6..DB = 0/0/500/500/500/500/0/0`（社区大学逐字相同），
所以四要件齐备：`0AFE27B1=95A9D6FD` + 学生 sink bundle + 配对容量列 + 8 个初值列。

s31 上的以下列**都不是学生必需**（原版教育建筑没有）：公园残留 `099B4C1F=5FFA86B9` /
看板 `D0621548` / `0DA3EE15=51`、学校劳工包 `D07C5A04/F236BB93`、排班 `0D00853A..41`
（原版学校**无排班**，s31 的 6/6/7/7/6/6/7/6 是构建器硬编码）。

### 10.4 发现的产品缺陷（待修）

`behavior_nodes.rs:757` + `behavior.rs:896` / `native_profile.rs:94` **无条件写 `0AD7FAC9`**，
而 `menu.rs:1228` 的统计行用的是 `0AD93A77`。后果：**任何用 `signatureBundle=6C042366`
（社区大学/大学）构建的资产，容量列与 bundle 不配对 → 0 席位 → 收不到学生，且日志无报错。**

修复方向（未实施，需跨 behavior/menu/native_profile 三层加 `seats_capacity`）：
按签名 bundle 选择容量列 —— `6C042367`→`0AD7FAC9`，`6C042366`/`3256C0A2`→`0AD93A77`，
并同步写各自的 8 个初值列。

### 10.5 "自制学校收不到学生"诊断清单（数据侧，按优先级）

1. 容量列与 bundle 族不配对（§10.4，静默失败）。
2. 叶子 `0A0209A5` 里没有学生 bundle（`6C042367`/`6C042366`/`3256C0A2`/`AF195AFC`）。
3. `0A0209B2` 排除了学生 bundle（叶子排除会遮蔽模板排除）。
4. `0AFE27B1 ≠ 95A9D6FD`（注意 `099B4C1F` 是另一列，s31 写公园值仍可工作）。
5. sink entry 必须存在且 key=`05DAADA6`、`invert=true`。
6. 路径：Unit `0DA69D6F` 指向存在的 PathEntry（学校 `2C7C7986`），Lot 人行可达。
7. 教学模块（`0DF1467C` 带模块号，如 `5DE9CD2A`）无 sink，不能当独立学校放置。


### 10.6 s31 实测补充：三系统叠加的实证（2026-11-11 游戏截图）

游戏内实测：s31 同时具备 **500 学生**（悬浮标签，= `0AD93A77` 席位容量全满）+
**固定时间上下班的劳工** + **公园机制正常吸引孩童**。这把 §10.3 的结论从
"学生必需列"升级为**多系统叠加原理**：

```
系统叠加法则：每个系统 = (bundle 族 + 配对容量列 + sink 集 + 判据) 的完整闭环；
              闭环齐备的系统各自独立运转，互不干扰，可无限叠加。
```

| 系统 | s31 的完整闭环 | 状态 |
|---|---|---|
| 学生 | `6C042366` + `0AD93A77=500` + 8 初值列 + sink `D10F7B4E` 族 | **500/500 满员** |
| 劳工 | `D07C5A04/F236BB93` + `30645625` + JobsWx/minW 12 + **排班 6/7 族** | **固定班次上下班** |
| 公园（吸引孩童） | `265FB8DD/C0672F57` + `099B4C1F=5FFA86B9` + 看板 `D0621548` | **正常吸引孩童** |

两条修订：

1. **排班 8 列实证为功能性**（此前文档标"语义未定/构建器硬编码"）：s31 的
   6/6/7/7/6/6/7/6 驱动了"固定时间上下班"的行为。C++ 无读者的结论不变——
   消费者是数据侧 labor 规则（班次窗口/每班规模）。**全 0 = 无班次** 由此从
   "推断"升级为"已解释"。
2. §10.3 所列"非学生必需"的列（公园残留/劳工包/排班）**对学生确实非必需，
   但它们是另外两个系统的必需件**——s31 的正确读法不是"这些列没用"，而是
   "每个列族服务各自的闭环"。

**对希望大厦的意义**（当前 `23fafa95`，parent=4E01E311）：按叠加法则核对三系统——

| 系统 | 闭环状态 |
|---|---|
| 观光 | 地标模板 A4（E15369A2 + 75D22FD0/1/2）✓ 本版恢复 |
| 劳工 | 模板 `3A70FAA2`（JobsWx→bin 映射 + worker sinks）+ 叶子 JobsWx/minW=5/A9=30 + 排班 116 族 ✓ |
| 公园（孩童） | 叶子 `265FB8DD/C0672F57` + `099B4C1F=5FFA86B9` + 看板 `D0621548` ✓（与 s31 逐字一致） |

三个闭环第一次同时齐备。若实测仍有缺失，按叠加法则逐系统单独排查（而不是再改单一列）。

---

## 11. 夜景灯与电源按钮定案（2026-11-11 深挖）

### 11.1 夜灯机制（`cGraphicsUnitInteriors` 0x7D4720/0x7D4690 源码直读）

```
FillFromProps:  从 unit 的【非模拟属性】(mNonSimProps) 读两个布尔开关
  0x0E0BE186 lightswhenactive            → mInteriorsAllLitWhenActive
  0x0F169C46 interiorsdrivenbyactiveflag → mInteriorsDrivenByActiveFlag
  GetBinIndices → mBinIndexWorkersNeeded (WorkersNeeded bin 0x5378D6F9)

Update (每帧):
  若 WorkersNeeded bin 存在: 目标亮灯比例 = amount/capacity + 0.15
  若 mInteriorsAllLitWhenActive 且目标>0 且 控制位 0x20000000(运营中):
      目标 = 0.85        ← ★ 运营中的地标 85% 亮，与工人数无关
  控制位 0x1 (废弃):  目标 = 0
  实际值向目标渐进 (速度 = gameSeconds×7, 上限 1)
```

**两个"开灯控制字段"就是 `0x0E0BE186` 和 `0x0F169C46`**（后者是我们既有研究里的
"Lot interior controller"）——它们是 **Bool**，但**不在 EcoGame exemplar、不在 Lot、
不在模型 RW4 的明面上**：它们住在**图形单元资源（组 `42E1C000`）**里，
`FillFromProps` 经 `FindUnitNonSimProps`（graphics 组按 `0xCA5CF4A/B` 检索）读取。

### 11.2 实证

- Graphics 包组 `42E1C000` 的资源（如 `50586A05`，92 列 PropertyFile）携带
  `0E0BE186 = Bool(true)`——17 个资源直接命中（≈19 个原版地标的图形单元）。
  资源结构：parent→Lot、BoundingBox、4 个模型引用（00F9EFBB-E）、17 组
  UnitLocation（02A907B5/B6/B9/BB/BC）、路网键（0C12EF24-2A）、`lightswhenactive`。
- **希望大厦包（23fafa95）完全没有 42E1C000 资源** → 引擎的 interior 对象没有
  `mInteriorsAllLitWhenActive`，目标比例 = `WorkersNeeded.amount/INT_MAX + 0.15 ≈ 0.15`
  → 夜景近乎全黑。**这就是"开灯控制字段漏掉"的实体**。

### 11.3 游客配额列的量化修正

`E15369A2`（游客配额载体）的映射表：`AttractionLevel ← 0E542F39`、
`TouristsAllowedW1/W2/W3 ← 0D8C29F6/FE/A01`（每日吸引的低/中/高收入游客配额）。

| 样本 | 0D8C29F6/FE/A01 | 每日游客上限 |
|---|---|---|
| 威利斯 | **150/75/50**（=275/天） | 肉眼可见 |
| 希望大厦 | **10/10/10**（公园 donor 继承值） | 每天 ≤30，观察窗口内几乎为 0 |

**且**「每日旅客人数」面板（居民/孩童/游民）是公园 donor 统计行，**读的就不是
观光客 bin**——游客就算进了门也不会显示在这三行里。判定"游客 0"被误导了。

### 11.4 电源按钮

罗马斗兽场面板底部有电源开关（运营中地标标配）。希望大厦电消费 `72/3`
（威利斯 72/4464）——3 单位的消费量级疑似不足以触发面板按钮/UI 查询。
修复随图形单元资源一起做（电/水提到地标量级）。

### 11.5 修复方案（下一轮实施）

1. **构建器新增 42E1C000 图形单元资源发射器**（native_profile）：
   parent→Lot、BoundingBox（编译模型实测）、模型引用×4、UnitLocation（Lot path
   端点转换）、路网键、`lightswhenactive=true`（旅游/地标族）、可选
   `interiorsdrivenbyactiveflag`。
2. **tourism loop 增配额三列**：`0D8C29F6/FE/A01` 缺省 Willis 量级（150/75/50）。
3. **utilities 电/水提到地标量级**（电力 Amount 按面积估算，≥100）。
4. **菜单统计行改绑**观光字段（`0E542F39` 吸引力 / `TouristW*`），替换公园三行。

### 11.6 证据索引

- 引擎：`src/SC/cGraphicsUnitInteriors/Update.c` 0x7D4720、`FillFromProps.c` 0x7D4690
  （235659654=0x0E0BE186、253140038=0x0F169C46）、`src/GB/FindUnitNonSimProps.c`
  （0xCA5CF4A/B 检索键）
- 数据：Graphics 包组 42E1C000（17 资源带 `0E0BE186=Bool(true)`；样本
  `50586A05` 92 列含 parent→Lot/模型引用/UnitLocation×17）；E15369A2 映射表
  （AttractionLevel←0E542F39、TouristsAllowedW1-3←0D8C29F6/FE/A01）；威利斯叶
  0D8C29F6/FE/A01 = 150/75/50 vs 希望叶 10/10/10；希望包无 42E1C000 资源
- FNV 佐证：0x0E0BE186 = FNV-1("lightswhenactive")、0x0F169C46 =
  FNV-1("interiorsdrivenbyactiveflag")

### 11.7 重要更正（2026-11-11 晚，据 `building-facade-engine-analysis.md`）

**夜灯不走 Unit 属性，也不走 parent——它走模型 slot5 的纹理 alpha。**

`docs/re/building-facade-engine-analysis.md` §2.1 早已定案：

| 采样器 | 槽位 | 内容 |
|---|---|---|
| `interiorMapSampler` | **slot5** | 内景房间图集：**rgb = 房间预渲染图，a = 逐像素灯亮自发光** |

- 窗户不是几何：窗洞 = tint.a 裁剪 + shaderMap.a 遮罩 + relief 视差 + interiorMap 假内景。
- `building4InteriorMapPS` 的调试变体 `interiorThresholds = 0` 会**强制全暗**——即 alpha
  通道为零 = 夜间必黑，与 parent / 类别 / lightswhenactive 无关。

因此：
1. 本文 §11.1–11.5 关于 `lightswhenactive`（42E1C000）的结论**只能决定"运营中是否整体提亮"**，
   **不能产生窗格灯**；把它当成夜灯根因是错的。
2. 我在 §7 之后"parent 从教育换地标导致熄灯"的判断同样被推翻——那次对拍里
   Lot 只差 180° 镜像、Unit 只差 6 个学历列，**真正变的是模型槽位载荷（旧 4,869,060B /
   新 4,869,140B）**，即 slot5 interior 的 alpha 内容。
3. 正确排查顺序（每次夜灯问题都按此走，不再猜）：
   - ① 编译产物的 LOD1 六槽 payload：**slot5 interior 的 alpha 是否非零**（非零=有灯）；
   - ② `author_materials` / `single_compatible` 的 room-strip 渲染有没有把
     source emission 写进 night 列（现有门禁文案："room N has sampled source emission
     but all native night states are empty" 就是这条）；
   - ③ Lot 的 `0F169C46=true`（`package.rs:964` 已有门禁）；
   - ④ 只有 ①②③ 都过后，才轮到 42E1C000 的 `lightswhenactive`。
4. **构建期应固化**：把 slot5 alpha 非零作为硬门槛（与 night-interior gate 并列），
   使"编译产出没有夜灯"在构建阶段直接失败，而不是进游戏才发现。

### 11.8 最终定案（据 SC5 研究报告《第九阶段》，2026-11-11 晚）

`D:/ea-games/simcity_offline/entity/001/第九阶段_内景控制器与昼夜驱动.md` 用零售引擎
反汇编给了完整答案：

| 环节 | VA | 行为 |
|---|---|---|
| 控制器创建 | `0x0081D020` | 检查居民/工作人员来源；**缺失时只有图形资产 Bool `0F169C46=true` 才允许创建**，否则返回空 |
| 初始化 | `0x0081CD60` | 把 `0F169C46` 读入控制器模式；另读 `0E0BE186` |
| 更新 | `0x0081CDF0` | 通常由人数/容量算目标；模式启用时按 active 状态取 0.85 或 0 |
| GetInfo | `0x0081CC80` | 输出 `[q*d*s, q*s, s]`（q=平滑亮灯比例，d=daytime，s=状态因子） |
| 默认上传 | `0x00783BC1` | **控制器为空 → 模型使用固定阈值 `(1,1,1)`** |

配合《第八阶段》：shader 用三个阈值与随机房间值比较，三次 `step` 之和决定内景图集
四列中的哪一列发光。所以：

- **控制器为空 + 固定 (1,1,1) → 永远第 0 列**；
- 本仓库材质把夜景发光编在第 1 列（`state%4==1`）→ **永远黑窗**；
- 有控制器后夜间 `q=0.85, d=0` → `[0,0.85,1]` → 约 85% 房间选第 1 列 → 窗灯亮；
  白天 `d=1` → `[0.85,0.85,1]` → 回到第 0 列 → 不亮。

**修复（已实施）**：`menu.rs` 的图形单元资源（42E1C000）同时写
`0F169C46=true` 与 `0E0BE186=true`（原版编码 `0f169c46 0001 8000 01`）。
包 `41da71186ad8b8e0` 已部署。

**这条必须成为构建期硬门槛**：图形单元资源缺 `0F169C46` → 窗灯必黑，
应在构建阶段 fail，而不是进游戏才发现（与 night-interior gate 并列）。

---

## 12. 复核：有效属性表如何决定招工和观光（2026-11-11）

### 12.1 先核对正在读哪一版

先查希望大厦项目 `.build/ledger`，再查游戏 `test-archive/MANIFEST.md` 第 449–480 行附近。所查构建台账未给下面这些部署哈希提供可直接匹配的 staging 记录，以下用部署文件和归档文件作为取证对象。台账末尾另有独立菜单的夜灯实验 `2536cbe6`，本轮目录实查未找到该文件。实际 `SimCityData/000_OpenSCP_HopeTower_tourism.package` 的 SHA-256 与归档 `41da7118` 一致。

| 取证对象 | SHA-256 |
|---|---|
| 归档 `03b056b3` | `03b056b34128da45e41435c2557c256e9f8272bd51460bb35aab23e25809dc60` |
| 归档 `23fafa95` | `23fafa95b2e8c89ae2f97cb93cea81ea5c0589bfec78173a5482e4af56c91465` |
| 实际部署文件及归档 `41da7118` | `41da71186ad8b8e05e3e010c41e27cd8fecc3fcaa06c75d175bb8b90e814a79d` |

对这三版用 `dump_tgi <package> 00B1B104 40E0C000 3CAAEFBB` 读取 Unit。原版依据为已安装的 `SimCity-Scripts_287520926.package`，用 SC5 Asset Packager 的 `engine/package_io.py:index` 和 `engine/inspect_park_route.py:parse` 读取。同一 instance 的 `40E0C700` bundle、`40E0C500` sink、`48E0C000` 模板各按完整 TGI 取证。下文引擎分支来自本地 Dev Beta 反编译，包列和有效引用是静态证据。实际到岗与进店仍需同一部署哈希的游戏观测。

### 12.2 游客为何接不进来

三版 Unit 的 `0A0209A4` **都是同一张公园 donor 表**，包含 `6EA8E701`、`5D1EC901` 等 12 项。两种 parent 下，这张表都覆盖了同名模板列。

| Unit 样本 | parent | 本地 A4 | 有效观光载体 |
|---|---|---|---|
| 希望 `03b056b3`、`41da7118` | `7E4B99E5` | 公园表 | 四个均缺 |
| 希望 `23fafa95` | `4E01E311` | 同一公园表 | 四个均缺 |
| 原版威利斯 `40E0C000:8D007745` | `4E01E311` | 缺列 | 从模板取到 `E15369A2` 和 `75D22FD2/1/0` |

`SP/GetPropertyAsKeyArray.c:26–31` 委托属性查询，`SP/cPropertyList/GetProperty__3.c:30–38` 先查本地哈希，缺键时才查 parent。`GB/cEcoUnitEntry/FillFromProps.c:270–384` 依次查 A2–A5 四个哈希，按 `0A0209B2` 跳过排除项，再把每行的 bundle ID 交给 `BundlesForID`。因此**四个不同哈希的表可以共同生效，同名 A4 的叶值会盖住模板整张表**。本轮沿 parent 以叶优先解析四行、扣除有效排除表、扫描引用 bundle 的 `07E0AD23`，三版希望的有效声明均无 `AttractionLevel`、`TouristsAllowedW1–3`、`TouristW1–3`。三版都能经 `6EA8E701` 取得 `OpenShoppers`。这比只看 parent 或 A5 更能解释包内接线。§9.4、§10.4 所记 `23fafa95`「已恢复观光」只核到了 parent。

| 链路 | 原版记录 | 希望大厦的断点 |
|---|---|---|
| 吸引与配额 | `E15369A2` 将 `0E542F39` 和 `0D8C29F6/FE/0D8C2A01` 映射到四个 bin 的 **capacity**，并引用规则 `AE3774FA` | 有 400 和 10/10/10，缺消费它们的载体。威利斯叶为 150/75/50，配额生效后的实际来客量还取决于派发 |
| 三档接收 | `75D22FD2/1/0` 对应 W1/W2/W3。各声明 Tourist bin，将 `0E2660C6` 写入纪念品容量，将 `0AD6A725` 写入 `OpenShoppers` 初值，将 `0EF29855/56/57` 写入各档的 `33CFF411/12/13` 初值 | 已挂的 `CF116B9C/2B5A2BAF/362087BD` 均未声明以上游客接收行或游客 bin |
| 到店结算 | 以 W1 为例，bundle 的 `0A2725CE/CF` 含 `23E901EB→64E8EB52`、`75D22FD2→64E8EB52`、`C9FD3C2C→71840EB7`。前两个 sink 的 key resource 是 `SouvenirsW1 (6AE939A5)`，并检查 `OpenShoppers` 与分档资源 `33CFF411` | 配额、接收行、各档 bin 与路段可用容量需同时核对。增加吸引力或补顾客容量不能补齐这些引用 |

`40E0C500:75D22FD2` 的 `0A810F29/2A` 要求控制位 8、19、21、29、14、46、49、28、20 为 true，`0D6727D0/D1` 两个资源测试标为 inverted，`0DA7921B` 还要求把连接单位并入 key resource 的容量计算。尚未采集当前楼的这些运行值，因此即使补齐静态引用，也需观察哪一层放行。

**两个读数**。地标悬浮卡 `tmp/js/template_registry.js:6277–6279` 的分子读连接单位的 `TouristW1+W2+W3`，分母读 `TotalShoppers+OpenShoppers` 的 amount。§9.2 把分子写成 Shopper 三档、分母直接等同于叶属性，均应以这个查询为准。公园菜单的静态参数行和顾客占用可以独立出现。验收应同时看单栋楼 Tourist 三档、当班前后进店和观光收入。

### 12.3 劳工为何会少，缺工提示分别说明什么

- **历史红字有一个已解释样本**。 `204c0738` 记录 26 人、minWorkers=30，符合 `kPopulationWorkers` 的不足条件。其他包记录的关门文案需核对 `normal && ShiftLength.amount > 0 && WorkersNeeded.amount != WorkersNeeded.capacity`（`template_registry.js:6499–6502`）。告警图标还需查其触发规则。 `30645625`、`D07C5A04` 都把 minWorkers 映射到 `WorkersNeeded` 的容量和 `54B5A6C6` 的容量，所以这个字段同时进入模拟。
- **工人槽是规则输入，具体接收容量按每个 sink 的 key 计算**。 `30645625`、`D07C5A04` 的 `0A6FE74B/C` 确实把 A9/B3/B4 写入 `07F42755/56/57` 的初值。实际同挂的额外劳工 sink `1BB6F93B/863E354E/C25384A9` 分别读取 `B1731656/55/54`（社区资料称 ExtraWorkerWanted），还检查班次/运营相关控制位和三个资源条件。 `AddBundle.c:410–437` 支持 bundle 以 `0AF38802` 覆盖 sink 默认 key。审核时要沿对应 sink 行逐个解出 key、规则和测试条件。
- **§2.2 的资源测试方向需要更正**。 `FillFromProps.c:478–533` 将本地测试编成阻挡值（普通为 0，inverted 为初始化时的 capacity），`SinkCapacity.c:64–157` 在 amount **等于**该阻挡值时返回 0。连接资源测试在汇总 amount **等于** 0 或 capacity（inverted）时返回 0。无 key bin 的 10 只在通过前置测试且该 sink 未使用 connected key 时成立。
- **包里确有劳工载体**。 `03b056b3` 和实际部署文件有效引用 `30645625`（A3）、`5D1EC901`（A4）、`D07C5A04`（A5），三者都声明 `WorkersNeeded`，都有共享 sink 和重叠规则，例如 `EED48EFB`、`6769395E`。 `23fafa95` 仍有前两者，即使其 A5 去掉学校劳工包。威利斯自身排除了 `3A70FAA2`、保留 `30645625`，所以不能用模板上出现 `3A70FAA2` 来解释威利斯的 105 人。重叠声明是否造成运行时重复执行还需读具体规则注册与去重分支。
- **6/30 的原因仍需运行证据**。 `03b056b3` 的记录已有 A9=30、8 列均 116、minWorkers=5，汇总仍记 6 人。三个旧样本的 0.83–0.87 比值只能作为观察。定位少到岗应按一个完整班次记录 `TotalWorkers`、`WorkersNeeded` amount/capacity、各档实际候选 sink 的 key 和剩余容量，再看派发模板、路段挂接与 `DeliverTransaction` 是否完成。 `0DA69D6F=2C7C7986` 是内部 path entry 选择器（`SC/GetUnitInternalPathEntries.c:26–31`），写入它本身无法证明道路已挂上可接收该事务的 sink。

### 12.4 构建器怎样给同一栋楼组合行为

1. **意图与身份**。 `plan.rs:110–120` 解析 `semantics.labor` 和 `semantics.behaviors`，编译一个 `ResolvedBehavior`。 `behavior_family.rs:149–182` 选一个身份适配器并列出多项闭环。希望大厦的 park+tourism 选公园 donor，增加 Labor、Tourism、LandValue。 `compile` 在 park/education 意图下忽略旅游节点的 `templateInstance`。随后 `apply_to_behavior` 中已落地的 Labor 又把 parent/类别写成 `7E4B99E5/95A9D6FD`。这个写入能压过前面的家族或节点决定，构成现有混合处理器对身份的耦合。
2. **参数与闭环合并**。 `behavior_nodes.rs:531–630` 先写作者值并跑劳工缺省接线，`behavior_family.rs:280–368` 再按活跃闭环的字段和 bundle 归属剥 donor 残留。 `behavior_loops.rs:192–229` 对尚无值的字段补基线、加入 bundle、解除排除、保留活跃字段。希望的 `0AD6A725` 被观光保留为 390，劳工的六个学历字段也因归属保留。归属保护只能防删。两个系统若需要同一哈希写不同值，仍需按原版消费者确定可共存值或拒绝该组合。另一个可静态复现的覆盖缺口是 `landed_loops` 允许仅 W2/W3 的岗位激活 Labor，`AuthorOnly` 自动接线却要求 W1>0，导致那条路径跳过合计、学历和默认班次。 `SchoolSample` 还会先写共享字段 51/17，使 Tourism 的缺省保留那些值。
3. **实际发射**。 `stages.rs:1301` 先用 `menu::generate` 生成独立注册项。 `behavior.rs:192–265,700–930` 之后改新 Unit 的 parent、类别、类型化叶字段、排除表和 bundle，必要时改 Lot。它保留 donor 的 A3/A4，补 bundle 时仅检查 A2/A5，删除时遍历 A2–A5。引擎依次消费四行，因此当前 Rust 逻辑可能把 donor A3 里已存在的 ID 再写进 A5。希望的 `F236BB93` 就在这两行同时出现。叶级回读检查也只在 A2/A5 判断 bundle 存在或被移除。这个检查面无法证明四行的有效依赖闭合。
4. **迁移范围**。 唯一迁移基准 `reference/sc5-packager-0.8.0/behavior_profiles.py:21–30,62–95,106–132` 提供 `legacy-display` 和固定 `native-park-w1-v1`，固定 TGI/资源哈希、逐列优先展开、保留作者身份字段、逐记录回读。完整快照的 `backend_compile.py:140,232` 调用 apply/verify，`engine/independent_park_profile.py:11–25` 从通用园区模板展开、去掉 parent。Python 0.8.0 没有独立的劳工与观光组合处理器，Rust 的 `AuthorOnly`、观光嫁接和多闭环合并属于待验证扩展。复制或修补时需逐函数指出基准。原版资源提供机制证据与可复核的记录，后续扩展应新增模块。

### 12.5 可转成构建硬闸门的复现条件

| 声明 | 对最终包执行的检查 | 应拦截的样本 |
|---|---|---|
| 任一行为 | 用最终 Unit、实际 parent 和安装资源重放 A2–A5 逐列取值、B2 排除、bundle 解析。验证每个必需 property 的类型、容量/初值映射及 sink/规则/资源 TGI；报出被哪一层覆盖或排除 | 仅换 parent 的 `23fafa95` |
| Tourism | 分别检查吸引/三档配额及三档接收，核对 `OpenShoppers`、纪念品和分档资源消费。Menu2 只验证静态列绑定，动态到店用实际 bin 读取 | 只写吸引力、收入、容量和三个威利斯 A5 辅助包 |
| Labor | 从原版选定的配方核实 W1/W2/W3 的映射、排班及可注册的接收行。覆盖仅 W2、仅 W3、显式覆盖槽值，区分同一 sink ID 下的规则和 key，校验路网端点及模式 | 有岗位展示值却缺运行输入，或重复载体提供不兼容映射 |
| 混合行为 | 每个闭环在**同一份最终包**上重跑检查。对共享列、bundle ID、sink 行与身份写入报告冲突，核对校验所用值与实际写出的值 | 单项检查通过，合并后 parent 被改写、A4 被遮蔽或共享字段仍是别族基线 |

静态验收通过后，游戏验证按独立菜单、存档隔离和台账流程执行。冷启动先记录主菜单加载，再在一次性城市里按班次记录工人到岗和观光三档实际交付。此次只读检查没有构建、部署或修改存档。

## 13. 作者 Packager 对照：保留原版行为配方，再更换模型和菜单

### 13.1 版本和证据

用户给出的 `entity/__pycache__/__pycache__` 是 CPython 3.12 缓存。用本机 3.12 的 `marshal` 和 `dis` 读指令、常量与原始行号，再读同目录上两级 `SC5 Asset Packager (1).exe` 的 PyInstaller 归档。未启动 EXE、导入缓存模块或执行其中函数。其 PYZ 的 `behavior_profiles`、`place_effects`、`visitor_access` 与所给缓存的指令、名称、常量和行号一致。 `backend_compile` 有版本差异，下面涉及实际打包入口时，以该 EXE 的 PYZ 和内嵌 `engine/build_candidate.py` 为准。

观光配方数据 `behavior_place_effects_profile.json` 从 EXE 内读取，SHA-256 为 `5e471ec3bf8c5cf5e341b99a6a99defd8ec6f0a2b67064f8ad509ae5237864d9`。按其 TGI 逐项读取本机 Game、App、Graphics 和最高编号 Scripts `287520926`，**79/79 个资源哈希匹配**。测试缓存记录的是离线断言，配方报告把实机状态标为 `runtimeVerified=false`。

### 13.2 同一栋建筑如何得到配套行为

| 环节 | 作者做法 | 取证位置 |
|---|---|---|
| 选行为 | `behaviorProfile` 是单个受支持的预设。公园、水塔、污水、公共图书馆、水处理、太阳能和地价观光各有来源。固定服务拒绝自定义 `behaviorParameters`。观光开启选东京塔 Unit `00B1B104:40E0C000:084DB48F`，地价单开则按档位选原版公园 | `behavior_profiles.pyc:29,75`，`place_effects.pyc:77,125` |
| 落 Unit | 按固定 TGI 和 SHA-256 校验，沿东京塔 → `4E01E311` → 通用祖先逐列 `setdefault`，保留叶优先的原始 Property 记录，再去掉 parent。整个展开 Unit 替换初始 `tested-unit.bin` 的行为表，只带回指定作者字段、造价和维护费 | `place_effects.pyc:102–132`，`behavior_profiles.pyc:127–146` |
| 有限调参 | 在东京塔的整套接线上修改吸引力和 `0D8C29F6/0D8C29FE/0D8C2A01` 三档配额，地价档位配对处理 A5 和 B2。 A4 的 `E15369A2` 与 `75D22FD2/1/0` 已经写入输出 Unit。东京塔劳工由其原版接线携带，原始岗位 20/10/4、minWorkers=0、槽 0/0/0、前两列班次 20，A4 有 `3A70FAA2` 且 B2 排除 `30645625` | `place_effects.pyc:134–156` 和本机原版 Unit 展开结果 |
| 独立建造 | 局部改建造计数和 A4 的 `8A60B969` 记录。私有替代 bundle 保留本地规则 `EC4FA440`，移除五个全城计数规则引用。 EXE 的实际入口分配私有 instance 并同步改 A4 引用，再经 `extraResources` 将其加入输出包。共享城市资源保持原样 | `place_effects.pyc:24–75`，EXE `backend_compile.pyc:152–169`，内嵌 `engine/build_candidate.py:93–102` |
| 入口与界面 | 独立迁移东京塔界面查询字段和道路接入。 `visitor_access.compile_access` 从 `40E1C000:084DB48F` 的 128×128 地块读取 8 条通道与 13 个命名位置，适配当前地块尺寸与偏移，保留路边外伸量。菜单注册与这些模拟记录分别写入 | `behavior_profiles.pyc:154–188`，`visitor_access.pyc:10–57`，EXE `backend_compile.pyc:215–220` |
| 回读 | 构建前验依赖，构建后比较 Unit 的 `expectedUnitRecords` 及 Asset 的界面、入口、内景字段，检查资源引用 | EXE `backend_compile.pyc:261–284` |

东京塔原版展开后的 `0AD6A725=175`，配额 100/50/25，`0EF29855/56/57` 未显式出现。这组数据补充 §12.2：这些哈希有映射行，某一配方是否必须显式写列要看已验证的原版记录及缺省行为。构建闸门应针对选定配方验记录与映射，避免把威利斯的叶字段和数量硬套到所有地标。

作者支持的混合是**有原版供体的组合**。例如东京塔保留劳工经济并提供地价和观光，公共图书馆整套保留原版 5 岗位、最低 1 人和访客参数。 `test_library_behavior.pyc:15–34` 逐记录比对原版图书馆，检查公园底稿独有的字段已经退出。太阳能主楼保留一个固定原版面板，校验附属模块引用和地块容纳条件。观光配方明确把任意服务叠加列为未支持范围。这个边界避免为每种行为各选一套互相覆盖的 Unit。

### 13.3 HTzhiyao 为何可作公园参照

先验 SHA-256，再读 `entity/SC5_Asset-HTzhiyao_0.1.0.package`，其哈希为 `699d6752ba25e1bebcc6be38fd4fa15500aaf46e5a0149c226b86ac6596c6871`。Unit `40E0C000:97DF9619` 无 parent，A4 的 12 项与原版中收入公园 `FB6760A4` 展开结果一致，A5 有同样的 `265FB8DE/C0672F54` 再加 `8BA6D35D`。三档岗位 5/2/0、minWorkers=0、游客配额列 0/4/0、吸引力 1。A4 上的 `5D1EC901` 有劳工声明和映射，`6EA8E701` 有顾客与空位映射，B2 则排除了 `30645625`。因此 §7.6 的「只有一个 bin、零映射」适用于它当时抽查的 A5 范围，不能用来判定整栋楼退出劳工模拟。实际在岗数仍需同一包的运行记录。

HTzhiyao 与另三个目录内的成品都保存公园 A4，且 Lot `40E1C000` 含 `0F169C46=true`。这些静态样本与作者的公园/展示底座一致。东京塔观光分支的静态证据来自配方、入口代码和 79 个哈希匹配的原版资源，目录里的四个成品没有提供该分支的实机验收证据。

### 13.4 对希望大厦构建器的直接启示

希望大厦当前从公园展开结果保留了公园 A4，再试图换 parent 或追加少量 A5 字段。作者观光预设从一开始就按东京塔的**有效 Unit**落盘，配套输入列、A2–A5/B2 和规则引用来自同一供体。这个差异直接解释为什么作者的配方避开 §12 的 A4 遮蔽。若要沿作者方案实现，先找到与这个 EXE 对应的新版 `.py` 和配方数据，逐函数对齐观光 Unit 展开、建造计数、入口和回读检查。当前 vendored 0.8.0 只能直接对照固定公园路径。缓存和 EXE 在本轮用作实现证据。

## 14. 原版热气球公园：公园访客与地标观光各有接收链

普通热气球公园（§§）的菜单是 `43F0C900:B2CC43D1`，模拟 Unit 是 `43EFC000:8D7A596F`，沿 `48E0C000:B77AFA8A → 34CE7E17` 进入中收入公园模板。都市版（§§§）用 `43EFC000:8D7A596C`，沿 `B77AFA8B → 34CE7E16` 进入高收入公园模板。两者都可作为「保留公园身份并接收访客」的原版供体。

| 检查项 | 热气球公园（§§） | 东京塔 |
|---|---|---|
| 有效行为表 | 公园 A4 含 `6EA8E701`（顾客、空位、分档公园访客统计），A5 的 `265FB8DE` 提供公园访客 sink，`A20CAC71` 提供劳工映射和 39 条 sink 行，B2 排除 `30645625` | 地标 A4 提供 `E15369A2` 和 `75D22FD2/1/0` |
| 入场结算 | `265FB8DE` 的 `6A0AC143→24629080` 成功链到 `885CF9D1`，从 nearby 消耗 `SimShopperW2`，本地增加 `ShopperW2`、`TotalShoppers` 并减少 `OpenShoppers` | 地标的 `TouristW1–3`、`OpenShoppers` 和纪念品资源链 |
| 界面 | `template_registry.js:6443` 的 `kParkVisitorW2` 查 `ParkVisitorsW2ThisPeriod` / `ParkVisitorsW2PerPeriod` | `template_registry.js:6277–6279` 的观光人数查 `TouristW1–3` |

普通版的叶属性有 `0E542F39=1`、`0D8C29F6/FE/0D8C2A01=0/25/0`、`0AD6A725=10`。都市版配额列为 0/0/5。两版按 A2–A5 和 B2 解析后的有效 bundle（包括 DLC 组 `43EFC700:7339A237`）均无东京塔四个观光载体。 `7339A237` 挂一个事件规则引用。普通版的 `C7FDC13E` 另有 `DF03590D`，检查在场顾客与风速，再沿成功链触发公园幸福度相关事务。这些静态事实证实了公园接收访客的路径。25 是 Unit 的配额列值，当前证据无法把它当成实际每日区域观光客数，也无法确定玩家看到的每位访客从哪里来。

因此 §12 的四个 bundle 检查应限定在**东京塔/威利斯式的地标观光配方**。若希望大厦的目标是热气球公园式的到访，可保留公园身份，用对应财富档的公园供体核对访客 sink、接收资源、班次、入口和公园统计。若目标还包括地标式的三档观光客与纪念品收入，则用对应地标供体核对那套接收链。在混合行为上应按作者的做法保留已配套的原版记录，再验证最终包。

取证用安装包 `SimCity_DLC0.package`（SHA-256 `7bae9222763752a6cbd8a9c240eb72cf8bc17c2cc86821ca60cb6759c0ef36a1`）、`SimCityDLCAirships-Scripts_277156632.package`（`b2cb696f30922778b0486395d8070fe72d341d93be8f6d040b85d860344d89b5`）与 `SimCity-Scripts_287520926.package`。主规则库 `08068AEB:40800200:622B9CD7` 用 `tmp/school-dissection/parse_rules.py` 完整解码，SHA-256 与本地留存的主 AEB 匹配。图形入口的 TGI 与通路列见 [地块 PathEntry 调查](path-spawner-investigation-20261104.md#热气球公园-00b1b10443f0c0008d7a596c)。此次未进游戏采集运行值。

## 15. 商业观光与酒店：从空白 Unit Property 接入现有游客事务

### 15.1 三组原版建筑验证了哪些能力可组合

| 原版样本 | 实际有效属性与接线 | 能力 |
|---|---|---|
| 普通商业 `40E0C000:C05FED42`，Graphics 有 `40AEA10B:C05FED42` 等外观变体 | 商业祖先 `48E0C000:4BCCE372` 的 A2 提供 `E15369A2`，`AB08A7D9` 的 A3 提供 `75D22FD1`，有效 B2 均未排除。吸引力 1，配额 0/168/0，顾客槽 502，`0E2660C6=168` | **普通 C 本身既贡献游客配额，又接收 W2 观光客**。小型商业 `2DDD403E` 同样有效，配额 0/3/0、顾客槽 9、`0E2660C6=2`，原版吸引力列为 0 |
| 中收入商业酒店 `40E0C000:6F05DD57` → `48E0C000:971A4B13` → `B9217049` | A5 有 `729A6626` 和劳工接线，B2 排除 `E15369A2`、`75D22FD1` 及两项旧商业 bundle。吸引力和配额均为 0，`0E2660C6=68` 给 W2 空房初值 | **独立接收城市派来的观光客并转成酒店客人**。同档模板 `97229A2C/571DB83A` 分别给 12/670 个初始空房 |
| 原版综合娱乐 Unit `40E0C000:3CE0F852` → `48E0C000:9F25895C` | 同一 A4 含 `E15369A2` 和三档入住 bundle `352F4263`，叶 A5 含 `75D22FD1/0`。吸引力 400，配额 0/50/250，顾客槽 400，W2 接收的辅助初值 `0EF29856=100` | **同一 Unit 可以同时贡献配额、接待观光客和接收入住**。该模板还有赌场事件，因此仅作组合机制证据 |

以上按 Unit 四张启用表及有效 B2 解算。Graphics 负责地块外观与通路，模拟接线来自 Scripts 的 Unit、模板和 bundle。商业酒店有明确的模拟模板、空房 bin 和入住规则。此前 [RCI §6](rci-closed-loop-residential-commercial-industrial-20261110.md#6-酒店化商业--劳工--观光客) 依据名称表推断「邻近地标触发酒店化」，现在可证明商业酒店模板确实存在。具体由哪条区划规则选择酒店模板仍待定位。

### 15.2 沿 W2 追到规则读写

1. **贡献到城市池**。引用 bundle `E15369A2` 后，`0E542F39` 与 `0D8C29F6/0D8C29FE/0D8C2A01` 映射为本地 `AttractionLevel` 与 `TouristsAllowedW1/W2/W3` 的 capacity。 `AE3774FA` 按日内触发参数约 0.11 执行，从四个 bin 的剩余容量同时填本地和全局。普通商业的低吸引力和正配额可以同存。配额进入城市供给，目的地仍由后续事务选择。
2. **城市派客**。主规则 `13FC9444` 消耗 1 个全局 `TouristsAllowedW2`，增加 `TouristW2EnteredToday`，向事务写 `TouristW2` 与 `1C313589`。目的地候选明确包含观光接收点 `75D22FD1` 和酒店接收点 `729A6626`。上游 `937FE020` 等分支还检查全局吸引力及入口本地库存。容量测试、交通方式和挂接决定是否送达该 Unit。
3. **购物或观光**。 `75D22FD1` bundle 的接收行 `35ECEE5E/75D22FD1 → 64E8EB51` 消耗来客携带的 `TouristW2` 和 `1C313589`，本地增加 `TouristW2` 与 `TotalShoppers`，消耗 `SouvenirsW2` 与 `OpenShoppers`。该 bundle 将 Float `0AD6A725` 写入空位初值，将 Float `0E2660C6` 写入纪念品容量。 `0EF29856` 有映射，普通商业样本缺该叶列，综合娱乐样本显式写 100。两种形态应各按原版记录核对。
4. **入住及返回观光**。酒店 bundle `729A6626` 把 Float `0E2660C6` 写入 `OpenGuestsW2 (4E35BFA5)` 初值。其同 instance 的 sink `40E0C500:729A6626` 以该空房 bin 为 inverted key，并要求 Unit 控制位 8、19、21、29、14、20 为真。绑定的 `774570F5` 扣除来客的 `TouristW2`，增加 `GuestW2` 和 `TotalGuests`、扣空房，成功链 `82DBB772` 增加商业利润和旅游顾客满意度。原版有离店规则 `6EA6E257`，归还空房、将 `GuestW2` 再转换成携带 `TouristW2` 的事务，候选 `75D22FD1`。反向也找到 `0CACE342`，将建筑内的 `TouristW2` 派向含 `729A6626` 的候选组。它们证明两种接收端可接入同一游客生命周期。仍需按配方核对具体离店/清理规则是否注册，及来客能否在同一地块再次交付。

### 15.3 空白 Property 的构建契约

空白 Unit 要从两组职责起步：**给城市游客池供给**，以及**注册城市事务能抵达的接收端**。最小可验证单元先选一个财富档，例如原版商业 W2。下面是依赖最少的行为切片，完整可放置的包仍需运行底座与自建菜单，单独这几列的充分性尚未经过实机删减实验。

| 写入层 | 一档可执行的参考 | 最终包硬检查 |
|---|---|---|
| Unit 运行底座 | 先完整展开 `2DDD403E`（小型商业 W2）的有效 Unit，按 Packager 的叶优先展开模式保留同一供体的属性类型、flags、bundle 次序、排除项和规则特化。自建身份、模型和地块独立发射 | 新 Menu2、Unit、Lot TGI 及入口均可解析，确认营业、供电等要求能满足。禁止用公园 A4 或教育 parent 隐式替换供体接线 |
| 供给 | 在有效 bundle 行保留 `E15369A2`，该行可来自 A2/A4/A5。小型商业原始 Float 为吸引力 0、配额 0/3/0。若要参考同时提供正吸引力的普通商业，用 `C05FED42` 的完整配方，原值 1、0/168/0 | TGI、SHA-256、四列绑定、规则 `AE3774FA` 及有效排除表。单测城市配额与实际派发分开观察 |
| W2 接收 | 保留 `75D22FD1` 和同供体营业、经济底座。小型商业原始 Float 为 `0AD6A725=9`、`0E2660C6=2`。入口至少要让目标 sink 在对应交通模式的道路池取得正容量 | 到店 sink 的规则、key bin、各测试和 path entry 一起回读。按阶段观察 `TouristW2EnteredToday`、本地 `TouristW2`、`OpenShoppers` 及离店后释放 |
| 可选入住 | 按 `6F05DD57` 核对 `729A6626` 的商业酒店配方，或按 `3CE0F852` 核对同 Unit 吸引+观光+住宿的组合。后者的 `352F4263` 与单档酒店 bundle 的空房初始化策略不同 | 检查空房初值实际由谁写，入住/离店有没有已注册规则，并分别观测 `GuestW2` 和空房释放。 `0E2660C6` 同时可被纪念品容量和酒店空房映射消费，合并时要核对共享值 |

游客循环可以由同一栋楼承载供给和接收，派发环节依然依赖城市与交通系统。若希望大厦沿公园方案扩展，先保留现有公园访客与劳工路径，再用一个新处理器表达 W2 观光配方的实际依赖。若目标是从空白 Unit 复现整套行为，先在新菜单的独立资产上完整复现单一原版供体，按当前项目约束以对应 Packager `.py` 的展开、写入和回读为迁移基准。待城市配额、道路交付、进店占用、离店释放均实测后，再做字段裁剪实验。此次只完成静态取证和契约，未改构建器、构建或部署测试包。

证据来自 `SimCity-Scripts_287520926.package`（SHA-256 `77ddced4a23a818673e44c19649732390d8771bc5a1434583032e5841baedd04`）的 Property 和主 AEB `08068AEB:40800200:622B9CD7`（SHA-256 `a388e3c15080a4721e7d0047da180f6ee9f38292060c67892fa34b952564c381`），Graphics 原版变体按完整 TGI 查读。规则用 `tmp/school-dissection/parse_rules.py` 解码，派发候选从 12 字节 transport row 保留原始整数位读取，容量与路由见 `SinkCapacity.c`、`GetInitialDestinations__2.c`。

## 16. 科潘大厦：住宅可以脱离住宅区划，仍需完整的居民接线

### 16.1 科潘实际组合了什么

科潘 `00B1B104:40E0C000:4DC31F9B` 与东京塔同为地标类别 `406CCC43`，parent 均指向 `48E0C000:4E01E311`。科潘叶自带 47 项 A4，覆盖模板 A4，A5 为空，B2 排除 `30645625/5CB402D9`。扣除 B2 并去重后，69 个有效 bundle 均能在原版脚本库解析。

- **住宅已实证**。前次用户观察的 265 居民与 Unit 确有住宅经济一致。界面 `template_registry.js:5854` 读的是 `TotalResidents.amount`。属性 `0A7917CB=1100` 供 `AB9A0A3E` 初始化居民容量等 bin。东京塔与一些商业 Unit 也出现 `0A7917CB`，有效读者决定它有没有住人作用。
- **东京塔式观光在科潘有效表里缺席**。科潘解析得到吸引力 400、配额 100/200/100、接待参数 300，但有效 bundle 中无 `E15369A2`、`75D22FD0/1/2`。Unit core 也无 bin/sink 声明。实际扫描的有效 bundle 未声明 `AttractionLevel`、`TouristsAllowedW1–3`、`TouristW1–3`。这排除了靠地标模板继承得到该套观光功能的解释。若游戏里观察到游客，仍需采集其到达单位、agent 载荷和最终增加的 bin。
- **科潘证明的是地标身份可承载完整住宅经济**。地标建造身份的 `8A60B969` 与居民 bin 的提供者各自有独立引用。常规商业 `C05FED42` 已证明普通商业可同时贡献配额与接客，`3CE0F852` 已证明吸引、观光、住宿可在同一 Unit 共存。三组样本分别给出组合证据。

### 16.2 四种职责的 Property 和 bundle

| 职责 | 原版承载者 | 关键输入与实际读写 |
|---|---|---|
| 地价效果 | `40E0C700:8BA6D35D`，科潘 A2 中已启用，也是作者新版配方明确 pin 的 `landBundle` | 无本地 bin 或 sink。事件行与 `0C614160` 特化表调用地图规则。 `548DC34A` 以 `0E585AE0/AE1` 为参数操作 `CC5539CC = Desirability_RW2`，配对规则 `548DC34B` 操作 `CC5539CD = Desirability_RW3`。主 AEB 的 `302BE934`、`6EA26B7C` 等读第二对 `0E585B00/B01`。科潘两组原始记录为 Float 500 + Int32 2、Float 500 + Int32 3，flags `0x8000` |
| 住宅消费者供给 | `AB9A0A3E`（科潘 A4 的 W1 住宅经济） | `7A41EB3A → 80D55C93 → 91BF9728`：扣本地 `MoneyToken`、`ShoppingToken` 和在家 shopper，事务带 `SimShopperW1` 与钱。回家 sink `487E1CC6/9B6FB379 → 104D6998` 消耗来客的 `SimShopperW1`，恢复本地 `ResidentShopperW1/TotalResidentShoppersW1/TotalResidents` |
| 商业消费者接收 | W2 商业样本 `C05FED42` 有效 A3 的 `3A8263E2`。公园用 `6EA8E701` 提供库存，另配收入档的访客 sink | `0AD6A725` 初始化 `OpenShoppers`，`0DA3EE15` 给 `HotItemsW2` capacity。商业或公园 W2 sink `6A0AC143→24629080→…→885CF9D1` 消耗 `SimShopperW2`，增加 `ShopperW2/TotalShoppers`，扣空位。可与 §15 的旅游接收并列，双方消费共享的 `OpenShoppers` |
| 商业经济 | 按普通商业 Unit 解析有效表。 `C05FED42` 有 `3A8263E2`、`2B5A2BAF`、`362087BD`、劳工 `D07C5A04/F236BB93` 及 `75D22FD1` | `2B5A2BAF` 声明 `ProfitToken` 等账本，含 `0E5468F3` 容量映射，自身无顾客 sink。 `362087BD` 提供四个辅助 bin，自身也无 sink。普通商业还依赖岗位、班次、货物/空位和利润结算。商业酒店 `6F05DD57` 则排除 `3A8263E2`、`75D22FD1`，改用 §15 的入住接收 |
| 住宅占用与生活 | 科潘 A4 的 `AB9A0A3E` + `EE66978E` + `323A4CF2` 等。 `5D5EF1FF` 提供施工和外部接入相关 sink | `AB9A0A3E` 有 49 个 bin、34 条 sink 行，把 Float `0A7917CB=1100`、`0E33502E=800`、`0E335022=300`、`0A7917CC=100` 分别绑定总居民、W1 工人、W1 shopper 和孩童容量。 `EE66978E` 的 `200D7C9C→09BE9888→9B8566E9→B0B6387A` 写入居民 shopper 和工人，并读写城市库存。 `323A4CF2` 消费幸福参数 `0E546790/91/92`。 `5D5EF1FF` 的本地 bin 是 `ConstructionProgress` 和 `87AA975C`，其 sink 结算施工和 `AccessToOutside` |

居民的通勤有独立派发，例如 `07A49947→F0A4D812→978B279D` 将 `ResidentWorkerW1` 转成 `SimWorkerW1`，减少本地在家人数。科潘 Lot `40E1C000:4DC31F9B` 有两个 `418A18B1` 人行 entry、一个 `03106C2E` entry 和 13 个命名位置，三个路连列均为 `C186DA37`。住宅是否能移植要验证实际的外部路段接入与事务送达，避免把其它配方的路连枚举强加到科潘。

### 16.3 对行为处理器的结论

1. **地价最容易独立。** 它是地图效果，需保留或显式启用 `8BA6D35D`，再写两组正确类型的值和档位。现有 `behavior_loops.rs` 的 `LAND_VALUE` 没有声明 bundle，依赖 donor 已带，且四列都进 Float 写表，档位默认顺序也与科潘/东京塔相反。后续地价处理器的回读应检查有效 bundle 和 Float/Int32 类型，用开启、关停、拆除后的地图变化验收。
2. **「消费者」要明确方向。** 住宅是 shopper 的出发端与返家端，商店/公园是接收端。只配置一个接待容量无法凭空制造购买者。若在同一栋楼组合住宅和商店，先验证两端各自能向城市交付和接收，再观察同地块能否走完一次交易。观光游客和住客各用 §15 的接收规则，和普通 shopper 共享容量时要显式检查映射。
3. **住宅可剥离的是身份，需一起迁移的是居民经济。** 当前 `RESIDENCE` 只挂 `5D5EF1FF/EE66978E` 并写人口和幸福列。前者没有居民 bin，后者的入住规则会写由 `AB9A0A3E` 声明的 bin。当前代码只证明两条引用和几列成功写包，缺少居民与幸福载体及分档容量绑定，无法据此宣称住宅闭合。原版居民经济还读城市供给、交通和服务，移植应先用完整供体证实移入、通勤、消费、返家和离开，再逐项缩小依赖。
4. **商业也需要一个完整配方。** 可先用中收入的完整普通商业 Unit 做参考，再决定是否换为酒店接收或并入观光。单独挂利润、购物辅助或财富标记无法产生交易。混合时按同一最终 Unit 检查资源、规则、排除项和共享列，避免用整张商业 A4 覆盖住宅 A4。此轮只补静态研究，未改构建器或部署。

取证用 §15 的同一份安装脚本库与 AEB。科潘 Unit 载荷 SHA-256 `be4e0df8ca83400ff99e7f866aee946c61ad34ea231a8a89414c7d6238330f5c`，`AB9A0A3E` 为 `e5b0271bcc41091b4268ffa8dd3f0211876c6654e03aed517f73ca8d537b0e1d`，`8BA6D35D` 为 `ab49059339b61ab06dec4889cc750bf0b95e29d16ef027348f792e26d3e664ee`。地图名与居民/访客显示从本地 `tmp/js/template_registry.js` 核对。Packager 地价 pin 和作者展开流程见 §13。

## 17. 工作台行为模板与 bin 重叠检查

新增住宅叶子写入 `residenceProfile: "copan-residence-v1"`。构建器将科潘 A4 中的居民经济、入住、幸福、施工和出入相关 bundle 按样本次序追加到目标 Unit 的 A5，并解除这些 bundle 在叶表 B2 的排除。目标建筑沿用自己的菜单和身份。默认总人口为 1100。修改总人口时，居民劳工、消费者和儿童容量分别按科潘的 `800:300:100` 相对 1100 等比取整，幸福与健康规则参数沿用原值。成品回读验证 bundle、字段的值和类型，以及住宅 Lot 的 PathEntry 引用。

新增地价叶子写入 `landValueProfile: "copan-land-value-v1"`，挂接 `8BA6D35D`，分别写中、高收入档的地价值和原版 Int32 档位。该叶子与公园一起配置时，两组地价列由地价模板提供。无模板标记的旧工作台节点继续走原有编译路径。原版记录取值依据为前述科潘 Unit 和 AEB，展开方式参照 `reference/sc5-packager-0.8.0/behavior_profiles.py` 的 `resolve`（62 行）、`apply`（106 行）与 `verify`（125 行）。

添加叶子时，后端将该叶子与同一行为聚合下的叶子比较。索引来自已安装原版包的 `40E0C700` Property 中的 `07E0AD23` 列，由 Packager 的 `engine/package_io.py:index` 与 `engine/inspect_park_route.py:parse` 提取。不同 bundle 声明相同 bin 时，工作台以 toast 提示。两个叶子引用同一个 bundle，以及配方内部本来就共享 bin 的情况均去重。该检查覆盖已知模板和索引中的原版 bundle，实际运行时的继承、自定义外部 bundle 和规则写入仍需根据具体成品调查。

## 18. 普通消费者与商业的原版模板候选

两者都选基础游戏中收入（W2）样本。消费者叶子表达目的地接收城市居民派出的 `SimShopperW2`，住宅配方提供其中一种居民派发与返家机制。商业配方同时负责到店、货品、在岗营业和利润。

| 配方 | 原版样本 | 原始参数 |
|---|---|---|
| 普通消费者接收 | **中型小径森林（§§）**，英文 **Medium Path Park (§§)**。Menu2 `00B1B104:09878A01:9D122A62`，标题来自 `6C969DEE:0EB0CD2D`。Unit `00B1B104:40E0C000:FB6760A4` 的有效集合为 45 个 bundle。Lot 有原版人行入口 `418A18B1` | `0AD6A725=4` 个接待空位，`0DA3EE15=48` 绑定各档 `HotItems` 容量。首版面板只开放接待容量，保留原版库存容量 |
| 小型商业经营 | **W2 低密度普通商业**，模拟 Unit `00B1B104:40E0C000:2DDD403E`，Graphics 变体 `42ACA10B:2DDD403E` / `42ACA115:2DDD403E`。`Density=1`、`Wealth=2`，有效集合为 57 个 bundle，B2 排除 `BC7613A3` | 9 个顾客空位、9 个 W2 货品容量，利润容量 `0E5468F3=96`，`0AB985AA=6`。岗位 3/2/1，开工阈值 3，八班 Int32 为 `2/2/3/3/2/2/3/2`。首版开放顾客容量，沿用其余默认配比；岗位覆盖沿用已有招工面板 |

**消费者如何闭环。** 公园 A4 的 `6EA8E701` 声明 `ShopperW2`、`TotalShoppers`、`OpenShoppers` 和公园访客统计，A5 的 `265FB8DE` 注册三个 sink。接收链 `6A0AC143 → 24629080 → BDAB870C → DC0F531E → 885CF9D1` 消耗来客的 `SimShopperW2`、占用一个空位。原版公园统计走 `98E7F02D` 的失败分支 `6618DF97`。离店 sink `0992906E → 64C38F81` 的后续规则释放 `OpenShoppers` 并送回 `SimShopperW2`，`C0672F54` 也带有清理路径。首次构建宜保留同一供体的完整有效记录再观察入场、离开和空位恢复。任意 Property 上的最小组合仍需在这之后验证。

**商业如何结算。** `3A8263E2` 提供普通顾客、空位和 W2 库存，以及接收、返程和补货相关规则。已有货品时，`64C38F81` 可沿 `F82519C6 → 9D3A54DC → 2D0AB10A → 6A449AC0` 更新 `ProfitToken`。`2B5A2BAF` 提供账本，`6827D126` 注册利润周期规则 `9D061D19`，`362087BD` 提供辅助状态，`D07C5A04/F236BB93` 提供劳工接线。原版还保留 `E15369A2/75D22FD1` 的 W2 观光能力，配额 3，纪念品容量 2。第一份商业配方应整套展开供体，保留字段类型、规则特化、启用顺序和排除项，再验证补货、营业、购买、利润和离店。

**组合时检查 bin 和 sink。** 公园 `6EA8E701` 与商业 `3A8263E2` 有 10 个重复 bin，包括 `OpenShoppers`、`TotalShoppers`、`ShopperW2`、`HotItemsW2`。两个接线 bundle `265FB8DE/3A8263E2` 的 bin 声明无交集，但同时注册 `6A0AC143` 和 `0992906E`。同一 Property 上可先让商业配方提供普通消费者接收。若要加独立的公园接收，再验证共享空位如何分配。商业供体自带的 `75D22FD1` 与 `3A8263E2` 共用 `OpenShoppers`、`TotalShoppers` 和接待初值 `0AD6A725`，因此与观光编辑面板组合时应从一个设置派生该列。现有 toast 比较 bin，复用 sink 的检查是下一项构建契约。

取证使用 §15 的原版 Scripts 包和 AEB，通过 Packager `engine/package_io.py:index`、`engine/inspect_park_route.py:parse` 读取。两份叶 Unit 的 SHA-256 分别为 `4e43ee3d7b262ea0fd09b7bb3823ae6200ec99400e6738e353770b98690e9bf6`、`8c7c109819fefd35ea813e9ed16486d472b0767f9c0b10abe13b4fda7044ee3d`。名称从本地 Locale 表交叉读取，规则从主 AEB 解码。本节确认了静态配方候选与可达规则，实际接待、交易量和利润需用独立菜单测试资产在游戏中观测。

## 19. 希望大厦原生模板的实机记录

用户截图显示独立菜单资产「大连希望大厦（原生观光验证）」为绿色「运作中」，劳工图层显示 20 劳工。这确认了该资产套用原生模板并调整数据后，能够在游戏中运行并显示在岗劳工。部署包 `000_OpenSCP_HopeTower_native_landmark_contract.package` 的 SHA-256 为 `d07d0940d952b854edf9d5726e31edfa89f782fcb781053bdf96972e9841de85`，与部署台账一致，台账菜单 ID 为 `E5EF17C0`。

截图中的「每日旅客人数」是公园面板统计，三行分别为居民、孩童、游民。原生观光使用地标观光查询，读取 `TouristW1`、`TouristW2`、`TouristW3`（`template_registry.js:6277–6279`）。本次截图的观光到访读数未采集。
