# Graphics RCI 闭环：住宅入住 / 商业吸客耗货 / 工厂生产（兼酒店化与劳工·观光）

日期：2026-11-10。  
锚点 Graphics Lot：`00B1B104:42E1C000:A493781E` = `C_gen_w3d1_boutique_04_01_N`。  
对照样本：住宅 `41E1C000:5EB99AC6`（`R_Gen_W2D2_Townhouse_01_01_N`）、工业 `43E1C000:60DE5E12`（`I_Gen_W1D2_Factory_01_01_N`）。  
证据层：Graphics 呈现 Lot、Game Lot 模板、Scripts zone 模板/`40E0C700` bundle、AEB 规则表（`tmp/behavior-exemplars/rci/`）、`database_main.s3db` 符号表。  
**声明：行为等价 ≠ 数据等价；下列"闭环"指属性→bin/sink→规则→全局资源的静态链，不声称本轮有新的局内运行时实测。**

---

## 0. 一页结论

1. **Graphics 的 R/C/I 记录不是 Unit**。`41/42/43E1C000` 共约 834 条全是 Lot 外观（模型/path/车辆 bin），无 `0x0A0209A5` bundle、无 Jobs/Shoppers。模拟身份在 Scripts 的 **zone 模板（`48E0C000`）**，经 ZoneID 生长，不经 Menu2。
2. **三类经济角色分字段族**：  
   - R：人口 `0x0A7917CB` + 幸福 token 升级；供给城市劳工。  
   - C：岗位 `JobsW*` + `TotalShoppers`/`AmountOfGoods` + 利润 token 升级；从 nearby 拉工人、吃货、吸客。  
   - I：岗位 + 最低开工人数 + 科技门槛 + 污染；生产/发货规则喂城市货物库存。
3. **劳工与观光客共享"容量槽 + agent + sink"形态**，但入口不同：劳工走 `ExtraWorkerW*/CarW*` 与 C/I 班次；观光客走吸引力/`TouristW*`（地标）或商业 `TotalShoppers`（顾客）。**同一哈希 `0x0AD6A725` 在商业=顾客容量、在地标=游客容量。**
4. **酒店化不是另一套 Graphics Lot**。库内无 `C_*Hotel` 模型名；存在 `UnitNamesCW{W}{D}Hotel`（UI 名）与 `DL_Tourists_HotelGuest`（`0x2F7D0004` 绘制列表）。经典/地标附近的商业可改挂酒店客源语义，**仍是 C 区划外壳，但能吸纳观光客**——这是劳工+观光与 RCI 商业交汇点。

---

## 1. 锚点解剖：`A493781E` boutique（C · W3D1）

| 项 | 值 |
|---|---|
| TGI | `00B1B104:42E1C000:A493781E` |
| 名称 | `C_gen_w3d1_boutique_04_01_N`（同族 `_A`/`_R` = `A4937811`/`A4937802`） |
| parent | `40E1C000:7F496E35` → … → `TemplateCommercial_N`=`4BCCE372` → `A5C7BE89` |
| path | 点列 + 段 + PathEntry×4（`03106C2E`/`418A18B1`/`C34881E7`/`8F7DDFE2`） |
| 车辆 bin 槽 | `0x0C12EF20` → `0AB4DFE1/2/3`（与 C 工人槽 bin 同号族，Lot 侧只挂绘制位） |
| 经济字段 | **Lot 上无** TotalShoppers/Jobs；数值在 zone 模板 |

同族 boutique 全部挂 `parent=7F496E35`（`_N`），属 **CW3 × D1** 外观池。Zone 生长侧 CW3 密度档 ZoneID 为 `FF5F4B70/72/73`（zone 规则采样 `kResourceIDTaxLevelCW3` + desirability map）。

**结构分工（必读）**：

```text
区划需求 / zone 规则(AEB 段 C)
  → SelectUnitEntryByZoneID（Scripts 叶子 Unit，ZoneID 继承自 48E0C000 模板）
  → zoning-lot 变体表 0x0CA5CF4A
  → Graphics 42E1C000:_N 外观（本锚点）
行为 = 模板经济字段 + A4 bundle（如 BC7613A3）+ AEB unit 规则
```

把 Graphics Lot 当 Unit 写入 `40E0C000` 扫描组 **不会**得到商业行为。

---

## 2. 商业（C）：雇工 · 进货 · 吸客 · 升级

### 2.1 叶字段（zone 模板 `97E741B3` = C_Gen W1D1 Shops 实测；W3 boutique 同族放大）

| 属性 | W1D1 实测 | 符号 |
|---:|---:|---|
| `0x0A7917C3` | 7 | `kPropWork_TotalJobs` |
| `0x0A7917C5/C6/C7` | 5 / 2 / 0 | `JobsW1/W2/W3` |
| `0x0A7917C8` | 3 | `MinimumWorkersForProduction` |
| `0x0A7917CB` | 14 | `kPropRCI_TotalPopulation`（商业侧"活动人口/规模"口径） |
| `0x0AD6A725` | 10 | **`kPropShopping_TotalShoppers`（顾客容量）** |
| `0x0DA3EE15` | 21 | **`kPropShopping_AmountOfGoods`（货量）** |
| `0x0AB985AA` | 8 | `kPropProfit_DefaultProfitTokens` |
| `0x0E5467E7` | 112 | 利润 token → 密度升级门槛 |
| `0x0E542F39` | **0** | 吸引力（普通商业为 0；地标另写） |

A4 bundle 含 `BC7613A3`（工人/班次/顾客 sink 大户）、`CB6376A2`（zone 侧）、`7424049F`（通路）等。

### 2.2 雇工闭环（与 plop 劳工同构）

```text
JobsW* / MinimumWorkers
  → bundle BC7613A3 声明工人槽 bin（0AB4DFE1/2/3 …）
  → sink 规则：
       ExtraWorkerW*Paused → ExtraWorkerW* 到岗；nearby 库存 −1
       CarW1/2/3 通勤版派发
       离开链再入
  → 事件槽交班 / 18:00 收班（ShiftLength、DayJobs 清点）
```

要点：工人来自城市 **Worker/ExtraWorker 库存**（住宅侧供给，见 §3）；商业是 **sink 拉取**，不是自产劳工。缺通路则 `kResourceIDAccessToOutside` 链断 → 不生长/不开工（规则 `7F63B2F*`）。

### 2.3 货物与顾客闭环

```text
AmountOfGoods → 货物 bin（规则侧 9EB4CB2B / 23B94F3B，fromBin 消耗）
TotalShoppers → 顾客容量
事件槽 9：nearby 顾客资源（47616D3C/3F/3E）到达扣减
08:00：ConstructionProgress / 补货推进（与 I 共用构造族 427E0344）
利润 token 累积 → B12434A3：Density 升级（local Density −2 + global 标志）
```

工厂（I）产出经城市货物网络 → 商业 `AmountOfGoods` 消耗；**数据层**已闭合到"fromBin 消耗货物 bin"，**agent 运货动画**属 Lot `0x0C12EF*` 绘制位，不等价于规则语义已逐帧证明。

### 2.4 与希望大厦/地标观光的对照

| | 普通商业 boutique | 地标（威利斯） |
|---|---|---|
| `0x0AD6A725` | 顾客容量（小，W1≈10） | **游客容量**（300–390） |
| `0x0DA3EE15` | 货量 | 常被 UI 复用为"每日旅客量"参数 |
| `0x0E542F39` | 0 | 400–800 吸引力 |
| `0x0EE2A50D/E/F` | 无（靠商业税/利润） | 观光收入系数 |
| 规则入口 | C zone bundles + 顾客 sink | 地标 A5 bundles + TouristW* |

**同一 `TotalShoppers` 哈希跨族复用**：学校适配器曾误剥 → 自建地标丢游客容量（见 `proven-student-behavior-and-tourism-gap-20261109.md`）。

---

## 3. 住宅（R）：住人 · 幸福 · 劳工供给

### 3.1 叶字段（zone 模板 `0AC9698D` 族 / W3D3 highRise 样本）

| 属性 | 量级 | 符号 |
|---|---:|---|
| `0x0A7917CB` | 100 | `kPropRCI_TotalPopulation`（住户容量） |
| `0x0E546790` | 100 | 默认幸福 token |
| `0x0E546792` | 11000 | 幸福 → 密度升级门槛 |
| JobsW* | **无** | 住宅不设岗位；人去 C/I 上班 |

Graphics 样本 `5EB99AC6` 仅有 Lot 几何/path；parent `513E2625` → 住宅 Lot 模板链。

### 3.2 入住与升级闭环

```text
zone 规则采样 Desirability_RW* + TaxLevelRW*
  → 生长出 R Unit（ZoneID EE66978*/0C6905*/FA6B27F*…）
  → TotalPopulation 容量
  → 空置/收益 sink（ED171D42：Vacant fromBin + IncomeFromRW*）
  → 幸福规则 68BEC5D5（税→幸福/悲伤）
  → 6FAD431F：Density 升级
```

### 3.3 劳工供给（与商业雇工的对接面）

- R 模板 **不写** `JobsW*`；城市人口规则维护 WorkerW*/ExtraWorkerW* 库存。  
- C/I 到岗规则 `nearby … 1→0` **从附近库存拉人**。  
- 因此："住宅住人 → 城市劳工池 → 商业/工厂招聘"是 **全局资源耦合**，不是单栋建筑自环。  
- ⚠ "人口→Worker 库存"的逐条 AEB 归属仍标 [unknown]（与 `rci-behaviors.md` B3 一致）；但 C 侧 ExtraWorker sink 已解析。

对希望大厦劳工问题的启发：plop 建筑走 **AuthorOnly LaborProfile / Jobs 列 + 内部 pathRef**，形态对齐的是 **C/I 雇工槽**，不是 R 的 TotalPopulation。

---

## 4. 工业（I）：开工 · 生产 · 发货 · 污染

### 4.1 叶字段（zone 模板 `0846121C` 族 / W1D1 warehouse 样本）

| 属性 | 量级 | 符号 |
|---|---:|---|
| `0x0A7917C3` | 13 | TotalJobs |
| `0x0A7917C5/C6/C7` | 10 / 3 / 0 | JobsW1/2/3 |
| `0x0A7917C8` | 6 | **MinimumWorkersForProduction** |
| `0x0AB985AA` | 60 | 利润 token 默认 |
| `0x0E5467E7` | 180 | 升级门槛（高于 C） |
| `0x0C450089/8A` | 210 | SkillThreshold E2/E3（科技工位） |
| `0x0C131CC4` 等 | 大 | 空气/地面污染发射 |

Graphics 样本 `60DE5E12` Factory：parent `483FBB03` → 工业 Lot 模板；Lot 侧另有货运相关 key（如 `0x0C12EF27→C7BBD2F5`），与规则里货运 sink 号段呼应。

### 4.2 生产闭环

```text
工人到岗（同 C：CarW*/ExtraWorker*）且 ≥ MinimumWorkers
  → 生产/班次规则运转
  → 货运/物料 sink（50DEF80A 族：nearby 物料 −1 + 本地标志）
  → 货物进入城市库存 → 被 C 的 AmountOfGoods/fromBin 消费
教育水平规则 6AFC1E78：EducationLevel≥5 → 点亮 W3 科技工位
污染属性 → 城市 pollution map（消费端规则归属 ⚠）
```

---

## 5. 城市级 R↔C↔I 物质流（合成图）

```text
        ┌──────────── R 住宅 ────────────┐
        │ TotalPopulation / Happiness    │
        │ 供给 WorkerW* / ExtraWorkerW*  │
        └───────────────┬────────────────┘
                        │ nearby 拉人
        ┌───────────────▼────────────────┐
        │  C 商业 / I 工厂               │
        │  JobsW* + MinimumWorkers       │
        │  班次 · 交班 · 收班            │
        └───────┬────────────────┬───────┘
                │                │
         货物 fromBin      利润/税/Density
                │                │
                ▼                ▼
        AmountOfGoods ←── I 产出库存
        TotalShoppers（顾客）
```

通路前置：`AccessToOutside` sink；断路则 zone 不长、岗位空转。  
水电垃圾：R/C/I 均带 Amount/Capacity 族；消费规则与小学同构（RCI bundle 内部分引用缺席，属已知未决）。

---

## 6. 酒店化商业 ↔ 劳工 + 观光客

### 6.1 证据：没有独立 Hotel Graphics 建筑族

- Graphics RCI 普查 834 行：**零** `*Hotel*` Lot 名。  
- 符号表却有完整酒店名系：

| ID | 名称 | 角色 |
|---|---|---|
| `C8F3BCD5`…`DB4998AD` | `UnitNamesCW1D1Hotel` … `CW3D3Hotel` | 按财富×密度的 **UI 单位名** |
| `DD957817` 等 | `unitnamescw3d1`（非 Hotel） | 普通商业同档名 |
| `B908F572` | `DL_Tourists_HotelGuest` | Game `0x2F7D0004` **绘制列表**（酒店客人外观/队列） |
| `E3BFCDDD` / `63F11294` | `hotel` / `hotels` | 图层/图标类符号 |
| `833323DF` | `kResourceIDCW3D1` | 商业档位资源（与酒店名并行） |

`find_instance E347E95F`（`UnitNamesCW3D1Hotel`）在 Scripts/Game property 组无命中 → **名称表/locale 侧 ID**，不是第二套 Lot。

### 6.2 机制读法（分层置信度）

**已证明（数据层）**  
- 酒店是 **商业 (W,D) 的呈现/客源特化**，不是新的 RCI 组。  
- 普通 C 已具备 `TotalShoppers` + 顾客 sink；地标具备吸引力 + `TouristW*` +（同哈希）游客容量。  
- `DL_Tourists_HotelGuest` 证明引擎为"酒店客人"单独准备了 agent 绘制通道。

**强推断（与用户观察一致，规则逐条未钉死）**  
- 经典/地标附近 → 区域吸引力/`AttractionLevel` 升高 → 邻近 C 建筑切换到 **Hotel 名系** 并接入 **HotelGuest / Tourist** 客源，而不是改成新的 Graphics 模型。  
- 因此"酒店化商业能吸纳观光客" = **商业顾客槽 + 观光 agent 通道** 的并集，不是第三套独立经济。

**对 Hope / 劳工+观光的工程含义**  
1. 要"像酒店一样留客"：需要 **容量槽**（`0AD6A725` 量级）+ **承接规则/bundle** +（可选）HotelGuest 绘制；只加吸引力不够。  
2. 要"像商业一样招工"：需要对齐 C 的 **JobsW\* + MinimumWorkers + 工人 sink/班次**，并保证 path/门（`0DA69D6F` 等）让 agent 能进。  
3. 不要把学校语境的 `0AD6A725` 当残留剥掉——在商业/地标它是顾客/游客容量。

---

## 7. 与 plop 建筑（小学 / 希望大厦）的边界

| | RCI（本文件） | Plop（Menu2） |
|---|---|---|
| 入口 | zone 生长 | Menu2 → Unit |
| 模拟记录 | zone 模板 + 稀少叶子 Unit | 每栋 Unit `40E0C000` |
| 外观 | Graphics `41/42/43E1C000` | Lot `40E1C000` 同 instance |
| 雇工 | C/I Jobs + ExtraWorker sink | LaborProfile / 岗位列 + 教育或服务规则 |
| 观光 | 地标吸引力；酒店化 C 借 Tourist | 地标模板 `4E01E311` + TotalShoppers |
| path | 多数 RCI 无车辆 entry；_N 常有人行 | 服务建筑常挂专用 PathEntry |

---

## 8. 证据索引

| 产物 | 路径 |
|---|---|
| 锚点 Lot dump | `tmp/rci_anchor_A493781E.txt` |
| C parent / Game 模板 | `tmp/rci_parent_7F496E35.txt`、`tmp/rci_game_lot_EB0B4B60.txt` |
| R / I Graphics | `tmp/rci_res_5EB99AC6.txt`、`tmp/rci_ind_60DE5E12.txt` |
| C W1D1 zone 模板 | `tmp/rci_zone_tpl_C_W1D1_97E741B3.txt` |
| CW3 zone bundle | `tmp/rci_zone_bundle_CW3_FF5F4B70.txt` |
| 行为闭环 / 解剖 / 规则 | `tmp/behavior-exemplars/rci/rci-behaviors.md`、`rci-unit-anatomy.md`、`rci-rule-table.json` |
| Graphics 命名普查 | `tmp/rci-building-census-20261105.txt` |
| 观光差距（地标） | `docs/re/proven-student-behavior-and-tourism-gap-20261109.md` |

### 未决（诚实）

1. 酒店切换的 **触发规则 ID**（吸引力阈值？距离经典？资源 `hotels`？）未在 AEB 中逐条钉名。  
2. R → Worker 库存的具体规则链。  
3. I 产出资源 ID 与 C `9EB4CB2B/23B94F3B` 的精确一一对应。  
4. boutique W3 专用 zone 模板实例 ID（相对 W1 `97E741B3` 的数值放大表）未单独 dump；逻辑同族。

---

## 9. 给后续实验的最短清单

1. **只读对照**：同城放置 boutique（C）+ factory（I）+ townhouse（R），确认货流/客流 UI 与 `TotalShoppers`/`Jobs` 分母同源。  
2. **酒店化**：在经典旁与远郊各放同档 C，对比悬浮名是否切到 `UnitNamesCW*Hotel`、是否出现 HotelGuest 队列（**用新档城市，勿污染已有 golden 存档**）。  
3. **Hope**：劳工对齐 C 雇工槽语义；观光对齐地标 `TotalShoppers`+bundle，而不是只加 `0E542F39`。

---

## 10. 附录：Hope vs Copan（2026-11-10 实机对拍）

用户证据：Hope「已关闭！招聘中」+ 每日旅客 居民/孩童/游民全 0；Copan 浮标「265 居民」+ 购物/文化幸福文案。

| | Copan `40E0C000:4DC31F9B` | Hope live `3CAAEFBB`（补齐前） |
|---|---|---|
| parent | 地标 `4E01E311` | 同 |
| JobsW* / MinWorkers | **无**（不进招聘态） | 30/20/0，Min=**30**/50（60% 门槛） |
| TotalPopulation | 1100 + mirror C2 | 5000（有列） |
| 幸福 token `0E546790/1/2` | 600 / 6600 / 66000 | **缺** |
| R 通路 bundle `5D5EF1FF` | 有 | **缺** |
| R 占用边 `EE66978E` | 有 | **缺** |
| TotalShoppers | （叶未写，靠观光模板） | 390 |
| Willis A5 购物/利润/财富 | — | **缺** `362087BD`/`2B5A2BAF`/`CF116B9C` |
| UI 语义 | 住人地标 | 招工地标（关店招聘） |

**结论**：原版允许杂糅，但 Copan 的「住人」与 Hope 的「招聘」是两条栈。只写人口列或只写吸引力不够；要住人需 Copan 幸福+R bundle，要进客需 Willis 购物/利润/财富容器，要开门需把 MinWorkers 降到可达成比例（Willis≈17%）。
