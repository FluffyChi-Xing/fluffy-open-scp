# SimCity UI 观察器（实验版）

观察游戏现有 UI 命令、查询与事件，向本机接收器批量发送 JSONL。此工具是未来 OpenSCP 控制台的采集基础，尚未接入应用界面，也不提供游戏命令执行或回放。

支持的 EXE / scrui 指纹见 [协议研究](../../../docs/re/retail-ui-command-event-protocol.md)。旧覆盖包方案不依赖 Frida；文末原生观察器使用 Frida。两种方案均不挂钩 D3D9，不修改 EXE 或原版 package。

## 当前状态：完整观察包已停用

2026-10-12，`000_` 修正版进入脚本内存后，实机出现黑屏、有背景音乐但未崩溃，接收器仍为零记录。卸载后用户确认主菜单恢复。全部实验包已撤回；此覆盖脚本路线未成功捕获。后续原生路线已捕获命令、查询和标量返回值。源码进入内存不证明执行成功，黑屏的具体原因仍未定位。

`prepare` 默认生成 **baseline**：同一资源原文重新打包，不追加代码。`--stage marker` 只追加静态标记；这两阶段也尚未通过实机验证，不应称为安全版本。`--stage observe` 仅供离线研究；`install` 拒绝完整观察器以及缺少 stage 的旧 manifest，避免重复安装已知失败方案。

当前已完成 EAWebKit 原生桥的命令参数及查询返回采样（见本文末尾），原始覆盖脚本仍停用。运行时研究见 [运行时研究](../../../docs/re/retail-eawebkit-runtime.md)。以下是工具用法，不表示应重新安装实验包。

## 离线准备

在仓库根目录运行：

```powershell
cargo build -p probes --example ui_observer_package
python tools/dynamic/ui-observer/trace.py prepare tmp/my-ui-trace --exe "D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCity\SimCity.exe"
# 默认 baseline 只用于隔离“资源重打包”和“追加脚本”两类变量。
# 当前不安装到游戏；baseline / marker 均不发送日志。
```

已有实验会话可以离线查看状态与结果：

```powershell
python tools/dynamic/ui-observer/trace.py status tmp/my-ui-trace
python tools/dynamic/ui-observer/trace.py report tmp/my-ui-trace
```

`status` 区分接收器存活与实际记录数量。零条记录不能视为观察包生效。`report` 生成 `analysis.json`，提取命令/查询/事件计数、`RegionTemplates` 响应的城市数量/UID 和城市选中事件。缺失或截断的数量显示 `null`，不能当成 0。

所有命令支持全局 `--lang zh-CN` / `--lang en-US`（放在子命令前）；JSON 字段与协议标识保持稳定，适合后续 OpenSCP 读取。默认端口 18765，可在 prepare 时用 `--port` 改变。`start` 在 Windows 使用隐藏的后台进程；也可用 `serve` 在当前终端运行。

## 停止和撤回

```powershell
python tools/dynamic/ui-observer/trace.py stop tmp/my-ui-trace
# 保存进度并关闭游戏后：
python tools/dynamic/ui-observer/trace.py uninstall tmp/my-ui-trace
```

停止接收器不会卸载游戏页面中的包装函数；页面会限量缓存并退避重试。退出游戏并撤回 package 后，下次启动不加载观察器。卸载前校验已安装文件的哈希，若被其他工具改变则保留文件并报错。原版 App.package 始终只读。

## 数据与实现边界

- 单资源覆盖：`67771F5C:40464200:7D14BE70`，在原始 scrui 末尾追加 ES5 代码。
- 包构建采用现有 DBPF v3 writer，并通过 reader 回读逐字节验证。提取基线与当前 App 资源必须一致。
- 包装 `PostGameCommand`、`RequestGameData` 和 `ReceiveGameEvent`，保留 `this`、返回值、异常以及已有回调的上下文、参数与调用次数。
- 不增加游戏查询、不切换 UI 事件模式、不读取或抢占事件 token。
- 每页独立随机 page ID；记录有顺序号、时间、通道、阶段、名称/事件 ID、payload 和关联请求顺序号。
- 快照不调用 getter / `toJSON`。数组以 `{$array:[{key,value}],length}` 表示；`$cut` / `$type` 显式表示截断、访问器、循环或非 JSON 值。日志是有界诊断快照，不是无损游戏状态。
- 每次遍历最多 2048 节点、10 层、每对象 128 个属性、每字符串 4096 字符，总文本预算 32768 字符。单条记录限长，内存队列最多 512 条或 2 Mi 字符；满时删除最旧条目并累计 dropped。
- 每 250 ms 最多发送一个异步批次（32 条、约 192 Ki 字符），一个在途请求，3 秒超时；失败指数退避至 30 秒。ready 和每 10 秒 heartbeat 提供模式与加载证据。
- 接收器只绑定 `127.0.0.1`，随机 capability URL，支持旧 WebKit 的 text/plain POST 和 CORS。EAWebKit 是否允许该外部请求仍须实机证明。
- 接收器拒绝错误 build/schema/顺序号，按 page/sequence 去重重试；每次落盘后确认，重启从 JSONL 恢复去重状态。最多 256 个页面、64 MiB 日志；达到上限返回 507，不无限占用磁盘。
- `manifest.json` 内含仅限本机使用的 token。接收器不提供游戏控制 API；日志及覆盖资源只存忽略的 `tmp/`，不提交原版脚本。

会话目录：

| 文件 | 内容 |
|---|---|
| `manifest.json` | EXE/脚本/产物指纹、build、安装位置与接收配置 |
| `scrui.baseline.js` | 从当前 App.package 提取的原版基线 |
| `observer.configured.js` | 含本次接收配置的观察器 |
| `000_openscp_ui_observer.package` | 当前候选覆盖包 |
| `trace.jsonl` | 真实收到的记录；无记录时不生成 |
| `status.json` / `analysis.json` | 捕获计数 / 离线语义汇总 |
| `collector.log` | 接收器启动与诊断信息 |
| `memory-probe.json` | 可选只读加载诊断 |

## 零记录排查

先确认 `collectorRunning`。若接收器运行且游戏已操作，使用当前游戏 PID：

```powershell
Get-Process SimCity
python tools/dynamic/ui-observer/trace.py probe tmp/my-ui-trace --pid <PID>
```

probe 只使用 `OpenProcess(QUERY_INFORMATION | VM_READ)`、`VirtualQueryEx`、`ReadProcessMemory`；最多扫描 20 秒/1.5 GiB 的 32 位地址范围内私有可读内存，不注入、不写进程、不调用引擎函数、不转储完整内存。它查找 build/观察器/原版标记；命中标记不单独证明执行，未命中也只是有界扫描结果。只有实际收到 ready/heartbeat 和操作记录，才能闭合运行时验证。

2026-10-12 首轮：使用 `zzz_` 文件名，用户反馈启动和操作正常，但捕获为 0；只读比对找到与基包完整一致的 680785 字节 scrui，未含观察器。随后扫描找到 `zzz_` 包名但未找到观察器标记。这支持“包被枚举但资源未胜出”的判断，尚不等于已反推出所有加载规则。

开发版 `InternalDatabaseList::Add`（1208693 行）在 `existingPriority >= newPriority` 时继续向后插入，等优先级保留先注册者在前。当前 App 的 package-info 未见 priority 属性 `0x06EF59E5`。第二轮 `000_` 包的观察器和 build 标记已在脚本内存中找到，但导致黑屏，随后撤回。不能由此证明脚本执行成功，也不保证所有版本或所有目录按文件名字典序加载。`SimCityUserData/Packages` 也参与冲突扫描。

## 验证命令

```powershell
node --test tools/dynamic/ui-observer/observer.test.cjs
python -m unittest discover -s tools/dynamic/ui-observer -p test_trace.py -v
node tools/dynamic/verify_ui_bridge_contract.cjs tmp/ui-bridge-retail/scrui-7D14BE70.js tools/dynamic/ui-observer/observer.js
```

覆盖回调/返回/异常保持、快照上限、撤回包装、异步传输与重试、接收端真实 HTTP/CORS、持久化去重、日志上限以及模板汇总。另已用 Acorn 的 ES5 模式解析观察器。测试样本使用临时目录与独立测试 build，不写入实机捕获日志。

## EAWebKit 静态核对

`eawebkit_audit.py <EAWebKit.dll> --output <report.json>` 按精确哈希核对库接口、client 全局指针读取位置和 DebugLog 调用路径；只读取磁盘文件，不加载 DLL。支持 `--lang zh-CN|en-US`。这不是运行时捕获工具，结果明确标记活跃 client/View 尚未验证。


## EAWebKit 运行时只读核对

```powershell
python tools/dynamic/ui-observer/eawebkit_runtime.py --pid <当前PID> --scan-views --output tmp/ui-webkit/runtime.json
```

仅接受登记过指纹的 EXE/DLL。读取模块、client 虚表、网页对象、当前 frame URL 和 Client 回调注册表；不加载 DLL、不写进程、不调用引擎。扫描覆盖完整 x86 地址空间，最多 20 秒/1.5 GiB/32 个候选；快照不是原子的，扫描未命中不能证明不存在。支持 `--lang zh-CN|en-US`。

2026-10-12 PID 14332 实机确认：DebugLog 为共享的空函数；Inspector 工厂返回空；Launcher3D 和 WebKitPanel 两个页面已绑定 7 项原生回调。用户确认主菜单及操作正常。回调注册表不等于调用日志，本工具不捕获瞬时指令流。

```powershell
python -m unittest discover -s tools/dynamic/ui-observer -p test_eawebkit_runtime.py -v
```


## 原生观察器（实验版，临时修改进程入口）

`native_trace.py` 与只读核对工具不同：使用 Frida 17 的 QJS agent 临时挂钩两个已确认的函数入口，退出时卸载。它不调用游戏 NativeFunction，不执行页面脚本，不读取事件队列，不修改安装目录。不能将其标为进程只读。

依赖 Python `frida`（本轮 17.19.0）。先确认 UI 已正常运行；每轮使用新目录，避免合并不同实验。默认 60 秒，允许 5–300 秒，Ctrl+C 或截止时间触发卸载。现有动态工具记录过反复 attach/detach 的风险，避免同一进程反复试探。

```powershell
python tools/dynamic/ui-observer/native_trace.py --pid <当前PID> --seconds 120 --directory tmp/ui-native-new
```

原生观察器支持有界采样预设：`balanced`（默认）、`ui-events`、`bridge-commands`、`engine-messages`。预设只调整已验证 UI bridge / EAWebKit 通道的每秒额度，不代表资源、建筑或交通模拟 native 日志；这些专题仍需独立探针。

```powershell
python tools/dynamic/ui-observer/native_trace.py --pid <当前PID> --profile ui-events --seconds 60 --directory tmp/ui-native-events
```

准备阶段重新校验 EXE/DLL 指纹、绑定对象和入口字节，不接受任意地址参数。目标为 EXE RVA `0x632930`（Client.invokeMethod）和 DLL RVA `0x5AC490`（ChromeClientEA::addMessageToConsole）。后者是已核对的日志构造函数入口，不 hook 多个回调共用的 DebugLog 空函数。目标均用实际模块基址加 RVA。

第一类记录只有方法名、参数数量、线程、绑定页面、序号和时间；不读取 JavascriptValue 参数内容。第二类复制最多 512 UTF-16 字符的控制台文本/来源、行号和等级。两类保留原调用；没有返回值替换。页面切换产生的新绑定不自动纳入首轮配置。

命令、查询、其他调用、日志、返回值、事件使用独立队列，每类最多 128 条；查询默认 40/s，其他类 80/s。每 250 ms 最多回传 64 条且只允许一个未确认批次，日志最多 8 MiB。丢弃和解码错误均显式计数；这是采样流。 进程内 onEnter 仍会被每次原调用触发，限流不等于消除了挂钩开销。

产物为 `preflight.json`、`config.json`、`agent.js`、`trace.jsonl`、`status.json`；首轮另保存 `analysis.json` 和 `restoration.json`。本机路径、地址和时间是诊断元数据，不应把临时堆地址当固定接口。

2026-10-12 实机首轮 `tmp/ui-native-v1`：9,395 条记录，其中 PostGameCommand 51、RequestGameData 9,344；线程 51960，参数数目均为 1。53,828 次被限流/队列上限丢弃，解码错误 0、agent 错误 0、控制台记录 0。120 秒后自动卸载/detach，外部只读检查确认两个入口字节恢复，进程仍响应。用户反馈已实际进入存档，画面/操作正常、没有明显卡顿；没有记录阶段转换时间，不能把这些调用归类为创建区域页测试。没有捕获参数字符串、查询结果或事件 payload，不能将每次 PostGameCommand 直接等同于一次玩家动作。

独立 32 位测试程序 `native_fixture.cpp` 验证 x86 ECX/栈参数、两线程、UTF-16 日志、限流和卸载后的原字节/行为；共 520 次原调用通过。测试程序不是注入游戏的 DLL。

```powershell
# 在 Visual Studio x86 Native Tools 环境中编译独立 fixture：
cl /nologo /EHsc /O2 /MT /std:c++17 tools/dynamic/ui-observer/native_fixture.cpp /Fo:tmp/ui-webkit/native_fixture.obj /Fe:tmp/ui-webkit/native_fixture.exe
python tools/dynamic/ui-observer/test_native_fixture.py tmp/ui-webkit/native_fixture.exe
```
## Native observer v4 / 原生观察器第四版

`native_trace.py` now uses independent command/query/other/console/result/event quotas and queues.
It copies bounded flat JavaScriptCore strings and simple scalar arguments without
calling game getters. Rope strings and unsupported values remain explicitly marked.
It temporarily hooks three native entrypoints; it is **not a process-read-only tool**.

现已分别限制命令、查询、其他调用和日志的额度及队列，防止查询洪泛挤占命令。
直接读取有界 flat 字符串和简单标量；rope 与不支持类型明确标记，不调用游戏 getter。
工具临时挂接三个原生入口，**不是纯只读进程工具**。

Offline report / 离线报告：

```powershell
python tools/dynamic/ui-observer/analyze_native.py tmp/ui-native-v2
python tools/dynamic/ui-observer/analyze_native.py --lang en-US tmp/ui-native-v2
```

`analysis.json` preserves representative raw strings, groups command/query prefixes,
and includes per-channel sampling counts and paired scalar query results. Reverse events are filtered by verified native call sites; object payloads remain unsupported.
报告保留代表性原文、按命令/查询名称汇总并包含分通道丢弃计数；含按 callSequence 关联的标量查询返回；包含已验证来源的反向事件；对象 payload 暂不展开。


Third capture / 第三轮：`tmp/ui-native-v3` contains 1,200 query/result pairs and 13 commands; 43,802 query samples dropped, no command/result drops. 用户确认正常、没有明显卡顿。Objects/arrays remain unsupported; object getters are never called. 对象和数组不展开，不调用 getter。

第四轮 / Fourth capture: `tmp/ui-native-v4`, 1258 events / 19 IDs, 158 commands,
2400 paired queries/results. No command/event sample drops. Query drops: 64682.
1031 event payload objects remain unsupported; only their outer [ID, data] arrays are decoded.
1031 条事件的对象字段尚未展开；目前只读出外层数组。三处入口已恢复，画面反馈待确认。

```powershell
python tools/dynamic/ui-observer/analyze_native.py tmp/ui-native-v4 --ui-script tmp/ui-bridge-retail/game-7CCC548C.js
```

## JSON samples and object fields / JSON 样本与对象字段

```powershell
python tools/dynamic/ui-observer/analyze_native.py tmp/ui-native-v5 --ui-script tmp/ui-bridge-retail/game-7CCC548C.js --export-dir tmp/ui-log-samples/v5
```

Export requires a new directory. 输出目录必须新建，避免覆盖历史样本。
`timeline.json` 保留全时间线；`commands.json` 命令；`queries.json` 查询返回配对；
`events.json` 事件；`event-catalog.json` 事件名称与样例；`manifest.json` 来源与文件哈希。
The files retain type and truncation markers; unsupported/depth-limited values are not null.
类型、截断和深度限制标记保留，未读取值不伪装成 null。

v5 reads bounded own data properties of verified plain objects, with no getter calls.
第五轮已读取普通对象的有界自有属性，跳过访问器，不遍历原型链。
User confirmed both v4 and v5 ran normally without noticeable lag. 用户确认第四、第五轮均无明显卡顿。


## Historical checks / 历史问题覆盖检查

```powershell
python tools/dynamic/ui-observer/historical_checks.py tmp/ui-log-samples/v5 --output tmp/ui-log-samples/historical-checks-v5
```

The report distinguishes observed-save signals from missing create-region, localization,
and resource-reload evidence. 报告区分已有存档信号与创建区域、词条、资源重载证据。


## 稀有写请求独立采样（2026-10-13）

`SaveLocalGame`、`SaveRegionAs`、`UpdateBoxName`、`UpdateRegionName` 虽然通过
RequestGameData 传输，却具有写副作用。原生观察器先读取经过结构校验的、最多
32 个 UTF-16 单元的请求前缀，再决定采样通道。它们使用 `mutation` 与
`mutationResult` 独立额度和队列，通过 callSequence 关联；原始 channel/name
仍保留 invoke/RequestGameData 和 result，附加 operationKind=mutation。
普通查询依旧限流；独立通道本身也有上限，仍须检查 accepted/dropped。

每批 diagnostics 包含 unknownInvokeBinding、unknownEventHost 和
requestPrefixUnreadable。前两项用于揭示初始绑定之外的调用，不会自动信任新对象；
未知绑定仍被过滤。字符串不是已验证的 flat 结构时不尝试执行 getter 或展开 rope。

```powershell
node --test tools/dynamic/ui-observer/native_agent.test.cjs
python tools/dynamic/ui-observer/test_native_fixture.py tmp/ui-webkit/native_fixture.exe
```

前者验证普通查询额度饱和后四种写请求和返回仍可配对，并拒绝相似前缀；后者
验证独立 x86 进程的原调用、线程、解码和撤回后入口恢复。不会主动调用游戏 API。


## 已校验绑定刷新（2026-10-13）

零售配置启用 refreshBindings。未知调用仅上报有界 host 候选（每批最多 16）；
Python 在游戏回调外只读验证 View/Private/Page、宿主反向引用、注册方法和
Client 入口，并检查事件派发入口。探针再复核对象、宿主、view/private/page
双向指针与入口表后才登记，最多 64 个绑定。每轮最多验证 4 个候选，单 host
至少间隔 2 秒；记录最多 64 次刷新结果。未校验对象继续过滤，不调用引擎 API。

status.json 的 bindingRefresh 记录候选、时间、校验结果和新绑定；diagnostics
记录未知调用及成功更新次数。新绑定出现到验证完成之间仍存在漏采窗口，
这些调用不能补回；此机制不是完整生命周期 hook，也不保证捕获最早的加载事件。


## RegionTemplates 专项（2026-10-13）

`RegionTemplates` 请求/返回使用 template/templateResult 独立通道（各每秒 2 次）。
返回的专用解码上限为数组 32 项、对象 64 个自有属性、深度 12、12000 个值、
40 ms 时间预算；普通请求仍使用原有上限。返回 observation 标明耗时、读取节点数
和 deadlineReached。必须递归检查 truncated、unsupported 和深度限制，才能判定
实际完整性；声明数组长度与实际展开项数分开统计。仍不调用 getter 或引擎 API。


## 区域创建与身份专项（2026-10-13）

CreateRegion、CreateBox、GetRegionDetails、LatestBox、activeBoxId、ActiveCityUID、RegionID
使用 region/regionResult 独立通道，各每秒最多4次；前两项仍标注 operationKind=mutation。
返回使用与模板相同的有界展开预算，observation.profile=region-lifecycle。
这些通道仍可能限流，采样后必须检查 dropped 与每个字段的截断标志，不能声称全量。
回归测试验证普通查询饱和时创建/身份请求仍能配对、创建操作副作用标签、相似前缀拒绝，
以及专项额度耗尽后的丢弃。独立 x86 fixture 验证520次原调用、双线程和撤回恢复。
