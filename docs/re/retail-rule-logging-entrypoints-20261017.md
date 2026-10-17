# 零售规则日志入口：IDA 对拍结果

本轮已实际运行 IDA 9.2 idalib / Hex-Rays 局部反编译，并用 Capstone 逐字节复核。使用自建数据库副本 `tmp/rule-logging-20261017/retail.i64`，没有打开或修改用户正在处理的 IDA 数据库。随后附加用户已进入城市的离线游戏，完成有界日志采集及原查询函数返回值对照。静态与运行证据分别归档。

## 十个区间已完成三样本对拍

| 语义识别 | 零售 VA | 关键证据 |
|---|---:|---|
| EncodeActionHeader | 0x5CC130 | data1 阈值 FFFE，data2 阈值 FE，调用大端写入 |
| DecodeActionHeader | 0x5CC1D0 | 新增版本参数，27 为 data2 编码分界 |
| cEcoGame::LogAction | 0x5CFB00 | 先补 tick 差，再写指定 action |
| cEcoGame::LogGlobalRule | 0x5CFE20 | action 5，stateIndex，随后写 applyCount |
| cEcoGame::LogUnitRule | 0x5D47F0 | 可先写单位移动、单位选择，再写 action 1 和 runState |
| cEcoBinSet::ApplyRule | 0x618AC0 | mRuleIndices 映射、CanRun / Run 分支、调用 LogGlobalRule |
| cEcoUnit::ApplyRule | 0x61FD60 | 单位本地索引映射、条件调用 LogUnitRule |
| cEcoGameScripts::UnitRuleFromIndex | 0x5F76B0 | 规则集高 12 位 / 集内低 20 位，deque 元素大小 0x9C |
| cEcoGameScripts::GlobalRuleFromIndex | 0x5F6AE0 | 同样拆分索引，vector 元素大小 0x68 |

| UnitRuleDeque.Advance | 0x5F6CD0 | 每块 4 个元素，每块 624 字节 |

VA 基于 0x400000。名称是根据开发版源码与零售控制流/字段的语义识别，不是从零售符号表读取。

真实安装 `SimCity：Cites of Tomorrow/SimCity/SimCity.exe`、备份 `SimCity.exe` 和 `SimCity_dump_SCY.exe` 的十个函数区间逐字节一致。前两者文件 SHA-256 为 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`，dump 为 `06d681444011f1297a6cf53bc233ee5f138d41786b451e6f18dc02bf273ff569`。完整指纹、入口字节、大小和静态字段见 [证据 JSON](evidence/retail-rule-logging-20261017.json)。

## 修正一：第二参数的扩展标记有版本差异

开发版编码器使用 data2 的 0x3E 为扩展、0x3F 为 -1。零售编码器 0x5CC130 使用 0xFE / 0xFF，因此 **62、63 和 253 都是合法的直接值**。

零售解码器签名多出一个参数：`(stream, version, header, outId, outData1, outData2)`，在 0x5CC24B 比较 version 与 0x1B。小于 27 走旧标记，否则走新标记。这个 version 不能直接使用 SLDelta 外壳的文件版本 4；当前还未闭合其上游版本来源。

修复 `tools/dynamic/delta_action.py`：要求显式 `stream_version`；没有版本则拒绝调用。新增独立字节测试，覆盖 62/63/253、FE 扩展、FF sentinel、双扩展顺序和截断。`analyze_xor_capture.py` 同样要求 `--stream-version`，并将输出命名为 `analysis-candidates-vN.json`、保留版本假设和“仅字节候选”说明，不覆盖旧证据。

## 修正二：零售版 action 2 不再是 unused

开发版枚举把 action 2 命名为 kLogActionUnused2，不能原样用于这个零售构建。

0x5D4835 比较 unitSlot 与 game+0x1908；不同则更新它，并调用 LogAction 写 **action 2 / data1=unitSlot / data2=0**。之后 0x5D4891 写 **action 1 / data1=0 / data2=stateIndex**，再写 runState 的 applyCount、maxApply、maxPossibleMapApply 和 selectedUnits。

因此零售规则流是有上下文的：如果只抓 action 1 或从流中途开始而没有 action 2，单位归属可能未知；不能把 action 1 的 data1=0 解释成单位 0。0x5CFD70 的规则附加路径也使用了这个当前单位状态。此结论来自写入指令，尚未进行运行时回放验证。

## 运行态字段与下一步采集契约

静态已确认：

- game+0x17A0 为参与日志的总 tick，+0x1904 为已记录 tick；+0x18F8 为 delta stream 指针，+0x1908 为当前日志单位 slot。
- 全局 ApplyRule：ECX 指向 bin set；栈参数为 stateIndex、game。`[binSet+0x98][stateIndex]` 取得 ruleIndex，ruleState 位于 `[binSet+0xA8] + stateIndex*0x2C`。
- 单位 ApplyRule：ECX 指向 unit；栈参数为 stateIndex、unitSlot、game。ruleIndex 来自 `[unit+0x1CC][stateIndex]`，ruleState 位于 `[unit+0x1DC] + stateIndex*0x0C`。
- LogGlobalRule：ECX=game，参数 stateIndex；applyCount 从 `[game+0x520] + stateIndex*0x2C + 0x1C` 读出。
- LogUnitRule：ECX=game，参数 unitSlot、stateIndex、runState 指针；runState 的 +0、+0x14、+0x18 分别是三个写入标量，+4/+8 为 selectedUnits 的 begin/end。
- 脚本查询从 `[scripts+0x5C][ruleIndex >> 20]` 找到当前规则集。单位规则为 deque：起始迭代器 set+0x20，终止迭代器 set+0x30，每块 4 个 156 字节元素；全局 vector 在 set+0x54，步长 0x68。实际 scripts vtable 为 0xD1B708，+0xD0 / +0xD8 分别指向 0x5F76B0 / 0x5F6AE0。

局部反编译的栈变量类型有伪影，尤其是 Unit ApplyRule 的 runState 局部变量；以上参数顺序以栈指令和 callee 读操作为准。采集器已经核验入口指纹、向量 begin/end、索引边界及对象寿命，不能只凭这些静态偏移直接扫描所有单位。

**仍未验证：** unitRef stamp、ruleIndex 到已加载脚本名称/资源的映射、具体输入输出 bin，以及 decoder version 的上游来源。LogGlobalRule/LogUnitRule 仍只覆盖条件成立后的日志路径，不是所有规则尝试。

## 复现与验证

静态证据工具（只读 EXE，不启动游戏）：

```powershell
python tools/rule_logging_probe.py 'D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCity/SimCity.exe' 'D:/ea-games/simcity_offline/SimCity.exe' 'D:/ea-games/simcity_offline/SimCity_dump_SCY.exe' --output tmp/rule-logging-check
python -m unittest discover -s tools/dynamic -p 'test_delta*.py'
```

三样本十区间 allEqual=true；delta header / container 共 11 项测试通过。局部 IDA 输出保存在 `tmp/rule-logging-20261017/00*.c`，其哈希列于证据 JSON。没有把局部 IDA 分析范围内的空 xref 列表当成“没有调用者”；调用点先从原始相对 CALL 候选定位，再逐指令验证。

## 实机闭环与纠错（2026-10-17）

先前将 vtable 基址误认作 0xD1B70C（偏移 4 字节），把 map/zone 查询错认为 unit/global 查询。首次实机采集的 vector_bounds 错误否定了这一假设；上述地址和布局已修正。不能继续使用旧版 0x5F6AC0 / 0x5F6B00 及 0x48 / 0x88 步长。

修正后全局采集 10 秒：stateIndex 318 → ruleIndex 380，38 条；单位采集 10 秒：slot 827 / stateIndex 135 → ruleIndex 31661，1 条，applyCount=3、maxApply=9。两次读取错误与丢弃均为 0。单位总命中 22620 次，其中 22619 次主动筛除，不属于丢弃。

独立被动监听原查询函数的返回值，全局与单位各 8 次、合计 16/16 与采集器计算指针一致；没有主动调用游戏函数。该结论只覆盖这两个运行态规则，不代表所有规则均已验证。所有探针已卸载并 detach，未关闭游戏。

采集的是 logger entry，不证明 delta 持久化成功，也不涵盖所有规则尝试。unitStamp 保留 null，脚本名称、输入输出 bin 仍未解释。运行状态、样本与指针对照见 [运行证据](evidence/rule-logging-runtime-20261017.json)。使用方法见 [采集器说明](../../tools/dynamic/rule-logs/README.md)。
