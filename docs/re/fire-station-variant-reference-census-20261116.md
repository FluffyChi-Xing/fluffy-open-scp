# 消防局原版变体引用普查

从 Scripts 版本 `287520926` 和该安装的五个原版 SimCityData 主包读取消防局 Unit `00B1B104:40E0C000:E917279C` 及 Lot `00B1B104:40E1C000:E917279C`。复用变体注册的依赖遍历与地面栅格定位规则，放置和存档无需参与。

共 3,241 条字段行。现有分类为 183 条已定位原版资源、40 条已识别运行时键、3,018 条未解析。未解析项来自 66 个 Property 对象、128 个字段哈希、688 个不同的 instance。根 Unit 占 12 条，根 Lot 占 68 条，剩余 2,938 条来自递归展开。1,165 条存在至少一个相同 instance 的包资源候选，只有字段机制能确定哪个候选有效。

## 有证据的字段

| 字段 | 数量 | 已核对的语义和处理建议 |
| --- | ---: | --- |
| `0D649A26` | 373 | 定向演出演员 ID。引擎用它索引当前演出的演员表。 |
| `0D649B64` | 373 | 定向演出 `animsToPlay`。按演出与演员的动画语境解析。 |
| `0D801143` | 373 | 定向演出的位置空间。按演出和地块锚点关联。 |
| `0DCA0D6A` | 309 | 定向演出 `audioToPlay`。290 行记录为 `none`。 |
| `0D649BD9` | 302 | 定向演出 `effectsToPlay`。264 行记录为 `none`。 |
| `07E0AD23` | 179 | Bundle 资源 bin 声明。179 行均命中 Property 组 `40E0C100` 的 GameBin 定义。 |
| `0A2725CE` | 69 | Bundle sink 入口。69 行均命中 `40E0C500`，62 行同时有 `40E1C500` 同 instance 项。按入口类型选择。 |
| `0A2725CF` | 69 | 与 sink 入口按行配对的模拟规则 ID。按规则表查询；其中一行的同 instance Property 实际属于 sink 组。 |
| `0A6FE749 / 0A6FE74B` | 68 | `kBinCapacityTable` 与 `kBinAmountTable` 的资源 bin ID。按各自行与容量或数值列配对。 |
| `0D2E8A61` | 58 | 演出演员外观 `modelNames`。52 行同时出现 `40E02D00` 外观 Property 和同 instance RW4，6 行为 `none`。先按外观组解析。 |
| `0CCB7FD4 / 0D0082F1` | 8 | 地块贴图和法线图字段。7 行有同 instance RW4 候选。 |
| `02A907B5 / 0CCB7FD5` | 5 | 4 条效果键和 1 条地面遮罩键在此次扫描范围内未定位到目标。保留所属对象和加载范围，继续用效果导出表和栅格索引核对。 |

前五列来自反编译 `cVignetteGame::FillEntryFromProps` 的 `kActorTableDirected`（本地 `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c` 第 851261–851290 行）。其中两列的 `none` 共 554 行，同 instance 恰好也存在一个 GameBin，说明单用 instance 回填会误连。规则、资源和位置空间的既有证据见 [行为字段普查](asset-behavior-census-and-gating-20261106.md) 和 [生成器与演出](spawner-render-and-assembly-20261101.md)。

## 解析器边界

新建一个只读语义解析器。首版按 owner group、字段 hash 及配对列选择查找方式。结果应保留原始 key、来源、字段和行号，并区分已定位包资源、局部符号、运行时查表键、显式空值和待核实目标。遇到独立 TGI 时按完整 TGI 查询；遇到隐式组时只查字段声明的候选组。作用域优先区分当前建筑直接引用、祖先继承，以及经共享演出或行为模板触达的间接引用。其余字段按证据逐步增补，现有构建闸门继续使用原来的判定。
