# Package 目录与启动链：dev 静态证据

后续运行时更新：实际目录与注册链已记录于 [实机报告](package-runtime-verification-20261017.md)；下文保留 dev/retail 静态证据的独立范围。

日期：2026-10-17。只读分析；未启动、注入或修改游戏。证据根 `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/`。以下是带符号反编译文本的观察，不是原始源码或 retail 运行结果。行号均相对于该根的文件。反编译中的 `loc_`/`byte_` 目录 ID 表达式按地址数值读取；类型名和函数签名可能失真。

## 目录映射

| ID | dev 映射与条件 | 证据 |
|---|---|---|
| 0xA02149 (`&loc_A02145 + 4`) | data 根；SC 首选 exe 目录的 `../SimCityData`，目录存在才注册。`dataDir` 参数可覆盖，否则已有注册保留，最后 fallback 当前工作目录 | SC/cSimCityApp/Init.c:180-185 (VA 0x4041A0)；SP/SetupStandardDirectories.c:30-45 (0x443A30)；SP/cAppSystem/SetupDirectories.c:46-55,82-86 (0x4546F0) |
| 0xA0214B | public/user-data 根：显式 userDataDir；否则 Documents/publicDir；否则 data/../UserData | SP/SetupStandardDirectories.c:55-74 |
| 0xA02151 | private 根。SC 非 devDirs 默认 exe/../SimCityUserData，先创建再注册；标准目录设置保留已有值，否则 AppData/privateDir；无 privateDir 则复用 user-data 根 | SC/cSimCityApp/Init.c:186-199；SP/SetupStandardDirectories.c:76-122 |
| 0x45962CC | data/Locale | SP/SetupStandardDirectories.c:47-50 |
| 0xA0214F | data/Config | SP/SetupStandardDirectories.c:51-54 |
| save area 0x11AC1AD | private 根/Packages；不是 directory ID | SP/CreateDefaultSaveAreas.c:409-425 (0x45B9C0) |
| 0x8F03C82 | 若未设置，cEcoGame::Init 注册 private 根/EcoGame/ 并 EnsureExists | GB/cEcoGame/Init.c:24-40 (0x5FB560) |
| 0x8F03C83 | 可选 game-package 路径，优先于 82；此反编译 src 的数值检索只发现消费者，未找到注册者 | GB/InitGameVersions.c:111-127 (0x5ED7B0)；GB/cUpdateManager/StartGameDownloads.c:44-45；StartSingleGameDownload.c:109-110 |
| 0x6CBED39 / 0x6CBED3A | 可选额外普通包根 / Locale 根；未闭合注册者与物理目录 | SP/cAppSystem/PreInit.c:359-361；SP/cLocaleManager/RegisterLocaleDatabases.c:236-254 |

注意 `userDataDir` 命令行参数不等同于 private 根：在此 dev 流程中 SC 已经预先设置 private 根，不能仅凭参数名判断 Packages/EcoGame 会一起重定向。SetupDirectories 中 public/private 字段与调用形参的恢复存在对调，但 SC 两者均设置为 SimCity（Init.c:178），本默认路径结论不依赖该歧义。

## 可确认的启动顺序

SC/cSimCityApp/Init.c:201 调用 AppSystem::PreInit；275 调用 Init；558 才调用 GB::InitGameVersions。

1. **PreInit (VA 0x456A30)**：SetupDirectories (:305)，检查 data 根 (:306-318)，可选 ID 0x6CBED39 普通 package 注册 priority=-1000 (:359-361)，然后 SetupResources (:364)。
2. **SetupResources (0x4A3990)**：data 根的普通 package 注册 priority=0 (:85-92)，排除的 Preload.package 独立 RegisterMemoryPackage priority=1 (:93-108)，data/Config 存在时注册 DatabaseDirectoryFiles priority=2 (:110-138)。这里 priority=2 是散文件数据库，不是 UserData/Packages。
3. **AppSystem::Init (0x453460)**：LocaleManager::Init (:142)；之后 CreateDefaultSaveAreas (:209)，取 Packages save area location 并 RegisterPackages priority=0 (:210-214)。因此默认 private/Packages 普通包比 data 根普通包晚注册且同 priority。
4. **LocaleManager::Init (0x4A6DA0)**：SetLocale (:276 或 :301) → RegisterLocaleDatabases (SetLocale.c:73)。注册函数 (0x4A61D0) 先遍历已注册数据库找对应 locale 的 embedded package，priority=0 (:89-111)；随后 data/Locale/localeName/*.package，priority=0 (:138-216)；可选 0x6CBED3A/localeName/*.package，priority=-1000 (:236-301)。非 en-us locale 随后递归注册 en-us fallback (:339-365)。因此不能描述成“扫描 Locale 下全部语言目录”。
5. **GB::InitGameVersions (0x5ED7B0)**：先基于 PropertyManager 的 Ecosystem 属性列表建立 game/version 信息 (:48-99)，再选择 83 存在则用 83，否则 82 (:111-113)，只扫描所选根的 *.package (:121-127)，交 AddGamePackage (:177)，最后 SwitchToProductionVersions (:190)。它不是普通 RegisterPackages 目录层级的一部分。

同 priority 的先注册者优先结论须结合前一份报告的 InternalDatabaseList::Add/FindDatabase，并限定查询路径及缓存条件。这里的顺序是可见调用链顺序，不能据此断言每个 TGI 首次消费时刻。

## 对 OpenSCP 的实际影响

- 完整性检测应区分“容器合法”“目录链会发现”“注册通过”“目标 TGI 获胜”“资源语义有效”。仅能打开 DBPF，不代表 mod 已生效。
- 自动打包输出应声明普通资源包或 EcoGame 版本包类型。少量 TGI 的普通覆盖包不能无条件放到 EcoGame：dev InitGameVersions.c:177-180 在 AddGamePackage 返回 0 时 Shutdown 后会 File::Remove 删除该文件。这是静态代码分支，不代表本轮发生删除。
- Config 的 priority=2 不能作为 package 输出目录的优先级依据，它注册的是散文件数据库。
- `000_` 只改变文件名，不能把 UserData/Packages 提到 data 根注册之前。跨目录阶段与 priority 必须纳入覆盖分析。
- 日志应至少包含 resolved directory ID/path、枚举序号、数据库 priority、注册结果、TGI 解析的最终 database；单独文件打开日志无法回答覆盖胜者。

## 仍未知

- retail Locale/UserData/EcoGame 完整调用链尚未对齐；普通资源注册/查找与 SetupResources 已完成，见下节。
- 0x8F03C83、0x6CBED39、0x6CBED3A 的实际注册入口；可能在未覆盖代码、配置或插件路径，不能从无直接数值命中推断从不设置。
- cEcoGame 实例 Init 的完整虚调用创建链、是否在某些模式中有路径预注册；只确认 82 的默认注册函数与使用者。
- 缓存、特殊资源 manager 与 EcoGame 选中版本后 Attach 时机须结合其它分工证据。

## Retail 静态对拍补充：priority 数值有变化

本补充直接从指定 `SimCity_dump_SCY.exe` PE section 映射读取 x86 指令，使用 Capstone 解码；未等待或依赖 IDA 自动分析完成。dump SHA-256 为 `06D681444011F1297A6CF53BC233EE5F138D41786B451E6F18DC02BF273FF569`。以下均为 VA，retail image base=0x400000；RVA=VA−0x400000。符号名称通过 dev 语义、构造器/vtable/调用者链匹配，是分析赋名。

主调查随后已使用 IDA 9.2 定点反编译复核 SetupResources、RegisterPackages、AddPackageIfValid、RegisterDatabase、InternalDatabaseList::Add、FindDatabase；输出保存在 `tmp/package-loader-20261017/ida-*.c`，私有数据库为 `focused.exe.i64`。全程序自动分析未完成，已停止本次创建的后台分析进程；用户原有 IDA 窗口和数据库未改动。局部反编译中的未知调用参数仍以机器码为准。

| dev 函数 | retail VA / RVA | 验证依据 |
|---|---|---|
| SP::SetupResources 0x4A3990 | 0x45F6E0 / 0x5F6E0 | 首条调用0x405C30获取manager；普通包注册、Preload字符串、Config ID与目录数据库连续调用结构相符 |
| Manager 构造器 | 0x914A00 / 0x514A00 | AppSystem创建0x148字节对象，0x41A866调用该构造器，0x41A888经SetResourceManager写全局0xE4EF7C；构造器0x914A0A写vtable=0xD2C138 |
| Manager::RegisterDatabase 0x8C83D0 | 0x9145A0 / 0x5145A0 | manager vtable+0x4C；注册分支先IsDatabaseRegistered，再0x9145CA调用Add，然后database Init与Attach；取消注册分支Erase/Detach |
| InternalDatabaseList::Add 0x8C4D80 | 0x911080 / 0x511080 | 0x9110D0比较现有node.priority和新priority；0x9110D3仅当现有值小于新值时跳到插入，否则继续next；稳定降序 |
| Manager::FindDatabase 0x8C4800 | 0x910B40 / 0x510B40 | manager vtable+0x54；0x910B68调用database slot+0x34查询；0x910B6C成功跳到返回当前database，失败遍历next；首命中 |

**重要差异：retail Preload priority=2，Config priority=10；不是 dev 的 1 和 2。**

- 普通 data 注册：0x45F825 `push ebp`，此时 ebp=0（0x45F706）；0x45F83E调用0x45F420。
- Preload：0x45F88A `push 2`，0x45F88E调用0x45DEB0；0x45DFAE读取第三参数，0x45DFC1压入，0x45DFC5调用manager vtable+0x4C，确认2确实传给RegisterDatabase。
- Config：0x45F8A6 `push 0xA0214F`获取目录；0x45F930 `push 0xA`，0x45F932 `push esi`，0x45F933 `push 1`，0x45F937调用manager vtable+0x4C。
- Add核心：0x9110D0 `cmp dword ptr [eax+8], ebp`；0x9110D3 `jl 0x9110DF`；0x9110D5-DB沿next继续；插入在0x9110DF-E3。因此“高priority先查，同priority先注册先查”在此retail通用路径也成立。
- FindDatabase：0x910B53读node.database；0x910B68查询；成功返回0x910B7D，失败0x910B6E读next。此结论不覆盖缓存已命中或专用EcoGame版本选库。

对当前 `SimCity.exe`（SHA-256 `2B0AFF527BAA3AA536ACA6C68332C208D1DBD904695F0F985D17D56B128D48AB`）通过独立PE section映射做逐字节对照，下列10个范围均一致：

`405C30+6`, `405E60+10`, `41A847+70`, `914A00+32`, `D2C138+156`, `9145A0+219`, `911080+139`, `910B40+75`, `45F6E0+784`, `45DEB0+384`（长度均十进制）。这支持将本补充具体范围的静态结论用于当前离线EXE；不表示两个文件全量相同。

复现文件：`tmp/package-loader-20261017/directory_probe.py`、`directory_verify.py`；原始指令证据 `manager-directory-disasm.txt`。tmp 按项目约定保留、不提交。本补充未重新对拍 retail Locale/UserData/EcoGame 的目录链，以上前文相关部分仍仅为 dev 证据。

## Retail 目录链收尾（继续核验）

### 已闭合的调用顺序与优先级

retail 主 Init 0x403FB0 在 0x4043C6 选择 AppSystem vtable+0x1C，0x4043CF调用 PreInit；在0x404513选择+0x20，0x404517调用 Init。AppSystem vtable 0xCF36C0 的这两项分别是0x41A480与0x416F80；构造赋值位置包含0x4186B9。retail 不是根据文件夹名字排序整个安装目录，而是显式分阶段调用。

| 阶段 | retail 证据 | priority |
|---|---|---|
| PreInit 可选 BaseData | 0x41AC16查询ID 0x6CBED39；0x41AC2A push -1000；0x41AC31调用RegisterPackages包装0x45F610 | -1000 |
| PreInit data/Preload/Config | 0x41AC4E调用SetupResources 0x45F6E0，详见上节 | 普通0 / Preload2 / Config10 |
| Init Locale | 0x4170B3调用LocaleManager::Init 0x4628F0；其虚表0xCF8BF8 +0x10为SetLocale 0x462620，Init在0x462B52经该槽调用；SetLocale于0x462716调用RegisterLocaleDatabases 0x461E90 | 见下一行 |
| Locale embedded → Locale目录 → BaseLocale | 0x461FB4 push1，0x461FBA调用AddEmbeddedPackage 0x45F210；0x462228 push1，0x46222B调用AddPackageIfValid；0x4623FC push -1000，0x462402调用AddPackageIfValid | **1 / 1 / -1000** |
| Init private/Packages | 0x417263调用CreateDefaultSaveAreas 0x40B090；0x417268查询save area 0x11AC1AD；0x417280压入ebx=0（Init入口0x416F8E清零）；0x417288调用RegisterPackages 0x45F610 | 0 |

Packages save area的目录来源也已对拍：0x40B6B8压入0x11AC1AD，0x40B6C2压入UTF-16字符串Packages（0xCF2A5C），0x40B6C7压入private目录ID 0xA02151，0x40B6CC调用CreateDirectorySave 0x40A050。主Init在非devDirs分支建立SimCityUserData并于0x404337/0x40433C把它注册为0xA02151。主Init的data根还含SimCity_App.package存在性判断与fallback分支；所以默认安装路径映射已确认，但不能把所有启动选项简化成固定绝对路径。

**retail 普通查询优先级序列应写为 Config散文件10 > Preload2 > Locale1 > 普通data/Packages0 > BaseData/BaseLocale -1000。** 同priority仍稳定，故普通data与Packages冲突时，此路径中先注册的data优先；不是Packages后加载就覆盖。这里只覆盖上述显式注册路径，特殊包动态Attach/缓存等仍须独立核验。

### Locale fallback 必须修正版本边界

dev RegisterLocaleDatabases的末尾递归注册en-us。但retail对应函数0x461E90到0x4624F1完整线性反汇编中没有该递归调用，末段完成清理后直接ret4；全文件直接call 0x461E90的检索只找到SetLocale中的0x462716。**因此不能继续把dev的“选中语言后追加en-us”作为retail既定流程。** 这不是证明整个retail系统不存在fallback；fallback仍可能由LocaleManager初始化、语言映射、已注册嵌入资源或其它路径提供。

### 可选目录 ID 来源的边界

retail只读数据表已给出准确语义：0xCF6F28存字符串指针0xCF6E80=`BaseData`，0xCF6F2C存ID 0x6CBED39；0xCF6F30存0xCF6E74=`BaseLocale`，0xCF6F34存ID 0x6CBED3A。这是名称映射证据，不是目录注册执行证据。对该表范围的绝对地址引用搜索没有找到明确代码消费者，尚不能说游戏会从哪个配置自动创建两根。

0x8F03C83 的retail立即数命中仅见消费者0x5ADDC5、0x5ADFCD、0x5C8DEE，均优先查询83，缺失再查询82，未发现直接常量注册者。此负面结果仅限定直接立即数与当前exe；不排除运行时计算ID、其它模块或未发现的间接配置。不能把83命名为另一个具体硬盘目录。

### 当前EXE一致性与复现

本轮增加10个逐字节相同范围（长度十进制）：`403FB0+3904`, `416F80+800`, `41A480+2048`, `40B090+1728`, `461E90+1636`, `462620+416`, `4628F0+768`, `CF36C0+48`, `CF8BF8+24`, `CF6E74+256`。dump与当前SimCity.exe均一致。证据位于`tmp/package-loader-20261017/retail-directory-chain-disasm.txt`，复现`directory_chain_verify.py`。未打开主进程的focused.exe.i64，未修改游戏文件或注入进程。

## Locale en-us 定向收尾：缺省选择不等于资源 fallback

本轮限定检查UTF-16字符串`en-us`（VA 0xCF8B94）在LocaleManager附近的引用，没有扩展到UI子系统。

- **0x4619B0 = HasLocaleResources 语义匹配。** 0x461A1D调用CompareLocaleIds（0x91FC20）比较传入locale与en-us，等价时0x461A50-0x461A5A直接返回true。否则0x461A6D/0x461A7E取BaseLocale，缺失时0x461A8F/0x461A94取Locale，拼接目标语言名称，0x461B4F检查目录Exists。它做可用性判断，不查询某个TGI，也不注册en-us包。SetLocale于0x462649调用它；该返回值随后被别的调用覆盖，不能仅凭这一调用断言缺目录会触发重选语言。
- **0x462B11-0x462B58 是缺省语言选择。** Init在0x462A5F读取AppProperties ID `0x0EAB3AC6`作为locale注册表路径（与dev Init.c:99-116语义一致）。路径转换成宽字符串后0x462AE2检测长度，非空跳0x462B85走注册表读取；空路径则构造en-us，0x462B52调用SetLocale(en-us)，0x462B58保存初始化结果。它不是“先注册所选语言再补en-us”，也不是TGI查不到后的逐资源fallback。
- **0x462D25 是语言名称映射值。** 0x462D0B的字符串0xCF8CE0为`en_US`，0x462D25压入`en-us`，通过map operator[]与字符串赋值形成`en_US → en-us`。同函数邻近建立其它locale映射；读取注册表后的值在0x46320F查表，找到条目才于0x46324E调用SetLocale(mappedLocale)。不能把字面量en-us出现本身当作fallback证据。
- **包注册fallback：在本函数链未见。** 完整RegisterLocaleDatabases（0x461E90..0x4624F1）没有dev的末尾递归en-us调用；直接call引用扫描仅找到SetLocale的0x462716。故本次可证实的是缺省语言与映射，并未证实retail自动追加en-us包或缺失TGI查询fallback。

复现：`tmp/package-loader-20261017/locale_default_verify.py`；原始指令`retail-locale-default-disasm.txt`。新增范围`4619B0+528`, `4628F0+2592`, `CF8B94+12`, `CF8CE0+12`（十进制长度）在dump与当前EXE逐字节一致。

SetLocale最后核验（0x462620..0x4627CE）：请求locale参数一直保留在esi；0x462649调用HasLocaleResources后**没有测试其返回值**，紧接0x462655的CompareLocaleIds覆盖al。0x46268A压入原请求esi，0x46268D赋给mLocaleId；0x462710取mLocaleId并于0x462716单次RegisterLocaleDatabases。注册失败只从0x46271D跳至0x46277E通知/清理，最终0x4627C8返回true，没有改成en-us、重试注册或恢复旧数据库的分支。因此就此调用链：**缺省选择en-us存在；缺资源后替换为en-us不存在；选中语言与en-us双注册叠加不存在。** 此结论不扩展到外部UI的字符串查询行为。
