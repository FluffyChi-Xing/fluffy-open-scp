# 包热重载可行性调研：引擎自带 ResourceHierarchyHotload 子系统（2026-10-27）

> 需求：测试 mod 包（data / userdata 下的 .package）时反复重启游戏过于耗时，
> 希望具备**运行时热重载**能力。本文在 dev RL 反编译源码树
> （`new-cource/SimCity2013-source-tree`，与
> `docs/source-code/SimCity-dev-ida.c` 同源）中调研引擎的包加载/卸载与
> 热更新机制，并给出可落地的实现路径与边界。

## 0. 结论

1. **可以实现，且不需要外挂模拟——引擎自带完整的"资源层级热加载"子系统**
  （`SP::InitResourceHierarchyHotload`），含目录文件监视、类型映射表、
   变更通知广播和按子系统的语义级刷新（rules/units/zones/sinks/gameProps）。
2. **该机制在零售 EXE 中存留**：`"GlobalMap.txt"`（UTF-16）字符串在零售
   PL EXE `0x8fc2b8` 命中（`InitResourceHierarchyHotload` 的唯一字符串特征）；
   符号已随 PL 剥离，可用 RL 符号按"形态指纹 + 字符串引用"锚定
  （`dev-beta-vs-retail-diff.md` §四.3 的既有方法）。
3. 官方使用场景 = **服务器运行时下发数据包后免重启生效**
  （`GB::cUpdateManager::DataFinalizeFinishFullPacked → SP::ReregisterData`：
   重新 Open 全部数据库 + 在 `GetDataDir()` 重装热加载）。我们的测试场景
  （本地改包→游戏内生效）是对同一条官方路径的外部触发。
4. 触发方案推荐 **Frida 直接调用**（仓库已有 Frida QJS 双钩子基础设施与
   零售 RVA 底账）；备选 dev 版控制台（`cCheatManager` 可注册自定义 cheat）。
   内置作弊命令表无现成 reload 命令（`AddBuiltInCheats` 全表核对）。

## 1. 机制证据链（dev RL 符号级）

### 1.1 资源管理器基础设施（EA::ResourceMan）

`src/EA/ResourceMan/Manager/` 下具备全套运行时变更原语：

| 文件 | 作用 |
|---|---|
| `RegisterDatabase.c` / `IsDatabaseRegistered.c` | 数据库注册/查重 |
| `DoDatabaseChanged.c` + `RegisterChangeNotification.c` | 数据库变更广播 + 变更订阅 |
| `FlushCache.c` / `ReloadResource.c` | 缓存冲刷 / 单资源重载 |
| `InternalDatabaseList/` | 数据库优先级表（零售侧底账：Add `RVA511080` 稳定 priority 降序、Find `RVA510B40` 首命中） |

### 1.2 目录数据库自动更新（热加载核心）

- `EA::ResourceMan/DatabaseDirectoryFilesAutoUpdate/`：维护
  `DatabaseDirectoryFiles`（**监视目录的数据库类型**）集合；
  `FileChangeCallback.c` 收到 `EA::IO::FileChangeNotification` 后，
  按路径匹配到对应 DDF → 调其虚表更新函数取受影响 key 列表 →
  **`mpManager->DoDatabaseChanged(database, keyList)`** 广播变更。
- `Init.c` 以 `pBaseDirectory` 建监视；`AddDDF.c` 登记/注销 DDF。

### 1.3 游戏侧入口：`SP::InitResourceHierarchyHotload(path)`（`src/SP/`，EA 0x4A2670）

1. 读 `<path>\GlobalMap.txt` → `SP::ReadGlobalTypeList`：逐行
   `0x<typeID> <扩展名>`（`buffer[10]=0` 截断、扩展名最长 31 字节）→
   `Manager::SetTypename(type, name)` 注册类型↔扩展名映射——**这是松散
   文件按文件名建键的依据**；
2. new `DatabaseDirectoryFilesAutoUpdate` 并 `Init(baseDirectory=path)` →
   `EA::ResourceMan::SetDDFAutoUpdate(...)` 全局生效；
3. `Manager::RegisterChangeNotification → OnDatabaseRecordChanged`。

`OnDatabaseRecordChanged`（`src/anonymous/`）：把变更打包成
`(type, group, instance, database, context)` 消息，**消息 ID = 16132831
（`0xF62ADF`）**，投递到 `SP::MessageServer`。

### 1.4 变更消费方（语义级刷新）

| 订阅方 | 行为 |
|---|---|
| `GB/cEcoRulesFactory/HandleMessage.c`（含消息常量 16132831） | **规则热重载**（`kHotLoadRules` 路径） |
| `SC/HotLoadTools.c`（0x858080） | 变更记录 group ∈ nonSim 工具组（`GetNonSimGroupIDs(150302909)`）时，按 instance 找到现存工具 → **重新 `FillFromProps`**（菜单/工具活刷新） |
| `SC::cZoningGame::HotLoad` 等 9 处子系统 HotLoad | `GB::tEcoHotLoadReason` ∈ {`kHotLoadUnit`, `kHotLoadGameProps`, `kHotLoadZone`, `kHotLoadSink`, `kHotLoadRules`}；`cZoningGame::HotLoad(kHotLoadGameProps)` 实测会重建 blocking 几何、重读 app 属性、**遍历已放置 zoning lot 重刷** |
| `SP::cScriptRenderer::HotLoad(newRenderer)` | 脚本渲染器整体热替换 |

### 1.5 官方运行时重注册路径

`SP::ReregisterData(downloadDataID)`（`src/global/`，调用方
`GB/cUpdateManager/DataFinalizeFinishFullPacked.c`）：

1. 以 `dataKey{instance=downloadDataID, group=kPackageInfoGroup_3,
   type=B1B104}` 找库，验库类型 FourCC `709082436`（`0x2A43BD44`）；
2. **对数据库列表全量 `Open(1, 6, false)`**（重开/刷新句柄）；
3. `SP::InitResourceHierarchyHotload(SP::GetDataDir())` 重装热加载。

即：服务器下发新数据包 → finalize → 免重启生效。这正是我们要的外部触发等价物。

### 1.6 零售存留证据与锚点

| 证据 | 零售 PL EXE | 说明 |
|---|---|---|
| `L"GlobalMap.txt"` UTF-16 | `0x8fc2b8` 命中 | `InitResourceHierarchyHotload` 存留（该字符串仅被它引用） |
| 消息常量 `0xF62ADF` / 库类型 `0x2A43BD44` 立即数 | 未命中 | 订阅侧存留性**未定**，需运行时锚定；也可能零售换值/内联 |
| RL 符号锚定法 | 可行 | 按 `dev-beta-vs-retail-diff.md` §四.3：反编译形态指纹 + 字符串引用逐函数锚定，禁止地址平移 |

## 2. 实现方案

### 方案 A（推荐起步）：Frida 调 `InitResourceHierarchyHotload`，mod 走松散文件目录

1. 用 RL 符号在零售定位 `InitResourceHierarchyHotload`（锚：`GlobalMap.txt`
   字符串引用 + 前置 `ResourceManager()` 调用形态）与 `SetDDFAutoUpdate`；
2. Frida 注入调用 `InitResourceHierarchyHotload(modDir)`（modDir 内放
   `GlobalMap.txt` + 按 `<实例>.<扩展名>` 命名的松散资源）；
3. 之后改文件 → 系统文件通知 → DDF 重索引 → `DoDatabaseChanged` →
   消息 `0xF62ADF` → 规则/工具/子系统各订阅方自动刷新。

- 优点：完全走引擎原生路径，文件级粒度，改动即生效；
- 待实测：DirectoryFiles 的松散文件键名规则（`GetKeyFromName` + typename
  扩展映射——实例段是十六进制数字还是 name-hash 需一次实验定谳）；
  openscp `build` 产物需增加"松散目录导出"模式。

### 方案 B：保持 .package 工作流，Frida 复刻 `ReregisterData`

改写 `SimCityData`（或 userdata）下的目标包后，Frida 调
`ReregisterData` 等价链（全库 `Open(1,6,false)` + 重装热加载）。

- 依据：`AddPackageIfValid` 对"多条同 metadata 注册"是拒绝的
  （既有零售 RVA 底账 `RVA5EC40`），**重复 Add 不可行**；ReregisterData
  的"重开"才是官方覆盖-生效路径；
- 优点：不改测试产物的包形态；缺点：粒度是整库，且"重开"对已加载资源
  （RW4/纹理缓存）是否失效需实测（`FlushCache` 在，是否被链上调用待验）。

### 方案 C（dev 版专属捷径）：控制台注册自定义 cheat

`cCheatManager`（`AddBuiltInCheats`/`RunCheat`/`RegisterShipCheat`）可用
Frida 注册一条 `reloadData` cheat 调 ReregisterData——仅 dev/研究构建适用，
作调试便利，不是交付路径。

## 3. 边界与待验证清单

1. **订阅侧零售存留**：`cEcoRulesFactory::HandleMessage`、`HotLoadTools`、
   9 处子系统 `HotLoad` 在 PL 的存留未逐一锚定（两个常量立即数未命中）。
   验证：Frida 调 `InitResourceHierarchyHotload` 后改一个 rules 记录，
   观察规则是否生效；不生效则需手动补发消息或直调子系统 HotLoad。
2. **已特化单元的刷新范围**：`kHotLoadUnit` 分发存在，但"已放置建筑的
   Unit 特化结果（资源 bin/规则实例）"是否随热加载重建，需一次放置前后
   对照实验；保底手段 = 重进城市（比重启游戏快一个量级）。
3. **缓存失效**：RW4 模型/纹理等已加载资源走工厂缓存；`FlushCache`/
   `ReloadResource` 存在但是否被热加载链调用未验。模型替换类测试可能
   仍需重进城。
4. **sim 状态与资源面的分界**：城市模拟状态来自服务器/存档，本地包热重载
   只改资源/属性/规则面；玩法数值验证需配合重进城或新放置。
5. **Windows 文件通知**：DDF 监视依赖 EA::IO 文件通知，网络盘/部分同步盘
   （OneDrive）目录可能收不到变更——hotload 目录应放本地盘。
6. dev RL（706228/707103）与零售 PL 的 `ReregisterData` 形态可能有小改
   （RL 时期服务器流程与 PL 离线模式差异）——锚定时以形态指纹为准。

## 4. 与工作台的集成点

- `mod_flow.rs build()` 增加产物输出目标 = hotload 目录（方案 A 的松散
  目录 / 方案 B 的包路径直写），配合 `payload_bin_grep` 类自检；
- 工作台"测试运行"按钮 = Frida 触发链（调用/验证/日志回显），复用
  `probes/runtime` 的 native_trace 基础设施；
- 验收：同一城市内改 prop bin → 不重启，新放置建筑即带新 prop；改
  Menu2 名称/locale → 菜单热刷新（`HotLoadTools` 路径）。

## 5. 证据文件

dev 源码树：`src/EA/ResourceMan/Manager/{RegisterDatabase,DoDatabaseChanged,
RegisterChangeNotification,FlushCache,ReloadResource,InternalDatabaseList}/`、
`src/EA/ResourceMan/DatabaseDirectoryFilesAutoUpdate/{Init,AddDDF,
FileChangeCallback}.c`、`src/SP/{InitResourceHierarchyHotload,
ShutdownResourceHierarchyHotload,ReadGlobalTypeList,AddPackageIfValid}.c`、
`src/global/{ReregisterData,UnregisterData}.c`、
`src/anonymous/OnDatabaseRecordChanged.c`、`src/SC/HotLoadTools.c`、
`src/SC/{cZoningGame,cGraphicsGame,cRoadManager,cPhysicsGame,cAudioGame,
cDisasterGame,cTornado,cTransportPipe,cGraphicsZone}/HotLoad.c`、
`src/GB/cEcoRulesFactory/HandleMessage.c`、
`src/GB/cUpdateManager/DataFinalizeFinishFullPacked.c`、
`src/SP/cCheatManager/`。
零售字节证据：PL EXE `GlobalMap.txt` UTF-16 @ `0x8fc2b8`（python 扫描，
2026-10-27）；既有零售 RVA 底账见 ctx knowledge（AddPackageIfValid
`RVA5EC40`、InternalDatabaseList Add `RVA511080`、Find `RVA510B40`）。
