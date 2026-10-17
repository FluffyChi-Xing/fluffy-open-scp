# 文件加载专题：疑点收尾与进入下一专题的条件

日期：2026-10-17。此表覆盖普通 package、Locale、EcoGame 的加载与覆盖，不扩展到所有资源格式的完整语义逆向。

## 当前判断

后续已解决 cape 包 Open 失败：旧 Rust/TS writer 固定写额外四字节却未设置 stored-size 高位标记，多条索引被游戏错位解析；仅修正四个字节的副本通过实机索引校验与 Open，两侧 writer 已修复并补回归测试。详见 [根因与修复](package-cape-index-sizeflag-20261017.md)。此前单条包命中不能外推为旧版多条写出正确。

普通资源加载的主要静态规则已闭合，三个 P0 已在实际安装目录完成运行时对照：[实机报告](package-runtime-verification-20261017.md)。一个 1,369 字节、无 metadata 的单 TGI 包成功注册，放入 Data 时被实际选中；放入用户 Packages 时虽注册成功但仍命中 App。不能推广为“任意最小 mod 均语义正确”。

本轮新增对拍后，`tools/package_loader_probe.py` 对两个指定 EXE 的 20 个有界区间均逐字节一致。证据清单为 `tmp/package-loader-closure-20261017/manifest.json`。这不等于全文件或运行中内存一致。

## 已解决或明确边界的疑点

| 问题 | 结论与范围 | 对 OpenSCP 的影响 |
|---|---|---|
| 按首字母排序吗 | 普通链没有文件名排序器，保留 Win32 枚举顺序；不是仅比较首字母的规则 | 文件名只能影响候选枚举，不能作为跨目录覆盖保证 |
| 后加载覆盖前加载吗 | retail 通用数据库列表按 priority 降序；同级稳定保留先注册者；FindDatabase 首命中 | 替换工具的简单 last-wins 预测模型，但需区分缓存/类型映射 |
| 用户 Packages 是否比 data 优先 | retail PreInit 先处理 data，Init 后处理 private/Packages；两者普通包都是0 | 后投放用户包成功注册也可能被基础包遮蔽 |
| Locale / Preload / Config 数值能沿用 dev 吗 | retail 分别是1/2/10；dev 是0/1/2；Config 为散文件数据库 | 必须用构建 profile，不能把Config当高优先级package目录 |
| 最小包必须有官方 metadata 吗 | retail Property type00B1B104/group40404000：0条普通注册；1条验签等；多条拒绝 | 默认最小overlay不补官方metadata |
| 注册成功就代表生效吗 | AddPackageIfValid返回值未检查RegisterDatabase结果；注册后还需目标查找和解码 | 分层报告扫描/准入/注册/命中/消费 |
| 缓存会影响覆盖吗 | retail GetResourceInternal先CheckCacheInternal，命中即返回；普通Attach无缓存清除 | 必须记录缓存条件；普通包不承诺热重载 |
| EcoGame按普通TGI顺序选包吗 | 它先建立game/version表，再切版本挂接；同版本入表与普通数据库优先级不是一回事 | EcoGame独立profile和诊断 |
| EcoGame版本必须是10吗 | 10为UInt32 Variant类型，实际版本取属性值 | 校验器不得硬编码version=10 |
| 83路径不存在就回退82吗 | 指定retail仅在GetDirFromID(83)返回null才查82，不检查磁盘Exists作为fallback条件 | 已配置但失效的83路径可能导致未扫描82，应单列诊断 |
| Locale会额外挂英文包吗 | retail对应注册函数无dev递归；SetLocale也未在失败后改en-us重试；初始化特定缺省分支会选en-us | 不应自动宣称缺失语言/缺失词条一定有英文fallback；词条查询层另论 |
| Header反编译的可疑返回是否伪影 | retail机器码证实offset一致性分支可独立返回true，见下节 | 发布校验采用保守规范，不复刻游戏的宽容/缺陷 |

## 当前已确认的普通启动顺序与查询优先级

retail 默认主链的注册时序：

1. PreInit：可选 BaseData(-1000) → data普通包(0) → Preload(2) → Config散文件(10)。
2. Init：Locale初始化，当前语言embedded/目录包(1)、可选BaseLocale(-1000) → private/Packages(0)。
3. EcoGame另行收集版本并切换挂接，不应直接并入上面同名目录排序。

对应通用查询优先级是数字降序，不是上述时间线倒序。已签名有效metadata可能修改priority；未走通用查找的专用资源、显式database和缓存需另行判断。

EcoGame retail外部版本库挂接priority=100，本地集合挂接=2；其版本表中同game/version后成功写入者替换候选。SwitchToScriptsVersion默认同版本短路，retail新增第4参数非零可绕过短路，但不等于已构建脚本缓存全部清空。Ecosystem group实值为0x40800200/+1。

具体调用位置见 `package-directory-chain-evidence.md`；EcoGame策略见 `package-ecogame-resolution-closure.md`。

## 本轮新增缓存与Header证据

以下是retail VA，image base=0x400000。

- GetResource封装0x911AE0在0x911AFE传cache=true，0x911B00调用GetResourceInternal0x911760。
- GetResourceInternal在0x911790调用CheckCacheInternal0x911110；0x91179B命中跳到返回。之后才在0x9117CB/0x9117DA调用FindDatabase/FindRecord。带显式database时绕过通用选库，资源类型映射还有独立顺序。
- CheckCacheInternal先GetLoadedResource（manager vslot+0x14，0x910AC0），再查异步请求；缓存不仅是磁盘文件缓存。
- DatabasePackedFile::Attach0x8E8830从manager进入时写mpManager并返回true，没有清理资源缓存的调用。不能由“重新注册”推导“重新解码所有旧资源”。
- VerifyHeaderRecordIntegrity0x8E9080先检查DBPF magic、major<=3、条目数、索引范围、key type；但0x8E90D5之后，若64位index offset非零且旧offset字段非零，返回两者一致且高32位为0的结果，0x8E90F3直接设AL=1，未再合并前面的BL。该点已用原始指令复核，并非仅反编译推测。
- 上一条仅说明此header子函数，不说明坏文件能通过后续索引解析/消费。OpenSCP不应为模仿游戏而放宽magic/版本/边界检查，也不应把“游戏函数返回true”称作完整性验证。

原始证据 `tmp/package-loader-20261017/header-integrity.asm.txt`、`cache-first.asm.txt` 及 `closure-*.c`。部分定点反编译存在参数/栈恢复失真（GetResourceInternal、SwitchToScriptsVersion），参数语义以原始调用指令和已知dev结构交叉确认；不能直接把伪C当ABI。

## 真正剩下的问题

| 优先级 | 未决项 | 已缩小的边界 | 如何关掉 |
|---|---|---|---|
| P0 已关闭（当前环境） | 实际启动目录/数据库列表 | EXE 指纹及 8 个运行入口匹配；已记录 Data→Config→Locale→Packages 与后续 EcoGame 挂接 | 可选 BaseData/BaseLocale/83 配置来源仍属其他配置场景 |
| P0 已关闭（单 TGI 对照） | 最小无metadata包的最终命中 | 同一 payload 在用户 Packages 注册但落败，在 Data 提前注册并被 FindRecord 选中、成功创建 Record | 工厂解码与任意资源依赖正确性不能由一个对照包推出 |
| P0 已关闭（观察范围内） | 启动注册链与目标查询日志 | 挂起启动安装观察后 resume；三次 errors=0/dropped=0，捕获首轮扫描 | baseline 有意限量，目标模式精确过滤；不是全量 I/O/缓存/所有工厂日志 |
| 后续本地化专题 | 缺失具体词条时的查询回退 | 本专题已闭合默认en-us与包挂接行为；UI/字符串查询层不是文件扫描规则 | 仅在支持多语言资源语义检测时继续查询层分析，不阻塞普通包加载结论 |
| P1 | EcoGame脚本对象缓存与特殊更新流程的retail全链 | 关键版本切换priority/强制参数已对拍；完整脚本缓存键目前主要dev证据 | 仅在支持脚本热替换时继续对齐；普通overlay明确要求重启即可避免承诺此能力 |
| 后续资源专题 | 任意TGI的完整依赖闭包 | loader不会替工具验证全部资源语义；地图/模型/UI/脚本各有不同依赖 | 由对应资源专题提供解析器与依赖规则，未知类型保留unknown |

## 本专题的退出条件

三个 P0 已具备真实单 TGI 验收记录，文件加载主链可以阶段性收尾。后续服务应保留逐产物验证状态：本轮验证通过不能自动赋予其他产物“实机生效”。只做离线构建的产物仍必须显示运行时未验证。

本轮未开始下一逆向专题；实际进行了三次启动观察和两次最小包投放对照，测试包已校验哈希后清理，实验进程正常请求退出。现有 cape-trinity-clearwater.package 的 Open=false 留作具体文件诊断项；未修改该文件。完整范围、事件号与工具见实机报告。
