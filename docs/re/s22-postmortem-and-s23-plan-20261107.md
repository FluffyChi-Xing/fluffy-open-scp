# s22 归档复盘与 s23 方案

日期：2026-11-07

## s22 身份

- 部署包：`000_OpenSCP_University_s22.package`
- 部署 SHA-256：`8d4f7d152c724f4e7531fe83652c44bb3e16f037b08e2fc4c27aacc0ae7825d3`
- 构建哈希：`5e387f9b21bc2bcc6ebf9a8b8198569caafdaa1dd5ded928f1482debff6e0832`
- 状态：broken，已用 `scripts/archive_test_package.py archive` 归档
- 参照：s14 仍为唯一 `GOLDEN-known-working`，不得覆盖

## 用户实机证据

1. s22 的建筑模型与地面显著偏移。
2. 场景 schema 中的静态组件是 **10 个树 bin 实例 + 9 个 Car W1 实例**，不是额外的树。
3. 红框区域出现超过 10 棵树。
4. 菜单/详情仍有 placeholder，游戏把该建筑判定为非法或失效建筑。

## 根因边界

### 已确认：树数量重复

场景文档 `asset.lot.json` 的 prop 清单核对结果：

| bin | 资源 | 数量 | 语义 |
|---:|---|---:|---|
| 12 | `0x14984C68`（末项为 `0xC8B4254A` 变体） | 10 | 静态树实例 |
| 0 | `0x0AB4DFE1` Car W1 | 9 | 车辆实例 |

s22 另外把 `E7690450` 追加到 Unit 的 `0x0A0209A5`。该 bundle 会激活动态
tree-bin 填充规则，游戏随后按 bin amount/capacity 再绘制一套树。结果是：

```text
10 个静态树 + 动态 tree-bin 实例 = 红框内树数量超出 schema
```

这不是场景多写树，也不是车辆 bin 被误识别；是行为层和表现层重复声明同一视觉能力。

### 待单独验证：模型/地面偏移

s22 仍使用旧的 192×96 Lot、旧 `modelTransform` 和旧 path 坐标。偏移必须在
Lot/模型/地面三个坐标系之间单独验收，不能通过修改 tree bundle 修复。s23 先
保持场景 10/9 组件不变，只冻结并重新检查模型原点、Lot bbox、ground raster
和 path 点的同一坐标基准。

### 待单独验证：菜单非法/placeholder

菜单标题链和建筑运行链是两条独立链。s23 使用全新 Menu2 identity、自有 locale
表、显式 `toolType`/building category，并在冷启动菜单 oracle 通过后才进入 disposable
city。详情 placeholder 仍需按 board、Unit template、sink 连接和 UI 查询逐层归因。

## s23 设计

s23 是“静态树表现 + 最小教育行为 + 新菜单身份”的隔离实验：

- 保留 schema 中 10 个静态树和 9 个车辆实例；
- `treeDecoration=false`，不追加 `E7690450` 动态树 bundle；
- 保留教育模板、学校核心 bundle、水/电/垃圾闭环；
- 新建菜单 identity，不覆盖 s14/s22 或原版 TGI；
- 菜单显式写入 `buildingCategoryInstance=C710B6E9`；
- 先验证模型/地面坐标，再验证行为，不在同一轮调树数量。

## 验收顺序

1. 构建 ledger：树 prop=10、车辆 prop=9、动态树 bundle=disabled。
2. 静态 TGI/资源闭包和输出 SHA-256。
3. 归档旧测试包并登记 MANIFEST。
4. 部署 s23，冷启动主菜单；菜单卡住则停止。
5. disposable city 中只放置一个 s23 建筑；不修改含健康建筑的存档包集合。
6. 观察模型/地面对齐、静态树数量、车辆数量、菜单标题和建筑生命周期。

## 停止条件

- 树数量仍超过 10：停止，不再调模型或树坐标，检查是否有其他动态 tree bundle。
- 模型仍偏移：停止行为实验，进入 Lot/model transform 专项实验。
- 菜单仍 placeholder：停止模拟实验，进入 locale/board/UI binding 专项实验。
- 建筑放置后被清理：停止保存，按 SinkCapacity/事件规则/几何接入链抓取。

## s23 构建记录（2026-11-07）

- 用户提供的模型路径已确认存在：`assets/models/1b05299daf92386006bf0508dd46910176a95eb63737d7b8f5ec6e6764e09bd4.lotm`。
- 源模型 SHA-256：`1b05299daf92386006bf0508dd46910176a95eb63737d7b8f5ec6e6764e09bd4`。
- 初始 `building40` 构建拒绝是**材质契约错误**，不是缺文件：LOTM 有 74 个材质组，而 `building40` 只接受单 atlas 材质。
- s23 已改用正式 `nh-atlas` 处理器：74 mesh/74 源材质组经过 atlas 生成与 UV 重映射，构建成功。
- 构建哈希：`ccc1a45b3d2e0dec01d9c31f6ded5a70e11268249b256dc08a3b79d82896a279`。
- 构建产物 SHA-256：`6ce2d7e10bfe92442e99fbbb142373ed8bc5babd65dfcf74c5c10187068201d2`。
- 构建账本：50 units、1 path、1 PathEntry、124 material resources；scene prop 语义保持 **10 个 bin=12 静态树 + 9 个 bin=0 Car W1**，`treeDecoration=false`，未追加 `E7690450`。
- 为满足路径闭包，staging reference 合并了原版 Path/PathEntry 资源；用户项目源文件未被改写。
- s23 已通过 `scripts/archive_test_package.py deploy` 部署到正确的 `SimCityData`，并登记 `test-archive/MANIFEST.md`。
- **实机状态：待用户确认。** 本次不会由工具启动、关闭、进入游戏或存档；菜单加载 oracle 与 disposable city 验证必须由用户手动执行。若主菜单加载挂起，立即停止，不进入存档。

## s23 菜单可见性复核（2026-11-07）

用户实机反馈 s23 菜单完全不可见。与 s14 GOLDEN 的构建账本对拍后，结论是：

| 字段 | s14 GOLDEN | s23 当前包 | 层结论 |
|---|---|---|---|
| menu instance | `0xBE18DA2D` | `0x9A766834` | 新身份，符合新入口要求 |
| categoryInstance | `0x07049250` | `0x899BEA70` | **不等价；s23 使用了未证明可见的类别** |
| menuParentInstance | `0xF970A822` | `0xAF042E9A` | **不等价；从公园子根切到 plop 根** |
| categoryInstances | 未扩展 | 空 | s23 没有注册 s14 的公园类别容器 |
| buildingCategoryInstance | 未写入 | `0xC710B6E9` | 额外实验字段 |
| lotBoardInstance | 未写入 | `0x6A1785B4` | 额外实验字段 |
| simulationTemplateInstance | `0x7E4B99E5` | `0x7E4B99E5` | 行为模板同构 |
| uiPosition/cost/maintenance/parentFree/rotation | 相同 | 相同 | 非根因 |

因此，不能再说“s23 只更新了词条”。当前 s23 的菜单层至少改变了 category 与 parent，且加入了 board/building-category 实验字段；在资源成功构建的前提下，游戏菜单完全不可见首先应归因于**菜单注册链不等价**，而不是模型、树数量或材质 atlas。

下一版若目标是“除标题外与 s14 同构”，菜单应恢复 `categoryInstance=0x07049250`、`menuParentInstance=0xF970A822`，删除 `buildingCategoryInstance`、`lotBoardInstance` 和其它未验证菜单字段；行为/模型实验必须另起包或另设明确开关，不能混入菜单可见性实验。

## s24：冻结场景、单独验证行为绑定（2026-11-06）

- s23 已归档：部署包 SHA-256 `6ce2d7e10bfe92442e99fbbb142373ed8bc5babd65dfcf74c5c10187068201d2`。
- s24 构建包 SHA-256 `48279f2671a74e42f9e34e24f106a0c57391f8cadb6fd9075f700dc3b5368aa6`，已用 `archive_test_package.py deploy` 登记。
- s14 GOLDEN SHA-256 仍为 `024f8631712292b073a5135c64919f41a342e0f9e461653d232860edaf88dc96`，作为 identity donor；没有覆盖或改写 GOLDEN。
- s24 菜单轴恢复 s14 入口：`categoryInstance=0x07049250`、`menuParentInstance=0xF970A822`；标题唯一改为“大学建筑测试包 s24”。
- s24 行为处理器只修改新 Unit identity 的增量字段：`behaviorCategoryInstance=0x95A9D6FD`、`simulationTemplateInstance=0x7E4B99E5`、`studentCapacity=250`，并保留 s14 donor 的教育/生命周期字段链。
- `treeDecoration=false`；不追加 `E7690450`。Lot、模型、材质、地面、静态树/车辆和路径均来自冻结 donor，scene-semantics ledger 为 `loweredUnits=0`、`materialResources=0`。
- 构建结果 16 resources，P3 property gate 建立 baseline；后续行为迭代必须复用 donor 并在 behavior processor 中声明字段，任何场景层资源漂移应在构建阶段硬失败。

### s24 验证边界

本次只完成静态构建、哈希、台账登记和部署。按照规则，不由工具启动游戏、不进入存档、不放置建筑。用户手动冷启动主菜单；若菜单加载挂起，停止，不进入城市。通过菜单 oracle 后，仅在 disposable city 放置一个 s24，观察教育行为是否生效。

## s24-r2 串线修复（2026-11-06）

用户反馈放置后表现为 s14。静态对拍确认 Menu2/Unit/Lot 均已分配新实例
`0x02472455`，Unit→Lot 与 Lot 自映射也都指向新实例，因此不是 instance allocator
串线。真正的回归是 s14 donor 的动态树 bundle 使用 `0x0A0209A2` 重复字段，而清理器
只处理了旧的 `0x0A0209A5` 字段，`E7690450` 没有被移除。修复后 `treeDecoration=false`
会清理两种字段编码；构建器闸门将该 Unit 列变化明确记录为 `lotChange`，不会静默放行。

- 修复版 SHA-256：`e5c1efdb0e41c816b4d49806507c83f95c1d105d4aedfbe3c58fa02e188fca3b`
- 修复版仍保持新实例 `0x02472455`、教育类别 `0x95A9D6FD`、容量 `250`。
- 静态检查确认 `E7690450` 已不存在；模型/材质/地面/路径没有重新生成。

## s24-r5：全量重打包 + 行为处理器（菜单/行为拆分落地，2026-11-06）

用户裁定：s24 全面重新打包——模型/菜单与 s14 相同，公园菜单只是入口，
行为完全拆开。同一身份（`0xBE18DA2D`），标题改 s24 标识（唯一声明的菜单层
漂移）。

### 架构

- **full 模式**：`outputs.mode: "full"` 把参考快照整体重打包 → 包独立，
  不再依赖 donor 在场（delta 模式的教训：s14 被改名后引用链全断）。
- **behavior-bind 阶段**（构建链第 8 阶段，`asset_build_m1/behavior.rs`，
  `semantics.behavior` 声明）：只允许写 Unit/Lot/locale(title 行)；菜单层
  （Menu2/图标/marquee）字节冻结，违例在阶段内 fail-closed。
  `plan.rs` 硬校验：behavior 必须 full 模式、与 semantics.menu 互斥。
- **property_edit.rs**：menu.rs 的 set_scalar/set_array 等提取为共享模块，
  两个处理器使用同一编码语义。

### 教育最小闭环字段（对照 `00B1B104:40E0C000:BE18DA2D` / Lot 同 instance）

| 字段 | s14 原值 | s24-r5 | 依据 |
| --- | --- | --- | --- |
| `0x00B2CCCB` parent | `00B1B104:40E0C000:7E4B99E5` | `00B1B104:48E0C000:7E4B99E5` | 原版 parent 是 type-ful 模板键（消防站解剖 `00B1B104:48E0C000:BD7BF4A1`）；s14 的 group 写错（40E0C000=Unit 组），此次修复 |
| `0x0A0209A5` bundle | 265FB8DD, C0672F57 | + `6C042367` 教育签名 bundle | 原版小学 Unit 即挂 6C042367（er2 解剖）；规则/bin 全在 scripts 库按 Key 引用 |
| `0x0AD7FAC9` capacity | 20 | 250 | 用户裁定容量 |
| Lot `0x0D7970ED` board | `D0621548`（公园残留） | `6A1785B4` 教育看板 | s19→s20 教训：公园残留导致人口地图不亮；s14 实测确实是残留，此次修复 |
| locale `0x00000001` | 大学（原标题） | 大学建筑测试包 s24 | 用户裁定的唯一菜单漂移；其余行字节冻结 |
| `0xE7690450` 树 bundle | A5 数组 | **保留** | s14 的树来自公园链；剥掉即重蹈 s15 树消失 |

### 静态对拍（资源/字段层等价声明）

- TGI 集合：20/20 相等；**17 条字节级相同**（Menu2 876 B、图标、marquee、
  PathEntry、全部 11 条 RW4 模型资源 ≈285 MB）。
- 变更仅 3 条，字段级 diff 恰为上表（dump_package/property_diff 工具核验）。
- 构建确定性：同输入两次构建 SHA 一致 `91921832889b28f7…`（processor /21）。
- 容器层声明：DBPF 由本工具链重排序重写（TGI 排序），字节布局与 s14 不同是
  容器层预期行为；等价结论只覆盖资源/字段层。

### 事故与纠偏（台账已登记）

1. s14 部署副本曾被台账外改名 `.package.1` 下架 → s24 delta 包三症状；
   已恢复、核哈希（仍 `024f8631…`），事故期间加载的存档按污染处理。
2. s24 项目 `build/reference.package` 是未知快照（`c1c867f3…`，956 MB），
   不是 s14——**参照必先验哈希再次生效**；已替换为核验 GOLDEN 副本。

### 部署态

- SimCityData：仅 `000_OpenSCP_University_s24.package`（`91921832…`，
  295,576,473 B）；s14 已归档（GOLDEN 副本在 test-archive，哈希复核一致）。
- 存档备份：`Games-backup-20261106-s24r5`。
- 验收流程（用户）：冷启动菜单 oracle（挂起即停）→ disposable city 放置 →
  教育面板/人口地图（容量 250）、树与 s14 一致、标题"大学建筑测试包 s24"。

## s24-r6：表现层教育化 + 通勤修复（2026-11-07）

r5 实测结论（用户多游戏日观察）：教育规则层已通——"4 學生"注册、已受教育
毕业离校、18:00 放学；但**通勤未通**——对面未注册学生从不选择本建筑，只有
中午 4 人（邻近转化规则的产物）。文档定案（elementary-school-rule-schedule
/5am-7am 观测）：**没有"6AM 开学触发器"**，入学是逐个 sink 到达事件
（原版首个学生 07:18 到达；06:00 规则只是 TotalStudents 清零）。零通勤
= 到达许可缺失。

### 根因（静态定案）

r5 Lot 的 path 网络只有一条 path（9 点，段 [0,8]），PathEntry 表
= `[03106C2E]`——**Cars And Crap 类型且被脚本库同名克隆遮蔽**（s14 已知
缺陷的承继）。行人过滤器 `E0CD3122` 下没有一条可走路径 → 学生到达许可
为零。小学走读最小配方（census §12）："仅 Pedestrians 路"（`418A18B1`）。

### r6 变更（全部为 behavior spec 声明项，闸门字段级校验）

| 层 | 字段 | 变更 |
| --- | --- | --- |
| Lot | `0x0CC8FE7A` PathEntry 表 | `[03106C2E]` → `[418A18B1]`（Pedestrians；六件套计数一致性 fail-closed，scope Local/roadConnect None 保持） |
| Unit | `0x0A0209A5` | 追加至原版教育四 bundle：`6C042367+29D4B82E+95573B5D+9A1BFB2F`（对照容量 800 的原版教育 Unit 64F750E2；E7690450 树 bundle 在 A2 保留） |
| Unit | `0x0DA69D6F` | 无 → `2C7C7986`（原版学校携带的内部 path entry 引用；GetUnitInternalPathEntries.c） |
| Menu2 | `0x0DB9FC63` uiCategories | `[07049250]` → `[07049250, 6A1785B4, 95A9D6FD]`（公园入口保留 + 教育看板/类别双挂，取自原版教育菜单 AF3EAA80） |
| Menu2 | `0x0EB1FC05/0x0EB1FC22` | 无 → 原版学校统计行（上课时间/学生容量两行 + 参数，逐字节克隆；公园"每日遊客"行不再显示） |
| 图标 | `2F7D0004:40E02400:BE18DA2D` | 1111 B（树）→ 20325 B（原版教育图标，字节克隆） |
| locale | 标题行 | 保持"大学建筑测试包 s24"（r5 已声明） |

实现：`behavior.rs` 新增 `ensureWalkPath`/`ensureInternalPathRef`/
`presentationPackage`+`presentationUnitInstance`+`adoptUiCategories/
adoptStatRows/adoptIcon`；Menu2 冻结改为**字段级**（声明外字段漂移硬失败）；
表现层供体包（SimCity_Game.package，SHA `79a80449…`）复制入项目
`build/presentation-donor.package` 并入 compute_identity 指纹（防缓存陈旧）。

### 静态对拍（r6 vs s14 GOLDEN）

TGI 20/20 相等；15 条字节相同（全部模型/marquee/03106C2E 资源本体）；
5 条变化 = 上表声明项，字段级 diff 与声明表零偏差。两轮构建 SHA 一致
`f6e8f21fd9a4aee6…`。

### 部署态与验收边界

- SimCityData：仅 `000_OpenSCP_University_s24.package`（`f6e8f21f…`）；
  r5 归档为 `…_s24-r5-91921832.package`；存档备份 `Games-backup-20261107-s24r6`。
- 已知边界：校车道（`7C9C4FFF` Global+First 接路网）未加——几何需对实际
  道路坐标，先验走读最小配方；若走读仍不通，下一轮加校车道与门口锚点。
  未解之谜（文档边界）：TransportOps/校车调度未反编译；锚点是否服务停靠未知。

## r6 实测与 r7 前置诊断：教育家族谱系（2026-11-07）

### r6 实测结论

人行道修复实证生效：学生沿设计路径步行到校（轨迹一致），注册 4→22。
但规模仍远低于容量，学生不"主动选择"。（定位牌滞后显示上一路径点，不能
作到达证据。）

### 教育家族谱系（静态定案，scripts 库 1214 Unit 全扫）

| 建筑 | Unit | parent 模板 | 类别 | 签名 bundle 集 | 叶容量 |
| --- | --- | --- | --- | --- | --- |
| 小学 | C264A268 | 7E4B99E5 | 95A9D6FD | `6C042367`+3A1C3971+95573B5D+9A1BFB2F | 150 |
| 中学 | 64F750E2 | 7E4B99E5 | 95A9D6FD | `6C042367`+29D4B82E+95573B5D+9A1BFB2F | 800 |
| **社區大學** | A826FA3E | **7E4B99E5（同！）** | **95A9D6FD（同！）** | **`6C042366`**+487CDA43+BA823893+472644E9+53BA360B+CF341CAC+9A1BFB2F | **0（面板 500 来自 bundle bins）** |
| 大学族 | 83E469E2 | 121D336E | 6A1785B5 | AF195AFC+A7ED0084+5D7F8D92 | 0 |
| 教室模块 | 5DE9CD2A/1C0EC2D0 | 121D336E | — | AEF52A8E(+E575E048) | 200/1000 |

### 签名 bundle 内部对比（6C042366 vs 6C042367）

中小学：**1 个 sink**（78B96709，收 Kids）+ **1 条规则**（B52C889D 邻近
KidE2 转化）；23 bins。
社區大學：**4 个 sink**（D10F7B4E/66883F3B/8365DE58/07ED31A2）+ **4 条到达
规则**（585F51B6/B5/B4/F59FDDF9——多通道到达：步行/驾车/公交的成人学生）；
30 bins；学生 bin 族 `182008439`（≠小学 `181926601`）。

### 假设裁决

1. **"path 是添头"**：否。path=到达许可（必要），实证 0→22；但到达只是
   运输事务，**"谁被派来"由 sink 类型与全城分配决定**。
2. **"小学初中靠校车、步行少"**：交通方式是 sink 内部分层，不是主瓶颈。
3. **"大学生开车来所以大学有车道"**：不成立——原版大学 lot 五条 path 全
   Local+None，无 Global 车道；自驾体现在停车场 props（CarW1/2/3
   percent-fill，我们 lot 已有）与多 sink，不在 path scope。
4. **真正的根因（新定案）**：我们的建筑是**小学身份**（sink 只收 Kids）。
   城市里适龄儿童少 → 22 即天花板。旁边紫色"未註冊"学生是想要**大学席
   位**的成人学生——他们的 sink（6C042366 的 4 个）我们一个都没有。

### r7 建案（待裁决，未实施）

对齐社區大學身份：
1. Unit bundle **替换**：[6C042367,29D4B82E,95573B5D] → [6C042366,487CDA43,
   BA823893,472644E9,53BA360B,CF341CAC]（9A1BFB2F 保留；需要 spec 新增
   removeBundles/replace 语义——现有 ensureBundles 只加不减）
2. 叶容量 0x0AD7FAC9 删除（vanilla 大学叶无容量，面板值来自 bundle bins）
3. 表现层 donor 换 menu B01CEDE4/unit A826FA3E：统计行 3 行
   （228241430+228241447+247259221，参数 182008439/205430681/240671456）、
   uiCategories [6A1785B5,95A9D6FD]、教育图标——现有 presentation 机制
   全部复用（menuUnit 反查 donor）
4. Lot 不动（人行道 418A18B1 已通；大学原版也无 Global 车道；停车场
   props 已在）
5. 风险注记：6C042366 的 4 sink 到达是否依赖未实证的交通定义
   （TransportOps 未反编译）——步行 sink 若在其中一条规则里，走读即可工作；
   若全部依赖驾车/公交，则 r7 后仍需停车场/车站 props 对齐（先实测再迭代）

### r7 实施（2026-11-07，用户裁决后）

- `BehaviorSpec` 新增 `removeBundles`（四行启用数组逐行剔除）与
  `removeFields`（叶子字段整体删除）；不变量新增 BUNDLE_REMOVAL/
  FIELD_REMOVAL（移除后必须真消失）。
- r7 = 社區大學身份对齐：remove [6C042367,29D4B82E,95573B5D]、ensure
  [6C042366,487CDA43,BA823893,472644E9,53BA360B,CF341CAC,9A1BFB2F]、
  removeFields [0x0AD7FAC9]、presentation donor 换
  menu B01CEDE4/unit A826FA3E（统计行 3 行/uiCategories/图标 19293B）。
- 静态对拍：20/20 TGI、15 条字节相同、5 条变化与声明零偏差；两轮
  SHA 一致 `40aa383cfbd57a04`。
- 部署：SimCityData 仅 `000_OpenSCP_University_s24.package`
  （`40aa383c…`）；r6 归档 `…_s24-r6-f6e8f21f.package`。
- 观察点：未注册成人学生是否开始注册（教育地图註冊数）、面板是否变为
  社區大學三行统计（上课时间/接受中的學生/城市學生）、容量是否由
  bundle bins 决定显示。若成人学生仍不来：下一候选是大学 4 sink 的
  到达方式核验（步行 sink 是否存在——TransportOps 未反编译）。

### r8 实施（2026-11-07，当日最后一版）

r7 实测补充：峰值 250 学生（成人+孩童混合，驾车到达已实证）。诊断定案：

1. **停车 bin 根因**：CarW1/2/3（0AB4DFE1/E2/E3）资源 bin（容量 2^31）在
   经济 bundle `30645625` 内，而 s14 排除表 0x0A0209B2 把它挡了 →
   percent-fill 恒 0。车辆 sink（r7 大学 bundle）与车辆 bin（经济 bundle）
   是一条链的两半，被排除表切开。
2. **250 峰值非硬容量**：8 教育席位 bin 的 bundle 默认容量=1，模板链无
   容量字段——250 是需求/分配饱和。
3. **原版大学 Lot 看板 = 6A1785B5**（大学族），非 6A1785B4（中小学族）；
   锚点仅 13 个通用（与我们一致），入口人群差异来自 prop 组（它 5 组
   bin-draw，我们 2 组）+ 注册规模。

r8 变更（BehaviorSpec 新增 removeExcludedBundles / setFloatFields +
EXCLUDE_REMOVAL / SET_FIELD 不变量）：
- 排除表解封 30645625（1B946073/E0C02E1F 保留）
- 大学叶子初始量 0x0B9880C6..DB = 0/0/500/500/500/500/0/0 + 0x0AD93A77=500
- 小学死字段 0x0B98804E 族清除；Lot 看板 → 6A1785B5
- 对拍零偏差，两轮 SHA 一致 `7ee01dc54a636a6c`；已部署。
- 观察点：停车场是否出现 percent-fill 车辆、Lot 看板换族后面板/地图聚合、
  注册峰值是否上移。校园 props 组（14984C6A/69 等）与校车道留待下一轮
  （几何需按 192×96 地块重排，不能照抄 88.7×69.9 的原版坐标）。
