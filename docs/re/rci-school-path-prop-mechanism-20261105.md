# RCI 建筑、功能 Unit、菜单文案、Path 与车辆 Prop Bin 机制证据

日期：2026-11-05

本报告只使用三类证据：开发版反编译源码、安装版 `SimCity_Game.package` / `SimCity_Graphics.package` 的实际 Property、以及当前 s17 包的逐字段转储。它不把“游戏里没有显示”反推成某个字段缺失；每个结论都列出实际消费者或可复核的 TGI。

## 结论先行

| 问题 | 已闭合的事实 | 责任边界 |
|---|---|---|
| 建筑成为某类功能建筑 | `Menu2.0x0975695F(menuUnit)` 先定位 `40E0C000:<unit>`；Unit 的 `0x00B2CCCB` parent、`0x0AFE27B1` category、bundle 列表和规则索引决定模拟行为。菜单 `uiCategories(0x0DB9FC63)` 只决定可见入口。 | 菜单位置与功能可以解耦；缺 Unit/template/bundle 闭包才会失去功能。 |
| 菜单乱码 | Text 的语义是 `(table_id, instance_id)`。legacy builder 写入 `table=BF82F7CE,title id=assetInstance,description id=assetInstance+1`，但同路径 locale 只生成 `id=1/2`，查找必然 miss。parent-free 路径已改为 `table=assetInstance,id=1/2`。 | 这是构建器可静态定案的确定性错误；如果包已走 parent-free 仍显示 `***`，再查包是否被游戏加载以及缓存代际。 |
| Path | `CreatePathsFromProps` 必须同时得到 ranges、points、tangents、point indices、PathEntry、scope/connect；PathEntry instance 还必须由 `IndexFromPathEntryID` 解析且 scope 被接受。 | 只有点和切线不构成运行时路径。 |
| “spawner” | `0x0E1BAC61/62` 在 `cUnitModel::FillFromProps` 中写入 `mUnitLocations`（名称 hash→transform），是图形/装配锚点，不是生成 Agent 的模拟 spawner。 | Agent 生成来自 UnitRule/TransportOps；锚点只提供位置。 |
| 车辆 prop bin | `0x0C12EF20 + set` 的每个 key 必须先经脚本 `IndexFromResourceID`，再命中 Unit 的 `mResourceBins`；之后还要有对应 transforms、slots、可选 percent-fill，以及 `resourceID→graphics` 映射。 | 直接把 model TGI 写进 bin 列不会显示车辆；那不是脚本 ResourceID。 |

## 1. 一个建筑如何获得特定功能

原版小学的闭合链是：

```text
Menu2 00B1B104:09878A01:29873FA2
  └─ 0x0975695F menuUnit = C264A268
      ├─ Unit 00B1B104:40E0C000:C264A268
      │   ├─ 0x00B2CCCB → 48E0C000:7E4B99E5 (教育行为模板)
      │   ├─ 0x0AFE27B1 → 0x95A9D6FD (教育类别)
      │   ├─ 0x0A0209A5 → 6C042367, 3A1C3971, 95573B5D, 9A1BFB2F
      │   ├─ 0x0A0209B2 → F90C8A94 (excluded bundle/resource container)
      │   └─ 0x0AD7FAC9 → 学生/课桌容量
      └─ Lot 00B1B104:40E1C000:C264A268
          └─ 0x00B2CCCB → 40E1C000:7E4B99E5
```

开发版 `GB/cEcoUnitEntry/FillFromProps.c` 先读 `0x099B4C1F` 的 set，再把 `0x0A0209A5` 的四个 bundle 逐个送入 `AddBundle`；`0x0A0209B2` 被读入 excluded list。`GB/cEcoUnitEntry/AddBundle.c` 对每个 bundle 读取 `0x08080D58 + 0..2` 的 RuleID 数组，并调用 `IndexFromUnitRuleID`；随后读取 `0x08327D48 + eventIndex` 的 21 个事件槽。也就是说，水电、学生、交通和服务规则均在 Unit → parent/template → bundle → UnitRule 链上。

因此 s17 的“公园菜单”本身不是功能原因。s17 转储显示：

```text
Menu2 00B1B104:09878A01:7FA8E661
  0x0DB9FC63 = 07049250       (公园菜单类别)
  0x0975695F = 7FA8E661       (menuUnit)
Unit  00B1B104:40E0C000:7FA8E661
  0x00B2CCCB = 48E0C000:7E4B99E5
  0x0AFE27B1 = 95A9D6FD
  0x0A0209A5 = 6C042367,3A1C3971,95573B5D,9A1BFB2F
  0x0AD7FAC9 = 250
```

这组字段已经满足“教育 Unit”的静态身份。若实机仍把它当公园，失败点就不是 `uiCategories`，而是 Menu2→Unit 解析、Unit 资源优先级/代际缓存、或 bundle/rule 资源闭包没有在运行时获胜；应在这三个消费者处记录失败，而不是继续对整包做字节对拍。

## 2. 菜单乱码的确定性根因

### 2.1 legacy 路径的引用与表内容不一致

`src-tauri/src/asset_build_m1/menu.rs` 的 `build_menu2`（约 895–999 行）写入：

```text
title Text = (table_id = 0xBF82F7CE, instance_id = assetInstance)
description Text = (table_id = 0xBF82F7CE, instance_id = assetInstance + 1)
```

同文件 `build_locale`（约 1040–1064 行）却只生成：

```json
{
  "0x00000001": "标题",
  "0x00000002": "描述"
}
```

因此 legacy 路径实际查找 `(BF82F7CE, assetInstance)` / `(BF82F7CE, assetInstance + 1)`，而产物只有 `(BF82F7CE, 1/2)` 或者把资源实例写成 asset instance 后查找表 ID 也不一致；标题必然解析不到。这不是编码猜测，是两个函数的输入/输出契约不相容。

### 2.2 parent-free 路径的正确形状

`generate_parent_free` 通过 `apply_menu_overrides`（约 1101–1102 行）把引用改成：

```text
title/description Text = (table_id = assetInstance, instance_id = 1/2)
locale resource       = 0A98EAF0:02FABF01:assetInstance
```

当前 s17 的实际资源 `0A98EAF0:02FABF01:7FA8E661` 内容为：

```json
{
    "0x00000001" : "小学性质建筑测试包 s17",
    "0x00000002" : "小学性质建筑测试：学生容量 250，保留车辆 prop、path 与 spawner；菜单入口仍在公园"
}
```

这三项（Text table、Text id、locale resource instance）已经相互一致。若该 s17 仍在游戏中显示 `***`，下一步应记录实际加载的数据库/TGI，而不是再改 JSON：`docs/re/package-directory-chain-evidence.md` 已给出 retail LocaleManager 的显式注册阶段和普通 Packages 的优先级；同时也要关闭并重新打开游戏/资源管理器，避免旧 package handle 继续提供旧代际。

本轮已把 legacy `build_menu2` 同样改成 locale 行 1/2，并把现有回归断言改为检查 `(BF82F7CE,1/2)`；因此新包不会再生成这类悬空 Text 引用。

### 2.3 不能把占位符误叫成 locale 损坏

统计行中的 `~population_amount~`、`~maintenance_cost~` 是 `unitEffectTitle` 模板与 `unitEffectParameter(0x0EB1FC22)` 的绑定失败。它们和标题 `***` 是两条不同链：前者是 Menu2→Unit→查询字段为空，后者是 Text→locale 查找失败。诊断日志必须分别记录。

## 3. Graphics RCI 普查：Path/Location/Bin 的阳性样本

探针 `probes/property/rci_building_census.rs` 对安装版 `SimCity_Graphics.package` 和 `SimCity_Game.package` 的 `40E0C000` Unit 做了只读普查，结果保存在 `tmp/rci-building-census-20261105.txt`：

| 数据源 | R/C/I Unit 行 | C | I | R |
|---|---:|---:|---:|---:|
| SimCity_Graphics.package | 834 | 352 | 219 | 266 |
| SimCity_Game.package（教程变体） | 3 | 1 | 0 | 2 |

Graphics 包中每个普通建筑都有 A/N/R 三个表现变体，各 278 行。字段分布如下：

| 变体 | 行数 | 有 Path points | 有 Path entries | 有完整 UnitLocation IDs/transforms | 有 vehicle-bin base 列 |
|---|---:|---:|---:|---:|---:|
| `_A` | 278 | 0 | 0 | 22 | 0 |
| `_N` | 278 | 276 | 276 | 278 | 226 |
| `_R` | 278 | 0 | 0 | 0 | 245 |

典型阳性样本 `C_Gen_W1D1_Shops_01_01_N`：4 个模型、3 个 bin、4 个 path、2 个 entry、13/13 个 UnitLocation；`I_Gen_W1D1_Warehouse_04_02_N`：4/3/11/5/13；`R_Gen_W3D3_highRise_01_01_N`：4/3/16/5/13。这个结果说明普通 RCI 的运行时几何/路径/锚点主要在 `_N` 变体；把 `_A` 或 `_R` 当作“完整建筑定义”会主动丢掉 path/location，单看模型数量无法判断功能完整性。

## 4. Path 的确切消费条件

文件 `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\CreatePathsFromProps.c` 的运行时顺序是：

1. 读取 `0x0CAA6841` path ranges。
2. 读取 `0x0CAA680D` points、`0x0CB00ED8` tangents、`0x0CAA6832` point indices；points 数量必须非零且与 tangents 相等。
3. 读取 PathEntry table（`0x0CC8FE7A` 是旧式 key-array 回退），每条 entry 的 instance 调 `gameScripts->IndexFromPathEntryID`。
4. 读取可选 `scope` 和 `roadConnect`；scope 交给 `AcceptedScope`。
5. 只有 entry 能解析且 scope 被接受时，才调用 `cUnitPathCreationObj::CreatePath`，随后更新 tangent。

因此“有点但没有 path”或“有 path entry 但脚本表没有该 instance”都会静默落在不建网的分支。构建器应该在 `validate-paths` 阶段记录每一段的 source TGI、entry instance、script index、scope、connect 和 CreatePath 结果。

## 5. UnitLocation 与真正的 spawner

`SC/cUnitModel/FillFromProps.c` 的 `0x0E1BAC61` 分支读取 UnitLocation 名称 key，`0x0E1BAC62` 分支读取对应 transform，并写入 `this->mUnitLocations`。同一函数还把 lights、decals、vandalism、construction、windmill 和 bundle 都从 `mNonSimProps` 创建出来；没有调用 Agent 创建器。该表是“命名锚点”，可供相机、vignette、模型装配、停车/入口逻辑查找。

学生、教师、校车 Agent 的生成和去向属于 UnitRule 的 AgentTests/TransportOps/HandlerID 及路径网络。一个 UnitLocation 可以被多个 Agent 或同一服务流程复用，数组长度不能当作 spawner 数量。

## 6. 车辆 Prop Bin 的确切消费条件

文件 `SC/cUnitBinDraw/FillFromProps.c`：

- 对 `set=0..14` 读取 `0x0C12EF20 + set` 的 ResourceID 数组。
- 每个 instance 先调用脚本 `IndexFromResourceID`；失败或在 Unit `mResourceBins` 找不到对应 bin 时，不加入 `mResources`。
- 只有资源映射非空后才读取 transforms（`0x0C12EF30 + set`）和 slots（`0x0C12EF40 + set` 族）。
- `0x0C12EF50 + set` 控制槽随机化，`0x0C12EF60 + set` 控制按 amount/capacity 的 percent-fill。

`SC/cUnitBinDraw/Update.c` 每帧按 Unit resource bin 的 amount/capacity 计算可见槽数量，再以 `mUnitBinDrawResources.mResourceIDToGraphics[resourceID]` 查到 graphics resource，最后才实例化模型。因此正确的 prop bin 闭包是：

```text
Unit ResourceID
  → GameScripts::IndexFromResourceID
  → Unit.mResourceBins
  → BinDraw transforms/slots
  → resourceID→graphics 映射
  → 实例化模型
```

“model TGI 存在”只证明最后一层资源可能存在，不能替代前三层。此前把 `modelTgi.instance` 直接写到 `0x0C12EF20` 的事故，正是跳过 `IndexFromResourceID` 的写入端守卫缺失。

## 7. 对 s15/s17 的直接判定与构建器要求

### 已能从包静态判定的部分

- s17 Unit 的教育 parent/category、四个 bundle、250 容量和 lot link 都存在。
- s17 Lot 保留了 path points、path ranges、PathEntry、UnitLocation、lights 和 bin transforms；资源引用数量与原版小学形状一致。
- s17 locale 字节有 BOM、CRLF、可解析 JSON，键 1/2 存在。
- s17 Menu2 的公园 category 与教育 Unit category 是两个字段，解耦成立。

### 不能靠静态包直接判定的部分

- 当前运行时究竟选中了哪个同 TGI database/哪一代 package handle。
- `IndexFromPathEntryID` 是否在安装版脚本表中返回有效索引。
- `IndexFromResourceID` 是否能识别每个车辆 bin resource ID，以及 Unit 当前 amount 是否大于零。
- 具体小学规则是否命中校车/学生 Agent；这需要在 `LogUnitRule`、`TransportOps` 和 `ApplyBinRule` 处按 UnitEntry 过滤。

### 构建器必须改成 fail-closed 的检查

1. **Menu 文案**：构建后解析所有 Menu2 Text 引用，检查 `(table,id)` 在同一输出闭包中确实存在；legacy 路径禁止生成 `table=BF82F7CE,id=assetInstance` 这种悬空引用。
2. **功能 Unit**：强制报告 `menuUnit→Unit→parent→category→bundles→rule index`，缺任何一段都拒绝发布；category 与 menu category 分开记录。
3. **Path**：逐段报告 `range/point/tangent/index/entry/scriptIndex/scope/connect`，不能只报告“有 path”。
4. **UnitLocation**：把它记录为 `locations`/`anchors`，禁止命名为 spawner；若 schema 声明了 Agent spawner，必须另有 AgentRule/TransportOps 证据。
5. **Prop bin**：逐 set 报告 ResourceID、脚本索引、Unit bin 索引、transform/slot 数和 graphics 映射；任何 `IndexFromResourceID=-1` 都在 `scene-semantics` 阶段失败。
6. **与 observability 设计对齐**：这些字段进入 `docs/design/build-pipeline-observability-20261105.md` 规定的 step ledger，并在基线漂移中按责任处理器归因；不再通过整包字节对拍寻找责任。

## 证据文件

- `tmp/rci-building-census-20261105.txt`
- `tmp/s17-package-dump.txt`
- `tmp/s17-locale.bin`
- `docs/re/elementary-school-mechanism-20261022.md`
- `docs/re/package-directory-chain-evidence.md`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\CreatePathsFromProps.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\cUnitModel\FillFromProps.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\cUnitBinDraw\FillFromProps.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SC\cUnitBinDraw\Update.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\GB\cEcoUnitEntry\FillFromProps.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\GB\cEcoUnitEntry\AddBundle.c`
