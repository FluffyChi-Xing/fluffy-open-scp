# 垃圾日周期：重启后单会话观测（2026-10-23）

用户观察到游戏 6:00am 全城出现垃圾，垃圾车即将开始收集。本轮只采集一次，未控制游戏时钟或暂停。采集到的是清运阶段的本地库存减少，**没有捕获垃圾生产瞬间，也没有独立核实采样开始时 HUD 恰为 6:00am**。

## 运行和质量

- 目标 PID 58376，路径 `D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCity\SimCity.exe`；哈希与既有零售版匹配。
- 开始：2026-10-23 02:11:00 HKT（真实时间），采集窗口 20 秒，记录中的 simTicks 为 46057–46063；未建立 tick 与游戏日历时间的换算。
- 单个宿主，`unit-effects --resource-id 0x7CB23A4E --all-targets --max-records 500`，三处业务 hook；互斥锁覆盖 attach 至清理。
- 输出：`tmp/garbage-6am-observation-20261023/{config,status,trace}.json*`；独立复核 `verification.json`；静态规则 `static-rules.json`。
- `unitRun` 36,125 次、`applyOps` 32,762 次、`binFind` 656,854 次入口命中。这些是函数调用计数，不是同等数量的独立垃圾事件或文本日志。
- 18 条记录、0 丢弃、0 未完成请求。其中 9 条有可独立验证的快照，另外 9 条配套规则含 27 次 `extra_lookup` 解码错误。宿主 `errors=[]` 不表示读错误为零；原程序会因 summary.errors 非零返回失败。本轮应标记为**部分有效**。
- `scriptUnloaded=true`、`detached=true`、`detachReason=application-requested`、`unexpectedDetach=false`、`targetAliveAfterCleanup=true`。采集后 PID 仍在，检查窗口未发现新的 SimCity Application 1000 事件。这不是长期稳定性保证。

## 实测垃圾减少

资源 `0x7CB23A4E` 的名称由当前安装资源导出的 UI 常量 `kResourceIDGarbage` 确认。规则 `0xC1F4C627`、运行索引 7416 的 input=1、output=0；每次实际减少量等于 applyCount，以下 9 条均通过独立快照校验。

| 当前会话 unitSlot | 垃圾库存变化 | 容量 |
| --- | --- | --- |
| 307 | 3000 → 0 | 9000 |
| 809 | 10 → 0 | 60 |
| 309 | 30 → 0 | 90 |
| 1014 | 300 → 0 | 900 |
| 157 | 100 → 0 | 600 |
| 952 | 10 → 0 | 60 |
| 909 | 1000 → 0 | 6000 |
| 1001 | 300 → 0 | 900 |
| 927 | 1000 → 0 | 6000 |

合计减少 **5750 个内部资源单位**，不是全城总产量，也不能直接改称吨数。slot 是当前会话身份，本轮没有建筑类型映射，不能把某个 slot 凭数量猜成小学。

## 静态 ERZ 对拍

`tmp/live-school-installed-main.bin` SHA-256：`a388e3c15080a4721e7d0047da180f6ee9f38292060c67892fa34b952564c381`。用 `probes/runtime/unit_rule_resource_bridge.py` 解码两个 ID；静态索引 7416、7417 与本轮运行索引一致。这里只解码资源前缀和 fromBinOps，不等于完整原始 ER2 源码。

`C1F4C627` 的静态操作：

| 作用域 | 资源 | input | output | 本轮证据 |
| --- | --- | --- | --- | --- |
| local | Garbage `7CB23A4E` | 1 | 0 | 实测扣减，9 条快照 |
| nearby | Garbage `7CB23A4E` | 0 | 1 | 静态定义；接收者及增量未捕获 |
| global | GarbagePickedUpToday `35B2D890` | 0 | 1 | 静态定义；全局增量未捕获 |

这一组合强烈支持其为垃圾清运转移规则。仍不能仅凭本地扣减证明垃圾车身份、目标库存或全局累计已成功写入。

每个单位随后都出现 `E08882A3`（索引 7417）。静态 local 操作依次是 Garbage 的负 output 范围条件、GarbageDisplayCount `54CA89F0` 和两个未命名资源 `2F207D6B` / `41FA6F69` 的消费；global 操作为 GarbagePickUpNeeded `F66950A5` 的消费。fromBinOps 把三个本地数量及显示计数读入消费参数，说明静态 999999 是被运行时替换的参数，不能当实测消费量。其结构符合清空垃圾后的显示/需求清理，但配套写入快照在本轮未可靠取得。

## 解码修复

原 `effects_agent.js` 用 resource-id 直接删掉操作数组中的其他资源，但原生函数仍逐个执行这些操作的 bin lookup。Garbage 在第二条规则中只是负 output 条件，不产生写入 lookup；剩余三个配套操作被过滤后，每条记录就多出三次 `extra_lookup`（9×3=27）。过滤还可能让前面的非目标 lookup 错配到目标资源，因此不能笼统信任旧版所有带 resource-id 的多操作记录。本轮有效规则只有一个 local 操作，并经静态对照及独立快照校验。

修复改为：resource-id 筛选含目标资源的整条更新，保留完整操作顺序及配套资源。增加三项回归测试：目标前有其他操作、目标仅为条件后有三个操作、没有目标时排除整条更新。共 27 项 JS 测试及 Python 编译检查通过；修改后没有再次注入，不能把离线测试说成修复后实机验证。原始采集数据不改写。

下一步仍需在已明确游戏时间的窗口捕获 Garbage 的正增量，并解析生产规则的时间条件；清运目标、车辆以及全局计数的闭合证据是另一个待验证环节。
