# 已证明的学生/教育行为机制 + 观光客差距分析（2026-11-09）

本文沉淀两件事：①此前多轮在游戏里**已证明**的学生（教育）行为机制的工程定式；
②2026-11-09 大连希望大厦 /39 包（f46851c8）菜单乱码 A/B 实证与"威利斯能分层吸引
低中高收入观光客、希望大厦不能"的列级/行为级根因。证据全部来自安装包逐字段
转储（prop_dump/pkg_diff）+ 已有 RE 文档，本轮**未改任何引擎代码**。

引用基底：
- `docs/re/elementary-school-full-dissection-20261106.md`（小学全行为扒底，字段→机制对照）
- `docs/re/willis-tower-landmark-mechanism-20261027.md`（威利斯叶/模板逐字段正名）
- `src-tauri/src/asset_build_m1/behavior_nodes.rs` / `behavior_family.rs` / `menu.rs`（机制落地代码）
- 本轮 A/B：`000_OpenSCP_HopeTower_tourism-d5fb84ab-pre-front-door-paths.package`
  （菜单正常）vs 部署版 f46851c8（菜单乱码），pkg_diff 仅 3 处差异（见 §2.1）。

---

## 1. 已证明的学生/教育行为机制（工程定式）

### 1.1 机制总纲（小学扒底 §0 的可执行版）

**"让建筑具有学生行为" ≠ 写属性字段**。属性字段只分三类：身份/容器/系数。
行为本体由四件套承载，缺一即死数据：

```text
该行为的 bin 声明(bundle) + 绑定表行(0x0A6FE749/4A/4B/4C) + 规则引用(0x08080D58+) + 全局资源
```

bundle 槽位四层独立生效（`0x0A0209A2/A3/A4/A5` = Toy 覆盖层/Toy 基类/模板/叶子）：
**没有模板边 = 失去模板层 bundle = 该类行为整体消失**。这是"选对 parent 模板"
比"堆字段"更根本的_structural_原因。

### 1.2 教育闭环的键表（r7–r10 + s31 实证，代码已固化）

| 键 | 语义 | 实证值（小学/大学） |
|---|---|---|
| `0x0AD7FAC9` | student capacity（桌位/座位统计） | 小学按规模 |
| `0x6C042367` / `0x6C042366` | 教育签名 bundle：小学 primary / 社区大学 college | complete-building = s31 大学叶 |
| `0xD07C5A04` / `0xF236BB93` | 学校样本岗位 bundle（main/aux） | SchoolSample labor 自动接线 |
| `0x0A7917C3` | 岗位合计（= ΣW1+W2+W3，写包时按和校验） | 威利斯 360=150+150+60 |
| `0x0A7917C5/C6/C7` | `kPropWork_JobsW1/W2/W3`（三财富档岗位） | 威利斯 150/150/60 |
| `0x0A7917C8` | `kPropWork_MinimumWorkersForProduction` | 威利斯 60 |
| `0x0D00853A..41` | 八班位排班表（int32×8，0=关班） | 小学/大学实证；希望大厦 6/6/7/7/6/6/7/6 |
| `0x6A1785B4` | 教育 lot board | 教育分派 |
| `0x95A9D6FD` | `kCategoryIDEducation` | Unit 级行为类别 |
| `0x0AFE27B1` | Unit→类别绑定列 | → 95A9D6FD |
| `0x7E4B99E5` | `TemplateToyBase_education`（小学 donor 模板） | education_adapter 的 simulation_template |
| `0x00B2CCCB` | Parent 边 | 叶 → 教育模板 |

### 1.3 落地定式（behavior_family.rs，已被游戏验证的组合）

1. **教育族**：donor=elementary-school + `EducationCategory` + labor 有参数时
   `SchoolSample`（jobs bundle + 学历分布 + 招工调参常量 `0x0AD6A725`）+
   generic Menu2 + 教育统计行覆盖。**不剥学校残留**（它就是学校）。
2. **学校残留剥离清单（SCHOOL_RESIDUE_FIELDS，12 项）**：`0AD7FAC9、0AD6A725、
   0C0F2290、0E2660C6、0E3F65A9/B4、0DC1DB39/43/45/5B/C/D` + 4 个 bundle
   （`6C042367/66、D07C5A04、F236BB93`）。**注释明确豁免**：作者岗位 W1–W3/
   合计/排班（`0A7917C3-C8、0D00853A-41`）不在清单内，由 LaborProfile 决定。
3. **LaborProfile**：`SchoolSample`（教育族）/`AuthorOnly`（只写作者
   W1/W2/W3/minWorkers/排班）/`None`。
4. **`0x0DA3EE15` 特例**：观光菜单统计行「Visitors per day」的参数
   （menu-contract 强制），公园/观光族必须写成旅客量（实证默认 51），
   不允许被剥离留占位符（7777392）。
5. **Menu2 统计行 text↔param 必须成对对齐**：
   - 观光默认：text `0EBCE29A` ↔ param `0x0DA3EE15`（显示"每日旅客量 51"）；
   - s31 GOLDEN 三对（教育过渡期）：`0D9AB016↔0AD93A77、0D9AB027↔0C3E9F99、
     0EBCE055↔0E585AE0`。
6. **游戏实证状态**：s14 大学 GOLDEN（2026-11-06 用户确认 working）、小学系列
   watch（2026-10-22/23 规则日志闭环）、d5fb84ab 悬浮卡无学校残留（2026-11-09 观察）。

### 1.4 关键教训（2026-11-09 /39 乱码事故，写进定式）

- **手写 removeFields 不得超出适配器 12 项学校残留清单**，尤其不得包含
  `0A7917C3-C8`（作者岗位）与 `0D00853A-41`（作者排班）——它们是商业/教育
  共用键，剥掉即丢统计行与商业运行参数。
- **删 labor 块 = AuthorOnly 不回写**：strip 后无人补写，列即消失。
- **运行时不做 parent 列回填（本轮 A/B 实证）**：两包 parent 链完全一致
  （叶 → `48E0C000:4E01E311`），仅 instance 列集不同，渲染/行为即不同。
  `sc-properties/inherit.rs` 的展平语义是社区工具侧（C#
  `GetParentProperties`）的，不能假设游戏运行时会替 instance 补缺失键。
  **面板绑定/模拟读的是 instance 级完整列集，模板列是"建包时的参考底稿"，
  不是运行时的兜底。**

---

## 2. /39 菜单乱码根因（A/B 实证链）

### 2.1 资源层差异（pkg_diff，全包 19 条目）

只有 3 处：①Unit `00B1B104:40E0C000:3CAAEFBB`（/39 少 240 字节，丢 21 列）；
②Lot `00B1B104:40E1C000:3CAAEFBB`（/39 变大——正门 path/props，预期内）；
③4 张 `2F4E681B` 材质池纹理换了 instance（57477764-67→2822F07E-81，lot 纹理
引用列同步更新，良性）。**locale 表、Menu2 骨架（2F7D0004）、模型 RW4、菜单
图标全部字节一致。**

### 2.2 Unit 丢失列清单（d5fb84ab 有 → /39 无）

| 键 | 语义 | d5fb84ab | /39 | 绑定的面板行 |
|---|---|---|---|---|
| `0E542F39` | 观光吸引力 | 400 | **500**（schema 调参，预期） | 吸引力行 |
| `0E585AE0/AE1` | 地价对1（值+财富档） | 500/3 | **缺** | `~population_amount:number~` 行的 param |
| `0E585B00/B01` | 地价对2 | 500/2 | **缺** | 地价行 |
| `0EE2A50D/E/F` | `ToyIncomePerResourceW1/W2/W3` 观光收入 | 11/11/11 | **缺** | 收入/花费行 |
| `0A7917C3/C5/C6/C7/C8` | 岗位合计/W1/W2/W3/min | 50/30/20/0/10 | **缺** | 用工行 |
| `0D00853A-41` | 八班位排班 | 6/6/7/7/6/6/7/6 | **缺** | `~hours_begin_time~-~hours_end_time~` 行 |

### 2.3 根因链（层：schema 输入 → 包列 → UI 绑定）

1. mod 目录 `m1.hope-tower.json` 19:14 前后状态：**删了 labor 块** +
   tourism/landvalue 叶参数为空（无 incomeW/地价）+
   **手写 removeFields 25 项**（适配器 12 项 + 岗位/排班 13 项超宽）。
2. d5fb84ab 构建时的 schema/工作台状态（ht-r3 一线）含 labor 块
   （minWorkers 10、排班 6/6/7/7/6/6/7/6、W1 30/W2 20/W3 0）→ AuthorOnly
   回写了岗位/排班列；收入 11×3 与地价对也在其列集内。
3. /39 从 mod 目录现 schema 构建 → 上表"缺"列全部消失。
4. Menu2 generic 骨架的统计行是 text↔param 对；param 指向的 Unit 列缺失时
   **游戏原样打印格式串**（menu.rs 2026-11-08 事故注释原话：
   "缺对应参数时游戏原样打印 `~population_amount:number~`"）。本次同一机制，
   只是连排班/收入列也丢了 → 时间/收入/用工行全部裸奔，Left1–Right3 为
   无绑定骨架行。
5. 修复在**参数层**（用户调 schema/场景 json，不动引擎代码）：
   - 恢复 labor 块（ht-r3 值）；
   - tourism 叶补 `incomeW1/W2/W3`（威利斯叶=11、模板默认 15）；
   - landvalue 叶补两对（500+财富档2、500+档3，威利斯叶同型）；
   - removeFields 收窄到适配器 12 项（或整段删除交给 `strip_school_residue`）。

---

## 3. 观光客机制：威利斯为什么能分层吸引低中高收入，希望大厦为什么不能

### 3.1 全局链条（willis-tower 文档已 RE 的部分）

```text
吸引力属性(0x0E542F39) → 全局 kResourceIDAttractionLevel / RegionalAttractionLevelTotal
  → 游客 Agent 生成/到达规则（财富档 W1/W2/W3，全局资源 TouristW1/W2/W3
     Entered/Satisfied/LeftEarly/PickedUpBy…Yesterday）
  → 进店参观 → 满意/离场统计
  → kPropTaxes_ToyIncomePerResourceW1/W2/W3 折算收入 → Tourism 费用科目
```

吸引力数值本身带**档位语义**：400=中阶（威利斯叶），500=模板默认，800=高阶
（悉尼，同一哈希）。收入分档由游客的财富档（W1 低/W2 中/W3 高）决定，
这正是悬浮卡"低收入/中收入/高收入觀光客"三行与三列收入的全局资源来源。

### 3.2 列级对照（本轮 prop_dump 实测）

| 键 | 语义 | 威利斯模板 4E01E311 | 威利斯叶 8D007745 | d5fb84ab | /39 f46851c8 |
|---|---|---|---|---|---|
| `0E542F39` 吸引力 | 500 | 400 | 400 | **500** |
| `0EE2A50D/E/F` 收入 W1/2/3 | 15/15/15 | 11/11/11 | 11/11/11 | **缺** |
| `0AD6A725` `kPropShopping_TotalShoppers`（**游客/顾客容量**） | **300** | **390** | **缺（被当学校残留剥离）** | **缺** |
| `0DA3EE15` `kPropShopping_AmountOfGoods` | 30 | 465 | 51（当"每日旅客量"显示） | 51 |
| `0E585AE0+AE1 / B00+B01` 地价两对 | — | 500+档2、500+档3 | 500/3、500/2 | **缺** |
| `0A7917C3/C5/C6/C7/C8` 岗位 | 10/10/0/0/0 | 360/150/150/60/60 | 50/30/20/0/10 | **缺** |
| `0D00853A-41` 排班 | 20×8 | 116/117 族 | 6/6/7/7/6/6/7/6 | **缺** |
| `0D7B1448-4B` | 0.07/0.18/0.3/0.75（未命名，疑似档位阈值/分布） | — | 缺 | 缺 |
| `0E965A70-75/BE0-E5` 六组 float+vector2(20,20) | 1+(20,20)×6 | —（叶未覆盖） | 缺 | 缺 |

**要点：`0x0AD6A725` 在地标语境是 `kPropShopping_TotalShoppers`（游客容量），
教育语境才是"招工常量"——同一哈希跨族复用。适配器把它当学校残留剥掉，
等于所有自建地标从未有过游客容量列。** 悬浮卡"觀光客 271/**300**"的分母
与模板值 300 同源（显示绑定的精确式待 UI 层核实，诚实标注）。

### 3.3 行为级差距（bundle → UnitRule，本轮实测）

| | 威利斯叶 | 希望大厦 /39 |
|---|---|---|
| A5 叶层 bundle（`0x0A0209A5`） | **15 个** | **3 个**（265FB8DD、C0672F57、树包 E7690450） |
| 由 bundle 索引的 UnitRule | 13 条（AEB 全命中，语义未反编译） | **0 条观光/商业规则** |
| 关键资源 bin | CF116B9C（Wealth/Density cap3，**顾客财富档容器**）、2B5A2BAF（ProfitToken cap10000×5）、362087BD（cultural 大 bin）、908B3CEC（劫匪） | 无 |

**结论（按层命名置信度）**：
- 列面（已证明）：威利斯叶是一份**自足的完整列集**——容量、货物、三档收入、
  三档岗位、地价、排班齐备；希望大厦从未有过 TotalShoppers/AmountOfGoods
  的地标量级，/39 又丢了收入/地价/排班。
- 行为面（已证明存在、语义未反编译）：威利斯 15 bundle→13 规则承接
  "游客进店→消费→计收入"；希望大厦 A5 仅 3 个公园系 bundle，无财富档容器、
  无利润代币 bin——**即使游客被城市吸引度生成，建筑侧没有任何规则/容器
  承接进店**。这与"有游客统计/路过，不进门"的观察吻合，也与用户
  "UnitRule 尚未逐条反编译"的判断一致。
- 修复实验顺序建议（先参数后规则）：①§2.3 参数修复（恢复列集，救菜单+收入）；
  ②`0AD6A725` 从学校残留清单豁免并按地标量级 author（300–390）、
  `0DA3EE15` 调到地标量级（威利斯 465）；③观察 TouristW1/W2/W3 Entered
  是否仍为 0——若仍为 0，则进入 bundle 层实验（给 A5 补地标/商业 bundle，
  这一步涉及引擎代码与规则语义反编译，属下一阶段）。

---

## 4. 附注

- **车流**：/39 引道（`7C9C4FFF` Global/First）已生效，车辆进场的观察与此一致。
- **灯数量**：场景源 11 灯（2 Spot/7 Line/2 Point）；包内 `0CAA8F12`=11 条
  （数量已对齐）、`02A907B6`=4 条、Unit `02A907B6`=2 条；/39 与 d5fb84ab
  数量相同仅位置镜像（180° 场景翻转的一致结果）→ **灯数量问题不是 /39 回归**，
  哪一列驱动游戏渲染待 `light_columns_dump` 取证。
- **台账**：本轮对比基准 d5fb84ab 已归档为
  `000_OpenSCP_HopeTower_tourism-d5fb84ab-pre-front-door-paths.package`；
  test-archive 裸名文件曾发现未登记覆盖（实际 6d2cf9da ≠ 台账 954cbf6f），
  已按实际哈希隔离（见 MANIFEST 2026-11-09 行）。
