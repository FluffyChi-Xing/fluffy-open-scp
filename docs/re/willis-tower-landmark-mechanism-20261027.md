# 威利斯大厦纵向研究：地标分类、静态 property 的动态行为、两维度父类普查（2026-10-27）

## 0. 结论速览

以文化/地标菜单下的威利斯大厦（菜单条目 `74168A83`，UI 工具 ID `Plop_T_LandMarks_WillisSearsTower_Base_01`）为样本，本轮把三层问题全部闭合到安装包证据：

1. **地标分类不是"标签"，而是一条归属边 + 一条继承边**。归属边：Menu2 的 `0x0DB9FC63 uiCategories = [3F43E038]` 把它挂进文化菜单 `94E412ED` 的"地标"子栏；继承边：menuUnit 指向的 EcoGame Unit `8D007745` 通过 5 层 `0x00B2CCCB parent` 继承 `TemplateToyBase_landmark` 模板，从而获得解锁、数量限额、费用科目等一类基本行为。
2. **水电消费是叶子上的静态属性，配额在模板**。威利斯叶子 Unit 上 `kPropPowerConsumer_Rate/Amount/Capacity = 72/4464/13392`、`kPropWaterConsumer_Amount/Capacity = 2976/8928`、`kPropGarbageProducer_GarbageOnlyAmount = 150`；这些值在运行时由 EcoGame 单元特化读入资源 bin，再由 ER2 规则产生实际消耗（与 2026-10-22 小学研究的机制闭环一致）。
3. **观光客吸引同样先有静态属性，后有规则**。叶子属性 `0x0E542F39 = 400`（中阶观光客吸引力，模板默认 500，悉尼同哈希 = 800 高阶）、`0x0E585AE0 = 500`（中价土地价值），菜单统计行 `unitEffectParameter = [240398137, 240671456]` 恰好就是这两个属性哈希——UI 数值与 Unit 属性一一对应；行为面由 15 个 bundle 引用的 13 条 UnitRule（已在 AEB 全部命中）+ 全局 `AttractionLevel` 资源体系承接。
4. **两维度普查确认了既有假设**：城市面板 14 个一级菜单、城市发展面板 9 个一级菜单；菜单级几乎所有 plop 工具共享 `AF042E9A`（plop 工具根），而 **Unit 级每个服务类别各有一个专属模板父类**（电力 `6D766518`、教育 `7E4B99E5`、地标 `4E01E311`……），模板用 `0x099B4C1F`/`0x0AFE27B1` 绑定类别 ID、用 `0x0AD65F07/08` 指定费用科目。mod 开发要"给建筑一类基本行为"，正确做法是继承对应服务模板，而不是复制叶子属性。

证据文件：`docs/re/evidence/willis-tower-landmark-20261027.json`；普查中间产物在 `tmp/willis-survey/`（menu-survey.json、locale-zhtw.json、census.txt）。

## 1. 样本定位

| 资产 | TGI | 位置 |
|---|---|---|
| Menu2 菜单项 | `00B1B104:09878A01:74168A83`（292B） | `SimCity_Graphics.package` |
| EcoGame Unit（模拟主体） | `00B1B104:40E0C000:8D007745`（840B） | `SimCity-Scripts_287520926.package` |
| Lot/资产配置 | `00B1B104:40E1C000:8D007745`（2677B） | `SimCity_Graphics.package` |
| 图标 PNG / rollover JPG / RW4 模型 | `2F7D0004:40E02400:8D007745` / `3F8662EA:0:74168A83` / `2F4E681B:0:8D007745` | Graphics / Game |

威利斯 Menu2 不在 `SimCity_Game.package`（那里是另外 457 条 `09878A01` 城市工具），地标这批 19 条 `09878A01` 工具集中在 Graphics 包。用 `find_instance` 全包扫描是本次定位的关键一步。

## 2. 问题一：威利斯如何被划归"地标"

### 2.1 归属边（uiCategories，决定"出现在哪个菜单"）

```text
HUD「文化」按钮
  └─ 容器 00B1B104:0C298BDD:94E412ED「文化」           ← 组 0C298BDD = kCitySpecializationGroupID（城市发展维度）
       ├─ 0x0C2585B1 toolCategoryGroupID = 0F978DAC     ← 指示：到该组里枚举子分类
       ├─ 0x0C2BF463 toolCategoryName = locale 768A8453:0E0E9C2C「文化」
       └─ 子分类 00B1B104:0F978DAC:3F43E038「地标」      ← 名称/顺序为 0x0C2BF463/0xCDCEF1AF
            └─ 成员 = uiCategories 含 3F43E038 的全部工具（静态 23 个）
                 └─ Menu2 00B1B104:09878A01:74168A83「威利斯大廈」
                      ├─ 0x0DB9FC63 uiCategories = [3F43E038]     ← 归属边，唯一分类依据
                      ├─ 0x0DC1E3E0 uiPosition = 11500            ← 子栏内排序
                      ├─ 0x00B2CCCB parent = AF042E9A（plop 工具根）← 功能家族边，与归属无关
                      └─ 0x0975695F menuUnit = 8D007745           ← 行为入口
```

要点：

- 分类只由 `0x0DB9FC63` 决定；`parent`（`AF042E9A`）与教育、电力等一切 plop 工具同根，**不参与菜单归属**。这与 2026-10-22 菜单机制研究的"归属数组与功能继承是两条独立关系"结论一致。
- 文化菜单的四个 tab（地标/体育场馆/景点/宗教）= `94E412ED` 的 `via` 容器 `3F43E038 / 546FD9CE / 0653C631 / E286FFA5`；地标子栏 23 个成员全部 `parent=AF042E9A`。
- 地标子栏 23 条菜单项共享同一组解锁条件（见 2.3），说明"这一类建筑"的共性 indeed 存放在共享位置。

### 2.2 继承边（Unit parent 链，决定"是哪类建筑"）

威利斯 Unit `8D007745` 的 `0x00B2CCCB` 向上五层，到空根终止（全部在 `SimCity-Scripts_287520926.package` 组 `48E0C000`）：

| 层 | instance | 角色 | 本层关键贡献 |
|---|---|---|---|
| 根 | `593B61D2` | 空属性（4B） | 继承链哨兵 |
| 1 | `9EC474FF` | Unit 基类 | `0x0AFE27B1 → C710B6E9（kCategoryIDBuilding）`、工人/尺寸默认、标志位 |
| 2 | `F98BCB35` | Toy 基类 | 水/电消费默认（48/16/72 等）、`http://en.wikipedia.org/wiki/Toy` 维基链接字段、默认 bundle 组 `0x0A0209A3`×24 |
| 3 | `2A25CF66` | Toy 覆盖层 | 水/电默认再覆盖、bundle 组 `0x0A0209A4` 候选、`0x0AFE27B1 → 406CCC43` |
| 4 | `4E01E311` | **`TemplateToyBase_landmark`（地标模板）** | 解锁 bin、数量限额、吸引力默认 500、观光收入 15、**费用科目 = Tourism** |
| 叶 | `8D007745` | 威利斯叶子 | 造价 140000、维护 605、工作岗位 360、水/电/垃圾、吸引力 400、15 个 bundle |

地标模板 `4E01E311` 给全体地标注入的"一类基本行为"：

| 模板字段 | 值 | 语义（符号已核实） |
|---|---|---|
| `0x0BF5D2E3 / 0x0BF5D2E4` | `4B427C97` / 1 | `kPropUnlock_Bin` + 数量：放进城市即解锁 `kResourceIDLandmarksUnlocked` |
| `0x0E51C90F` | `4B427C97` | 共享数量上限来源（该 bin 容量 3 = 全城地标总额度，见 building-count-limits-20261020） |
| `0x0CC8BE51` | Float 1 | 单项上限（每种地标 1 座），`0x0DF1467C → 7D11699A` 选中下标 0 |
| `0x0E542F39` | 500 | 观光客吸引力默认值（叶子按各建筑覆盖） |
| `0x0EE2A50D/E/F` | 15/15/15 | `kPropTaxes_ToyIncomePerResourceW1/W2/W3`（观光收入系数默认） |
| `0x0AD65F07 / 0x0AD65F08` | `999779EE` / `6FDFB90D` | 费用科目：`ExpensesFromMaintenanceTourism` / `ExpensesFromTourism` |
| `0x099B4C1F` | `8A60B969` | 模板级类别/资源绑定（符号未命名） |
| `0x0DBA0761` | `C1F569B9`（配 float 0.2） | `kResourceIDTotalShoppers` 关联 |
| `0x0DBA076E` | `31912F17`（配 float 120） | `kResourceIDCriminalMugger`——地标会吸引劫匪 |
| `0x0A0209A4` | 12 个 bundle | 模板级建筑定义候选 |

### 2.3 解锁条件（菜单级，23 个地标共用）

Menu2 `0x0DE84DC9/DD3/DDC` = 三个 Key + 数量 `[1,1,1]` + 三条文案：

1. `C93FD4B5` = `kResourceIDHQ_CityHall_IsOn`（市政厅启用）
2. `895DD8CE` = `kResourceIDHQ_CityHall_Level2_IsOn`（市政厅 2 级）
3. `BF800BE4` = `kResourceIDHQ_CityHall_Tourism_Module_IsOn`（**旅游部门模块**）

即"划归地标"在 UI 层的完整表达是：归属边进文化/地标子栏 + 模板继承进地标行为家族 + 市政厅旅游模块解锁。

## 3. 问题二：静态 property 如何具有动态行为

### 3.1 叶子 Unit 上的经济与资源属性（已命名部分）

来自 `40E0C000:8D007745`，符号名来自 `SimCity_Game.package 0x40464200` GameUI 脚本的 `simcity.k*` 常量表：

| 属性 | 值 | 符号名 |
|---|---:|---|
| `0x0A8A41DA` | 140000 | `kPropSCUnitConstructionCost`（建造价） |
| `0x09AE19D7` | 605 | `kPropSCUnitMaintenanceCost`（维护/小时） |
| `0x0AFB9882` | 72 | `kPropPowerConsumer_Rate` |
| `0x0B88904E` | 4464 | `kPropPowerConsumer_Amount` |
| `0x0C09DAA0` | 13392 | `kPropPowerConsumer_Capacity` |
| `0x0C09DA65` | 2976 | `kPropWaterConsumer_Amount` |
| `0x0C09DA5A` | 8928 | `kPropWaterConsumer_Capacity` |
| `0x0C216562` | 150 | `kPropGarbageProducer_GarbageOnlyAmount` |
| `0x0A7917C3` | 360 | `kPropWork_TotalJobs`（工作岗位） |
| `0x0A7917C5/C6/C7` | 150/150/60 | `kPropWork_JobsW1/W2/W3` |
| `0x0A7917C8` | 60 | `kPropWork_MinimumWorkersForProduction` |
| `0x0A7917C4` | 256 | `kPropTaxesBuildingTaxValue` |
| `0x0A7917CB` | 670 | `kPropRCI_TotalPopulation` |
| `0x0AB985AA` | 310 | `kPropProfit_DefaultProfitTokens` |
| `0x0AD6A725` | 390 | `kPropShopping_TotalShoppers` |
| `0x0DA3EE15` | 465 | `kPropShopping_AmountOfGoods` |
| `0x0E5467E7` | 49600 | `kPropProfit_ProfitTokensThresholdForDensityUpgrade` |
| `0x0EE2A50D/E/F` | 11/11/11 | `kPropTaxes_ToyIncomePerResourceW1/W2/W3`（覆盖模板的 15） |
| `0x0E542F39` | 400 | 地标吸引力（中阶观光客；模板默认 500，悉尼 = 800 高阶，同一哈希） |
| `0x0E585AE0 + 0x0E585AE1` | 500 + 2 | 增加中价土地价值（int32 2 = 财富档） |
| `0x0E585B00 + 0x0E585B01` | 500 + 3 | 增加土地价值第二档 |
| `0x0E3CC693 / 0x0E3CD3BD` | Key | → `kResourceIDProfitCTokenW3` / `_Capacity`（W3 商业利润代币 bin） |
| `0x0EACDB63` | Key | → `kResourceIDProfitToken` |
| `0x0EC5D62E` | Key → `E32012F0` | 单项计数资源候选（帝国大厦同字段 → `4A73D81E`，见既有研究） |

数量关系：电/水的 `Capacity = 3 × Amount`（13392=3×4464；8928=3×2976），且每个资源族都伴随一个与 Capacity 同值的伴生哈希（水 `0x0C09DA6E`、电 `0x0C09DAB2`）和一个恒 0 的伴生哈希（`0x0C09DA61`、`0x0C09DAA6`）；这四个伴生哈希在 UI 符号表中无名字，功能未定，**不能凭数值命名为已确认语义**。

顺带修正小学研究的一个待定项：小学 Unit 的 `0x0AFB9882 = 72` 当时标注"教育相关数值待确认"，本系列表证实它就是 `kPropPowerConsumer_Rate`——小学和威利斯功耗系数同为 72。

### 3.2 从静态属性到运行时行为的机制链

静态属性本身不执行任何消耗；运行时闭环（与小学研究的已验证链路一致）：

```text
放置时 EcoGame 单元特化：读 Unit(含模板合并) 属性 → 建资源 bin（Power/Water/…容量取 Capacity 族）
   → AddBundle 读每个 bundle 的 0x08080D58(+0..2) 规则属性与 0x08327D48+ 事件槽 → 索引 AEB 中的 UnitRule
   → 规则按时间表执行：AgentTest/条件通过 → ApplyBinRule 修改资源 bin（消耗 Power、产 Garbage…）
   → 全局资源（AttractionLevel / LandmarksUnlocked / TouristW2…）驱动跨建筑与跨城行为
```

威利斯的 15 个 bundle（`0x0A0209A5` → 组 `40E0C700`）解析结果：

- 13 条 UnitRule ID（`928917B5 5C936A9F 04E36970 8081DC5C B019AC5E 9CD4DEAD 4C696671 1AD7E6C2 60A49767 25685C7F 08E5D9E3 00FE2614 7D511561`），**13/13 在离线 AEB（5/2/14，28441 条 UnitRule）中以 scope=unit 命中**；
- 多个 bundle 携带事件规则槽（`08327D49/4C/4D/51/52`），其中 `8BA6D35D` 有 8 条 `08327D49` + 10 条 `08327D4D` + 2 条 `08327D52`，是事件规则最密集的一个；
- bundle 资源 bin 直接可见行为素材：`CF116B9C`（`kResourceIDWealth` cap3 + `kResourceIDDensity` cap3）、`362087BD`（cap 1000000 的大 bin + `kCategoryIDCultural`）、`908B3CEC`（`kResourceIDCriminalMugger` cap1——对应模板的吸引劫匪字段）、`2B5A2BAF`（`kResourceIDProfitToken` cap10000 ×5）；
- 观光体系的全局资源（UI 符号表已命名）：`kResourceIDAttractionLevel`（城市吸引度）、`kResourceIDRegionalAttractionLevelTotal`（区域吸引度）、`TouristW1/W2/W3 Entered/Satisfied/LeftEarly/PickedUpBy…Yesterday`、`HasLandmark*`、`LandmarksUnlocked`。地标的"吸引观光客"是：吸引力属性 → 全局吸引度 → 游客 Agent 生成/到达规则 → 参观/满意/离场统计 → `ToyIncomePerResource` 折算收入并记入 Tourism 费用科目体系。

边界：UnitRule 的 AgentTests/TransportOps/资源操作语义尚未反编译（与小学研究的结论一致），本轮不能给 13 条规则逐条命名"哪条是水电消耗、哪条是游客生成"；只能证明它们存在于当前安装包的规则库并被威利斯 bundle 索引。

### 3.3 与"没有 effect/动画"观察的对照

威利斯 Lot（`40E1C000:8D007745`）包含：bbox（-32,-44,0)-(32,44,408)、3 个 RW4 模型引用（主体+2 个 LOD/部件）、10 个Path 点位变换、28 条 agent/路径键（`0x0CAA8F10/11` → `BC76E6BA/0820ABAF/75D4C8CD`）、**16 个命名 UnitLocation**（`Front_entrance / Rside_entrance / Bside_entrance / Rside_light2 / mid / top_tip`，其中 `top_tip` ×6 分布在 z=354–412 的塔尖）、窗口光/vignette 参数（5 个键中含 `kVignetteID`）。**没有 effect 实例键**——塔尖锚点是给警示灯/夜景光预留的语义槽位，是否发光取决于 effect 定义与运行时状态，这与"静态感"的观察一致：地标的表现层是纯模型+锚点，动态全在模拟层。

## 4. 问题三：城市 / 城市发展两维度父类普查

### 4.1 两个维度的定义

- **城市面板**：一级 14 个分类（复刻数据 CATEGORY_MODEL，instance 即容器 ID）：道路 `AC12C904`、区域(RCI) `3193C0BE`、电力 `8E8B4E10`、水 `9E3C3DFA`、污水 `CD1DDC35`、政府 `674129FD`、废弃物 `7CB23A4E`、火灾 `9FAC79C9`、健康 `DB2D51A3`、警察 `1159ECD7`、教育 `95A9D6FD`、公共运输 `CBC27856`、公园 `899BEA70`、灾难 `9B7C811F`。顶层菜单容器在 `09A4BA03` 组（工作台 `gameMenuGroupsCompile` 的 parentGroup 常量）。
- **城市发展面板**：`kCitySpecializationGroupID = 0C298BDD` 组的 9 个容器：采矿 `AFA8707D`、钻井 `3A798ED7`、贸易 `74CD2EB0`、电子 `BEE1C73E`、赌博 `0BFA0784`、文化 `94E412ED`（4 tab）、巨塔 `895E329B`（2 tab）、学院 `1D23D2FD`、欧米茄 `692D1886`。

### 4.2 菜单级父类（Menu2 `0x00B2CCCB`）普查

对两维度全部可静态解析成员（基础 + EP1 + DLC0 包）统计（完整表见 `tmp/willis-survey/census.txt`）：

| 维度/菜单 | 成员数 | 菜单父类分布 |
|---|---:|---|
| 城市·道路 | 7 | 道路家族根 `12E3214B`×4、`976868C6`×3 |
| 城市·区域(RCI) | 4 | `6E336892`×4 |
| 城市·电力 | 8 | plop 根 `AF042E9A`×8 |
| 城市·水 | 3 | `25F260C3`×3 |
| 城市·污水 | 3 | plop 根×3 |
| 城市·政府 | 3 | plop 根×3 |
| 城市·废弃物 | 3 | plop 根×2、`25F260C3`×1 |
| 城市·火灾/健康/警察/教育 | 3/4/5/6 | 全部 plop 根 |
| 城市·公共运输 | 22 | plop 根×17、道路根×3、其他×2 |
| 城市·公园 | 96 | 六个公园子根 `F970A822`×28、`F4694C0F`×16、`4EEB801B`×15、`B674E73B`×13、`F1035727`×10、`C725C506`×9、plop 根×3 |
| 城市·灾难 | 7 | `BE5744E0`×7、`051EBCAA`×6 |
| 发展·采矿/钻井/贸易/电子/赌博/文化/巨塔/学院/欧米茄 | 5/3/3/3/6/37/7/2/2 | **全部 plop 根 `AF042E9A`**（文化 37/37） |

结论：**菜单级父类几乎只有两类语义**——`AF042E9A`（plop 放置工具家族根，全维度共享）和 `12E3214B`（道路/路径家族根）；公园、灾难、水等有少量中间子根，它们是"价格/子栏聚合"层面的父，不是行为家族。

### 4.3 Unit 级模板父类（EcoGame Unit `0x00B2CCCB`）普查——真正的基本行为层

menuUnit → `40E0C000` Unit → 其 parent（模板）：

| 菜单 | Unit 模板父类（×成员数） | 模板绑定的类别/费用（已核实的样本） |
|---|---|---|
| 电力 | `6D766518`×5 | `0x099B4C1F → 8E8B4E10 kCategoryIDPower`；费用 → `ExpensesFromMaintenancePower`/`ExpensesFromPower` |
| 水 | `8CE11AD2`×2 | |
| 污水 | `EB6B535D`×2 | |
| 政府 | `CC145FE0`×3 | |
| 废弃物 | `871590E6`×2 | |
| 火灾 | `BD7BF4A1`×2 | |
| 健康 | `C0C7911B`×2 | |
| 警察 | `35EE068F`×2 | |
| 教育 | `7E4B99E5`×5（另 `DEFA792F`×1） | `0x099B4C1F/0x08A16042/0x0AFE27B1 → 95A9D6FD kCategoryIDEducation`；费用 → `ExpensesFromMaintenanceEducation`/`ExpensesFromEducation` |
| 公共运输 | `7E8A693D`×4、`254C6B2E`×3、`14B3CC3F`×2 … | 按运输子类分模板 |
| 公园 | `4A4E6F30`×8、`B51EBFA8`×6、`7F67EA81`×6 … | 按公园子类分模板 |
| 发展·采矿/钻井/贸易/电子 | `EA6A4D34`、`5E9285DF`、`3E3EF188`、`5B7CC559` 等 | 各专业化模板 |
| 发展·文化(地标) | **`4E01E311`×19**、`FB87DCB8`×2 | `TemplateToyBase_landmark`；费用 → `ExpensesFromMaintenanceTourism`/`ExpensesFromTourism` |
| 发展·赌博 | `BD4E8A7C`、`AFEFBF8F`、`2ABFC148`、`21B156C1`、`3CE0F852` 各×1 | |

全体 Unit 模板家族分布（组 `40E0C000`+`48E0C000`）：`9AC1FAC2`×343（极简模板：仅 Lot 引用+标志+1 bundle，无 parent，装饰/简单物家族）、`2D7BDB01`×108、`C34674D9`×38、`7A8606D1`×34 等。

**核心结论：用户的假设成立且可以精确化**——"菜单下的建筑继承一个父类来拥有一类基本行为"发生在 **Unit 模板层**，不是菜单层：

1. 菜单层所有人共享 plop 根（管"怎么放下去、显示什么"）；
2. Unit 层每个服务类别一个模板（管"是什么"）：类别 ID 绑定（`0x099B4C1F`/`0x08A16042`/`0x0AFE27B1`）、费用科目（`0x0AD65F07/08`）、解锁/限额（`0x0BF5D2E3/E4`、`0x0E51C90F`、`0x0CC8BE51`）、类别默认数值（如地标吸引力 500）、模板级 bundle；
3. 模板之上还有统一基类链（`2A25CF66`/`F98BCB35` Toy → `9EC474FF` Unit 基类 → 空根），水/电/维护等最通用默认放在 Toy 层；
4. 叶子只做**特化覆盖**：造价、维护、容量、吸引力、bundle 增删（`0x0A0209A5` 启用 / `0x0A0209B2` 排除）。

### 4.4 已知缺口

- 悉尼/奥斯陆等带演出地标的 `0x0F331B8F`（最大利润）/`0x0DDDAD15`（最高出席）未在 UI 符号表中找到名字；
- `0x0D00853A..41`（8×int32，模板 20、威利斯 116/117）语义未定；
- `0x0AFB9886`（Toy=40/60，小学 900，威利斯 29760）与水/电伴生哈希未命名；
- 13 条 UnitRule 未反编译到资源操作级；
- `8A60B969`（地标模板 `0x099B4C1F` 绑定）与 `406CCC43`（Toy 层绑定）未命名；
- 普查只覆盖 Graphics/Game/EP1/DLC0 + 两个 Scripts 包；其余 DLC 脚本包（巨塔、Launch Arcology 等）未入普查，巨塔/学院/欧米茄的 Unit 父类显示"无"正是因为其 Unit 在未扫描包或 C900 组。

## 5. 对 mod 开发流程的改进建议

1. **新建筑先选模板，再写叶子**。做一个新地标 = Menu2（uiCategories 指向子栏 + menuUnit）+ Unit（`0x00B2CCCB → 4E01E311`）+ 覆盖造价/维护/吸引力/地价 + 复用模板 bundle；解锁与"全城 3 座"限额自动从模板继承，不需要重写。反之，若想让某建筑获得某类行为（服务覆盖、观光、购物），换/加模板父类比堆属性更符合引擎读取路径。
2. **属性写作按"模板默认 → 叶子覆盖"审计**。工作台资产编辑已有 parent 保持逻辑；建议在编辑 Sheet 中展示"该值来自模板还是叶子"（本样本中 `0x0EE2A50D` 模板 15 → 叶子 11、`0x0E542F39` 模板 500 → 叶子 400 是典型覆盖），避免误改共享模板殃及全部同类建筑（building-count-limits 研究已有同样的"写叶子不写模板"守则）。
3. **普查产物可直接复用**：`tmp/willis-survey/menu-survey.json`（24654 条，含 menuUnit 字段——本次给 `ui_menu_survey` 探针新增了该字段导出）+ `locale-zhtw.json` 可以作为 mod 兼容性检查（"我的建筑 parent 是否指向正确模板"）与批量为新建筑选模板的查找表。
4. **水/电/垃圾等消费数值的单位对齐**：`Amount` 与 `Capacity` 是 1:3 关系、`Rate` 另成一族；做数值 mod 时应整族修改（Rate/Amount/Capacity + 两个伴生哈希），只改一个字段可能造成 UI 显示与实际 bin 容量脱节。
5. **下一步验证路径**：用 `rule-logs/capture.py --scope unit --rule-id 0x…` 对 13 条威利斯规则做定向采集，把"哪条规则消耗电/水、哪条写 `HasLandmark`/吸引度"落实到运行时证据；这需要城市里实际放置威利斯大厦。

## 6. 证据与工具

- 威利斯四件套 + 五层模板 raw dump：见会话执行记录与 `docs/re/evidence/willis-tower-landmark-20261027.json`（含全部哈希→符号映射）。
- 探针：`tool_info`、`prop_dump`、`find_instance`、`find_group`、`ui_menu_survey`（新增 menuUnit 导出）、`locale_tables_dump`（新增，导出 locale 包全部 0x0A98EAF0 表）、`resolve_ids.py`（AEB 规则查询）。
- 参照文档：`menu-mechanism-research-20261022.md`（菜单六样本与判定原则）、`building-count-limits-20261020.md`（地标容量/单项上限的读取端证据）、`elementary-school-mechanism-20261022.md`（规则日志闭环与资源符号方法）、`menu-entry-schema.json`（Menu2 字段字典）。
