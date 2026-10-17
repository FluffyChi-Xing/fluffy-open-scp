# Package 加载研究：OpenSCP 服务与日志落地

后续运行时更新：三个 P0 已完成实机对照，单 TGI 包成功注册且在 Data 中被选作实际资源来源；已交付启动观察脚本。见 [实机报告](package-runtime-verification-20261017.md)。下文“尚未验收”为早期静态调查状态，资源语义边界仍适用。

日期：2026-10-17。结论来自 dev 带符号反编译与指定 retail EXE 的静态机器码；尚未运行新 mod 的游戏验收。完整性检测和自动打包可开发，但输出状态必须区分结构、准入、优先级与消费结果。

后续收尾见 [疑点清单](package-loading-open-questions.md)：retail Locale priority=1、UserData Packages时序、EcoGame priority100/2与force-like参数已补齐；普通缓存与Header可疑分支已机器码复核。

## 已闭合的最小包准入

retail `AddPackageIfValid` 为 VA `0x45EC40` / RVA `0x5EC40`。它筛选 type `0x00B1B104`、group `0x40404000` 的记录：零条走普通注册；一条进入签名等检查；多条拒绝。这不是要求每个 mod 都携带 manifest 的协议。不能自动复制官方 package-info 来“补全”普通 overlay。

两个用户指定 EXE 已按 RVA 比较本函数、RegisterPackages、DirectoryIterator::Read 的有界代码区间及 package-info group，全部一致。这里只覆盖所列区间，不能宣称两份 EXE 整体相同或运行中的内存未被修改。

复现工具：`tools/package_loader_probe.py`。默认仅依赖 Python 标准库；`--disassemble` 另需 Capstone。输入必须匹配本轮记录的 SHA-256，未知构建拒绝套地址。

```powershell
python tools/package_loader_probe.py `
  'D:\ea-games\simcity_offline\SimCity_dump_SCY.exe' `
  'D:\ea-games\simcity_offline\SimCity.exe' `
  --output tmp/package-loader-20261017/verified --disassemble
```

本次实际执行使用本地 bundled Python 与已有 `tmp/re-python` Capstone。产物 `tmp/package-loader-20261017/verified/manifest.json` 保存样本及区间 SHA-256、RVA、文件 offset、入口字节和比较结果。完整游戏反汇编只保留在本地 ignored 目录。

IDA 9.2 idalib 与 Hex-Rays 已实际运行成功，定点反编译六个关键函数并保存到 `tmp/package-loader-20261017/ida-*.c`，与原始指令一致。数据库 `focused.exe.i64` 已保存并关闭。初次全程序 auto analysis 耗时较长，已停止该自建进程，改用 `plan_and_wait(start,end)` 的局部分析；没有把未完成的全程序分析算作验证。

## 不同顺序必须分开建模

扫描、打开 DBPF、校验接受、注册/Attach、查找目标 TGI、反序列化和缓存命中是不同阶段。

dev `Manager::GetResourceInternal`（0x8C5400）先 CheckCacheInternal，命中就返回；未命中才 FindDatabase/FindRecord。显式传入数据库可限制查找范围。`FindRecord`（0x8C5CC0）还存在 resource type→record type 映射：先按映射类型，再遍历数据库。因此普通 TGI 冲突表只是解析预测的一部分，不适用于所有资源工厂或 EcoGame 版本表。

例如，将 `000_mod.package` 放到默认 UserData/Packages，并不会令它在 SimCityData 阶段之前注册；同级先注册优先时，它可能成功进入数据库列表但始终没有覆盖原资源。另一个情况是资源已经缓存，新注册包也不等于立即热更新。

目录与版本差异见 `package-directory-chain-evidence.md`。特别是 retail SetupResources 的 Preload priority=2、Config 散文件 priority=10，与 dev 的 1/2 不同，普通 data priority 仍为 0。服务需以 build profile 保存这些数值及证据等级。

## 建议的最小服务

### inspect_package

输入：package 文件、普通资源/EcoGame/add-on profile、游戏构建指纹及可选基础包集合。

输出应分项而不是单一 valid 布尔：

| 维度 | 检查内容 | 不能据此声称 |
|---|---|---|
| container | header/index/payload 范围、溢出、预算、重复 TGI、压缩与截断 | 游戏已经注册 |
| resource | 对已知类型执行严格解析；未知类型标 opaque | 未知格式一定损坏 |
| admission | 普通 metadata 0/1/多条分支；独立 EcoGame 要求 | 所有 add-on 签名已验证 |
| dependencies | 新增记录的必要依赖；基础游戏依赖的存在、构建和指纹 | 所有依赖都必须复制进 mod |
| resolution | 目标目录、注册 priority/阶段、冲突候选、类型映射和缓存限制 | 预测 winner 等于运行时观察 |
| runtime | 是否观察到注册/Attach、目标记录命中、消费成功 | 静态通过即完成 runtime 验收 |

诊断使用稳定 code、severity、TGI、offset、expected/actual、evidence 字段。面向用户的文案和可访问性标签进入 zh-CN/en-US i18n。未知项单独返回 unknown，不转换成 passed。

### build_minimal_overlay

输入：修改集合、基础包指纹、目标 profile、依赖策略。

1. 读入源资源，对几个字段的修改重建成每个 TGI 的完整 payload；不把字段差分作为资源。
2. 仅加入修改资源和必要的新增依赖。允许明确绑定的基础游戏外部依赖。
3. 普通 profile 默认不生成 package-info，不隐式将普通包投到 EcoGame，也不静默剥掉用户明确选择的 add-on 元数据。
4. 复用现有 `write_uncompressed_overlay`，暂存写出后重新打开并逐 TGI 比较完整字节、条目数和 SHA-256。
5. 生成独立 sidecar 构建报告，记录源指纹、TGI 清单、外部依赖、预测冲突和未知项；sidecar 不是游戏必需的包内记录。
6. 全部门禁通过才原子发布产物。发布状态为“构建与静态检查通过”；只有运行时证据能升级为“已观察到生效”。

现有 writer 的未压缩布局为 `104 + 28*N + payload 总长` 字节。单个 TGI 已有能力写出，无需重新设计文件格式。相关 parser 缺口和既有调用点详见 `package-minimal-validation-evidence.md`。

## Package 载入日志能否采集

可以设计分层采集，但当前 UI bridge 不是该日志源。本轮没有采到新的游戏启动事件；检查时未发现名为 SimCity 的进程。

### 被动文件观察

Procmon/ETW 文件事件可回答：哪个进程何时访问哪个 package、文件打开结果以及读操作。启动后才开始采集会漏掉早期加载。一次 CreateFile 成功不能证明 DBPF 校验、注册、TGI 获胜或反序列化成功；也不能将工具自读 package 当作游戏加载。

### 原有游戏日志

本轮读到的 dev RegisterPackages、AddPackageIfValid、DatabasePackedFile::Open/ReadHeaders、AddGamePackage、InitGameVersions 路径未见直接输出详细拒绝原因的 Trace 调用。EA::Trace 存在不代表加载器每个失败分支会打日志；OutputDebugString/已有 reporter 只能收集实际输出的内容。零售是否有其它错误通道仍未穷尽，不能声称绝对无日志。

### Native 观察点

静态地址对齐后，可做保留原行为的观察器，最小阶段建议：

| 观察点 | 记录 | 限制 |
|---|---|---|
| RegisterPackages | path、枚举阶段、默认 priority | 返回 1 不代表每个文件成功 |
| DatabasePackedFile::Open | database 指针、路径、返回值 | 尚未注册，虚表/布局需按构建核验 |
| AddPackageIfValid | database、入参 priority、返回值 | 返回 true 仍未检查内部 RegisterDatabase 返回值 |
| Manager::RegisterDatabase | register/unregister、database、priority、返回值 | Attach 失败可能已插入列表，须保留完整事件语义 |
| FindDatabase/FindRecord | 仅白名单 TGI、返回 database、实际 record key | 缓存命中可能绕过；EcoGame 另走版本表 |
| 指定资源工厂 | payload 读取/解码结果 | 成本较高，仅针对验收资源 |

上述为观察器设计，尚未实施 live hook。32 位 `thiscall` 的 this 在 ECX，普通参数在栈上；不能把 dev 伪签名直接用于 Frida，也不能混用 VA 与 RVA。安装前核验进程可执行文件 SHA-256、模块 base、入口字节和 ABI；运行时补丁导致不一致应拒绝绑定。

日志 schema 最少保留 session、build SHA、pid、thread、sequence、timestamp、phase、path、database、priority、TGI、result、source（static/predicted/observed）、dropped。热路径按目标 TGI 过滤并有界排队；若丢记录，报告 coverage 不完整。单纯 onLeave 返回 false 通常只能说明失败阶段，详细“签名错/缺少权限/损坏索引”等原因需要分支证据或离线复核，不能凭空补写。

## 尚需运行时完成的验收

在独立测试环境中使用同一个单 TGI 包，分别验证普通 Data 与 UserData/Packages 路径、不同文件名、冲突记录、重启清缓存。每次同时记录构建报告、游戏文件访问、校验/注册事件和指定 TGI 最终数据库。EcoGame 单独测试 game/version，不将普通 overlay 放入其目录试错：dev 有校验失败后删文件分支。

本轮没有自动启动游戏、安装实验 mod、注入运行进程或实现产品服务；交付的是可复核证据、旧结论修正和服务/观测契约。
