# 希望大厦容器面板与统计柱修复

用户对 `03587ca5` 的游戏反馈：标题正常，但人口、维护费、营业时间显示占位符，内容区出现默认 Title/Left/Right；柱形图位置异常；行为看起来相对正常。这不是完整行为验收，不将该包标记为完整 GOLDEN。

## 根因与改动

宿主已无业务，Asset 仍继承单体公园面板、人口和营业时间处理器。其中本地业务属性、bin 查询无法由宿主满足。此前 v3 的容器闸门只检查组件图和面板字段存在，遗漏了查询是否仍指向无业务宿主。

新增 `behavior_container_ui.rs`，默认容器仅绑定已核对的原版连接单元查询：`kMaintenance` 和存在观光组件时的 `kBlockLandmarkTourists`。人口头使用原版 `kPopulationNone`；不将宿主假装成住宅、提供未经验证的营业时间或利润数字。Tooltip 删除单体业务参数行；保留菜单原有放置身份、成本、图标、说明和总开关。F970A822→AF042E9A 原版菜单父链已回读，不含这些统计行。

这些原版处理器已经使用游戏 locale 文本，无新增硬编码界面文案。完整的利润、岗位和各类业务汇总仍是后续工作，本次不能称为完整的多业务信息面板。

统计柱的原版实现按每个 Unit 分组，并使用其 Graphics Unit 的包围盒。旧组件只继承无模型基类，缺少整栋建筑的 `00F9EFBA`，回退到很小的盒子。现在原样复制宿主的 BoundingBox 属性（含编码），不复制模型、不更改业务 Unit。构建前及最终回读均验证组件与宿主包围盒相同。此改动修正显示空间的输入；多个业务柱在游戏里的实际排列和遮挡仍需复测，不能声称引擎已将它们合并为一根整栋统计柱。

## 原版证据

- `tmp/installed-school-game-ui.js:6015–6016`：`kMaintenance` 使用 connectedUnits 的 ExpensesFromMaintenance。
- 同文件 `:6275–6279`：`kBlockLandmarkTourists` 的全部查询均为 connectedUnits/binAmount，包含游客财富分档和容量。
- 同文件 `:5856`：`kPopulationNone` 无业务查询，更新并隐藏人口头。
- `docs/source-code/SimCity-dev-ida.c:653168`：`GetUnitBoundingBoxInternal` 先初始化 [-1,1]，再读取 `00F9EFBA`。
- 保留源码 `SC/cUnitModel/FillFromProps.c:477`：使用 GetUnitBoundingBox 取得 Graphics Unit 包围盒。
- `SC/cBarGraphManager/Update.c:340–390`、`SC/cBarGraphRenderer/Draw.c:109–135`：按当前 Unit 获取包围盒并定位柱图；未沿宿主聚合。

不能从截图中的“450 高收入观光客”单独断言财富档或容量算法出错，因此本次不调整游客配方、配额或业务参数。

## 构建与数据边界

新旧包先核对 SHA-256，再比较解码资源 payload。见 [字节审计](evidence/hope-container-ui-fix-20261122.json)。希望大厦修正版共 29 资源，SHA-256 `9eb140f03e72349dae11cc9f9f45a681bbbaf2b3b681835b203720f053a536e2`。

只有 6 个资源变化：宿主 Menu2、宿主 Asset 和 4 个组件 Asset。5 个业务/宿主 Unit、模型、纹理及其余资源解码内容逐字节不变。该结论仅限资源数据，不能据此声称运行期行为等价。

希望大厦和六类组合均通过完整构建及 DBPF 回读。61 项行为回归和 13 项既有菜单契约回归通过，共 74 项；`git diff --check` 通过。编码检查曾拒绝 count=0 的 Text 数组，已改为删除不适用的字段，没有放宽原有布局闸门。新回归覆盖恢复错误人口头时拒绝构建、包围盒编码完整复制且不增加模型。

修正版随后经用户明确授权「直接撤掉之前的包重新部署，不备份」部署。已通过 `archive_test_package.py` 归档旧 `03587ca5` 包并部署 `9eb140f0` 修正版，部署文件 SHA-256 与构建一致，台账追加了授权与验收边界；其他 000 包未变。本轮按用户要求不备份存档，未启动游戏。旧包归档及工作区字节副本保留，等待用户冷启动和显示复测。部署证据见 `tmp/hope-container-ui-deployment-20261122.json`。
