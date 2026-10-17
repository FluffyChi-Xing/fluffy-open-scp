# 单位资源消费运行时对拍（2026-10-17）

## 结论

在正在运行的离线零售版进程（PID 7424，SHA-256 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`）上，新增的 `unit-consumption` 采集器在 15 秒窗口捕获了 52 条完整的单位本地资源更新。每条记录均满足：单位本地 `ApplyBinRule`、至少一个 `input > 0` 的操作、资源 bin 前后快照可读、原生返回成功。

## 实测证据

采集目录：`tmp/unit-consumption-followup-20261017b`（临时输出，不纳入 Git）。

- 记录数：52；`errors=0`、`dropped=0`、`incompleteRequests=0`。
- 目标单位：slot `889`，unit pointer 在本次会话内稳定；`unitStamp` 仍未知。
- 规则：`ruleId=0xD715D72F`，`ruleIndex=21593`。
- 模拟 tick：`37829` 至 `37831`。
- 消费操作：resource `0xE71F36B7`，`input=1`、`output=0`，amount 连续递减（例如 `104→103→102`），每次 `delta=-1`。
- 同一规则还包含多个 `input=0, output=1` 的生产操作；它们被保留在同一条规则记录中，但不是选择该目标的依据。

采集器按 `delta = applyCount × (output - input)` 对拍，并以 bin 的实际 before/after 快照核验。该窗口的目标 `applyCount=1`，因此消费差值与实测递减一致。

随后使用 `--all-targets --max-records 100` 做了 5 秒短窗口复核：捕获 100 条记录，覆盖 57 个不同单位指针和 80 个单位/规则组合；`ApplyBinRule` 命中 46,369 次，其中 13,873 次在达到记录上限后被主动丢弃。这个结果说明前一轮 52 条是首目标采样造成的数量限制，资源规则本身确实以更高频率执行。

## 代码变化

`unit-consumption` 在进入 `Unit.RunRule` 时只保留候选帧，在 `ApplyBinRule` 读取最多 16 个单位本地操作后，只有存在 `input > 0` 才锁定目标。单位本地 bins 为 `unit + 0x210`，全局、区域和交易 return site 会被过滤。新增 harness 覆盖了 `input=0` 筛除、`input>0` 接受和既有单位上下文约束。

## 仍未解决的语义

- slot 889 只能证明“单位本地规则”，不能证明它是建筑、车辆或行人；当前结构没有经过验证的建筑分类字段。
- `unitStamp`、脚本名称、资源 TGI 到磁盘 package 的来源仍未知。
- 该采集器锁定首个符合条件的单位规则，不能代表城市中所有消费主体；高频 hook 只适合短窗口定向观察。
- `--all-targets` 可以扩大单位/规则覆盖，但只保证“达到上限前”的有界样本；出现 dropped 时不能称为全量。
- `nativeSuccess` 只表示资源更新函数返回成功，不等于完整 gameplay rule 成功或存档持久化。

## 车辆/人流状态

开发源树中的 `btDiscreteDynamicsWorld::updateVehicles/addVehicle/removeVehicle` 仍只有开发地址证据，尚未完成与当前零售 EXE 的特征字节、xref 和安全 hook 对拍。因此渲染上下文采集到的 traffic/draw 事件不能升级为车辆或行人模拟移动证据。本专题保持 unresolved，下一步应先建立零售入口映射，再做只读位置/路径采集。
