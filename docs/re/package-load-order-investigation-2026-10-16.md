# Package 加载顺序：下一阶段范围与初查

日期：2026-10-16。承接 `next-reverse-engineering-roadmap.md` 的 P1 package 项，按本次需求优先调查加载顺序。本轮为范围确认与静态初查，尚未完成 retail IDA 对拍或运行时采样。

## 样本与证据边界

- dev 证据根：`D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src`。这是带符号的反编译树，不是原始 C++ 源码；类型恢复和 xref 注释可能不完整。
- retail dump：`D:\ea-games\simcity_offline\SimCity_dump_SCY.exe`，SHA-256 `06D681444011F1297A6CF53BC233EE5F138D41786B451E6F18DC02BF273FF569`。
- 当前离线 EXE：`D:\ea-games\simcity_offline\SimCity.exe`，SHA-256 `2B0AFF527BAA3AA536ACA6C68332C208D1DBD904695F0F985D17D56B128D48AB`。
- 已确认 `D:\ida\ida.exe`、`idat.exe` 和 idalib 文件存在；尚未验证 headless 分析、Python 或反编译器可用性。本会话未暴露 REA/IDA MCP 工具；REA doctor 的 npx 调用因缓存目录 package.json 缺失而失败。这不是 IDA 不可用的证据。

## 本轮实际读取的 dev 证据

以下路径相对于 dev 证据根，地址均为 dev VA，不是 retail 地址。

| 入口 | 观察 | 边界 |
|---|---|---|
| `anonymous/RegisterPackages.c`，0x4A3480 | `DirectoryIterator::Read` 枚举当前目录 `*.package`，顺着返回链表打开并校验注册 | 本函数没有额外排序或递归 |
| `EA/IO/DirectoryIterator/Read.c`，0x89C9B0 | `EntryFindFirst/Next` 返回的文件逐一 `push_back` | 没有文件名比较器 |
| `EA/IO/EntryFindFirst.c`、`EntryFindNext.c` | 分别调用 `FindFirstFileW`、`FindNextFileW` | 观察到字母顺序不等于引擎承诺字母排序 |
| `SP/SetupResources.c`，0x4A3990 | 普通 dataRoot 包 priority=0；排除 Preload 后单独注册 Preload priority=1；另有条件目录数据库 priority=2 | 该额外目录的 ID 尚未映射，不能直接称为 UserData/Packages |
| `EA/ResourceMan/Manager/RegisterDatabase.c`，0x8C83D0 | 新数据库交给 InternalDatabaseList::Add，再 Init/Attach | 注册不等于资源已消费 |
| `EA/ResourceMan/Manager/InternalDatabaseList/Add.c`，0x8C4D80 | 遍历时越过所有 priority >= 新 priority 的节点后插入 | priority 降序，同 priority 稳定保留注册顺序 |
| `EA/ResourceMan/Manager/FindDatabase.c`，0x8C4800 | 从链首查询，返回首个资源查询成功的数据库 | 仅证明此查找路径；缓存、OpenRecord、专用管理器仍需检查 |
| `GB/InitGameVersions.c`，0x5ED7B0 | 开始先从 PropertyManager 收集 Ecosystem group 的 game/version 信息 | EcoGame 不能简化为普通目录顺序，后续版本选择仍需追踪 |

组合以上观察：dev 通用链并非“后加载覆盖前加载”；在 FindDatabase 路径中，高 priority 优先，同 priority 先注册的命中优先。尚不能据此认定 retail 全部资源类别都遵循同一规则，也不能从以前个别覆盖失败实验推出所有同 TGI 覆盖都不支持。

## 接下来按顺序深入

1. **目录映射与启动调用链**：从 cSimCityApp::Init、cAppSystem::SetupDirectories、GetDirFromID、SetupResources 的调用者恢复实际路径、命令行/配置重定向、目录是否存在的分支。重点闭合 0x8F03C82/0x8F03C83；给 SimCityData、SimCityUserData/Packages、EcoGame、Locale、Preload 分别标明用途和时机。
2. **普通资源完整解析语义**：补查 AddPackageIfValid、Attach、OpenRecord、缓存与 Property 继承，验证 priority、同级顺序及失败回退。将“枚举→打开→校验→注册→首次查询→缓存”分列。
3. **EcoGame 独立链**：InitGameVersions→AddGamePackage→版本表→实际选用数据库，区分资源 TGI、game ID、版本键冲突；不能把普通 ResourceMan 的规则直接套用。
4. **发行版静态对拍**：优先使用 dump 的独立分析数据库，建立 dev 函数名/VA→retail RVA/入口字节/调用与分支差异表；离线 EXE 保留独立指纹，不能仅凭相同 PDB 字符串或相近布局认定代码相同。
5. **必要的运行时闭环**：静态链闭合后，在独立测试环境记录枚举、注册 priority 与目标 TGI 的最终数据库。对文件名、复制顺序、目录、priority 分别控制变量；已有缓存与启动时机需纳入实验。文件访问日志只能证明打开，不足以证明覆盖胜者。本轮未向正在游玩的进程注入、未修改游戏包。

## 交付与验收

- 一张目录加载时序表，一张 priority/同级冲突表，一份 dev/retail 函数对照表。
- 每个结论标明 dev 静态、retail 静态或运行时观察，附样本指纹和函数位置。
- “首字母排序”目前只得到 dev 通用链无显式排序的证据；retail 待验证。
- 全目录先后顺序、目录 ID 映射、retail 同 TGI 最终规则目前仍未闭合。

本记录不修改昨天已有未提交报告，也不把初查结论写入 OpenSCP 的覆盖实现。
