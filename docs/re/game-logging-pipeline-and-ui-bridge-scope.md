# 游戏日志管线与 OpenSCP UI bridge 能力边界

2026-10-17 更新：已完成零售引擎文本两次启动捕获，Server::Trace 131 条（Config 119、UpdateManager 12），另有控制台输出 10 条；新增工具与产品路线见 [日志工作台调查](mod-diagnostics-logging-20261017.md)。下文的 dev 管线推断不能替代该报告的零售证据。

本报告回答两个问题：OpenSCP 当前的 UI bridge 日志是否只是演示；以及未来能否让 mod 主动打印日志、观察模拟过程。结论区分 EA::Trace 引擎日志、SP Cheat/控制台输出和 EAWebKit 页面 console 三条不同管线。

## 结论

当前 UI bridge 不是纯演示：它已经能在真实进程中观察到 EAWebKit 页面向原生桥的调用、返回、事件以及页面 console 消息，并将有界记录写入 JSONL，再同步部分关键事件到 OpenSCP activity store。但它不是游戏通用日志订阅器，也不能因为能看到 `console` 记录就宣称已经捕获引擎模拟日志。

当前实现的日志入口是 `EAWebKit.dll` 中已核对的 `ChromeClientEA::addMessageToConsole`（RVA `0x5AC490`），不是游戏的 `EA::Trace::Server`，也不是 EXE 中共享的 `EAWebKitClient::DebugLog` 空实现。native observer 只在已验证页面绑定上挂接，读取 level、line、message、source，保持原调用不变；每类有独立限额和队列，限流会丢记录但不阻断游戏。

## 源码中的 EA::Trace 管线

### TraceHelper → Trace → Server → reporters

`EA::Trace::TraceHelper::TraceHelper`（`src/EA/Trace/TraceHelper/TraceHelper.c`）创建 helper，设置 trace type、group、level 和 reporting location，然后向 `gTraceHelperTable` 注册。`IsTracing` 先让当前 tracer 判断过滤器，再返回 `mIsEnabled && !mIsFiltered`。

`TraceFormatted`（`src/EA/Trace/TraceHelper/TraceFormatted.c`）本身不写文件；它把格式串和参数交给 helper 的 `ITracer::TraceV`。`EA::Trace::SetTracer`/`SetServer`（`src/EA/Trace/SetTracer.c`、`SetServer.c`）负责替换全局 tracer，并通知 helper table 更新已有 helper。

`EA::Trace::Server::Trace`（`src/EA/Trace/Server/Trace.c`）为文本构造 `LogRecord`，附上 helper、等级名和递增记录号，然后复制当前 reporter 列表，依次执行 `IsFiltered` 与 `Report`。所以最终输出取决于是否安装 reporter 以及 group/level 过滤器；TraceHelper 存在不等于日志已经落盘。

`Server::SetOutputLevel`（`src/EA/Trace/Server/SetOutputLevel.c`）可以按 reporter、group、level 添加或移除过滤级别，默认过滤器和每个 reporter 都可以被更新。`TraceHelperTable::SetAllEnabled` 可以统一启停 helper，但这也是 native API，不是 package 字段。

### 已发现的 reporter

- `EA::Trace::Tracer::Trace`（`src/EA/Trace/Tracer/Trace.c`）在默认输出类型中调用 Windows `OutputDebugStringA`；高等级还会显示 trace dialog。该路径适合被外部 debugger/DebugView 观察，但源码没有证明它自动写固定日志文件。
- `anonymous::cLogReporterFile::Report` 使用 FileStream 写出 formatter 结果。后续已找到 `SP::CreateLogReporterFile`、SC 更新模式的 VersionLog、dev App 初始化的 ReleaseLogForDevs 创建点；各创建点是否保留在 retail 必须分别核对。retail ConfigLog 的另一条 LogReporterFile 路径已经由机器码、实际 `_config.log` 和 Report 事件确认。
- `anonymous::cLogReporterDebugConsole::Report`（`src/anonymous/cLogReporterDebugConsole/Report.c`）把格式化记录传给 `SP::cCheatManager::Output`。

### Cheat/控制台是一个可利用的中间层

`SP::cCheatManager::Init`（`src/SP/cCheatManager/Init.c`）创建 ArgScript parser、安装 `cConsoleStream`，然后向 EA::Trace server 添加名为 `AppConsole` 的 reporter：

```text
SetOutputLevel("AppConsole", null, 100)
SetOutputLevel("AppConsole", "Console", 1)
```

`cLogReporterDebugConsole::Report` → `cCheatManager::Output`，而 `cCheatManager::Output`（`src/SP/cCheatManager/Output.c`）遍历已注册 console，把字符串转发给每个 console。ArgScript parser 的输出也经 `cConsoleStream::Write` 回到同一 `Output`。

这证明游戏有“写到控制台”的真实 native 管线。`RunCheat` 能解析命令并把命令加入 history；但从 package 中加入任意新 native `TraceHelper` 或任意 C++ cheat command，仍需要代码执行能力。纯 DBPF/package 覆盖只能改变资源和脚本数据，不能凭字段自动调用 `EA::Trace::TraceFormatted`。

## OpenSCP 当前 UI bridge 实际做了什么

实现位于：

- [native_trace.py](../../tools/dynamic/ui-observer/native_trace.py)
- [native_agent.js](../../tools/dynamic/ui-observer/native_agent.js)
- [runtime_observer.rs](../../src-tauri/src/runtime_observer.rs)
- [LogStream.vue](../../src/pages/runtime-observer/LogStream.vue)

启动流程是：枚举 `SimCity.exe` → 只读扫描 EAWebKit view/binding → 校验模块 RVA、入口指令和宿主虚表 → 临时 Frida QJS agent attach → hook 三个已确认入口 → 输出 `trace.jsonl` → 到时卸载脚本并 detach。它不调用游戏 NativeFunction，不执行页面脚本，不读取事件队列，也不把任意用户提供的地址当成 hook 目标。

三类入口的含义不同：

| 入口 | 当前记录 | 能证明什么 |
|---|---|---|
| EXE `0x632930` | `invoke`、方法名、参数摘要、返回值 | UI→原生 bridge 请求/查询/命令发生过 |
| EXE `0x62F500` | `ReceiveGameEventFromClient` | 页面事件进入游戏 UI handler |
| EAWebKit `0x5AC490` | `console` channel 的 message/source/level/line | 页面 console/addMessageToConsole 发生过 |

`observer_sync` 只把 `ReceiveGameEventFromClient` 事件写成 `runtime.key_event` activity；普通 console、query 和 result 仍留在本次捕获目录的 JSONL 中，不会伪装成 engine log。

## 为什么当前 console 观察不到完整引擎模拟过程

源码已证明 EAWebKit 的 `EAWebKitClient::DebugLog` 在当前零售宿主中是共享的 `ret 4` 空实现；文档中的静态核对结果显示它不会自动把页面日志写入文件。因此当前桥接到的是页面 console 构造点，而不是这个宿主回调，更不是 EA::Trace server。

EA::Trace::TraceHelper/TraceFormatted 可以进入文本 reporter 链，但不能把所有带 Log 的模拟函数归为同一管线。`LogGlobalRule` 写二进制 mDeltaStream，`StateValidationFailed` 只赋值状态字段。UI bridge 既没有订阅 EA::Trace reporter，也没有因此自动获得规则的输入输出状态。

此外，当前观察器是有界采样器：每通道有速率/队列上限，普通查询在高负载时会丢弃大量记录；因此它不是无损的模拟事件总线。

## 对 mod debug 的实际可行性

### 仅资源/数据 package

可以作为实施方向，但需区分已经捕获的通道与尚未验收的 Mod 输出：

- 页面 console 可作为带 Mod 前缀的诊断候选；自制最小输出仍需验收。`scrui.DebugPrint` 及其 Client 包装层检查 `scrui.DEBUG`、client 和方法存在，不能等同于无条件 console 输出；不建议全局打开 DEBUG。
- 通过已有 UI→原生请求读取游戏公开给 UI 的状态，并把请求、返回、事件和资源字段写进页面日志；
- 使用 OpenSCP bridge 观察这些页面日志及其前后的 `PostGameCommand`/`RequestGameData`/事件序列。

这适合验证“UI 请求是否发出、引擎返回什么、某个状态字段何时变化”，不能直接打印隐藏的模拟 bin、规则中间值或任意 C++ 局部变量。

### 带代码执行能力的 mod/插件

如果将来有受控 native plugin、DLL 或已验证的脚本到 native 入口，则可以：

1. 注册自己的 `EA::Trace::TraceHelper` group；
2. 调用 `IsTracing` 后再 `TraceFormatted`；
3. 将自定义 `ILogReporter` 注册到 `EA::Trace::Server`，输出 NDJSON/命名管道/本地文件；
4. 或注册 ArgScript cheat command，通过 `cCheatManager::Output` 发送一条可见控制台消息。

OpenSCP 可以再增加一个“mod diagnostic channel”，按 `modId/group/level/timestamp/thread` 结构接收并做限流、完整性哈希和会话关联。这个方案需要版本绑定、ABI 校验、线程安全和撤回机制；不能把当前页面 bridge 直接升级成任意 native 调用器。

### 观察引擎现有日志

更现实的下一步有三档：

1. **外部 DebugView/OutputDebugString 采集**：低侵入，能看到默认 tracer 输出，但依赖进程级 Windows debug 输出，可能混入其他模块，也不保证完整。
2. **只读 hook `EA::Trace::Server::Trace` 或指定 reporter 的 `Report`**：可以得到 group/level/text，但必须按版本指纹和调用约定绑定；不能 hook 共享空函数。
3. **安装受控 reporter**：最完整，可直接把 `LogRecord` 转为 OpenSCP schema，但这是 native 注入/插件能力，不是现有 DBPF mod 能力。

## 最终判断

OpenSCP 当前 UI bridge 是一个有真实研究价值的、受限的运行时观察器，不是炫技 demo；它已经能帮助我们把“UI 动作 → bridge 请求/返回 → 游戏事件”串起来。它目前不能代表“mod 可以向引擎日志管线写入任意日志”，也不能单凭 UI console 观察模拟内部。

未来可以实现 mod debug，但应把能力拆成两层：数据/UI mod 通过页面 console 和已有状态接口提供可观测标记；native 诊断插件通过 EA::Trace reporter 或独立诊断通道输出模拟过程。OpenSCP 当前 bridge 应继续保持只读、版本绑定、有界和自动撤回，把主动触发 native 日志列为后续独立能力，而不是扩大现有 hook 的权限。
