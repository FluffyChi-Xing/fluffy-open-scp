# Package 合法性校验、完整性与地图编辑依赖闭包

> 分析分支：`dev/rendering-optimisation`
>
> 本文回答两个问题：OpenSCP 是否能增加 package 导出/完整性/流程转译支持，以及地图词条、城市槽位、地图资源编辑分别需要哪些 package 和 TGI 依赖。

> 2026-10-17 修订：纠正 package-info 必需性与版本 tag 含义；新证据见 [最小 package 与完整性证据](package-minimal-validation-evidence.md)。dev 为带符号反编译树，不能直接代表 retail。

## 1. 结论

可以增加，但需要把“容器合法”“资源语义合法”“游戏可注册”“编辑闭包完整”分成四层。当前实现只稳定覆盖第一层的一部分和确定性写出；不能把 `write_uncompressed_overlay` 成功等同于游戏一定会加载。

建议新增一个面向流程引擎的 `PackageValidationReport`，至少输出：

1. **结构检查**：DBPF/DBBF magic、版本、保留字段、索引范围、entry offset/size、压缩标志、重复 TGI、文件截断。
2. **资源检查**：Property、F0/ED 地图 tile、Locale JSON、RW4、ERZ/ER2 等按 TGI 类型调用对应解析器，并报告尺寸/字段类型错误。
3. **依赖检查**：从编辑目标 TGI 出发反查 Property Key、locale string、模板、城市状态和伴随 tile，生成缺失/外部依赖列表。
4. **游戏配置检查**：区分普通 overlay、数据根 package、EcoGame 脚本包和带 package-info/signature 的 add-on；不能用一个宽松规则覆盖所有类别。

导出流程应使用“校验 → 写出 → 重新打开 → 逐 TGI 比较 → 记录 SHA-256/依赖闭包”的事务顺序。失败时不激活 `build.json`，避免生成一个格式正确但语义不完整的包。

## 2. 当前 OpenSCP 能力与缺口

### 2.1 已有能力

- `crates/dbpf/src/header.rs` 检查 DBPF/DBBF magic、头部长度和 reserved 字段；读取版本字段但尚未校验其兼容性。
- `crates/dbpf/src/index.rs` 解析共享 TGI 字段、offset、压缩标志和大小；`Package::read_raw/read` 检查 payload 是否越过文件边界，并支持 RefPack 解压。
- `crates/dbpf/src/writer.rs` 输出 major version 3、确定性排序的未压缩 DBPF overlay，并拒绝同一输出内重复 TGI。
- `src-tauri/src/mod_flow.rs` 已有输入 SHA-256、schema/graph staleness 检查、输出 artifact SHA-256、写出后重新打开并比较资源的流程。
- `crates/sc-properties` 已能解析 Property、区域 F0/ED、高度、资源画刷、区域模板、Game 注册表和 Locale 字符串。

### 2.2 当前缺口

- `Package::open` 主要验证“能否解析”，没有公开一份逐项诊断报告，也没有检查 entry 之间的重叠、索引声明长度与实际记录数的关系、重复 TGI 或 payload 是否落入索引区。
- writer 只写未压缩 overlay，没有生成完整 package-info、签名或 EcoGame 所需的脚本版本结构；因此它适合“资源 overlay”配置，不能自动宣称是可安装 add-on 或脚本包。
- `mod_flow` 中 `city-slots`、`resources`、`roads` 和部分 `map-metadata` 节点已进入 schema/图校验，但当前构建器对这些节点主要保留源数据，尚未实现完整的语义写入器。现有文档也明确列出 slot 增删、资源画刷、道路编辑、新地图注册和完整区域模拟尚未实现。
- 同 TGI 的最终游戏优先级仍不能由 OpenSCP 的 override 扫描模型推导；校验报告应把冲突标为风险与候选覆盖，而不是“已生效”。

## 3. 推荐的校验分层

### 3.1 ContainerProfile

输出前先选择 profile：

| Profile | 适用目标 | 必须检查 | 当前状态 |
|---|---|---|---|
| `overlay` | `SimCityUserData\\Packages` 普通资源 overlay | DBPF 结构、TGI 唯一、资源语义、依赖闭包 | 可在现有 writer 上扩展 |
| `data-root` | `SimCityData` 类基础数据包 | overlay 检查；存在 package-info 时才检查签名/priority 分支 | dev 与指定 retail dump 的 metadata 分支已对拍，运行时待验证 |
| `ecogame` | `SimCityUserData\\EcoGame` 脚本/规则包 | Property type `0x00B1B104`、Ecosystem group、版本字段为 UInt32（类型 tag 10，非版本值 10）、ER2/ERZ 结构 | 解析器和最小样本仍需补齐 |
| `addon` | 带 package-info 的 add-on | 唯一 metadata、签名、条件授权/能力和可选 Bless 字段 | 不应由普通 overlay writer 默认生成；无 metadata 的普通包也可走 AddPackageIfValid |

dev `SP::AddPackageIfValid` 对匹配的 package-info Property 数量明确分支：0 条直接采用调用方 priority 注册，1 条走签名和条件授权检查，多于 1 条拒绝。因此最小普通 overlay 不需要为了通过此入口而复制或伪造官方 metadata。注册接受仍不等于目标资源最终命中。

`SP::SetupResources`、`anonymous::RegisterPackages`、`SP::AddPackageIfValid` 和 `GB::InitGameVersions` 已证明这些加载类别存在不同入口；因此 profile 必须进入流程中间模型，而不是只作为文件名选项。

### 3.2 完整性检查结果

建议诊断代码固定为可机器消费的枚举：

- `header.magic`, `header.version`, `header.reserved`
- `index.truncated`, `index.out_of_file`, `index.overlap`, `index.duplicate_tgi`
- `payload.compression`, `payload.refpack`, `payload.decoder`
- `semantic.property`, `semantic.locale`, `semantic.raster`, `semantic.rw4`, `semantic.erz`
- `dependency.missing`, `dependency.external`, `dependency.profile_mismatch`
- `output.reopen_mismatch`, `output.hash_mismatch`

校验报告应同时保留原始 TGI、来源 package、解析器版本和输入 SHA-256，这样流程引擎可以判断“源文件变了”与“解析器升级了”两种不同的 stale 情况。

## 4. 地图编辑依赖闭包

### 4.1 地图词条/区域与城市介绍

| 编辑对象 | 主要资源 | 关键关系 | 依赖 package |
|---|---|---|---|
| 区域数字 ID | Property `0x00B1B104`, instance `0x51E7A18D`，字段 `0x4EE97F5B` | 区域 group → 数字 ID | `SimCity_RegionTerrain0/1.package` |
| 区域模板名/城市 UID | Locale/模板 JSON type `0x0A98EAF0`, instance `0x4529F96F` | `map.cities[].uid`、`map.greatWorks[]` 与 bootstrap environment join | `SimCityDataEP1.package` |
| 区域/城市显示名 | Game 注册表 JS type `0x67771F5C` | `regionName:"AutomatedRegionTemplates.json!0x..."`、`cityName:...` | `SimCity_Game.package` |
| 中文/本地化文本 | Locale type `0x0A98EAF0`，表 instance 为 `FNV1_lower("automatedregiontemplates")` | string ID → 文本 | 对应语言 Locale package |
| 城市介绍和资源摘要 | 同一 Game 注册表 JS 中的 `cityDescription`、`percentageBuildable`、`wind`、`water`、`coal`、`rawOre`、`crudeOil`、`rail`、`shipping` | 城市 string ID → Locale 文本/数值字段 | `SimCity_Game.package` + Locale package |

因此，词条编辑不能只改 RegionTerrain 的一个 Property。最小闭包应包含区域描述、EP1 模板、Game 注册表和目标 Locale 表；缺少后两者时可以保存数值，但游戏 UI 名称可能继续显示旧文本或空值。

### 4.2 城市槽位、伟大工程位和静态城市

区域包中的城市组通过 Property 引用区域描述：

- `0x00B1B104` 小型 Property 的字段 `0xC1949C4D` 指向区域描述 instance `0x51E7A18D`；group 是城市/地块身份。
- `0x00B1B104` instance `0x2B9C480C` 的字段 `0x16B7B1EF` 保存城市 ID 数组，`0xF01DE4B1` 保存对应世界坐标。
- `SimCityDataEP1.package` 的 bootstrap group `0x95F4AEF0` 使用 `0x46066280`（静态状态 Key 数组）、`0x46066281`（UID 数组）和 `0xF240A8CA`（模板 metadata Key）绑定区域与各 site。
- 静态状态 Key 的 type 是 `0x08068AED`；site `0` 是区域级网络/曲线的权威来源，其他 site 是城市状态。当前解析器还验证 environment、UID、城市数量和 128² map 尺寸。

因此城市槽位编辑至少需要同时更新“区域包城市组”和“EP1 bootstrap/静态状态”。只改坐标数组会导致预览看似移动，但静态城市环境、存档绑定或区域网络仍指向旧 site。新增/删除槽位还需要生成匹配的 `0x08068AED` 状态资源，目前没有安全 writer，应先做只读闭包和差异实验。

### 4.3 地图高度、地面资源与资源画刷

| 编辑对象 | 资源族/字段 | 现有实现 |
|---|---|---|
| 高度 | type `0x03E421F0`，每区域 group 341 个 256² tile，五级 mip 组合成 4096² @ 8m | 已支持高度偏移、噪声和 grayscale 输入写出 |
| 地面/生态/资源场 | type `0x03E421ED`，每区域 group 341 个 128² tile，A8R8G8B8 通道；当前实现按 b0/b1/b2 解读地下水/森林/土壤 | 已支持读取和预览；完整资源 brush writer 尚未实现 |
| 资源刷 | Property `0x00B1B104`，字段 `0x00B2CCCA` 为 `*brushes` 名称，`0x02A907B6` 为 Transform stamp 数组 | 已支持枚举名称和坐标，未支持语义编辑 |
| 水位 | 区域描述字段 `0x0E16BE1A` | 已支持读取/预览覆盖参数 |
| 可见区域窗口 | 区域描述字段 `0x5E3D16DF`，经校验映射到 8192/16384/32768 等中心窗口 | 已支持读取和裁剪预览 |

高度和 ED 资源必须保持同一 group、tile identity 和尺寸关系。只改高度而不更新 ED 会产生地形/生态不一致；只改 ED 而不更新 brush/property 可能只改变预览通道，未必改变游戏模拟资源。

## 5. 流程引擎节点建议

### 第一阶段：只读闭包和导出门禁

新增 `dependency-closure` 服务，输入 `source_package + region_group + target`，输出：

- 目标 TGI 与反向引用；
- 必须随包导出的 TGI；
- 外部运行时依赖（Game、EP1、Locale、EcoGame）；
- 缺失资源、profile 冲突、未实现 writer 的阻断项。

把 `output` 节点的 `profile`、`validation_level` 和 `allow_external_dependencies` 写入 IM。普通 overlay 可以允许外部 Game/Locale 依赖；“自包含地图包”必须把依赖列为错误或明确打包。

### 第二阶段：实现安全写入器

优先顺序建议为：

1. 区域词条的新增 string/TGI 引用（不重排已有城市）；
2. 高度与 ED 的成对编辑；
3. 现有槽位坐标移动；
4. 资源画刷的可逆替换；
5. 槽位增删、静态状态生成和新区域注册。

每个 writer 都必须声明写入的 TGI 集合、保留的 TGI 集合和未覆盖的外部依赖。新增/删除槽位和 EcoGame 状态在没有最小运行时样本前应保持只读或阻断。

### 第三阶段：游戏行为验证

对每个 profile 固定三组实验：单 package、同 TGI overlay、全新 TGI + 引用更新。收集启动日志、资源可见性、UI 名称、城市载入和存档重载结果。只有运行时实验能把“结构合法”升级为“游戏行为已确认”。

## 6. 当前建议

近期最有价值的实现不是立即增加一个“万能导出”按钮，而是先把校验结果和依赖闭包接入现有流程的 `inspect/build` 门禁。这样可以马上提升流程节点配置的准确性，同时不会把尚未确认的同 TGI 覆盖、槽位增删或 EcoGame 写入误报为成功。

## 证据清单

- `crates/dbpf/src/header.rs`
- `crates/dbpf/src/index.rs`
- `crates/dbpf/src/package.rs`
- `crates/dbpf/src/writer.rs`
- `src-tauri/src/mod_flow.rs`
- `docs/mod-development-flow.md`
- `crates/sc-properties/src/region_map.rs`
- `crates/sc-properties/src/region_3d.rs`
- `crates/sc-properties/src/region_city.rs`
- `crates/sc-properties/src/region_bootstrap.rs`
- `crates/sc-properties/src/locale.rs`
- `docs/re/package-loading-and-mod-resolution.md`
- `docs/re/map-save-package-dependencies-and-boundaries.md`
