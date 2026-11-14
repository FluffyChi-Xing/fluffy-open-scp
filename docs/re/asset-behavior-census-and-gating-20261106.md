# 资产行为普查与门控机制定案（探针 + PDB 反编译源码交叉）

日期：2026-11-06。证据三源：

1. **普查探针** `probes/property/behavior_census.rs`（v3）：扫描
   `SimCity_Game/Graphics/DLC0/DataEP1.package` + EcoGame 脚本库
   `SimCity-Scripts_287520926.package`，产出 `tmp/behavior-census-20261106/census.json`
   （1214 Unit / 2027 Lot / 476 Menu2 / 1308 bundle / 1047 sink / 254 模板 /
   3131 种资源 bin / 1 主表）。探针与小学 `C264A268` 全字段对拍通过
   （parent 链、43 bundle、179 资源 bin、路网 [Global+First, Local+None×2]、
   board=教育容器全部命中既有结论）。
2. **dev 反编译源码树** `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\`
   （门控/降级链取证，注意 dump 中 hash 常量多为十进制书写）。
3. 既有文档 `building-eco-chain-and-s17-diagnosis-20261106.md`（s17–s20 个案链）。

本文回答四个问题：行为族全量普查；行为是否可独立存在；property 的门控/合法性
校验；s 系列"无水无电/菜单乱码"降级的再定案。

## 0. 结论速览

1. **"行为"的正确抽象不是 named 属性，是 bundle 库调用。** 引擎消费的是三列
   通用资源表（`0x07E0AD23` 资源 / `0x07E0AD24` 容量 / `0x08635E12` 值）、
   sink 表（`0x0A2725CE/CF` → point 组 `40E0C500` 条目）和规则引用
   （`0x08080D58..`、`0x08327D48(+0..20)`）——它们都住在 bundle（组
   `40E0C700`）里，经四级启用数组 `0x0A0209A2..A5`（并集）− 排除表
   `0x0A0209B2` 挂到单位。**水电三资源（Power/Water/Garbage）的 bin 行在
   全部 1214 个单位里没有任何一个写在自身 core 表上**（bin_core=0；只有
   33 个单位带任意 core bin，且都是非水电资源），水电消耗全部经标准
   bundle 继承而来。
2. **named 属性 hash（kPropPowerConsumer 族、水/垃圾/岗位/地价/吸引力/税…）
   在反编译源码中没有 native 读者**——唯一实证读者是电力看板
   `BuildTotalsJSValue`（UI 统计）。给资产"写属性"而不挂 bundle，写的是
   看板文案，不是行为。
3. **行为独立性的判定**（普查 §3）：数值型行为（水电垃圾消耗、水电生产、
   岗位/学生/游客流量）**数据上全部依赖 bundle**，不能独立存在；展示型字段
   （造价/类别/维护费/解锁）引擎直读、可独立；land value / attraction 有
   10/47 个 0-bundle 样例，但运行期效果经规则间接落地（引擎不直读）。
4. **无水无电的再定案**：s17 起 Unit 属性"逐字段全等"克隆就已携带完整行为
   数据链（43 bundle 含电/水/垃圾 bin + connected sink + 规则，全部从
   Toy 层 `2A25CF66` 继承）。**降级从来不是 property 数据问题**，而是运行时
   门控层。且普查反证了 R2 的因果链：463 个耗电单位中 369 个（80%）lot
   路网为纯 Local+None、915 个单位 lot 无任何 path 条目——**roadConnect
   与水电接入无关**（Q11 代码定案：Local 路径实现完全忽略 roadConnect；
   该标志只被 GlobalConnector 的 30m 吸附消费，属路径几何层）。**水电
   接入的真正机制是 pipe 几何邻接**：建筑 bbox 的 local +Y 面（正面）
   中点为门前点，在道路 segment 集合里按 sink 条目 mMaxDist 找最近段
   绑定（§4.3）。s17–s22 的头号嫌疑因此收敛为**几何/朝向**：克隆 lot 的
   footprint/bbox 是小学的近 3 倍（§5.2 既有 diff），若正面(+Y)背对道路
   或门前点超出 mMaxDist，attach 永不成立 → SinkCapacity(e) 恒 0 →
   无水无电 → FailUnit 降级链。
5. **菜单乱码**：标题链 = Menu2 `0x0A09F5FA`(text[])→ locale 表；miss 时 UI
   显示 placeholder 串或 JS null（`BuildTextPropertyJSValue` 的 placeholder
   字段），**不会崩溃、不会拒绝**。原版 455/476 菜单共用官方表
   `6C969DEE`。菜单族字段（parent/kCategoryID）在代码中零特判——s18 标题
   变正常与 parent 修复无因果关系，真正生效的更可能是 locale 表注册链。
6. **门控总纲**：引擎几乎没有"校验失败→报错"路径，全部是**静默降级**
   （默认 0 / null / placeholder / 跳过 / 丢弃）。唯一"拒绝"粒度：
   工具 FillFromProps 返 false（静默丢工具）、二进制属性表解析失败（单条目
   拒绝）、AddPackageIfValid 签名/DRM 失败（整包拒绝）。
7. **path 有四层正交语义（§4.7），与水电无关**：①材质/宽度（视觉车道
   类型）②**filter 成员**（PathEntry `0x0D6498E8` 声明所属 filter ID；
   车辆 `0x0EB22775` 选 filter，256 位 bitset，缺省全通；行人载体
   `418A18B1`→`E0CD3122`(Pedestrians) 即"只能走人行道"）③scope（Local=地块内 pathSet /
   Global=全城+跨城路径网络，由 AcceptedScope 分派）④roadConnect（端点
   30m 吸附路网，仅 GlobalConnector 消费）。"只有校车进那条路"是
   **sink/目的地**语义，不是 path 排他。
8. **空表与变体必须从主统计中剔除**（用户观察）：9.1% 的驻留表是 4 字节
   空表（资源组 40E0C100 占 1121 张），123/1214 个 Unit 是无 bundle/
   无 bin/无菜单的结构变体（RCI 族 2D7BDB01 占 65）；剔除后行为族统计
   几乎不变，§3 结论稳健。`0x02A907B5/B6` 是**模型挂件 effects**，
   不是建造阶段（旧文档推断作废）。

## 1. 数据分布：哪个包装什么

| 组 | 语义 | scripts | game | graphics | dlc0 | ep1 |
|---|---|---:|---:|---:|---:|---:|
| 40E0C000 | Unit（模拟身份） | **1214** | 0 | 0 | 0 | 0 |
| 48E0C000 | 模板层 | **254** | 0 | 0 | 0 | 0 |
| 40E0C700 | bundle（bin+sink+规则） | **1308** | 0 | 0 | 0 | 0 |
| 40E0C400 | PathEntry | **154** | 0 | 0 | 0 | 0 |
| 40E0C500 | point（含 1047 sink 条目） | **1236** | 0 | 0 | 0 | 0 |
| 40800200 | 主表（唯一，版本 287520926） | **1** | 0 | 0 | 0 | 0 |
| 40E1C000 | Lot（表现） | 0 | **1928** | 99 | 0 | 0* |
| 09878A01 | Menu2 | 0 | **457** | 19 | 0 | 0 |

\* EP1 的 lot 变体住独立命名空间（`42E1C000`×976、`41E1C000`×708 等
`4x/6x` 前缀组）；DLC0 同理（`4xF0xxxx`）。**DLC/EP1 不携带任何模拟记录**
——它们的行为差异全部实现在各自 Scripts 包（`SimCityDLCEP1-Scripts_*` 等，
本普查未扫，主库已含 1214 单位的全集）。

主表 `622B9CD7` 的扫描组声明值全部为 **`FFFFFFFF` 通配**（unit/resource/
map/zone/path/point/transport/bundle 九组皆然）——组 ID 是代码常量
（`AddEntitiesFromGroupIDs` 0x61EBA0），主表只声明"启用"，这修正了此前
"主表声明扫描组"的表述。

## 2. 行为的数据模型（三层，全部住在 bundle 生态）

一个"行为"（如"消耗 10kW 电"）= 以下数据的组合，**除注明外都经 bundle**：

| 层 | 属性 | 内容 | 证据 |
|---|---|---|---|
| 资源 bin | `0x07E0AD23`(Key[]) / `0x07E0AD24`(int[]) / `0x08635E12`(int[]) | bin 的资源 ID / 容量 / 初始值三列平行表 | `GB::FillFromProps` 0x64C280 kResourceBinTable；小学 bundle 6C042367 23 行 |
| sink 绑定 | bundle `0x0A2725CE`(sinkEntryID) / `0x0A2725CF`(unitRuleID) / `0x0AF38802`(keyResource) | bundle→sink 条目/规则/key 资源三列绑定表；引用的 sink 条目不存在→**该行静默丢弃**；空数组合法 | `GB/cEcoUnitEntry/AddBundle.c` 0x6490B0 第二张表 |
| sink 条目本体 | 组 40E0C500（主表 sink 组声明 0x0A30A557）：`0x0A810F28` 资源、`0x0DA7921B` connected、`0x0B555629` invert、`0x0E7A94FF`/`0x0E7D5F1F` modifier、`0x0A810F29`+`0x0A810F2A` 测试表（按等级 int+bool，如电 sink 6DAA42EB=[11,13,15,22,19,60]/[F,F,F,F,T,F]）、UI 颜色 | **sink 条目自身没有消费量字段**——消费量三来源：①unit bin 的 capacity−amount（主来源）②amountModifier 偏移 ③无 key bin 保底常量 10 | `AddBaseEntitiesFromGroupIDs.c:139`（sink 组装载）、`cEcoSinkEntry::FillFromProps` 0x64B2C0、`SinkCapacity.c` 末段；普查 1047 sink/150 connected |
| 规则 | `0x08080D58..5A`(Key[]) / `0x08327D48(+0..20)`(Key[]) | 常态规则引用 / 事件规则槽（21 槽，首槽 +1 起） | `AddBundle` 0x6490B0；`0x08080D58` 可为空数组 |

**规则怎样读取 Property**：常规资源操作使用资源 ID、输入输出量和运行时 bin。
`CanRunRule` 支持从 Unit Property 或 SimProperties 读取整数型应用上限，
规则专用化也能用指定的 PropOp 从属性改写规则字段。具体字段必须沿规则引用
核对。水电等 named 属性的 UI 用途与 bundle 的运行时资源操作应分别追踪。
见 [UnitRule 能力边界](./er2-unit-rule-capabilities-20261113.md#执行能力和依赖)。

**挂接机制**：单位合并属性表（parent 链已在 PropertyManager 装载期展平）
读四级启用数组 `0x0A0209A2/A3/A4/A5`（`cEcoUnitEntry::FillFromProps.c:261-275`，
j+167905698 循环）取并集，减排除表 `0x0A0209B2`；单位自身表先无条件作为
"core bundle"过一遍 AddBundle（但原版 core 全部不带 bin 行）。

**水电标准件 bundle**（编辑器应直接引用，不要重造）：

| 行为 | bundle | 引用数 | 带的 sink | 备注 |
|---|---|---:|---|---|
| 耗电 | `48939D96` / `19E5DB54` / `EA2B32BB` | 123/78/235 | 6DAA42EB(connected) / 6DAA42EB / 无 sink | EA2B32BB 疑为 RCI/区划通道 |
| 耗水 | `825B4CA4` / `D6068BEA` / `4922A3A5` | 121/78/232 | 562A687D(connected) / 同 / 无 | |
| 产垃圾 | `D7C62954` | 198 | 自引用 sink(invert) | |
| 公共底座 | `44F5CF6C`(514 引用)、`4A42F50E`(514，3+3 sink)、`1B26B735`、`8086D38A`、`FF3A51ED`… | — | — | Toy 基类 `F98BCB35` 启用 |

**模板层级承载**（普查 254 模板，被引用≥3 的行为相关层）：

```
593B61D2 (空根)
└─ 9EC474FF  Unit 基类 [613 引用] category+jobs 默认
   └─ F98BCB35  Toy 基类 [448] +水电垃圾消费默认 + 公共 bundle 10 项
      ├─ C34674D9 [301] +水电
      └─ 2A25CF66  Toy 覆盖层 [137] +水电垃圾 + 电48939D96/水825B4CA4/垃圾D7C62954
         ├─ 7E4B99E5  教育模板 [5] +4 教育规则 bundle（水电来自本层继承）
         ├─ 4E01E311  地标模板 [19] +attraction/cost/tourism/unlock/quota
         ├─ F1CFC5B0  公园全家桶 [51] 11 行为族
         └─ 34CE7E14/16/17  公园变体层（自带 bundle 265FB8DD..F/C0672F54..57）
```

小学有效 bundle = 43（自有 A5 级 4 个教育 bundle + 模板链继承 39 个），
有效资源 bin 179 种（含电 10 / 水 10 / 垃圾 20）。**此前文档"小学 4 个
bundle"是 A5 单级读数，实际生效集是 43**——克隆 Unit 时"逐字段全等"已经
覆盖了启用数组，因此 s17–s22 的行为数据链自始完整。

**named 属性的真实地位**：`0x0AFB9882/0x0B88904E/0x0C09DAA0`（电）、
`0x0C09DA65/5A`（水）、`0x0C216562`（垃圾）、`0x0A7917C3..C8`（岗位）、
`0x0E585AE0..B01`（地价）、`0x0E542F39`（吸引力）、`0x0A7917C4`（税）在
src/include/types/docs 全树按十进制/hex 双变体搜索均无 native 读者；唯一
实证是 `BuildTotalsJSValue`（电力看板 UI 读 `0x0AFB9882/83/87`+`0x0B88904E`
做分母分子，miss 一律按 0 统计）。它们是模板层的展示默认值 + 规则数据
可能引用的参数面，不是引擎行为开关。

### 2.1 空表与"非主 property"（用户观察 1、2 的普查化）

用户 2026-11-06 指出：包内存在大量**仅 4 字节的 property**（计数头 0 条
属性），且每栋建筑有大量**结构极简、无行为消费**的条目，它们不是主
property。普查 v4 已把这两类从主统计中分离（判据写死在探针
`classify_unit`）：

| 类别 | 判据 | 数量 |
|---|---|---:|
| 空表 | 叶级属性 = 0（文件 4 字节） | **1422 / 15556 驻留表 = 9.1%** |
| 微表 | 叶级属性 1–3 | 6447 |
| Unit: empty | 叶级属性 0 | 0 |
| Unit: variant | 无 bundle、无资源 bin、无菜单、叶级属性 ≤12 | **123 / 1214** |
| Unit: primary | 其余 | **1091 / 1214** |

- 空表高度集中：**资源组 `40E0C100`（5799 张表中 1121 空 + 4584 微表
  = 98% ≤3 属性）**、表现层 nonSim point 组 `40E1C500`（graphics 130 /
  game 98 空）、transport nonSim 组 `40E1C600`（game 28 空）。
- 变体高度集中：**RCI 族 `2D7BDB01` 65 个**，其余为无 parent 的 22 个、
  `3AF27108` 11、`051EBCAA` 7 等；叶级属性多为 3–4 条。
- **排除变体后行为族统计几乎不变**（power 528→527、water 526→526、
  jobs 623→612、category 631→620），说明变体确实不承载行为——§3 的
  结论对"只看主 property"成立。小学 `C264A268` = primary（74 叶级属性 /
  43 有效 bundle）。

## 3. 行为普查表与独立性判定

1214 单位、17 个 named 行为族的普查（own=叶级声明；bund0=零 bundle 仍带此族；
menu-=无菜单入口；lotGF=lot 含 Global+First 路网）：

| 行为族 | 总数 | own | inherit | bund0 | menu- | 判定 |
|---|---:|---:|---:|---:|---:|---|
| power_consumer | 528 | 151 | 377 | 1 | 119 | bin 层 463/528 经 bundle；**不可独立** |
| water_consumer | 526 | 152 | 374 | 0 | 117 | bin 层 457 经 bundle；**不可独立** |
| garbage_producer | 525 | 159 | 366 | 0 | 116 | bin 层仅 200；326 个"只有 named 无 bin"（named 装饰性的直接证据） |
| jobs | 623 | 309 | 314 | 24 | 203 | 岗位流量经 jobs bundle（F236BB93/D07C5A04）；195 个 0-bundle 样例疑为区划系统读 named（native 无读者，未定案） |
| tax | 82 | 7 | 75 | 0 | 77 | 商业族模板继承；规则驱动 |
| land_value | 121 | 46 | 75 | 10 | 3 | 10 个 0-bundle 叶级样例 = 数据可独立；运行效果经规则 |
| attraction | 231 | 47 | 184 | 47 | 79 | 同上（47 个 0-bundle 样例） |
| capacity | 97 | 8 | 89 | 0 | 1 | UI 展示（详情面板容量） |
| maintenance/cost | 482/488 | 370/357 | — | 12/14 | — | 引擎直读（cToolPlop::FillFromProps 造价 miss→默认，不阻断） |
| category | 631 | 118 | 513 | — | — | 引擎直读 |
| secondary_units | 32 | 31 | 1 | 0 | 4 | 引擎直读（CreateSecondaryUnits） |
| unlock/quota/module_limit | 236/76/127 | — | — | — | — | 数据驱动门控字段 |

**资源 bin 空间**：单位有效 bins 共 **3131 种资源 ID**（这就是"行为"的
真实清单域）。named 族只是其 UI 投影。高频资源如 `14984C68..6B`（567 单位，
四联，疑为 W1/W2/W3 岗位+合计）、`E74AD045`(564)、`44F5CF6C`(545，即同名
bundle带的 bin)…完整表在 census.json `resource_census`。

**菜单独立性**：476 Menu2（game 457 + graphics 19），menuUnit 指向 430 个
单位——**784/1214 单位没有菜单入口照样有完整行为**（RCI/内部单位），菜单
与行为正交。菜单侧：parent 直方图 `AF042E9A`(plop 根)=329、`F970A822`(公园
根)=30、`12E3214B`(道路)=17…；kCategoryID（Key 类型）`C710B6E9`=67、
`F2FF41C8`=25、**`8E8B4E10`(电力资源 ID 当类别)=12**；标题表 `6C969DEE`
=455、`FFFFFFFF`=7；模块挂载三件套 33 个。

**路网编码普查**（2027 lot，(scope,connect) 组合直方图）：
`Local+None` 537、`Global+First` 32、`Global+Last` 13、`Global+FirstAndLast` 6、
`Global+None` 9、`Local+First` 3、`Local+Last` 1。**连接类条目（Global×First/
Last/FirstAndLast）合计仅 51 条**。耗电单位 lot：local/none 369、global_first
18、global_any 5、无 lot 71。→ roadConnect 的 First/Last 语义是**路径端点
与道路的衔接方式**（车辆进出车道），不是电网/水网开关。

## 4. 门控与降级链（源码定案）

### 4.1 属性库装载（cPropertyManager / AddPackageIfValid）

- 二进制属性表解析失败 → **该条目整表拒绝**，不波及同库（ReadResource.c
  0x4AC980）；文本表坏行 → 静默跳行。
- AddPackageIfValid（retail RVA 5EC40）：packageInfo 记录 0 条→按入参
  priority 注册；恰 1 条→验签；**>1 条→整包拒绝**。签名/entitlements/
  bless 失败→整包拒绝。普通最小 overlay 不需要官方 metadata（既有结论维持）。
- **parent 断链**（父 (group,instance) 不存在）→ 子表保持无 parent，静默，
  无祖父回退（UpdatePropertyListParent.c 0x4A7E50）。注意 parent Key 缺
  group 时继承自己的 group。普查发现 12 个链断单位（内部资源单位，无实害）。

### 4.2 单位接受与特化（全部静默默认值型）

- `AddEntitiesFromGroupIDs`：对代码常量组（unit=40E0C000 等）枚举**全部
  属性库**（含普通包，priority 降序首命中）建 entry；唯一接受条件是
  GetPropertyList 成功。组内缺属性一律走默认值。
- `cEcoUnitEntry::FillFromProps`（0x64A3A0）：bin 行的资源 ID 必须已在
  资源组注册（`IndexFromResourceID` ≥0），否则**该行整体丢弃**；amount
  钳到 capacity；`amount = min(max(0,amount),capacity)` 终检。bundle
  启用/排除缺失→不加不排，entry 照建。
- 放置特化 `CreateUnit`（0x60D980）→ `StartCreatingUnit`：
  `CanCreateUnit_2` 查事件规则 0 型 MinApply 资源门槛；`CanRunEventRules(type0)`
  失败→**释放槽位返回 -3**（建筑放不下/秒拆的来源）；带 `0x08635E16` 的
  Unit 只建 SecondaryUnits 返回 -7。

### 4.3 水电消费的运行时判决：SinkCapacity 五分支（0x616BF0）

```c
if ( !mIsSimulating )                          return 0;  // (a) 未开模拟
if ( unitControlFlagTests 不匹配 )             return 0;  // (b) 单元控制旗
if ( 全局控制旗不匹配 )                         return 0;  // (c) 全局旗
while ( 单元 bin 测试失败 )                    return 0;  // (d) bin 条件
SumConnectedCapacities(FindConnectedUnits(slot));          // (e) 连通网络总量
if ( amount == (invert ? capacity : 0) )       return 0;  //     未连接
...
if ( mKeyBinIndex < 0 )                        return 10; // 无 key bin 保底容量 10
```

- **没有任何分支"拒绝单位"**——降级 = 容量归零（消费为 0、断水断电图标、
  percent-fill prop 清零）。s19 的"车库 prop 暂停解除后被移除"正是这条
  降级链的 FailUnit 表现，不是数据缺失。
- 消费量三来源（Q8 定案）：bin 差值（主）+ `0x0E7D5F1F` modifier 偏移 +
  无 key bin 保底 10。sink 装载链：`AddBaseEntitiesFromGroupIDs`(主表
  sink 组 0x0A30A557) → `cEcoSinkEntry::FillFromProps`；挂接链：
  `AddBundle` 读 bundle 的 `0x0A2725CE/CF/0x0AF38802` 三列；放置链：
  `CreateUnit → AddUnitSinks`（首次赋 mCapacity/mMaxCapacity，并通知
  GameHandler/TransportHandler——SC 管网层挂载点）；重算链：
  `UpdateUnitSinkCapacities` ← ConnectUnits/DisconnectUnits/
  SetIsSimulating/DeliverTransaction。
- **连接图（Q9/Q11 定案）——与水电无关的特例机制**：`mConnectedUnits`
  唯一运行时写入口 = `GB::ConnectUnits`（0x5FED40），调用方穷尽：
  CreateSecondaryUnits（模块连主楼）、delta 回放、存档回填、
  `cGraphicsGame::Update` 消费 `mUnitsToConnect` 队列。队列入队者仅两个：
  编辑器 ConnectUnitsHelper 与 `UnitCreate` 读 lot 侧 `0x0CE4B739`
  可连 unit ID 列表 → `FindClosestUnitMatchingIDs` 最近匹配。普查实测：
  带 `0x0CE4B739` 的 lot 仅几十条且全部是**基础设施单位**（无菜单、
  指向道路 unit `8C214A11`+`4A153DA0..7`、塔/公园 unit）——显式
  unit↔unit 直连语义，不是水电通道。小学 lot 全链没有它。
- **水电接入的真正机制 = pipe 几何邻接 attach（Q11 定案）**：
  `AddUnitSinks`（放置完成）→ 通知 TransportHandler →
  `cTransportPipe::AddSink`（0x7704D0，入池待绑，**无任何资格检查**）→
  **`GetFrontRoadAttachPoint`（0x7D78E0）：取建筑 bbox 的 local +Y 面
  （正面）中点为"门前点"** → `cPool::FindSinkAttachment`（0x76E510）在
  **道路 pathSet 的 segment 集合**里按 sink 条目的 `mMaxDist` 找最近
  segment 绑定（缓存，挪动 >0.33m 才重算）→ ERZ 规则的 `mAgentTests`
  （sinkEntryID+范围）驱动 pipe pool 送达 → `DeliverTransaction` →
  `UpdateUnitSinkCapacities` 写 bin。**全程不触碰 lot path、roadConnect、
  0x0CE4B739、mConnectedUnits**。因此标准临路建筑有水有电的充要条件：
  ① 门前点（**模型/lot 的 +Y 正面朝向**）落在道路 segment 的 mMaxDist
  内；② sink 三件套（sink 条目 + bundle `0x0A2725CE` 引用 + 资源表三列）；
  ③ 投递规则（mAgentTests，随标准 bundle 继承）。
- `FindConnectedUnits` 是**单跳**展开；只服务于 connected 型 sink 的
  直连资源测试（如塔间互联）。
- roadConnect 的唯一消费者 = `cUnitPathCreateGlobalConnector::CreatePath`
  （30m 吸附，路径几何层）；`cUnitPathCreationLocal::CreatePath` **完全
  忽略 roadConnect**；`SC::PrepareForConnection` 本体只是吸附+切分+修
  交叉口，无 ConnectUnits——路径图与 eco 连接图、管网三层互相独立。
- `cPipeSinkEntry::FillFromProps`（0x76B610，SC 表现层）读的 maxDist
  （`0x0B695F0C`）/ sinkAttachInterval（`0x0B73EC90`）正是 pipe 绑定
  的半径参数——**这一层不是装饰参数，是水电接入半径**。

### 4.4 菜单/工具门控（静默三连）

| 门 | 条件 | 降级 |
|---|---|---|
| 工具创建 | Menu2 缺 `0x08F672C7`(toolType) | 工具不创建，静默 |
| 工具类型 | toolType 未知 ID | `CreateTool` 返 nullptr |
| 目标单位 | `0x0975695F` 指向不存在的 unit entry | `cTool::FillFromProps` 返 false → **整个工具丢弃**（菜单不出现） |
| 菜单可见 | `0x0DB9FC63` uiCategories 缺失/空 | `InUIToolCategory` 返 0 → **不进任何调色板**（工具仍可用） |
| 家族 | parent=`AF042E9A`/`F970A822`… | **代码零特判**，纯数据侧菜单树键 |

`0x0975695E`(kCategoryID) 在代码中不存在——它是数据侧字段（s18 补写它
无害但也无门控作用）。

### 4.5 文本门控与"乱码"的真实来源

- 标题/描述：`cTool::FillFromProps` 读 `0x0A09F5FA/FB`（**text[] 数组**），
  miss → 保持空 cTextProperty。
- 查找 miss 的显示：`BuildTextPropertyJSValue` 把 {tableID, stringID,
  **placeholder**} 交给 JS——placeholder 就是占位文案；`BuildToolPickerInfoJSValue`
  miss 时 JS 收到 **null**。`cString::Load` 取不到表 → 默认串。
- locale 注册：`RegisterLocaleDatabases`（0x4A61D0）只扫 `locale\<lang>\*.package`
  + 内嵌（type 06EFC62A，instance=FNV1(语言名)）+ BaseLocale(-1000)，并
  **总是递归补注册 en-us**（miss 回退链：本语言→en-us→placeholder）。
  坏表按文件粒度跳过，绝不整库中止。**无 BOM/编码校验逻辑**。
- 详情面板 `~xxx~` 占位符 = JS 数据绑定收到 null（native 侧
  `Build*JSValue` 单查询 null 语义；"整批中断"聚合逻辑在 JS 层，不在
  native dump 内——既有 fire-station 结论与此一致）。

### 4.6 无 hash→名字典

源码树不存在属性名字典；名字→ID 映射是运行时从文本表 `property <name>
<id> <type>` 行注册的（ParseAndAddProperty.c）。资源 ID（Power=8E8B4E10
仅 2 处有符号常量；Water/Garbage 全树无命名）也由数据注册。普查中 3131
资源 ID 的命名需要走 SimCityData/Config 文本表或运行时 hook（后续工作）。

### 4.7 路径（path）的四层语义与"谁能走"的判定（用户观察 3）

用户 2026-11-06 指出：跑道/港口/建筑都有 path，小学的 path 既含校车路径
又含两条行人路径，且小车可开去隔壁城——path 必然有多重语义。源码+普查
定案，path 有**四个正交维度**，互不替代：

| 维度 | 载体 | 语义 | 证据 |
|---|---|---|---|
| ① 材质/宽度（视觉类型） | PathEntry 的 parent 模板（组 `48E0C400`）链到 lot 材质族：`0x0B762710`=材质 SPID、`0x0B7E3A39`=lot 宽度、`0x0B7E3A3B`=lot 最小面积 | 决定**路面外观**（车行道 vs 人行道）与宽度（`PathEntryFromIndex(...)->mWidth`，属性 `0x09E6794C`） | `SC::cNewLotData::FillFromProps`；小学 3 条 path = 1×`84139BA1`（车道材质族）+ 2×`DE38BA78`（人行道族） |
| ② **谁能走（agent 过滤）** | PathEntry 的 `0x0D6498E8`(Key[]) = 本路径所属 **path filter ID 列表**；transport 条目 nonSim 的 `0x0EB22775` = 选用哪个 filter；filter 本体是 **256 位 bitset** | 装载时对每个 PathEntry 索引 `idx`：`filter.mActivePathEntries[idx>>5] \|= 1 << (idx&31)`。agent 找路时（`FindPathsInSphere`）若 `idx>=256` 或对应位为 0 → **该 path 对它不可见**。transport 缺省无 filter = **全 1（所有 path 都能走）** | `GB::cEcoGameScripts::AddBaseEntitiesFromGroupIDs`（filter 构建段）、`GB::cEcoTransportEntry::FillFromProps`（末段）、`GB::GetPathFilter`（找不到 filter ID → **全 0 = 什么路都不能走**）、`GB::FindPathsInSphere`（位测试） |
| ③ scope（Local/Global） | PathEntry 的 `0x0D9A4E54`；创建器 `cUnitPathCreationObj::AcceptedScope` = `mPathScope==All \|\| ==pathScope` | 决定**这条 path 进哪个 path 网络**：Local 创建器（`cUnitPathCreationLocal`）只 `AddPoints+AddPath` 进**本 unit 的 pathSet**（小人走到门口、地块内交通）；GlobalConnector（`cUnitPathCreateGlobalConnector`）进**全城/区域 path 网络**（车辆跨城，如小学校车开去邻城），并在 `roadConnect=First/FirstAndLast` 时 30m 内吸附已有路网 segment | `SC::CreatePathsFromProps`、`cUnitPathCreationLocal::CreatePath`（**完全忽略 roadConnect**）、`cUnitPathCreateGlobalConnector::CreatePath` |

**实测数据**：
- 小学：`7C9C4FFF`（校车引道）→ filter `C37D661D`（**School busses**）；`03106C2E` → `[979CC22E,3713EF31]`（**Cars And Crap**）；`418A18B1` → `E0CD3122`（**Pedestrians**）。scope/connect = `[Global+First, Local+None, Local+None]`。
- **用户推论被数据证实**：`CreatePathsFromProps` 里 path 表/scope/connect 三列
  逐下标平行（`pPathEntries[j]`/`pPathEntryScope[j]`/`pPathEntryRoadConnect[j]`），
  所以**第 0 条 = 校车引道 7C9C4FFF = Global+First**（进全城/区域路网，可跨城），
  **第 1/2 条 = 本地通道 = Local+None**（`03106C2E`=Cars And Crap、`418A18B1`=Pedestrians）。
  注册表命名修正：`03106C2E` 不是人行道，行人网络是 `418A18B1`（filter `E0CD3122`）。
- 行人网络 filter = `E0CD3122`（Pedestrians）：原版 `418A18B1` 等声明它；
  `03106C2E` 声明的是 `979CC22E`（**Cars And Crap**，污水/杂物网络——载体 `EB91A84C` 的 bin 为 sewageimported/exported）。
  2026-11-06 注册表命名修正：此前把 `03106C2E` 当人行道是错的。
- `C37D661D` 同时被**大量道路 PathEntry**声明（`359C46BB`/`AD5600A3`/`F1E94198`/`173E62C1`/`06191B6F`…），是**通用车辆类 filter**，并非校车专属；Game 包里只有 7 个 transport nonSim 声明 filter（行人、若干车道类），其余车辆用缺省全通。
- 结论：**"只有校车能进那条 path"不是 path 本身排他**——校车之所以驶入，是它的 **sink/目的地**在该点（transport pipe 投递），其他车没有该目的地。path filter 是"路面级"的通行许可（行人/车辆/特殊载体），sink/agent test 是"目的地级"的到达许可；两者叠加。

**path 与水电的关系（用户判断确认）**：**行为层没有必然联系**（水电数值
接入是 pipe 几何邻接，§4.3），但**网络层是同一套 path 机制**：水/电/污水/
垃圾等 agent 各自是一个 transport 载体，携带自己的 filter，只走声明了该
filter 的 path。实测 8 个载体（`40E1C600` nonSim + EP1 `61F0C600`）：

| 载体 | filter | 资源 bin | 判定 |
|---|---|---|---|
| `EB91A84C` | `979CC22E` | `362C3FA5/9F4E3E6C`，速度 2.1 | 车辆/杂物（注册表名 **Cars And Crap**；**非行人**，2026-11-06 修正） |
| `02957CE3` | `C0D6DEEE` | `9E3C3DFA`=**水** + `7B5637C5` | **水 agent** |
| `E99F8B9D` | `5DFAC324` | `8E8B4E10`=**电** + `4ACE81A5` | **电 agent** |
| `E2AD993F` | `C4AAF79A` | `F4FAC676`，棕色 | 污水/垃圾类 |
| `3FF2747F` | `69E76B61` | 4 个 bin | 服务车类 |
| `EBCD02ED` | `799F1CA5` | 3 个 bin | 公交/服务类 |
| `36BA75FA` | `A5AA50CC` | 4 个 bin（量 1/3/1） | 服务车类 |
| EP1 `C3003063` | `5DFAC324` | — | 电 agent（EP1 变体） |

即"水电管网"= 声明了水/电 filter 的 path 集合；`0x0D6498E8` 是"本路属于
哪些 agent 网络"的声明，载体 `0x0EB22775` 是"我只走哪个网络"的选择；
**缺省（无 filter）= 全通**，找不到 filter ID = 全禁。

**RCI / plop 路径普查（`probes/path/path_census.rs`，v1）**：

| 类别 | 总数 | 无 path | 有人行道 | 有车辆 entry | 有 Global scope |
|---|---:|---:|---:|---:|---:|
| plop（有菜单） | 430 | 266 (62%) | 82 | **2** | 27 |
| RCI（无菜单） | 784 | **763 (97.3%)** | 18 | **0** | 3 |

- **用户观察实锤**：RCI 建筑绝大多数**根本没有 path**（97.3%）；有 path 的
  21 个全是 `418A18B1`（**Pedestrians**，filter `E0CD3122`）/`03106C2E`（Cars And Crap）
  （filter `E0CD3122`）一类，**零车辆 entry**、仅 3 个带 Global scope。
  连真正的区划族 `2D7BDB01`（108 个变体）也是 **全部 0 条 path**。
- plop 里只有 **2 个单位**引用全游戏唯一的车辆 entry `7C9C4FFF`
  （小学及其同类），这正是"车道"稀缺的原因。
- path entry 分类（sim 组 154 + nonSim 镜像 108）：`pedestrian` 84、
  `other_filter` 69、`no_filter` 108（**nonSim 镜像全数**，无 filter）、
  `vehicle` **1**。挤出/交叉口模板（`0x09532375/0x75152C4A`）不是独立
  类别而是条目的附加属性（`7C9C4FFF` 同时是车道 + 有挤出模板）。
- **索引空间定案**：`AddBaseEntitiesFromGroupIDs` 的 path 循环按
  `pathGroups`（sim）每项建一个 index，`pathGroupsNS` 仅用于给同一条目
  挂 `mNonSimProps` 镜像——**索引空间 = sim 组 154 条**，余量 102
  （<256），新建 path entry 可行。

## 5. s17–s22"降级"问题再定案

| 症状 | 旧归因 | 普查+源码再定案 |
|---|---|---|
| 无水无电 | R2：roadConnect 编码写反→未接入 | **数据链自 s17 起完整**（43 bundle 继承齐全，含 bin+connected sink+规则）。R2 编码 bug 真实存在但**不是管网因果**（80% 原版耗电单位 lot 为 Local+None；Local 路径实现忽略 roadConnect；PrepareForConnection 纯路径几何）。**Q11 定案：水电=pipe 几何邻接**——门前点（bbox +Y 正面中点）→ 道路 segment mMaxDist 内绑定（`GetFrontRoadAttachPoint` 0x7D78E0 + `FindSinkAttachment` 0x76E510）。s 系列头号嫌疑=**几何/朝向**：克隆 lot 范围盒是小学近 3 倍（−96,−48..96,48 vs −34,−25..34,16），正面(+Y)背路或门前点超 mMaxDist → attach 永不成立。判别实验：①对照小学/s22 的 `GetFrontRoadAttachPoint` 输出（hook dev 0x7D78E0）；②把 s22 放置在紧贴道路且正面朝路的位置测水电；③量 footprint 与 sink mMaxDist（`0x0B695F0C`，缺省用 cPipeSinkEntry 默认） |
| 菜单乱码（s17 观感） | 绑定批量中断 or 标题 miss（未定案） | 标题链与菜单族字段无关（parent/kCategory 零代码特判）。miss 显示 = placeholder/JS null。**主嫌：locale 表注册链**——RegisterLocaleDatabases 只扫 `locale\<lang>\*.package`；SimCityData 根目录裸包里的 `0A98EAF0` 表是否被注册取决于三定律实验当时的部署位置（判别：把自有表放进 `SimCityData/Locale/<lang>/` 或直接引用官方表 `6C969DEE` 新增 id）。`~xxx~` 面板占位符则是查询 null（模拟侧 bins/connectedUnits 空），与标题链独立 |
| 车库 prop 消失 | percent-fill prop 清零 | pipe attach 不成立 → 容量 0 → percent-fill prop 清零 + FailUnit 降级链表现——与无水无电同根（几何/朝向），非数据问题 |
| 人口地图暗色 | board 实例错（s20 已修） | 维持：`0x0D7970ED` 看板容器是 UI 侧索引（普查：board 值含资源 ID 8E8B4E10=17 个 lot，教育 6A1785B4=2 个，公园 D0621548 等） |

### 5.1 s14（GOLDEN）专项诊断：行为正常而 path 失效的原因

s14 = `000_OpenSCP_University_s14.package`（sha256 `024f8631…`，用户终审
GOLDEN）。**行为正常**：Unit `BE18DA2D` parent = 教育模板 `7E4B99E5`，
经 `2A25CF66` 继承电 `48939D96`/水 `825B4CA4`/垃圾 `D7C62954` 三件套；
排除表 `0x0A0209B2` 只排掉 `30645625/1B946073/E0C02E1F`，不碰水电；
叶子只覆盖数值（电 72/9、水 3/9、垃圾 1、岗位 7、吸引 1）。

**path 失效**（逐字段定案）：
- s14 lot 只有 1 条 Local/None path（点 0→8），entry 列表
  `[40E0C400:03106C2E]`。
- s14 自带的 PathEntry `40E0C400:03106C2E` 内容 = **大学门**
  `40E1C400:675B61B1` 的逐字段克隆（parent `48E1C400:84139BA1` +
  `09532375/76/77`→`09558B7E` + `0EA241EA` + `75152C4A`），
  **无 filter、无几何、无宽度**。
- 该 TGI 在 scripts 库已存在（真身是**人行道** `DE38BA78`、filter
  `[979CC22E,3713EF31]`、宽 4.5），scripts priority 2 > 普通包 0 →
  **作者副本被遮蔽**，lot 实际解析到游戏人行道。
- 结果：一条 Local 人行道折线；无 Global 车道 → 任何车辆永不进入；
  门语义完全未生效。
- 原版大学 lot `41E1C000:D7A06E31` 的正确形态 = **5 条 path**
  `[03106C2E, 418A18B1, 418A18B1, 675B61B1(门), C34881E7]` 对应 5 个点段，
  全部 Local/None。门资源实际住 `40E1C400`，key 却写 `G40E0C400` →
  **引擎按 instance 解析，key 的 group 只是名义值**。

> 语义化 path 编辑的可行性与架构设计见
> `docs/design/path-semantic-authoring.md`（含 Catalog/Handler/Emitter/
> Validator 分层与 P0 索引预算探针计划）。

## 6. 对资产编辑器"分配行为"的指导

1. **行为面板的正确后端是 bundle 库**，不是 named 属性表。最小行为集 =
   "启用标准 bundle"（耗电 48939D96、耗水 825B4CA4、产垃圾 D7C62954…），
   编辑器应做成可检索的 bundle 目录（1308 条，普查已带 shape/bins/sinks/
   规则引用/被引数元数据），named 属性只作为看板参数暴露。
2. **模板选择决定行为默认集**：挂 `2A25CF66` 即得水电垃圾全家；教育模板
   7E4B99E5 在此之上加教育规则 bundle。编辑器的"行为继承预览"应展示展平
   后的 43-bundle 生效集（探针 flatten 逻辑可直接复用）。
3. **新建行为（原版没有的资源流量）** 才需要自定义 bundle：三列表 + sink
   （组 40E0C500 新 instance）+ 规则引用，且资源 ID 必须已在资源组注册
   （否则行被静默丢弃——fail-closed 构建器应前置校验）。
4. **fail-closed 清单新增**：①输出包与全部已注册库的同 TGI 冲突检测
   （维持）；②菜单 `0x0975695F` 目标存在性断言（静默丢工具）；③
   uiCategories 非空断言（静默不可见）；④bundle 引用的 sink/资源 ID
   可解析性断言（`IndexFromSinkEntryID`/`IndexFromResourceID` = -1 →
   静默丢行）；⑤named 属性改动不影响行为的提示（防"改了没效果"的
   错误心智模型）；⑥**水电接入门控（Q11 后新增）**：依赖 connected
   sink 的行为必须校验 门前点可达性——模型/lot 的 **+Y 正面朝向**与
   footprint 尺寸（门前点=bbox +Y 面中点；克隆 footprint 放大会把门前
   点推出 mMaxDist）；工作台应可视化"门前点 + mMaxDist 圆"作为水电
   可达性预览。lot path 的 roadConnect/scope 列**不再作为水电相关
   字段**对待（纯车辆/行人路径语义，编辑器 UI 应按此归类）。
5. **验证顺序维持菜单加载 oracle**；行为验证用 DISPOSABLE 城市 +
   动态捕获对照（SinkCapacity 分支归因 + `GetFrontRoadAttachPoint`
   hook 对比小学基线）。
6. **路径编辑器按四维语义分组**（§4.7）：①材质/宽度=视觉（车道/人行道/
   跑道/航道）；②filter 成员=通行主体（行人 979CC22E、通用车辆 C37D661D、
   飞机/船各自 filter）——UI 应显式展示"这条 path 谁能走"；③scope=网络
   归属（Local 地块内 / Global 全城+跨城）；④roadConnect=端点是否吸附
   路网。**不要把 scope/roadConnect 当成水电字段**（构建器的
   `property_gate` 分类也应把 path 列归"移动"，与 utility 列分离）。
7. **构建输出应剔除/不复制空表与变体噪声**：普查中 9.1% 是 4 字节空表、
   123 个结构变体；构建器做同 TGI 比对与基线指纹时，空表和 ≤3 属性微表
   应单列（或被忽略），避免"变体差异"被误报为主 property 漂移。

## 7. 未决（按优先级）

1. **s 系列无水无电的实机定案**（Q11 机制已定，剩实证）：三个判别实验
   ——hook `GetFrontRoadAttachPoint`（dev 0x7D78E0）对比小学/s22 门前点；
   s22 紧贴道路正面朝路放置测水电；对照两者 footprint 与 sink mMaxDist
   （`0x0B695F0C`）。若几何排 Align，则转向放置特化（`CanRunEventRules`
   失败→-3）与投递规则（mAgentTests）两条次级通道。
2. 3131 资源 ID 的命名表（Config 文本表解析或运行时 GetIDFromName hook）。
3. jobs 的 195 个 0-bundle 样例的运行期消费方（区划系统？）。
4. locale 三定律与 RegisterLocaleDatabases 扫描根的精确对拍（乱码判别
   实验的前置）。
5. sink 条目 `0x0A810F29/0x0A810F2A` 测试表的精确语义（按等级 int+bool，
   疑与 SinkCapacity branch (d) 单元测试相关）。
6. pipe attach 的 mMaxDist 缺省值数值（cPipeSinkEntry 构造默认）与
   `0x0B695F0C` 在各 sink 条目 nonSim 表上的分布普查。
7. **RCI 变体选择机制（部分定案）**：同族 `2D7BDB01` 108 个 unit 变体
   **全部 0 条 path**（普查），且不使用 replacement priority（全库仅 2 个
   单位带 `0x096E8966/0x097151FF/0x0971522F`）。区划选型走
   `cZoningGame::AssignLotToUnit`（几何/地块适配）；升级替换判据是
   `CanUnitAReplaceUnitB`：`mUnitIsFiller` > 抢占标志
   （`mCanReplaceEqualPriorityUnits`/`mCanReplaceLowerPriorityUnits`）>
   `mUnitReplacementPriority`（属性 `0x0971522F`/`0x097151FF`/`0x096E8966`）。
   `0x0BFB535A` = `mUnitIsFiller`（filler/占位单位）。"建造阶段"的实际
   表现更可能是 **filler 单位 + graphics state 切换**，而非多 unit 变体；
   `0x0CA5CF4A` 是 graphics state 列表（`FindUnitNonSimProps` 按 state 取）。
8. path filter 全表：哪个 filter ID 对应哪类载体（行人/轿车/卡车/公交/
   飞机/船）需要把 33 个声明 filter 的 PathEntry 与 7 个非缺省 transport
   条目的 `0x0EB22775` 做成对照表（引擎无字典，需按行为特征聚类）。

## 证据文件索引

- `tmp/behavior-census-20261106/census.json`（普查全量数据）、`analyze.py`（分析脚本）
- `probes/property/behavior_census.rs`（探针，用法见文件头）
- 反编译关键文件：`src/GB/SinkCapacity.c`、`src/GB/SumConnectedCapacities.c`、
  `src/GB/cEcoSinkEntry/FillFromProps.c`、`src/GB/cEcoUnitEntry/FillFromProps.c`、
  `src/GB/FillFromProps.c`、`src/SC/CreateTools.c`、`src/SC/cTool/FillFromProps.c`、
  `src/SC/CreatePathsFromProps.c`、`src/SP/cPropertyManager/UpdatePropertyListParent.c`、
  `src/SP/AddPackageIfValid.c`、`src/SP/cLocaleManager/RegisterLocaleDatabases.c`、
  `src/anonymous/BuildTotalsJSValue.c`、`src/anonymous/BuildTextPropertyJSValue.c`
- 路径语义（2026-11-06 追加）：`src/GB/cEcoTransportEntry/FillFromProps.c`
  （path filter 装载）、`src/GB/GetPathFilter.c`、`src/GB/FindPathsInSphere.c`
  （位测试闸门）、`src/GB/cEcoGameScripts/AddBaseEntitiesFromGroupIDs.c`
  （filter 构建：`0xD6498E8`→bitset）、`src/SC/CreatePathsFromProps.c`、
  `src/SC/cUnitPathCreationObj/AcceptedScope.c`、
  `src/SC/cUnitPathCreationLocal/CreatePath.c`、
  `src/SC/cUnitPathCreateGlobalConnector/CreatePath.c`
- 空表/变体/挂件（2026-11-06 追加）：`src/SC/cNewLotData/FillFromProps.c`
  （lot 材质 `0x0B762710`/宽度 `0x0B7E3A39`/最小面积 `0x0B7E3A3B`）、
  `src/SP/cModelWorld/AddAttachments.c`（`0x02A907B5/B6` = 模型挂件
  effects keys+transforms）、`src/GB/FindUnitNonSimProps.c`
  （`0x0CA5CF4A` graphics state 列表）
- 既有文档：`building-eco-chain-and-s17-diagnosis-20261106.md`（个案链）、
  `fire-station-economy-anomaly-20261020.md`（绑定中断）、
  `menu-item-relations-20261018.md`（locale 三定律）
