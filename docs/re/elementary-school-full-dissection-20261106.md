# 小学 C264A268 全行为扒底报告（2026-11-06）

对象：EcoGame Unit `00B1B104:40E0C000:C264A268` + Lot `00B1B104:40E1C000:C264A268`
+ 模板链 `48E0C000:{7E4B99E5, 2A25CF66, F98BCB35, 9EC474FF, 593B61D2}` + 引用 bundle。
方法：转储逐字段 × 反编译引擎源码（约 30 个函数）× 离线 AEB 规则库全量解析
（32 条规则 0 未解 + 24 条一跳链展开）。**未改任何代码，未启动游戏。**

详细机器可读产物（本文的引用基底，均在 `tmp/school-dissection/`）：
- `field-map.md` — 字段→机制完整对照（Unit 74 ID + Lot 56 ID）
- `minimal-loops.md` — 九个最小闭环 + 独立/成链终表
- `rule-table.md` / `rule-table.json` — 32 条规则逐条：触发/资源操作/成功失败链/推断行为/置信度
- `parse_rules.py`、`resource-symbols.json`（2302 个 UI 资源符号）

## 0. 一页结论

1. **小学的一切模拟行为都不在属性字段里**——字段分三类：身份/容器/系数。
   行为全部由 **bundle 声明的 bin + ERZ 规则** 承载；属性经 bundle 的绑定表
   （`0x0A6FE749/4A/4B/4C`）和规则专用化（PropOp）进入模拟。
2. **bundle 槽位是四层独立生效**：`0x0A0209A2/A3/A4/A5` 分属 Toy 覆盖层/Toy 基
   类/模板/叶子——小学的主经济 bundle **30645625 来自 Toy 覆盖层**（不是叶子），
   树 bin bundle **E7690450 来自 Toy 基类**。没有模板边 = 失去这两层 = 主经济
   与树 bin 全部消失。
3. **水/电消费规则已正名**：`D51CB94C`（电）/`D54F8B18`（水）——sentinel 输入
   999999 被 fromBin 用 `PowerNeeded/WaterNeeded` 当前值替换后消耗（净消耗
   = Needed），检查周期被 `kPropPowerConsumer_Rate`/`kPropWaterConsumer_Rate`
   专用化为 72；失败链写 `Power 0→1` 等（断电表现路径）。**但这两个规则不
   在小学 5 个转储 bundle 的引用里**——水电规则经"模板/Toy 层候选 bundle"
   或规则侧挂接进入，本地 270 容量 bin 的创建机制仍是最大未决点（诚实标注）。
4. **九类行为的最小闭环**全部给出（§4），每一环标注所在层。核心答案：
   **"让建筑具有某种行为" = 该行为的 bin 声明 + 绑定表行 + 规则引用 + 全局
   资源四件套，缺一即死数据**；纯 UI/表现字段才可单字段独立。
5. 噪音污染：小学 75 字段无任何 noise 字段，规则表亦无——**未找到机制**，
   不编造；参照系应是交通 Handler 族。

## 1. 数据分层（谁贡献什么）

| 层 | 成员 | 贡献 |
|---|---|---|
| [叶] 叶子 Unit | 75 字段 | 身份覆盖、经济数值、容量、劳工配置、模块五件套、排除表 |
| [模] 模板链 | 教育 7E4B99E5 → Toy 覆盖 2A25CF66 → Toy 基类 F98BCB35 → Unit 基类 9EC474FF → 空根 | 类别（`0x0AFE27B1`/`0x099B4C1F`/`0x08A16042`）、费用科目（`0x0AD65F07/08`）、**bundle 槽位 A2/A3/A4**（30645625 经济包、E7690450 树包）、水电默认值、模块 addon 容器 `0x0AFE27CA` |
| [bundle] 40E0C700 | 启用 4 + 排除 1（经 Toy/模板候选层实际进入的更多） | bin 声明（`0x07E0AD23/24`）、绑定表（`0x0A6FE749/4A/4B/4C`）、规则引用（`0x08080D58+0..2`）、事件槽（`0x08327D48+0..20`）、sink 引用（`0x0A2725CE/CF`） |
| [规则] AEB | 32 条直接引用 + 24 条一跳链 | 全部行为执行 |
| [全局] | Power/Water/Garbage/学生族/教育族资源、Transport 定义、脚本表 | 城市级资源池与索引 |
| [Lot] | 56 字段 | 模型//path 六件套/UnitLocation 锚点/车辆 bin/灯光/地面/行人贴靠 |

继承语义（新发现）：**单赢家父指针回退**（`cPropertyList::GetProperty→mParent`），
数组不拼接——叶子排除表 `0x0A0209B2=[F90C8A94]` 会**遮蔽**模板的排除项
`E0C02E1F`；而 bundle 槽位 A2-A5 四层**各自生效**（不遮蔽）。

## 2. 行为逐类解剖

### (a) 电力消费 —— 最小闭环（8 件，缺一不消费）
1. [叶] `0x0AFB9882`=72（Rate，UI+规则专用化周期）
2. [叶] `0x0B88904E`=90（Amount=每次消耗量）
3. [叶] `0x0C09DAA0`=270（Capacity=3×Amount）+伴生 `0x0C09DAB2`
4. [bundle/模] 含 marker **`0x19E5DB54`** 的 bundle（BuildTotalsJSValue 据此把
   entry 计为电力消费者）
5. [全局] Power 资源 `8E8B4E10` 的 bin（⚠ bin 创建机制未决点）
6. [bundle] sink 表（30645625 `0x0A2725CE` 族）+40E0C500 定义组 → 连接判定
7. [规则] `D51CB94C`：`local Power 999999→0` + fromBin `PowerNeeded→op0` +
   propSpec `kPropPowerConsumer_Rate`→simRate；成功链查 `OpenForBusiness`
8. [Lot] path/roadConnect 连通（断连→sink 容量 0→不消费，s18 实测定案）

### (b) 水消费 —— 与 (a) 同构
`0x0C09DA65`=90 / `0x0C09DA5A`=270 / 伴生 `0x0C09DA6E` / Water `9E3C3DFA` /
规则 `D54F8B18`（`kPropWaterConsumer_Rate` 专用化，s14 叶子有 `0x0C09DA6A`=72）。

### (c) 垃圾
[叶] `0x0C216562`=10（GarbageOnlyAmount，叶子独有）+ [全局] Garbage
`7CB23A4E` + [规则] 生产规则 ⚠（运行时 100/日 ≠ 静态 10——字段是存在性+
系数候选，产额由规则决定）+ 清运链 ⚠ 未闭合。

### (d) 雇佣劳工 —— 闭合最好、且主要由模板层承载
[叶] `0x0A7917C8`=5（MinimumWorkers）+`0x0A7917C5/C6/C7`=20/5/0（JobsW1/2/3）
→ [bundle] **30645625**（Toy 覆盖层 A3 槽）绑定表：bin 5378D6F9 容量=1×5、
54B5A6C6 容量=3×5、47BEDF8B/88/89 容量=JobsW×1、D3A424B9/BA/BB 初始量
→ [规则] 阈值规则 `11514BA4`（in=out=5）+ ExtraWorker/到岗/暂停链（938552F2 等）
→ [全局] 工人 agent。**没有 30645625 → C8/C5/C6/C7 是死数字。**

### (e) 小人到达（步行 + 校车）
步行最小闭环：[bundle 6C042367] 入学规则 `B52C889D`（sink 78B96709→
`0x0A2725CF`）+ bin 族（Student/TotalStudents/StudyingE2/SpotsForKids，容量
10000）+ [叶] `0x0AD7FAC9`=150 经 6C042367 `0x0A6FE749/4A` 绑定 8 bin +
[Lot] path 六件套（ranges/points/tangents/indices/PathEntry/scope+connect，
缺一整网拒绝）+ [Lot] UnitLocation 锚点。
校车 = 之上加：校车 sink/Transport 定义 + 停车子单位五件套（`0x07E0AD0A/0B`+
chance+rotate+connect [叶]）→ `CreateSecondaryUnits`→`ConnectUnits` + bus 车辆
bin draw。⚠ TransportOps 语义未反编译。
每日循环：`C193A5BB`（day=0.25）fromBinOps 清 TotalStudents。

### (f) 教育/知识产出
学生入学（(e)）→ 教育 bin 族（475A73A5/A6/A3/AC、465A7230/37/3A、455A705E，
容量=150=课桌数）+ 初始量 [叶] `0x0B988054/57/5A`=150 → [规则] 教育提升/
考试规则 ⚠（`C193A5BB` 清零 ExamsPassedE2 有运行时证据；3AF32DBE=批量入学
150）→ [全局] 专业化资源 → 科技等级。**叶子没有承载"产出"的字段**
（`0x0AD7FAC9` 只是席位容量）；技能注入 `0x07E0AD04/05` 小学未写。

### (g) 地价
[叶] `0x0E585AE0`=500+`0x0E585AE1`=3（档）、`0x0E585B00`=500+`0x0E585B01`=2
→ [全局] LandValue → RCI 消费规则 ⚠。**"叶子字段+全局规则"型闭环，不需要
bundle**（与劳工型对照）。菜单统计行参数 `0x0EB1FC22` 直接引用它。

### (h) 噪音污染
**未找到**：75 字段无 noise 符号、32+24 条规则无 noise 资源操作。交通噪声
属交通 Handler 族。不编造。

### (i) 扩展槽位（两套机制）
- **自动子单位**（校车棚 8C4245A6）：[叶] 五件套 `0x07E0AD0A`(ID)/`0B`(偏移)/
  `0x08635E13`(chance)/`0x08635E14`(角度)/`0x0D363DF3`(connect) → 引擎
  CreateSecondaryUnits（伪随机 ×chance）→ ConnectUnits。
- **手动购买模块**（教室 5DE9CD2A）：模块自带 Menu2+Lot(`0x0AFE2FEB`=true)+
  Unit(`0x0DF1467C`→kModuleResourceIDs[16])；主体 Menu2 挂载三件套
  `0x0E9F828E/8F/90`；限额=主体 `0x0CC8BE51+i`（小学 4/5/1/1）。
- **显示 ≠ 运行**：面板显示模块只需 [叶]`0x07E0AD0A`+[模]`0x0AFE27CA`；
  模块规则在跑需要完整模块 entry+Lot+ConnectUnits（s17 教训的机制解释）。

### (j) 本轮新解析出的"此前未知行为"
- `NumberOfGradeSchoolBases` ±1（城市小学计数，放置/拆除时）
- `SchoolBusT1StopUnlocked` +1（校车站解锁）
- 8 条 day=0…0.875 的 `StudentsNeeded_XX` 分时检查 + 8 条 global Hour 分发
  （24h×3h 窗口）——学校的作息引擎
- `3AF32DBE` 批量入学 150（常量=课桌数）
- 排除容器 F90C8A94 = RCI 住宅税基座规则组（被叶子排除表屏蔽，不启用；
  其链目标含 `F6A1C9B9:=EducationLevel`）

## 3. 独立存在 vs 必须成链（终表）

**可独立（单字段即有效）**：Lot 全部表现字段（模型/LOD/灯/贴花/编舞/地面/
名字）、菜单价 `0x0A8A41DA`、解锁 `0x0BF5D2E3/E4`、限额显示 `0x0CC8BE51-54`、
占位半径、替换优先级、自动子单位五件套、维护费/地价/电耗 UI 直读字段。

**必须成链（缺一环即死数据）**：
1. 一切成 bin 的数值 = bundle `0x07E0AD23/24` + 绑定表 `0x0A6FE749/4A`（或
   `4B/4C` 初始量）行——实证：`0x0A7917C8=5` 经 30645625 绑定才是
   WorkersNeeded 5/5；`0x0AD7FAC9=150` 经 6C042367 绑定 8 个教育 bin。
2. 一切成规则的行为 = bundle `0x08080D58`/事件槽引用必须在 AEB
   `IndexFromUnitRuleID` 命中，否则 AddBundle 静默丢弃。
3. 一切成网络的服务 = sink + 40E0C500 定义组 + path/roadConnect 连通。
4. 一切成身份的类别 = `0x0AFE27B1`/`0x0AD65F07/08`/`0x099B4C1F` 全在模板层
   ——**没有模板边 = 类别悬空 + 维护费无科目 + 失去 Toy/模板 bundle（经济
   包和树包都在那）**。

## 4. 遗留 unknown（诚实清单）

Unit：`0x0AFB9886`、`0x0BCD3E46`、`0x0BF46DD1/DD7`、`0x0C3E9F98/99`、
`0x0C579CFA`、`0x0CEC98AB`、`0x0D7B1448-B`、`0x0D9BDB7F`、`0x0DC1DB39/43/45`、
`0x0E965A70-75`、`0x0EA88940`、`0x0ECAEB01`、`0xB0221D00`、`0xB7760E4C`；
Lot：`0x0BFF56D5/DF`、`0x0CABB2FC`、`0x0CAFBDC3`、`0x0CAA8F17`、`0x0FE01484`、
`0x0D7AF049`；机制级：水电 bin 创建、TransportOps 语义、教育考试规则逐条
归属、垃圾产额换算、噪音机制。资源符号未定名：`C02F343B`、`BE2C0EB3`、
`219D6623`、`10272391`、`7366D1C2`、`9715F557`。

## 5. 对"让一个建筑具有某种行为"的直接答案

以"消费电力"为例的最小集合：叶子写 Rate/Amount/Capacity 三件套 → 挂一个
含 marker `0x19E5DB54` 与 sink 表的 bundle（或继承自模板）→ 规则库中存在
`D51CB94C` 型消费规则（sentinel+fromBin+Rate 专用化）→ Lot path 连通。
**克隆小学之所以脆**：75 字段每一个都被规则网络专用化消费，改一个/漏一个
即触发失败链；**正确路径** = 按目标行为从 §2 各闭环挑最小集合，在极简形状
（s14 型）上逐项叠加，每步用菜单加载验收器把关。
