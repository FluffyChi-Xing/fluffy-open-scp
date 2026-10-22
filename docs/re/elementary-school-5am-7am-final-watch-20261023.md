# 小学 05:00–07:00 最终运行时观测（2026-10-23）

这是今天最后一轮实时采集。采集器使用 `probes/runtime/school_timed_watch.py`，只通过进程外 `ReadProcessMemory` 读取，没有注入、hook 或调用游戏函数。游戏 PID 为 `58376`，可执行文件 SHA-256 为 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。

探针在约 05:20:43 启动，目标子窗口覆盖到 07:00:38，共 96 个稳定样本、tick `47420–47520`。**05:00–05:20 未采到**，不能通过截取补回。由于等待和停止参数设置不当，实际记录至 08:20:49（tick 47600），共136个样本；下表只分析目标子窗口，后文明确标出附加观测。全部样本通过身份和向量稳定性校验，没有读取错误，探针以tick_target_reached正常结束。之后游戏进程由用户手动退出；这次退出不是崩溃。此前 `simcity-frida-crash-20261023.md` 记录的是更早的 PID 36688 并发注入事故，不属于本轮 PID 58376。

所有时间来自模拟周期字段，未经 HUD 同帧校准。原始 status 的 `userClockAnchor` 为探针硬编码的旧 09:00 标签，本轮不将其作为时间证据。以下时刻为首次看到新值的采样时刻，不是精确事件时刻；06:00变化的时间夹在05:59:56与06:01:12两个样本之间。

## 关键变化

| 字段 | 起始值 | 结束值 | 观测到的变化 |
|---|---:|---:|---|
| Student | 0 | 0 | 05:20–07:00 未捕获学生入校 |
| StudentStudyingE2 | 0 | 0 | 与 Student 一致 |
| SpotsForKids | 950 | 950 | `Student + SpotsForKids = 950` 保持不变 |
| TotalStudents | 950 | 0 | 06:01:12 从 950 重置为 0 |
| WorkerW1 | 5 | 5 | 无变化 |
| TotalWorkers | 5 | 6 | 06:30:40 增加 1 |
| WorkersNeeded | 5 | 5 | 无变化 |
| ExpensesFromMaintenance | 300 | 300 | 未观察到变化 |
| Garbage | 0 | 0 | 06:01:12 增至 100，06:11:16 回到 0 |
| Sewage | 0 | 0 | 未观察到变化 |
| Power | 270 | 270 | 06:01:12 消耗 90，随后恢复 |
| Water | 270 | 270 | 06:01:12 消耗 90，随后恢复 |

`TotalStudents` 是累计或日程统计字段，不能直接当作当前在校人数；当前在校人数应以 `Student` 和 `StudentStudyingE2` 等状态字段解释。窗口结束时虽然已到 07:00:38，但快照仍未捕获 `Student` 增长，因此不能推断当天上学事件未发生，只能说明它不在本窗口的采样结果中。

## 规则时间关联

- `0xD51CB94C`（电力）和 `0xD54F8B18`（用水）实例 `simRate=72`，跨06:00两侧样本的计数器由1变71，且库存各 `270→180`，随后分别恢复到270；中间归零/重置到72的瞬间没有采到，不能报告为06:01发生重置。
- `0x23E737BD`（教育整点规则）在 06:01:12 将 `triggerCount` 从 27 推进到 28，在 07:00:38 从 28 推进到 29。
- 垃圾、`TotalStudents` 和水电在同一采样点变化，属于外部快照的时间相关性；当前证据不能把这些变化归因到某一条 ER2 rule 的函数级 effect。
- `TotalWorkers` 在 06:30:40 增加 1，晚于 06:00 批量变化，提示劳工填充可能是独立的异步过程，但仍需要更细粒度的人口/工人实体证据。

## 全字段离线分析补充

原先的固定资源过滤列表遗漏了额外劳工和暂停状态。新脚本 `probes/runtime/analyze_school_dawn.py` 对全部bin做差分，保留前后样本时间边界，同时扫描已导出的安装版ERZ；全过程不访问游戏进程。

| 首次观测时间 | ExtraWorkerW1 | ExtraWorkerW1Paused | TotalWorkers | 说明 |
|---|---|---|---|---|
| 06:30:40 | 0→1 | 0 | 5→6 | WorkerW1仍为5，增加的是额外状态 |
| 06:58:36 | 1→0 | 0→1 | 6 | 额外工人转为Paused字段 |
| 07:14:42（窗口外） | 0 | 1→0 | 6→5 | 额外状态消失，不能单凭此确定人物离开路径 |

在136个已保存样本中检查 `TotalWorkers = WorkerW1 + WorkerW2 + WorkerW3 + ExtraWorkerW1 + ExtraWorkerW1Paused`，全部符合。此式只描述本次样本，不是所有建筑与所有劳工状态的通用公式。

安装版静态候选规则如下：

- `938552F2`：本地ExtraWorkerW1与TotalWorkers各+1，同时通过fromBinOps清理三项请求资源。其中7DCE3191从5→0与本次样本一致；未证明本次调用就是此规则。
- `6C855B1F`、`D2460875`：ExtraWorkerW1 -1、ExtraWorkerW1Paused +1；小学祖先bundle `30645625` 的sink规则数组 `0A2725CF` 引用它们。还存在其他相同核心操作的规则，不能唯一归因。
- `B667C3C7`：ExtraWorkerW1Paused与TotalWorkers各-1；匹配附加窗口的变化形态，但完整绑定与调用链待查。

学生统计也出现跨日证据：入校规则B52C889D累加TotalStudents；此前放学后该值保持950；本次跨06:00清零，与C193A5BB的day=0.25触发和fromBinOps读取自身数量作为input吻合。因此应解释为06:00重置的累计统计，不是建筑容量。ExamsPassedE2也在同一采样区间950→0，其具体执行规则仍待核对。附加窗口07:18:44首次见Student、StudentStudyingE2、TotalStudents各0→1及SpotsForKids 950→949，与入校资源签名吻合。

垃圾：除Garbage 0→100，还同时看到GarbageDisplayCount 0→1及未命名资源2F207D6B 0→1，清空时三者一起回0。TotalGarbageToCreate的amount/capacity是100/100，RecyclingLevel为0/10。静态bundle D7C62954将属性0C216562（小学值10）通过资源初始化表及倍率10关联EEE64A65（TotalGarbageToCreate）；与运行值100一致。规则74A74AB3从RecyclingLevel读取剩余容量到Garbage输出，提供“100生产量如何分配”的候选链，但其触发链、applyCount以及零售初始化倍率语义尚未逐函数闭合，不能把基础output=999999作为实际垃圾产量。

## OpenSCP与后续离线任务

1. 运行时资源检查器必须覆盖ExtraWorker和Paused等状态，并显示amount/capacity、前后样本区间；不能只靠固定白名单或把所有Total字段当容量。
2. 规则视图分别呈现每日重置、间隔检查、sink到达、状态转换与资源条件；显示Property专用化和fromBinOps替换后的候选值。
3. 优先离线追踪劳工sink/后续规则链，解释额外工人到达、Paused转换及退出；随后闭合垃圾06:00入口、实际applyCount及显示/清运链。
4. 维护费300和Sewage=0仍不足以解释财务结算和污水排放，下一步检查全局账目规则与排出路径。当前不继续采集，也不根据尚未闭合的语义写回游戏资产。

## 证据边界

快照间隔约 0.5 秒且不是原子事务，可能遗漏短暂状态。维护费和污水在本窗口保持可见值，不等于不存在瞬时结算。今天的实时抓取到此结束，后续只进行离线分析。

原始数据与分析文件：

- `tmp/school-5am-7am-final-watch-20261023/window-05-07/snapshots.jsonl`
- `tmp/school-5am-7am-final-watch-20261023/analysis-05-07.json`
- `docs/re/evidence/elementary-school-5am-7am-final-watch-20261023.json`
- `docs/re/evidence/elementary-school-dawn-offline-correlation-20261023.json`：全字段变化、静态候选及源文件SHA-256。

复现离线关联：

```powershell
python probes/runtime/analyze_school_dawn.py tmp/school-5am-7am-final-watch-20261023 --out tmp/school-5am-7am-final-watch-20261023/offline-correlation.json
```
