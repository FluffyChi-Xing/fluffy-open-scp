# 菜单到游戏机制研究底稿

日期：2026-10-22。本文记录菜单编辑之后的机制研究第一阶段。静态资源、反编译结论和运行时装配分别标注；本文不把菜单标题直接当成建筑行为定义。

## 1. 总体链路

```text
HUD/Layout
  -> Menu/Menu2 (0x00B1B104)
  -> uiCategories / parent / uiPosition
  -> locale + icon + marquee
  -> menuUnit 或道路分段
  -> EcoGame Unit / Resource / Path / Transport
  -> ER2 unitRule/globalRule
  -> GlassBox 模拟行为
```

菜单项是入口 Property。它负责显示、归属、排序、解锁和行为指针；建筑的生产、容量、资源和服务行为主要由 EcoGame Unit 与规则资源提供。

## 2. 六类菜单的第一批样本

| 菜单 | 静态/运行时入口 | 代表样本 | 第一条机制追踪路径 |
| --- | --- | --- | --- |
| 道路 | 静态城市容器 `AC12C904`；另有平行容器 `5E0AC22E` | `7DD8A627`、`41158FE5` | Menu2 → 道路分段 `0xD13BB4A6` → road cost `0x0E76A7E2` → path/network |
| 教育 | 城市教育容器；大学子菜单 `9D2EF585` | `9A38E382`、`75A90B66`、`64088034`、`2491343B` | Menu2 → Unit → 教育容量/覆盖规则 → ER2 |
| 公园 | 按钮 `899BEA70`，成员由四个公园父菜单运行时聚合 | `46173A50`、`646AAA32`、`7A90E787` | parent → 运行时价格子栏 → Unit/Lot/Prop/Effect |
| 公共交通 | 按钮 `CBC27856`，静态条目按交通模块容器聚合 | `79F3ED13`、`F0DBD6F2`、`E8792D59` | uiCategories → transport Unit → vehicle/path/capacity |
| 地标 | 发展菜单文化容器 `94E412ED` 的地标子栏 `3F43E038` | `60861B5E`、`C0E5BE77`、`D6093278` | Menu2 → landmark Unit → `HasLandmark`/容量数组 → 全局规则 |
| 欧米伽 | 发展菜单容器 `692D1886` | `EF7A242B`、`2CF4EF86` | Menu2 → Omega Unit/resource chain → ER2 生产/需求规则 |

### 已确认的关键样本

- `7DD8A627` 的 `uiCategories = [5E0AC22E, AC12C904]`，`parent = 12E3214B`。同一条菜单 Property 可同时出现在两个道路容器中。
- `41158FE5` 的 `uiCategories = [5E0AC22E, AC12C904, 4A052724]`，证明归属数组与功能继承是两条独立关系。
- `9A38E382` 是教育菜单中的人行道，但 `parent = 12E3214B`，说明显示菜单不等于功能家族。
- 大学建筑样本 `75A90B66`、`64088034`、`2491343B`、`16FC99FA`、`36E3E922`、`1F1799B4` 都进入 `9D2EF585`，但多数 `parent = AF042E9A`，行为需要继续追到 Unit。
- 公共交通按钮本身在静态 `tools.json` 中没有直接成员；`extra-containers.json` 以交通模块容器聚合成员。
- 公园按钮同样没有直接 `uiCategories` 成员，而是由 `C725C506`、`F1035727`、`4EEB801B`、`F970A822` 四个父菜单反推。
- 地标子栏 `3F43E038` 当前有 23 个静态样本；文化、体育场馆、景点和宗教场所使用不同子栏。

### 2.1 资产引用的样本证据

工作台 catalog 对这些条目已经保存了字段角色和候选 TGI，可作为后续 Unit 扫描的输入：

- 公园 `46173A50`：继承 `F1035727`，归属 `2A31842A`，图标 `44969C52`，rollover 使用条目自身 instance；动作字段 `0x0975695F` 的候选包含 `00B1B104:40E1C000:44969C52`。
- 公园 `646AAA32`：继承 `C725C506`，同时挂入 6 个价格/功能容器；动作资源 instance 为 `05EBF0A0`，说明一个公园工具可以跨多个运行时子栏复用同一资产。
- 教育/大学宿舍 `75A90B66`：进入 `9D2EF585`，继承 `AF042E9A`，状态图标为六态，动作候选包含 `00B1B104:40E1C000:A9D67AA8`。
- 教育/学院 `64088034`：进入 `9D2EF585`，继承 `AF042E9A`，动作候选为 `00B1B104:40E1C000:426F38E2`。
- 地标 `C0E5BE77`：进入 `3F43E038`，动作资源 instance 为 `118282E9`；该 instance 已在地标容量研究中确认位于 `40E0C000`，继承 `TemplateToyBase_landmark`，并与 `HasLandmark`/共享容量链相连。
- 欧米伽 `EF7A242B` / `2CF4EF86`：进入 `692D1886`，使用 EP1 的 `61F0C900` 菜单资源；动作资源分别为 `071D31E5`、`2AF0198C`，说明 EP1 欧米伽资产采用独立的配置/图形组，需要再向 EcoGame Unit 和 ER2 规则反查。

这里的 `0x0975695F` 是动作/模型详情字段。catalog 会返回多个同 instance 候选组；不能只取第一个候选就断言它是 EcoGame Unit。需要依据 group、包来源和字段类型继续解析，这也是菜单编辑与资产编辑联动时必须保留完整 TGI 的原因。

## 3. 可绑定资产分类

1. **UI 入口**：Menu/Menu2 Property、父菜单、子分类、排序、解锁字段。
2. **显示资源**：locale 表、槽位 PNG、rollover JPG、状态图标。
3. **经济资源**：EcoGame Unit 的建造价、维护费、子 Unit 和统计参数。
4. **建筑空间**：Lot 尺寸、前缘、放置变换、道路连接、路径和位置点。
5. **图形资源**：RW4 模型、LOD、facade、材质、纹理、impostor。
6. **场景附属物**：props、effects、decals、paths、unit locations、resource bins。
7. **模拟资源**：resource、map、zone、sink、transport、bundle 表项。
8. **规则资源**：ER2 文本/二进制规则，以及由 EcoGame 脚本注册的 `unitRule` / `globalRule`。

## 4. 建筑分类判定原则

建筑不能依据单个字段分类。建议按以下顺序判定：

1. `uiCategories`：它从哪个菜单入口出现。
2. `parent`：它继承哪个工具/功能家族。
3. Unit/非模拟 Property：它是否具备 zone、service、education、landmark、omega 等资源或规则引用。
4. Lot/Path/Transport：它的放置、道路连接和 agent 行为类型。
5. ER2：它实际消费、生产、容量、覆盖、解锁和全局约束的规则。

因此住宅/商业/工业更多是 zone 与模拟资源语义；地标是 Unit 加全局唯一性/容量规则；学校是教育 Unit 加覆盖/容量规则；欧米伽是发展菜单入口加专用资源链和 ER2 规则。模型、菜单名称和建筑外观只能作为辅助证据。

## 5. EcoGame/ER2 研究路线

已有反编译证据确认：EcoGame 脚本主表位于 `0x40800200`，资源表主要在 `SimCityUserData/EcoGame/SimCity-Scripts_*.package` 的 `0x40E0C100`，Unit、resource、map、zone、path、point、sink、transport、bundle 通过主表的 KeyArray 组装。

下一步对每个代表样本执行：

```text
Menu2.menuUnit
  -> Unit property (0x40E0C000 / 0x48E0C000)
  -> 子 Unit / resource / path / transport 引用
  -> ER2 unitRule/globalRule
  -> 可观察的容量、生产、服务、agent 或全局限制
```

ER2 `0x08068AEC`/`0x08068AEB` 的类型和加载链已确认，但完整规则语义仍需按学校、欧米伽、地标和交通样本逐条验证。

## 6. 当前边界与下一阶段

- 公园和公共交通是运行时装配菜单，不能只读取 `tools.json` 得出完整成员。
- 菜单 Property 到 Unit 的 `menuUnit` 仍需对六类样本逐项补齐。
- 住宅/商业/工业的统一分类字段尚未被单一 Property 键定案，需要结合 zone/resource/rule 交叉验证。
- ER2 规则文件目前可以定位、分类和追踪加载，尚未完成通用反编译器。
- 下一阶段优先做学校、欧米伽、地标各两个样本的 Unit/规则纵向追踪，再处理公园、交通和道路的特例。

## 证据文件

- `docs/re/menu-item-relations-20261018.md`
- `docs/re/menu-organization-20261021.md`
- `docs/re/dev-dump-lot-placement-and-script-resources.md`
- `docs/re/props-effects-spawners-paths-engine-flow.md`
- `public/game-ui/tools/tools.json`
- `public/game-ui/tools/extra-containers.json`
