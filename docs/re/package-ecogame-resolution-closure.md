# EcoGame 目录选择、版本表、挂接与缓存闭环

日期：2026-10-17。证据根 `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src`。本报告以 dev 带符号反编译树为依据，非原始源码；未打开主调查 IDA 数据库、未启动或注入游戏。下文末节已补充指定两个 retail EXE 的独立机器码对拍；其余未明确标明 retail 的结论仍限定 dev。

## 1. 两层选择，不是直接按 TGI 挂接全部 EcoGame 包

`GB/InitGameVersions.c`（VA 0x5ED7B0）先从已注册资源的 PropertyManager 枚举两类 Ecosystem group，建立 gameID → version → database 表；再扫描选定 EcoGame 根，将合格包加入表；最后调用 `SwitchToProductionVersions`，按选定版本实际挂接数据库。

普通 ResourceMan 的同 TGI priority 规则与 EcoGame 的 gameID/version 选择是不同阶段。外部 EcoGame 候选包被识别不代表当时已经注册到 ResourceManager，所有版本不会同时挂接。

## 2. 83 → 82 回退的精确条件

`GB/InitGameVersions.c:111` 取 `GetDirFromID(0x8F03C83)`；第 113 行仅在返回 `nullptr` 时才取 `0x8F03C82`。`SP/GetDirFromID.c`（0x4434D0）只查 `sDirectoryMap`：找不到 ID 返回 nullptr，找到即返回字符串 `mpBegin`。

因此此处条件是 **目录 ID 是否有映射**，不是目录 Exists，也不是字符串是否为空。若 83 已映射但路径无效/无包，本函数不再扫描 82；若映射值为空字符串，也不会由此分支触发回退。两项皆无映射则跳过整个扫描块，连块尾的 SwitchToProductionVersions 也不执行。路径确定后只对该根 `*.package` 调用 DirectoryIterator::Read；没有在此函数递归扫描、按版本排序文件或依次扫描两根。

82 默认注册路径的证据见 `GB/cEcoGame/Init.c`（0x5FB560）及 [目录链报告](package-directory-chain-evidence.md)：private 根/EcoGame，未映射才设置并 EnsureExists。83 的配置注册者仍需继续定位，不能仅凭读取点称其为固定实体目录。

本子调查随后独立读取两份 retail PE 的 `0x5C8DEE..0x5C8E52`，确认 83 返回指针与0比较，非零跳过82；证据详见末节。

## 3. 建表与同 game/version 冲突

### 已挂接的本地来源

`InitGameVersions.c:55–99` 从 PropertyManager 得到每个 group 的 ID；获取对应 Property 后，默认 version=0，若字段 `0x09EEB11C` 类型 tag=10，则取 UInt32 值。建立 `mVersions[version] = nullptr`；nullptr 表示使用本地数据库集合，不是无效版本。

同时调用 `anonymous/GetLocalSimDatabases.c`（0x5ECB90）：以 gameID / Property type / Ecosystem group 调 `FindDatabase` 找当前命中来源，从其 location 读 `Sim_DDFList.txt`，再按已有数据库列表筛选 location 在 DDF 集合里的数据库，保存为 `mLocalDatabases`。这不是把所有含相同 gameID 的 package 都收进来；也不是直接把 DDF 行顺序等同注册顺序。

这段 Init 本身没有更新 mLatestVersion。后续查到 `eastl/anonymous/DoAllocateNodeFromKey.c`（dev 0x5EC1E0，第37–38行）明确把新节点 current/latest 都初始化为0。因此全新本地条目即使 metadata 记录别的数字版本，也不会由这段代码自动抬高 latest；默认 production 请求0且会命中 current=0 的短路，保留既有本地挂接。此初始化结论尚未单独对拍retail。

### 扫描得到的外部版本包

`anonymous/AddGamePackage.c`（0x5ED2B0）先 Open，顺序搜索 `kEcosystemGroup` 与 `+1`；首个有候选的 group 停止。只消费该 group 的首个匹配 key，gameID 为 key.instance；要求 Property `0x09EEB11C` 为 UInt32（tag 10），然后取真正版本数值。这里不是 version 必须等于10，也不是多条候选即拒绝。

`mVersions[version]` 被赋为当前 database，替换并 Release 原引用。因此在同 script type / gameID / version 的扫描竞争中，**后一次成功 AddGamePackage 的数据库替换版本表条目**。这与普通 ResourceMan 同 priority 先注册优先并不冲突：两个规则作用的表不同。

在第一次插入 gameID 或 version 大于已有 mLatestVersion 时更新 latest；不同版本按数值比较，不按文件名比较。函数完成后 Close database；它持有版本表引用，但未在这里调用 ResourceManager::RegisterDatabase。

注意：版本表后写胜出不保证当前挂接立即切换，同版本短路与脚本缓存见下文。

## 4. 无效候选的删除分支

`InitGameVersions.c:177–180`：AddGamePackage 返回 false 时先 Shutdown，再 `EA::IO::File::Remove(fullPath)`，最后 Release。失败包括数据库不能 Open、找不到 Ecosystem Property、首条 Property 不能读取、缺少版本或类型不是 UInt32。

此代码没有检查 Remove 返回值，所以准确描述是“尝试删除”，不是保证文件已删除。它作用于选定 EcoGame 扫描根，不可外推为所有普通 package 都会被删除。校验服务不得把最小普通 overlay 放进此类根做未经隔离的探测。本轮未执行该分支。

## 5. 真正挂接与版本切换

`GB/SwitchToProductionVersions.c`（0x5EC5F0）遍历两类版本表，对每个 gameID 调 SwitchToScriptsVersion(gameID, 节点 i[12], scriptType)。`anonymous/cGameVersionInfo/cGameVersionInfo.c:52–53` 的复制构造可核对布局：对象+40 为 current、+44 为 latest；节点 key 额外占4字节，因此 i[12] 即 mLatestVersion。这里 production 表示选择记录的 latest，而不是再按文件名寻找 Production 包。

`GB/SwitchToScriptsVersion.c`（0x5EBCB0）流程：

1. 找不到 gameID 返回 false。
2. 请求版本等于 current，立即 true；在此之前尚未检查目标数据库是否替换。
3. mVersions 查找精确版本，不存在返回 false。lower_bound 后还检查相等，不会自动取邻近版本。
4. 旧外部数据库存在：枚举其 Property keys，RemovePropertyLists、取消注册数据库、Close；旧来源为本地集合时，对每个本地数据库清 Property 并取消注册。
5. UnregisterResource(gameID, type `0x08068AE9`, Ecosystem group+scriptType)，调用 FlushAllCaches(1)。
6. 保存新 database 引用与 currentVersion。目标非 null 时 Open；失败清注册引用、置 current=-1、返回 false，不自动恢复旧挂接。成功调用 RegisterDatabase(true, target, **100**) 并返回 true。
7. 目标 null 时将本地集合各数据库 RegisterDatabase(true, local, **2**) 并返回 true。

100/2 是此 dev 函数的优先级数值；retail 必须单独对拍，不能根据普通 SetupResources 的版本差异推定一致。RegisterDatabase 返回值在此未检查，true 不是 Attach 成功或最终脚本初始化成功的充分证据。SwitchToProductionVersions 也没有处理各次切换的返回值。

实际保存游戏可要求非 latest：`GB/cEcoGame/Read.c:91–99,147` 读取保存的 gameScriptsVersion（或显式 asVersion），随后 SwitchToScriptsVersion；add-on 同样指定其保存版本。不存在所需版本时加载失败，不能概括为始终用最新脚本。

## 6. 缓存层与同版本重打包边界

### Property 与通用资源

`SP/cPropertyManager/RemovePropertyLists__2.c`（0x4A8F20）对旧数据库 Property keys 尝试取已载入资源并 UnregisterResource，随后从 mPropertyLists 按 group+instance 删除。

`SP/FlushAllCaches.c`（0x470620）按 depth 对 registry FlushCache 和所有 cLRUCache::Flush 执行清理。`anonymous/cLRUCache/Flush.c`（0x4701B0）只在资源引用计数条件满足时注销 registry 中对象，并清空自身缓存槽；不能据函数名宣称所有被外部持有的对象都销毁或自动重建。

### 已构建的 EcoGameScripts

`GB/GetCurrentGameScripts.c`（0x5ED4E0）会先尝试 `anonymous/FindLoadedScripts.c`（0x5EB640），匹配 gameID、currentVersion、add-on 数量、**有序** addOnIDs 及 addOnVersions。匹配键不含包路径、时间戳或内容 hash。命中直接复用已构建脚本对象；不命中才 FlushLoadedScripts(reserveSpace=1)、创建并 Init 新对象。

`anonymous/FlushLoadedScripts.c`（0x5ECA70）只是容量压力淘汰，只有超过 cache size 时才处理，并保留仍有外部引用的对象；不是每次调用清空全部脚本。

故即使版本表同键条目已换成新包，同版本 Switch 的 early return 与已载入脚本缓存都可能保留旧行为。应区分“版本表候选替换”“数据库挂接替换”“脚本对象重新构建”三件事。不能把磁盘覆盖/重新枚举当成有效热重载。

`GB/ShutdownGameVersions.c`（0x5ECE50）对 sLoadedScripts 调 Shutdown/Release、清集合，并清版本表；已有外部挂接时尝试切换版本0。`GB/cUpdateManager/GameFinalize.c:87–89`、`CleanupGameDownloads.c:172,234` 采用 ShutdownGameVersions → InitGameVersions 的重建路线。这里证明存在显式生命周期重建，不建议对正在游玩的进程主动调用这些函数。

## 7. 闭合状态与观察字段

| 问题 | 本轮状态 |
|---|---|
| 83/82 回退依据 | dev 已闭合：ID 查找 null；主调查另有 retail 同型指令 |
| 同 game/version 谁入表 | dev 已闭合：后成功写入替换引用 |
| 多版本默认选谁 | dev 已闭合：SwitchToProductionVersions 用 mLatestVersion；保存加载可显式请求旧版 |
| 何时挂接 | dev 已闭合：SwitchToScriptsVersion，非 AddGamePackage |
| Property tag10 含义 | dev 已闭合：UInt32 类型 |
| 无效包删除 | dev 已闭合：Shutdown 后尝试 File::Remove，不保证成功 |
| 缓存与同版本替换 | dev 已闭合短路和对象缓存键；实际运行效果未采样 |
| 本地初始 latest/current | dev节点分配已确认均为0；retail分配器待对拍 |
| 83 注册者、retail完整缓存链 | 仍须独立补证 |

最小观察事件应包括：resolved dirID/path/null 状态、枚举序号、候选 group/gameID/version、替换的版本表数据库、请求版本与 current/latest、是否走相同版本短路、旧/新挂接及 priority、Open/Register 返回、缓存命中或新脚本 Init。只有文件打开事件不足以回答本专题。

## 8. Retail 独立机器码对拍：force 参数、priority 与 Ecosystem group

样本与已验证完整摘要：

- `D:/ea-games/simcity_offline/SimCity_dump_SCY.exe` SHA-256 `06d681444011f1297a6cf53bc233ee5f138d41786b451e6f18dc02bf273ff569`。
- `D:/ea-games/simcity_offline/SimCity.exe` SHA-256 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。

本子调查使用现有 `tools/package_loader_probe.py::PE` 读取并限制已知hash，独立按节映射取字节，以已安装 `tmp/re-python` 的 Capstone 解码。证据文件：`tmp/package-loader-20261017/ecogame-switch-disasm.txt`、`ecogame-switch-verify.json`。未打开 IDA 数据库，未执行目标代码。

| retail VA（RVA=VA−0x400000） | 范围长度（十进制） | 对拍结果 |
|---|---:|---|
| SwitchToScriptsVersion 0x5C6D70 | 728 | 两份 EXE 字节一致 |
| SwitchToProductionVersions 0x5C7740 | 119 | 两份 EXE 字节一致 |
| AddGamePackage 0x5C86B0 | 549 | 两份 EXE 字节一致 |
| 目录回退 0x5C8DEE | 100 | 两份 EXE 字节一致 |
| 无效包处理/production调用 0x5C9135 | 98 | 两份 EXE 字节一致 |
| Ecosystem group 全局 0xDF56C0 | 4 | 两份 EXE 字节一致，值 0x40800200 |

这只是所列范围一致，不表示两个 EXE 全量相同或已经实际运行。

**retail 比 dev 多一个跳过当前版本短路的参数。** `SwitchToScriptsVersion` 入口 `0x5C6DA8` 检查第4参数 byte `[esp+0x50]`：非零跳 `0x5C6DBF`，直接继续版本查表/切换；等于0才在 `0x5C6DB3` 比较请求版本与节点 current `[esi+0x2C]`，相等返回true。这是行为上 force-reload-like 参数，原始源码名未知；它仅明确绕过此短路，不应称为保证所有缓存全部重建。

`SwitchToProductionVersions` 在 `0x5C777B` 取节点 `+0x30` latest，`0x5C7780 push 0` 为第4参数，`0x5C7785 call 0x5C6D70`。因此默认 production 切换仍会相同版本短路；“零售完全不能强制同版本重挂接”的说法则不成立。其余 force=true 调用者仍待追踪。

外部数据库挂接：`0x5C6FFC push 0x64`、`0x5C6FFF push 1`、`0x5C7003 call manager vtable+0x4C`。本地数据库挂接：`0x5C702B push 2`、`0x5C702E push 1`、`0x5C7032 call manager vtable+0x4C`。**外部100/本地2在两个指定retail构建中都保留。** 该函数也保留外部Open失败将current设为-1，以及忽略RegisterDatabase返回值的分支。

`AddGamePackage` 的 `0x5C8701` 从全局 `0xDF56C0` 取 group，`0x5C870B` 加 i，i在0/1循环；该全局映射文件 offset `0x9F44C0` 值为 **0x40800200**。所以两类 group 为 `0x40800200`、`0x40800201`，type 仍为 `0x00B1B104`。

`0x5C8791` 压入版本字段 `0x09EEB11C`，`0x5C87A8` 比较 Variant+0x12 的类型为0xA，之后 `0x5C87B3` 转换并读取实际 UInt32；再次确认10不是版本值。`0x5C8808 mov [ebp],esi` 替换版本表数据库引用，`0x5C882B/2E` 比较latest、版本不大于旧值时不更新，`0x5C8834` 写新latest；同键后成功写入替换规则也已得到retail支持。

目录回退区间 `0x5C8DEE push 0x8F03C83`，返回指针移到esi，`0x5C8E03 cmp esi,edi`（edi为0）、`0x5C8E05 jne 0x5C8E52`；空指针才调用82。无效包段 `0x5C9136 call 0x5C86B0`，返回false则调用数据库Shutdown再在`0x5C9153`调用 File::Remove 候选 `0x8DB320`；末尾 `0x5C9192` 调 production。Remove对应名称由dev控制流映射，尚未独立反编译0x8DB320，但失败即尝试该文件路径操作的分支已确认。

更新后的边界：目录回退、候选版本替换、版本字段类型、production current短路、force-like参数、100/2挂接优先级、Ecosystem group实值均已获两个retail文件局部机器码支持；已构建scripts缓存与完整Shutdown生命周期仍主要为dev证据。不能把这些静态结果报告成已完成运行时热重载验收。
