# 城市一级菜单 Plop 行为闭环普查（含电力/水/污水/垃圾）

日期：2026-11-06。数据源：`SimCity_Game.package`（457 条 Menu2、411 条指向
Unit 的 plop 目录项）+ EcoGame 脚本库 `SimCity-Scripts_287520926.package`
（Unit/模板/bundle/sink/PathEntry）+ **UI 工作台同源的 sc-registry 名字表**
（`src-tauri/resources/database_main.s3db`）+ 官方 locale（zh-tw/en-us）。

探针：`probes/menu/plop_behavior_map.rs`（复用工作台的解析语义：`dbpf` +
`sc-properties` 属性解析、`sc_registry::Registry` 实例名、`locale::parse_string_table`
标题解析）。产物：`tmp/plop-behavior-map-20261106/plop-map.json`。

## 0. 读法：一个建筑的"闭环"由三层构成

| 层 | 内容 | 作用 |
|---|---|---|
| 共享底座 bundle | `11D271CD`、`18CA40A1`(CriminalWhiteCollarTarget)、`2B1E8F1D`、`4A42F50E`、`6404FA6E`、`8272746D`、`DFFA74FC`、`E0C02E1F`、`30645625`、`5D7F8D92`；通用 `95573B5D`、`9A1BFB2F` | 几乎所有市政建筑都继承的通用 bin/规则/事件 |
| 水电垃圾三件套 | 电 `48939D96`、水 `825B4CA4`、垃圾 `D7C62954` | 消费/产出网络；来自模板链继承 |
| **签名 bundle** | 本文表格主体，每类建筑独有的 sink + 规则 + bin | **该建筑"是什么"的判据** |

判定"核心闭环 bundle"的机械标准（探针实现）：同一 bundle 同时含
① bin 行（`0x07E0AD23`）② sink 绑定（`0x0A2725CE`）③ sink 规则绑定
（`0x0A2725CF`）。只有签名 bundle 才具备这个三元组；共享底座多为
bins-only / events-only。

**最小闭环配方 = 签名 bundle + 水电垃圾三件套（若需要）+ 模板**。
扩展能力（次级单位 `0x07E0AD0A`、addon 容器 `0x0AFE27CA`、菜单挂载
`0x0E9F828E/8F/90`）与闭环无关，可全删。

## 1. 消防（kCategoryIDFire）

| 建筑 | Unit（注册表名） | 签名闭环 bundle | 闭环内容 | 水电垃圾 | 扩展 | 最小配方 |
|---|---|---|---|---|---|---|
| 消防局 | `E917279C` `T_Fire_T1_Base` | **`8B7B1FC8`**（11 bin） | sink `6D1A2BC0`/`A7CB1C3C`/`22965D18` → `NumberOfFireTrucks`、`kResourceIDFireTruckT1`×2；sink 规则 3、常态规则 1、事件 4 | 电/水/垃圾 | — | `8B7B1FC8` + `202E8FFB`(3 bin/2 事件) + 三件套 |
| 大型消防局 | `A6B04F92` | `F9ACFDFD`（6 bin）+ `472644E9` | 无 sink 三元组（车辆循环疑在继承层） | 电/水/垃圾 | 次级单位×1、addon | 需补一次继承层核查 |

## 2. 警察（kCategoryIDPolice）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 | 水电垃圾 | 扩展 | 最小配方 |
|---|---|---|---|---|---|---|
| 警察局 | `18D36A29` `T_Police_T1_Base_01_01_N` | **`01387CA8`**（policelot，3 bin）+ **`E062C5F5`**（Jail，24 bin） | `01387CA8` sink×4 → `kCategoryIDPolice`（sink 规则 4、事件 4）；Jail sink `E062C5F5`/`DC12000E` → `kResourceIDOpenCells`（sink 规则 2） | 电/水/垃圾 | addon、挂载 2 | `01387CA8`+`E062C5F5`+`101106E5`(9 bin) + 三件套 |
| 警署 | `B40CD52C` | **`101106E6`**（16 bin）+ Jail + `E8E286D6`（4 bin） | 4 sink → 侦探/外联车停放（`kResourceIDPoliceDetectiveVan[_Parking]`、`kResourceIDPoliceOutreachVan[_Parking]`）；`E8E286D6` sink×4 → `kCategoryIDPolice`（sink 规则 4） | 电/水/垃圾 | addon、挂载 3 | `101106E6`+Jail+`E8E286D6`+三件套 |

## 3. 医疗（kCategoryIDHealth）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 | 水电垃圾 | 最小配方 |
|---|---|---|---|---|---|
| 诊所 | `30236D71` | **`61C1B2DF`**（`kCategoryIDHospital`，53 bin） | 24 sink（sink 规则 24）→ `kResourceIDWaitingRoom` 等；常态规则 3、事件 2 | 电/水/垃圾 | `61C1B2DF` + 三件套 |
| 医院 | `E02667A4` | **`536F4D49`**（53 bin） | 24 sink（sink 规则 24，同上资源族）；常态规则 5 | 电/水/垃圾 | `536F4D49` + 三件套 |

## 4. 教育（kCategoryIDEducation）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 | 水电垃圾 | 扩展 | 最小配方 |
|---|---|---|---|---|---|---|
| 小学 | `C264A268` `T_Edu_T1_GradeSchoolBase_01_01_N` | **`6C042367`**（23 bin） | sink `78B96709` → `kResourceIDSpotsForKids`；sink 规则 `B52C889D`（入学）；常态规则 10（含 `C193A5BB` 每日清零）、事件 4 | 电/水/垃圾 | 次级单位×1、addon、挂载 4 | `6C042367` + 三件套 |
| 中学 | `64F750E2` | **`6C042367`**（同一核心）+ `29D4B82E`（1 bin/3 事件） | 同上 | 电/水/垃圾 | 次级单位×1、addon、挂载 2 | `6C042367`+`29D4B82E`+三件套 |
| 社区大学 | `A826FA3E` | **`6C042366`**（30 bin） | 4 sink → `kResourceIDSpotsForKids`；sink 规则 4；常态规则 20 | 电/水/垃圾 | addon | `6C042366`+`487CDA43`(3 bin)+三件套 |
| 大学 | `4250AE35` `T_Edu_T2_University_01_01_N` | **`3256C0A2`**（31 bin）+ `69772224`(hq, 6 bin) | 5 sink → `SpotsForKids`×4 + `kResourceIDStudentToLeave`；rules 20；`69772224` sink `33DDC394` | 电/水/垃圾 | addon | `3256C0A2`+`69772224`+三件套 |
| 公共图书馆 | `0BFE06AB` | **`A20CAC71`**（49 bin） | 39 sink（工人/技能流，`ExtraWorkerW1..W3` 等）；sink 规则 39；常态规则 3、事件 4 | 电/水/垃圾 | — | `A20CAC71`+三件套 |
| 校车站牌 | `9182FA79` | `5A045D31`(11 bin)、`A35E487D`(20 bin)、`A7C6D100`(17 bin) | 多个 Kids/接送 sink（`0163C6C1 Kids`） | — | — | 交通附属，非教育主闭环 |

**结论（s23 用）**：小学/中学的教育闭环就是 `6C042367` 一个 bundle；
扩展（车库/教室/旗杆）全部在 Unit 的 `0x07E0AD0A`/`0x0AFE27CA` 与
Menu2 挂载里，不参与闭环。

## 5. 公园（nature/park 子菜单）

公园没有 sink 三元组——它的行为是**地价 + 吸引力 + 游客规则**，不靠
sink 投递。签名 bundle 集合（多数公园共用）：
`5D1EC901`、`5FFA86B9`、`7187A49D`、`C0672F54`、`EA0254BF`。
树/林类：`6778D02B`（单 bundle）；清除森林：`4A42F50E`。

| 建筑 | Unit | 签名 bundle | 特征 | 最小配方 |
|---|---|---|---|---|
| 城市公園/社區公園/小徑公園族 | `788F8F9E`/`74C549D3`/… | 上述 5 件套 | 无 sink；含地价/吸引力规则；模板带水电垃圾（公园也计入消费） | 公园五件套 + 模板（水电三件套随模板） |
| 池塘 | `077E326B` | 同上 | 同上 | 同上 |
| 种植森林/清除森林 | `0DCCD822`/`1B7C0FF7` | `4A42F50E` | 仅地形/绿化规则 | `4A42F50E` |

## 6. 交通（kResourceIDBus / 机场 / 渡轮 / 重轨）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 | 水电 | 扩展 | 走读/路径 |
|---|---|---|---|---|---|---|
| 公車站牌 | `FE043FDC` | `377AA3B3`(19 bin)、`42F3AD5E`(2 bin)、`98A3F491`(102 bin) | 37/48/55 个 sink 规则；`kResourceIDBusPickUp`、`kResourceIDTradeBusPickUp` | — | — | 0 path |
| 公車站（Shuttle Depot） | `0403377D` `T_Transport_Shuttle_T1_Base_Modern_01` | `03D50124`、`A2405F15`、`AB3A9BB9`(`movingtruck`) | 各 1 bin / sink 规则 1–2 | 电/水/垃圾 | 次级×1、addon | 4 path |
| 公車總站 | `4722AB8D` `T_Transport_Bus_T2_Base_01` | 上述 + `0E4DDAC7`(66 bin,`Tourist_Leaving`)、`4B8D7A2F`(busR2Station)、`67731342`(CoachPickUp) | sink 规则合计 >100 | 电/水/垃圾 | 次级×1、addon、挂载 4 | 7 path |
| 公車轉乘站 | `919D2E2C` | `377AA3B3`/`42F3AD5E`/`98A3F491` | 同上 | — | — | 2 path |
| 跑道 | `AE936DEA` | `5540B5FE`(13 bin) | 3 sink 规则 | — | — | 2 path |
| 客運渡輪碼頭 | `0EC5C375` | `C795B2EF`(96 bin)、`D6FF2FCA`(149 bin)、`A7B4785E` | `kResourceIDFerryPickUp` | — | — | 4 path |
| 客運火車站 | `8357B87F` `T_Transport_Rail_T2_Base_01` | `AFF9B15B`(156 bin)、`E2CA5426`(96 bin)、`E39E7BB5` | `Tourist_Leaving` 等 | — | addon | 4 path |

交通建筑的闭环是"到站/上车"型：sink 资源是载具/乘客/货物，必须靠
PathEntry 网络（车辆 filter）抵达；这是 path 语义真正起作用的地方。

## 7. 政府（cMayorRating.kUICivic）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 | 水电 | 扩展 |
|---|---|---|---|---|---|
| 鎮公所 | `2D8850B8` | `0FDD94D0`(18 bin)、`69772224`(hq)、`F237990A`(24 bin) | mayor/Simoleons/Protester；sink 规则 4/1/1 | 电/水/垃圾 | addon、挂载 6、5 path |
| 市長豪宅 | `B4399D59` | `478E1C0F`(5 bin)、`A20CAC71` | `mayor`、`Simoleons`；sink 规则 3/39 | 电/水/垃圾 | 3 path |
| 市長官邸 | `57EA3A31` | `3A8263E3`(19 bin)、`444C2C8D`(9 bin)、`F237990A` | `HotItemsW3`、mayor、Protester | 电/水/垃圾 | addon、挂载 7、6 path |

## 8. 电力（kCategoryIDPower）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 | 依赖 |
|---|---|---|---|---|
| 所有电厂共用 | — | **`4106B159`**（7 bin） | sink `17D4D243` + 规则 `C9F69342`/`FA31F3E3`，资源 `BA111A2A`（生产/输出） | — |
| 太陽能 | `9122C84D` | `408E22D8`(12 bin,1 规则,2 事件) + `4106B159` | — | — |
| 煤礦/石油 | `94C243EB`/`6E296726` | **`114B720A`**（52 bin，20 sink→排放/燃料链）+ 燃料 `683FE8D8`(煤,`kCategoryIDCoal`)/`16D55F40`(油,`kResourceIDOilCrude`) + `E1700A3F`(48 bin,10 规则) | 燃料输入 + 排放/污染 | 水（两厂均需） |
| 風力 | `B5923139` | `148FBF48`(7 bin,`SkillTokenE2/E3`)+`4106B159`+`BD50AFD9`(servicetruck/OpenForBusiness) | — | — |
| 核能 | `DA87B792` | `148FBF48`+`234B6693`(`Injury`/`Ambulance`)+`4106B159` | 事故/救护 | 水 |

## 9. 水（kCategoryIDWater）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 |
|---|---|---|---|
| 水塔 | `BBF240B9` `T_Water_T1_WaterTower_01_01_N` | **`21CAFB39`**(3 bin,sink `E9171885`) + **`A8D0E850`**(22 bin, sink `020DF4EE` → `kResourceIDWaterInMap`, 规则 `15FF0D7A` 等 4 条) | 产水入管网 |
| 抽水泵浦站 | `244F51DB` `T_Water_T2_Base_01_01_N` | `6E4E4262`（继承层 5 bin）+ 电 `48939D96` + 垃圾 `D7C62954` | 需电，产水 |

## 10. 污水（kCategoryIDSewage）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 |
|---|---|---|---|
| 污水排放管道 | `4510D0E3` `T_Sewage_T1_Base_01_01_N` | **`8DD57E9D`**（16 bin） | sink `767285C0`/`230EB18E` → `kResourceIDSewageMainStorage`；规则 7、事件 3 |
| 污水處理廠 | `DCA517F2` `T_Sewage_T2_Base_01_01_N` | **同一 `8DD57E9D`** + 电 `48939D96` | 同上；需电 |

## 11. 垃圾（kCategoryIDGarbage）

| 建筑 | Unit | 签名闭环 bundle | 闭环内容 |
|---|---|---|---|
| 垃圾處理廠 | `5244A0A5` `T_Garbage_T1_Base` | **`C2A99083`**（4 bin） | sink `C2A99083` → `garbageToken`；规则 2、事件 2 |
| 回收中心 | `A16FC27F` | `A0067CB2`(`Recyclables`)、`CF71DF81`(`Plastic`)、`48960264`(`Metal`)、`E365109C`(`Alloy`) | 4 条回收生产线，各 1 sink + 1 规则 |

## 12. 最小闭环配方模板（给编辑器）

```
build_plop(dept, building):
  unit = new_instance()
  unit.parent = template_for(dept)            # 教育 7E4B99E5 / 按部门查表
  unit.bundles_enable = [signature_bundle]    # 本表"签名闭环 bundle"列
                     + [power, water, garbage 按"依赖"列]
  unit.bundles_exclude = [扩展 bundle]         # 可选：不要扩展时
  unit.secondary_units = []                   # 不写 0x07E0AD0A
  unit.addon_container = 无                   # 不写 0x0AFE27CA
  menu.mounts = []                            # 不写 0x0E9F828E/8F/90
  lot.board = dept_board[dept]                # 教育 6A1785B4 等
  lot.paths = path_plan[dept]                 # 走读类：仅 Pedestrians 路
```

**fail-closed 检查**（对应域）：签名 bundle 存在且三元组完整（bin+sink+规则）、
sink 资源已在资源表注册、三件套按依赖列齐全、扩展字段为空（若声明"最小"）、
lot board 与部门匹配、菜单 toolType 显式。

## 13. 命名修正（重要，影响既有文档）

用工作台注册表解析出的真实名字，纠正此前按频率推测的 path 语义：

| PathEntry | filter | **注册表真名** | 此前误判 |
|---|---|---|---|
| `7C9C4FFF` | `C37D661D` | **School busses** | "车道"（实为校车专用） |
| `03106C2E` | `979CC22E` | **Cars And Crap** | "人行道"（实为**污水/杂物网络**——载体 `EB91A84C` 的 bin 是 sewageimported/exported） |
| `418A18B1` | `E0CD3122` | **Pedestrians** | 误当作车辆类（注意：该 filter 无载体声明，行人实际走"默认全通"） |
| `359C46BB` 等道路 | 多 filter | 道路类 | — |
| — | `C0D6DEEE` | **WaterMain** | 水 agent 网络 |
| — | `5DFAC324` | **Power Lines** | 电 agent 网络 |

因此 s23 走读学校应引用 **`418A18B1`（Pedestrians）** 作为人行道，
而不是 `03106C2E`；校车场景才用 `7C9C4FFF`（School busses）。

## 14. 未决 / 注意

1. 注册表对 **bundle** 命名覆盖有限（多为内容名如 `Jail`、`policelot`、
   `earthquake`），表中 bundle 以 ID 为准；后续可用 ERZ 规则集名字补全。
2. 部分 sink 资源在注册表无名（表中显示 `?`），不影响结构判定。
3. 大型消防局 / 抽水泵浦站的签名闭环疑在继承层，需再跑一次"只列
   继承 bundle 的三元组"确认。
4. 本表只覆盖 7 个部门 + 4 个公用事业；DLC/EP1 建筑（赌场、世博、
   机场扩展等）未纳入。
5. 生产类（电/水/污水/垃圾）的"容量"落点与学校课桌问题同源：
   `0x0AD7FAC9` 类 named 值可能只影响 UI，功能上限需规则层确认。

## 证据文件

- 探针：`probes/menu/plop_behavior_map.rs`（用法见文件头）
- 数据：`tmp/plop-behavior-map-20261106/plop-map.json`
- 名字表：`src-tauri/resources/database_main.s3db`（工作台同源）
- 相关：`docs/design/behavior-capability-composition.md`（最小闭环方法论）、
  `docs/re/asset-behavior-census-and-gating-20261106.md`（行为/门控总报告）

## 15. Path filter 全语义（111 个 filter 的完整普查）

数据源：`tmp/family-loop-census/all.json`（base + Graphics + DLC0 + EP1 数据包 +
全部 59 个 DLC scripts + EP1 scripts）。统计：**111 个 filter，26 个注册表有名字**。

### 15.1 有名字的 filter（按成员数排序）

| filter | 注册表名 | 成员 PathEntry 数 | 声明它的载体（`0x0EB22775`） |
|---|---|---:|---|
| `2B9F0330` | Stop Signs? | 104 | — |
| `979CC22E` | **Cars And Crap** | 80 | `EB91A84C`（bin=`sewageimported`/`sewageexported` → **污水 agent**） |
| `4E5BC655` | Trade Trucks | 61 | — |
| `4A6F1921` | Busses | 48 | — |
| `632102DB` | Police | 48 | — |
| `E0CD3122` | Pedestrians | 48 | —（无载体 → 行人走默认全通） |
| `BF5E73C2` | Garbage trucks | 47 | — |
| `C37D661D` | School busses | 46 | — |
| `A1FC28C6` | Mayor | 43 | — |
| `5DFAC324` | Power Lines | 38 | `E99F8B9D`（电）、EP1 `C3003063` |
| `C0D6DEEE` | WaterMain | 38 | `02957CE3`（水） |
| `A26BC91A` | Intersect | 28 | — |
| `587D51B9` | Buldoze | 21 | — |
| `1CD3881B` | Parks | 10 | — |
| `3B0E63C8` | Low Density | 10 | — |
| `E281D1A3` | Street Cars | 10 | — |
| `9A79F996` | Train | 9 | — |
| `1A0E13A2` | Ships?? | 7 | — |
| `3B0E63CB` | Medium Density | 7 | — |
| `3B0E63CA` | High Density | 5 | — |
| `A5AA50CC` | Shuttle Busses | 5 | `36BA75FA`（bin=`kResourceIDAmusementParkGuests`） |
| `2EE66207` | Train Fright!? | 4 | — |
| `08DF7947` | Garbage Path | 2 | — |
| `67505A00` | Oil Path | 2 | — |
| `C4AAF79A` | Wind Services | 2 | `E2AD993F`（bin=`service`） |
| `90F1BBCC` | University Path | 1 | — |

### 15.2 全部载体（8 个，仅它们声明 filter）

| 载体 | filter | 身份证据 |
|---|---|---|
| `EB91A84C` | `979CC22E` Cars And Crap | bin `sewageimported` / `sewageexported` → **污水** |
| `02957CE3` | `C0D6DEEE` WaterMain | bin `9E3C3DFA`=水 + `kResourceIDGerms` → **水** |
| `E99F8B9D` | `5DFAC324` Power Lines | bin `8E8B4E10`=电 → **电** |
| `E2AD993F` | `C4AAF79A` Wind Services | bin `service` → **服务车（风机维保）** |
| `36BA75FA` | `A5AA50CC` Shuttle Busses | bin `kResourceIDAmusementParkGuests` → **接驳巴士** |
| `3FF2747F` | `69E76B61`（无名，18 成员） | bin 无名；成员含 `04F90CCD`（交叉口模板）→ 服务类 |
| `EBCD02ED` | `799F1CA5`（无名，10 成员） | bin 无名；成员含交叉口模板 → 服务类 |
| EP1 `C3003063` | `5DFAC324` Power Lines | EP1 电力变体 |

**其余 103 个 filter 没有任何载体声明**——按源码，未声明 filter 的载体默认
**全通**（`mActivePathEntries` 全 1）。因此这些 filter（Pedestrians、Busses、
Police、School busses、Garbage trucks、Trade Trucks、Mayor、Parks、Train、
Ships、Street Cars、密度类、Intersect、Buldoze…）在当前数据下是**路径标签**，
用于其他系统（区划/道路类型/园地/相机等），而不是"强制排他"。

### 15.3 filter 的两个方向语义（源码定案）

1. **PathEntry 声明成员**：`0x0D6498E8` = 本路径属于哪些 filter；装载时把
   该 entry 的索引 OR 进每个 filter 的 256 位集。
2. **载体选择 filter**：`0x0EB22775` = 我只走哪个 filter 的路径；缺省全 1。
   **没有声明任何 filter 的路径，对"有 filter 的载体"不可见**（位未置），
   对"无 filter 的载体"可见——所以"污水/水/电"是严格受限网络，
   行人/巴士/警车等是宽松网络。
3. **索引 <256**：`FindPathsInSphere` 对 `idx >= 0x100` 直接跳过；当前
   sim path entry 154 条，余量 102。

## 16. DLC / EP1 覆盖（家族命名空间）

### 16.1 结构（实锤）

DLC/EP1 使用**独立组命名空间**，后缀与基础游戏一致、前缀不同：

| 层 | 基础游戏 | EP1 | DLC（示例） |
|---|---|---|---|
| 主表 | `40800200` | `40800201` | `40800201` |
| Unit | `40E0C000` | `61EFC000`（147） | `51EFC000`/`5BEFC000`/`5CEFC000`… |
| 资源 | `40E0C100` | `61EFC100`（594） | … |
| PathEntry | `40E0C400` | `61EFC400`（13） | `41F0C400`（1） |
| Sink | `40E0C500` | `61EFC500`（174） | `41F0C500`/`43F0C500`… |
| Transport | `40E0C600` | `61EFC600`（98） | `41F0C600`… |
| **Bundle** | `40E0C700` | **`61EFC700`（286）** | `51EFC700`…（在各自 Scripts 包） |
| 菜单 | `09878A01` | **`61F0C900`**（99） | `41F0C900`/`52F0C900`… |

即：**DLC 数据包只有 unit/sink/transport/path/resource，bundle 全在对应的
`SimCityDLC*-Scripts_*.package` 里**。用"组后缀"识别即可通用处理。

### 16.2 普查结果

| 范围 | 菜单数 | 有闭环三元组 | 说明 |
|---|---:|---:|---|
| EP1（EP1 data + EP1 scripts + 基础 scripts） | 96 | 60 | 大量"巨塔楼层/冠楼"模块，各带自己的闭环 |
| DLC0（DLC0 data + 全部 59 个 DLC scripts） | 72 | 19 | 多为游乐设施/地标（无 sink），少数设施有闭环 |

EP1 代表性闭环：
- 抽水泵浦站 `F86C906D` → `21CAFB39` + `A8D0E850`（同基础水塔）
- 波浪發電廠 `03BEC006` → `4106B159` + `26A2E32F`
- 公寓樓層 `E41C0CB3` → 11 个闭环 bundle（住户/岗位/垃圾等）
- 商場樓層 `8A807C96` → 9 个；維安樓層 `3F02A9E5` → 4 个
- OmegaCo 總部 `2AF0198C` → `69772224`（hq）
- 醫療/法治無人機機棚 `42FE15E7`/`770C25A3` → `8C4EA1B1`/`5E39A1B5` + `C272E0A1`

DLC0 代表性闭环：
- 通勤飛船停放處 `3001966A` → `138C0B24`/`581EAF6D`/`C38D8E6B`
- 雙層巴士車庫 `40FD7FA7` → `1F8493AA`
- 宗教場所（寺廟/廟宇/清真寺）`5C8790F6`/`B697EBEA`/`8D603F7B` → `9FB24395`
- Nissan 充電站 `8B308226` → `E06A8223`/`FC436B49`
- 羅馬幸運賭場 `A94FB11E` → `1283EFC5`/`75D22FD0`/`75D22FD1`

> 覆盖边界：DLC0 数据包含多个家族（`41F0`/`43F0`/`46F0`/`47F0`/`48F0`/`49F0`/
> `50F0`/`51F0`/`52F0`/`56F0`…），本次把全部 DLC scripts 一并装入，
> 7 条菜单的 unit 仍未解析（其 unit 只存在于未加载的 DLC 数据包），
> 但不影响"家族结构 + 识别方法"的结论。

## 17. 两个继承层疑点定案

| 建筑 | 结论 | 证据 |
|---|---|---|
| **大型消防局** `A6B04F92` | **自身没有消防车闭环**；`8B7B1FC8`（NumberOfFireTrucks/FireTruckT1 + 3 sink）在**消防车车库模块单位 `2F608238`** 上。它是"容器建筑"：最小可用 = 本体（`F9ACFDFD` 6 bin/2 事件 + `472644E9`）+ 至少 1 个车库模块 | `82E02B34` 模块组：Fire Station Garage `2F608238` own `8B7B1FC8`(core)；大型消防局 flat 集合无任何 sink 三元组 |
| **抽水泵浦站** `244F51DB` | 取水闭环来自**继承的 `6E4E4262`**（23 bin；sink `4E2DF4A2`/`E9171885`；规则 `F955D0B2`；被 2 个单位引用）；自身 `111F869C`(1 bin) 只是辅助 | 展平后 core 列表含 `6E4E4262`；其 bundle 摘要三要素齐全 |

**结论**：并非所有部门建筑都"自包含"。编辑器需要区分两种闭环形态：
**自包含型**（学校/医院/警局/电厂）与**容器型**（大型消防局→车库模块；
巨塔→楼层模块）。容器型的最小配方必须包含至少一个提供闭环的模块。

## 18. 对编辑器的直接结论（补充）

1. 最小闭环表新增"容器型"标记：容器建筑只提供外壳与事件门，行为来自模块；
   编辑器应把模块作为**行为依赖**而非外观选项。
2. path 类型面板的"通行主体"以**载体**为准（8 个），而不是以 filter 名为准；
   filter 名只作路径标签显示。
3. 水电/污水三条管网是**严格受限网络**（载体带 filter），铺管网 = 铺声明
   对应 filter 的 path；行人/巴士/警车等无载体声明，默认可走所有路径。
4. DLC/EP1 编辑器必须支持**家族命名空间**（prefix 不固定、后缀固定），
   bundle 从 Scripts 包解析。

## 附录 A. 全部 111 个 path filter（成员数降序）

| # | filter | 注册表名 | 成员数 | 载体 | 样例成员 |
|---:|---|---|---:|---|---|
| 1 | `2B9F0330` | Stop Signs? | 104 | — | 00883F45, 012F4CE1, 017A430A |
| 2 | `979CC22E` | Cars And Crap | 80 | SimCity_Game:EB91A84C | 012F4CE1, 017A430A, 03106C2E |
| 3 | `4E5BC655` | Trade Trucks | 61 | — | 012F4CE1, 017A430A, 06191B6F |
| 4 | `B0861DD7` | — | 56 | — | 017A430A, 09674DA3, 0EAF693C |
| 5 | `4A6F1921` | Busses | 48 | — | 012F4CE1, 017A430A, 06191B6F |
| 6 | `632102DB` | Police | 48 | — | 012F4CE1, 017A430A, 06191B6F |
| 7 | `E0CD3122` | Pedestrians | 48 | — | 012F4CE1, 01F8E755, 06191B6F |
| 8 | `BF5E73C2` | Garbage trucks | 47 | — | 012F4CE1, 017A430A, 06191B6F |
| 9 | `C37D661D` | School busses | 46 | — | 012F4CE1, 017A430A, 06191B6F |
| 10 | `2C7C7986` | — | 43 | — | 012F4CE1, 01F8E755, 0EAF693C |
| 11 | `A1FC28C6` | Mayor | 43 | — | 017A430A, 06191B6F, 09674DA3 |
| 12 | `5DFAC324` | Power Lines | 38 | SimCity_Game:E99F8B9D, SimCityDataEP1:C3003063 | 012F4CE1, 06191B6F, 09674DA3 |
| 13 | `C0D6DEEE` | WaterMain | 38 | SimCity_Game:02957CE3 | 012F4CE1, 06191B6F, 09674DA3 |
| 14 | `044DD5A5` | — | 33 | — | 09674DA3, 17E70A7D, 1F04AFA4 |
| 15 | `A26BC91A` | Intersect | 28 | — | 012F4CE1, 017A430A, 06191B6F |
| 16 | `2D65A776` | — | 26 | — | 17E70A7D, 31B2B922, 359C46BB |
| 17 | `9B52C09D` | — | 25 | — | 06191B6F, 09674DA3, 1F04AFA4 |
| 18 | `3713EF31` | — | 23 | — | 03106C2E, 06191B6F, 173E62C1 |
| 19 | `587D51B9` | Buldoze | 21 | — | 00883F45, 06191B6F, 31AB71CE |
| 20 | `69E76B61` | — | 18 | SimCity_Game:3FF2747F | 017A430A, 04F90CCD, 4C4DFCB1 |
| 21 | `1F12721F` | — | 12 | — | 09674DA3, 1F04AFA4, 21EFBFCA |
| 22 | `2AA2E11C` | — | 11 | — | 09674DA3, 1F04AFA4, 21EFBFCA |
| 23 | `1CD3881B` | Parks | 10 | — | 012F4CE1, 06191B6F, 31AB71CE |
| 24 | `3B0E63C8` | Low Density | 10 | — | 012F4CE1, 06191B6F, 31AB71CE |
| 25 | `54B6D9A3` | — | 10 | — | 017A430A, 4C4DFCB1, 4CB8F48E |
| 26 | `799F1CA5` | — | 10 | SimCity_Game:EBCD02ED | 017A430A, 4C4DFCB1, 4CB8F48E |
| 27 | `A2F9BEEB` | — | 10 | — | 012F4CE1, 06191B6F, 31AB71CE |
| 28 | `E281D1A3` | Street Cars | 10 | — | 31AB71CE, 6490263F, 6D99366C |
| 29 | `9A79F996` | Train | 9 | — | 5509DDA9, 565BCF6C, 6F140E38 |
| 30 | `B943E91B` | — | 8 | — | 017A430A, 173E62C1, 4C4DFCB1 |
| 31 | `1A0E13A2` | Ships?? | 7 | — | 224E8DD6, 38758F9C, B6D30176 |
| 32 | `2048E681` | — | 7 | — | 06191B6F, 31AB71CE, 8248C6AC |
| 33 | `3B0E63CB` | Medium Density | 7 | — | 012F4CE1, 06191B6F, 31AB71CE |
| 34 | `3B0E63CA` | High Density | 5 | — | 012F4CE1, 31AB71CE, 8248C6AC |
| 35 | `A5AA50CC` | Shuttle Busses | 5 | SimCity_Game:36BA75FA | 01F8E755, 17E70A7D, 675B61B1 |
| 36 | `FDB341D7` | — | 5 | — | 2F229A6E, 32C07DCF, 409A8C52 |
| 37 | `2EE66207` | Train Fright!? | 4 | — | 2AE8FEC7, 58EBB737, DAE55396 |
| 38 | `4A0F89E0` | — | 4 | — | 9FE3CBF5, B4E028F7, CF20326E |
| 39 | `6192801B` | — | 4 | — | 31AB71CE, 73E1BE27, AC45F73A |
| 40 | `67E330A8` | — | 4 | — | 9FE3CBF5, B4E028F7, CF20326E |
| 41 | `70387C46` | — | 4 | — | 2F229A6E, 409A8C52, 45909F1E |
| 42 | `A536CA85` | — | 4 | — | 55E3C97C, B4E028F7, C7143417 |
| 43 | `B940136F` | — | 4 | — | 2F229A6E, 863A6E7D, C1C2EF97 |
| 44 | `BA60C5C9` | — | 4 | — | 61DC7636, 7BCA0112, 96DF5720 |
| 45 | `C52A6411` | — | 4 | — | 5509DDA9, 6F140E38, 7B7A0394 |
| 46 | `D2D603C7` | — | 4 | — | AEE71498, DAE55396, F516F4B2 |
| 47 | `EF8F168B` | — | 4 | — | 2F229A6E, 82B167CA, 863A6E7D |
| 48 | `F37E0850` | — | 4 | — | 06191B6F, 8248C6AC, 97596225 |
| 49 | `1C86750C` | — | 3 | — | 01F8E755, 2AE8FEC7, 675B61B1 |
| 50 | `27C9C416` | — | 3 | — | B6D30176, F2C46AA3, F2C46AA9 |
| 51 | `4CB1F193` | — | 3 | — | 32C07DCF, 409A8C52, 45909F1E |
| 52 | `E49A5DC8` | — | 3 | — | 61DC7636, 6F3F4505, 7BCA0112 |
| 53 | `E5BDBB1F` | — | 3 | — | 2CFCF6D1, 7641C45C, C40DAE0F |
| 54 | `F17E0506` | — | 3 | — | 31AB71CE, 963203AA, F5DFFCC2 |
| 55 | `FA6CFBD5` | — | 3 | — | 9FE3CBF5, B4E028F7, CF20326E |
| 56 | `0714D370` | — | 2 | — | 252B8051, 82DABDA2 |
| 57 | `0742F3F8` | — | 2 | — | 252B8051, 82DABDA2 |
| 58 | `08DF7947` | Garbage Path | 2 | — | 2EC89702, 5B98F4F1 |
| 59 | `1DA24A43` | — | 2 | — | 17E70A7D, 58EBB737 |
| 60 | `2366F19C` | — | 2 | — | 5700D3A5, 8F2EBD7A |
| 61 | `3BF25BBA` | — | 2 | — | 5A5A4723, 80DBC84A |
| 62 | `47093F0A` | — | 2 | — | 675B61B1, AEE71498 |
| 63 | `4A4EFE8C` | — | 2 | — | 01F8E755, 2AE8FEC7 |
| 64 | `55008112` | — | 2 | — | 16BB20E7, C4C0B5B6 |
| 65 | `578994D2` | — | 2 | — | 863A6E7D, C1C2EF97 |
| 66 | `67505A00` | Oil Path | 2 | — | 55E3C97C, C7143417 |
| 67 | `6888D48E` | — | 2 | — | 31AB71CE, B89F598C |
| 68 | `7DEA45CA` | — | 2 | — | 96DF5720, DAD915B4 |
| 69 | `7F87087E` | — | 2 | — | 16BB20E7, C4C0B5B6 |
| 70 | `816E708E` | — | 2 | — | 01F8E755, DAECF62F |
| 71 | `90B0C410` | — | 2 | — | 00883F45, F6F599E2 |
| 72 | `A396B93D` | — | 2 | — | B9F57C65, CDC69FE4 |
| 73 | `B7E5700E` | — | 2 | — | 7F7F4E43, D6335EF1 |
| 74 | `C4AAF79A` | Wind Services | 2 | SimCity_Game:E2AD993F | 00883F45, F6F599E2 |
| 75 | `C5D2EE9C` | — | 2 | — | 5700D3A5, 8F2EBD7A |
| 76 | `D9EB8317` | — | 2 | — | 2F229A6E, 82B167CA |
| 77 | `F267862D` | — | 2 | — | 7F7F4E43, A334785C |
| 78 | `062D8980` | — | 1 | — | 6BD045FC |
| 79 | `118D2027` | — | 1 | — | C40DAE0F |
| 80 | `11F73095` | — | 1 | — | 5B98F4F1 |
| 81 | `156737B1` | — | 1 | — | 2AE8FEC7 |
| 82 | `158D2A2B` | — | 1 | — | 04F90CCD |
| 83 | `1E29C493` | — | 1 | — | 7C9C4FFF |
| 84 | `20322329` | — | 1 | — | 46AE3EA0 |
| 85 | `26299A1C` | — | 1 | — | 55E3C97C |
| 86 | `319F4943` | — | 1 | — | 8724F05F |
| 87 | `46359AE1` | — | 1 | — | FE4F2761 |
| 88 | `47419E66` | — | 1 | — | 863A6E7D |
| 89 | `4AEF0CDA` | — | 1 | — | DDAE4E43 |
| 90 | `4B8AF107` | — | 1 | — | 15AE0A8A |
| 91 | `4E6B007B` | — | 1 | — | 3155D351 |
| 92 | `63251644` | — | 1 | — | C1C2EF97 |
| 93 | `63DA9CA4` | — | 1 | — | B290AC27 |
| 94 | `65E9144E` | — | 1 | — | 459AF036 |
| 95 | `67E55D06` | — | 1 | — | 00883F45 |
| 96 | `73AA3EF5` | — | 1 | — | 32C07DCF |
| 97 | `774B795F` | — | 1 | — | 5A5A4723 |
| 98 | `805C7745` | — | 1 | — | 6D99366C |
| 99 | `80F0F5A6` | — | 1 | — | DDAE4E43 |
| 100 | `90F1BBCC` | University Path | 1 | — | B718847E |
| 101 | `9F2E2D79` | — | 1 | — | 82B167CA |
| 102 | `B57DB3F7` | — | 1 | — | FE4F2761 |
| 103 | `B95FD0E2` | — | 1 | — | 459AF036 |
| 104 | `C38AB149` | — | 1 | — | C40DAE0F |
| 105 | `DCE9D288` | — | 1 | — | 7783EC34 |
| 106 | `E147D100` | — | 1 | — | 7783EC34 |
| 107 | `E44F3647` | — | 1 | — | 46AE3EA0 |
| 108 | `E8DAF4B1` | — | 1 | — | 15AE0A8A |
| 109 | `E958A785` | — | 1 | — | 3155D351 |
| 110 | `F70B508F` | — | 1 | — | 012F4CE1 |
| 111 | `F7966A46` | — | 1 | — | 6F140E38 |
