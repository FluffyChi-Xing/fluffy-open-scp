# 全局规则资源数量的执行前后实测

2026-10-17，接续 [规则定义对拍](rule-definition-fields-20261017.md)。本阶段实现了定向观察引擎真实资源写入的采集器，以及工作台前值/后值/差值/容量表格。不是手动执行规则，也不将配置数值冒充实际数量。

## 实机结果

PID 7424、game `0x2dc4dbf8`、规则 `0xC2181F55`（index 368），15 秒窗口捕获 7 次资源操作：

| 资源 ID | 资源索引 / bin 索引 | 数量变化 | 容量 | 实际差值 |
|---|---|---|---|---|
| `0xC9FB4C92` | 4631 / 968 | 2→3、3→4、4→5、5→6、6→7、7→8、8→9 | 每次前后均 24 | 每次 +1 |

applyCount=1，实际传入操作 input=0/output=1。7/7 的快照和独立计算的 x86 32 位结果一致；nativeSuccess 均为 true。读取错误、丢弃、停止时未完成请求均为 0，已卸载探针并 detach，没有关闭游戏。

入口命中 globalRun 74 次、ApplyBinRule 11593 次、bin lookup 364348 次；只有 7 条目标操作产生记录。短时筛选不代表零开销，也不代表覆盖所有规则或资源。完整状态、7 条原始记录、独立校验和三样本指令摘要见 [证据 JSON](evidence/rule-resource-effects-20261017.json)。

## 如何保证读的是引擎实际修改的槽

三个入口均先验证目标 EXE 完整 SHA-256 和入口指纹：

1. `Global.RunRule=0x618890` 建立当前线程的规则上下文，读取 ruleIndex、ruleID、game；限定 ECX=game+0x478。
2. 仅接纳该上下文中来自调用点 `0x6189F9`（返回地址 `0x6189FE`）的 `ApplyBinRule=0x606580`，读取 applyCount、实际操作数组、bins、game。此时 fromBinOps 的动态替换已发生，因此采集的是传入更新函数的操作。
3. 仅接纳 ApplyBinRule 内 `0x6065CB`（返回地址 `0x6065D0`）调用的 `ResourceBins.find=0x58EC70`。在返回时读取引擎实际返回的 binIndex，核对该槽中的有符号 16 位 resourceIndex，再记录 amount/capacity。没有重新实现游戏 hash 查找，也没有主动调用查询函数。

槽步长 12 字节，resourceIndex 位于 +0（int16），amount 位于 +4，capacity 位于 +8。先检查 vector begin/end、步长和索引范围。下一次受控 lookup 前或 ApplyBinRule 返回时读后值，并再次核对槽指针和身份。同一资源重复出现在操作数组中时，分别记录相邻操作的前后值，避免把总差值重复归给每个操作。

每个线程有嵌套边界；不匹配的更新仍阻断父上下文关联。固定首个满足过滤条件的 game/rule；最多 16 项操作、最多 2000 条记录、最多 120 秒。超大操作数组主动筛除，不能当成完整覆盖。尚未结束的更新计入 incompleteRequests。

## 返回失败不等于没有写入

零售指令在 `0x6065FD` 先写 amount，之后检查负值或超容量；失败直接返回 false，未回滚已经写入的槽。因此 nativeSuccess 仅表示该资源更新函数返回值，不代表整条规则成功，更不代表存档已持久化。失败后的部分写入已经用模拟测试覆盖，但本轮实机没有遇到失败案例。

output<0 的范围条件在这个函数中跳过写入，显示为“范围条件（此函数不写入）”；没有观察到的操作保留未知，不填 0。整数运算按 x86 NEG/SAR/AND、乘法和加法的 32 位回绕验证，实测 delta 另按后值减前值保存。`matchesInt32` 是运算一致性，不是业务正确性。

本轮没有排除其他线程同时写同一槽的可能，也没有测量整条规则的全部副作用。只能报告受控调用间读到的值及其与更新公式的一致性，不能将它泛化为所有变化的独占因果证明。

## OpenSCP 使用

```powershell
python tools/dynamic/rule-logs/capture.py --pid <当前PID> --scope effects --rule-id 0xc2181f55 --seconds 15 --out tmp/my-rule-effects
python tools/dynamic/rule-logs/verify_effects.py tmp/my-rule-effects
```

`--rule-id` 可省略，此时选首个符合条件的全局资源更新；不接受 state-index/unit-slot 或冷启动。导入 trace.jsonl 后工作台显示资源操作表，搜索资源 ID 可高亮；再导入同目录 status.json 查看质量。当前仍由命令行启动观察窗口，不宣称已经实现 UI 一键采集。

剩余范围：单位规则、条件失败时为什么未执行、资源显示名和源码侧表，以及 UI 启停定向观察。无需直接调用 LogGlobalRule/LogUnitRule 伪造 delta 记录来实现这些诊断功能。
