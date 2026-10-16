# 诊断会话工作台：第一阶段验收

入口：OpenSCP → 运行时日志 → 诊断会话。现有实时 UI 捕获仍独立工作；新增面板读取本地完整 `trace.jsonl`，不会只分析实时页的最后 80 / 1000 条记录。

## 使用

1. 用已有 package-loader、ui-observer 或 engine-logs 探针生成采集文件。选择“导入 trace.jsonl”；每次导入替换当前会话，避免不同运行的相同 sequence 被误配。
2. 资源页输入 `00B1B104:00000000:4483E856`，同时显示此 TGI 的访问记录及相同路径快照的 open/register。检查 result、register、priority；`probe` 只是存在性测试，`materialize` 才请求 Record 创建。注册记录不是 TGI 覆盖成功证明，Record 创建也不代表之后资产语义验证通过。
3. UI 页按请求名或参数筛选，展开“配对记录”。配对使用 binding、page、name、requestName 和 callSequence；缺失和重复歧义均不能伪装配对成功。同完整参数的连续成功返回提供变化字段路径。事件没有此关联键，不根据时间相邻宣称由命令引发。
4. 单位观察页查看 UnitRuleUIEvent 的 unit slot/stamp、内部事件 ID 和位置。这里只能验证 UI 接收到单位事件，不能证明 gameplay 规则执行。
5. 引擎页按 group、level、message、sink 筛选。保留 trace/report/format 等原始阶段，不合并为业务事件计数。关键词通过 Vue 文本节点高亮，日志内容不作为 HTML 执行。
6. 手动标记记录当前主机时间以及导入记录尾部 sequence，导出 JSONL 可再次读取。它是分析批注，不是游戏内日志触发；对历史采集添加标记不会制造过去的游戏时间。关闭会话前需导出。

每页呈现 100 条，但检索与配对使用完整已导入会话。文件限制 256 MiB，解析保留无效行号。导出仅包含成功解析的记录及新增标记；原文件不会修改，无效行不写进导出文件。采集丢弃量、采样策略及探针异常仍需核对同目录 status.json；当前面板不把未知质量标为零。

## 组件边界

- `diagnostics.ts`：探针数据解码、证据分类、配对、返回差异与筛选；纯数据逻辑。
- `useDiagnosticSession.ts`：本地导入、标记、导出与错误状态。
- `DiagnosticWorkbench.vue`：筛选、分页和会话操作；独立于旧实时日志流。
- `EvidenceDetail.vue`：原始证据与配对展开。
- `EvidenceValue.vue`：安全的字段值文本高亮。

## 实际验收

2026-10-17，以现有完整采集文件重新运行产品解析器，无重新启动或修改游戏：

| 会话 | 成功导入 | 无效行 | 验证结果 |
|---|---:|---:|---|
| ui-native-exhaustive-build-20261014 | 142566 | 0 | 70363 对调用/返回，1324 条 UnitRuleUIEvent |
| package-runtime-data-20261017 | 207 | 0 | 保留全部资源阶段，TGI 可查到 probe/findRecord/materialize |
| engine-logs-server-20261017 | 384 | 0 | 保留所有引擎阶段，未合并重复管线文本 |

定向测试覆盖资源证据、真实 UI 配对、binding 隔离、会话隔离、歧义拒绝、同参数比较、无效行、标记安全输出以及中英文实时切换。`vue-tsc -b --noEmit` 通过。既有 observer 测试与新增诊断测试共 12 项通过。

全局 i18n 审计的消息编译与动态前缀检查通过，但静态扫描仍失败：它将旧页面的局部 `useI18n` key（tickHint/chartSource/shareHint/requests/source）当作全局 key，并把旧有字符串拼接 `runtimeObserver.` 当成完整 key。没有用虚构全局翻译掩盖此问题。

## 尚未闭合的运行时部分

这不是“三条运行时闭环全部完成”。资源来源复盘与 UI 请求/返回复盘已落地；UI 操作到事件的精确因果尚无通用关联键。第三条目前只有单位事件观察，未获得规则输入/输出和 tick。

后续应优先验证 `GB/cEcoGame/LogGlobalRule` 写入 mDeltaStream 的 stateIndex/applyCount 与规则定义映射，以及同一采集会话中的 tick/单位身份。不要把它接到 EA::Trace 文本链，也不要把 StateValidationFailed 的状态赋值当成日志输出。当前没有新增 NativeFunction、任意 gameplay trace 执行或堆叠采集 hook。
