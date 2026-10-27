# 社区“建筑人口调查（显示）V2.41”与小学运行数据对照

## 样本与拆包

用户指定的 RAR 位于：

`D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityUserData\Packages\建筑人口调查 (显示）V2.41待定.rar`

本次分析的 RAR SHA-256：`30be9a367ce5a7167b6a5a1036e2c17d717da713bbfac74d86b9f67c067701f1`。

RAR 内只有一个资源包 `ExtendedWorkerDataInRollover.package`（520,927 bytes）。离线拆包得到 100 个条目：96 个 Property（`0x00B1B104`）和 4 个 UI 脚本（`0x67771F5C`），没有 ER2/ERZ 条目。包内包含小学 Graphics Property `40E1C000:C264A268` 的覆盖项；与安装版 `SimCity_Game.package` 比较，唯一属性差异是 `0x0CAFBDC3`（kPropUnitRolloverExtendedItemIDs）数组新增 `0xE15027F1`，等于小写FNV-1("kWorkers")。它将人员面板接到小学扩展悬浮信息上。这一对照只覆盖小学这个TGI，不宣称96份Property都只改了显示。

资源包和解出的 UI 脚本只作离线证据，未加载到游戏，也未修改存档。

## Mod 的实际 UI 数据模型

`8792243B:58FDB9EE` 中的 `simcity.RolloverItems.kWorkers` 明确使用以下查询：

```text
connectedUnits → unitRef → binAmount → WorkerW1/2/3
connectedUnits → unitRef → binAmount → ExtraWorkerW1/2/3
connectedUnits → unitRef → binAmount → ExtraWorkerWantedW1/2/3
connectedUnits → unitRef → binAmount → JobsW1/2/3
```

它的显示计算为：

```text
显示实际工人 W1 = WorkerW1 + ExtraWorkerW1
显示岗位容量 W1 = JobsW1 + ExtraWorkerWantedW1 + ExtraWorkerW1
```

W2/W3 使用相同公式。Mod 还查询 `UnitFlagActiveBuilding`，但这段update没有使用返回的active值，不能声称它已显示营业状态。它没有把 `ExtraWorkerW1Paused` 加入 `kWorkers` 的实际工人公式；Paused是另一个状态字段，不能与活动额外工人混为一谈。这里的“capacity”是Mod传给文本模板的计算值，既不是直接读取bin.capacity，也不保证是永久不变的岗位上限。

这与本次小学快照的关系是：

| 模拟时间 | WorkerW1 | ExtraWorkerW1 | ExtraWorkerW1Paused | TotalWorkers | Mod 公式投影 |
|---|---:|---:|---:|---:|---:|
| 05:20 | 5 | 0 | 0 | 5 | W1=5 |
| 06:30 | 5 | 1 | 0 | 6 | W1=6 |
| 06:58 | 5 | 0 | 1 | 6 | W1=5，Paused=1 |
| 07:14（附加窗口） | 5 | 0 | 0 | 5 | W1=5 |

在已保存的 136 个小学样本中，下面的快照关系始终成立：

```text
TotalWorkers = WorkerW1 + WorkerW2 + WorkerW3
             + ExtraWorkerW1 + ExtraWorkerW1Paused
```

这只是当前小学实例的观测关系。Mod 的 `connectedUnits` 聚合范围可能包含默认校车站、教室等子 Unit，因此 Mod 面板的数值不一定等于小学本体 slot 802 的单独 bin。

## 最低劳工与额外岗位

安装版小学 Property `40E0C000:C264A268` 明确包含：

```text
0x0A7917C8 = 5  // kPropWork_MinimumWorkersForProduction
0x09AE19D7 = 300 // maintenance expense property
```

最低生产劳工阈值5有本体Property和安装版常量名证据。用户补充“低于5设施不能正常开启”是实际游戏观察，现有快照没有低于5的区间，不能把该关闭过程写成已捕获事件。快照读取的是资源bin ActiveBuilding=1，不能直接等同Mod查询的UnitFlagActiveBuilding；二者转换链仍待闭合。

最低阈值还进入规则11514BA4的专用化表：prop=0A7917C8、opId=7、data16=0、data8=2。开发版带符号反编译的SpecialiseUnitRule把opId=7路由到local资源操作，OverrideResourceOp(type=2)把input和output同时改成属性值。因此该规则对TotalWorkers的基础(0,0)可专用化为(5,5)，这是保持净数量不变的最低数量检查。此解释依赖开发版语义与安装版专用化数据的对照，尚未逐指令核对零售专用化分支或最终关闭分支。

`ExtraWorkerW1` 表示基础劳工之外的额外就业状态；`ExtraWorkerWantedW1` 是Mod为B1731656提供的额外需求名称，属于社区命名佐证。`JobsW1/2/3` 是Mod容量公式的一项，不应脱离请求与占用状态就解释为总就业上限。小学本体静态TotalJobs=25、JobsW1/2/3=20/5/0，而满配实例快照JobsW1/2/3=68/13/0；扩建聚合与覆盖路径尚未完整复现，不能用后者倒填本体Property。工商建筑“最低几十人、容纳数百人”的例子由用户提供，本轮未采集工商样本。

投影也显示分母会变化：06:24 W1为5/83、W2为0/18；06:30 W1为6/83、W2为0/13；06:58 W1为5/82，而TotalWorkers仍为6。Mod的显示口径和资源总数出现差异，是理解Paused与请求资源的线索，不能判定Mod或模拟计数必有一个错误。

## 与 ER2 候选规则的对照

小学父 bundle `40E0C700:30645625` 引用了 Worker/Jobs 资源。已解码的候选操作包括：

- `0x938552F2`：`ExtraWorkerW1 +1`、`TotalWorkers +1`，并清理额外岗位请求资源；形态上符合 06:30 的 `0→1` 与 `5→6`，但没有运行时 rule-hit 记录，不能唯一归因。
- `0x6C855B1F`、`0xD2460875` 等：`ExtraWorkerW1 -1`、`ExtraWorkerW1Paused +1`；形态上符合 06:58 转入暂停状态，但同样只是候选。
- `0xB667C3C7`：`ExtraWorkerW1Paused -1`、`TotalWorkers -1`；形态上符合附加窗口 07:14 的回落。

这些候选说明 ER2 资源操作可以表达“基础工人、额外工人、暂停额外工人、总工人”的状态链；它们不是本次快照的函数级执行证明。完整闭合仍需要单宿主短时运行日志或静态绑定到具体 sink/event 数组。

## OpenSCP 应如何使用这项证据

人员面板和资产编辑器应把以下概念分别建模：

1. `minimumWorkersForProduction`：运行阈值，来自 Property `0x0A7917C8`。
2. `workersByWealth`：当前基础工人 `WorkerW1/2/3`。
3. `extraWorkersByWealth`：额外已吸纳工人 `ExtraWorkerW1/2/3`。
4. `extraWorkersWantedByWealth`：额外岗位需求。
5. `jobsByWealth`：Mod显示容量的基础项，保留其与静态岗位属性的区别。
6. `pausedWorkersByWealth`：暂停状态，不能默认计入活动工人。
7. `activeBuilding`：设施是否允许运行，单独绑定 `UnitFlagActiveBuilding`。

UI 查询路径必须显示 `connectedUnits` 还是本体 `units`，并给出数据来源。资产验证器在用户把建筑定义为学校、商业或工业时，应分别检查最低劳工、岗位容量、ExtraWorker 状态资源和 ActiveBuilding 规则；不能用一个 `totalWorkers` 字段代替整个闭环。

原始 Mod 包不纳入仓库；可复核的拆包清单、脚本 SHA-256、属性差异与小学快照对照保存在：

- `docs/re/evidence/community-worker-mod-comparison-20261023.json`
- `tmp/community-worker-mod-20261023/`

关键UI源位置：8792243B-58FDB9EE.js:3231定义Wanted别名，3255定义查询，3276定义公式；布局334E67F8对应FNV-1("RolloverItemWorkers")。来源SHA和计算复现由`probes/runtime/compare_worker_mod.py`生成，不执行解出的JavaScript。JSON/原始包按仓库现有ignore规则留在本地，报告和探针代码入库。

复现（先用7-Zip解出上述单个package，再用ui_dump_tgi、prop_dump导出报告引用的TGI）：

```powershell
python probes/runtime/compare_worker_mod.py --capture tmp/school-9am-counters-r2-20261023 --capture tmp/school-3pm-6pm-watch-20261023 --capture tmp/school-5am-7am-final-watch-20261023 --out tmp/community-worker-mod-20261023/comparison.json
```
