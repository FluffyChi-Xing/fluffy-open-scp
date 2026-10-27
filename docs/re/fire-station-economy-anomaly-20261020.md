# 消防局经济修改异常：静态调查与对照包

## 结论边界

用户复现：把消防局复制到道路菜单，将价格从 20000 改为 20。异常包已丢弃。截图出现 `~population_amount:number~`、`~maintenance_cost:simoleons~`、`Left1/Right1` 等未绑定模板内容。

目前未确认游戏运行时根因。静态证据确认名称与经济编辑走不同资源链；对最新原版消防局进行同等价格补丁，资源格式和引用并未损坏。不能把截图直接解释为价格类型错误，也不能用自家解析器往返成功宣称游戏行为正确。上轮数量编辑的自动化验证不覆盖游戏内初始化和存档兼容性。

用户进一步确认：此前 A（仅复制菜单）、B（加入不改值的模拟资源）均正常；异常包由开发启动的项目生成。因此不再要求重复 A/B，也不把安装版 exe 较旧或“加入模拟资源本身”当作解释。当前游戏安装的包只有菜单复制与改名，不是异常现场。

## 已验证的详情更新中断机制

零售提取脚本 `tmp/ui-bridge-retail/game-7CCC548C.js` 的 `Private_BuildDataBindingResults` 在必需查询返回 `null` 时返回 null，`RequestDataBindingData` 随即不调用整批回调。`cRolloverUI.UpdateItems` 用此接口更新多个详情组件。因此单条查询缺失可以使多个模板一起保持未更新。

`kMaintenance` 查询 `connectedUnits/<unitref>/bins/4227066323`，没有开启 `allowNullResults`。组件自身的零值兜底发生在更新回调中，不能绕过前面的拒绝逻辑。该查询读取模拟中的维护费资源 bin，不是直接显示价格或维护费属性。

`tools/audit-fire-rollover.cjs` 仅在隔离 VM 执行提取出的绑定定义和批处理函数，模拟查询响应，验证：

- `null` 阻止回调；`{amount:0}` 允许回调。
- 同批含正常结果与 null 时，仍取消整批回调。
- 断言内容为 `Null data binding data: maintCost\nPublishing scripts may fix this.`。

运行：`node tools/audit-fire-rollover.cjs tmp/ui-bridge-retail/game-7CCC548C.js`。带源文件 SHA-256 的输出保存在 `tmp/fire-station-economy-audit/retail-rollover-binding-audit.json`。

这是已验证的失败机制，不是已捕获的异常现场；不能认定当时一定是 maintCost 缺失，其他必需查询或 JavaScript 异常仍需运行时区分。

## 实际导出调用与原生参数消费

`src/stores/uiWorkbench.test.ts` 的消防局复制道路菜单回归用例经过真实 store 导出编排、模拟 native 边界：只改价格时，菜单导出请求不变，唯一 Unit patch 为 `E917279C / 40E0C000 / Float 0x0A8A41DA = 20`；不夹带维护费、统计或数量修改。该轮 17 项 store 测试及 vue-tsc 通过。此测试没有验证历史异常包或真实 native 运行。

开发样本反编译 `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c`：

- `cToolPlop::FillFromProps`（1027811 行附近）按 Float 读取价格、累加子单元费用后转整数；资金判断使用此费用。已查路径没有价格最低阈值控制详情显示的分支。
- `cEcoUnitEntry::FillFromProps`（699918 行）先处理自身 Unit，再加载 bundle；`AddBundle`（699034 行）调用资源 bins 初始化并注册规则。不能从此推导所有编译模拟规则都不消费价格。

以上原生线索来自开发样本，不能直接当作当前零售进程执行轨迹。

## 已核实的资源链

| 层 | TGI / 字段 | 证据 |
| --- | --- | --- |
| 原消防局菜单 | `00B1B104:09878A01:6D0235EE` | `SimCity_Game.package` |
| 菜单指向模拟建筑 | `0x0975695F -> E917279C`（type/group 为 0，需按领域解析） | 原菜单 dump |
| 模拟建筑 | `00B1B104:40E0C000:E917279C` | `SimCity-Scripts_287520926.package` |
| 造价 / 维护费 | Float `0x0A8A41DA = 20000` / Float `0x09AE19D7 = 400` | 原模拟 Unit dump |
| 模拟父模板 | `00B1B104:48E0C000:BD7BF4A1` | `0x00B2CCCB` |
| 非模拟引用 | `0x08E17ED7 -> 00000000:40E1C000:E917279C` | 模拟 Unit 的显式引用 |
| 模型及界面侧建筑 | `00B1B104:40E1C000:E917279C` | `SimCity_Game.package`；与模拟 Unit 共 instance，但 group 不同 |

两套本机安装的 `SimCity-Scripts_287520926.package` SHA-256 一致：
`77DDCED4A23A818673E44C19649732390D8771BC5A1434583032E5841BAEDD04`。

名称编辑写菜单的 Text 引用及 locale 文本。价格编辑额外写模拟 Unit；数量编辑额外写模拟 Unit / C700 容量定义。复制菜单仍指向原来的 Unit，并未生成独立建筑，因此经济覆盖也影响引用该 Unit 的原菜单，不能宣传为“复制后建筑行为互不影响”。

## 字节核对结果

`probes/property/building_economy_audit.rs` 使用原版模拟 Unit，采用与后端相同的 PropertyFile 编码器：

- 原资源解压后 1167 字节；不改值的解析/编码结果逐字节一致。
- 价格改为 20 后长度仍为 1167；仅偏移 `0x0162:46→41`、`0x0163:9C→A0`、`0x0164:40→00` 不同。
- 所有非价格属性逐项相等，父模板、非模拟引用、维护费和数组均未变化。
- 输出 DBPF 重新读取后 payload 与输入相同。

该结果排除了本次最小静态重建中的 Float 类型、整表丢字段、引用丢失问题；不能追溯证明已经丢弃的旧异常包也只有这三个字节变化。

## 代码中存在但不能冒充本例根因的问题

1. `src/stores/uiWorkbench.ts` 经济导出只用 instance 调用 `gamePropertyRead`，没有领域约束；`pick_property_candidate` 取路径数字版本最大者。它不是游戏实际加载优先级或已选脚本版本。消防局当前候选中最新脚本会选择正确的 `40E0C000`，所以此问题尚不能解释本例。
2. 无经济字段时，导出会改父模板。父模板可能被多个建筑共享，正确的单建筑覆盖应保留叶子身份。本例消防局叶子已有价格与维护字段，不经过此分支。
3. 所有统计参数一律当 Float 写入，缺少字段类型验证；用户此次只修改价格，不能把该问题当成此次已证实原因。

## 加载链线索（尚待零售运行时对拍）

沿用现有逆向资料 `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c`：

- `FindNonSimProps`（654306 行）先读取 `0x08E17ED7`；无法取到非模拟属性时回退模拟属性。模拟/非模拟绑定失败是需要排查的具体分支，但未证明截图走了此分支。
- `AddEntitiesFromGroupIDs`（662001 行附近）按资源枚举顺序建立 Unit 表及 ID→index 映射。
- `Manager::GetRecordKeyList`（1212167 行）跨数据库去重，并按数据库/条目遍历顺序输出；不能误称同 TGI 会重复注册成两栋建筑。覆盖是否改变枚举顺序及现有存档索引，需实际观测。
- `cPropertyManager::GetPropertyList`（345694 行）按 group+instance 缓存，首次加载时更新父引用。要检查实际命中源和加载时序，不能只看安装目录里有无文件。
- 既有零售加载证据 `package-runtime-verification-20261017.md`：普通 Packages 注册 priority 0、选中的 EcoGame 脚本 priority 100。一般资源 overlay 的验证不能自动推广到脚本模拟资源。

这些资料解释了为什么“包能解析、游戏不崩溃”不保证建筑界面绑定和模拟状态正确，但不构成此次异常的实机归因。

## 已生成的隔离对照包

目录：`tmp/fire-station-economy-audit/`，没有写入安装目录、替换现有包或操作运行中的游戏。

菜单样本来自当前安装仍存在的 `SimCityUserData/Packages/OpenSCP-ui-overlay.package`，取其中道路菜单消防局副本 `F8196D0D`。该现存包不含模拟 Unit，不能把它称为用户丢弃的异常包。只提取这一条菜单，不带其他副本和图片。

| 包 | 内容 | 判别用途 |
| --- | --- | --- |
| `A-menu-only.package` | 仅道路菜单消防局副本 | 检查纯复制行为 |
| `B-unit-unchanged.package` | A + 完全原样的模拟 Unit（20000） | 检查加入模拟覆盖本身的影响 |
| `C-price-20.package` | A + 仅价格改为 20 的同一模拟 Unit | 检查数值变更相对 B 的影响 |

用户已确认历史 A/B 正常，不要求重做。上面生成的具体文件尚未由本轮实机验证。

下一项所需证据是重新出现异常时的完整查询/响应与控制台错误。优先使用开发项目重新导出的价格 20 包并保留其原件；若无法重现，再用 C 区分当前导出与最小补丁。应记录实际扣款和异常建筑的 unitref，使用临时城市避免混入旧存档状态。

已有观察器可在异常现场运行：`python tools/dynamic/ui-observer/native_trace.py --pid <当前游戏PID> --profile bridge-commands --seconds 60 --directory <新取证目录>`。就绪后打开异常消防局详情，定位 `Null data binding data` 或 JavaScript 异常，并将 query 与 result 的 callSequence 配对。截断/未解码对象不能判作 null。观察器会临时安装 Frida hooks 并自动撤回，不是严格意义的纯只读进程访问。

本轮没有替换游戏包、重启游戏或安装观察器 hooks，故不宣称已修复。生产导出逻辑未修改，新增内容为取证探针、导出回归测试和本报告。
