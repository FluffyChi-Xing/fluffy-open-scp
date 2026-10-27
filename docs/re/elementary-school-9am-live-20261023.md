# 小学实时验证：水电72 tick周期与整点教育调度

2026-10-23，承接[离线规则分析](elementary-school-rule-schedule-20261023.md)。用户从游戏显示约09:00解除暂停，本轮只用进程外 `ReadProcessMemory`，没有注入DLL、安装hook或调用游戏函数。

## 已观察到的对应关系

两条水电规则的实时 SimInfo 都是72。两次计数器从1重置为72，均在同一tick内观察到对应库存减少90；两次重置相隔恰好72 tick。随后水电分别补回270。

| 运行时tick | 读取到的日周期时刻 | 水/电计数器 | 后续同tick库存变化 |
|---|---|---|---|
| 46380 | 12:00:13 | 两者1→72 | 12:00:29，两者270→180 |
| 46452 | 13:12:07 | 两者1→72 | 13:12:22，两者270→180 |

第一次补给在46381观察到，第二次在46453观察到。上述秒数是轮询时读到的插值时钟，不是精确函数执行时间。两个计数器重置和库存变化出现在相邻的0.5秒采样中，不能将0.5秒差直接视为引擎固定执行延迟。

这组证据比只看库存端点更强：静态资源操作、Property专用化、运行实例配置、计数器周期和库存差值全部吻合。不过它仍是外部快照中的时间关联，没有函数级 before/after 效果日志，不能声称排除了所有并发写入。

## 当前实例身份与脚本参数

- PID：58376；game：`0x3c863e20`；小学 UnitEntry：`C264A268`；slot：802；unit：`0x436bf708`。
- EXE SHA-256：`2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。
- 本次重新读取资源ID→index映射，没有借用旧进程索引。
- 电力：stateIndex=9，ruleID=`D51CB94C`，当前ruleIndex=30695，SimInfo.rate=72，pool offset=1607520。基础规则的rate=30，当前实例确实使用了专用化结果。
- 用水：stateIndex=10，ruleID=`D54F8B18`，ruleIndex=7069，SimInfo.rate=72，pool offset=427192。
- 教育整点任务：stateIndex=53，ruleID=`23E737BD`，ruleIndex=5150，triggerPeriod=`D5A08F93`，triggerTime=0。
- 入校 sink：本次初始映射stateIndex=251，ruleID=`B52C889D`。它位于核心调度数组之外；不能套用水电计数器解释。

实时数组区分如下：

| Unit偏移 | 用途/本轮处理 |
|---|---|
| +1BC | 核心规则 SimStates，28字节/条，本小学55条 |
| +1CC | 规则索引数组，包括按需追加的事件/sink规则 |
| +1DC | 12字节/条的规则执行状态；不是28字节的调度计数器 |
| +210 | 资源bin，12字节/条 |

SimState按 `<5ifi>` 读取：simCounter、3个timeIndices、triggerTimeIndex、nextTriggerTime、triggerCount。开发版带符号布局及实时计数递减/重置支持该解释。核心规则通过stateIndex对齐；事件追加不能被误判为原建筑身份改变。

## 教育规则与消费规则使用不同节奏

在12:00（tick46380）、13:00（tick46440）、14:00（tick46500），`23E737BD` 的triggerCount分别9→10、10→11、11→12，nextTriggerTime每次+1个Hour周期。13:00没有水电计数器重置；水电随后在13:12才再次重置并下降。

这验证了两条独立时间线：

```text
Hour周期：12:00 ────────── 13:00 ────────── 14:00
水电周期：12:00 ───────────── 13:12 ───────────── 14:24（下一次预测）
```

调度triggerCount增加只证明周期门槛被推进，不保证后续资源条件通过。学校满员时，教育任务仍可能到点被检查。

主要窗口开始时学生875、席位75；之后学生达到950、席位0，Student、TotalStudents、StudentStudyingE2同步增加，逐帧Student+SpotsForKids始终950。WorkerW1/TotalWorkers均为5，W2/W3为0；WorkersNeeded为5，JobsW1/W2/W3为68/13/0。ExpensesFromMaintenance始终300；Sewage/Garbage采样值始终0。维护费资源稳定不能证明市政账户没有扣款，污水为0也不能排除轮询间发生生产和排出。

## 采集覆盖与中断

最初一段从tick46198开始，读取的插值时钟约08:58:23，与用户报告的09:00并非同帧HUD对照。在tick46200附近，规则事件数组扩展，过严的“全部规则数组不能变化”校验停止了探针。第二段从46250开始，同样在46260整点附近停止。

两次停止都是采集器自己的验证错误 `rule_binding_changed`，不是游戏崩溃。原始状态文件保留。修正后允许事件数组增长/重新分配，每帧重新取地址，只要求核心规则索引保持一致；若核心规则变更或两数组长度不一致，仍停止读取。

修正后的主要窗口从tick46334（11:14:08）开始，到46508（14:08:34），实际推进174 tick，共1081帧快照。原目标180 tick，末尾tick长时间停止推进，在已完成两轮消费观测后由外部Ctrl+C结束采集器；命令退出码1，原始status没有写入finishedUtc。保留原始status不补写，另记外部终止说明，不伪装为自然完成。游戏进程58376随后仍存在且Responding=true。

主要窗口记录0个身份不稳定样本、0个跨tick样本、3次正常事件数组变更；停止前status没有读取错误。相同tick并不使多个RPM读取具有原子性。因此本次**没有连续覆盖09:00起的全部事件**，不得把三段拼接成无缺口的时间线。完整结果、终止说明、原始文件摘要及两段早期状态见 [JSON证据](evidence/elementary-school-9am-live-20261023.json)。

## 对时钟与诊断界面的意义

本次直接读取了period IDs、inverseLengths、phases、times、cycles，确认Day=1440 tick、Hour=60 tick，初相位约07:00。实时periodTime还包含一个tick内的插值，故同tick可以读到不同秒数。外部墙钟、整数simTick、插值日周期时间、HUD显示时间应该作为四个字段保存。

OpenSCP可以据此增加：

1. 规则实例检查器：同时显示基础rate30、Property覆盖72、运行实例rate72及当前倒计数。
2. 资源时间线：显示消耗与补给两条事件，避免库存首尾相同就显示“未消费”。
3. 调度分类：整点trigger与周期counter、sink到达事件分别展示。
4. 采集完整性：正常事件数组扩展不结束观察，核心绑定变化和采样缺口明确标注。

## 文件与复现

- `probes/runtime/school_timed_watch.py`：有墙钟/tick双重上限的进程外只读探针。
- `probes/runtime/analyze_school_timed_watch.py`：离线汇总和计数器/库存时间关联。
- `probes/runtime/test_school_timed_watch.py`：3项回归测试，覆盖正常追加、核心规则变化及数组过渡状态。
- `tmp/school-9am-watch-20261023/`、`tmp/school-9am-counters-20261023/`：两段提前停止的原始记录。
- `tmp/school-9am-counters-r2-20261023/`：修正后的主要采集。

后续优先观测放学前后：倒计数是否继续推进、控制状态何时阻止消费，以及学生离校/教育输出链。维护费需同时读取财政资源或扣费规则，污水需先定位生产规则的入口和频率；仅增加采样时长不能自动解决因果归属。

本轮后新增了输出目录下 `stop.request` 的正常停止通道，并补获KeyboardInterrupt，后续应优先用停止文件收尾，以保存最终status。该收尾改动未再对游戏启动额外采集。
