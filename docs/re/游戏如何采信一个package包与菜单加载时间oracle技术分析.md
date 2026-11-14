# 游戏如何采信一个 package 包与菜单加载时间 oracle 技术分析

日期：2026-11-08  
证据：`SimCity2013-source-tree` 反编译源码、零售 PDB/IDA 资产、现有静态探针与构建账本。本文不启动或修改游戏。

## 结论摘要

“秒进菜单 = 包没问题，卡菜单加载 = 包有问题”是一个非常有价值、且在当前测试基线下可重复的工程 oracle，但严格含义应是：**包通过了启动阶段的 DBPF/ResourceMan 注册和随后菜单/基础资源索引工作**。它不是数学意义上的“所有建筑行为都正确”，也不是单独的 DBPF 合法性证明。

反编译代码能解释这个 oracle 的来源：启动时游戏会枚举目录中的 `*.package`，打开 DBPF，检查 header，调用 `SP::AddPackageIfValid` 检查 package-info/property-list/signature/entitlement/bless 条件，然后按 priority 注册 Database。之后 `FindDatabase` 按注册链首个命中 TGI，菜单和 EcoGame 初始化会触发大量 property/resource 查询。坏包可能在第一阶段被拒绝，也可能在第二阶段因资源索引、Property、locale、Menu2、Unit/Bundle 扫描异常而让主线程长时间等待或反复重试。

## 1. 启动采信链

### 1.1 目录枚举和打开

`src/anonymous/RegisterPackages.c` 调用 `EA::IO::DirectoryIterator::Read`，过滤模式为 `L"*.package"`，最大结果数 `0x100000`；对每个文件创建 `EA::ResourceMan::DatabasePackedFile`，执行 `Init → SetLocation → Open`，Open 成功后调用 `SP::AddPackageIfValid(database, priority)`。

因此文件名、目录位置和扩展名直接决定包是否进入候选集。`SP::SetupResources.c` 对基础数据目录注册 priority 0，另有内存 Preload、Locale 和 prop 目录；`cLocaleManager::RegisterLocaleDatabases` 又按 locale 目录注册 embedded/locale package，并使用 0、-1000 等优先级。

### 1.2 DBPF header 的第一道门

`EA::ResourceMan::DatabasePackedFile::VerifyHeaderRecordIntegrity` 检查：

- file identifier 必须是 `DBPF`（反编译常量 `1179664964`）；
- major version ≤ 3；
- index entry count 小于 `0x7FFFFFF`；
- index position/size 必须落在文件大小内；
- key type 非零；
- 低/高位 index position 一致且高位为 0。

这解释了真正损坏、截断、索引越界的 package 会在 Open/Attach 阶段失败，通常表现为启动时明显停顿或直接跳过包。

### 1.3 `AddPackageIfValid` 的第二道门

`src/SP/AddPackageIfValid__2.c` 负责从路径创建并打开 Database；核心逻辑在 `src/SP/AddPackageIfValid.c`：

1. 以 `kPackageInfoGroup` 建立 KeyFilter，要求 package-info 资源数量为 0 或恰好 1；
2. 打开 package-info record，读取 `kMaxisPublicKey` stream；
3. 调 `PropertyListHasValidSignature`；签名失败直接 invalid；
4. 解析 property list，读取 package id、capabilities、entitlement 检查；
5. 对需要 Bless 的包再打开 Bless record，验证公钥签名、registry/product/path fingerprint 和注册表 `BlessID`；
6. valid 时调用 `ResourceManager()->RegisterDatabase(..., priority)`，并发布 package message；invalid 时关闭/shutdown database，并清理已登记 addon 状态。

这说明“包能被游戏采信”的最低静态条件不是只有 `DBPF` magic，而是 package-info 数量/格式、签名和可选授权链均能通过。

## 2. 注册后如何解析资源

### 2.1 priority 与首命中

`EA::ResourceMan::Manager::RegisterDatabase` 把 Database 加入内部链表并 Attach；`Manager::FindDatabase` 从链表头开始，对每个 Database 调 `OpenRecord(key, ...)`，第一个成功者立即返回。

这意味着：

- 同一 TGI 不会做字段级合并；生效的是优先级/注册顺序上的首个完整 resource；
- 一个错误的高优先级 overlay 可以遮蔽正确的原版资源；
- “包内资源存在”不等于“游戏实际读到它”，必须确认获胜 Database。

### 2.2 按需加载与缓存

`Manager::GetResourceInternal` 先尝试 `CheckCacheInternal`；缓存未命中时调用 `FindDatabase`，再 `OpenRecord`、查 Factory、创建 Resource、Flush，并按需要写入 cache。`CheckCacheInternal` 在存在异步请求时会 `WaitForResource`。

因此启动菜单期间，如果某个 Menu2、locale、Property、RW4、raster 或 factory resource 触发同步等待，用户看到的就是菜单长时间不出现；这不必然是死循环，也可能是包导致的反复失败/重试或大规模索引扫描。

## 3. 菜单为什么会成为便宜 oracle

菜单不是单个 UI 图片。一个建筑菜单入口会牵引：Menu2 → locale text → icon/marquee → menuUnit → Unit → Lot → model/material/LOD，并可能触发 EcoGame 的 property group、category、bundle 和统计模板解析。`GB::cEcoGameScripts::AddEntitiesFromGroupIDs` 明确从主表的 Unit/Map/Zone/Transport group 读取 group IDs，再通过 PropertyManager 的 `GetPropertyListIDs` 枚举全部 property list，构造 Unit/Map/Zone/Transport entries；`cEcoGameScripts::Init` 随后加载规则集、specialise global/unit rules、添加 base entities 和 bundles。

换句话说，主菜单初始化是一个低成本的全局索引压力测试：它会尽早触发“包注册成功但资源图不闭合”的问题，而无需进入城市或污染存档。

## 4. “秒进 = 合法”需要怎样表述

可以严格表述为四层：

| 层 | 秒进菜单能证明什么 | 不能证明什么 |
|---|---|---|
| 容器 | DBPF header/index/Open 基本可读 | 每条 resource payload 都正确 |
| 注册 | package-info、签名/授权门通过，Database 已注册 | 该包的每个 TGI 都是获胜版本 |
| 菜单/基础索引 | 启动期间关键 Menu2/locale/Property/Factory 查询未造成明显阻塞 | 建筑放置、模拟规则、道路/水电连接正确 |
| 运行时 | 无直接证明 | Unit bins、sink、ERZ、BoundingBox、LOD、材质 shader、存档重载 |

同理，“卡菜单”是强烈的失败信号，但不能仅凭时间定位根因。可能原因包括 DBPF/index/header、package-info/signature、同 TGI 遮蔽、Property flags/数组布局、locale JSON 形状、Menu2→Unit→Asset 闭包、RW4/raster 解码、超大资源导致的同步 I/O，或 GraphicsCache 污染。

## 5. 能否从反编译代码“破获”这一机制

能，且已经破获到足以指导构建器门控的程度：

1. `RegisterPackages` 证明游戏启动时批量枚举 package，而不是只在进入城市时发现错误；
2. `AddPackageIfValid` 证明存在明确的整包拒绝路径（package-info、签名、entitlement/bless）；
3. `RegisterDatabase`/`FindDatabase` 证明 TGI 首命中和 priority 遮蔽；
4. `GetResourceInternal`/`CheckCacheInternal` 证明菜单资源解析可能同步等待异步请求；
5. `cEcoGameScripts::Init`/`AddEntitiesFromGroupIDs` 证明菜单阶段会继续触发 Unit/Bundle/规则库扫描；
6. `RegisterLocaleDatabases` 证明 locale package 也是独立 Database，且会按语言目录和 priority 注册。

反编译代码不能单独给出“卡了 3.2 秒就是哪一个 TGI”的答案；要做到精确归因，需要对零售 exe 的 `RegisterPackages`、`AddPackageIfValid`、`FindDatabase`、`GetResourceInternal` 做动态 hook/trace，记录文件、priority、TGI、OpenRecord 结果和等待耗时。

## 6. 推荐的工程验收顺序

1. 构建器生成 ledger，记录 schema/build/source hash、TGI 数量、依赖闭包和 warning/error。
2. 按用户规则核对 build ledger 与 `test-archive/MANIFEST.md`，对参照包和部署包分别 SHA-256。
3. 清理旧测试包并归档登记；新包使用新 instance，避免 same-TGI 和存档污染。
4. 冷启动游戏，只测主菜单加载时间；卡住即停止，不进城市、不放置、不保存。
5. 秒进后再做 disposable city 的菜单可见、放置、渲染、模拟、保存/重载分层测试。
6. 若渲染异常但菜单秒进，先隔离 `GraphicsCache.package`，不要立刻改写 package 数据。

## 7. 对构建器的直接含义

构建器应该输出两个独立 verdict：

- `package-admission`: DBPF/header/package-info/signature/索引层可注册；
- `game-asset-readiness`: Menu2/locale/Unit/Lot/Bundle/Path/Material/LOD/行为闭包与 placement/runtime contract 完整。

“秒进菜单”只能把第一项和部分第二项标为通过；不能把它当成完整资产验收。反过来，菜单卡顿应作为阻断信号，因为它意味着继续进入城市的成本高且有污染存档风险。

本轮后端已经把这条边界落实为构建器阶段：`material-binding-contract` 负责编码产物的 RW4→raster 闭包，`menu-contract`/`layout-contract` 负责身份和放置字段，`GeometryRoute` 负责从 LOTM 事实推导模型路线。它们分别对应“可注册”“可解析”“可放置”的不同层，不再用一个 `build succeeded` 状态代替游戏侧语义。

三条真实样本的离线结果已作为固定验收面：商业建筑 98/98、大学 71/71、希望大厦 66/66 mesh/material binding 全匹配；普通菜单 Unit 也必须包含 `0x0AFE27B1` placement family，这属于解释器补齐的游戏侧语义，而非前端输入字段。

截至 2026-11-08，`asset_build_m1` 全部离线测试为 148 passed、0 failed、22 ignored。该结果只证明构建器的资源图和契约检查通过；游戏冷启动、菜单加载时间和 disposable city 放置仍必须由用户手动执行，本工具链不会自动启动游戏。

菜单解释不再读取 Asset Editor 的 `enabled` 伪开关。菜单是否进入构建只由配置对象及其类别/父级/标题等字段决定；这避免 UI 没有该控制项时出现代码级静默关闭。大学 s31 仍沿用手工 schema，不纳入工作台逻辑覆盖。

商业地标没有 M1 schema 时，一键构建现在走 Asset Editor workbench 适配器：已有 schema 优先；无 schema 才从 `asset.lot.json` 推导场景、菜单、劳工和行为，并从 sourceLot 游戏包中选择真实 `58F0C000` lot-config donor。`41F0C000` 只作为 sourceLot 事实记录，不被直接克隆成新菜单身份；源包副本不一致时构建器拒绝继续。这样 UI 只提供可理解的 IR，游戏侧 donor、菜单身份和闭包由解释器补齐。

## 8. 证据边界

本文引用的是反编译源码中的静态控制流和现有离线证据，没有把未做动态 hook 的等待点写成已证实的单一根因。要完成“100% 必现”机制的最终定案，应在固定 exe 哈希、固定 package 集合、冷启动和无缓存条件下重复测量，并保存每次的启动时间、日志、包 hash 和数据库注册顺序。

## 9. 与希望大厦案例的交叉验证

希望包的账本显示 `resource_count=137`、`material_resources=119`，说明构建器确实写出了大量资源；但这不能推出这些资源都在模型的 MATERIAL 行中被引用。该包同时有 `modelShape=null` 和 66 材质组，后端自动 NH 分支会把模型材质行归零到头材质。于是可能出现两种完全不同的结果：

* package admission 通过、菜单索引可读，但渲染使用错误的单一材质；
* 某个被归零后的 MATERIAL 行指向空/不匹配资源，触发启动阶段的同步解析或重试，表现为菜单卡顿。

因此“秒进菜单”仍是高价值的阻断 oracle，但它不能证明希望模型的 66 组材质已被正确采信；需要在 oracle 通过后再做 RW4 MATERIAL 行到 raster/RW4 TGI 的逐行闭包检查。反之，菜单卡顿只证明启动阶段的资源图没有在给定包集合和缓存状态下顺利完成，不能把根因直接归咎于 `ny_facade` 插件。
