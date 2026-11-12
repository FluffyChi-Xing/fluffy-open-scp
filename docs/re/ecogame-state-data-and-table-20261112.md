# EcoGame State Data、State Table 与 ER2

2026-11-12 · 零售 EP1 资源、零售 EXE 静态分析、开发版带符号反编译。未启动游戏。

## 已知认知与本次校正

- [静态地图基线](map-static-bootstrap.md) 已证明 15 个区域和 130 个城市可以用官方包的 bootstrap 预览。模板组 `95F4AEF0` 下，`46066280` 的 State Data Key 与 `46066281` 的 site UID 逐项对应，`F240A8CA` 指向地图元数据。区域道路取 site `0`，城市网格取对应城市 site。生产代码已经校验 site、environment 和网格。
- 文件类型词典的「`622B9CD7` 魔数」「State Table 为 12 字节记录」需要校正。前者是本批快照的 **game ID**。后者有两段，记录分别为 16 和 20 字节。包加载笔记里把 `08068AED` 列进编译规则资源族的表述也需要校正。
- [包浏览器后端](../../src-tauri/src/package_service.rs#L5705) 对 State Data 调用生产解析器后只保留五个摘要字段，对 State Table 只显示前 256 个 BE word。State Data 的 `exact` 表示现有子集读取成功，State Table 的 `exact` 只检查四字节对齐。两处标志都不能用于判定资源完整解码。

## State Data（08068AED）

在 EP1 中它是每个 site 的模拟初始快照。DBPF 条目可先由 RefPack 解码，所得 payload 再做 gzip 解压。解压后的状态流使用大端序。对当前安装的 `SimCityDataEP1.package` 全量调用 [region_state::read_state](../../crates/sc-properties/src/region_state.rs#L484)，145/145 通过该读取器覆盖的部分，分别为 v17 95、v18 11、v20 39。

| 段 | 当前零售读取器覆盖 |
| --- | --- |
| 头 | 状态版本、game ID、脚本版本、add-on 个数及 ID/版本区、environment、skill 区、模拟 ticks、next UID。v17/v18 额外接受一个为空的扩展集合。 |
| EcoMap | 地图计数、地图子版本。每图含 map ID、格子数、BE i32 栅格、格子值的算术和。子版本 3 的和为 32 位，4 为 64 位。 |
| Unit | 有效记录数、槽容量、子版本 13。每条有 slot、Unit 定义 ID、实例 UID、图形组/状态、网格位置、带 flags 的变换、控制/分区信息、资源 bins、连接槽。当前投影保留定义 ID、图形状态和变换等字段。 |
| PathSet | 子版本 6。点、切线、segment、path、group 按槽组织，保存空闲槽及引用。由端点和切线恢复曲线。曲线数量对应解出的 segment，path 可由多条曲线组成。 |
| 后续状态 | 开发版序列化在 PathSet 之后继续处理 Zone、全局 bins、Transaction、server delta 和历史。当前零售读取器到 PathSet 即返回。后续字段的零售版本布局须单独核对。 |

开发版证据位于指定目录旁的 `SimCity2013-source-tree/src/GB`。指定目录的 `SimCity.c` 只有一行 IDA INTERR。实际读取了其 `SimCity.pdb` 的 Info 流和 EXE 的 RSDS，原始 GUID 字节均为 `9866ed0360472c479f1e67face23f7cc`、age 均为 1。PDB 包含 `cEcoGame::Read`、`WriteInternal`、`cEcoUnit::Read` 的修饰符号。函数正文来自同一研究目录中的带符号反编译树，作为开发版语义证据。该树的 `cEcoGame/Read.c:79–99` 接受 v11–15，`WriteInternal.c:59–85` 写 v15，skill 段顺序也与本批零售字节有差异。

### 截图样本

`08068AED:C3F7EB56:031BDB86` 属于 **TwinCities／追日湾，site 1027**。由 `00B1B104:95F4AEF0:E3568C78` 的 Key/UID 数组及元数据 `0A98EAF0:C3F7EB56:4529F96F` 交叉确认。其他模板也使用实例 `031BDB86`，查找须带完整 TGI。

- DBPF 层未压缩，条目自身是 222,255 B gzip，解压为 2,212,958 B。状态版本 17，game ID `622B9CD7`，脚本版本 `0F5716BC`（257365692），add-on 数 0，environment `89190D64`，ticks 0。
- 33 张 128×128 网格。例：煤 `D779C976` 值域 0–60,500，地下水 `0F92DC56` 值域 43–65,534，森林 `1D52BE3E` 值域 0–65,404。值域为原始整数，游戏显示单位需结合 MapEntry 定义。
- 19 个 Unit 槽记录。PathSet 有 161 个点、184 条切线、158 个 segment、26 个 path。当前子集读到解压偏移 `0x2170C2`，余 21,404 B。余段开头为 `4,4,3`，符合开发版 Zone 计数、槽容量、子版本的顺序，余段未完整解码。

## State Table（08068AEE）

这是区域模板挂接的跨 site 贸易包路由表。TwinCities 的同一模板 Property 通过 `19F5C77C` 指向 `08068AEE:C3F7EB56:F9B19BD3`。EP1 有 15 张表，实例均为 `F9B19BD3`，以 group 区分。TwinCities 样本经 DBPF RefPack 从 169 B 展开为 412 B。

```text
u32be version = 0
u32be n
n × (u32be, u32be, u32be, u32be)
u32be m
m × (u32be, u32be, u32be, u32be, u32be)
```

15/15 都满足总长 `12 + 16n + 20m`。TwinCities 为 n=10、m=12，第二段计数偏移 `0xA8`。例如第一段 `[1030, 1030, DDDDDDD2, 1027]` 和 `[1030, 1030, DDDDDDD2, 1026]` 表达同一查找键的两个目标 site。第二段例如 `[1026, AAAAAAA2, 1032, AAAAAAA2, 1026]`。

零售反汇编确认 `0xC17730` 从模板取 `19F5C77C`，`0xC1AE95` 将取出的 stream 交给 `0x605840`。后者用大端 32 位读取逐字段建立两张表。后续 `0xC1E589 → 0x606190` 使用它们处理 `cEcoServerDelta::mPackages`。零售路径迭代 0x54 字节记录，对应开发版 PDB 类型导出的 `include/GB/cEcoResourcePackage_ord5508.h` 和 `cEcoServerDelta_ord5514.h`。首两个字段分别为 `mTradeID` 和 `mUID`，并可对上发送时对单位槽和重试计数的重置。

零售 RTTI 将 `0xC1ADF0` 所在虚表归属为 `GB::cGameManager`，虚表入口在 `0xDA06C0`。读表和转发都使用该对象的 `+0x154` 成员。

- **区域 → 城市，16 B。** `0x605F90` 用输入包的 `mUID` 查第一列，按第四列的目标 site 分发。输出 `mUID` 取第二列，`mTradeID` 取第三列。一个键可以对应多个目的地。
- **城市 → 区域，20 B。** `0x605D50` 用 `(源 site UID, mTradeID)` 查前两列，必要时用 `mTradeID` 的低四位重试。输出 `mUID` 和 `mTradeID` 取第三、四列，写入 site 0 的分组。该函数未使用第五列。

这些是根据零售控制流命名的列用途。`DDDDDDD2` 等值在这里作为打包的贸易 ID 使用。其具体业务类别、开发者原始列名、20 B 行最后一列的用途，以及完整消息类型枚举尚未逐一验证。

全 15 表共有 1,158 条第一段记录和 1,288 条第二段记录。第二段改写后的 `(mUID, mTradeID)` 均能在同一表的第一段匹配。第二段第五列在本批样本恒等于第一列。TwinCities 中，城市 1026 的 `B1111112` 先被改写为贸易 UID 1046、贸易 ID `B1111112`，对应的第一段路由再指向城市 1027。这里的贸易 UID 用于接收单位查找，site UID 用于选择城市，二者在某些行数值相同。

TwinCities 第一段的 9 个不同查找 UID 均存在于区域 site 0 的 Unit 快照 `08068AED:C3F7EB56:7439CB9C`。两个城市快照并未覆盖全部输出贸易 UID。预览可以尝试按 site 和 UID 定位实例，找不到时保留原始路由。游戏的 `CanAcceptServerPackage` 在指定 UID 无匹配时还可通过 `NextTradeUnit(tradeID)` 查找。

## 与 ER2 的关系

ER2 是规则定义，State Data 是在某一套脚本下产生或恢复的模拟实例。开发版 `cEcoRulesFactory/GetSupportedTypes.c:25–32`、`ReadResource.c:27–42` 显示运行时抽象请求 `08068AE9` 可落到文本 `08068AEA` 或编译 `08068AEB`。开发版 `cEcoGame/Read.c:91–175` 按 game ID、脚本版本及 add-on 版本选择脚本，初始化环境后，再用脚本注册的定义恢复状态。

- Map ID 经 `IndexFromMapID` 找 MapEntry。Unit 的磁盘 ID 经 `IndexFromUnitEntryID` 找运行时 `mEntryIndex`，实例 UID 单独读。PathEntry ID 和资源 bin ID 也经脚本映射。证据分别在 `cEcoGame/Read.c:187–205`、`cEcoUnit/Read.c:44–56`、`cEcoPathSet/Read.c:155–176`、`ReadBins.c:29–48`。
- 截图所声明的版本可在已安装的 `SimCity-Scripts_257365692.package` 找到。实读索引存在规则库 `08068AEB:40800200:622B9CD7` 及配套 Property `00B1B104:40800200:622B9CD7`，Property 的 `09EEB11C` 为 257365692，AEB 开头的版本字为 4.2.13。同一 TGI 也出现在其他版本的脚本包，跳转需带脚本版本和 add-on 配置。现有 [Unit 规则探针](../../probes/runtime/unit_rule_resource_bridge.py) 针对 5.2.14，直接打开历史规则仍需验证版本支持。静态关联表示查到定义，规则执行仍需运行时证据。
- 跨 site 贸易链中，开发版 `cEcoUnit/RunRule.c:350–364` 的 Package action 调用 `cEcoGame::CreatePackage`。后者在 `CreatePackage.c:128–137` 把规则生成的资源数量、`mUID` 和带方向的 `mTradeID` 写入 `mToServerDelta.mPackages`。零售的路由消费者选择目的地并改写这些标识。接收侧的开发版 `CanAcceptServerPackage.c:35–90` 和 `AcceptServerPackage.c` 用 `mTradeID & 0xFFFFFFF0` 取贸易定义 ID，并通过 `mTradeUIDMap` 将贸易 UID 映射到接收单位槽。

## 包浏览器预览方案

1. **State Data 先展示「这是哪座城的哪份状态」。** 顶部给出区域、site、environment、状态版本、声明的脚本版本及完整 TGI。DBPF 压缩和内部 gzip 分开显示。标注解析到哪个段、未解析多少字节。未绑定模板时保留原始标识。
2. **提供地图、实例和路径三个可检索的视图。** 地图复用现有图层渲染，按 MapEntry 显示名字、尺寸、值域和鼠标所指格子。实例表支持按定义 ID、UID 或槽检索，点选后在地图定位。路径以 PathEntry 分组，查看 path/segment 拓扑和原始字节。完整区域路网入口使用 site 0。城市快照的曲线可在该 site 内单独检查。
3. **State Table 先做路由表格。** 两段各自分页，可按 site、查找键和标记过滤。点击 site 打开模板对应的 State Data。选中一条路由时展示可能的输出和改写前后值，复用已有地图坐标展示可定位的 site。保留原始 word、偏移及 Hex，未证实的标记以十六进制展示。
4. **定义链接按状态声明的版本解析。** 首屏先读头和目录，选中地图才取网格，选中定义才加载对应版本的脚本元数据。解析不到脚本版本时显示缺失的版本，方便核查跨版本差异。

## 样本身份

| 样本 | SHA-256 |
| --- | --- |
| 零售 `SimCityDataEP1.package` | `1a556f499cc643e6d96a2cce8ff8492bc202e7e141c6b362ada7accd27ae758d` |
| 截图样本，gzip 展开后 | `70c5bbf730ff4dcfb2903743c360ac3a5ca876d8e48e6274e26f546db8a0178c` |
| TwinCities State Table，DBPF 解码后 | `67e6a65019ed1ba064e529ef9a4f079a8834a6796d8185d4b7d76efe899a1c03` |
| `SimCity-Scripts_257365692.package` | `f38cd611e69b1c0193997e78180af8d17530a877c862030e113e2415a1e6f7db` |

本轮使用 DBPF/生产 Rust 读取器、PDB Info 流与 PE RSDS 的只读核对、零售 PE 的 Capstone 定点反汇编。零售 VA 以基址 `0x400000` 表示。
