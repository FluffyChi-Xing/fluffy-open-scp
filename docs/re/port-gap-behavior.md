> **总入口**：`docs/re/engine-contracts-and-port-status-20261111.md`。本文为行为/灯光/菜单逐函数缺口清单。

# SC5 Asset Packager 0.8.0 → Rust 行为 / 灯光 / 菜单 / 地块链路迁移缺口审计

- 日期：2026-11-11
- 范围：只读调查。基准 = `reference/sc5-packager-0.8.0/`；对照 = `src-tauri/src/asset_build_m1/`。
- 方法：逐文件读 Python（全部 ≤ 131 行，逐行核对），对照 Rust 实现与单元测试；用自写解析器
  （临时目录、只读）dump 了 `resources/sc5/*.bin` 的逐字段值做数据层对拍。
- 未运行构建、未运行基准仓库里的任何 Python、未改 Rust。

---

## 0. 结论摘要（按严重度）

**最重要的一条**：Rust 的「公园」行为**不是**基准的 `native-park-w1-v1` 配方，而是基准
`legacy-display` 所用基座（`engine/tested-unit.bin`，SHA-256 与基准清单一致）＋我方自有覆盖。
基准 PARK 配方的核心——从 `00B1B104:40E0C000:59292C11` 起 8 跳 parent 链克隆、46 个资源哈希
校验、12 个效果列专化——**在 Rust 中完全不存在**。这条不是"细节偏差"，是配方层面的缺失。

| 级别 | 数量 | 主题 |
|---|---|---|
| P0 必须修 | 4 | 公园 Unit 配方未克隆（含 12 效果列缺失）；`verify()` 回读校验缺失；内景控制器图形单元只写了一半 |
| P1 必须修 | 6 | 菜单排序数组长度/扫描算法；菜单目录与原版 donor 校验；图标 128 归一化；侧车清单来源校验；灯光名/夜阈值字节偏差 |
| P2 建议修 | 12 | LEGACY 类别值、08E17ED7 类型、09FB78CB、道具 marker、LINE/校验范围、prop 校验、报告证据字段等 |
| 等价 | 17 | 见 §6 |

---

## 1. 基准身份与证据边界（先验哈希）

### 1.1 基准包含什么、不包含什么

`reference/sc5-packager-0.8.0/` 是 **0.8.0 源码快照，但 `engine/` 目录未随包提取**。
`source-manifest.json` 记录了 19 个 engine 文件与 4 个研究模板的 SHA-256，但文件本体不在目录中：

```
engine/build_candidate.py        3665da1e…   ← 菜单/Unit 物化、asset_bool_overrides 的实际写入者
engine/interactive_park_profile.py 4569b60c…  ← Asset 字典 / menu / unit 的 profile 工厂
engine/prepare_asset.py          127bde4c…   ← 几何/材质/内景
engine/collision_proxy.py        07291fec…
engine/tested-unit.bin           21b3fcd0…
```

因此本报告凡是涉及 `build_candidate` / `interactive_park_profile` **内部**行为的结论，均标注
「engine 缺失，无法核实」；能核实的都以仓库内 Python 源码行号为准。

### 1.2 `tested-unit.bin` 的身份（关键证据）

| 文件 | SHA-256 | 结论 |
|---|---|---|
| `reference/.../source-manifest.json` 记录的 `engine/tested-unit.bin` | `21b3fcd097db6b83003a6368b4b33ac8ab5d8b5949937493c77f7d35f3b58476` | 基准文件 |
| `src-tauri/resources/sc5/tested-unit.bin` | `21b3fcd0…`（同上） | **字节相同** |

`prepare_engine.py:52-57` 说明它的来源：

```python
baseline = RESEARCH / '交互朝向研究0.7.0/构建核验'
r = json.loads((baseline / '候选包验证报告.json').read_text(...))
unit = tuple(int(x, 16) for x in r['identities']['unit'].split(':'))
(DEST / 'tested-unit.bin').write_bytes(next(e.read() for e in index(baseline / r['package']) if e.key == unit))
```

即：**它是一次历史研究构建产物里的 Unit**（`交互朝向研究0.7.0` 的 `identities.unit`），
不是 `behavior_native_profile.json` 的 `sourceUnit`（`40E0C000:59292C11`），也不是
`resolve()` 的产物。它的 172 个字段里 12 个效果列全为 0（见 Q1「12 个效果列实测」），而基准 PARK
配方要求 1.0/[2.5,2.5]（`behavior_profiles.py:85-87` 对 `pin['effectValues']` 做**相等**校验）。

**推论（数据层，已证）**：`tested-unit.bin` 不可能通过基准 `resolve()` 的效果列校验，
因此它不是基准 PARK 的 Unit；Rust 用它当 `generic-park` donor，等于只拿到了
`legacy-display` 的基座。

---

## 2. 逐项回答

### Q1. `behavior_profiles.resolve()` 克隆循环 vs Rust 的 Unit 生成

#### 基准（`behavior_profiles.py`）

| 步骤 | 行号 | 语义 |
|---|---|---|
| 资源哈希预检 | 62-69 | 对 `pin['resourceHashes']`（46 个）逐个读游戏包、算 SHA-256，不一致直接报「公园功能依据与已研究版本不一致」 |
| parent 链克隆 | 70-78 | 从 `pin['sourceUnit']` 起，`cloned(data).items()` → `fields.setdefault(h, raw)`（首见即胜），沿 `00B2CCCB` 走到无父 |
| 链一致性 | 79 | `parents != pin['parentChain']` → 报错（8 跳，顺序敏感） |
| 去父边 | 80 | `fields.pop(0x00B2CCCB)` |
| 子模块检查 | 83-84 | `07E0AD0A` 非空 → 报错（不允许残留默认子模块） |
| 效果列专化校验 | 85-87 | 12 列必须等于 `pin['effectValues']` |
| 返回 | 88-95 | `fields` + evidence（含 `candidateRuleCount=205`、`nativeDefaults`） |
| apply：OMIT | 106-114 | `fields.pop` 掉 `00B2CCCB / 0BF5D2E3 / 0BF5D2E4` |
| apply：PROTECTED | 115-117 | `08E17ED7 / 0ACFFA64 / 09FB78CB`：在旧 unit 里就拷贝，否则 pop |
| apply：费用 | 119-120 | `0A8A41DA=cost`、`09AE19D7=maintenance`（floatprop） |
| apply：期望记录 | 121 | `expectedUnitRecords` = 除 `08E17ED7/09FB78CB` 外全部字段的 recordHex |

#### Rust

| 步骤 | 位置 | 现状 |
|---|---|---|
| 通用链克隆 | `native_profile.rs:274-320` `flat_chain()` | **是 `resolve()` 的忠实移植**：`00B2CCCB` 走链、首见即胜、去父边、循环检测 |
| 该克隆用在哪 | `native_profile.rs:1235-1272` | 只用于 **Asset 字典**（`40E1C000:9EC474FF`）与 `34CE7E14` 交互链；Unit 不走链 |
| Unit 生成 | `native_profile.rs:1611-1629` | **直接 parse 静态 `tested-unit.bin`**（`generic-park`）；elementary/complete 也是静态 bin |
| 资源哈希预检 | 无 | 只检查 `GENERIC_ASSET`/`INTERACTION_CHAIN`/`GENERIC_MENU`/地面 atlas **存在**（`1235-1330`、`1511-1517`），无 SHA-256 pin |
| parent 链一致性 | 无 | 没有 park 链，也没有 `parentChain` 概念 |
| 去父边 | 仅 `flat_chain` 内部 | Unit 依赖 donor 本身无 `00B2CCCB`（tested-unit 确实没有） |
| 子模块检查 | 无 | donor 若带 `07E0AD0A` 不会被拒 |
| 效果列 | 无（全仓库 grep `0E965A70…0E965BE5` 零命中） | 保留 donor 值；tested-unit 全 0 |
| OMIT（B3/B4/父边） | 无显式 pop | tested-unit 本身没有这三列 → 结果碰巧等价，但无保护 |
| PROTECTED | 分散覆盖 | `08E17ED7` 改指新身份（`1689-1702`，type-less）；`0ACFFA64` 保留；`09FB78CB` **新增**（`1703-1706`），基准是 pop |
| 费用 | `1630-1645` | `0A8A41DA/09AE19D7` 按 spec 写 Float32 ✓ |
| Unit 类别 | `1649-1659` | 默认 `07049250`（公园）；基准 LEGACY 是 `95A9D6FD`（教育） |

#### 判定：**不等价**

- 不是「从零拼装」，也不是基准的「链克隆」；是 **静态 donor 克隆 + 字段 upsert**。
- 克隆算法本身（`flat_chain`）与基准一致，但**用错了对象**（Asset 而非 Unit）。
- 基准 46 个哈希 pin 的游戏版本闸门完全缺失 → 换游戏版本时基准会 fail-closed，
  我们不会，会静默沿用 2026-11 的冻结字段。

#### 12 个效果列实测（本次 dump，数据层）

| 字段 | 基准 pin | `tested-unit.bin`（Rust park donor） | `elementary-school-unit.bin` | `complete-building-unit.bin` |
|---|---|---|---|---|
| `0E965A70/72/74` | 1.0 | **0.0** | 1.0 ✓ | 0.0 |
| `0E965A71/73/75` | [2.5,2.5] | **[0,0]** | [2.5,2.5] ✓ | [0,0] |
| `0E965BE0/E2/E4` | 1.0 | **0.0** | 缺失 | 0.0 |
| `0E965BE1/E3/E5` | [2.5,2.5] | **[0,0]** | 缺失 | [0,0] |

→ 任一路径都拿不到基准 PARK 的 12 列值。

---

### Q2. `verify()` 回读校验 vs Rust

#### 基准（`behavior_profiles.py:125-131`）

1. 对 `evidence['expectedUnitRecords']` 逐字段比较 `unit[h]['recordHex'].lower()`；
   任一不等 → 「建筑功能字段回读不一致」。
2. `profile == PARK` 时：`00B2CCCB` 存在 → 报错；`0BF5D2E3/0BF5D2E4` 存在 → 报错。
3. 调用点在最终包回读之后（`backend_compile.py:225-234`）：`parse(built[identities.unit])`
   → `behavior_profiles.verify` → 再检查 cost/maintenance 值。

#### Rust 现状

| 基准检查 | Rust 对应 | 结论 |
|---|---|---|
| 逐字段 recordHex 相等 | `property_gate.rs:202`（自基线漂移）、`delta_audit`（仅 Lot，`stages.rs:1880`）、`connector::verify`（字节回读） | **无对应**：都只能保证"写进去的字节没变/与上次一致"，不能保证"等于已研究配方" |
| PARK 禁止 `00B2CCCB` | 无 | 缺失 |
| PARK 禁止 `0BF5D2E3/0BF5D2E4` | 无 | 缺失 |
| cost/maintenance 回读相等 | `menu_contract.rs:375-400` 只查 **类型=Float**，不查值 | 弱化 |
| —— | `behavior.rs:1259` `ensure_behavior_invariants` | 只校验本次**声明**的 parent/category/bundle/remove/set 字段，不校验 profile |

`property_gate.rs:202-300` 的逻辑是「第一次构建建立基线，后续构建不允许漂移」——
第一次构建即使是错的也会被接受；这对"1:1 迁移验收"没有约束力。

---

### Q3. 内景控制器 `0F169C46 / 0E0BE186`

#### 基准里写在哪

- `0F169C46` 在 0.8.0 源码中**只出现一次**：`backend_compile.py:206`
  `asset_bool_overrides={0x0f169c46:True}`，交给 `build_candidate.main`。
  engine 缺失，无法核实其最终落点；但 `p['asset']` 是 Asset 字典（`40E1C000`），
  `asset_bool_overrides` 从命名与 `build_candidate` 的 profile 入参推断就是写入该字典。
  0.4.0 报告只说「保留原生内景控制器启用属性 0F169C46」，未给资源组。
- `0E0BE186`：**整个 0.8.0 源码零出现**（grep 确认）。
- 基准**不生成** `42E1C000` 图形单元资源（源码中无该组、无相关构造）。

#### Rust 写在哪

| 资源 | 位置 | 内容 |
|---|---|---|
| Asset 字典 `00B1B104:40E1C000:<id>` | `native_profile.rs:1601-1604` | `0F169C46=Bool(true)` —— 与基准 `asset_bool_overrides` 对齐 |
| 图形单元 `00B1B104:42E1C000:<id>` | `menu.rs:552-600` | `00B2CCCB`→自身 Lot、`00F9EFBA` BBox、`00F9EFBB..BE` 模型引用×4、`0E0BE186=true`、`0F169C46=true` |
| Lot 属性回读闸门 | `package.rs:940-968` | 编译模型存在 ⇒ Lot 侧 `0F169C46=true` 必须回读成功 |

`menu.rs:1003-1019` 的 `verify_interior_controller_contract` 在构建期要求两个 Bool 均为 true。

#### 对组/实例/flags 的判断

- **组**：`42E1C000` 与引擎研究一致（`FindUnitNonSimProps` 在 graphics 组按 state 检索，
  见 `docs/re/hope-tower-labor-engine-alignment-20261111.md` §11）。**这是超出基准的扩展**，
  不是移植；基准只有 Asset 字典里的 `0F169C46`。
- **实例**：用菜单身份 instance（与 Menu2/Unit/Lot 同一实例）。原版图形单元（如 `50586A05`）
  是独立实例，由 Lot/Unit 的 graphics state 列表引用；用身份实例**未经实证**，
  属于假设。若引擎按 state 列表（`0CA5CF4A`）检索，同实例并不会自动命中。
- **flags**：`set_scalar` → `canonical_flags`（`crates/sc-properties/src/lib.rs:316-347`）
  对标量补 `0x8000`，与基准注释中的 `0f169c46 0001 8000 01` 一致 ✓。
- **遗漏的属性**：原版图形单元（92 列）还带
  - 17 组 `UnitLocation`（`02A907B5/B6/B9/BB/BC` 等）；
  - 路网键 `0C12EF24-2A`；
  - graphics state 列表 `0CA5CF4A`（检索键，见 `asset-behavior-census-and-gating-20261106.md` 证据索引 `FindUnitNonSimProps.c`）；
  - 我们只写了 2 个开关 + 父边 + BBox + 4 模型引用。

结论：**组对、flags 对、实例存疑、属性不全**；且它是我方扩展，不应在"1:1 迁移"里算已迁移项。

---

### Q4. `light_contract` 列清单 vs Rust

基准列（`light_contract.py:61-70`）与 Rust（`native_profile.rs:184-205`、`scene.rs:1240-1345`）
逐列对照：

| 字段 | 基准值/来源 | Rust | 差异 |
|---|---|---|---|
| `0CAA8F10` | `[0,0,GROUP]` | `zero_key(LIGHT_GROUP)` | 等价 |
| `0CAA8F11` | POINT `0x75D4C8CD` / SPOT `0x2F0FF9FD` | 同 + LINE `0x0820ABAF` | 扩展（基准拒绝 LINE） |
| `0CAA8F12` | Transform flags 14 + 12f | 同 | 等价 |
| `0CAA8F13` | color | ColorRgb | 等价 |
| `0CAA8F14/15/16` | radius/inner/intensity | 同 | 等价 |
| `0CAA8F17` | `'SC5_'+id[:24]`（≤28 字符） | `"SC5_"+debug_name.take(24)`（≤24） | **偏差**：长度上限不同 |
| `0D4C96B4` | specular | 同 | 等价 |
| `0D4CAD23` | length | 同 | 等价 |
| `0E01200F` | `[0,0,cullTier=1048449613]` | `CULL_MAX=0x3E7E124D` | 等价 |
| `0E4DF244` | **恒 0.0** | 作者 falloff（默认 0） | 扩展：可被工作台写成非 0 |
| `0E98D445` | **恒 False** | 作者 volumetric（默认 false） | 扩展 |
| `0EC4567E` | **恒 0.0** | 作者 volStrength（默认 0） | 扩展 |
| `0CE71B10` | `[0,0,GROUP]` | 同 | 等价 |
| `0CE71B11` | `[0,0,311764537]` | `NIGHT_PERIOD=0x12952639` | 等价 |
| `0CE71B12` | `[0.875, 0.21527299284934998]` | `[0.875, 0.215_273_01]` | **1 ULP 偏差**：f32 位型 `0x3E5C7087` vs 基准 `0x3E5C7086`（`scene.rs:1341` 用的是正确值 `0.21527299`，两处写入源不一致） |
| `0CE71B13` | `[0,0]` | 同 | 等价 |

#### 灯光名（值得单列）

- 侧车路线：`scene.rs:422-425` 把 `lights.json` 的 `id` 直接设为 `"SC5_{id}"`；
  `native_profile.rs:799-807` 又加一次 `SC5_` → **`SC5_SC5_...` 双前缀**。
- 工作台路线：`scene.rs:556-559` 用 `debugName` 原文；`native_profile.rs` 再加 `SC5_`。
- 基准：`'SC5_' + id[:24]`，即 id 截 24、总长 ≤28。

#### 灯光清单校验缺失（`light_contract.validate` 9-33 vs Rust）

基准强制：schema/unit/upAxis/space；≤128 个；id 非空且唯一；matrix 正交、无镜像；
位置 ≤100000；color 0..1；`radius/innerRadius/length/intensity/specular` 范围；
`innerRadius <= radius`；`control == 'NATIVE_NIGHT'`。
Rust `merge_sidecar_manifests`（`scene.rs:388-427`）只 checks `instances` 是数组、
`matrix_field` 12 个有限数；其余全部未校验。数量上限也没有。

#### 清单来源校验缺失

基准 `read_manifest`（`light_contract.py:35-45`、`prop_contract.py:31-41`）要求
LOTM 的诊断信息里声明 `lightManifest/propManifest` 的 `file`、`sha256`、`count`，
三者都要匹配；有清单但未声明 → 报错「楼体未声明灯光清单」。
Rust 只要有同名 `.lights.json`/`.props.json` 就无条件合并（`package.rs:373-380`），
**没有任何来源/哈希/数量核对**。

---

### Q5. `menu_locations` / `menu_icons` vs `menu.rs`

#### 基准能力

| 能力 | 行号 | 说明 |
|---|---|---|
| 22 个具名位置目录 | `menu_locations.py:6-31` | `key → label/path/categoryInstance/native donor TGI` |
| 位置合法性 | 33-36 | 不支持的 key → 明确报错 |
| donor 校验 | 38-51 | 游戏包必须含该 donor；donor 的 `0DB9FC63` 必须包含 `[0,0,categoryInstance]`，否则报「菜单归属与已核对资料不一致」 |
| 归属数组写出 | 54-55 | 非教育 = **恰好一个** `[0,0,cat]`；教育 = **保留 donor 全部成员** |
| 排序 | 65-85 | 扫描全部 `09878A01` 属性、目标类别的 `0DC1E3E0`，取 `max+100`；`require(len(cats)==len(orders))` |
| 回读 | `backend_compile.py:229-231` | 归属集合相等、归属数 == 排序数 |
| 图标 | `menu_icons.py:6-10` | 一律 `exif_transpose → RGBA → ImageOps.fit(128×128, LANCZOS)`，保留 alpha |

#### Rust 对照

| 能力 | Rust 位置 | 差异 |
|---|---|---|
| 22 位置目录 | 无 | 只有常量 `LANDMARK_CATEGORY`/`PARK_BEHAVIOR_CATEGORY`；类别实例由 schema 自由传 |
| 位置合法性 | `menu.rs:277-293` | 不校验，任意 u32 都写 |
| donor 校验 | 无 | 不读游戏 donor，不检查 `0DB9FC63` 是否含目标类别 |
| 归属数组 | `menu.rs:1554-1559`、`1717-1722` | 非教育单类别 ✓；教育**不保留** donor 额外成员（基准测试里的教育额外成员是 `[0,0,0x6A1785B4]`） |
| 排序数组 | `menu.rs:1573-1578`、`1737-1742` | **恒写 1 个值**（默认 11500），即使 `uiCategories` 有 N 个（Labor 接线 3–4 个）→ **长度不等** |
| 排序算法 | 无 | 基准 `max+100` 扫描；Rust 固定 11500，无扫描、无溢出检查 |
| 回读 | `menu_contract.rs:197-217` | 只查非空；不查集合相等、不查长度相等 |
| 图标 | `menu.rs:2077-2122` | 用户 PNG **原样透传**，不裁切/缩放；非 PNG 只做格式转换。基准必做 128×128 等比裁切 |
| 统计行 | `menu.rs:1280-1334`、`1745-1762` | 基准 0.8.0 源码里没有统计行代码（由 engine 的菜单模板提供，无法核实）；Rust 手写 s31 三对 text↔param。这是**超出基准的配方**，不能算迁移项，且缺少与"原版菜单模板"的对拍依据 |

> 备注：0.5.0 报告第 46/56 行明确写了「排序…取最大排序值后追加 100，并保证归属与排序数组等长」
> 以及「两个完整包 17 个资源里只有 1 个 Menu 资源变化，变化仅为 `0DB9FC63` 与 `0DC1E3E0`」。
> Rust 当前的多容器路径违反这条已被作者写入验收的最小不变量。

---

### Q6. `behavior_native_profile.json` 常量 vs Rust 硬编码

| 常量组 | 基准 | Rust | 判定 |
|---|---|---|---|
| `sourceUnit` + `parentChain`（8 跳） | `40E0C000:59292C11` → … → `593B61D2` | 无 park 链；Unit = `tested-unit.bin` | **缺失** |
| `resourceHashes`（46 个） | 逐资源 SHA-256 | 无 | **缺失** |
| `candidateRuleIds`（205 个） | 只进 evidence `candidateRuleCount` | 无 | 报告层缺失（不影响包） |
| `effectValues`（12 个） | 1.0 / [2.5,2.5] | 无写入；donor 全 0 | **缺失**（Q1 已证） |
| job/worker 字段 | `0A7917C3`（total jobs）、`0A7917C8`（min workers）、`0A7917C5/6/7` | 无 park 硬编码；labor 节点按作者值写 `0A7917C5/6/7/8` + 自动 Σ `0A7917C3`（`behavior_nodes.rs:418-530`） | 语义不同：基准读链、我们按工作台输入写；无 `nativeDefaults` 对拍 |
| `0AD7FAC9` 容量 | evidence `nativeDefaults.capacityField` | 仅 `spec.student_capacity` 显式时写 | 无对拍 |
| 灯光常量 | `GROUP 8E43F125`、`nightPeriod 311764537`、`cullTier 1048449613`、POINT/SPOT | 全部一致（`native_profile.rs:167-205`，测试 `1867-1890` 钉死十进制） | **等价**（仅 `NIGHT_RANGE` 1 ULP，见 Q4） |
| `light_native_profile.source` | `40E1C000:E0301934` | 未使用（列已硬编码） | 仅证据字段缺失 |

---

### Q7. 资源清单差异（Python 一个资产 vs Rust）

基准 `backend_compile.py` 决定性的产出（engine 缺失项标注）：

| 资源 | Python 依据 | Rust 位置 | 差异 |
|---|---|---|---|
| Asset 字典 `00B1B104:40E1C000:X` | `p['asset']`（136-181、145-148） | `native_profile::build` | 均有 |
| Unit `00B1B104:40E0C000:X` | `p['unit']`（139-142） | `native_profile`/`menu` | 均有；内容不等价（Q1） |
| Menu2 `00B1B104:09878A01:X` | `p['menu']`（143） | `menu.rs` | 均有；位置/成员规则不同（Q5） |
| 图标 PNG `2F7D0004:40E02400:X` | `menu_icon_path` | `menu.rs:2077` | 均有；尺寸归一化缺失 |
| Rollover JPG `3F8662EA:0:X` | engine | `menu.rs:2126` | 均有（内容策略不可比） |
| Locale 表 `0A98EAF0:02FABF01:*` | engine | `menu.rs:544`（instance=身份） | TGI 不同（我方按身份隔离），属有意改进 |
| 地面 mask raster | `p['mask']`（172） | `native_profile.rs:1404-1425` | 均有 |
| 模型 RW4 LOD | engine；基准只允许 `stable_single_mesh`（`backend_compile.py:41-42`） | `package.rs:824-840`（**4 份相同字节**） | 复用策略不同（engine 缺失，LOD 引用细节不可核实） |
| 六槽材质 | `prepare_asset`/`compile_tower_asset` | `package.rs:470-510` | 均有（算法已另案迁移） |
| 内景图集 | `prepare_asset` | `author_materials` | 均有 |
| 碰撞代理 | `collision_proxy.py` | `author_building40`（并入模型 RW4） | 资源形态不同 |
| **图形单元 `42E1C000`** | **基准不生成** | `menu.rs:552-600` | **我方多出 1 个资源**（扩展） |
| 预览图资源 | engine（`preview_path`） | 无 | 可能缺失（engine 缺失，无法确认是否进包） |
| `0DC1E3E0` 排序数组 | `menu_locations:83` | 有资源字段，但长度错误 | 见 Q5 |

---

## 3. 缺口总表

> 级别定义：**P0** = 配方/校验层缺失，导致产物与基准语义不同；**P1** = 菜单/灯光/清单层可观测偏差，
> 可能造成排序、可见性、图标或数据串味；**P2** = 字节级/证据级/边界校验偏差。

| ID | 级别 | 主题 | 基准（函数:行） | Rust（文件:行） | 差异 | 影响 |
|---|---|---|---|---|---|---|
| G1 | P0 | 公园 Unit 未克隆 | `behavior_profiles.py:62-95` | `native_profile.rs:1611-1629` | Unit = 静态 `tested-unit.bin`，不是 8 跳 park 链 | 行为/经济/容量字段与基准 PARK 不同；无版本闸门 |
| G2 | P0 | 12 个效果列 | `behavior_profiles.py:15,85-87` | 无 | 效果列不写；donor 全 0 | 幸福度/地价等专化参数缺失 |
| G3 | P0 | `verify()` 语义 | `behavior_profiles.py:125-131` | `behavior.rs:1259`、`property_gate.rs:202` | 无 recordHex 逐字段相等；无 PARK 禁 parent/B3/B4 | 错误配方可静默出货 |
| G4 | P0 | 图形单元属性不全 | 基准无（扩展项） | `menu.rs:552-600` | 缺 UnitLocation×17、路网键、`0CA5CF4A` state 列表；实例=身份 | 控制器检索可能命中不了；夜景提亮不确定 |
| G5 | P1 | 排序数组长度 | `menu_locations.py:78-83`、`backend_compile.py:231` | `menu.rs:1573-1578,1737-1742` | N 个类别只写 1 个排序值（Labor 接线 3–4 个） | 面板排序/绑定可能错位 |
| G6 | P1 | 排序算法 | `menu_locations.py:65-85` | 无 | 固定 11500，不扫描 `max+100` | 与模组/其他条目撞序；不保证末尾 |
| G7 | P1 | 菜单位置目录/donor 校验 | `menu_locations.py:6-62` | 无 | 无 22 位置表、无 donor/类别校验 | 选错类别 → 条目不可见且无构建期报错 |
| G8 | P1 | 图标归一化 | `menu_icons.py:6-10` | `menu.rs:2077-2122` | 用户图标原样透传 | 非 128/非方形图标与原版菜单不符 |
| G9 | P1 | 侧车来源校验 | `light_contract.py:35-45`、`prop_contract.py:31-41` | `scene.rs:350-430` | 不查 file/sha256/count；无数量上限 | 混用不同次导出；超限数据进包 |
| G10 | P1 | 灯光名 | `light_contract.py:64` | `scene.rs:422`+`native_profile.rs:799` | 侧车双前缀 `SC5_SC5_`；截断 24 vs 28 | `0CAA8F17` 字节不一致 |
| G11 | P2 | 夜阈值 1 ULP | `light_native_profile.json:63-66` | `native_profile.rs:174`（`scene.rs:1341` 正确） | `0.215_273_01`→`0x3E5C7087` vs `0x3E5C7086` | `0CE71B12` 字节不一致（scene.rs 正确） |
| G12 | P2 | LEGACY 类别 | `behavior_profiles.py:110` | `native_profile.rs:1649-1659` | 基准 LEGACY=`95A9D6FD`，Rust 默认=`07049250` | "仅展示"资产行为类别不同 |
| G13 | P2 | `08E17ED7` 类型 | `behavior_profiles.py:115-117` | `native_profile.rs:1689-1702` vs `menu.rs:713-722` | parent-free 路线 type-less；基准从 tested-unit 继承 type-ful；我方两条路线互相矛盾 | 链接解析风险（Rust 注释称 type-ful 会断） |
| G14 | P2 | `09FB78CB` | `behavior_profiles.py:115-117` | `native_profile.rs:1703-1706` | 基准对 tested-unit 是 pop；Rust 新增 Unit 级标题边 | Unit 多一列 |
| G15 | P2 | 道具 marker | `prop_contract.py:83-84` | `native_profile.rs:725-736` | `0C12EF60` 车辆 bin 写 true，基准恒 false | 车辆道具百分比填充行为不同（可能是改进） |
| G16 | P2 | 灯光校验范围 | `light_contract.py:9-33` | `scene.rs:388-427` | 无正交/镜像/范围/inner<=radius/control 校验 | 非法清单可进包 |
| G17 | P2 | 道具校验 | `prop_contract.py:8-29,51-86` | `native_profile.rs:623-738` | 无 scale/矩阵/镜像校验；无模型 SHA/依赖校验 | 非法道具可进包 |
| G18 | P2 | 报告证据 | `behavior_profiles.py:88-95` | 无 | 无 `candidateRuleCount`/`nativeDefaults`/`validatedResources` | 台账无法复核配方身份 |
| G19 | P2 | 教育菜单成员 | `menu_locations.py:52-54`、`test_menu_locations.py:29-33` | `menu.rs:1554-1559` | 基准教育保留 donor 全部成员（测试样例含 `[0,0,0x6A1785B4]`）；Rust 单类别 | 教育入口可能少看板归属 |
| G20 | P2 | 地块栅格方形约束 | `backend_compile.py:160-163` | `native_profile.rs:462-503` | 基准要求 w==h；Rust 放宽为任意 2 的幂 | 非方形 mask 会被接受（有注释说明是有意放宽） |
| G21 | P2 | 灯光 LINE 类型 | `light_contract.py:15` | `scene.rs:407-410`、`native_profile.rs:795-798` | 基准只允许 POINT/SPOT；Rust 放行 LINE | 有意扩展（有 s2 实证） |
| G22 | P2 | falloff/volumetric | `light_contract.py:64` | `native_profile.rs:854-856` | 基准恒 0/false；Rust 可被场景写非 0 | 有意扩展 |

---

## 4. 必须修（可执行建议）

### P0-1 新增 `behavior_profile` 模块，按基准实现 PARK 配方

**改哪里**：新增 `src-tauri/src/asset_build_m1/behavior_profile.rs`（或并入 `native_profile.rs`），
并在 `MenuRegistrationSpec` 增加 `behavior_profile: Option<String>`（`"legacy-display" | "native-park-w1-v1"`），
由 `plan.rs` 传给 `native_profile::build`。

**做什么**：

1. 把 `reference/sc5-packager-0.8.0/behavior_native_profile.json` 原样复制到
   `src-tauri/resources/sc5/behavior_native_profile.json`（或用 `include_str!` 内嵌）。
2. `resolve_park(resources) -> Result<(PropertyFile, Evidence)>`：
   - 对 `resourceHashes` 46 项逐个 `resources.get(key)` + SHA-256 比对；缺失/不一致时用基准原文
     `公园功能依赖缺失 / 公园功能依据与已研究版本不一致`（fail-closed）。
   - 复用 `flat_chain(resources, pin.sourceUnit)` 得到 `fields`；额外实现 `parentChain` 顺序相等
     校验（`flat_chain` 返回的 `sources` 可直接比对）。
   - `07E0AD0A` 非空 → 报错；`effectValues` 12 项逐项相等 → 否则报错。
3. `apply_park(donor_unit, resolved) -> (PropertyFile, expected_records)`：
   - `pop 00B2CCCB / 0BF5D2E3 / 0BF5D2E4`；
   - `PROTECTED`：`08E17ED7` 用**新身份**覆盖（保留 donor 的 type 字段写法，见 P2-2）、
     `0ACFFA64`/`09FB78CB` 按「在 donor 中则拷贝、否则删除」；
   - 写 `0A8A41DA=cost`、`09AE19D7=maintenance`；
   - 返回 `expected_records`（除 `08E17ED7/09FB78CB`）。
4. `native_profile::build` 的 Unit 分支：当 profile=`native-park-w1-v1` 时用 `resolved`，
   不再用 `tested-unit.bin`；`legacy-display` 保持现有 tested-unit + `0AFE27B1=95A9D6FD`。
5. 测试（离线夹具）：用一份含 park 链的参考包（或已归档的游戏包）断言：
   - 46 哈希全过；改动任一字节 → 报错；
   - 12 个效果列 = 1.0/[2.5,2.5]；
   - 产物 Unit 无 `00B2CCCB/0BF5D2E3/0BF5D2E4`；
   - `expected_records` 回读全等。

**验收**：构建台账输出 `profile: native-park-w1-v1`、`validatedResources: 46`、
`candidateRuleCount: 205`、`effectValues`；任一 pin 不匹配时构建失败。

### P0-2 加回 `verify()` 等价闸门

**改哪里**：`behavior.rs` 新增 `verify_behavior_unit(ctx, unit_id, expected, profile)`，
在 `ensure_behavior_invariants` 之后调用；同时在 `package.rs` 的最终暂存读回处再跑一次
（与 `backend_compile.py:225-234` 的位置一致）。

**做什么**：逐 hash 比较 `PropertyFile` 的 value（类型+值；`recordHex` 等价物），
不等 → `建筑功能字段回读不一致：<hash>`；profile=PARK 时断言
`00B2CCCB/0BF5D2E3/0BF5D2E4` 均不存在。

### P0-3 补全/收敛图形单元

**改哪里**：`menu.rs:552-600`（`generate_parent_free` 的 `graphics_unit` 构造）。

**做什么**（按优先级）：
1. 把 `0CA5CF4A` graphics-state 列表从 `native_profile` 的 Lot/Unit 侧显式声明，或在图形单元里
   写入与 Lot 相同的 state 键，保证 `FindUnitNonSimProps` 能查到本资源；
2. 从场景 path 端点生成 `02A907B5/B6`（UnitLocation）与 `0C12EF24-2A` 路网键
   （可直接复用 `native_profile::compile_spawner_fields`/`compile_path_fields` 的旋转后变换）；
3. 在构建台账里记录「图形单元属性数 / 与 vanilla 92 列的差集」，避免"写了一半"无人可见；
4. 在不具备实证前，保留 Asset 字典的 `0F169C46`（与基准一致），把图形单元标记为
   `runtimeVerified:false` 的扩展项。

### P1-1 排序数组与算法（G5/G6）

- `apply_menu_overrides`（`menu.rs:1737-1742`）与 `build_menu2`（`1573-1578`）都改成
  `vec![Int32(position); categories.len()]`。
- 增加 `menu_position(reference_or_game_index, categories) -> Option<i32>`：扫描所有
  `09878A01` 组属性，读取 `0DB9FC63`/`0DC1E3E0`，取匹配类别的 `max+100`；
  无匹配且未显式给 `uiPosition` → fail-closed（基准 `require(positions and not unreadable)`）。
- `menu_contract` 增加「`len(0DB9FC63)==len(0DC1E3E0)`」硬断言（与
  `backend_compile.py:231` 同款）。

### P1-2 菜单位置目录与 donor 校验（G7）

- 新增 `menu_locations.rs`，把 `_ROWS` 22 行（label/path/categoryInstance/donor TGI）
  逐条搬入（可放在 `src-tauri/resources/sc5/menu_locations.json`）。
- 构建前用 game index（只读）验证 donor 存在且 `0DB9FC63` 含 `[0,0,categoryInstance]`；
  失败文案沿用基准。
- 教育位置：写出 donor 的全部成员（而不是单个），与基准 `selected=keys if education` 一致。

### P1-3 图标归一化（G8）

- `build_icon_png` 对用户图片统一：`exif` 方向 → RGBA → 等比裁切到正方 → 128×128
  （`image` crate 可用 `resize_to_fill` 或先 `crop_imm` 再 Lanczos3）。
- 输出永远是 128×128 RGBA PNG；非 PNG 也走同一路径。
- 测试：512×512、300×200、带 alpha 各一张，解码后尺寸/alpha 断言。

### P1-4 侧车来源校验与数量上限（G9）

- `merge_sidecar_manifests` 接收 LOTM 诊断信息（`probe_tower.read` 的 diagnostic 等价物）；
  若存在 `lights.json/props.json`，要求 `file == path.name`、`sha256 == 实际`、
  `count == instances.len()`，否则 fail-closed；
- 若清单存在但模型未声明 → 报错；
- 灯光 ≤128、道具 ≤1000。

### P1-5 灯光名与夜阈值（G10/G11）

- `native_profile.rs:799-807`：改为 `"SC5_" + id.take(24)`，且若 `debug_name` 已以 `SC5_` 开头
  先去前缀（或让侧车不预加前缀，二者只保留一处）。
- `native_profile.rs:174`：`0.215_273_01` → `0.215_272_99`（`0x3E5C7086`），
  并把 `scene.rs`/`native_profile.rs` 两处字面量收敛到同一个 `pub(crate) const`。

### P2 建议修

1. LEGACY 默认类别恢复 `95A9D6FD`（G12）。
2. 统一 `08E17ED7` 的写法：以 donor 实际写法为准（vanilla 是 type-less，tested-unit 是 type-ful），
   并在台账里记录选择；不要两条路线互相打脸（G13）。
3. `09FB78CB` 按基准「donor 有则留、无则删」处理 Unit 侧；Asset 侧保留（G14）。
4. 道具 marker `0C12EF60` 与基准对齐，或把"车辆 bin 需要 percentFill"写成显式 schema 开关（G15）。
5. 灯光清单补 `validate`：正交/镜像/颜色/半径/`inner<=radius`/`control`（G16）。
6. 道具补 `validate`：scale 0.001..100、矩阵正交、无镜像、位置上限、模型 SHA/依赖（G17）。
7. 证据字段补齐：`candidateRuleIds` 计数、`nativeDefaults`、`validatedResources`（G18）。
8. 地块栅格恢复 `w==h` 约束，或把放宽写成显式 `schema` 选项（G20）。

---

## 5. 等价清单（已逐项核对，可视为 1:1）

| # | 项目 | 基准 | Rust | 证据 |
|---|---|---|---|---|
| E1 | 链克隆语义（首见即胜、去父边、循环检测） | `behavior_profiles.py:70-80` | `native_profile.rs:274-320` | 逐分支对照 |
| E2 | 克隆算法用于 Asset 通用链 | `interactive_park_profile`（engine 缺失） | `native_profile.rs:1268-1272` | 注释与 `INTERACTION_CHAIN` 常量 |
| E3 | 费用/维护费覆盖 | `behavior_profiles.py:119-120` | `native_profile.rs:1630-1645` | 同为 Float32 `0A8A41DA/09AE19D7` |
| E4 | Unit 类别键形状 | `apply` 的 `key_record`（type/group=0） | `zero_key(category)` | 3 个 0/0/instance |
| E5 | Unit→Asset 链接（parent-free） | `interactive_park_profile` 的 `unit[08e17ed7]=key(asset)` | `native_profile.rs:1689-1702` | 值语义相同（类型见 G13） |
| E6 | 地面数字字段 | `backend_compile.py:153-157,167-171` | `native_profile.rs:1339-1363,1427-1510` | LOT_TYPES + `0CCB7FC8/9/D0/D6` + `0BBB CDED/CE2A` + `0FE01484`（flags 0x801C）逐字段一致 |
| E7 | 地块栅格旋转 | `asset_transforms.rotate_lot` | `native_profile.rs:419-458` | 90/270 交换宽深/周期，offset 同向旋转，`np.rot90` ↔ `rot90_ccw` |
| E8 | 地面 atlas 键 | `backend_compile.py:173` | `native_profile.rs:1511-1541` | `0CCB7FD4`/`0D0082F1` 两个共享实例 |
| E9 | 灯光 18 列与 4 个固定列 | `light_contract.py:61-70` | `native_profile.rs:184-205,829-882` | 除 G10/G11 外逐列同型同值 |
| E10 | 灯光 group/nightPeriod/cullTier/POINT/SPOT | `light_native_profile.json` | `native_profile.rs:172-205` + 测试 `1867-1890` | 十进制钉死 |
| E11 | 道具列布局 | `prop_contract.py:77-84` | `native_profile.rs:704-737` | `0C12EF20/30/40/50/60+bin`；Transform flags 15 + scale；标记 scalar flags 0x8000。差异：Rust 按 bin 归并多个 preset 且 `indices` 是 bin 内局部下标（基准每 preset 一组、后写覆盖），属改进 |
| E12 | 菜单 Key 编码 | `menu_locations.py:55`（`pack('>3I', inst,0,0)`） | `menu.rs:283-293`（instance/type/group） | 文件线序 instance,type,group 一致 |
| E13 | 菜单 menuUnit/title/desc/icon/marquee 形状 | `backend_compile.py:143` + engine | `menu.rs:1542-1668` | T/G=0 menuUnit；Text 行 1/2；`2F7D0004:40E02400`；`3F8662EA:0` |
| E14 | `0F169C46` 在 Asset 字典 | `backend_compile.py:206` | `native_profile.rs:1601-1604` | Bool true，canonical flags 0x8000 |
| E15 | 内景开关构建期闸门 | 无（engine） | `menu.rs:1003-1019`、`package.rs:959-968` | 两道 |
| E16 | 标签/描述独立表 | engine | `menu.rs:959-977,1672-1693` | 基准 engine 无法核实；我方更严格（禁共享表） |
| E17 | `tested-unit.bin` 基座 | `prepare_engine.py:52-57` | `resources/sc5/tested-unit.bin` | SHA-256 相同 |

---

## 6. 无法核实（engine 缺失）

以下结论**不能**从本快照得出，报告中已避免当作事实：

1. `build_candidate.main` 对 `asset_bool_overrides` 的最终落点（Asset 字典 vs 其它资源）。
2. `build_candidate` 的初始 Menu2 模板、统计行 text/param 绑定（`unitEffectTitle/Parameter`）、
   locale 表 TGI、排序字段默认值。
3. `interactive_park_profile.make_profile` 的 Asset/Lot/Unit 骨架与 `compile_fields` 的
   实际合并顺序（Rust 注释声称已按此移植，但无法与原件对拍）。
4. `append_menu_position` 是否在主构建路径被调用（源码里只有 `audit_menu_locations.py` 调用；
   `backend_compile.py` 只调用 `menu_record`）。但 0.5.0 报告明确写了构建会做 `max+100` 与
   等长校验，所以 G5/G6 仍按"基准行为"要求。
5. Python 成品的完整 17 资源 TGI 列表（只能从 0.5.0 报告的"17 个资源"间接得知）。

---

## 7. 复现与证据

```bash
# 1) 基准与本地基座哈希
sha256sum src-tauri/resources/sc5/tested-unit.bin
#  21b3fcd097db6b83003a6368b4b33ac8ab5d8b5949937493c77f7d35f3b58476
grep -n "tested-unit" reference/sc5-packager-0.8.0/source-manifest.json
#  "engine/tested-unit.bin": "21b3fcd0…"

# 2) 效果列实测（只读解析 resources/sc5/*.bin）
#    tested-unit.bin 0E965A70..75 / 0E965BE0..E5 全为 0.0/[0,0]
#    elementary-school-unit.bin 0E965A70..75 = 1.0/[2.5,2.5]，B 组缺失
#    complete-building-unit.bin 全 0

# 3) Rust 侧零命中（效果列未实现）
rg -n "0E96|E965A|E965B" src-tauri/src   # 无输出

# 4) 排序数组长度
rg -n "0DC1_E3E0|ui_position" src-tauri/src/asset_build_m1/menu.rs
#  1575/1739: vec![Value::Int32(spec.ui_position.unwrap_or(11500))]
```

---

## 8. 一句话结论

Rust 目前迁移的是 **Packager 的“外部形态”**（Asset 链克隆、灯光列、道具列、菜单身份、地块数字链），
但**没有迁移 Packager 的“公园配方本体”**（8 跳 Unit 链 + 46 哈希 pin + 12 效果列 + 逐字段回读 verify）；
同时菜单排序/图标/侧车校验这几条“用户可见但不在几何链上”的规则被简化成了固定值或缺失。
按“1:1 原样迁移”的标准，**P0 三条未修之前，不能说公园行为已迁移**。
