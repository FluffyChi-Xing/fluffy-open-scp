# 破洞 decal 的出现条件与资产编辑契约（2026-11-21）

本文是静态反编译证据，不声称观察到游戏运行。源码根为 `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/`；以下行号均相对此树。用户指定调试文件目录中的 `SimCity.c` 是 42 字节的失败占位，不能作为证据正文。

## 已确定：集合状态与 shader 家族是两条独立维度

`SC/cGraphicsUnitDecals/Update.c:37-58`（0x7D52E0）先令 `desiredDecalSetId=0`，然后：

1. `unit->mControlFlags & 0x400000` 非零，选 set 2，优先级最高。
2. 否则 extractor map 索引有效且 set 3 有记录时，读取单位位置的地图格；值大于 0，选 set 3。
3. 否则维持 set 0。

`SC/cGraphicsUnitDecals/SwitchToDecalSet.c:24-59`（0x7D5230）释放旧集合，set 0 创建 `mNormalDecalSet`，set 2 创建 `mVacantDecalSet`，set 3 创建 `mExtractorDecalSet`。它们互相替换，不是叠加。`cGraphicsUnitDecals.c:25-43` 明确各自 setId 为 0/2/3。

`SC/cGraphicsUnitDecals/FillFromProps.c:22-25`（0x7D51E0）从同一份 props 填这三个集合，并以 map ID `0x6FB277EF`（十进制 1873967087）取得 extractor map 索引。

因此“破洞材质”不自动意味着废弃时出现：决定出现时机的是该 decal 被写进哪个 set。破洞条目若作者放在 set 0，会走常规集合；放在 set 2 才受上述 vacant 标志门控。旧文档把 set 2 和破洞家族写在一起，不能反向理解为所有 hole shader 都由 set 2 专用。

## 已确定：属性列与创建前置条件

`SC/cDecalSetInfo/FillFromProps.c:28-49`（0x7D4F80）按下表读取，同一索引是一条 decal 记录：

| 列 | 属性 ID |
|---|---|
| decal Key 列 | `0x0D109050 + setId` |
| Transform | `0x0D109060 + setId` |
| Depth | `0x0D109070 + setId` |
| MaterialInfo Vector3 | `0x0D109080 + setId` |
| RenderGroup | `0x0D109090 + setId` |
| MachineSpec | `0x0D1090A0 + setId` |

关键前置条件：`SC/CreateGraphicsUnitDecals.c:19-23`（0x7D4EE0）只在存在 **normal Key 列 `0x0D109050`** 时创建整个组件。仅新增 vacant 列不能保证组件生成。此处只检验属性存在，没有检验 normal 列非空；空数组能否经过完整打包链保持属性存在，仍须资产侧验证。

`SC/cUnitModel/FillFromProps.c:175-208` 从 `unit->mNonSimProps` 创建组件、填表并以当前单位状态 Update。渲染组件没有直接查 bundle ID 或 vacant resource bin。

`SC/cDecalSetInfo/CreateDecals.c:48-75`（0x7D50A0）还有逐条画质门控：MachineSpec 缺失或 `machineSpecs[i] <= graphicsDecalLevel` 才创建；depth 缺省 0.1；materialInfo 缺省零。RenderGroup 特殊值 `0x96AF4B50` 强制 group 1。

## 已确定：Vandalism 是另一条局部 bin 链

`SC/CreateGraphicsUnitVandalism.c:19` 以 `0x0D109051` 存在为组件创建前提。`SC/cGraphicsUnitVandalism/cGraphicsUnitVandalism.c:26` 将集合 ID 设为 1。

`SC/cGraphicsUnitVandalism/FillFromProps.c:28-33`（0x7D54C0）对 resource ID `0xEC74175E`（有符号 -327936162）执行 `IndexFromResourceID`，再到 **unit 自己的 `mResourceBins`** 中 find。旧 `decal-engine-reanalysis-ida.md` §8 的“全局 bin”说法应以此处为准纠正。

`SC/cGraphicsUnitVandalism/Update.c:43-110`（0x7D5510）在 bin 索引有效、集合非空、capacity>0 时计算：

```text
enabledCount = int(float(amount) / float(capacity) * float(setCount))
```

数量变化后释放旧实例，按记录顺序创建前 enabledCount 条，仍有 MachineSpec 门控。它不按 decal raster、shader 家族或随机抽选决定顺序。反编译函数未见对 enabledCount 的 clamp，资产不应依赖 amount 超 capacity 时的行为。

## 已确定的规则连接与尚未确定的包级身份

`GB/cEcoUnit/RunRule.c:106-127` 通过 rule 的 `mControlFlags.mOffset` 从规则 store 取控制字段，并执行低/高 32 位的通用 clear/set：`new = set | old & ~clear`。这给出了规则改变 `0x400000` 的机制。`GB/cEcoUnit/FillFromProps.c:137-138` 还从 entry 初始化控制标志，`GB/cEcoUnit/Read.c:103` 从存档恢复控制标志。因此不能说所有 vacant 状态都必须在当次运行由同一个规则产生。

后续同日补查已找到具体包级实例，见下一节；仍未穷举每个置位规则的全部上游 bundle 和所有适用建筑。

## 补查：具体施工 → 空置 → decal 链

读取 `tmp/megatower-re-20261113/base.aeb` 与 `ep1.aeb`，按照已存在的 `tmp/school-dissection/parse_rules.py:272-350` 布局遍历记录和 pool，两个文件均精确消耗到 EOF。base pool 文件偏移为 4430172，ep1 为 851661。扫描本地 clear/set mask，base 有 39 条改写包含 `0x400000` 的记录，其中 15 条置位；ep1 有 8 条改写记录，其中 4 条置位。扫描结果保留在 `tmp/hole-vacant-base-scan.json` 和 `tmp/hole-vacant-ep1-scan.json`。扫描范围是这两个 AEB，不声称所有版本通用。

可以核对的一个完整具体例子：

1. bundle `40E0C700:7424049F` 的 `0x08080D58` 引用周期根 `427E0344`；原始属性转储在 `tmp/behavior-exemplars/rci/bundles/40e0c700-7424049F.txt:6`。同表含施工进度 bin `568282EB` 和施工车 bin `87AA975E`。
2. 该施工 bundle 的交付/施工推进链进入 `F1495C98`，该规则检查施工进度并以失败分支进入 `BA30F7B9`。既有规则解码见 `tmp/behavior-exemplars/rci/rci-rules.md:583`；本轮亦从原始 AEB 的 success/fail 字段复读。周期根 `427E0344` 自身的 success/fail 字段为零，不能把它写成直接调用 `F1495C98`；交付 sink `9654C8AB → 499DE95A` 也在该 bundle 中，施工交付与周期调度应分别保留。
3. `BA30F7B9` 的 base AEB 记录起点 **434381**，control offset **198976**。pool 中本地控制数据前四个 little-endian uint64 为 `0, 0, 0x40490000, 0x40480000`，即清除 mask `40490000` 后设置 `40480000`，其中包含 vacant 位 `0x400000`。本地资源操作为施工车 `87AA975E:1→0`、`ED901481:0→1`、`1CBF8231:1→-2`，nearby 施工车 `87AA975E:0→1`；失败后续 `B2D5AD38` 也设置 vacant 位。
4. 状态记账 bundle `40E0C700:182D2924` 的 `0x08327D52`（OnStateChange）列引用 `B4B7A614/11B97B74/7B8C1FF3/07A8466B`。原始读取记录为 `tmp/megatower-re-20261113/selected.json` 的 `lookup[182D2924]`。
5. `B4B7A614` 的 base AEB 起点 **478145**、control offset **215424**，本地 testMask/testValues 都为 `0x400000`。它增加 vacant bin **F168C300**，清除 UnderConstruction **EBACEF66**、Normal **4527C498**、Abandoned **2BF942CF** 和废弃计数相关 bin；`fromBinOps` 决定实际清除量，不能把编译常量 999999 当作实际拥有的量。
6. 渲染 `cGraphicsUnitDecals::Update` 直接读同一个 vacant 位选择 set 2；它不先读 F168C300。这里是 **规则置位 → 状态事件同步 bin，渲染读位**，并非 vacant bin 直接打开 decal。

需明确纠正术语：**Vacant（空置，bit 22=`0x400000`）与 Abandoned（废弃，bit 11=`0x800`）不同**。同一 `182D2924` 中 `7B8C1FF3` 测试 `0x800` 并写 Abandoned bin，而 `B4B7A614` 测试 vacant 位并写 Vacant bin。旧文档的“Vacant（废弃/破洞族）”不够准确。破洞若位于 set 2，已证明其直接可见条件是空置标志，不能泛化为建筑经营废弃的全部表现。

尚未确定：各建筑实际引用的破洞条目分布及空置/废弃 Graphics 变体的完整交互；当前例子证明至少存在一条施工转空置的真实规则链，不代表所有破洞都由该施工 bundle 创建。

## 编辑器可直接采用的契约

- 分别保存 shader 家族、setId 和集合内顺序；材质选择与出现状态不能合并成一个枚举。
- 常规/废弃/采集预览按引擎优先级切换集合，Vandalism 作为独立局部 bin 控制。
- 创建 vacant/extractor 支持时保留 normal Key 属性存在的前置契约，并验证列长度一致、transform 有效及画质阈值。
- 不向作者承诺“挂某个 bundle 就有破洞”：可见内容仍须具备集合记录、字典/材质/raster链和上述组件创建前提。
- 以上只验证静态逻辑和数据契约；游戏的冷启动、实际废弃状态与最终视觉效果仍须用户验收。
