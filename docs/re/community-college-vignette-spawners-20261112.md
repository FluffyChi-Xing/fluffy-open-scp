# 社区大学的人群演出与位置锚点

2026-11-12。对拍范围是零售包 Property、现有反编译函数和工作台构建路径。游戏截图用于定位屋顶和地面的人群，本次未在游戏中部署或逐组重放。

## 原版数据

零售 `SimCity_Game.package` 的社区大学 Lot 为 `00B1B104:40E1C000:A826FA3E`。该 Property 自带 13 行 `0E1BAC61`（位置名 Key）和 `0E1BAC62`（Transform），原版每行的 Transform flags 为 14。Lot 还以三列并排登记两条演出：

| 演出 ID `0D2E8A55` | 定义 Key `0D5A28B8` 的 instance | Sink 选择 `0D50908D` | 定义里的演员与位置 |
| --- | --- | --- | --- |
| `BB4239F8` | `00F0F854` | `2CA33BDB` | 32 条演员记录。`startPoint` 全指向 `980F070E`，局部坐标约 `(-0.51, 7.11, 2.00)`。资源绑定覆盖 `WorkerW1/W2/W3`。 |
| `CC337468` | `ACE25EE3` | `2CA33BDB` | 84 条演员记录，资源绑定指向 `Student`（`2C1DA112`）。`startPoint` 分别引用 `141EED53`（40）、`3B8ADFE0`（11）、`3B8ADFE3`（33）。 |

两份定义都在同一个包的 `00B1B104:40E1E800` 组。演出 ID 用于建筑内演出查找，定义 Key 的 instance 用于解析组里的定义。旧报告以 `0D2E8A55` 对比组内资源 instance，漏掉了实际的引用列 `0D5A28B8`。

`2CA33BDB` 是无专用 sink 的哨兵值。 `cVignette::FillFromProps` 遇到此值会将演出 sink 索引设为 `-1`，与省略该列时相同。学生、劳工等资源绑定仍由演出定义提供。

学生定义另有 40 行区域锚点 `0D36410D`，其中 20 行使用地面 `3B8ADFE3`，另 20 行使用屋顶 `3B8ADFE5`。后者的局部坐标为 `(-0.364, -20.608, 23.124)`，第三轴为高度。该屋顶名也出现在定向动作的位置空间列 `0D801143`。原版资产因此确实把一组学生演出绑定到屋顶区域。表中的 20、32、84 都是数据记录数，同时可见的人数还受运行时条件影响。

## 引擎调用

反编译使用相邻的 `SimCity2013-source-tree/src/SC/` 函数树。指定的 PDB 目录含 `SimCity.exe` 和 `SimCity.pdb`，EXE 的 RSDS GUID 字节也见于该 PDB。该目录的 `SimCity.c` 只有 IDA 内部错误一行，不能作为函数正文来源。

1. `cUnitModel/FillFromProps.c:398` 起把两列组装为 `mUnitLocations`。取位置时按 name ID 查询。
2. `cVignetteGame/StartGame.c:171–249` 按全局 NonSim 列 `0D77CF78` 加载定义组。零售基础游戏 Property `00B1B104:40E0C800:622B9CD7` 指向 `40E1E800`。
3. `cVignetteGame/UnitCreate.c:26–35` 把建筑 NonSim Property 交给演出创建流程。`cVignette/FillFromProps.c:123–193` 逐层读三列，把 `propFiles[v77].mInstance` 传给 `GetVignetteEntry`，找到定义才登记演出及其可选 sink。
4. `cVignetteGame/FillEntryFromProps.c:452–496` 读取演员的外观、起点和资源绑定。第 705–750 行把 `0D36410D` 存入 `mImprovLocationSpace`，并读取该演员的活动区域和偏移。第 854 行起读取定向动作的位置空间。
5. `cVignette/GetWorldLocation.c` 根据位置空间 ID 查 sink 或建筑 UnitLocation。演出激活后，`cVignette/Update.c:548–675` 解析世界坐标并调用 `cVignettePool::RequestActorEntry` 分配可见演员。实际学生进出建筑还由模拟层的学生 sink、规则和交通事务处理。

## 改动前的工作台对拍

- `useWorkbenchAsset.ts:1275` 起的手动放置默认给出 `anchor` 系列的哈希。 `scene.rs:1280` 只写 name Key 与 Transform。其格式与原版位置表一致，放置动作本身没有关联演出定义或触发条件。验证器检查锚点 ID 和重复值，未检查该 ID 有没有消费者。
- `native_profile.rs:932–1028` 从 `edu-institute-anchors.package` 取位置两列。在这条路径，13 个 ID 与社区大学逐字节相同，Transform 列来自另一份模板，和社区大学不同。同名的手工锚点可以替换模板坐标。模板文件虽然也有三列演出登记，`education_anchor_pack` 只读取位置两列。
- 完整建筑 donor `complete-building-asset.bin` 无演出登记三列。小学 donor 有自己的登记，引用 `7A5C0A91` 和 `BE2FBD12`。因此能否看到演员取决于最终 Asset 的演出来源与触发状态。即使给完整建筑挂了教育行为，也不能从这 13 个坐标推出它会播放社区大学的人群演出。
- 本地 SC5 Asset Packager 0.8.0 Python 快照中未定位到可直接迁移的演出构建函数。后续若做原版配方，需要同时验证最终 Asset 三列的行对应关系、定义 Key 的可解析性、演出所需的锚点及其资源绑定和 sink。搬位置时用演出定义实际消费的 ID，屋顶活动区域还需落在模型可用的平面内。

## 配方驱动的写回

`asset_build_m1/spawner.rs` 按已编译的叶子配方查找原版演出记录，并写入新菜单身份的 Asset。原版定义仍由游戏加载。构建时用相同的演出 ID、定义 Key 与可选 sink 值，并复核输出的三列及必需位置。

| 已选配方 | 原版 Lot instance | 引用的原版定义 |
| --- | --- | --- |
| 小学 `6C042367` | `C264A268` | `7A5C0A91`、`BE2FBD12` |
| 社区大学 `6C042366` | `A826FA3E` | `00F0F854`、`ACE25EE3` |
| 大学 `3256C0A2` | `4250AE35` | `BBCC6A6D`、`A4294F4A` |
| 东京塔观光 | `084DB48F` | `637221E5` |

`resources/sc5/spawner-*.bin` 各自只保留上述 Lot 的五条原版属性记录。定义所需位置来自其起点、终点、定向动作、活动区域四列与 Lot 位置名的交集。选择了多个有演出的叶子时，行按演出 ID 合并。教育和观光共用位置名时沿用教育配方的位置。手动命名的位置优先，通用锚点依次填入尚未覆盖的配方位置。社区大学先填地面人群，再填屋顶人群。剩余位置沿用原版模板的坐标，因此模型尺寸或层高不同时可在场景中放置锚点调整。

## 可复核的数据指纹

以下 SHA-256 对应从零售包解压的 Property 内容：

| TGI | SHA-256 |
| --- | --- |
| `00B1B104:40E1C000:A826FA3E` | `4882746f576647d277fb3cd6c94b446f02359be0ed1db11457013e359d7ab41d` |
| `00B1B104:40E1E800:00F0F854` | `059c55a03d637f5cbe10d35becbcb095b5a224bc8162b09311157cce76ae3bbc` |
| `00B1B104:40E1E800:ACE25EE3` | `9a1e275c61e8f12fe833a2264b52e9e17b64952b457a7726f92b4178dd1a2b17` |

可用 Packager 的 `engine/package_io.py` 的 `index()` 与 `Entry.read()` 取得原始内容，再用 `engine/inspect_park_route.py` 的 `parse(include_raw=True)` 读列。社区大学的位置 Transform 列在 16 字节数组记录头后有 13 个 50 字节的元素，每个元素从第 38 字节起的三个大端 Float32 是平移。
