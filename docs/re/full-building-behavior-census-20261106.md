# 建筑行为全普查：本体、DLC0、EP1 与最小行为结构

日期：2026-11-06。本文是只读逆向结论，未启动游戏、未改包、未改构建器。证据来自：

- 本体、DLC0、EP1 数据包（哈希见 §1）；
- EcoGame scripts 的 Unit / Template / Bundle / Sink / PathEntry 属性；
- `SimCity2013-source-tree` 反编译源码及 `SimCity.pdb`；
- `tmp/behavior-census-20261106/census.json`、`tmp/family-loop-census/{all,dlc0,ep1}.json`；
- 既有探针 `probes/property/behavior_census.rs`、`probes/menu/plop_behavior_map.rs`、`probes/menu/family_loop_census.rs`、`probes/lot/spawner_refscan.rs`。

文中的“观察”表示包或源码直接可见；“推断”表示由多个观察拼出的运行链；“未知”表示当前没有足够证据，不把资源名或单个字段当成行为证明。

## 1. 盘点范围与身份

本次核对的文件：

| 文件 | SHA-256 |
|---|---|
| `SimCity_Game.package` | `79A804494912D06B565B1EDE1CE183586D5C2E3D1AC9668E3D176518341C59E2` |
| `SimCity_DLC0.package` | `7BAE9222763752A6CBD8A9C240EB72CF8BC17C2CC86821CA60CB6759C0EF36A1` |
| `SimCityDataEP1.package` | `1A556F499CC643E6D96A2CCE8FF8492BC202E7E141C6B362ADA7ACCD27AE758D` |

本体模拟侧的主要组是 Unit `40E0C000`、Template `48E0C000`、Bundle `40E0C700`、Sink/Point `40E0C500`、PathEntry `40E0C400`。DLC0/EP1 使用相同的后缀语义但独立的组前缀；例如脚本 bundle 进入 `61EFC700` 或 `51EFC700` 等命名空间。DLC/EP1 数据包本身主要提供 Unit/Lot/Sink/Transport/Path，行为规则仍在对应 Scripts 包中，因此“某行为只在 DLC 数据包里找不到”不等于该行为不存在。

已扫描规模：本体脚本约 1214 个 Unit、254 个 Template、1308 个 Bundle、1047 个 Sink 条目；本体/Graphics/DLC0/EP1 合计 111 个 path filter。家庭闭环数据中记录 168 个 Unit 行，DLC0 72 行，EP1 96 行；这些统计是资源家庭的交集，不等同于菜单数。

## 2. 行为的真实数据模型

一个可运行的数值行为不是一个 named property，而是以下链条：

```text
Unit/Template parent 展平
  -> 四级 bundle 启用数组 A2/A3/A4/A5，减排除表 B2
  -> Bundle 的 resource-bin 三列
  -> Bundle 的 sink 绑定与规则引用
  -> 全局 resource / sink / rule 注册
  ->（需要网络时）PathEntry + 几何连通
  -> 运行期 bin amount / sink capacity / agent 或 UI 效果
```

Bundle 里的关键列是资源 ID `0x07E0AD23`、容量 `0x07E0AD24`、初值 `0x08635E12`；sink 绑定是 `0x0A2725CE/CF`，规则引用是 `0x08080D58..5A` 与 21 个事件槽 `0x08327D48..`。Unit 自身的水、电、垃圾 named 字段大多只是模板默认值或 UI 投影。源码中真正建立行为入口的是 `GB::cEcoUnitEntry::AddBundle`、`FillFromProps`、`RunUnitRules`、`GB::SinkCapacity`。

**关键观察：** 1214 个 Unit 中，水/电/垃圾资源 bin 没有写在 Unit core 表；它们来自模板继承的 bundle。因而“复制几个消费字段”不会产生消费行为。

## 3. 行为族普查与独立性

| 行为 | 最小运行链 | 是否必须 bundle | 是否必须 Unit/Template | 是否需要 path/sink | 结论层级 |
|---|---|---:|---:|---:|---|
| 电消费 | 电资源 bin + 电 sink + 消费规则 + 电网连通 | 是 | 是（通常经模板） | sink 必须；电网 path 由带 `Power Lines` filter 的载体连接 | 观察+源码定案 |
| 水消费 | 水资源 bin + 水 sink + 消费规则 + 水网连通 | 是 | 是 | sink 必须；`WaterMain` 载体连接 | 观察+源码定案 |
| 污水生成 | sewage bin/规则 + sewage sink；污水 agent 走专用 path | 是（生产型） | 是 | `Cars And Crap`/污水载体与 path | 观察+推断；产额换算仍未知 |
| 垃圾产生 | garbage bin + 产生/清运规则 | 常见是 | 是 | 产生本身不依赖 path；清运依赖车辆/道路链 | 观察；清运细节未完全闭合 |
| 岗位/劳工 | JobsW1/2/3、MinimumWorkers + worker bins + 阈值/到岗规则 | 是（完整模拟） | 是 | 通常不需要建筑专属 path，agent 到达可需要 path | 观察+源码定案 |
| 学生/游客流量 | capacity + 入学/访客规则 + sink + agent bins | 是 | 是 | 学生步行需要 Pedestrian path；校车另有 transport | 观察+源码定案 |
| 教育产出 | 学生 bins + 日程/考试/教育规则 + 全局专业化资源 | 是 | 是 | 到达链需要 path；产出规则本身不直接读菜单 | 观察+推断 |
| 地价 | 叶级 land-value 数值 + 全局 RCI 规则 | 不一定 | 通常是 | 不需要 sink；效果由全局规则间接应用 | 观察+推断 |
| 吸引力/游客 | attraction 数值 + 公园/游客规则 | 不一定 | 通常是 | 通常不需要专用 sink | 观察+推断 |
| 维护费/造价/解锁/类别 | 叶级字段被 plop/UI/门控直接读取 | 否 | 叶级即可 | 否 | 源码观察 |
| 二级单位/模块 | Secondary-unit 五件套或模块 entry + Lot + ConnectUnits | 不一定 | 是 | 由子单位自身决定 | 观察 |
| 动态 prop/bin draw | bin-draw 三列 + 同资源 bin + 规则实际填量 | 资源行为通常需要 | Lot 必须 | 不需要 path；数量由 bin amount 驱动 | 源码+实包观察 |

普查中 named `power_consumer` 528、`water_consumer` 526、`garbage_producer` 525、`jobs` 623；但命名字段与 bin/规则并不一一对应。尤其垃圾字段存在而无生产 bin 的单位，说明字段只能证明“候选/展示”，不能证明运行时产生垃圾。

## 4. 四类公共事业的最小结构

### 电和水

最小可解释结构是：

1. Unit 继承一条有效模板链，获得 bundle 槽位；
2. 消费 bundle 含资源 bin、sink 绑定和消费规则；
3. 对应资源已在全局脚本索引中注册；
4. Lot 的门前/服务几何能与对应管网 path 建立连通。

`SinkCapacity` 的源码路径只会返回容量或 0，不会因为缺 path 抛出建筑错误。未模拟、控制旗不匹配、bin 测试失败、无 connected unit，都会静默得到 0。由此可解释“建筑能放下但显示断水断电”：这是运行时容量为零，不是 Property 被判非法。

### 污水和垃圾

污水是生产/转运行为：污水资源 bin、sink 与 sewage 规则组成建筑侧行为，污水 agent 使用专用载体与 path filter。垃圾产生常见于 `D7C62954` 等模板 bundle；垃圾字段 `0x0C216562` 本身不是完整产额公式。垃圾车辆清运是另一条 transport/sink 链，不能把“有垃圾字段”直接等同于“场景中已出现垃圾车”。

### 劳工

劳工闭环比水电更清楚：JobsW1/2/3 与 MinimumWorkers 通过公共经济 bundle 绑定到 worker bins，再由阈值、到岗和暂停规则消费。没有该 bundle 时，叶级岗位数只是 UI/配置数字。劳工 agent 的具体到达路径由载体和建筑 Lot path 决定，不由 spawner id 决定。

## 5. path、spawner、prop bin 的动态语义

### path

PathEntry 有四个正交概念：材质/宽度；filter 成员；Local/Global scope；roadConnect 端点吸附。源码 `CreatePathsFromProps` 和路径搜索表明，`roadConnect` 只参与 Global 端点吸附，不是水电开关。水、电、污水之所以表现为受限网络，是因为它们的载体声明了对应 filter（Power Lines、WaterMain、污水载体）；行人/巴士等大量载体没有 filter 声明，默认可见路径更宽。

### spawner / UnitLocation

`0x0E1BAC61` 与 `0x0E1BAC62` 成对构成 UnitLocation 的 id 和 transform。id 是运行时名字哈希，不是 DBPF 资源键；小学和警察局共享大量相同锚点，职业由岗位/agent 资源和规则决定。开发者应克隆功能相近 donor 的锚点集合，不能发明新 id。

### prop bin / BinDraw

`cUnitBinDraw::FillFromProps` 读取最多 15 组平行列：资源 `0x0C12EF20+n`、transform `0x0C12EF30+n`、slot `0x0C12EF40+n`。资源必须先在 Unit 的 resource-bin 索引中存在；`Update` 按 bin amount（或 amount/capacity 百分比）增减可见实例。停车场车辆、工厂货架、帐篷/道具都走同一机制。缺列、缺资源注册或规则从未填充 amount 时，效果会静默为零。

## 6. 本体、DLC0、EP1 的行为覆盖方式

DLC0/EP1 没有另一套行为语义；它们扩展的是同一套 Unit→Template→Bundle→Sink→Rule 管线。区别主要是：

- 家族组前缀独立，不能把一个 DLC bundle 的 instance 当作本体 bundle 使用；
- 规则/bundle 常住对应 Scripts 包，数据包只提供引用端；
- EP1 继续复用基础电、水、污水、交通结构，同时增加自己的资源、transport 和楼层/模块族；
- 继承型建筑（例如泵站、楼层/容器型建筑）可能只在自身 Unit 声明外壳，真正闭环来自继承或模块 bundle。

因此编辑器的行为索引必须以“命名空间 + instance + parent 展平后的有效 bundle 集”作为键，不能只按 instance 或菜单分类查找。

## 7. 给资产编辑器的最小心智负担设计

编辑器应把行为制作变成可验证的 capability contract，而不是让开发者手填几十个 hash：

```text
BehaviorCapability {
  family: power | water | sewage | garbage | jobs | education | traffic | ...
  donor: verified Unit/Template family
  required_bundles: [namespace, instance]
  required_resources: [resource id]
  required_sinks: [sink id]
  required_paths: [carrier/filter, scope, connect]
  optional_visuals: prop_bin / spawner / animation
  acceptance: static checks + menu-load check + disposable-city runtime check
}
```

构建器每次只做增量校验：

1. 先冻结已确认的模型、材质、地面、树、菜单和 Lot 图形闭包哈希；
2. 新增行为时只展开该 capability 的 bundle/resource/sink/rule 依赖；
3. 失败必须指出责任链断点，例如 `missing bundle`、`resource not registered`、`sink unresolved`、`path carrier mismatch`、`bin never populated`；
4. 对 prop-bin 给出“资源量→预期实例数”预览；对 spawner 显示 donor、位置和频次；
5. 将行为层与表现层分开做差异报告，禁止用“整包字节不同”代替行为等价声明。

推荐的最小处理器顺序是：`TemplateResolver → BundleClosure → Resource/SinkResolver → RuleResolver → PathContract → OptionalBinDraw/Spawner → StaticGate`。每个处理器只接受结构化输入并输出证明或明确失败原因；不在失败时自动换 donor 或静默补字段。

## 8. 证据边界与下一步

已确认：bundle 是水、电、完整劳工、教育、交通等数值行为的载体；菜单与行为正交；path 是网络/载体可达性层；spawner 是位置锚点；prop bin 是由资源量驱动的动态绘制层。

仍属未知：垃圾静态系数到每日产额的换算、TransportOps 的全部分支、教育考试规则的逐条业务含义、少数 EP1 专属资源的注册名，以及运行时名字哈希表的完整反解。这些问题需要专门的离线探针或用户手动进行的 disposable-city 观测，不应通过继续修改 s24 来猜测。

## 9. 本轮离线追踪：三个未知项的收敛

### 9.1 垃圾：已闭合“清运”侧，生产倍率仍未完全闭合

源码与既有离线快照共同给出一条比“垃圾字段=产额”更精确的链：

```text
本地 Garbage bin
  -> C1F4C627：local Garbage input=1/output=0（清空本地库存）
  -> nearby Garbage output=1（向接收方转移，静态存在）
  -> global GarbagePickedUpToday 35B2D890（全局统计，静态存在）
  -> E08882A3：清理 GarbageDisplayCount / 2F207D6B / 41FA6F69
  -> global GarbagePickUpNeeded F66950A5 减少
```

观察证据：9 条独立快照中本地垃圾库存按 `applyCount` 精确归零，合计减少 5750 个内部资源单位；这证明 C1F4C627 是清运/转移规则的强证据，但不能把内部单位直接称为吨，也没有证明每个记录都对应同一种建筑或车辆。

生产侧已知 `D7C62954` 把 `GarbageOnlyAmount=10` 通过初始化资源 `EEE64A65 (TotalGarbageToCreate)` 放大到 100；`74A74AB3` 从 RecyclingLevel 读取剩余容量再写入 Garbage。尚缺的是该规则的时间触发、`applyCount` 生成和零售初始化倍率的逐函数闭合。因此资产编辑器可以验证“垃圾生产 capability 的资源/规则闭包”，但暂时不能把 10 或 100 当作通用每日产额。

### 9.2 TransportOps：已证明它是“交易路由选择器”，不是路径几何字段

`CreateTransaction` 的源码显示：规则中的 `mTransportID` 先通过 `IndexFromTransportEntryID_2` 解析成 TransportEntry；如果 TransportEntry 带资源 bins，就复制到交易，否则从规则的 local/global/nearby/map 输出操作收集资源并建立交易 bins。之后 `CanDeliverTransaction` 将 `mHandlerID` 或 `mTransportID` 映射到已注册的 `cIEcoTransportHandler`；没有 handler 时直接返回允许，有 handler 时交给 handler 判定。`DoAddTransaction` 再把交易交给所有游戏 handler 和目标 transport handler。

这给出三个可用结论：

1. 行为规则可以没有 transport（纯本地/全局 bin 更新）；
2. 一旦声明 `TransportID`，必须能解析 TransportEntry，并且 handler 注册决定是否经过交通系统；
3. PathEntry 是 handler 寻路/载体层，TransportOps 本身只携带交易资源与路由身份，不能代替 Lot path。

`CreateSecondaryUnits` 也已闭合：每个二级单位先按 `mChance` 做确定性伪随机门控，解析 UnitEntry，叠加相对 transform；创建成功后，若 `mConnectToPrimaryUnit` 为真才调用 `ConnectUnits`。`ConnectUnits` 会双向写入 connected-unit 列表、刷新 sink capacity，并通知注册的 game handlers。因此“二级单位存在”与“它是否参与主建筑行为”是两个独立断点。

### 9.3 Spawner：哈希算法可确认，完整名字表不可由静态资源恢复

源码中的名字常量统一通过 `EA::StdC::FNV1_String8`（初值 `0x811C9DC5`，按需要 lower/any case）生成；例如 `Eat_garbage`、`road`、`sidewalk`、`path` 和大量 handler/resource 名字都在反编译初始化器中直接出现。这证明 UnitLocation/spawner id 属于名字哈希命名空间。

但 472 个 Lot 中的 7761 个 UnitLocation id 在当前本体、Graphics、DLC0、EP1 属性资源中没有对应资源键。静态分析可以可靠地得到 transform、频次和 donor 复用关系，不能从 DBPF 反推出完整原始名字。编辑器因此应保存“哈希 + donor 语义 + 空间层级”三元组，不能把未知哈希重新命名或生成新 id。

## 10. 对行为处理器的新增契约

本轮结果建议把三个检查写成硬性 contract：

| 处理器 | 必须证明 | 失败含义 |
|---|---|---|
| `GarbageTransferResolver` | 生产/清运规则分别存在；资源方向、nearby/global 目标和显示清理链可解析 | 只能声明“有垃圾字段”，不能声明垃圾行为已闭合 |
| `TransportResolver` | `TransportID → TransportEntry → Handler` 三段可解析；无 handler 要显式标记为本地交易 | 交易会静默退化为本地或完全不执行 |
| `SpawnerResolver` | id 属于已知 donor 哈希集合，transform 数量与 bin/animation 对齐 | 新哈希只会成为无人消费的锚点 |

这三项都能在离线构建阶段给出责任链错误，不需要用实机反复 PID 调参。

可复核入口：

- `tmp/behavior-census-20261106/census.json`
- `tmp/family-loop-census/all.json`
- `tmp/family-loop-census/dlc0.json`
- `tmp/family-loop-census/ep1.json`
- `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/GB/cEcoUnitEntry/AddBundle.c`
- `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/GB/SinkCapacity.c`
- `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/SC/cUnitBinDraw/FillFromProps.c`
- `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/SC/CreatePathsFromProps.c`
