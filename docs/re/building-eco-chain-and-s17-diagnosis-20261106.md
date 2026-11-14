# 建筑 eco 能力链全解与 s17 合法性诊断（对拍反编译源码 + 包逐字段实测）

日期：2026-11-06。证据三类：dev 反编译树（`SimCity2013-source-tree`，约 7100 个带函数体 dump）、安装版实包（`SimCity_Game/Graphics/App` + `SimCityUserData/EcoGame/SimCity-Scripts_287520926` + Locale）、s17 包逐字段转储（`tmp/s17-package-dump.txt`，本次与原版小学三件套做了全量 diff，diff 脚本产物见本文 §6）。本文不重复既有文档已闭合的结论，只给出处；**新证据**集中在 §5–§7。

## 0. 结论速览

1. **"eco 能力"不是引擎硬编码**。exe（GB::/SC:: 命名空间）只提供解释引擎：属性→资源 bin、规则（ERZ）→资源操作、sink→连接判定、控制位→可铲除性。公园提地价、吸引游客，学校收学生、提教育，全部定义在 EcoGame 脚本数据（ERZ 规则库 + 属性记录）里。建筑=五层记录：Menu2（入口）+ Unit（模拟身份）+ Lot（非模拟/图形）+ bundle（规则容器）+ PathEntry（路网），由脚本主表（`40800200`）声明的组扫描组装。
2. **能力来自"模板继承 + 叶子覆盖 + bundle 规则"**：Unit `0x00B2CCCB` parent 链到服务模板（教育 `7E4B99E5`、地标 `4E01E311`、电力 `6D766518`……），模板绑定类别 ID/费用科目/解锁/默认值；叶子覆盖造价/容量/水电/吸引力/地价；bundle `0x0A0209A5` 把规则挂到 AEB 规则库。**没有 bundle 的 Unit = 没有规则 = 死建筑**。
3. **s17 静态包已经非常接近原版小学**：Unit 与小学 `C264A268` **逐字段全等**（仅设计值不同），locale 表字节格式合规，Unit/PathEntry 都在主表声明的扫描组内。因此"不被视为合法建筑"**不能再靠字节对拍找原因**，剩余风险收敛为 4 项静态疑点（§7 R1–R4）+ 3 个运行时判别点，全部给了判别实验（§9）。
4. **菜单乱码是两条独立链**，s17 的 locale 标题链在字节层面已合规；"乱码"更可能是已验证的**详情绑定批量中断机制**（`kMaintenance` 等必需查询返回 null → 整批组件不更新 → 模板占位符原样显示）或标题查找 miss（`***`），需按 §9.4 判别表区分。
5. 方法论警告：`target/release/examples/find_instance_all.exe` 旧编译产物**存在漏报**（同 instance 的 `00B1B104` 条目漏报，本次用 `list_group`/`prop_dump` 新编译复核纠正了一处误判）。任何"某 TGI 不存在"的旧结论都应换新编译探针复核。

## 1. 证据地形

- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\`：dev-beta PDB 恢复反编译树。`HEADER_INDEX.txt` = 41900 函数符号→地址；`include\GB|SC\` = 类型头；`src\GB|SC\` = 函数体 dump。命名空间 `GB::`（GlassBox 模拟）/`SC::`（表现与工具），没有 `cSC*`。
- 属性 ID 与规则名 ID 都是 **FNV-1(lowercase(name))**（seed 0x811C9DC5，prime 0x01000193；`crates/dbpf/src/erz.rs::fnv1_name_hash` 与 dev `HashName32` VA 0x475BF0 双重证实）。
- 零售 exe（`SimCity.pdb`）地址与 dev 不同；已对拍的零售地址见 `docs/re/package-directory-chain-evidence.md`、`docs/re/package-ecogame-resolution-closure.md`。

## 2. 数据模型：一个建筑 = 五层记录 + 一个主表

| 层 | TGI 组 | 角色 |
|---|---|---|
| Menu2 | `00B1B104:09878A01` | UI 入口：`0x0DB9FC63` uiCategories（唯一归属边）、`0x00B2CCCB` parent（功能家族边：plop 工具全挂 `AF042E9A`，道路挂 `12E3214B`，公园项挂公园子根 `F970A822` 等）、`0x0975695F` menuUnit→Unit、`0x0A09F5FA/FB` title/desc（Text{table,id}）、`0x0EB1FC05+0xEB1FC22` 统计行模板+参数、`0x0DE84DC9/DD3/DDC` 解锁三件套、`0x0E9F828E/8F/90` 模块挂载三件套 |
| Unit | `00B1B104:40E0C000` | 模拟身份：模板 parent、类别 `0x0AFE27B1`、造价 `0x0A8A41DA`、维护 `0x09AE19D7`、容量 `0x0AD7FAC9`、bundle 启用 `0x0A0209A5`/排除 `0x0A0209B2`、水电垃圾 `0x0AFB9882/0x0B88904E/0x0C09DAA0/0x0C09DA65/0x0C09DA5A/0x0C216562`、地价 `0x0E585AE0(+E1)/0x0E585B00(+B01)`、吸引力 `0x0E542F39`、岗位 `0x0A7917C3..C8`、lot 链接 `0x08E17ED7`、子单位 `0x07E0AD0A` |
| Lot | `00B1B104:40E1C000` | 非模拟/图形：脚印 `0x0A14EDB3..B7`、模型 `0x00F9EFBB..BE`、path 点/切线/索引/段 `0x0CAA680D/0x0CB00ED8/0x0CAA6832/0x0CAA6841`、PathEntry 表 `0x0CC8FE7A`、scope/roadConnect `0x0D9A4E54/0x0D9AA2BB`、UnitLocation `0x0E1BAC61/62`、车辆 bin `0x0C12EF20..6C`、灯光 `0x0CAA8F10..17`、地图矩形 `0x0CCB7FC8..D6` |
| bundle | `00B1B104:40E0C700` | 规则容器：规则引用 `0x08080D58+0..2`、事件规则槽 `0x08327D48+0..20`（21 槽）、资源 bin 定义 |
| PathEntry | `00B1B104:40E0C400` | 路网段定义（parent 组 `48E1C400`），经 `IndexFromPathEntryID` 解析（`SC/CreatePathsFromProps.c`） |
| 主表 | `00B1B104:40800200:<gameID>` | 声明扫描组：unit `0x07E0AC00→[40E0C000]`、resource `0x07E0AC01→[40E0C100]`、map `0x07E0AC03→[40E0C200]`、zone `0x09138113→[40E0C300]`、path entry `0x09138114→[40E0C400]`、point `0x0A30A557→[40E0C500]`、transport `0x0A30A558→[40E0C600]`、bundle `0x0A30A559→[40E0C700]`；类别注册表 `0x08A16043/44`；版本 `0x09EEB11C`(UInt32)；NS 属性表 `0x0C947ABC→40E0C800` |

**组装**（dev `GB::cEcoGameScripts::Init` 0x61F830 → `AddEntitiesFromGroupIDs` 0x61EBA0）：EcoGame 版本表挂接脚本库（外部 priority 100 / 本地 2，`SwitchToScriptsVersion` 0x5EBCB0）→ 对主表声明的每个组，调 **全局 PropertyManager 的 `GetPropertyListIDs(group)` 枚举全部属性表**（含普通包！RCI 单位就在 Graphics 包）→ 逐个建 `cEcoUnitEntry` → `FillFromProps`（0x64A3A0）→ `AddBundle`（0x6490B0）把每个 bundle 的规则 ID 经 `IndexFromUnitRuleID` 挂进 AEB。**因此自定义包里的 Unit（组 40E0C000）会自然获得 unit entry**——s17 满足此条件。

**模板继承发生在属性表层**：`0x00B2CCCB` 不被任何 GB 函数读取，而是 `cPropertyManager::GetPropertyList` 首次加载属性表时合并父链（见 `docs/re/fire-station-economy-anomaly-20261020.md` 加载链线索节）。继承链五层（小学/威利斯实测）：叶子 → 服务模板（`48E0C000:7E4B99E5` 教育 / `4E01E311` 地标…）→ Toy 覆盖层 `2A25CF66` → Toy 基类 `F98BCB35`（水/电默认）→ Unit 基类 `9EC474FF` → 空根 `593B61D2`。

**放置特化**（`cToolPlop::CommitPlopTransform` → `GB::cEcoGame::CreateUnit` 0x60D980 → `StartCreatingUnit` 0x60D5F0）：`CanCreateUnit_2` 检查事件规则 0 型的 MinApply 资源门槛；`cEcoUnit::FillFromProps` 合并（已含模板的）属性建资源 bin（容量取 Capacity 族），bundle 规则挂接；`CanRunEventRules(type0)` 失败 → 释放槽位返回 -3；带 `0x08635E16` 的 Unit 只建 SecondaryUnits 返回 -7。**拒绝原因全表**：`SC::cToolPlop::UnitAtValidLocation` 0x7ED4A0 的 `tValidUnitCreation` 枚举（InvalidPlacement/Overlapping/OnRoad/OutsideBox/CannotAfford/CannotPlaceOnGround/TerrainTooSteep/NotOnSnapPoint/CannotPlaceOnWater/OverlappingOnFireUnit/InvalidRoadIntersection）。

## 3. 六条 eco 链（属性 → 规则 → 全局资源 → UI/结算）

### 3.1 公园/地标：地价与游客
- 地价：叶子 Unit `0x0E585AE0`(值)+`0x0E585AE1`(财富档 int32，2=中价 3=高价)、`0x0E585B00(+B01)` 第二档 → 全局 LandValue 资源/map → RCI 规则消费。
- 游客：吸引力 `0x0E542F39`（模板默认 500）→ 全局 `kResourceIDAttractionLevel`/`kRegionalAttractionLevelTotal` → 游客 Agent 生成/到达规则 → `TouristW1/W2/W3 Entered/Satisfied/LeftEarly` 统计 → `kPropTaxes_ToyIncomePerResourceW1/W2/W3 0x0EE2A50D/E/F` 折算收入 → 费用科目 `0x0AD65F07/08`（`ExpensesFromMaintenanceTourism`/`ExpensesFromTourism`）。
- 解锁/限额：`0x0BF5D2E3/E4`（解锁 bin + 数量）、`0x0E51C90F`（共享额度）、`0x0CC8BE51`（单项上限）。

### 3.2 学校：学生流（全链闭合，运行时实测见 `docs/re/elementary-school-mechanism-20261022.md`）
- 校车 Transport → sink（`0x0A2725CE`，sink entry `0x78B96709`）→ 规则 `B52C889D`：`nearby KidE2 −1 / local Student(2C1DA112) +1 / TotalStudents(06EDFBED) +1 / StudentStudyingE2(CF0B7832) +1 / SpotsForKids(05DAADA6) −1`。席位+在校=容量（原版 150+扩建 800=950）。
- 每日 06:00 规则 `C193A5BB`（day=0.25 周期）用 `fromBinOps` 清 TotalStudents——**TotalStudents 是当日统计不是在校人数**。
- 教育→科技：技能系统 `GB::cEcoGame::AddSkillForUnit` 0x5FD920 / `UnitSkillLevel` 0x5FD8A0（`mSkillDeltas[0..2]` 阈值）；技能数组来自 `cEcoGame::InitGame` 的 `SimProperties[0x7E0AC02]`；居民教育 W1/W2/W3 与 `Work_JobsW1/W2/W3 0x0A7917C5..C7`、`Work_MinimumWorkersForProduction 0x0A7917C8` 挂钩；工业科技等级 = 全局专业化资源被大学/学院教育规则抬升（逐条规则语义归属仍需 ERZ 反编译，见 §10）。

### 3.3 水电与垃圾：纯数据驱动
- 静态属性 → bin：`kPropPowerConsumer_Rate/Amount/Capacity = 0x0AFB9882/0x0B88904E/0x0C09DAA0`、`kPropWaterConsumer_Amount/Capacity = 0x0C09DA65/0x0C09DA5A`、`kPropGarbageProducer_GarbageOnlyAmount = 0x0C216562`；**Capacity = 3 × Amount**，各带同值伴生哈希（水 `0x0C09DA6E`、电 `0x0C09DAB2`）。
- 消耗：bundle 规则按 72-tick 周期执行，`delta = applyCount × (output − input)`（`ApplyBinRule`）；资源 ID：Power=`8E8B4E10`、Water=`9E3C3DFA`、Garbage=`7CB23A4E`。
- 连接判定：`cEcoSinkEntry` 的控制位/资源测试不过 → sink 容量 0（断水断电表现）；SC 侧 `cTransportPipe`/`cPipeSinkEntry::FillFromProps` 0x76B610。

### 3.4 财政
费用科目是模板上的 Key（`0x0AD65F07/08`）；维护费 bin（UI 查询 `connectedUnits/<unitref>/bins/4227066323`）；税收 `kPropTaxesBuildingTaxValue 0x0A7917C4`；全部经规则与全局 bin 结算，UI 走 RequestGameData。

### 3.5 模块/扩展设施（三层）
1. 放置层：模块是独立菜单项，主楼 Menu2 带 `0x0E9F828E/8F/90`（挂载 Key 数组/XY 偏移/旋转）。
2. 模拟层：主楼 UnitEntry `mSecondaryUnits`（68B/条：mID、mChance、transform、mConnectToPrimaryUnit）→ `GB::cEcoGame::CreateSecondaryUnits` 0x60D740（伪随机 ×mChance）→ `ConnectUnits` 0x5FED40 互写 `mConnectedUnits`；`SC::GetModulesOfBaseUnit` 0x7D8830 用 lot 属性 `0x0AFE2FEB` + `FindConnectedUnits` 枚举模块。
3. UI 层：`cToolBulldozer::IsModuleOfEditorUnit` 0x87DFF0、`cEditorUI::GetModuleLimitCount` 0x7D5FB0；限额 `0x0E51C90F`/`0x0DF1467C`(kModuleResourceIDs 16 项)/`0x0CC8BE51`。
- s17 的模块显示来自 Unit 子单位 `0x07E0AD0A=[8C4245A6]`（校车棚）+ 模板 addon 容器 `0x0AFE27CA=7987C2C3`——纯属性继承即可显示，不代表规则在跑。

### 3.6 铲除
`SC::IsUnitAllowedToBeDeleted` 0x799250：

```c
if (unit->mControlFlags & 0x4000000000u64) { *errorFlags = 1; return false; }  // 不可删除位
if (unit->mControlFlags & 0x100000)        { *errorFlags |= 2; return false; } // 受保护位
```

控制位由 ER2 规则的 `cUnitControlFlags`（local/global set/clear + filterNearby tests）运行时写入——**可铲除性是规则状态，不是静态属性**。道路走 DRP 分支（`IsDRPReachable`/`DestroyWholeUnit(AndMergePaths)`/`IsModuleOfEditorUnit`）。注意 `docs/re/debug-tool-census-20261024.md` 的教训：流浪汉帐篷"缺拆除 player rule"的静态推断被实机否定，铲除问题必须实测。

## 4. 文本系统与"乱码"的两条链

- **标题/描述链**：`SP::cTextProperty{mTableID, mInstanceID}` → UI JS；locale 表资源类型 **0x0A98EAF0**（JSON），解析器 **`SP::cJSONResourceFactory`**（`RegisterJSONResourceFactory` 0x451D90，`AddRecordPackType(0xA98EAF0, 0xA9C85A5, 0xA98EAF0)`——挂在 record-pack 容器体系上）；locale 数据库注册走 `cLocaleManager::RegisterLocaleDatabases` 0x4A61D0（embedded 包类型 `0x06EFC62A`、instance=FNV1(语言名) + `data/Locale/<lang>/*.package` + BaseLocale −1000；retail 无 en-us 递归）。
- **实机三定律**（`docs/re/menu-item-relations-20261018.md` §9.1，2026-10-19 闭环）：①裸表任意目录可用（扁平 TGI）；②格式必须 BOM+`\r\n`+缩进宽松排版，紧凑单行整表丢弃；③原版表 ID（6C969DEE/50AA0BEA/4B54417A）被 Locale 数据先占，重定义不生效；全新表 ID 任意目录可用。另 Text 数组 item_size 必须 520（§9.2）。
- **详情绑定链**（已验证，`docs/re/fire-station-economy-anomaly-20261020.md`）：零售 UI `Private_BuildDataBindingResults` 任一必需查询返回 null → **整批回调取消**；`kMaintenance` 查询 `connectedUnits/<unitref>/bins/4227066323`（模拟侧 bin）。特化失败/unitref 漂移 → 占位符（`~population_amount~`、`~maintenance_cost:simoleons~`）原样显示 + 铲除找不到目标——**消防局异常与 s17 同症状族**。
- s17 的 locale 表 `0A98EAF0:02FABF01:7FA8E661` 实测（`tmp/s17-locale.bin`）：BOM+CRLF+4 空格缩进+键 `"0x00000001"/"0x00000002"`，与官方 `Locale/zh-tw/Data.package` 逐形状一致（官方表组亦全部 `02FABF01`）；表 ID `7FA8E661` 为全新 ID（SHA-256(标题)[:4]）。**标题链字节层面无缺陷**。

## 5. s17 与原版小学逐字段 diff（本次新证据）

dump 来源：s17=`tmp/s17-package-dump.txt`（20 条目）；小学 Menu2=`SimCity_Game.package 00B1B104:09878A01:29873FA2`；小学 Unit=`SimCity-Scripts_287520926 40E0C000:C264A268`（`tmp/school-rule-schedule-20261023/property-40e0c000-c264a268.txt`）；小学 Lot=`SimCity_Game.package 40E1C000:C264A268`（`tmp/school-lot-dump.txt`，本次新 dump）。

### 5.1 Unit（40E0C000）——全等
s17 的 75 条属性与小学**逐字段一致**，仅设计值不同（容量 150→250、造价 13000→50000、维护 300→100）；唯一多出 `0x09FB78CB`（名称 Text→自有表 id1，小学该属性在 Lot 上、指向共享表 `6C969DEE:22643031`）。parent（`48E0C000:7E4B99E5`）、类别（`95A9D6FD`）、四 bundle、排除容器、水电全套、车辆 `0x08E17ED7`、校车棚子单位 `0x07E0AD0A` 全部在位。

### 5.2 Lot（40E1C000）——克隆 + 公园化改造 + 收窄
- **s17 多出 57 项**（小学 Lot 没有）：车辆 bin 扩展（`0x0C12EF2C/3C/4C/50/5C/6C`）、图形映射（`0x0CE71B10-13`、`0x0CD4E49C`）、公园交互字段（含 `0x0BC5051B = key(T67771F5C,G0B074E5A,I2BDC8BDD)`——指向 Game UI JS 脚本容器）、建造/贴花参数（`0x0C070D10-17`、`0x02A907B5..BC` 建造阶段×2）等。来源：构建器从 `34CE7E14`（热气球公园 lot）交互链恢复的字段（`asset_build_m1/native_profile.rs:51-67,1191-1197`）。
- **结构性收窄**（最可疑的一组）：

| 属性 | 小学 | s17 | 含义 |
|---|---|---|---|
| `0x0CAA6841` path 段 | `[0,2,3,4,5,8]`（6 段） | `[0,8]`（1 段） | 可行走路网 |
| `0x0CC8FE7A` PathEntry 表 | 3 条（`7C9C4FFF`,`03106C2E`,`418A18B1`） | 1 条（`03106C2E`） | 路网段定义 |
| `0x0D9A4E54` scope | 3 项 | 1 项 | 路径作用域 |
| `0x0D9AA2BB` roadConnect | 3 项（`F0F61CED`,`C186DA37`×2） | 1 项（`C186DA37`） | **道路连接** |
| `0x0CAA8F17` 灯光名 | `F`/`roof` 混合 | 11×`SC5_Light` | 灯光语义名 |
| `0x0CCB7FC8` 地图矩形 | (96,96) | (192,96) | 地面/小地图 |
| `0x0FE01484` 范围盒 | (−34,−25)−(34,16) | (−96,−48)−(96,48) | 地块范围 |
| `0x0D7970ED` | `6A1785B4`（教育城市类别容器） | `D0621548` | lot 级类别绑定 |

- **同 TGI 冲突**：s17 自带的 `40E0C400:03106C2E` 与游戏脚本库同 TGI（脚本库实测含 `03106C2E/418A18B1/7C9C4FFF` 各 1 条）。脚本库 priority 100 > 普通包 0，**游戏版获胜**，s17 克隆成死条目（内容克隆自同一来源，暂无实害，但 fail-closed 构建器必须禁止）。

### 5.3 Menu2——归属与家族
| 字段 | 小学 | s17 |
|---|---|---|
| `0x00B2CCCB` parent | `48E1EB00:AF042E9A`（plop 工具根） | `48E1EB00:F970A822`（**公园子根**） |
| `0x0DB9FC63` uiCategories | `[6A1785B4, 95A9D6FD]`（教育子栏+教育类别） | `[07049250]`（公园子栏） |
| `0x0975695E` | `C710B6E9`（kCategoryIDBuilding） | **缺失** |
| `0x0E9F828E/8F/90` 模块挂载三件套 | 教室模块 ×4 | **缺失** |
| `0x0A09F5FA/FB` | 共享表 `6C969DEE`/`50AA0BEA` | 自有表 `7FA8E661` id 1/2（合规） |
| `0x0EB1FC05/22` 统计行 | 同款 | 同款（`50AA0BEA:0D9AB016/0EBCE055` + 参数 250/500）✓ |

### 5.4 已满足 / 未满足清单
已满足：Unit 教育身份（parent/类别/bundle/容量）；主表扫描组覆盖（40E0C000、40E0C400）；locale 表格式与 ID 全新性；统计行与图标/rollover 引用闭合；Placement 所需 lot 层字段（脚印/模型/PathEntry/UnitLocation/车辆 bin）在位。
未决（按可疑度）：Menu2 家族与缺失字段（§7-R1）；path/roadConnect 收窄（§7-R2）；运行时代际/优先级（§7-R3）；铲除控制位状态（§7-R4）。

## 6. 为什么此前"总觉得包没问题但游戏不认"——机制层面的解释

1. **属性表能读 ≠ 模拟能跑**。UI 面板的容量/模块显示读的是 unit entry 属性（swept 属性表），而水电/教育/铲除读的是**放置特化后的 cEcoUnit 状态**（bins、规则、控制位）。两层数据源不同，可以一半工作一半不工作——s17 正是这种"半合法"状态。
2. **同 TGI 不是同优先级**。普通包 priority 0，EcoGame 脚本库 priority 100（外部）/2（本地）；`FindDatabase` 首命中。s17 的 PathEntry 克隆被脚本库遮蔽就是实例。
3. **同版本切换短路 + 已构建脚本缓存**（`GetCurrentGameVersions`/`SwitchToScriptsVersion` force 参数）：磁盘覆盖 ≠ 重新组装；每次部署迭代应更换 instance/版本并重启游戏。
4. **UI 批量绑定中断**：一条必需查询 null → 整批不更新 → 看起来像"乱码"的占位符其实可能是模拟侧 bin 缺失。

## 7. s17 诊断：候选根因排序与判别实验

> 结论边界：静态包证据到 §5 为止；以下 R1–R4 均未实机定案，每项附最小判别实验。

- **R1（最可疑）放置工具家族**：原版一切 plop 建筑 Menu2 parent=`AF042E9A`；公园项才是公园子根。s17 parent=`F970A822` 且缺 `0x0975695E`(kCategoryIDBuilding)、缺模块挂载三件套。若游戏按 parent 家族选择放置路径（公园放置 vs 建筑放置），特化/铲除/UI 全套行为都会走错分支——与"半合法"症状吻合。
  判别：构建 s18 变体 A——仅把 Menu2 parent 改回 `AF042E9A`、补 `0x0975695E=C710B6E9`、uiCategories 改 `[07049250, 6A1785B4]`（保持公园入口+教育归属），其余不动。若水电/铲除恢复即定案。
- **R2 路网连接收窄**：s17 path 6 段→1 段、roadConnect 3→1、scope 3→1。若入口连接判定失败，建筑永远不接入水电网络 → 不消费；详情 `kMaintenance` bin 查询为空 → 绑定批量中断（乱码）。
  判别：游戏内查看 s17 建筑是否出现"未连接道路/水电"警告图标；或 hook `cPipeSinkEntry`/`UpdateUnitSinkCapacities` 0x5F7730 观察容量。
- **R3 运行时代际**：部署后未重启/同 instance 覆盖导致旧 package handle 或旧脚本缓存获胜（`docs/re/package-ecogame-resolution-closure.md` §6）。
  判别：全新 instance（s18 新 ID）+ 冷启动；hook `Manager::FindDatabase`（retail 0x910B40）记录 `40E0C000:7FA8E661` 的获胜数据库与 priority。
- **R4 铲除控制位**：`mControlFlags` 的 0x4000000000/0x100000 位被继承规则置位且状态卡死（例如连接不成立的分支不清位）。
  判别：hook `SC::IsUnitAllowedToBeDeleted` 0x799250 读 errorFlags 与 mControlFlags；对照原版小学同状态值。

## 8. "让一个建筑被游戏接受并具备任意行为"——完整配方（工程清单）

以"公园菜单入口 + 小学行为"（s 系列目标）为例，按依赖顺序：

1. **身份**：新 instance（SHA-256(标题)[:4] 线性探测），五层记录同 instance：Menu2/Unit/Lot/Locale/图标+rollover；PathEntry 用新 instance 或直接引用游戏脚本已有 entry（**禁止 same-TGI 克隆**）。
2. **行为**：Unit `0x00B2CCCB → 48E0C000:<服务模板>`（行为模板决定类别/费用/解锁/限额），`0x0AFE27B1` 显式写类别，`0x0A0209A5` 启用模板 bundle（规则都在 bundle 里）；数值覆盖写在叶子。水电整族修改（Rate/Amount/Capacity+伴生哈希）。
3. **菜单**：`0x0DB9FC63` uiCategories 决定入口（可多容器）；parent 建议 plop 根 `AF042E9A`（R1 判别后定）；title/desc=自有新表（`0A98EAF0:02FABF01:<新ID>`，BOM+CRLF+缩进）；Text 数组 item_size=520；统计行引用原版表+参数哈希。
4. **Lot**：脚印/模型/UnitLocation/车辆 bin（E1/E2/E3 全族+`0x0C12EF60=true`）；**path 段/PathEntry/scope/roadConnect 保持与模板参照一致的数量**，scope 仅 `local`；灯光语义名保持原版命名习惯。
5. **构建器 fail-closed 检查**（rci-school 文档 §7 六项 + 本次新增）：same-TGI 冲突检测（输出包与全部已注册库比对）；Menu2 parent 家族断言（plop 类工具必须 `AF042E9A` + `0x0975695E`）；path/roadConnect 数量与参照 lot 的 diff 报告。
6. **部署**（AGENTS.md 验证规则）：旧测试包先归档到 `D:/ea-games/simcity_offline/test-archive/`；新包用新 instance；冷启动游戏；一次只改一个变量。

## 9. 定向运行时取证方案（先取证后改包）

工具已就绪：`tools/dynamic/rule-logs/capture.py`（`--scope unit|unit-effects|unit-consumption|conditions`，Frida 被动监听 delta stream）、`tools/dynamic/ui-observer/native_trace.py --profile bridge-commands`。

1. 规则层：`--scope unit --rule-id 0xB52C889D`（入学规则）+ `--scope unit-consumption`（水电 input>0）——对照原版小学同城市同窗口基线（`docs/re/elementary-school-mechanism-20261022.md` 有量级基线：15 秒 69,811 次入口）。
2. 铲除层：hook `IsUnitAllowedToBeDeleted`（dev 0x799250）读 `mControlFlags`/errorFlags。
3. 数据库层：hook `FindDatabase`（retail 0x910B40）+ `AddEntitiesFromGroupIDs`（dev 0x61EBA0）确认 `40E0C000:7FA8E661` 枚举自哪个库、unit entry index。
4. UI 层：`native_trace.py --profile bridge-commands` 打开 s17 详情，捕获 `Null data binding data` 与 query/result 配对——区分"乱码"到底是标题 miss 还是绑定中断。
5. 特化层：读放置后 `cEcoUnit::mID`（+0x0C）与 bins——确认特化产物与 entry 属性一致。

判别表：`***`=标题 locale miss（查表注册/代际）；`~x~`占位符=绑定中断（查模拟 bins）；字符变字形=编码/JSON 解析（查 cJSONReader 日志）；菜单在但建筑行为全无=放置/特化分支（查 R1/R3）。

## 10. 未决与后续

- ERZ→ER2 完整规则反编译器（AgentTests/TransportOps 语义归属）——"哪条规则消耗水电、哪条写教育"的最终命名仍缺。
- `0x08A16043/44` 类别注册表、`0x09D43DA8/C4` 数据层注册表的完整语义。
- ecogame pack-record 容器（`cEcoRuleSet::Read` 0x627810）结构解码。
- 零售 VA 重定位表（dev 地址≠零售），成链零售 hook 需逐个对拍。
- s18 变体 A（R1 判别）构建与实机验证（遵循 §8.6 部署规则）。

## 11. R1 判别实验执行记录（2026-11-06 补）

- 构建器增强（`asset_build_m1`）：新增 `buildingCategoryInstance` schema 字段（写入 Menu2
  `0x0975695E`，parent-free 与 legacy 双路径）；scene-semantics 台账 detail 携带菜单家族
  字段（parentInstance/buildingCategoryInstance/categoryInstances）；新增无头 CLI
  `openscp-build`（设计文档 M5）。`asset_build` 回归 97/97 绿。
- s18 变体 A 已构建并部署：instance `197BF5C3`（与游戏三库零冲突）、R1 三字段静态验收
  全过、产物 sha256 `0b964743…` 与两次独立构建逐字节一致；s17 已归档至 test-archive。
  完整日志与构建器设计目标评估：`tmp/build-logs/20261106-s18-variantA/eval.md`。
- 待实机读数（冷启动后）：标题是否正常、是否消费水电、能否铲除、是否"合法建筑"。

## 证据文件索引

- `tmp/s17-package-dump.txt`（20 条目全转储）、`tmp/s17-locale.bin`（194B 原始字节）、`tmp/school-lot-dump.txt`（本次新 dump）
- `tmp/school-rule-schedule-20261023/`（小学 Unit/模板/bundle 全家 + 主表 `property-40800200-622b9cd7.txt`）
- 反编译关键文件：`GB/cEcoGameScripts/AddEntitiesFromGroupIDs.c`、`GB/cEcoUnitEntry/{FillFromProps,AddBundle}.c`、`SC/IsUnitAllowedToBeDeleted.c`、`SC/cToolPlop/*`、`SC/CreatePathsFromProps.c`、`SP/cLocaleManager/RegisterLocaleDatabases.c`、`SP/RegisterJSONResourceFactory.c`、`anonymous/FindNonSimProps.c`、`GB/FindUnitMasterNonSimProps.c`
- 既有闭环文档：`rci-school-path-prop-mechanism-20261105.md`、`willis-tower-landmark-mechanism-20261027.md`、`elementary-school-mechanism-20261022.md`、`menu-item-relations-20261018.md`、`package-ecogame-resolution-closure.md`、`package-directory-chain-evidence.md`、`fire-station-economy-anomaly-20261020.md`、`s11-simulation-consumption-20261105.md`、`university-s12-runtime-package-20261105.md`

## 12. R2 定案：roadConnect 编码写反（2026-11-06 实机+PDB 双证据）

s18 实机读数：标题正常（R1 菜单家族修复生效，locale 链闭环）；但无水电、
不可铲除、夜里完全不亮灯、详情面板裸占位符（`~population_amount~`、
`Left1/Right1`——消防局异常同款）、放置时出现的校车车库（lot 装配态 prop）
随游戏时间被移除。电力覆盖图显示建筑从未接入路网。

PDB 枚举解码定案（`include/SC/tPathScope_ord2502.h`、`tRoadConnect_ord2503.h`）：

```text
kPathScopeLocal  = 0xF44F784A   kPathScopeGlobal  = 0x57572DAC
kRoadConnectNone = 0xC186DA37   kRoadConnectFirst = 0xF0F61CED
```

原版小学三路网组 = [Global+First（东缘车道）, Local+None, Local+None]；构建器
旧常量 `PATH_ROAD_CONNECT_YES=0xC186DA37` 实为 None（列扫描把高频值误当 YES），
s18 的"接入马路"意图被编码成"不连接" → 建筑无道路连接 → 水电网永不接入 →
`connectedUnits` 全空（面板批量绑定中断）→ 夜灯不亮 → percent-fill prop
（车库）清零消失。"游戏自动纠偏"的观感 = 未连接建筑的正常降级链。

构建器修复（asset_build 97/97 绿）：FIRST/NONE/GLOBAL 三常量按 PDB 修正、
scope/connect 逐路径编码且 connect 列恒写、删除 mixed-roadConnect 错误限制、
金测断言修正。s19（instance `5C8F1719`）路网恢复小学三组结构（entries
7C9C4FFF/03106C2E/418A18B1@40E0C400），Asset 路网列与小学逐值一致，已部署
（s18 归档）。日志：`tmp/build-logs/20261106-s19-roadconnect/`。

## 13. s19 实机读数与城市看板机制（2026-11-06 第二轮）

s19 读数：标题正常；**path 已在游戏内渲染且方向与编辑器一致**（path 列被消费）；
小车车库放置时出现、**时间暂停解除后被模拟移除**（运行期规则失败链触发
`FailUnit(unitSlot, stateIndex)` 的行为特征）；人口地图呈接近废弃的暗色；
详情面板仍为裸占位符。

新机制发现：`cEcoGameUIEventHandler::UnitCreate` 在单元创建时读取 **Lot 的
`0x0D7970ED`**（GetAsKeyInstance），把单元槽位挂进
`mGameUI->mCachedUnitSlotHashMap`（UnitDestroy 对称注销）——人口地图/教育面板/
城市聚合按此看板索引单元。原版小学 = 教育容器 `6A1785B4`；构建器的交互链
恢复（INTERACTION_FIELDS，源自 34CE7E14 公园链）把公园残留值 `D0621548`
覆盖了上来。构建器新增 `lotBoardInstance` schema 旋钮（交互链之后 upsert），
s20（instance `89D7697B`）已修正并部署（s19 归档；asset_build 97/97 绿；
日志 `tmp/build-logs/20261106-s20-board/`）。

边界：看板是 UI 侧索引，可能解释人口地图/面板聚合；水电/铲除的运行期归因
需 delta stream 捕获（配方：`tmp/build-logs/20261106-s20-board/capture-recipe.md`，
`tools/dynamic/rule-logs/capture.py --scope unit --all-targets` + 小学基线
对照 0xC264A268）。产品侧待办（用户指出）：asset 工作台绘制 path 时无法
区分小人/车辆/航线/水路语义——对应本报告 §2 的五类记录层，属于
`docs/design/` 的低代码资产管线需求。
