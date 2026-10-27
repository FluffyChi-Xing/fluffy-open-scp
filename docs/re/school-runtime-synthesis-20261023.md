# 小学机制与社区人员面板：离线综合结论

本轮分析使用用户实际安装版的已导出资源、PID58376的小学slot802快照及用户指定的社区Mod；没有启动游戏或新探针。用户已确认最后游戏退出是手动操作，不是崩溃。更早PID36688的Frida并发事故单独保留，不与本次退出混淆。

## 数据覆盖和质量

| 原始目录（tmp下） | 实际模拟时段 | 样本数 |
|---|---|---:|
| school-9am-counters-r2-20261023 | 11:14:08–14:08:34 | 1120 |
| school-3pm-6pm-watch-20261023 | 14:52:28–20:52:58 | 304 |
| school-5am-7am-final-watch-20261023 | 次日05:20:43–08:20:49 | 136 |

共1560个原始快照，均通过保存的身份稳定检查，tickBefore=tickAfter，原始行数与最终status样本计数相符。这不保证原子读取或没有漏掉瞬时变化。时段之间有空档，不能拼成完整连续一天；文件名不是覆盖起点。时间来自模拟周期字段，未与HUD同帧校准。最后两轮超过用户要求的结束时刻，黎明缺少05:00–05:20，报告明确保留偏差。

此前按窗口切片后再差分，会漏掉第一帧相对窗口外前一帧的变化。现已修复分析器，保留前一帧为参照，同时保持统计范围只包含选定窗口。

## 建筑不是一个工人数，也不是一种资源

安装版小学本体Property的静态值：

| 字段 | 哈希 | 值 | 用途 |
|---|---|---:|---|
| Work_MinimumWorkersForProduction | 0A7917C8 | 5 | 最低生产劳工阈值 |
| Work_TotalJobs | 0A7917C3 | 25 | 本体岗位配置 |
| Work_JobsW1/W2/W3 | 0A7917C5/C6/C7 | 20/5/0 | 按财富类别配置的岗位 |

最低阈值与可吸纳的就业人数是不同参数，支持用户对最低员工数和ExtraWorker的区分。运行时本体JobsW1/2/3却为68/13/0：满配扩建、连接和聚合的完整来源仍需追踪，不能把运行bin直接写回本体配置。用户所举工商建筑的数十最低员工和数百岗位，本轮没有相应实例证据。

运行快照显示基础WorkerW1=5、W2/W3=0，WorkersNeeded的amount/capacity=5/5。它不是“还有5个职位空缺”：bundle30645625的容量映射把WorkersNeeded关联到0A7917C8；规则还会读取remainingCapacity，amount和剩余缺口必须区分。ActiveBuilding资源bin始终为1；这是资源，不是已读取的UnitFlagActiveBuilding。

阈值规则11514BA4引用0A7917C8，按开发版SpecialiseUnitRule/OverrideResourceOp语义可将TotalWorkers操作专用化为input=output=5：检查至少有5，净变化为0。关闭分支、flag同步和零售实现逐分支确认仍未完成，且本轮没有低于5的样本。因此“阈值5”已获静态佐证，“低于5导致关闭”由用户观察补充，不能伪装成此轮捕获的状态变化。

还有一个不能忽略的候选：小学本体SCUnitBuildingType指向406CCC43；其中F1FD62EF读取WorkersNeeded剩余容量，增加TotalWorkers、WorkersNeeded并增加ActiveBuilding。这提示某些基础人员/运行状态可能由设施初始化或激活规则填充。当前未捕获该入口，不能从WorkerW1=5推断有5名具体老师一直站在建筑中。

## 已有证据解释了哪些时间事件

| 事件 | 保存数据 | 静态对应与边界 |
|---|---|---|
| 到校 | 11:14以后Student875→950、席位75→0；次日07:18首次见Student0→1 | B52C889D的sink资源签名吻合；旧函数级日志已证明同tick可处理多次到达 |
| 放学 | tick46559→46560跨15:00：Student/Studying950→0、席位0→950；TotalStudents留950 | 原始全窗口捕获了转换；离校具体规则及人物路径仍未知 |
| 跨日统计重置 | 05:59:56–06:01:12：TotalStudents950→0，ExamsPassedE2也950→0 | C193A5BB配置day=0.25并用fromBinOps清零TotalStudents；考试清零规则另查 |
| 水电消费 | 两条规则均专用化为72 tick；库存多次270→180后补回 | 每次90等于Needed值。调度重置与消费相邻，不代表每次条件必通过 |
| 垃圾生产/清空 | 跨06:00垃圾0→100、显示计数0→1；约06:11同时回0 | TotalGarbageToCreate=100；生产applyCount、每日入口和具体清运车仍未闭合 |
| 额外工人到达 | 06:30 ExtraWorkerW1 0→1，TotalWorkers5→6 | 938552F2具有相符操作，尚非运行命中证据 |
| 暂停/退出 | 06:58 ExtraWorker→Paused；附加07:14 Paused清零、TotalWorkers6→5 | 存在对应sink及后继规则候选，不能把Paused直接解释为下班 |

三个窗口的Student+SpotsForKids均保持950。TotalStudents更符合每天06:00清零的累计到校统计，不是当前在校人数或容量。

“学生放学”和“设施停止运行”也必须分开：放学后Student=0，但ActiveBuilding资源仍1，水电仍出现减90和补给；实测不支持“只要放学就绝对停耗”的泛化。18:00之后的附加样本还展示了水电计数器继续运行。垃圾和污水短窗口为0不等于不生产；本次垃圾的跨06:00增加正是此前遗漏的时间尺度。

维护费资源ExpensesFromMaintenance始终300，不能将静态费用/账目输入当成实际市政扣款流水。污水始终0也不能排除瞬时生成后排出。两者的结算/传输路径仍待离线分析。

## 社区Mod提供了可直接核对的UI公式

详见[社区Mod报告](community-worker-mod-analysis-20261023.md)。包中没有ER2/ERZ；它以Graphics Property的扩展rollover项连接kWorkers面板。小学该Property与安装版相比仅增加FNV-1("kWorkers")=E15027F1。

面板查询connectedUnits的Worker、ExtraWorker、ExtraWorkerWanted、Jobs，按财富等级计算：

```text
显示人数 = Worker + ExtraWorker
显示capacity = Jobs + ExtraWorkerWanted + ExtraWorker
```

这个显示capacity是公式结果，不是资源bin.capacity。Mod未计入ExtraWorkerPaused，也未查询最低阈值。用小学本体快照代入公式：06:30低财富6/83，06:58变5/82，但TotalWorkers仍6。差异由Paused状态和查询口径解释，不能把Mod当成无条件准确的总人数测量。它聚合connectedUnits，而现有快照只有小学本体；两者数值不可直接相等验证。

## 对OpenSCP的具体设计约束

- 资产编辑器分开提供最低运行劳工阈值、按财富分配的岗位配置和运行时状态。保留基础/继承/扩建来源，不把运行bin当作Property原值。
- UI绑定保存完整scope、connectedUnits或units、amount/capacity/flag、resource/property ID、计算公式和是否计入Paused。菜单类别只决定入口，不能自动赋予学校或工商业行为。
- 规则检查器显示定时触发、sink事件、条件检查、fromBinOps和Property专用化。input=output是可能的数量门槛，不能把它统计成净消耗。
- 行为闭环检查至少覆盖最低劳工、维护费、水电补给/库存、污水垃圾、教育输入输出和模块路径。当前可支持只读依赖检查和已知模板校验，仍不能声称任意ER2编辑已就绪。

下一步仅离线推进：①最低劳工检查到关闭/开启的分支；②ExtraWorker请求、Paused和离开链；③垃圾定时生产及清运；④维护费全局结算和污水发送；⑤教育考试结果回传住宅及校车/path/spawner绑定。

## 复现与保存

```powershell
python probes/runtime/analyze_school_timed_watch.py tmp/school-3pm-6pm-watch-20261023 --start-tick 46560 --end-tick 46740 --out tmp/school-3pm-6pm-watch-20261023/analysis-15-18-with-boundary.json
python probes/runtime/compare_worker_mod.py --capture tmp/school-9am-counters-r2-20261023 --capture tmp/school-3pm-6pm-watch-20261023 --capture tmp/school-5am-7am-final-watch-20261023 --out tmp/community-worker-mod-20261023/comparison.json
```

原始快照、解包内容和JSON证据按仓库.gitignore保留在本地，不强制入库。代码、研究文档和修复测试提交；因此新checkout需按报告重新提供本地安装资源和采集文件才能复算本次实例。所有逆向规则说明均基于编译ERZ，不是已恢复的原作者ER2文本。
