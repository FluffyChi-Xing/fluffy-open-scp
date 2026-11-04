# s11 车辆 prop 与 UnitLocation 未被消费

## 结论

s11 的 `Asset` 属性确实包含 `0x0C12EF20` 车辆资源列和
`0x0E1BAC61/62` UnitLocation 列，但与原版功能资产有两个关键差异：

```text
0x0AFE27B1 = 0x07049250
0x099B4C1F = 0x5FFA86B9
```

1. parent-free flatten 丢掉了 Asset/Unit 的 cUnit 功能 parent。原版小学使用
   `40E1C000:7E4B99E5` + `40E0C000:7E4B99E5`，热气球公园使用
   `40E1C000:34CE7E17`；菜单类别不决定这条继承链。
2. s11 车辆 bin 只有 `0AB4DFE1` 且 `0x0C12EF60=false`；原版小学和热气球
   公园都写入完整 `0AB4DFE1/E2/E3` 车辆族，并将 `mUseBinPercent` 设为 true。

因此车辆 bin 没有按原版方式解析，UnitLocation 也没有通过功能 parent 接入
教育/交通规则。静态列存在不能证明运行时消费链闭合。

原版学校 Unit 的类别是 `0x95A9D6FD`，并且 Unit 侧还带教育 bundle/规则引用；
仅把 Asset 的 spawner 列或车辆列补齐不能替代这些 Unit 语义。

## 修正

大学测试 schema 应保持公园菜单，但显式指定小学功能 parent：

```json
{
  "categoryInstance": "0x07049250",
  "menuParentInstance": "0xF970A822",
  "simulationTemplateInstance": "0x7E4B99E5"
}
```

构建器现在在 ledger evidence 中记录行为类别、行为 parent、是否存在
path/vehicle/spawner；车辆 bin 自动补齐 E1/E2/E3 并启用 `mUseBinPercent`，避免把
“列已写入”误报为“运行时会消费”。

## 证据

- s11 Unit dump：`tmp/s11-unit-dump.txt`
- 原版学校 Unit：`tmp/school-rule-schedule-20261023/property-40e0c*.txt`
- 教育类别与 Unit 关系：`docs/re/elementary-school-mechanism-20261022.md`
- prop bin 动态消费：`docs/re/prop-bin-state-rendering-20261031.md`
