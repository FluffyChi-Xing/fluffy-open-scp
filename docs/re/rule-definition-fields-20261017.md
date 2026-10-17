# 规则名称与资源操作定义：零售对拍和实机验证

2026-10-17，接续 [实际 package 来源](rule-resource-provenance-20261017.md)。本阶段新增全局规则定义采集与离线解析，运行态 59 条定义全部与导出的 AEB 条目匹配。没有主动调用游戏函数，采集结束已 detach。

## 名称为何不能直接读出来

零售 `RuleBase.Read=0x5FDB00` 先读 ID；若 namesIncluded 为真，名称读进栈上的临时 string，随后释放。对象仅保留 ID、flags、simInfo offset。开发版 `GB/cEcoRuleBase/Read.c` 也是这一行为。当前加载资源 header 为 5/2/14、namesIncluded=0，没有这段名称数据。

因此扩展内存扫描并不能从该规则对象恢复名称。后续应建立来自 mod 文本源码或构建产物的符号侧表，键至少包含资源 TGI、规则 ID、来源包或内容摘要；同 ID 多候选须保留歧义。侧表名称是外部关联，不应显示成引擎直接输出的名称。此功能尚未实现。

## 已验证的全局定义字段

零售 `GlobalRule.Read=0x5FDD30` 与开发版 `cEcoBinSetRule::Read` 对照后，以下字段确定：

| 内存偏移 | 字段 | 说明 |
|---|---|---|
| +0 / +4 / +8 | ID / flags / simInfoOffset | simInfo=-1 表示没有该引用 |
| +12 / +16 | resources offset / count | 资源操作数组 |
| +20 / +24 | fromBinOps offset / count | 动态数量来源操作 |
| +28 / +32 / +36 | minApply / maxApply / applyResourceID | 基础应用次数约束 |

这些前十个字在 AEB 文件中为大端；数据池则由 RuleSet.Read 原样复制。`StoreForRule=0x5F69C0` 返回 ruleSet+4，池 begin/end 位于 ruleSet+4/+8。资源操作步长 12 字节，原始池按 x86 小端读取 `resourceID:uint32, input:int32, output:int32`。不能把整个 AEB 用同一字节序解读。

运行探针对每条目标全局日志采样，检查 StoreForRule 的 vtable、池边界和整个数组范围，最多输出 16 个操作并保留 count/truncated。新增读取失败进入 readErrors，不影响已有身份字段。离线工具严格校验整个 5/2/14 文件后，再解析相同字段及有界操作列表。

## input/output 的语义边界

零售 `BinRuleCount=0x606370` 表明：

- output>=0 时，input>0 会按现有 amount/input 限制次数；output>input 时，还按剩余容量/(output-input) 限制次数。
- output<0 时，测试 `input <= amount < -output`；这不是负产出。
- `CanRunRule=0x618660` / `RunRule=0x618890` 在 fromBinOps 存在时复制基础操作，再调用动态数量处理。基础定义并不总是最终使用的数量。

`0x606580` 对非负 output 的操作执行数量更新，并检查结果是否超出 0..capacity。现有 LogGlobalRule 在 RunRule 返回之后调用；人为直接调用 LogGlobalRule 会写入 delta 日志，却不等于执行规则或重新测量资源变化。因此本阶段没有把它包装成“手动触发规则日志”的按钮。适合后续实现的是按用户选定目标开启短时观察，或在明确线程及只读接口契约后生成单独标识的诊断快照。

## 实机与离线对照

15 秒窗口，PID 7424：59 条记录、读取错误 0、丢弃 0、入口命中 328，其中主动筛除 269。game=`0x2dc4dbf8`，stateIndex=315 → ruleIndex=368 → ruleID=`0xC2181F55`，规则集仍为 `0x2e722140`。

该 game 对象不同于上一轮 `0x2dc4f520`；上一轮 stateIndex=315 对应的是 ID `0x739D42FF`。不能跨 game 沿用状态索引。此次按实际 ID 重新查找离线条目，59/59 的全部新增字段相等：

```json
{
  "resources": {"offset": 631216, "count": 1,
    "values": [{"resourceId": "0xc9fb4c92", "input": 0, "output": 1}]},
  "fromBinOps": {"offset": -1, "count": 0},
  "minApply": 1, "maxApply": 1, "applyResourceId": "0x00000000"
}
```

这证明定义解码一致，不证明实时 amount 的前后变化。资源显示名仍未知。离线内容 SHA-256 沿用来源阶段导出的 AEB：`a388e3c15080a4721e7d0047da180f6ee9f38292060c67892fa34b952564c381`。九个相关函数区间在真实安装、备份及 dump 三样本中一致；指令、哈希、采集状态、首尾样本与对照结果见 [证据 JSON](evidence/rule-definition-fields-20261017.json)。

工作台新增 ruleDefinition 字段，可筛选及高亮资源 ID；中英文均提示它是基础定义而非实时资源数量。原始记录仍完整保留。单位规则操作、实时 bin 的 amount/capacity、执行前后差值及失败原因是下一步的独立验证范围。
