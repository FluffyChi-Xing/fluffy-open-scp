# 单位本地资源更新实测

2026-10-17 在用户正常游玩的 PID 7424 上完成 15 秒被动观察，随后卸载探针并 detach，没有主动调用规则或重启游戏。目标为 `D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCity/SimCity.exe`，SHA-256 为 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。

## 实际观察

仅采集首个匹配 game/slot/pointer/rule 的本单位资源更新，共 1 条。单位 slot 189、地址 `0x45a2b1a8`，tick 24056，规则 `0xAC95E620`、当前规则索引 31862。资源 `0x9E3C3DFA` 的 amount 从 586 增至 864，capacity 保持 864。有效 input=0/output=1，原生 applyCount=278，实测增量 278 与整数更新公式一致。原生 ApplyBinRule 返回 true。

读取错误、丢失和未完成请求均为 0；独立快照计算验证 1/1 通过。该验证确认前后槽身份和算术一致，不独立证明内存所属单位。所属单位由探针的向量边界、指针和调用点检查约束。

完整记录、状态、各函数指令及三份 EXE 对拍哈希保存在 [证据 JSON](evidence/unit-resource-effects-20261017.json)。UI 固定样本为 `src/pages/runtime-observer/fixtures/unit-effects.json`。

## 零售调用链

- Unit.RunRule：VA `0x61DB00`，ECX 为 unit，栈参数为 unitSlot、ruleIndex、rulePointer、runStatePointer、game。以指令为准，不能沿用 ApplyRule 反编译器猜测的签名。
- Unit.ApplyRule：VA `0x61FD60`，在 `0x61FDEF` 调用 RunRule。
- game 的单位向量 begin/end 位于 `+0x554/+0x558`，元素步长 `0x380`；检查 slot 范围及计算出的指针与 ECX 一致。
- 本单位 bins 位于 `unit+0x210`。只接受 ApplyBinRule 返回地址 `0x61DE80`，排除区域 `0x61DEF4`、全局 `0x61DF26`、交易 `0x61DF96/0x61E1C0`。
- ApplyBinRule VA `0x606580`，其中 ResourceBins.find VA `0x58EC70` 的目标返回地址为 `0x6065D0`。bin 步长 12，resourceIndex 为 +0 的 signed16，amount/capacity 为 +4/+8。

相关四函数在真实游戏 EXE、备份 EXE 和脱壳样本的对拍区间一致。开发源树 `src/GB/cEcoUnit/RunRule.c` 和 `ApplyRule.c` 辅助解释；执行事实来自本次零售运行记录。

## 使用与边界

```powershell
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope unit-effects --seconds 15 --out tmp/unit-effects
python tools/dynamic/rule-logs/verify_effects.py tmp/unit-effects
```

可附加 `--unit-slot N` 和 `--rule-id 0x...`；不接受 state-index 或冷启动。工作台导入 trace.jsonl/status.json 后，可检索资源 ID、查看前后数量与容量，并显示本单位归属及双语限制提示。

stamp 仍未知并保留 null，slot/地址不能作为跨会话稳定身份。单位特化索引可超过基础规则数量；本次未证明规则名称或该特化规则的 package 来源。nativeSuccess 仅表示资源更新内核返回值，不代表完整规则成功、存档持久化或所有模拟影响。并发写入的影响也未排除。

本轮不覆盖全局、区域、邻近单位或交易资源。单条样本不能推导整座城市的行为。15 秒内 unitRun 命中 53,867 次、applyOps 命中 39,385 次、binFind 命中 1,024,054 次，存在高频 hook 开销，不能描述为零影响常驻监听。目前仅完成定时 CLI 采集和 UI 导入分析，尚无 UI 一键采集或长期后台监控。
