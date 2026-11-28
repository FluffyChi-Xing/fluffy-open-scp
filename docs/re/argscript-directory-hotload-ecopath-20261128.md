# 社区线索核对：开发目录、热加载与 Eco Path

日期：2026-11-28。用户提供 Danny50205 的社区引文，并称其为 Maxis 前成员。本文核对技术内容；未独立验证作者身份、发言时间、原始上下文。引文中的操作建议是待核实线索，不构成更改当前游戏配置的指令。本轮没有修改游戏、部署包或连接运行期探针。

## 开发目录资源与热更新：有直接代码依据

开发版反编译 `SP::InitResourceHierarchyHotload`（EA 0x4A2670）执行：

1. 取得资源管理器；在传入路径后追加 `GlobalMap.txt`，调用 `ReadGlobalTypeList`。
2. 创建 `EA::ResourceMan::DatabaseDirectoryFilesAutoUpdate`，以该路径初始化。
3. 调用 `SetDDFAutoUpdate`，注册 `OnDatabaseRecordChanged` 通知。

`DatabaseDirectoryFilesAutoUpdate::Init`（EA 0x8D44D0）检查目录存在，设置文件变更回调及被监视目录，然后启动文件变更通知。与引文的开发目录和热更新描述相符。

但这两段函数没有证明：零售版开放同样入口、任意目录即能替代 SimCityData、目录内 ER2 源文会自动编译、或者特定的 `Content/code/ArgScripts/Eco_RCI/...` 路径就是所有版本的约定。`Scripture Sutra` 工具的实现和实际可用性也尚未核验。目录资源数据库、类型映射、具体资源工厂、源脚本编译是不同环节，需要逐段追踪。

对 UnitRule 研究的意义：继续区分通用 `EA::ArgScript` 词法/元命令、Eco 领域命令处理、编译后的规则记录，以及运行期资源加载。运行期 `cEcoUnitRule` 构造默认值不自动等于领域编译器在省略声明时写出的默认值。当前未证明领域编译器在游戏内还是外部工具内，不能据引文填补默认次数等未知契约。

## Eco Path：过滤 PathEntry 类别有直接代码依据

`GB::GetPathFilter`（EA 0x616F70）按 filter ID 查索引，返回对应 `cEcoPathFilter.mActivePathEntries`，类型为 256 位 bitset；查不到 ID 时返回全零。

`GB::cPathSearchInfo::FilterPath`（EA 0x644570）的判定是：

```text
全部 256 位为零 → 此函数放行
否则 → 路径 mEntryIndex 必须小于 256，且对应位为 1
```

因此“允许哪些 agent 走哪些路径”的描述有机制依据，但本段代码的直接对象是 PathEntry 类别。还需核对 agent/运输处理器选择哪个过滤器、实际路径的 entry index，以及其它搜索条件。不能把它描述为每条可见道路上直接存有车辆白名单，也不能将空过滤器解释成拒绝所有车辆。

公交专用路径的后续证据链应为：原版公交运输处理器 → 实际过滤器 ID → PathFilter 允许集合 → PathEntry 定义 → 道路生成出的路径类别 → 搜路和通行结果。行人路径 Mod 可以提供对照样本；本轮没有取得并验证该 Mod，不能据引文宣布公交专用路径已实现。既有公交研究见 [市政公交运输分析](municipal-bus-transport-20261113.md)。

## 对新行为设计的影响

以邮件为研究案例，需要分开验证：UnitRule 中资源/控制/调度/派遣与接收；agent/运输处理器及 sink；可达性与路径过滤；Unit、规则集和资源的加载装配。路径允许车辆通行，本身不会产生邮件需求、派车或幸福度变化；这些还需要规则与原生消费者共同完成。

## 证据身份与复核范围

以下均为开发版 CodeDumper 反编译单函数文件，根目录为 `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/`。地址仅用于开发版定位，不是零售版探针地址。验证方式为静态阅读及 SHA-256 身份记录；没有进行目录加载或热更新实测。

| 文件 | SHA-256 |
| --- | --- |
| `src/SP/InitResourceHierarchyHotload.c` | `47dd0ac9ad014bfd5a395a45e69d4999b4b244ec15e7a98ab7531f859a22f6ac` |
| `src/EA/ResourceMan/DatabaseDirectoryFilesAutoUpdate/Init.c` | `8070ca658f64c515244378b5b7afeb62f785db468e681080ba750f9f149a9dc6` |
| `src/GB/GetPathFilter.c` | `1f7b46c67187b950a3c43a787127080d88577e1eea3089dd18983a628004c4c7` |
| `src/GB/cPathSearchInfo/FilterPath.c` | `3df036881c9fc53e11c2e9e324e15c4cbb8ad63b2a5a65f167d567c5f65fc272` |
