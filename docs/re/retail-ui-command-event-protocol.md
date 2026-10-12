# 离线发售版 UI 命令与事件协议

2026-10-12。静态协议已由后续实机原生采样补充：命令参数、查询参数和标量返回值均已捕获，反向事件已在第四轮捕获，普通对象 payload 尚未展开。曾导致黑屏的覆盖脚本已撤回，当前使用限时原生观察器；详见 [运行时研究](retail-eawebkit-runtime.md)。

## 结论

内嵌 UI 的默认路径是 `window.Client` 原生方法回调发送命令/查询，以及原生直接推送 UI 事件。HTTP/URL transport 是另一条路径，不能把游戏内操作一概当成 localhost 网络请求。

已确认可供观察的语义接口及参数格式，足以设计控制台的协议适配层。已实现独立采集器输出 JSONL；尚未接入 OpenSCP 应用界面，也未确认零售 HTTP 服务启用。UI 事件只覆盖引擎主动给页面的通知，并非完整模拟事件、所有快捷键或可重放的游戏录像。

## 样本身份与复现

- EXE：`D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCity\SimCity.exe`
- 长度：10,971,552 字节；PE32 x86；首选 image base `0x400000`。
- SHA-256：`2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。
- 从同一安装的 `SimCityData/SimCity_App.package` 提取 scrui：TGI `67771F5C:40464200:7D14BE70`，entry 2065；launcher：`67771F5C:40464200:696447EA`，entry 2211。
- 从 `SimCity_Game.package` 提取 game UI：`67771F5C:40464200:7CCC548C`，entry 984。
- 提取文件与身份记录：`tmp/ui-bridge-retail/`。这些是基包样本；不能证明覆盖包参与后页面最终使用的仍是这些字节。

scrui SHA-256：`c36316891708c6bf89c9530714704446dd20104f9e40420c461c6972257e776a`。
launcher SHA-256：`7520edf4ed0e9ea6ae3a4bb2c9623da7814f47c63013f35ec804e21e80b144f1`。
game UI SHA-256：`683c66fe0d0146b9b630bbe59e1df89848fc05d907f3028147c01ff4f407431b`。

合约验证：

```powershell
node tools/dynamic/verify_ui_bridge_contract.cjs tmp/ui-bridge-retail/scrui-7D14BE70.js
```

已通过。测试从实际提取文件截取有关函数，以 mock 原生/XHR 接收端检查默认分支、命令参数、返回行为、查询批处理、事件开关和序列化。不是对人工重写协议的自测，也不是实机连接测试。测试脚本检查 scrui 哈希，版本变化时必须重新审核。

## 默认传输与调用链

`InitializeGameClient` 将 `window.Client` 保存到 `scrui.gClient`，并缓存 `RequestGameData`、`PostGameCommand`、`RequestGameEvents` 等方法引用。因此初始化完成后只替换 `window.Client` 的方法可能漏掉调用。

请求管理器构造时：

```javascript
this.mAsynchronous = !scrui.IsClientAttached();
this.mUseClientCallback = scrui.IsClientAttached();
this.mUseGameEventQueue = !this.mUseClientCallback;
```

有 Client 时默认同步原生调用、直接事件推送；没有 Client 时默认走 URL 请求与事件队列。运行时仍须记录实际 flag，不能仅凭默认值推断最终模式。

```text
页面操作 → request manager → scrui.Client → 缓存的 window.Client 原生方法
                                        → 原生命令/查询分发器
原生 UI 通知 → ReceiveGameEventFromClient → ReceiveGameEvent → 各 UI handler
URL 队列响应 → ProcessGameEvents ─────────→ ReceiveGameEvent
```

## 真实数据形态

| 类别 | 原生路径 | URL 路径 |
|---|---|---|
| 命令 | `PostGameCommand(commandString, optionalBodyString)` | `POST gamecommand/<commandString>`，`text/plain` body |
| 数据 | 每个 query 单独 `RequestGameData(queryString)`，管理器汇总为数组 | `POST gamedata/batch/`，每行一个 query，末尾也有换行；返回 `{results:[...]}` |
| 事件 | 原生推送到 `ReceiveGameEventFromClient(id,data)` | `GET gameevents/attach` 获得 `{gameEventToken:N}`；`POST gameevents/` body 为十进制 token；返回 `{events:[{eventID,eventData},...]}` |

表中的 URL 是相对路径，由 `scrui.kCurrentHostPath` 拼接。并未证明外部 OpenSCP 可访问它们，WebKit 内部 transport 与真正的 TCP listener 是不同层。

`BuildCommand` 与 `BuildRequestQuery` 本质上是数组 `join("/")`，并不自动进行 URL 编码，也没有统一 JSON 命令信封。例如实际调用点用 `regiondata/<regionID>` 发出命令；离线查询包括 `RegionTemplates`、`LatestBox`、`PlayingRegions`、`UserData`。`camera`、`simticks` 等查询名也有消费线索，但返回结构仍需逐项登记。

其他参数编码：

- UnitRef：`slot,stamp`。
- ResourceKey：`group,instance,type`，不同于常见 TGI 展示顺序。
- Vector：`x,y,z`。
- `RunCheat` 将作弊字符串放在第二参数 body 中；脚本里存在此入口不代表发售版支持所有开发命令。

命令管理器可用第三参数 callback 接收原生返回值，但自身没有显式返回；全局 `scrui.PostGameCommand(a,b)` 只转发两个参数。不能把全局 helper 当成 Promise 或完整的请求/响应 API。即便拿到返回值，也不能默认操作已经在模拟中完成。

查询管理器按输入顺序同步调用原生查询，把 `undefined`/`null` 归一成 `null`，再回调结果数组。控制台应保留 query 与结果的逐项对应。

## 这份 EXE 的原生证据

以下是此哈希对应的静态 VA，不是通用 hook 地址；若用于运行时须计算 `模块基址 + (VA - 0x400000)` 并重新验证字节、调用约定与对象生命周期。

| 静态 VA | 核对内容 |
|---|---|
| `0x899EC0` | command JS callback；在 `0x899F98` 调用 `0x8C73F0`；以 `ret 0x10` 返回 |
| `0x8C73F0` | 命令分发器；拆分字符串并计算小写 FNV1 命令 hash，进入多个分支 |
| `0x89B640` | HTTP command Start；零售反编译显示非主线程入队逻辑 |
| `0x8975D0` | 返回 `/gamecommand/*` 字符串地址 |
| `0x898720` | 队列事件构造，引用 `eventID` 与 `eventData` |
| `0xD1BF68` | 直接事件回传 JavaScript 字符串；push 指令位于 `0x62DB54`、`0x769C9B` |

最后一个字符串原文：

```javascript
(function(event) {
  if (typeof event === 'number') {
    scrui.cUIManager.ReceiveGameEventFromClient(event, null);
  } else if (event.length) {
    var type = event[0], data = event[1];
    scrui.cUIManager.ReceiveGameEventFromClient(type, data);
  }
})
```

由此确认直接推送传入形态可以是 **数字事件 ID** 或 **`[ID, data]`**；队列 HTTP 的 `{events:[...]}` 是另一种封装，不能混用。

实际 UI 常量 `kGameMessageRegionUnitSelected = 246368015`，消费代码读取 `eventData.regionBoxID`。这是很适合首轮验证的城市选中事件。未知事件应保留原始数值 ID 与 payload，不能按名称猜测结构。

## 观察入口与实时日志

优先在以下现有调用链观察，不额外发出查询或命令：

1. `cDataRequestManager.prototype.PostGameCommand`：命令、可选 body、已有 callback 结果/异常。
2. `cDataRequestManager.prototype.RequestGameData`：原始 query 列表及原有回调结果。
3. `cUIManager.prototype.ReceiveGameEvent`：直接推送和队列模式汇合处，观察 `(eventID,eventData)`。

上面是候选观察面，不是已安装探针。需要保持 `this`、参数、返回值、异常和回调调用次数；按序号关联异步结果，避免将两个层级重复计为两条用户操作。序列化必须限深、限长、捕获循环引用；快照不能保留随后被游戏修改的可变对象。使用有上限的缓冲区和丢弃计数，避免逐帧同步落盘拖垮 UI。

建议由 OpenSCP 定义自己的记录结构（不是游戏原有协议）：`sequence/time/channel/phase/name-or-id/rawPayload/requestSequence/droppedCount`。保留游戏原文，解析后的友好名称放在附加字段。

事件队列读取会删除已取事件。不可盗用 UI token 自己轮询；也不需要为观察强制切换队列模式。脚本存在 `SetGameEventListener` 管理器方法，但当前提取的 `scrui.Client` 没有对应定义，不应仅凭方法名调用。

日志不是现成的全量文本流：

- `scrui.DebugPrint` 路径受 `scrui.DEBUG` 等条件影响；不能承诺 `console.log` 自动写到文件。
- EXE 保留 Spark HTTP、8088、`noServer` 等字符串；开发版有服务器启动及日志关闭逻辑，但当前零售的监听、路由和日志目录仍未实测。
- 模拟层 `LogAction`/`LogMapRule` 写二进制 delta；要做资源/规则调试，后续需独立解码和关联。

Frida 本身并非已证明不可用。此前 D3D9 hook 的崩溃不排除地址、对象、调用约定、线程或重入问题；当前证据不能指定唯一根因。既有 `tools/dynamic/safe_attach.py` 的 VA 与 `base.add(va)` 用法也应在复用前审计，不能凭文件名判定安全。

## OpenSCP 控制台演进与下一次实机验收

第一阶段只观察：给 UI 观察补丁独立版本标记，验证它确实加载；记录 Client 存在性、请求管理器模式、游戏 EXE 指纹。建立从游戏到 OpenSCP 的导出通道仍是独立缺口：可选已验证可用的 HTTP 服务，或经过验证的 UI 导出桥。当前没有可直接调用的外部 RPC 保证。

最小实机验收步骤：

1. 在离线创建区域页切换模板，捕获原有 `RegionTemplates` 查询及响应，记录目标模板 `map.cities` 与 `boxes`。
2. 进入测试区域选中不同城市，确认 `246368015` 的实际 payload 和 `regionBoxID`，与 UI 槽位对照。
3. 捕获页面自然发出的 `regiondata/<ID>` 等命令，观察后续事件；不假定每条命令都有唯一 ack。
4. 连续操作、空闲和切换页面，验证记录顺序、缓冲上限、丢弃计数以及 UI 行为；保留未观察版本作对照。

第二阶段开放已确认无修改副作用的查询白名单，按游戏版本登记参数与结果结构。第三阶段才加入用户明确执行的命令、参数校验和前后状态对照；不提供盲目整段回放。UI 命令流不足以保证确定性重演。

地图调试先用它证明“覆盖是否加载、哪个消费者实际读了什么”：词条对应实际 locale/key，槽位对应模板响应/运行时 boxes，资源对应初始化前后字段与存档。摄像机再用 `camera` 查询和已有相机命令辅助观察；它们不是已证明可解除 XY 边界的接口，多层约束分析仍见 [map-consumers-ui-bridge-camera.md](map-consumers-ui-bridge-camera.md)。


## 2026-10-12 实机回归与路线调整

首轮 zzz 包启动正常但零记录；只读内存比对找到完整原版 scrui。第二轮 000 包的观察器/build 标记进入脚本内存，但游戏黑屏、有音乐且未崩溃，依然零记录。全部实验包和接收器已撤回，用户确认主菜单恢复。当前仅证明这组改动与回归关联，不能指定为 JS 语法、传输、资源格式或重入的某一种原因。

观察工具已禁止安装 observe 与旧 manifest；默认 prepare 改为不追加任何字节的 baseline 对照产物。隔离测试通过不代表旧 EAWebKit 实机兼容。下一步转向 [EAWebKit 原生运行时](retail-eawebkit-runtime.md)，先核对版本、引擎、错误回调及 Inspector，不重装失败观察包。


续轮只读实机验证已取得：Launcher3D 和 WebKitPanel 当前页面均注册 PostGameCommand / RequestGameData / RequestGameEvents，原生处理入口与静态分析一致。DebugLog 宿主为空处理，Inspector 工厂返回空。见 [运行时研究补充](retail-eawebkit-runtime.md)。这证明桥在当前进程中实际绑定，不表示已抓到某次操作或证明事件队列正在使用。


2026-10-12 原生观察首轮已取得真实方法调用元数据：PostGameCommand 51 条、RequestGameData 9,344 条，全部在线程 51960，参数数目均为 1。共享额度丢弃 53,828 次，不能当完整流；参数内容、结果、事件 payload 尚未捕获。实验自动撤回并核对入口恢复，详见 [原生观察首轮](retail-eawebkit-runtime.md)。
# 2026-10-12 实机更新：参数字符串已捕获

第二轮原生观察已取得真实 `PostGameCommand` / `RequestGameData` 参数，见
[EAWebKit 第二轮证据](retail-eawebkit-runtime.md#第二轮真实命令和查询字符串2026-10-12)。
60 秒窗口中 86 条命令全部保留，查询 65889 次中采样 2400 次。
实测形式包含无参数名称、斜杠后缀、JSON 后缀；如 `deselecttool`、
`toggleLayer/51886653/false`、`RegionPlayers/{...}`。
原始字符串必须保留，不能将所有后缀按 `/` 拆成固定参数。
这证明 UI→原生的命令/查询输入可观测；查询返回、反向事件和全部原生输入仍未覆盖。


## 第三轮：查询返回与反向调用候选

`tmp/ui-native-v3` 保存 1200 组 query/result，使用 callSequence 逐项关联，未匹配返回为 0。结果包含 int32、double、boolean、null、flat string；对象/数组标记 unsupported，不执行 getter。13 条命令保留，43802 次查询采样丢弃；实际游戏查询照常运行。用户确认正常、没有明显卡顿。

直接事件代码的两处已核对调用点为 EXE RVA `0x22DC89`、`0x369D7A`，通过宿主虚表 +0x50 调用。当前宿主 `0x36DDBCD0` 虚表 `0xD5F3C8` 对应 EXE RVA `0x62F500`。该函数 thiscall、ret12，三个参数为函数包装引用、可空结果包装引用、可空参数包装引用；经 `0x1C4570` 取底层 JavascriptValue，再调用函数值虚表 +0x6C。事件路径传入一个数字或数组值，而不是 eventID/eventData 两个独立 ABI 参数。

这是通用原生→JS 函数调用入口，不能将其所有调用都命名为游戏事件。下一步需要核对事件函数身份、数组布局与生命周期，再观察事件 ID 和有界 payload；目前未挂接此入口。

## 第四轮：直接推送事件已证实

生产观察器仅接收 EXE RVA 0x22DC8B / 0x369D7C 两个已验证派发调用来源。
实机第四轮捕获 1258 条事件、19 个 ID，均有 UI 常量名称对应；包含摄像机移动开始/停止通知各 26 条。
直接数字 ID 与 [ID, data] 两种形态均得到验证；1031 条对象 payload 暂不展开。
详情及边界见 [第四轮运行时证据](retail-eawebkit-runtime.md)。没有读取事件队列或主动调用页面函数。

## 创建区域专项证据（2026-10-12）

详见 [EAWebKit 运行时补充](retail-eawebkit-runtime.md)。稳定创建区域页面的 v6 样本首次完整展开 `CreateRegion`/`GetRegionDetails` 的 `map.cities`、`greatWorks`、`boxes` 和 `resources`，确认区域地图槽位由响应中的 `map.cities[].uid/x/y/z` 提供，box 的模板与资源由各 box 对象提供。

### 60 秒已有区域跟随采样

已有区域窗口再次捕获 `regiondata/2` 和包含城市 UID 1026、1027、1029 的 `RegionPlayers` 请求，证明创建区域响应中的城市槽位会进入存档运行时链路。资源查询和事件流在 60 秒窗口内持续出现；该场景适合后续验证改名、切城和保存重载。

### 改名/切城专项 60 秒证据

已有存档切城后，`showCityRegionData/1029`、`regiondata/2` 和 `RegionPlayers` 城市 UID 集合形成稳定输入链。未见明文城市名称或独立保存命令，后续需对页面显示文本、保存时序和区域数据提交做定向采样。

### 保存/重载链路的当前缺口

60 秒窗口中用户确实执行了保存、退出菜单、重载和切城，但桥上只出现 Escape/选项菜单和重载后的 ticker 初始化，没有 GameLoaded 或 save/load 文本。保存服务可能位于原生菜单或页面重建之外；后续应把观察范围扩展到存档文件时间线、进程窗口消息或更高层原生状态，而不是将桥记录缺失当成操作未发生。
