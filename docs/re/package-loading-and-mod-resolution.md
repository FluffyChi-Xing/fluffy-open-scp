# Package 加载、识别与 Mod 资源解析机制

> 分析分支：`dev/rendering-optimisation`
>
> 结论分为三类：**源码/样本已证实**、**OpenSCP 当前实现**、**仍需运行时复核**。

> 2026-10-17 修订：保留旧实测为有限样本，撤回“普通 mod 同 TGI 不支持”的概括；纠正 metadata 与版本 tag 解释。详见 [最小 package 证据](package-minimal-validation-evidence.md) 及 [加载顺序调查](package-load-order-investigation-2026-10-16.md)。

## 1. 结论先行

1. 游戏加载对象的核心不是文件名，而是 ResourceMan 数据库中的资源记录；文件名主要决定扫描范围和分类。
2. `.package` 是 DBPF 容器。容器通过 `DBPF` magic、版本/索引头和 TGI（Type/Group/Instance）索引识别资源。
3. EcoGame/脚本包还需要满足额外的类型、group 和 property 约束；单纯把任意 DBPF 改名为 `SimCity-Scripts_*.package` 不会使它变成脚本包。
4. 已有证据支持三类投放位置：`SimCityData\`、`SimCityUserData\Packages\`、`SimCityUserData\EcoGame\`。但它们对应不同加载类别。
5. **新增和同 TGI 修改是不同构建目标。** dev 通用 ResourceMan 支持同 TGI 候选数据库并按 priority 降序、同级先注册者优先查找；已有覆盖失败实验不能证明游戏不支持同 TGI。retail 和具体消费路径仍需验证。
6. OpenSCP 的覆盖扫描可以展示“候选覆盖链”，但它的 `wins = 后出现者` 是工具层模型；不能单独证明游戏运行时采用同样的优先级。

## 2. 文件扫描与 DBPF 识别

### 2.1 文件层

OpenSCP 的 `src-tauri/src/package_browser.rs`：

- 只把扩展名大小写不敏感地等于 `.package` 的文件列为 package；
- 递归扫描时跳过符号链接，深度上限为 4，单目录条目上限为 4096；
- 这是 OpenSCP 浏览器的文件发现规则，不等同于游戏的完整启动扫描器。

游戏侧的磁盘布局和类别来自 [`simcity-game-architecture.md`](../blog/simcity-game-architecture.md)：

| 位置 | 主要类别 | 当前证据 |
|---|---|---|
| `SimCityData\` | 基础数据、模型、UI、渲染和区域数据 | 17 包普查与包分工表 |
| `SimCityUserData\Packages\` | 普通用户/Mod package | 游戏架构报告、OpenSCP `game_launch.rs` |
| `SimCityUserData\EcoGame\` | `SimCity*-Scripts_*.package` 等脚本/规则包 | `glassbox-engine-architecture.md`、`dev-dump-lot-placement-and-script-resources.md` |

OpenSCP 的实际部署代码在 `src-tauri/src/game_launch.rs::deploy_game_mod`：从已配置的游戏数据目录取父目录，创建 `SimCityUserData\Packages`，并把构建产物复制为安全化文件名。这证明了 OpenSCP 的默认 Mod 部署位置；它本身没有证明游戏支持在这里覆盖任意既有资源。

### 2.2 DBPF 层

OpenSCP 的 `crates/dbpf/src/package.rs::Package::open`：

1. 只读 memory-map 文件；
2. 解析 DBPF header；
3. 解析 index；
4. 按索引 entry 读取原始 payload；
5. 对 RefPack entry 解压。

`crates/dbpf/src/writer.rs::write_uncompressed_overlay` 固定写出：

- magic `DBPF`；
- major version 3；
- 28 字节索引记录；
- 每个 entry 的 type/group/instance、offset、size、flags；
- 按 TGI 排序并拒绝同一 overlay 内重复 TGI。

因此，一个“可加载 package”至少需要：

- 文件在游戏扫描范围内；
- DBPF/DBBF 头部和索引合法；
- payload 与 type 约定匹配；
- 对特殊类别，还要被对应的 ResourceMan/Property/ArgScript 解析器接受。

## 3. 游戏侧的类别识别

### 3.0 已复原的通用扫描与注册链

源码树中 `SP::SetupResources` → `anonymous::RegisterPackages` → `SP::AddPackageIfValid` 提供了比文件名实验更直接的证据：

- `RegisterPackages` 对传入目录调用 `EA::IO::DirectoryIterator::Read(..., L"*.package", ..., 0x100000)`，然后为每个结果创建 `DatabasePackedFile`、设置完整路径并打开；该函数本身没有显示递归调用，因此至少这条主资源链是“按目录直接枚举”而不是 OpenSCP 的递归浏览规则。
- `SP::SetupResources(dataRoot)` 以 priority `0` 注册数据根目录中的 package，并单独以 priority `1` 注册 `Preload.package` 的内存/文件流数据库。
- `SP::AddPackageIfValid` 先计数 package-info group 的 Property：0 条直接按调用方 priority 注册，1 条检查签名、条件授权/能力字段和可选 Bless 信息，多于 1 条拒绝。通过后调用 `ResourceMan::Manager::RegisterDatabase`。因此“DBPF 可打开”与“被主资源管理器接受”是两个不同门槛。
- `GB::InitGameVersions` 另外从 `SP::GetDirFromID(0x8F03C83)`，失败时回退到 `0x8F03C82`，枚举 `*.package`，将每个包交给 `AddGamePackage`；无效脚本包会被关闭并尝试删除。`AddGamePackage` 只接受包含指定 Property/TGI 结构、Ecosystem group 和 UInt32 版本字段（类型 tag 10，非版本值 10） 的数据库，并把 database 放入 game/version 表。

这条链解释了为什么“改扩展名”不能让普通 DBPF 变成 EcoGame 脚本包，也解释了为什么 priority/类别来自注册调用方，而不是 TGI 本身。`0x8F03C82/0x8F03C83` 对应的产品目录 ID 尚未从当前源码树映射为公开目录名，标记为 `[TODO]`。

### 3.1 数据库访问与脚本包注册示例

`EA::ResourceMan::Database` 是游戏侧资源数据库抽象。反编译树中的 `anonymous::AddGamePackage`（地址 `0x5ED2B0`）显示：

- 打开数据库；
- 使用 `GetKeyList` 和 `SP::cKeyFilter` 按 type/group 筛选资源；
- 对候选 key 调用 `OpenRecord`；
- 读取 property list；
- 再把 database 绑定到 game/version 映射。

这说明“包被扫描到”与“其中的资源被某个系统接受”是两个阶段。

### 3.2 EcoGame/脚本包

`anonymous::AddGamePackage` 的可验证约束包括：

- type filter 为 `0x00B1B104`（Property）；
- group 使用 `kEcosystemGroup + i`，循环两个 group；
- 取首个匹配 key 的 instance 为 game ID；
- 读取 property `166637852` 作为版本字段；
- `EA::Variant` 类型 tag 必须为 10（UInt32），随后调用 `operator<unsigned int>` 读取实际版本值，再把 gameID/version/database 放入版本表；这里没有要求版本值等于 10。

`GB::cEcoGameScripts::Init` 及相关 `AddGlobalRules`、`AddBundleGlobalBins` 等函数进一步从脚本/属性表建立 unit、resource、map、zone、path、point、sink、transport、bundle 等表。由此可知：

> 脚本包可加载 ≠ 文件名正确；它必须同时提供引擎期待的资源类型、group、property 结构和规则数据。

### 3.3 规则脚本

现有报告将规则链确定为：

```text
EcoGame package
  → ER2 文本规则 / property 资源
  → ERZ 编译规则库
  → 主规则库 instance 622B9CD7
  → 存档中的状态投影
```

`0x08068AE9` 是抽象类型，`0x08068AEA/0x08068AED` 等属于规则实现/编译数据族。规则包的消费路径是 EcoGame/ArgScript/规则库，而不是普通 UI JavaScript。

## 4. “覆盖”到底是什么

### 4.1 OpenSCP 的覆盖模型

`src-tauri/src/overrides.rs::scan_overrides`：

- 按 roots 顺序收集 package；
- 每个 package 按目录字母序递归收集；
- 以 `(type_id, group_id, instance_id)` 为 key 分组；
- 同 TGI 出现多次时，把链尾标为 `wins`。

该模型适用于“发现冲突”和“生成候选覆盖链”，也适用于 OpenSCP 自己的资源集合视图。

### 4.2 游戏行为的限制

旧报告记录了三类位置和文件名实验：后投放 package 未稳定替换目标 TGI，并定位过 `FUN_00403FB0`。这些是特定样本的阴性结果，缺少注册 priority、枚举次序、缓存与消费路径记录，不能推广为普通 Mod 不支持覆盖，也不能证明新增 TGI 必然被消费。

新的 dev 静态证据为：`InternalDatabaseList::Add` 按 priority 降序且同级稳定插入；`Manager::FindDatabase` 返回首个命中数据库。因此该路径支持同 TGI 竞争，并非简单“后来覆盖前面”。实际 retail 构建及专用管理器、缓存、打开失败的处理仍需对拍。新增资源也必须更新引用或注册表，不能仅凭入包判定生效。

### 4.3 正确的 Mod 构建原则

1. 先确定目标系统：普通资源、Property、EcoGame 规则、区域资源或 UI。
2. 新增资源时分配不冲突的 TGI；修改已有资源时保留目标 TGI 并核验查找优先级。
3. 对新增资源更新 Property/规则/注册表引用；对已有资源修改序列化完整资源，而非只写字段差分。
4. 对脚本包遵循 EcoGame 的 group、版本和规则表结构。
5. 用 OpenSCP 做 TGI 冲突扫描，但把结果标为“候选覆盖”，不要直接称为游戏最终生效结果。
6. 在干净游戏目录、仅放入一个 package 的条件下验证资源是否被消费。

## 5. 与当前 OpenSCP 实现的差异

| 问题 | OpenSCP 当前行为 | 游戏侧结论 |
|---|---|---|
| 哪些文件被浏览 | `.package` 扩展名、递归深度/数量限制 | 游戏实际扫描器规则更复杂，需运行时/路径初始化进一步确认 |
| TGI 冲突 | `override_scan` 以最后链项为 wins | dev 通用路径高 priority 优先、同级先注册优先；retail 待核验 |
| 写 package | 写确定性、未压缩 DBPF overlay | 格式正确不等于该资源类别会被引擎消费 |
| 部署位置 | 默认复制到 `SimCityUserData\Packages` | 适合普通 Mod；EcoGame 脚本需要单独目录/类别 |
| 源包修改 | overlay 写出，不覆盖源文件 | 可构建同 TGI 候选 overlay；最终命中须按目标加载链验证 |

## 6. 仍需验证的关键点

- `[已确认]` 通用主资源链的目录枚举、`*.package` 过滤、Preload priority、package-info 校验和 ResourceMan 注册已由 `SP::SetupResources`、`RegisterPackages`、`AddPackageIfValid` 复原；`FUN_00403FB0` 仍需与这条新源码链逐一对地址，确认它是否是同一版本/同一入口。
- `[TODO]` `0x8F03C82/0x8F03C83` 的目录 ID 到实际安装目录名称的映射尚未在当前源码树中确认。
- `[dev 静态已确认 / retail 待验证]` 通用数据库 priority 降序，同级先注册者在前；FindDatabase 首命中。具体消费者、缓存和失败回退需要进一步静态追踪与最小运行时测试。
- `[TODO]` `SimCityUserData\Packages` 与 `EcoGame` 的实际扫描时机是否不同，需用启动日志/文件探针确认。

## 证据清单

- `src-tauri/src/package_browser.rs`
- `src-tauri/src/overrides.rs`
- `src-tauri/src/game_launch.rs`
- `crates/dbpf/src/package.rs`
- `crates/dbpf/src/writer.rs`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\anonymous\AddGamePackage.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\anonymous\RegisterPackages.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SP\SetupResources.c`
- `D:\ea-games\simcity_dev\new-cource\SimCity2013-source-tree\src\SP\AddPackageIfValid.c`
- `docs/blog/simcity-game-architecture.md`
- `docs/blog/glassbox-engine-architecture.md`
- `docs/re/dev-dump-lot-placement-and-script-resources.md`
