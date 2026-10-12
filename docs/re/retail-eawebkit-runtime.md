# 离线发售版 EAWebKit 运行时与观察入口

更新：2026-10-12。结论适用于已登记指纹的本地零售版；开发版反编译用于解释调用链，不作为零售 hook 地址表。

## 结论

游戏内 HTML/CSS/JavaScript UI 由进程内的 **EAWebKit 12.4.2.2.2** 承载，脚本引擎是 **JavaScriptCore（JSC）**。没有 V8 并不影响其执行 JavaScript。EA 官方公开了同版本源码；`WebFrame::evaluateJavaScript` 直接调用 WebCore ScriptController 并使用 `JSC::JSValue` / `JSC::ExecState`。

观察运行时具有明确接口基础：页面求值、原生对象绑定和日志回调存在。后续只读实机采样已确认真实页面、Client 绑定和回调注册表；DebugLog 宿主为空处理，零售 Inspector 工厂返回空。**续轮原生探针已捕获真实调用元数据，参数和事件流尚未解码。** 只读阶段没有注入；后续限时实验使用了两个 Frida 入口钩子，结束后已卸载并核对字节恢复。没有替换 DLL 或重装 UI 覆盖包。

## 版本与可复核证据

- 本地 EXE SHA256：`2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。
- 当前游戏目录 EAWebKit.dll SHA256：`0be0e0c3104739cb56208f91d643c429c8262daff768b2afdcd1799444350e86`，9,602,224 字节，PE32 x86。
- 首轮核对来自恢复正常的游戏进程模块；续轮 PID 14332 已完成真实对象只读验证，见下方实机补充。
- [EA 官方 SimCity 源码页](https://gpl.ea.com/simcity.html) 列出 [12.4.2.2.2 源码包](https://gpl.ea.com/packages/EAWebKit_12.4.2.2.2.zip)。源码 `EAWebKitConfig.h:43` 的版本五元组与 DLL 字符串一致。
- 网络完整下载中断；相关源文件通过 ZIP Range 独立取得并校验各条目 CRC32。未将不完整 ZIP 当作完整源码包，也没有运行其中构建脚本。
- 本地证据：`tmp/ui-webkit/dll-audit.json`、`interface-audit.json`、`source-evidence.json`；源码缓存 `tmp/ui-webkit/sdk/`。

## 页面如何启动和持续运行

宿主启动链：

1. 游戏 UI 管理器初始化，注册游戏资源 transport 和 JavaScript callbacks。
2. `LoadLibraryW` 加载 EAWebKit.dll，`GetProcAddress(module, ordinal 1)` 取得 `CreateEAWebkitInstance`，获取库接口。
3. 初始化分配器、文件系统、文字系统、计时和宿主 client，创建 `View`，设置大小、用户数据和渲染接口。
4. `BindJavaScriptObject("Client", bindings)` 将原生回调对象放入 JavaScript window。
5. 构造 `game://localhost/resource/<resource-key>`，通过 `SetURI` 加载包内 HTML；后续脚本、样式等由资源路径解析。这里的 localhost 不等于必须运行 HTTP 服务。
6. EAWebKit 解析 HTML/CSS，JavaScriptCore 执行脚本；宿主向视图送入输入并安排更新。SDK `View::Tick` 推进 transport/timer/paint，游戏还可使用硬件渲染接口将 UI 合成到游戏画面。

开发版反编译 `docs/source-code/SimCity-dev-ida.c` 的对应锚点：`GetGameUIURL` 945960、UI 初始化 946358、`SP::InitWebKit` 1437933、视图初始化 1438100、绑定 Client 1438155、模块加载 1448386、ordinal 查找 1448406、系统初始化 1448521。开发版版本判断为 12.4.2.0.0，不能与当前 DLL 版本混淆。

精确版本 SDK 锚点（以下路径均相对 `tmp/ui-webkit/sdk/WebKit/ea/`）：

| 文件 / 行 | 证据 |
|---|---|
| `Api/EAWebKit/source/internal/source/EAWebKitDll.cpp:46` | 库工厂入口 |
| `Api/EAWebKit/include/EAWebKit/EAWebKit.h:310` | 库虚接口定义 |
| `Api/EAWebKit/include/EAWebKit/EAWebKitView.h:151` | EvaluateJavaScript、BindJavaScriptObject |
| `Api/EAWebKit/source/EAWebKitView.cpp:1272` | 求值前保护 FPU 精度和 GC 栈基址，进入 mainFrame |
| `Api/private/WebFrame.cpp:892` | executeScript → JSC 值与执行上下文 |
| `Api/private/WebFrame.cpp:412` | 原生对象绑定进入 JSC bindings |
| `Api/EAWebKit/source/EAWebkitJavascriptValue.cpp:54` | JavascriptValue 的 JSC 实现 |

因此，音乐仍响而 UI 黑屏与网页加载/脚本/渲染路径失败相容，但不能据此确定上次故障的具体原因。

## 两条值得继续的观察路径

### 先观察现成错误日志

`WebCoreSupport/ChromeClientEA.cpp:293` 的 `addMessageToConsole` 将消息格式化为：

```text
JSWarning: <source> (<line>): <message>
JSError: <source> (<line>): <message>
JS: <source> (<line>): <message>
```

随后调用 `EAWebKitClient::DebugLog(DebugLogInfo&)`。信息包含 View、用户数据、临时日志字符串和类型。`EAWebKitClient.h:256` 定义结构，`:715` 的默认处理为空函数，所以“引擎发出日志”不代表“游戏已经写日志文件”。消息先做 ASCII 转换，不能承诺非 ASCII 文本无损。

已在本地 DLL 中对应到日志格式串和 `client vtable + 0x0c` 的间接调用。下一步可先只读解析当前宿主 client 的虚表，判断 DebugLog 是空处理还是有真实落盘路径；若需要截获，复制日志必须在回调有效期内完成，并在另一侧限量输出，不能持有临时指针或在回调中执行 JS。

这一路优先解决“网页为什么失败”，并不天然包含每条游戏命令。

### 再观察游戏与页面的语义边界

默认通信是 `Client` 原生方法调用和游戏直接向页面推送事件。详见 [零售协议](retail-ui-command-event-protocol.md)。HTTP 批处理/事件队列是另一条路径，单纯抓网卡包不保证看到默认桥上的操作。

候选观察点：JS→原生的 `IJSBoundObject::invokeMethod` / 游戏 callback dispatcher，以及原生→JS 的 EvaluateJavaScript 和直接事件回调。不要记录所有页面求值为“玩家命令”：其中可能包含 UI 内部脚本、初始化和其他消息。

后续最小验证顺序是：确认活跃 View 与页面身份 → 确认所有者线程和生命周期 → 捕获自然发生的错误/消息 → 仅在已有执行线程的合适时机测试无副作用常量求值 → 单个语义点采样 → 有界事件导出。需要保留调用约定、返回值、异常、FPU 状态和 GC/对象所有权。

## Inspector 为什么还不能直接打开

`WebCoreSupport/InspectorClientEA.cpp:78` 的实际实现：

1. 向宿主 `EAWebKitClient::CreateView` 发送 `kEventInspectorOpen`。
2. 宿主必须填回 `mpCreatedView`；为空则不会建立调试器前端。
3. 加载 `file:///.../inspector/inspector.html`，创建前端 client。
4. 后端消息通过页面内的前端分发传递（`:141`），不是这里自行创建一个 Chrome 调试 TCP 服务。

SDK 还要求宿主在断点停住期间继续处理 Inspector 的输入、渲染和 tick（`EAWebKitClient.h:735`）。贸然开断点可能表现为冻结。

本地安装目录普通文件扫描没有发现 `inspector.html`；这不排除打包资源。`SimCityUserData/WebStorage/InspectorSettings.txt` 只有格式头，且 SDK 关闭时会保存设置，文件存在不能证明用户打开过 Inspector。之前端口 19117 的请求超时，仍不能认定它是 Inspector。

开发版存在 launch-inspector/debugger 配置和宿主窗口处理。续轮已确认零售保留打开分支，但其工厂为空实现，见下方补充；修改配置或添加通用 Chromium 参数不能补齐这条链路。

## 已核对的零售静态地址

以下全部为 **DLL RVA**，运行时必须加实际模块基址；不能加 EXE 基址，也不能直接套用首选基址。地址只适用于上述 DLL 哈希。

| 项目 | RVA / 偏移 |
|---|---|
| ordinal 1 工厂 | `0x5A4750` |
| 库单例 / 虚表 | `0x8B39E0` / `0x85C378` |
| 库虚表 slot 3：GetEAWebKitClient | `0x59AD70` |
| 该 getter 读取的 client 全局指针位置 | `0x8B2F34` |
| 日志格式化/dispatch 已核对代码窗口 | `0x5AC560` |
| DebugLog 的 client 虚表偏移 | `0x0C` |

新工具 `tools/dynamic/ui-observer/eawebkit_audit.py` 只读取文件、核对 SHA256/PE/关键指令字节并输出 JSON；不加载 DLL、不 attach、不调用引擎方法。它不宣称活跃 client/View 已验证。

```powershell
python tools/dynamic/ui-observer/eawebkit_audit.py '<游戏目录>/EAWebKit.dll' --output tmp/ui-webkit/interface-audit.json
```

## 已发生的回归与开发路线

v1 zzz 覆盖包：游戏正常，零记录，找到完整原版 scrui。v2 000 覆盖包：观察器与 build 标记出现在脚本内存，黑屏、有音乐、零记录；卸载后用户确认主菜单恢复。根因尚未定位，不能把“成功加载字节”写成“成功执行观察器”。

工具已禁止重装 observe/旧 manifest；默认离线 prepare 生成原文字节不变的 baseline 对照包。baseline 与 marker **尚未实机验证**。本轮通过 4 项 Node 测试、6 项 Python 测试、Rust 示例构建以及 baseline 包回读；这些检查不能替代旧 EAWebKit 实机兼容测试。

当前路线：先建立 native 日志诊断，再建立版本绑定的 Client/事件桥观察，最后接入 OpenSCP 日志面板和显式命令控制。避免替换整套 EAWebKit DLL：分配器、渲染、transport、ABI 和 JSC 对象所有权都需要一致，公开源码不等于能直接重编替换。

地图词条/槽位/资源消费分析仍沿原计划推进；实时桥用于验证实际消费者，不自动解决地图初始化或相机边界。相机 XY 限制仍需追踪原生约束，见 [地图消费者与相机边界](map-consumers-ui-bridge-camera.md)。

## 续轮实机：宿主、页面和 Client 注册表

用户按平常方式启动游戏；初次回复仍在播片。PID 14332 的只读采样已经完成，DLL 实际基址 `0x38FE0000`。不以工作集是否超过 1 GB 判断页面生命周期；本次采样约 420 MB 私有可读内存，在约 1 秒内找到两个通过反向指针校验的 View。只读采样不能单独证明画面已完成主菜单切换。随后用户确认已到主菜单且画面/操作正常，追加主菜单快照 `runtime-14332-main-menu.json`。

证据：`tmp/ui-webkit/runtime-14332.json`、`runtime-14332-bridge.json`、`native-bindings-14332.json`。新工具 [eawebkit_runtime.py](../../tools/dynamic/ui-observer/eawebkit_runtime.py) 使用 `OpenProcess(0x410)`、模块快照、VirtualQueryEx 和 ReadProcessMemory；没有写内存、暂停线程、注入 DLL 或调用虚函数。JSON 明确标记非原子快照。

### 日志当前被丢弃

库 client 全局指针指向 `0x00E197B4`，宿主虚表位于 EXE RVA `0x95F498`。slot 3（DebugLog）指向 EXE RVA `0x81E7C0`，实际机器码为 `C2 04 00`（`ret 4`），因此当前宿主直接忽略这条日志回调。

**不要直接 hook 这个 ret 函数地址。** GetLocalizedString、NetworkError、CursorMoved、ViewProcessStatus、JSMessageBox 等不同签名/语义的回调共享同一个空实现地址。若把所有调用都当 DebugLogInfo 解读，会误读其他结构。未来应针对指定 client 的 DebugLog 虚表槽，或已验证的日志分发调用点，避免在共享空实现上做全局 detour。

这只证明该回调不落盘，不排除游戏其他子系统有独立日志。也不能恢复已经发生且被丢弃的历史错误。

### 零售 Inspector 工厂确实为空

实机 CreateView slot 5 指向 EXE RVA `0x631900`，转发到 `0x62FC00`。其中 `eventType == 3` 对应 SDK 的 `kEventInspectorOpen`：当 `host + 0x148` 为空时，调用 EXE RVA `0x74BED0` 获取调试器对象。

该工厂在磁盘和进程内均为 `33 C0 C3`（`xor eax,eax; ret`）。当前两个 host 的 inspector 指针也均为零。故在本版本、当前空对象状态下，只调用 ShowInspector 或打开 launch-inspector 开关无法创建内置调试器。以后若实现 Inspector，需要补充宿主视图和事件循环，不是只找一个开关。

### 两个已绑定的页面

| View（本次进程临时地址） | 从当前 WebFrame 读取的 URL | 说明 |
|---|---|---|
| `0xFBBB0040` | `game://localhost/resource/0x40464200!0x82c29d9b.0xdd6233d6` | 从 App.package 导出的 HTML 是 packed Launcher3D 页面，加载 SCRUI_min / GameEntryUI_min，body onLoad 调用 `simcity.cLauncher3D.Initialize()` |
| `0xFBBB6F18` | `game://localhost/resource/WebKitPanel.html` | 辅助网页面板；不能把它当 Inspector |

原始 HTML 保存在 `tmp/ui-webkit/launcher-82c29d9b.html`。`ViewPrivate::mURL` 只是 GetURI 维护的缓存；第二个 View 的缓存为空，但当前 frame URL 非空，因此探针沿实机 accessor 对应的数据字段读取 `WebFramePrivate::url`，没有调用 GetURI。

两个 View 均在 2 GB 以上。工具扫描范围为完整 x86 地址空间，限 20 秒/1.5 GiB 读取/32 个候选；未命中不表示不存在。候选还须满足 `View + 4 -> private`、`private -> View`、有效 Page，以及宿主 `host + 0xB0 -> View` 等关联。堆地址仅用于本次快照，不能硬编码到下一次会话。

### 原生桥从静态推测推进到真实注册表

两个页面各有自己的绑定对象，绑定虚表 EXE RVA `0x95F5DC`，`invokeMethod` 入口 `0x632930`，转发到 `cEAWebKitWebView::InvokeJavaScriptCallback` 对应的零售 RVA `0x631DA0`。绑定对象反向指针与 host 一致。

从 host 回调 hashtable 读出各 7 项；前三项的原生处理器在两个页面间共享：

| 注册名 | OnCallback 的 EXE RVA |
|---|---|
| PostGameCommand | `0x499EC0` |
| RequestGameData | `0x499DF0` |
| RequestGameEvents | `0x4993C0` |
| Set3DCoords / Set3DViewSize / Set3DMode / Set3DViewWorldSize | `0x635BE0` |

处理器名来自运行时容器中的真实字符串，入口与已有零售静态分析吻合。**注册表是可调用能力清单，不是已发生的操作记录；RequestGameEvents 注册存在也不证明 UI 正在用队列模式。** Set3D* 关联网页/3D 合成，不能凭名称认定它解除世界摄像机边界。

## 最小观察实验的设计边界（后续实现见下节）

本阶段已完成对象和接口只读验证。下一阶段若开发进程内探针，应先仅捕获明确的 DebugLogInfo 文本，以及指定 Client 绑定的 `invokeMethod` 方法名、参数数量、线程 ID 和时间；保持原调用，暂不调用 EvaluateJavaScript、不遍历 JavascriptValue、不请求事件队列。

输出通道宜用有界本机缓冲区/IPC，避免再让启动页面承担序列化和跨域 XHR。只有在捕获到真实调用并确认线程后，才研究结果值、事件 payload 和小范围脚本求值；Inspector 宿主补建放在更后阶段。上述是下一阶段设计，尚未安装或实现注入器。

本轮验证：真实进程只读采样通过；5 项边界测试覆盖高地址、空 URL 缓存、错误反向指针、异常字符串长度和回调容器环；中英文命令帮助均已检查。


## 原生观察首轮：已捕获调用元数据

实现见 `tools/dynamic/ui-observer/native_trace.py` / `native_agent.js`。先用独立 x86 fixture 验证，再对 PID 14332 执行一次 120 秒采样；源代码与版本配置固定，游戏恢复正常后才接入。

最终选用两个真实函数入口：Client.invokeMethod（EXE RVA `0x632930`）以及日志构造函数 ChromeClientEA::addMessageToConsole（DLL RVA `0x5AC490`），避免修改共享 DebugLog 空实现或猜测整个虚表的替换生命周期。启动时再次匹配入口字节与绑定反向指针。Frida 使用独立 QJS，与游戏 JSC 不同；未执行游戏 NativeFunction/EvaluateJavaScript，未读取破坏性事件队列。

证据目录：`tmp/ui-native-v1/`。`analysis.json` 汇总：

| 项目 | 实测 |
|---|---|
| 保存的调用元数据 | 9,395 条 |
| PostGameCommand | 51 条 |
| RequestGameData | 9,344 条 |
| 记录中的线程 ID | 全部 51960 |
| 原生接口参数数目 | 全部 1 |
| 丢弃计数 | 53,828 |
| 解码错误 / agent 错误 | 0 / 0 |
| 实际控制台消息 | 0 |
| 结束状态 | deadline；script unloaded；session detached |
| 外部只读核验 | 两个入口原字节均恢复；进程仍响应 |

单个实例（来自实机 JSONL，字段节选）：

```json
{"channel":"invoke","name":"PostGameCommand","argumentCount":1,"threadId":51960,"sequence":1}
```

这是方法边界真实发生的调用，证明 UI→原生桥可以被观测；它不含 `command/args` 字符串，也没有证明一条记录对应一次用户操作。未捕获事件、查询结果、游戏 console 文本或历史黑屏错误。用户反馈游戏已实际进入存档，不能将本轮标记为创建区域页切换实验；阶段转换时间未记录，不能为每条记录指定主菜单/存档阶段。随后用户明确确认进入存档后画面和操作正常、没有明显卡顿。此反馈支持本轮短时稳定性，不等同于长期性能验证。

当前共享每秒 200 条额度导致查询大量占用记录预算。因此下一版应在原生侧或独立语义入口按命令/查询分开计数和限流，同时保留丢弃统计；仅减少落盘量无法消除每次 onEnter 的运行时开销。随后只解码有边界、无 getter 调用的字符串/数值参数，最后再接事件方向。此次不重复 attach 做负载扩展。

独立 fixture 确认 520 次原调用参数/返回/后续行为不变，两个线程可记录，消息字段和入口恢复正确。早期 fixture 暴露定长 readCString 会包含尾部字节的问题，已改成限 96 字节且遇 NUL 停止读取后通过；该失败未在游戏中尝试。

旧 `safe_attach.py` 的默认 HOOKS 使用 EXE VA，却在 agent 中再次加模块基址；新工具使用独立核对的 RVA，不复用该模板。这个地址计算缺陷不能单独解释过去每一次 D3D9 崩溃。

## 第二轮：真实命令和查询字符串（2026-10-12）

用户继续授权后，完成按 command/query/other/console 独立额度与独立队列的观察器。每类队列最多 128 条，默认 command 80/s、query 40/s；批次按 sequence 合并。限流仅丢弃观察记录，不阻断游戏调用。每次 invoke 的 Frida 回调仍执行，不能将采样量降低等同于消除运行开销。

字符串布局同时由同版本 SDK 和实际 DLL 机器码确认：

| 结构 | 已验证布局 | DLL RVA 证据 |
|---|---|---|
| JavascriptValue | 大小 24；vtable `0x778A88`；payload +8、tag +12 | 构造 `0x59DCA0`、分配 `0x59B263` |
| JSValue cell | tag `0xFFFFFFFB`；cell+4 指向 Structure，Structure+8 字节为 5 时是 String | Type `0x59E173` |
| JSString | length +8；StringImpl* +12；fiberCount +16 | getString `0x65DE80`，SDK JSString.h |
| StringImpl | length +4、UTF-16 数据指针 +8 | `0x59DE20`、`0x59DE36` |

只复制 flat string；每参数最多 2048 UTF-16 单元、每调用最多 4 参数。rope 不展开，无效指针、类型或长度明确标记 unsupported，不调用 getter 或游戏函数。包装 vtable 按 DLL 实际基址校验。支持 int32、boolean、null、undefined；double/object 暂不解码。

独立 x86 fixture 通过：520 次原调用行为正常、双线程；查询洪泛中 10 条命令全保留，查询丢弃 470；flat/rope/超长/无效指针处理通过，两个入口字节恢复。首次测试的 3 秒窗口短于 Windows Sleep(2) 实际调度后的工作时间，改为 6 秒后覆盖完整测试；不是游戏侧失败。

实机证据 `tmp/ui-native-v2/`：PID 14332，60 秒，保存 2486 条；86 条 PostGameCommand 全部记录（观察到的绑定中丢弃 0），65889 次 RequestGameData 保留 2400、丢弃 63489。全部保存参数为 flat string；agent/decode 错误 0。日志入口命中 0、反向事件未捕获。自动 deadline、unload、detach 完成，外部 Reader 校验两个入口原字节恢复，进程仍响应。本轮画面/卡顿情况等待用户反馈，不能沿用首轮主观反馈。

真实参数示例：`regiondata/2`、`toggleLayer/51886653/false`、`deselecttool`、`RequestGreatWorksContributors`、`resourceAmount/932594546`、`cachedUnits/3133537757/binAmount/932594546`。`RegionPlayers` 的后缀为 JSON，包含城市和用户 ID；解析时只按第一个 `/` 分离名称与后缀，保留原始字符串，不统一假设每段都是位置参数。86 条命令中 51 条为 sendsoundevent，不能当成 86 次用户操作。

本轮没有记录摄像机名称的命令；用户动作尚未精确对时，因此这不证明摄像机没有通过其他桥接或原生输入路径运行。资源查询提供新的消费者入口，但没有返回值，尚不能证明资源初始化或存档持久化。下一步追踪查询返回和既有事件推送入口；摄像机仍需定位实际控制器的约束函数。

## 第三轮：查询与返回配对（2026-10-12）

`invokeMethod` 的第四个栈参数是 JavascriptValue* resultOut，同版 SDK 声明及 EXE RVA `0x631DA0` 分发函数均支持此结论。在原函数 onLeave 中，仅对已采样的 RequestGameData 读取输出，记录低 8 位 bool 返回与 callSequence；返回 false 则不读取输出。新增独立 result 队列和丢弃计数。double 根据 JSC JSValue32_64 的 tag 边界直接读取；非有限值转为显式字符串。

独立 x86 fixture 验证 520 次原调用，双线程查询/返回关联、int32 和 double 值、原始返回行为、命令不被查询挤占，以及撤回恢复字节。既有运行时读取单测 5 项通过。

实机证据 `tmp/ui-native-v3/`，PID 14332、30 秒：

| 项目 | 实测 |
|---|---|
| 查询/返回配对 | 1200 / 1200，未匹配 0 |
| 命令 | 13；命令丢弃 0 |
| 查询调用 / 采样丢弃 | 45002 / 43802 |
| 返回值采样丢弃 | 0 |
| 返回类型 | int32 823、double 82、boolean 26、null 90、string 2、unsupported cell 177 |
| agent / 解码异常 | 0 / 0；unsupported 不算解码成功 |
| 撤回 | deadline、unloaded、detached；外部核对两处入口字节恢复 |
| 用户反馈 | 正常，没有明显卡顿 |

实例：`resourceAmount/932594546` 与 `binAmount/932594546` 均出现返回 int32 `1709560`；当前包 game UI 的 `simcity.kResourceIDSimoleons=932594546` 将其映射到模拟币。由此连起 UI 常量、运行查询和实际结果，不能据此推断地图初始资源分布、写入能力或持久化效果。`totalElapsedGameTime` 为 double（例 `1560.06396484375`），`globalFlags/2` 为 boolean true；这些都是窗口中的样本值，不是固定配置。

返回对象和数组目前不展开；未捕获反向事件或 console 消息。新增 onLeave 仍有每次调用的 Frida 开销，短时稳定性不代替长期负载测量。当前只识别采样启动时的绑定；动态新增页面不自动纳入。

## 第四轮：反向事件实测（2026-10-12）

新增第三个临时入口：EXE RVA `0x62F500`，thiscall，3 个栈参数（函数引用、结果引用、参数引用），ret12。只接受当前已验证宿主，且返回地址须为 `0x22DC8B` 或 `0x369D7C`。这两处静态派发代码使用 `ReceiveGameEventFromClient` 包装函数；生产启动前校验宿主虚表 +0x50、入口字节和调用点 21 字节。没有把通用 native→JS 调用都当作事件；没有读取/删除事件队列。

同版 DLL 机器码核实 JSArray：构造函数 `0x667930` 写 vtable RVA `0x870CD0`；初始化 `0x666CF0` 写 capacity +0x30、storage +0x38；GetArrayLength `0x59D940` 从 storage+0 取长度；GetArrayValue `0x59DF50` 从 storage+0x18+index*8 取 JSValue。仅接受精确数组虚表，最多展开 2 层、每数组 8 项、每值树 32 个值；稀疏/惰性数组跳过，循环在深度限制处停止，空槽显式标记 hole，截断显式记录。

独立 fixture：原调用 520 次验证通过；双线程反向事件、错误宿主/错误来源过滤、自引用数组深度限制、稀疏数组与异常长度均通过。3 个入口字节恢复。

实机 `tmp/ui-native-v4/`，60 秒，PID 14332：保存 6216 条记录——158 命令、2400 查询、2400 对应结果、1258 事件。查询 67082 次中丢弃 64682 次；命令/结果/事件采样丢弃均为 0。事件共 19 个 ID，全部可在已提取 game UI 脚本中找到数值常量名称；映射源 SHA256 写入 analysis.json，数字 ID 保留为主键。所有记录来自已验证的 `0x369D7C` 派发点。

| 事件常量 | ID | 次数 |
|---|---:|---:|
| kGameMessageUnitRuleUIEvent | 199450092 | 962 |
| kGameMessageBeatCountChanged | 237557144 | 120 |
| kMessageGameCameraMovingStart | 205419630 | 26 |
| kMessageGameCameraMovingStop | 205419631 | 26 |
| kGameMessageLayerToggled | 182943898 | 20 |
| kGameMessageBuildingRolledOver | 245515571 | 13 |

事件参数为 int32 ID 225 条、double ID 2 条、`[ID, data]` 数组 1031 条。**1031 条数组事件的 data 都是尚未展开的对象**；不能称为已经完整解码事件 payload。查询结果另有 43 条数组成功识别。agent/解码异常 0，但 unsupported 仍是未解码数据。自动卸载、detach 完成，外部只读检查 3 个入口字节全部恢复；本轮主观稳定性反馈待用户回复。

离线报告支持 `analyze_native.py <capture-dir> --ui-script <extracted-game-ui.js>`，不执行 UI 脚本，只提取直接数值赋值；名称仅作注释，不能据名称猜测 payload。后续应读取普通数据属性的有界快照、建立事件与 query/command 时间线，再接入 OpenSCP 界面。

## 第五轮：对象参数与 JSON 导出（2026-10-12）

用户确认第四轮无卡顿。按用户要求导出 `tmp/ui-log-samples/v4/` 和 `v5/`，含 timeline、commands、queries、events、event-catalog、summary、status、restoration、manifest 等 JSON。manifest 保存源 JSONL 和导出文件哈希；全部文件重新解析和校验通过。`analyze_native.py --export-dir` 要求新目录，避免覆盖历史样本。

普通对象只接受 JSFinalObject 精确虚表 RVA `0x777B08`（DLL 创建代码 `0x2BC20`）。`0x6330` 核实 object+8 为 propertyStorage、Structure+0x34 为属性表；`0x5540`/`0x661160` 核实表头和 16 字节条目；`0x661F70` 核实变更链 previous +0x1C、name +0x20、offset +0x3C。直接读自有数据属性，不查原型链、不调用 getter。Structure+0x44 bit1 表示含访问器，整对象跳过。

限制：对象深度 3、每对象 12 属性、整个值树 32 值、容量 256、变更链 16 步且检测循环、属性名 128 UTF-16 单元。数组仍限制深度 2。properties 用条目数组存储，避免将任意属性名写到 JS 原型上。truncated:false 仅表示本节点枚举未截断，不表示子节点完整。

fixture 通过物化属性表、删除条目、变更链、访问器跳过、循环对象深度限制；520 次原调用和三入口恢复仍通过。

实机 `tmp/ui-native-v5/`，30 秒：3315 记录，包括 75 命令、1200 查询、1200 返回、840 事件（25 ID）。查询采样丢弃 26393，命令/返回/事件丢弃 0。事件中成功识别 1325 个对象节点；619 个嵌套数组仍为 depth_limit。查询返回识别 230 个对象。agent/解码异常 0，自动撤回后三个入口字节恢复。用户确认正常、无明显卡顿。

实测字段：建筑事件 unitRef={slot,stamp}；图层事件 layerID/isEnabled；工具事件 toolID/actionID/cursorKey；EditorActivated 事件 propKey={group,instance,type}、unitRef 及分类 ID；UnitRuleUIEvent 含内部 eventID、eventPosition、unitRef。其中 eventPosition 本轮深度受限。

unitRef.slot 是模拟对象引用的槽号，**不是区域地图的城市槽位**。外层消息 ID 和 payload 内规则 eventID 也须分开。现在可沿工具资源键、单位引用与查询结果关联消费路径，但地图模板/区域 box 映射和摄像机 XY 约束尚未解决。

本轮之后用户反馈观察器注入过程“几乎无感”，第五轮结束后确认正常、无明显卡顿。这是当前短时实机证据，不能替代长期负载测试。

## 历史遗留问题覆盖检查

新增离线工具 `tools/dynamic/ui-observer/historical_checks.py`，对已导出的时间线生成覆盖报告，不附加进程、不注入游戏：

```powershell
python tools/dynamic/ui-observer/historical_checks.py tmp/ui-log-samples/v4 --output tmp/ui-log-samples/historical-checks-v4
python tools/dynamic/ui-observer/historical_checks.py tmp/ui-log-samples/v5 --output tmp/ui-log-samples/historical-checks-v5
```

结果显示：

| 历史问题 | v4/v5 证据 | 当前结论 |
|---|---|---|
| 模板城市数量、boxes 对齐 | 未见 `RegionTemplates` / `LatestBox` | 必须在创建区域页专项采样 |
| 区域城市槽位 | v4 有 `regiondata`、`RegionPlayers` 命令 | 只证明命令发出，不含返回 boxes/cities |
| 词条覆盖 | 两轮都没有 locale 加载/显示值信号 | 需要页面加载和实际显示值对拍 |
| 资源消费 | `resourceAmount`、`binAmount`、`cachedUnits`、`bins`、`regionUnits` | 已有城市读取链已证实，初始化/保存重载仍未证实 |
| 摄像机 | 移动 start/stop 事件、`screenPositionCenter`/`origin` 查询 | 可验证运动时间线，不能证明边界已解除 |

这避免把已有城市存档的运行流误判成创建区域证据。报告还生成三个下一步场景：创建区域、资源保存重载、四边摄像机移动；当前不自动操作页面，需在用户可观察的场景下执行。

## 创建区域专项 v4/v6（2026-10-12）

在页面稳定停留后，`tmp/ui-log-samples/create-region-v6/` 捕获到真实创建区域链路。`CreateRegion/33333/248/true` 返回一个区域对象：`boxId=66`、`id`、`map.cities` 和 `map.greatWorks`。本轮 `map.cities` 为 5 个槽位：

```text
1026 (-1536, 2560, -870)
1027 ( 1024,  760, -870)
1028 ( 2550,-1856, -870)
1029 (    0,-2584, -870)
1030 (-2560,-1656, -855)
```

同一响应包含 5 个 boxes（67–71），每个新 box 的 `regionTemplateName=Confluence`，资源初始化包含 `mayor_Rating.amount=0`、`simoleons.amount=0`、`simoleonsForFrontEnd.amount=50000`、`total_Residential.amount=0`。`GetRegionDetails/9` 重复返回同一 map/boxes；`PlayingRegions` 返回包含 boxes 的 regions 集合。`CreateBox/7/55/...` 返回认领后的 box 对象，`cities_count=0`，名称在当前 UTF-16 flat string 读取中出现乱码，需以原始值和游戏界面对拍，不能据此断言词条资源损坏。

这证明了“模板→城市槽位→box→资源初始化”的 UI 消费链，补足了已有存档样本没有的证据。仍未观察到独立 `RegionTemplates` 请求或 locale/string 表加载；模板名来自创建响应中的 `regionTemplateName`。查询采样有独立限流，不能将丢弃计数视为游戏请求失败。

## 60 秒已有城市/区域跟随采样（2026-10-12）

`tmp/ui-log-samples/followup-60s/` 为 60 秒窗口，捕获 116 条命令、520 组查询/返回和 441 条事件；观察器自动卸载，未见解码错误或崩溃。命令中再次出现：

```text
regiondata/2
RegionPlayers/{"ids":[{"city":1026,"user":1911777},{"city":1027,"user":1911777},{"city":1029,"user":1911777}]}
RequestGreatWorksContributors
UpdateVisualizedData
UpdateVisualizedTradeData
```

这把创建区域返回的城市 UID（1026、1027、1029）与已有区域运行时的 `regiondata` / `RegionPlayers` 请求关联起来，说明城市槽位在进入已有区域后会继续作为前端到原生桥的输入。高频 `resourceAmount`、`binAmount`、`cachedUnits/.../binAmount` 查询持续出现，资源消费链得到 60 秒重复证据。当前窗口没有独立 locale/string 表加载信号。

## 改名/切城/保存后 60 秒采样（2026-10-12）

`tmp/ui-log-samples/rename-switch-60s/` 捕获 103 条命令、400 组查询/返回和 63 条事件。切城相关输入明确出现：

```text
showCityRegionData/1029
regiondata/2
RegionPlayers/{"ids":[{"city":1026,"user":1911777},{"city":1027,"user":1911777},{"city":1029,"user":1911777}]}
```

事件包含 `ShowRegionUI`、`HideRegionUI`、`RegionViewItemSelected`、`GameLoaded`、`GameLoadingFinalized` 和摄像机移动通知。资源查询继续出现。该轮没有捕获明文城市名称，也没有独立的 `save`/`load` 命令；城市改名可能走区域数据提交、存档层或未纳入当前桥绑定的接口，不能据此断言改名未保存。

## 保存/重载专项 60 秒（2026-10-12）

用户在观察器运行期间执行了保存城市、退出菜单、重新读档并切换区域城市。`tmp/ui-log-samples/save-reload-60s/` 捕获到两次 `handleEscapeKey`、`optionsMenuActive/true|false`、`deselecttool` 以及重载后的一组 `tickerMod/*` 初始化命令，但没有 `GameLoaded`、`GameLoadingFinalized`、`regiondata` 或独立 `save/load` 命令。有效桥记录的时间跨度约 4.8 秒，之后长时间无新记录。

因此当前证据表明保存/加载主流程可能由更外层的原生菜单/存档服务处理，或在页面重建期间暂时脱离已绑定的 EAWebKit Client 桥。不能把“未捕获到 save/load”解释为“保存没有发生”；下一步应把原生文件/进程级观察与页面桥采样并行，而不是继续增加同一入口的采样时长。
