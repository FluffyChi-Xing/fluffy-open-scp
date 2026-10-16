# 面向 Mod / Assets 开发的日志工作台：实机证据与实现边界

日期：2026-10-17。package 修复与加载研究已提交为 `4c99e1a`。本轮继续调查日志，并新增可运行的引擎启动采集原型；尚未把原型接入产品界面。

## 实测进展

同一个实际离线安装 EXE，SHA-256 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。两次挂起启动观察均为 35 秒，没有投放 Mod、没有修改过滤器、没有手动调用游戏 API、没有进入或保存城市。

| 运行 | PID | 捕获层 | 结果 |
|---|---:|---|---|
| baseline | 14760 | 格式化入口、文件/控制台 reporter、控制台 Output、GetServer | 253 条：119 file reporter、120 format、10 console、4 server identity |
| server | 62100 | 上述入口 + Server::Trace | 384 条，额外获得 131 条完整引擎文本 |

两次 errors=0、dropped=0，均正常结束采集并 detach。第二次 Server::Trace 文本包括 Config 119 条、UpdateManager 12 条；文件 reporter 仅有 Config 119 条，控制台 reporter 本次未命中。格式入口的 120 条是模板，不应叠加成 120 条独立完整日志。

第二次确认 Server::Trace 能看到未到达这两个所观察 reporter 的 UpdateManager 文本，包括包版本 ID；不能由此断言系统所有其他 reporter 都没有输出。也不能认为通过这一个入口能恢复 IsTracing 之前已被过滤、编译移除或根本没生成的日志。

证据：[摘要与脱离敏感配置的样例](evidence/engine-logs-startup-20261017.json)。完整原始日志在 `tmp/engine-logs-{baseline,server}-20261017`，摘要记录 trace 哈希。工具：[capture.py](../../tools/dynamic/engine-logs/capture.py)、[agent](../../tools/dynamic/engine-logs/engine_agent.js)。

## 已核对的零售入口和数据

以下为 VA（基址 `0x400000`），不沿用 dev 地址：

| 入口 | VA | 本轮观察内容 |
|---|---|---|
| TraceHelper::TraceFormatted | 0x8E5630 | cdecl 栈参数 helper、format；不猜变参格式化 |
| Server::Trace | 0x917380 | thiscall，helper、已格式化 text；ret 8 |
| LogReporterFile::Report | 0x916370 | thiscall，LogRecord；在文件写出前观察 |
| cLogReporterDebugConsole::Report | 0x46BE30 | thiscall，LogRecord；本窗口零命中 |
| cCheatManager::Output | 0x46B9E0 | thiscall，文本；转发 console 注册表 |
| GetServer | 0x915D90 | server 实例身份；本次 vtable 0xD2C258 |

TraceHelper 构造函数 `0x8E5BD0` 与 IsTracing `0x8E52C0` 确认：+0x10 level、+0x14 group、+0x18 file、+0x1C line、+0x20 function、+0x24 tracer。解码前确认 helper vtable `0xD29CE8`。LogRecord 本次 vtable `0xD2C20C`，+8 record number、+12 helper、+16 text、+20 level name，与 dev 结构和运行文本相符。

零售日志 location 多为空；group/level 有意义，不应把日志行的数字 ID 都解释成 TGI。已观察的 `0xa9c9479d` 是 UpdateManager 文本中的版本对象标识，若关联 package metadata，必须提供对应索引依据。

## 手动触发应拆成四种操作

| 用户操作 | 实现与价值 | 当前证据 / 验收缺口 |
|---|---|---|
| 标记“开始放置资产” | OpenSCP 本地写 marker，与会话时间线关联；不影响游戏 | 可直接实现，但必须标为 host.marker，不能伪装成引擎输出 |
| Mod 自己输出一条诊断 | 页面 console、或已确认解析器的 ArgScript output/trace | 原版 Config 的 trace 已实测；自制 UI Mod console 需独立最小验收 |
| 获取选中资产/单位状态快照 | 只使用已经核验的查询白名单，记录输入、返回、耗时、截断 | 现有 bridge 观察不等于拥有安全的主动查询执行通道；需专门验证调度线程及绑定寿命 |
| 打开某个引擎日志组 | 定向修改 group/reporter level，保存原状态并恢复 | 零售有 SetOutputLevel 调用链，但本轮未主动调用；不能上线一个“全部开启”按钮 |

**ArgScript 已有实际打印机制。** dev `cParser::Init.c:436–451` 将 `output` 和 `trace` 注册到同一 cOutputCommand；Execute 要求一个主参数，调用 `EA::ArgScript::Output(parser, "%s\n", arg)`，写 parser 的 OutStream，并非因为命令名为 trace 就必经 EA::Trace。零售注册调用点 `0x951DAB` 与 `0x951DF0` 分别使用 output/trace 字符串，并构造同一 vtable `0xD3DDDC`。

真实安装 `SimCityData/Config/ConfigManager.txt:875` 的 `trace "=== Instancing can be enabled"`，以及 888–893 行包含 `${gpuLevel}`、`${cardName}` 等插值的语句，与捕获到的控制台文本对应。这证明配置解析路径的变量插值输出可被捕获；没有修改 Config 文件来制造此结果。

`output "[OpenSCP] mod=example stage=parse"` 可作为这个解析器路径的诊断语法示意。对 gameplay ArgScript，必须先确认它的 parser 是否保留命令、OutStream 是否存在、脚本什么时候解析。配置/编译时输出不是规则每 tick 执行的输出，也不自动拥有运行中 unit/bin 局部变量。

**UI DebugPrint 不是通用 console 的别名。** 提取的 retail scrui 同时在 DebugPrint 包装层和 Client.DebugPrint 包装层检查 `scrui.DEBUG`；后者还检查 client 与方法是否存在。当前 native observer 捕获的是 EAWebKit addMessageToConsole。不能把“看见 DebugPrint 函数”写成“Mod 调用必然进入现有 console 通道”，也不应全局翻转 DEBUG 来试图打开所有开发功能。

## 模拟观察需要独立事件层

之前文档把 LogGlobalRule、StateValidationFailed 放进文本日志 reporter 链是不准确的：

- dev `GB/cEcoGame/LogGlobalRule.c` 写 `mDeltaStream`，包含 sim tick 差、rule state index、apply count；是二进制动作记录。
- `LogGlobalRuleAttach.c` 转到 LogAction；也不是 EA::Trace 文本。
- `StateValidationFailed.c` 只是把 reason 写进 `mStateValidation`，自身不输出日志。
- 已有 [delta 调查](retail-delta-stream-analysis.md) 有捕获与解码边界，不能把存档 payload、实时动作流和 UI 请求当同一种日志。

面向 gameplay 的目标应是定向捕获 rule/unit/bin 事件，再关联游戏脚本资源和符号：`tick → unitRef → ruleId/stateIndex → 输入 bin → 输出 bin → action`。哪些字段可安全读、何时有效需要逐入口实证。首先限定某个 unit 或规则、短时间窗口；不要每 tick 扫描所有单位，更不要把“时间接近”显示成确定调用因果。

## 产品结构建议

采用“采集器 → 统一事件 → 会话存储/索引 → 诊断视图”，采集器只提供有证据的字段：

1. UI adapter：请求/返回关联、页面 console、事件。保留 requestName、callSequence、page/binding 身份。
2. Engine text adapter：优先 Server::Trace，reporter 作为输出去向证据；console 单列。
3. Resource adapter：package 打开/注册/TGI 选库，复用本轮 package loader 证据。
4. Simulation adapter：只接已经确认的规则/单位/动作字段；未解码 payload 原样保留并标 unknown。

统一事件最小字段建议：`schemaVersion, sessionId, source, kind, sequence, wallTime, monotonicTime, pid, threadId, group, levelRaw, levelName, message, fields, correlation, provenance, quality`。

- `provenance` 保存 EXE/模块指纹、入口、解码器版本、原始记录定位；从文本提取的字段标 inferred，不与原生参数混同。
- `quality` 分开记 capture dropped、intentionally filtered、decode errors、truncated；“暂停滚动”不暂停落盘。
- UI 请求结果用 callSequence 关联；engine recordNumber 加 session/server 身份；跨通道只有明确依据才建立因果边，时间相关另标。
- 同一条 engine 文本经过 Server 与 reporter 会重复出现，归为同一事件的不同阶段或保留关联，不能直接累计为两次游戏动作。
- 原始 JSONL 保留；派生索引用于筛选，不让界面最近 1000 条决定历史是否存在。

现有 LogStream 只有文本包含搜索、sequence 合并、最近 1000 条显示。开发视图应增加结构化条件，如 `source=engine group=UpdateManager level>=50`、`tgi=...`、`unitRef=...`、`requestName=...`，以及包含/排除、固定观察目标、前后文、请求耗时和字段值差异。

**字段高亮的语义应可解释：** 来源、severity、TGI、rule、unit、差异值分别着色；点击 TGI 打开资源、点击 rule 打开已映射的脚本位置、点击 unit 查看其观察时间线。未知 hash 只高亮为 hash，不能擅自当资源链接。文本以文本节点渲染，不把游戏字符串当 HTML。所有新增 UI 文案维护 zh-CN/en-US。

## 按开发收益安排的实施与验收

| 阶段 | 交付 | 必须通过的真实开发场景 |
|---|---|---|
| 1 | 接入引擎文本采集、统一 schema、组/等级筛选、会话 marker、原始导出 | 新启动能同时看到 Config/UpdateManager 和 UI 来源；空白说明原因而不是“连接成功”；捕获质量可见 |
| 2 | Mod 输出约定、资源关联、字段高亮/上下文、before/after 比较 | 改一条 TGI 后证明选中了哪个包；触发 UI 动作能看到请求、结果和关联资产，不需要人工翻巨量 JSON |
| 3 | 白名单状态快照、触发条件与窗口捕获 | 操作前后采集同一单位字段差异；错误触发时保留前置环形缓冲与后续窗口，不篡改模拟 |
| 4 | 规则/单位/bin 专用观察及脚本符号映射 | 指定一条 gameplay 规则，解释为何没有触发或写出了什么；具有 tick、规则身份和有效状态证据 |

阶段 1、2 的目标是工具可用性，不要求先解决全部模拟逆向。阶段 3、4 需要继续研究线程、对象寿命和规则流；当前证据不支持“任意脚本表达式执行器”或“完整模拟追踪”。

本轮已交付引擎文本捕获原型和真实记录，尚未交付上述产品 UI、主动命令或 gameplay 调试器。此前存在的完整 scrui 观察覆盖包仍停用，不将其作为接入前提。
