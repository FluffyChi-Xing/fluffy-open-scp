# 树回归分析报告：为什么 s18-s21 的树消失了（2026-11-06）

结论边界：本文只使用转储、反编译源码与用户实机证词；"单元死亡"的最终根因
（创建后第一批 tick 的规则失败链）仍待规则表（tmp/school-dissection/）与
运行时捕获定案，本文给出的是树这一症状的完整机制链与证据等级。

## 0. 结论速览

1. **树不是"丢失的列"，是活单元的资源 bin 内容**。树 = prop-bin（`0x0C12EF2C`
   资源族 `14984C68-6B`，容量各 10——对应"10 棵树"）的 percent-fill 渲染：
   可见数量 = 单元资源 bin 的 amount/capacity（`cUnitBinDraw::Update` 每帧计算）。
2. **bin 的定义来自 Toy 基层继承的 bundle `E7690450`**（`0x07E0AD23` 资源键、
   `0x07E0AD24` 容量 10×4、填充规则 `0x08080D58`→`26D4942C/2D/2E/2F`），不在叶子
   Unit 的 bundle 列表里——树是模板继承给的，不是叶子声明的。
3. **s18-s21 树消失 = 单元在放置后死亡**（`FailUnit` 级联）：bin amount 清零 →
   percent-fill = 0 → 0 棵树。同一根因同时解释：车库（次级单元）消失、无水电、
   夜灯不亮、面板占位符——它们都是"单元死亡"的不同可见面。
4. **s14 树正常**：它的单元是极简公园族（自带 bundle 对、费用科目、set 绑定），
   规则正常运行，bin 有量。s14 的 Asset 甚至没有 prop 列——树不依赖我此前
   猜测的"props 列"。
5. 因此**不存在独立的"树回归"**——任何把树渲染回来的尝试，若不先解决
   "单元为何死亡"，都只是表面修补。

## 1. 机制链（每一环的证据）

```text
Toy 基类 F98BCB35 (0x0A0209A3 候选 bundle 组，含 E7690450)
  → bundle E7690450：0x07E0AD23=[14984C6B,6A,69,68] 0x07E0AD24=[10,10,10,10]
                     填充规则 0x08080D58=[26D4942E,2F,2C,2D]
  → 单元特化（GB::FillFromProps）：声明 bin（资源 14984C68，容量 10）
  → 填充规则运行：bin.amount 上升（10 = 满配树）
  → Lot 渲染端 cUnitBinDraw::FillFromProps：
      0x0C12EF2C(set3)=[14984C68, C8B4254A] --IndexFromResourceID--> mResourceBins
      0x0C12EF3C(set3)=10 个 transform；0x0C12EF4C=10 个 slot
  → cUnitBinDraw::Update：可见槽数 = f(amount/capacity)（0x0C12EF60=true）
  → 10 棵树
```

证据：
- bundle E7690450 转储：`tmp` 查询 `SimCity-Scripts_287520926 00B1B104:40E0C700:E7690450`
  （本文写作时实查，193B，字段如上）。
- BinDraw 消费链：`SC/cUnitBinDraw/{FillFromProps,Update}.c`（反编译）；
  "IndexFromResourceID 失败或 mResourceBins 无对应 bin → 不加入 mResources"。
- percent-fill：`0x0C12EF60=true` 在小学 Lot 与 s17-s21 Lot 均存在。
- 单元死亡级联：s18-s20 三轮实机读数（车库消失/无水电/面板占位符/
  人口地图废弃色）+ `SC/IsUnitAllowedToBeDeleted.c`、`GB/cEcoGame/FailUnit.c`
  的规则失败链机制。

## 2. 时间线与根因（2026-11-06 用户聊天记录修正终版）

**树从 s15 起丢失**，根因是 s15 的一次有意修改：

> s14 把 tested-unit.bin 的公园字段物化到教育 Unit，覆盖了教育 bundle，
> 因此 path/spawner/车辆都不会被消费。已修改 native_profile.rs：
> simulation asset 的 Unit parent 改为 48E0C000（40E0C000 是具体 Unit 叶子组，
> 48E0C000 才是 cUnit 行为模板组）；simulation Unit 不再继承公园
> tested-unit.bin 字段，改为最小叶子，让教育模板提供行为 bundle。

即：**s14 的树来自 tested-unit.bin 的公园字段（公园链激活了树 bin）**；
s15 为修"公园字段覆盖教育 bundle"而剥离公园字段时，**树 bin 的激活随公园链
一起被剥掉了**。教育模板链不激活树 bundle E7690450——这就是 s15-s21 全部
无树的机制。

| 时期 | 包 | 树 | 解释 |
|---|---|---|---|
| s14 | 024f8631（GOLDEN） | 有 | 公园字段物化在叶子上，公园链激活树 bin（代价：覆盖教育 bundle） |
| s15-s17 | 685341b8…/36ee72ee…/917d3ddc | 无 | 剥离公园字段修教育覆盖，树 bin 激活随之消失 |
| s18-s21 | 本轮四包 | 无 | 同 s15 形状 + 本轮失败链 |

注：本文初版把一张早期截图误归为"s17 有树"，以用户聊天记录为准修正。

## 2b. 外科手术级修树方案（新发现的直接推论）

树 bundle **E7690450 是共享原版 bundle**（40E0C700 组，脚本库内）：
`0x07E0AD23=[14984C6B,6A,69,68]`、`0x07E0AD24=[10,10,10,10]`、填充规则
`26D4942C/2D/2E/2F`。而 **s17-s21 的 Lot 已经携带完整的 BinDraw 渲染端**
（`0x0C12EF2C=[14984C68,C8B4254A]`、10 transforms、slots、percent-fill=true
——见 tmp/s17-package-dump.txt）。

所以缺的只有一环：**把 E7690450 加进叶子 Unit 的 bundle 引用
（`0x0A0209A5` 数组追加一项即可）**——不继承任何公园字段、不覆盖教育
bundle、只激活树 bin。这正是"行为绑定"架构的最小实例（§4 的
tree-decoration 行为闭环）。

修复方向（按依赖顺序）：
1. 叶子 `0x0A0209A5` 追加 `E7690450`（树 bin 激活）——单一变更，可独立验收。
2. 根因（新放置单元第一批 tick 死亡）待规则表 + 运行时捕获定案。
3. 在此之前不做其他"补树"操作。

## 3. 两个"10 棵树"的辨析（容易混淆的两层）

- **bin 树**（本文主链）：资源 `14984C68`，容量 10，percent-fill——小学本体
  与 s14 的树。
- **场景 prop 树**：工作台场景里手摆的 10 个 prop（模型 `2F4E681B:C2FBD178`
  + `315F77E8`，均在 SimCity_Graphics 基础包内，游戏可自行解析）。s18-s21
  的构建台账每轮都报告 `closureManifest.missing: [prop:…315F77E8, prop:…C2FBD178]`
  （只影响 delta 是否携带，基础包可解析故非致命）。这层在坏单元下同样
  不可见（lot 装配挂在单元的 mNonSimProps 上，`FindUnitMasterNonSimProps`
  的回退链在单元死亡时退化）。

## 4. 修复方向（按依赖顺序）

1. **树 bin 激活**（§2b，单一变更可独立验收）：叶子 `0x0A0209A5` 追加共享
   原版 bundle `E7690450`。
2. **根因**：定位"新放置单元第一批 tick 死亡"的失败规则（规则表
   tmp/school-dissection/rule-table.json + 定向运行时捕获）。
3. 根因解决后，车库/水电/面板占位符应同时恢复（同一根因的其余症状）。
4. 在此之前任何其他"补树"操作（加 prop 列、改 bin 列）都是表面修补，不做。

## 5. 遗留

- 填充规则 `26D4942C-2F` 的触发条件与失败原因：待规则表 + 运行时捕获。
- `C8B4254A` 在脚本库 raw 扫描零命中（IndexFromResourceID 可能失败）——
  set3 的第二资源可能从未渲染，只渲染 14984C68；对"10 棵树"的数量语义
  无影响（容量 10 在 14984C68）。
