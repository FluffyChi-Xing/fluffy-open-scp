# Prop bin 动态状态与渲染分流（对拍与设计）

日期：2026-10-31。问题：树/车/垃圾桶/废墟/待售牌/工厂产品/仓库货物能否按语义单独渲染；
当前管线缺什么。

## 1. 引擎侧：bin 是“资源绘制组”，不是物体类型

来源：`docs/source-code/SimCity-dev-ida.c`（dev 反编译）

`SC::cUnitBinDraw::Update`（:956948）：

```
bin = unit->mResourceBins[binIndex]        // 每个 BinDrawSet 绑定一个 EcoResourceBin
amount = bin.mAmount
if (set.mUseBinPercent)                    // lot 列 0x0C12EF60
    amount = ceil(bin.mAmount / bin.mCapacity * slotCount)
// 只让前 amount 个 mResourceSlots 可见，其余清空
```

`SC::cUnitBinDrawResources::FillFromProps`（:957518）用 **脚本资源表**
（`scripts->NumResources`）为每个 `resourceID` 解析 `mGfxId`，再按
`mResourceSlots` 逐槽实例化。因此：

- **bin 编码“画几个”**（数量/百分比），不编码“是什么”；
- **物体语义在 prop 的脚本资源上**（`cGraphicsResource::FillFromProps` 分支，
  已在 `prop-semantic-renderer-decomposition-20261031.md` 列出）；
- 静态树类 prop 不绑资源 bin → 全显；垃圾桶/煤堆/原油/货物等绑资源 bin →
  随 amount 显隐。

## 2. 各类动态对象的真实来源

| 现象 | 游戏机制 | 证据 |
| --- | --- | --- |
| 树 | 静态 prop（不绑资源），或 tree/tree_model 脚本资源 | 本文 §1、prop-semantic 文档 |
| 车辆（停车位随入住出现） | prop bin 绑定 resident/vehicle 资源，`amount` 决定槽位数 | `building-animation-runtime-design` :224 `car slot 3: visible because bin[residentVehicle]=7/10` |
| 垃圾桶 | `GarbageDisplayCount` 资源 bin（`0x54CA89F0`）驱动；清运后 amount 回 0 | `school-property-er2-runtime-model` :161-171、`garbage-observation` |
| 工厂产品/仓库货物 | 产品/货物资源 bin 的 amount，决定 BinDrawSet 槽位数 | 同上；`building-animation-runtime-design` :12 |
| 废墟 | **建筑 shader/model 变体**：`deformRubbleVS` / `setAbandoned`，由建筑状态选择，不是 prop | `n8-abandoned-deform.md`、`cShaderDataAbandonedBuilding` |
| 待售牌 | 未在 PDB 字符串中找到独立符号；按现有证据更像建筑状态 overlay（decal/effect）而非 prop bin | 待补证 |

结论：**“prop bin 语义”只能解决前四类中的数量/显隐；废墟是建筑状态，待售牌
大概率也是状态层**。不能把 14 个 bin 数字硬编码成物体类型。

## 3. 当前渲染管线能做什么

现有 `src/render/plan/units.ts` 只按 `ResolvedPropModel.source` 分流：

- `tree/tree_model/descriptor` → 树处理器
- `vehicle_models` → 车辆材质链（但仍在 props 组内）
- 其余 → static 模型

**不能**：

1. 按 bin 的 `mAmount/mCapacity/mUseBinPercent` 显隐槽位——DTO 没有
   `percentFill/randomize`，也没有运行时资源量；查看器把所有槽位全画。
2. 区分垃圾桶/货物/产品——它们与普通 static prop 同走 `static`，语义只存在
   资源名/脚本类里，未进入 plan。
3. 废墟——没有建筑状态通道，模型永远按正常建筑渲染；`deformRubbleVS`
   未接入。
4. 待售牌——没有状态/overlay 通道。

## 4. 需要补的两层数据

### 4.1 Prop bin 元数据（静态可解析）

`crates/sc-properties/src/lot_unit.rs` 已读 `0x0C12EF40/50/60` 但只放进
`fields`，未进入 DTO。应补：

```
PropUnit {
  bin, index, resourceId, transform, slot, scale,
  randomizeSlot: bool,     // 0x0C12EF50
  percentFill: bool,       // 0x0C12EF60
}
```

这些足以让 plan 知道“这个 bin 是百分比填充”，但仍缺 `amount/capacity`。

### 4.2 状态通道（运行时/预览）

```
BuildingState = normal | abandoned | rubble | forSale
PropBinState  = { bin, amount, capacity, resourceId? }
```

来源只能是：
- 游戏存档/运行时（EcoGame resource bins）——查看器需要额外数据源；
- 或编辑器“状态预览”UI（手动设 amount/percent）——离线可对拍静态布局。

## 5. 目标处理器拆分

```
plan/propState.ts     BuildingState + bin amounts → 每个 slot 的 visible
plan/propSemantic.ts  script resource class/source → tree|vehicle|trash|sign|product|static
exec/three/props/     tree / vehicle / static / trash / sign / product 六个执行器
exec/three/buildingState.ts  normal / abandoned(deform) / rubble / forSale overlay
```

分发键优先级：**显式场景语义 > 脚本类名 > source 分支 > 资源名回退**，禁止
贴图 alpha 启发式（2026-10-31 车辆裁穿教训）。

## 6. 下一步建议（需要用户确认）

1. 先补 4.1 的 `randomize/percentFill` DTO，并在 plan 里保留 bin 元数据；
2. 若要“动态状态”对拍，需提供 **游戏存档/运行时 resource bin 快照**，或接受
   编辑器手动状态预览；
3. 废墟/待售牌先按 **building state** 立项，不要塞进 prop bin；
4. 工厂产品/仓库货物按 **resource bin amount** 做槽位显隐，模型语义仍由脚本
   资源解析。

在拿到状态数据前，查看器全显是“静态布局正确、动态数量不正确”，不能声称
已经能区分这些 prop bin。
