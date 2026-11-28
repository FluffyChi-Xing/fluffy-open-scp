# OpenSCP 游戏内重载：零售版入口与工作台可行性

日期：2026-11-15。结论：**指定资源的免重启重载有零售引擎入口，值得做实验；任意 package 和已放置建筑的无条件热替换尚未成立。** 本次只读取本地文件，未附加、启动游戏或改动包和存档。

## 证据身份

开发版源证据来自用户提供的 `Simcity Development Files Including PDB Debug files` 与同级 `SimCity2013-source-tree`。后者是带符号的反编译 C，类型和参数名可能有误，地址属于开发版。PDB 信息流 GUID `03ed6698-4760-472c-9f1e-67face23f7cc`、Age 1，与同目录开发 EXE 的 RSDS 一致；PDB 含 `ReregisterData`、`ReloadResource`、`HotLoadTools` 符号串。

| 样本 | SHA-256 |
| --- | --- |
| 开发 EXE | `fe4651fdc9c322947c42217d36148bd4a1e61eb1dd48596995b97a3b2ea185bd` |
| 开发 PDB | `1ad5ea47d1230998e532dfc90966e0ca41c4ce7f9d852e206305b2cf5001f952` |
| 实际安装 `SimCity/SimCity.exe` | `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab` |
| 零售转储 `SimCity_dump_SCY.exe` | `06d681444011f1297a6cf53bc233ee5f138d41786b451e6f18dc02bf273ff569` |

安装版和转储的下列证据区间按 PE RVA 读取后逐字节一致。使用 PE 节表、Capstone 和开发版函数体/虚表槽交叉核对，零售名称是分析赋名。基址 `0x400000`。区间用于复核证据，部分仅覆盖函数前段。

| 入口 | 零售 RVA / 比对长度 | 识别证据 |
| --- | --- | --- |
| `SP::ResourceManager` | `0x5C30 / 0x6` | 返回全局 manager；由热加载入口与变更处理器调用 |
| `InitResourceHierarchyHotload` | `0x5E8F0 / 0xEA` | 引用 `GlobalMap.txt`，安装 DDF 监视器，传入回调至虚表 `+0x3C` |
| `OnDatabaseRecordChanged` | `0x5E390 / 0xA2` | 构造含 key、数据库、context 的消息，发送 `0xF62ADF` |
| `ReregisterData` | `0x196990 / 0xCA` | metadata 查库、FourCC 校验、遍历数据库虚表 `+0x18` 调用 `Open(1,6,false)`，再调热加载入口 |
| `Manager::DoDatabaseChanged` | `0x511330 / 0xBB` | 虚表 `+0x60`；校验数据库已注册，逐 key 查优先命中源，再调回调 |
| `Manager::ReloadResource` | `0x511E50 / 0x70` | 虚表 `+0x18`；查库及记录类型映射，读取到传入的资源对象 |
| `cAppSystem::HandleMessage` | `0x17FC0 / 0x238` | 匹配 `0xF62ADF`；处理缓存，调用 `ReloadResource` 虚表槽，发送前后消息 `0xF62ADD/DE` |
| `DatabasePackedFile::Open` | `0x4E8C70 / 0x30` | 普通打包库虚表 `+0x18`；对象 `+0x14` 非零立即返回 false |
| `RegisterMemoryPackage` | `0x5DEB0 / 0x157` | 启动时调用，加载文件字节到内存流，再开库注册。适合参照其所有权做法；现有 helper 不返回可管理的库句柄 |
| `Manager` 虚表 | `0x92C138 / 0x78` | `+0x4C` 已验证的 RegisterDatabase；`+0x54` 已验证的 FindDatabase；用于定位其余槽 |

## 三个不同的生效层

1. **数据库**：普通包启动路径确实扫描 `RegisterPackages`，但 `RegisterDatabase(bool,...)` 本身具备注册和注销。开发版 `Manager/RegisterDatabase.c:29–64` 在挂接成功确认前先插入列表；不能只凭 helper 的返回值推断最终命中或清理完成。它也不会扫描新包。可用独立代际数据库承载工作台新产物，运行中不替换原文件；每次必须验证目标 TGI 实际命中。
2. **资源对象**：开发版 `DatabaseDirectoryFilesAutoUpdate/FileChangeCallback.c:99,119` 把松散文件的变更 key 送进 `DoDatabaseChanged`；零售侧确认通知、消息与单资源重读仍在。通知要求库已注册、且该 key 当前未被其他库抢先命中；`ReloadResource` 要有正确的资源对象、工厂和上下文，它不是“按文件名刷整个包”。参见开发版 `Manager/DoDatabaseChanged.c:27–60`、`Manager/ReloadResource.c:36–61,102–114` 与 `SP/cAppSystem/HandleMessage.c:45–135`。
3. **玩法和显示**：开发版 `SC/HotLoadTools.c:44–55` 只遍历已有工具并按 instance 调 `FillFromProps`，没有创建新按钮；`SC/cGameUI/HandleMessage.c:525–545` 是该消费者的开发版证据。新菜单项、Unit 列表或索引、已放置对象、RW4/纹理和 EcoGame 脚本都需要各自的消费方证据和验收；资源重读成功不能替它们背书。

旧研究的两个修正：

- [10 月 26 日概述](package-hot-reload-research-20261026.md) 的“没有可验证的公开入口”不排除零售内部入口；本文已经定位内部入口。其自动启动方案也不符合当前仅允许用户启动游戏的约束。
- [10 月 27 日提案](package-hotload-feasibility-20261027.md) 的“全库 Open 等于覆盖生效”不能用于普通包。开发版 `global/ReregisterData.c:26–43` 和零售循环都只接受已能解析的特定下载库，并忽略逐库 Open 的结果；开发版 `DatabasePackedFile/Open.c:25–26` 和零售指令都证实已开的库会拒绝 Open。该路径也不提供任意包失效和消费者刷新保证。零售消息常量确实存在；这不证明所有开发版订阅者仍在或已注册。

## 工作台落地建议

未来入口放在 Develop 顶栏 `CodeWorkbench.vue` 的「构建并启动」右侧。本阶段只设计和验证后台。资产项目的「构建」走 `build_queue_enqueue_project`，成功的 `BuildJob.result` 已带包路径和 SHA-256。现有 `buildDeployAndLaunch` 通过 `flowPanel.buildArtifact()` 获取产物，资产项目接入时应使用同一个后台构建结果。两条操作都根据明确的构建任务 ID 取包，避免多次提交时挑到别的任务或旧产物。

「构建并启动」负责构建、部署，以及根据进程状态启动游戏。现有 `deploy_game_mod` 写入 `SimCityUserData/Packages`，资产包应由后续接入按包类别选择已验证的目的地。点击「重载」建议使用最近一次成功的构建产物，用户可重复应用已有构建，后台也能记录当前游戏进程已应用到哪一版。此按钮收到的输入是项目、构建任务 ID 和目标 PID。Tauri 校验构建记录、产物哈希、进程路径和版本，再把任务送给进程内助手。按钮最后可显示实际观测到的阶段结果。

落实到引擎侧，先把两版 DBPF 按 TGI 比较，比较解压后的记录内容，过滤纯压缩或布局差异。用自有菜单工具的单条属性记录做最小实验。游戏线程上从新一代数据库读取字节，再走 `ReloadResource` 和原生消息链。候选方案是在启动加载的测试包上方登记一个仅含本轮变化记录的临时内存数据库。登记前后查询 `FindDatabase` 并核对记录内容，确认临时库只取得项目自有 TGI 的优先命中。引用和回调仍在使用旧库时保留其寿命。A→B→A 使用固定的测试资产身份，验证回切后的命中源和实际属性。临时数据库的生命周期、游戏线程调度点和并发资源读取仍需活体验证。Frida 可用于采集入口参数、线程与结果。

本轮将首个实验目标补到零售版消费端。已安装 EXE 和转储中的下列指令一致：

| 环节 | 零售 VA（基址 `0x400000`） | 可观测结果 |
| --- | --- | --- |
| `cAppSystem::HandleMessage` | `0x417FC0` | 处理 `0xF62ADF`，调用资源重读，成功后发送 `0xF62ADE` |
| `cPropertyManager::HandleMessage` | `0x4643E0` | 匹配 `0xF62ADE` 和 Property 类型 `0x00B1B104`，更新已缓存属性的父链，发送 `0xF62DEF` |
| `cGameUI::HandleMessage` 内的属性处理分支 | `0x7727A2` | 形态与开发版相同，处理 UI 消息、控制层并调用 `HotLoadTools` |
| `HotLoadTools` | `0x89C370` | 核对工具组和实例 ID 后调用已有工具的 `FillFromProps`，预期只影响匹配的项 |

开发版 `SP/GetExternalFileAsResource.c:95–144` 还提供单文件的手动重读先例。零售对应片段 `0x45E729` 直接调用 `ReloadResource`，随后发送 `0xF62ADE`，中间未判断重读返回值。实验必须单独记录重读结果，不能仅凭消息判成功。开发版 `SP/cAppSystem/PostInputUpdate.c` 末尾推进 `mReloadTick`，可作为查找帧间调度点的线索。这里仍需验证零售调度点、线程和可安全切换数据库的条件。

1. **只读探针**：用户手动启动本机离线版。验证进程映像哈希、入口字节、活体 manager/虚表、消息订阅和命中源；定位游戏自身的调度点，确认可执行重载的线程和资源读取静止条件。不得使用开发版 RVA 或由注入线程直接调用。找不到安全时机就拒绝变更。
2. **单资源闭环**：按既有 recipe 建立独立的新测试菜单项，先做用户启动的冷启动菜单检查。在已备份存档的隔离会话及一次性城市里，对照构建 A/B，只改这项测试资产的一个**已存在、经验证可观察**的 UI 属性记录；使用独立 staging 产物，两版 TGI 集合、菜单身份、单位索引保持不变。先验证新的库/资源字节被读到，再沿原生变更通知验证对象刷新，最后验收屏幕实际变化。不要同 TGI 覆盖原版资源，不改健康存档对应的包集合。初次部署若涉及安装目录，按 archive 脚本与清单规则处理。
3. **重复与失败**：同一 PID 验证 A→B→A；无崩溃、无重复注册、没有残留回调指向已释放数据库。旧数据库的句柄与引用寿命需独立验证；逻辑回切只能恢复已验收的资源类型，不能承诺撤销城市状态。验证失败应明确区分“已注册”“已命中”“已重读”和“画面已生效”。若新产物增删 TGI、改变菜单注册或 Unit 身份，首版拒绝热重载，保留产物等待用户下次启动。

阶段目标是同一游戏进程内反复迭代已经登记的开发资产。动态新增菜单、已有建筑实时替换、地图和模拟规则另行定项，逐类通过游戏内验收后再开放。

## 初版离线探针与消息队列

`probes/runtime/package_reload_prepare.rs` 已实现离线准备步骤。将同一测试资产的 A、B 包及菜单 instance 交给 CLI，输出文件使用项目 staging 目录下的新路径：

```powershell
cargo run -p probes --example package_reload_prepare -- A.package B.package MENU_INSTANCE_HEX delta.package
```

CLI 比较解压后的记录，拒绝重复或增删 TGI、其他资源变化，以及指定 Menu2 以外的变化。当前只接受一处既有标题或描述的文本引用改动。A、B 应事先包含相同的 locale 内容，两个备选文本需在冷启动前已存在。这一限制用于隔离菜单属性刷新与 locale 缓存刷新。CLI 输出两个输入和差量包的 SHA-256，以及目标 TGI 和属性哈希。输出的单记录 DBPF 是进程内试验的输入，不能当作独立注册资产部署。离线测试已验证 A→B→A 的记录字节、拒绝附带修改和拒绝覆盖输出。

开发版 `anonymous/OnDatabaseRecordChanged.c:42–49` 通过 `MessagePost` 投递 `0xF62ADF`。零售回调 `0x45E390` 在 `0x45E41F` 同样调用消息服务器虚表 `+0x18`（`MessagePost`）。开发版 `SC/cSimCityApp/PreInputTick.c:78–80` 在输入更新前调用队列处理。零售 `0x40185F–0x40186C` 有相同的 `MessageServer` → 虚表 `+0x38` 调用，安装 EXE 和转储的该片段逐字节一致。该调用点可用来记录游戏线程，但单靠这段静态指令还不能确认线程、帧阶段或并发读取已经静止。

开发版 `EA/Messaging/Server/MessagePostFunction.c:23–46` 和 `ProcessQueue.c:91–114` 还揭示一个调度候选。消息队列存储函数和 context，出队后在释放队列锁的情况下调用函数。启用引用计数时，入队会先传入 ID 1，出队运行后会再传入 ID 2；若将来用这条路径，进程内助手要区分这两个生命周期回调与重载请求，并让 context 和数据库引用存活到处理完成。待确认零售对应实现、活体虚表和队列所在的线程后，再选择该机制或使用已验证的帧调用点。
