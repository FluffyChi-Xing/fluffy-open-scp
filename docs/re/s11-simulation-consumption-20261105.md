# s11 车辆 prop 与 UnitLocation 未被消费

## 结论

s11 的 `Asset` 属性确实包含 `0x0C12EF20` 车辆资源列和
`0x0E1BAC61/62` UnitLocation 列，但 Unit 被构建成了公园/诊断类别：

```text
0x0AFE27B1 = 0x07049250
0x099B4C1F = 0x5FFA86B9
```

`0x07049250` 是公园工具类别。它只足以让自定义模型出现在菜单和场景中，
不会挂载大学的教育、交通和 ER2 规则。因此车辆 bin 没有 resident/vehicle
resource-bin 数量可供 `cUnitBinDraw::Update` 消费，UnitLocation 也没有教育
transport transaction 的引用方。静态字节对拍无法证明运行时消费链闭合。

原版学校 Unit 的类别是 `0x95A9D6FD`，并且 Unit 侧还带教育 bundle/规则引用；
仅把 Asset 的 spawner 列或车辆列补齐不能替代这些 Unit 语义。

## 修正

大学测试 schema 应使用：

```json
{
  "categoryInstance": "0x95A9D6FD",
  "menuParentInstance": "0x9D2EF585"
}
```

构建器现在在 ledger evidence 中记录 `behavior.categoryInstance`、是否存在
path/vehicle/spawner，以及该类别是否为教育 simulation template，避免把“列已写入”
误报为“运行时会消费”。后续若要让自定义大学真正参与教育模拟，还需要选择并保留
原版教育 Unit 的 bundle/ER2 引用；公园模板不能作为行为模板。

## 证据

- s11 Unit dump：`tmp/s11-unit-dump.txt`
- 原版学校 Unit：`tmp/school-rule-schedule-20261023/property-40e0c*.txt`
- 教育类别与 Unit 关系：`docs/re/elementary-school-mechanism-20261022.md`
- prop bin 动态消费：`docs/re/prop-bin-state-rendering-20261031.md`
