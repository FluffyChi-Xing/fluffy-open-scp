# 标准建筑资产编辑契约：Property → EcoGame → ER2 → UI

日期：2026-10-23。本文把小学 `C264A268` 和基本抽水泵 `CEAA14AE` 的已验证证据抽象成 OpenSCP 编辑器的校验模型。以下“必需”是选定标准模板的检查项，不是所有建筑统一的引擎最小字段集，字段也可以继承。教室/校车属于可选功能依赖；Education.PrimarySchool等名称为拟议OpenSCP标签，并非引擎枚举。没有运行时/静态证据的字段不应由编辑器自动填充。

## 1. 四层身份不能混用

```text
Menu2 instance                 # 工具栏、分类、名称、价格入口
  └─ menuUnit → EcoGame UnitEntry ID
       ├─ Unit Property         # 费用、容量、属性、bundle、父子关系
       ├─ Lot/Graphics/Locations# 模型、地块、锚点、路径入口
       └─ connected bundles     # 资源 bin、规则索引、扩展行为
            └─ ER2 UnitRule/EventRule → ApplyBinRule/Agent/Transport
UI query binding → resource/property/attached-unit readout
```

Menu instance、UnitEntry ID、运行时 unitSlot、ER2 ruleId、resourceId、UI text/icon TGI 是不同主键。OpenSCP 必须在内部数据模型中分别保存并显示来源；不能用菜单 instance 推断规则，也不能用 unitSlot 写回资产。

## 2. 编辑一个标准 Unit 的字段契约

### A. 所有可放置建筑

| 层 | 必须核对 | 作用 |
|---|---|---|
| Menu2 | `menuUnit`、分类/父菜单、位置、建造价/维护显示、locale/icon/marquee | 能否从 UI 放置和显示 |
| UnitEntry | UnitEntry ID、父 Unit/子 Unit、建造价、维护费、放置/lot 引用 | 模拟主体与扩展经济 |
| Lot/Graphics | `40E1C000` graphics、模型、地块尺寸、朝向、位置表 | 可视化、放置边界、锚点 |
| Bundle | `0A0209A5` connected bundles、`0A0209B2` excluded bundles | 资源/规则/模块装配 |
| 位置/路径 | UnitLocation、Path/PathPoint、停车/入口锚点 | Agent/车辆可达性 |
| UI | attached UI type、rollover/query binding、文本/图标 | 选择后面板与统计显示 |

建造价和维护费要从主体 Unit 加子 Unit 重新计算，不能只复制菜单显示值。小学主体 13000/300 加默认子 Unit 3000/100，菜单显示 16000/400，是该规则的实证。

### B. 教育建筑（小学样本）

必须在通用字段上增加：

- 教育菜单类别和教育 Unit/attached UI type；
- 学生容量/席位资源、学生学习资源和教育等级/知识链；
- 默认校车站/车辆模块、教室扩展 Unit、UnitLocation 与路径；
- 垃圾生产属性 `kPropGarbageProducer_GarbageOnlyAmount`；
- 连接到教育 bundle 的 UnitRule/EventRule 索引；
- UI 查询至少区分当前学生、总学生、可用席位、在校学习学生、校车站点/车辆容量。

资源 bin 的静态名称已确认：`Student`、`TotalStudents`、`SpotsForKids`、`StudentStudyingE2`。这些是教育状态，不是水电。规则执行时必须保留 `applyCount`，实际 delta = 每次操作系数 × 应用次数，并以 bin 前后快照复核。

### C. 水生产建筑（基本抽水泵样本）

必须在通用字段上增加：

- 水生产者资源引用 `Water`/`WaterToTransmit`；
- `WaterProducer_Rate`、`Amount_Minimum`、`Amount_OneOverSteps`、`Amount_PerStep` 等生产参数；
- 地下水/枯竭/污染状态资源和连接网络；
- 连接 bundle 与水生产 UnitRule、发送/回流/统计规则；
- UI 绑定生产速率、地下水、污染、输送量和本地使用开关。

基本泵运行时每个实例的规则 `0xB78DD3BE` 每 2 个 `simTicks` 将 `WaterToTransmit` 增加 1000；这是基础事件，另有修正与传输规则。编辑器不能把一个 1000 字面量直接显示为完整每小时产水量。

### D. 交通/车辆建筑

必须增加：TransportID/HandlerID、车库/停车位容量、车辆 Unit/Agent 类型、生成/回收规则、路径入口、站点 attached UI type。只有同时存在这些引用并被运行时日志验证，才可把资产标为“自带车辆”或“可生成车辆”。模型中出现一辆车不能替代车辆实体规则。

## 3. 如何把 Unit 定义为特殊建筑

特殊类别不是一个 `isSchool=true` 的单字段。OpenSCP 应将类别作为可验证的证据集合：

```text
Specialization = {
  menuCategory,
  unitEntry,
  requiredProperties,
  connectedBundles,
  attachedUIType,
  requiredResourceBindings,
  ruleSet/ruleIds,
  agent/transport/path requirements
}
```

例如小学只有在教育菜单归属、学生/席位资源、教育 attached UI、教育 bundle、教室/校车扩展和对应规则均存在时，才标记为 `Education.PrimarySchool`。抽水泵只有在水生产属性、WaterToTransmit 规则、水生产 UI 查询和地下水状态链均闭合时，才标记为 `Utility.WaterProducer`。缺少任一必需证据时显示“部分匹配/未知”，不自动伪造字段。

## 4. 如何配备 ER2 脚本

编辑流程应以已有 ER2 资源为模板：

1. 选择目标 Unit 与功能模板（教育/水生产/交通等）。
2. 复制或引用已验证的 bundle，不直接手填 ruleId。
3. 对每个 bundle 解析 `0x08080D58 + 0..2` 的 UnitRule 数组及 `0x08327D48 + eventRuleIndex` 的事件槽。
4. 在 AEB/ERZ 中校验 ruleId 存在、规则索引与运行版本一致、success/fail 链闭合、资源操作引用已注册。
5. 将 `AgentTests`、`TransportOps`、`TransportID`、`HandlerID` 标为已解码/未知；未解码时禁止声称具备校车、行人或车辆行为。
6. 运行时用 `simulation.rule`、`simulation.effect`、`simulation.condition` 三通道对拍：规则命中、条件、bin 前后变化必须一致。
7. 保存为新 overlay/package，并生成依赖清单和可回滚诊断报告。

EA::ArgScript 的原始 `goToWork`、`sendTo`、`switchTo` 等文本语义在发行版中通常已编译/哈希化。编辑器现阶段应支持“引用已验证规则模板 + 二进制字段检查”，不能把规则 ID 伪装成可编辑的原始脚本文本。

## 5. UI 编辑器绑定模型

UI 绑定应保存完整 query 路径，而不是只保存资源 ID：

```text
query = [scope, unit/connectedUnit/attachedUIType, dataKind, resourceId/propertyId]
```

例如教育类别面板使用 `cachedUnits → EducationE2 attached UI type → bins → Student`，建筑面板使用 `connectedUnits → unitSlot → binAmount → Student`，水泵面板使用 Unit/connectedUnits 的 `WaterProduction`、`WaterInMap`、`GermsInPump` 和属性速率。编辑器应显示：数据来源、本体或 connectedUnits、当前值/统计值/估算速率、单位格式、刷新周期。

## 6. OpenSCP 优先实现

1. **只读资产依赖图**：Menu2 → Unit → Lot/Graphics → Bundle → Rule → Resource → UI query。
2. **特殊建筑校验器**：小学、水泵作为首批模板，输出缺失字段和证据等级。
3. **ER2 规则检查器**：显示规则索引、资源操作、条件、applyCount、success/fail 链和未知字段。
4. **运行时对拍时间线**：显示游戏时间、UnitEntry、slot、ruleId、resourceId、before/after、delta、dropped/readErrors。
5. **语义编辑器**：先允许修改已完成静态/运行时闭合的字段，如容量、垃圾量、泵的明确生产参数；修改前自动生成备份和依赖报告。
6. **UI 绑定检查**：验证 locale/icon、attached UI、query 路径与资源/属性是否存在，并维护 zh-CN/en-US。

任意写回之前必须通过静态闭合、ER2 索引闭合、UI 查询闭合和运行时对拍四道检查。当前小学的校车 Agent、spawner/path、教育知识回传仍未达到写回条件。


## 本轮实际解析突破

新增只读探针 `probes/runtime/unit_rule_resource_bridge.py`，输出 `evidence/unit-rule-resource-bridge-20261023.json`。只解码当前5/2/14、namesIncluded=0的UnitRule资源前缀与fromBinOps；控制流和交通尾部未解码，开发版与零售版记录长度不同，不能直接套用尾部偏移。

### 水泵的动态产量来源

- CEAA14AE实例属性0CD9C693（WaterProducer_Amount_Minimum）=1000。
- 父191D9EC9通过0A0209A4连接A8D0E850。该bundle的0A6FE74B=[2B6A5AFE WaterMin]、0A6FE74C=[0CD9C693 PropertyID]形成资源初值/属性来源对应。
- B78DD3BE静态local[0]=(WaterToTransmit,input=0,output=999999)。
- fromBinOps为来源scope0、WaterMin，目标scope0/op0，writeMode2。
- 开发版ApplyAmountsFromBins（SimCity.unwrapped.exe.c:701205）显示writeMode2将来源bin amount写入output。运行时有效output=1000，70次实际更新均为+1000。

因此999999是动态替换之前的基础字段，不能直接作为产量编辑。属性→bin初始化→规则动态操作→实际delta已经获得交叉证据；完整零售初始化调用链仍待单独验证。

### 小学的跨作用域规则

B52C889D.local包括Student+1、TotalStudents+1、SpotsForKids-1、StudentStudyingE2+1、FromTradingConnection output=999999。nearby包括KidE2 input=1；global包括TotalStudents+1和StudentStudyingE2+1。fromBinOps来源scope1的FromTradingConnection写入local操作4的output；运行时该output为0。

该规则flags=02000000，开发版RunRule（697349起）把该标志与bound transaction资源路径关联；nearby不能简单译为附近建筑。上述global操作为静态预期，还没有由本轮unit-local探针证明全局实际变化。

### 产品约束

资源编辑应显示容量、初值、属性来源与数组索引。规则检查应区分基础值、动态来源、有效值。Bundle解析同时覆盖0A0209A4/A5和排除列表；只看A5会漏掉核心水生产bundle。菜单分类、attached UI类型、模拟功能bundle、zone/growth分类是独立维度，改变菜单不能自动改变住宅/商业/工业行为。复用已验证规则模板与新写ER2是不同能力；任意脚本写回仍缺完整编译/加载验证。

验证：小学18条与水泵基础70条，共88条记录，资源顺序和非动态系数与静态定义一致，delta均匹配。截断文件被拒绝。另发现59F74513静态local系数为(1,1)/(0,1)，运行时为(436,436)/(0,18)，applyCount=20，实际+360；该规则没有fromBinOps，说明仍有Unit特化/其他参数处理层待追踪，不能认为fromBinOps解释所有动态值。
