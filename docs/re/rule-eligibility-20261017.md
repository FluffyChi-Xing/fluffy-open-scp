# 全局规则为何未通过条件检查

2026-10-17，接续 [资源变化实测](rule-resource-effects-20261017.md)。新增 `conditions` 模式观察实际 CanRunRule 返回值、控制标志、资源允许次数及引擎选中槽的数量/容量。工作台直接解释已证实的阻止原因；允许通过不等于实际执行。

## 实机结果

PID 7424、game `0x2dc4dbf8`，10 秒内 410 次检查，独立 Python 校验 410/410：

| 结果 | 次数 | 证据含义 |
|---|---:|---|
| 允许通过 | 243 | CanRunRule 返回 true，尚不证明随后执行或写入 |
| 输入不足 | 83 | 实际数量不能满足一次正输入需求，资源允许次数返回 0 |
| 剩余容量不足 | 7 | 剩余容量不能容纳一次净增加，资源允许次数返回 0 |
| 达到或超出范围上界 | 76 | output<0 条件要求 amount < -output，实际不满足 |
| 控制标志不匹配 | 1 | 64 位 flags/mask/expected 检查失败，未进入资源计数 |

读取错误 0、丢弃 0、停止时未完成请求 0，采集结束已 detach。globalCheck 入口命中 428 次，其中 18 次主动筛除；资源计数入口命中 121748 次、接纳 409 次；bin lookup 命中 410523 次。所有 hook 有运行开销，这不是无损全局跟踪。

三个具体例子：

- `0x739D42FF` / 资源 `0x98EC1F24`：22/24 时资源允许次数为 2，经 maxApply=1 限制，最终为 1；达到 24/24 后允许次数为 0，原因是容量不足。这是两个时刻的检查证据，不单独证明两次检查之间所有写入的来源。
- `0x9172AD26` / 资源 `0x80550550`：数量 0、容量 90、input=1/output=0，返回 0，原因是输入不足。
- `0xE8FC6CBE`：控制 flags 低字为 `2147909446`，mask=4、expected=0，实际 masked bit=4，因此拒绝；不把它归为资源不足。

完整 410 条记录、状态、独立校验结果及三样本一致的指令见 [JSON 证据](evidence/rule-eligibility-20261017.json)。UI fixture 取每类第一条真实记录。

## 零售调用链

`Global.CanRunRule=0x618660` 的 ECX 为 bin set，四个参数依次为 ruleIndex、rule、state、game；仅接纳 bin set=game+0x478 的全局检查。规则 +40/+44 为 64 位 test mask 两个字，+48/+52 为 expected；bin set +0xB8/+0xBC 为当前 flags。

规则 +28 为 minApply，+32 为配置 maxApply，+36 为 applyResourceID。state+28 的 applyCount 初始设为 -1；控制条件不满足时置 0。资源计数后，正数低于 minApply 才置 0；只有有效 maxApply>0 时才限制上界。因此配置 maxApply=0 不能在诊断中解释成禁用。applyResourceID 非零时可能从属性或资源覆盖 maxApply，本探针不将配置值冒充有效上限。

仅接纳调用点 `0x618821`（返回 `0x618826`）的 `BinRuleCount=0x606370`，读取动态数量处理后的实际操作数组。仅接纳其中调用点 `0x6063AE`（返回 `0x6063B3`）的 `ResourceBins.find=0x58EC70`；使用原函数返回槽号，按 12 字节步长读 amount/capacity，并核对 signed16 resourceIndex。没有主动调用游戏函数，也没有复制游戏 hash 查询算法。

线程内嵌套边界隔离父子调用。最多记录 16 项实际操作；超限 countCapture=over-limit，不给出猜测原因。执行提前返回时，后续操作保留 not-observed。诊断按原始顺序重算允许次数，只有与引擎实际计数返回值、最终 applyCount 和 false 结果相符才指明某项资源阻止条件。

## 已覆盖与仍未知

输入不足、容量不足、范围上界及控制标志均有本轮实机样本。最低次数不足、范围下界及 bin 缺失有模拟反例测试，本轮未遇到实机样本。

如果资源注册查询失败导致没有执行 bin lookup，或动态 fromBinOps 计算提前失败，没有足够直接证据时显示“原因未确定”。不把未观测到的操作当成数量 0，不声称复原完整调度器、所有规则尝试或单位规则的失败原因。

当前重算只对非负数量、capacity>=amount 及无相关有符号溢出的快照解释具体资源原因；其他情况保留原始数值而不推断。快照没有排除其他线程并发写入，因此报告调用间观测与指令结果的一致性，不声称独占因果。

## 使用

```powershell
python tools/dynamic/rule-logs/capture.py --pid <PID> --scope conditions --seconds 10 --max-records 1000 --out tmp/conditions
python tools/dynamic/rule-logs/verify_conditions.py tmp/conditions
```

可加 `--rule-id 0x739d42ff` 定向观察。默认固定首个匹配 game，记录其中各全局规则；与 effects 模式默认固定首个 game/rule 的策略不同。不支持 state-index/unit-slot，也不冷启动游戏。工作台导入 trace.jsonl 和配套 status.json 后可按规则 ID、资源 ID 或原因代码筛选；中文/英文解释随界面语言切换。当前仍通过 CLI 启动观察窗口。
