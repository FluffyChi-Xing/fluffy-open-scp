# Debug 工具普查

日期：2026-10-24。

本文把“debug 工具”拆成三个可验证层次：

1. `Menu/Menu2` Property 中仍存在的隐藏菜单条目；
2. 发行版/开发样本 `CreateTool` factory 中仍有明确实现的 C++ 工具；
3. 工具目录中标记为 debug-only、但本身是可放置资产的资源。

这样可以避免把一个有标题的菜单条目误判为可执行工具，也避免漏掉没有菜单入口、但引擎仍保留实现的编辑能力。

## 普查范围与结论

本轮交叉检查了：

- `public/game-ui/tools/tools.json`：649 个 `Menu/Menu2` 工具资源；
- `tmp/development-menu-survey.json`：9,733 个已调查 Property 记录；
- `public/game-ui/tools/costs.json`：工具/资产名称、描述和解锁字段；
- `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c`：`SC::CreateTool`、`SC::CreateTools` 及 `cTool*` 类；
- [file-types.zh-CN.md](../../src/assets/knowledge/file-types.zh-CN.md) 与现有 overlay 实现。

当前可以确认：

- 隐藏父对象 `0xBE5744E0` 下有 **14 个 Menu 条目**，另有 1 个 `Menu2` 条目；
- 已安装发售版 `SimCity.exe` 的 `CreateTool` factory 仍保留 **12 个有明确构造函数的 C++ 工具实现**；旧开发样本反编译文本只列出其中 10 个，不能直接代替发售版核验；
- 目录中有 **3 个明确命名的 debug 工具**；
- 目录中另有 **6 个 debug placement 住宅帐篷资产**，它们不是工具类；
- 目前没有证据证明所有隐藏 Menu 都能被 `CreateTool` 实例化。

## A. 隐藏 Menu 条目

隐藏条目的共同特征是 Parent `0xBE5744E0`。它是一个 0 属性空壳父对象；可见建筑工具使用的真实父对象是 `0xAF042E9A`。隐藏条目常把 `0x08F672C7 (ecoGameToolAction)` 指向自身，这意味着有菜单数据，但未必有对应的 C++ action 实现。

| instance | 类型 | 当前证据 | 可执行性判断 |
| --- | --- | --- | --- |
| `AD947E95` | Menu | 有标题、解锁条件和图标 | 未找到 factory case，待验证 |
| `A4C5D581` | Menu | 有标题、解锁条件和图标 | 未找到 factory case，待验证 |
| `6B735424` | Menu | 无标题，保留 UI 分类 | 占位或内部条目 |
| `A2AA4268` | Menu | Edit Snap Points Tool；debug UI 分类 | overlay 可改分类，factory case 未找到 |
| `03650DFB` | Menu | 无标题 | 占位条目 |
| `BFE9891F` | Menu | 无标题 | 占位条目 |
| `C741981F` | Menu | 有标题/描述字段，factory 有实现 | **已确认 `cToolPathSelector`** |
| `53B77423` | Menu | Edit Unit Prop Tool；debug UI 分类 | overlay 可改分类，factory case 未找到 |
| `9A2BAEE8` | Menu | 有标题、解锁条件和图标；Property action 实际为 `70930C64` | **菜单使用 `cToolDisasterPlop`，不是直接实例化 `cToolTornado`** |
| `EB627DE3` | Menu | 有标题、解锁条件和图标 | 未找到 factory case，待验证 |
| `A5800DC3` | Menu | 有标题、解锁条件和图标 | 未找到 factory case，待验证 |
| `E48FCA6B` | Menu | 有标题、解锁条件和图标 | 未找到 factory case，待验证 |
| `B65E1F2E` | Menu | Unit Connecter Tool；两个 UI 分类 | overlay 可改分类，factory case 未找到 |
| `454041B4` | Menu | 无标题 | 占位条目 |

同一空壳父对象下还出现 `BDA9BCC1`（类型 `Menu2`），标题为 Outworlder 6。它属于普通/扩展内容工具，不能因为 Parent 相同就归类为 debug 编辑器。

### 已有 overlay 解锁能力

当前 [debug_tools.rs](../../crates/sc-properties/src/debug_tools.rs) 只针对以下三个条目生成 overlay：

```text
0x53B77423  Edit Unit Prop Tool
0xA2AA4268  Edit Snap Points Tool
0xB65E1F2E  Unit Connecter Tool
```

补丁修改 Parent、菜单分类和 UI 子分类，保留其他 Property 字节。它证明这些条目可以从数据层重新归类，但尚未证明每个条目在当前零售进程中都有可执行 action；实机验收仍需要单独记录。

## B. `CreateTool` factory 中仍有实现的工具

对安装目录中的发售版 `SimCity.exe` 做 PE 静态核验后，`SC::CreateTool(unsigned int toolID)` 位于 `.text` 的 `0x0089BF00`，switch 明确保留以下 12 个 action ID。`SC::CreateTools` 会从工具脚本组 `0x08F570BD` 枚举 Property，读取 `0x08F672C7`，再把该 instance 传入 factory；factory 不认识的 ID 直接返回空。旧开发样本的反编译文本在同一函数中只呈现 10 个分支，发售版二进制是判断当前游戏执行侧的优先证据。

| action ID | C++ 实现 | 主要能力 | 对资产/地图编辑的价值 |
| --- | --- | --- | --- |
| `134CDAC9` | `cToolPathPlacer` | 交互绘制道路/路径，吸附、坡度、桥梁/隧道、重叠检查和提交 | **最高**：可作为 Path/Network 编辑语义的对拍基准 |
| `1943CB96` | `cToolPlop` | 通用 Unit/Lot 放置、预览、snap point、props 和路径创建 | **高**：对应建筑/Unit 放置与附属资源装配 |
| `2EA8FB98` | `cToolDefault` | 通用工具基类/默认放置行为 | 中：用于解释未专门分类的工具 fallback |
| `476A98C7` | `cToolUFO` | UFO 灾害/特殊单位工具 | 低：偏模拟/灾害，不是资产编辑 |
| `6FE8D729` | `cToolMetaLotZone` | Meta Lot/分区相关放置和生成 | **高**：可研究城市分区、Lot 生成和区域编辑 |
| `70930C64` | `cToolDisasterPlop` | 灾害对象放置 | 低：主要用于灾害测试 |
| `9730F3A0` | `cToolBulldozer` | 拆除/清理 Unit，处理重叠和拆除图形 | **中高**：可作为删除、清理和回滚语义参考 |
| `9A2BAEE8` | `cToolTornado` | 龙卷风特殊工具 | 低：不是地图编译入口 |
| `C741981F` | `cToolPathSelector` | 选择路径、返回选中的世界坐标点 | **高**：可用于路径审查、选点和 Property 对拍 |
| `EF089B06` | `cToolRegion` | 区域工具、区域边界/区域单位相关操作 | **高**：可能连接 Region/City 编辑，但需要继续追写回链路 |
| `22D4ACD5` | `cToolAddTowerLevel` | 添加高塔楼层/楼层单元 | 中高：保留了高塔楼层装配执行侧，但当前对应 `Menu2` 没有标题 |
| `ECC08133` | `cToolSkybridge` | 空中桥梁放置/连接 | **高**：对应 `Menu2` 条目，标题为“空中桥梁”，可作为跨 Unit 连接语义的阳性样本 |

这里的“有 factory 实现”只表示 C++ 构造和方法存在，不自动表示当前菜单可见、输入绑定已打开或能安全写回 package。`C741981F` 是隐藏 Menu → `cToolPathSelector` 的直接闭合样本；`22D4ACD5` 与 `ECC08133` 是 `Menu2` → 高塔/空中桥梁工具的直接闭合样本。

这一段需要更正：隐藏 Menu `C741981F` 的 Property action 确实是 `C741981F`，因此闭合到 `cToolPathSelector`；隐藏 Menu `9A2BAEE8` 的 action 是 `70930C64`，它走的是 `cToolDisasterPlop`。`9A2BAEE8` 这个 instance 恰好与 `cToolTornado` 的 factory action 同名，不能据此推断菜单会创建 `cToolTornado`。

### 发售版的直接二进制证据

本机发售版文件：

```text
D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCity\SimCity.exe
size   = 10,971,552 bytes
SHA256 = 2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab
```

工厂函数的反汇编分支为：

```text
134CDAC9  1943CB96  22D4ACD5  2EA8FB98  476A98C7  6FE8D729
70930C64  9730F3A0  9A2BAEE8  C741981F  ECC08133  EF089B06
```

发售版中 `53B77423`、`A2AA4268`、`B65E1F2E` 的 little-endian 和 big-endian 原始字节出现次数均为 0；同时没有 `cToolUnitConnecter`、`cToolPathEdit` 或 `cToolSnapPoint` 这类对应 RTTI 字符串。结合 `CreateTool` 没有三项分支，可以把三个 authoring 工具判定为“菜单/Property 残留，发行版执行侧未保留”。

同一发售版仍有 `cToolAddTowerLevel` 与 `cToolSkybridge` 的 RTTI。其 factory 分支分别分配 `0xBC8` 和 `0xB40` 字节，并调用对应构造函数；这两个是明确的执行侧残留，而不是只有菜单文本的空壳。全量 SimCityData 的 Menu/Menu2 Property 对拍还确认：`22D4ACD5` 是无标题的 `Menu2`，`ECC08133` 是标题为“空中桥梁”的 `Menu2`，二者的 action 均指向自身并能命中 factory。

## C. 明确标记为 debug 的工具与资产

`public/game-ui/tools/costs.json` 中有三个明确的 debug 工具描述：

| instance | 名称 | 描述 | 当前状态 |
| --- | --- | --- | --- |
| `A2AA4268` | Edit Snap Points Tool | setup and edit snap points on a unit | 有 overlay，factory case 未找到 |
| `53B77423` | Edit Unit Prop Tool | setup and edit a unit | 有 overlay，factory case 未找到 |
| `B65E1F2E` | Unit Connecter Tool | manually connect units | 有 overlay，factory case 未找到 |

同一目录还标出 6 个 debug placement 资产：

```text
23934B12  Homeless Tent 5  -> AB3EB9E4
23958A7D  Homeless Tent 6  -> AB37FDAB
239DD8B6  Homeless Tent 1  -> AB494688
639171CB  Homeless Tent 4  -> EB3CDFDD
63A07C41  Homeless Tent 2  -> EB415C8F
63A2BAA8  Homeless Tent 3  -> EB439B16
```

帐篷是可放置 Unit 资产，不是 `cTool*` 实现。它们能帮助研究“debug placement 如何把一个 Unit 放进城市”，但不能直接替代 Lot、Path、Spawner 或 ER2 编辑器。

### Path / Homeless Tent 实机探针

已经新增 `probes/menu/path_debug_overlay.rs`，生成一个只包含 7 条 Menu
覆盖的测试包：

| 槽位 | 覆盖条目 | 行为闭合 | 测试目的 |
| --- | --- | --- | --- |
| 110 | `C741981F` | action `C741981F` → `cToolPathSelector` | 验证路径选择器是否能创建、响应点击并显示路径点 |
| 111–116 | 六个 Homeless Tent 条目 | `menuUnit` → `40E1C000:EB415C8F` | 验证 Unit plop、预览、放置和删除生命周期 |

生成命令：

```text
cargo run -p probes --release --example path_debug_overlay -- \
  "D:\\ea-games\\simcity_offline\\SimCity：Cites of Tomorrow\\SimCityData\\SimCity_Game.package" \
  "D:\\ea-games\\simcity_offline\\SimCity：Cites of Tomorrow\\SimCityData\\000_OpenSCP-path-debug-probe.package" \
  110
```

该包已生成到当前安装的 `SimCityData` 目录。帐篷菜单使用完整的可见建筑
Property 外壳，只替换 `menuUnit`、标题、描述、道路分类和槽位；因此它能
验证实际 Unit 放置链，但不能证明帐篷自身包含路径或游民 AI 逻辑。路径选择器
则保留可见道路 Property 的字段，只把 action 改为发行版仍有 factory case 的
`cToolPathSelector`。

实机记录应分别观察：菜单出现、点击有响应、是否出现预览、是否能提交放置、
是否能删除、保存重载后是否仍存在。路径选择器如果只显示菜单但无预览，说明
缺少它所需的运行时输入上下文；帐篷如果能放置但没有游民活动，说明 Unit
资源存在而其模拟/Spawner 关系仍需另行研究。

## D. 与地图编辑最相关的突破口

按对资产编辑的直接价值排序：

1. **Terrain Editor / RunScriptBrushes**：虽然不属于 `Menu` factory，但仍有 `ApplyBrush`、`ApplyGlobalBrush`、`ApplyRibbonBrush`、`BeginAccumulatingBrushes`、`EndAccumulatingBrushes` 和 `Save341HeightMaps`。这是地下水、土壤、森林和资源 typed map 的核心突破口。证据见 [hidden-debug-tools-and-terrain-brushes.md](hidden-debug-tools-and-terrain-brushes.md)。
2. **`cToolPathPlacer` + `cToolPathSelector`**：引擎内已有道路/路径吸附、坡度约束、桥梁/隧道、交叉检查、PathEntry 和 Network 提交逻辑，适合作为 Mod Studio Path 节点的语义参考。
3. **`cToolMetaLotZone` + `cToolPlop`**：前者帮助理解分区与 Lot 生成，后者覆盖 Unit/Lot/props/path 的放置预览和 snap 规则，适合作为建筑资产装配与放置验证的参考。
4. **`cToolRegion`**：可能触及区域边界和区域单位，但目前只有类和 factory 证据，尚未证明可以修改 RegionTerrain 或 EGB。
5. **三个 debug 编辑器条目**：Edit Unit Prop、Edit Snap Points、Unit Connecter 对建筑资产 authoring 很有价值，但必须先找到实际 action/factory 或运行时绑定；仅修改菜单分类不能保证能执行。
6. **Bulldozer**：可用于删除/清理和验证 Unit 生命周期，不是地图生成器。
7. **UFO、Tornado、DisasterPlop**：主要是模拟/灾害测试工具，对地图编辑流水线帮助有限。

## 当前可用性分级

| 等级 | 含义 | 工具 |
| --- | --- | --- |
| A | factory 有明确 case，可继续做输入和运行时验收 | PathPlacer、Plop、Default、AddTowerLevel、Skybridge、UFO、MetaLotZone、DisasterPlop、Bulldozer、Tornado、PathSelector、Region |
| B | 菜单/Property 和数据 overlay 已确认，但 action 是否可执行尚未闭合 | Edit Unit Prop、Edit Snap Points、Unit Connecter |
| C | 有菜单记录但无 factory case、标题不完整或只有占位字段 | 其余隐藏 Parent `0xBE5744E0` 条目 |
| D | debug-only 可放置资产，不是工具实现 | 六个 Homeless Tent |

## 后续验证顺序

1. 从工具脚本组 `0x08F570BD` 导出所有 Property 的完整 TGI、`0x08F672C7` action、Parent、分类和标题，建立 Menu instance 与 action instance 的双向表。
2. 对 A 级工具做输入绑定和最小运行时记录：创建、预览、提交、撤销/删除、保存/重载；优先验证 `Skybridge` 与 `AddTowerLevel` 的 Menu2 入口。
3. 对 B 级工具追查非 `CreateTool` 的注册路径、热加载组和 JS/UI action；在未闭合前不把它们标成“可执行”。
4. 以 `cToolPathPlacer`、`cToolMetaLotZone` 和 Terrain Editor 为三个独立实验，分别建立 schema 输出：Path/Network、Lot/Zone、typed map/341 tile。
5. 将 `cToolRegion` 单独作为 Region/EGB 研究，不把区域边界工具与地形资源写回能力混为一谈。

## 证据索引

- `public/game-ui/tools/tools.json`
- `public/game-ui/tools/costs.json`
- `tmp/development-menu-survey.json`
- `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c`：`SC::CreateTool`、`SC::CreateTools`、各 `cTool*` 类
- 发售版 `D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCity\SimCity.exe`：`CreateTool` 分支、PE 字符串/RTTI 和三个 authoring action 的缺失字节核验
- [file-types.zh-CN.md](../../src/assets/knowledge/file-types.zh-CN.md)
- [crates/sc-properties/src/debug_tools.rs](../../crates/sc-properties/src/debug_tools.rs)
- [hidden-debug-tools-and-terrain-brushes.md](hidden-debug-tools-and-terrain-brushes.md)
