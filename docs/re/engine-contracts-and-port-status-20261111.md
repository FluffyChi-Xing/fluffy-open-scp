# 引擎契约与迁移进度总表（2026-11-11 汇总）

本文是今天全部逆向、对拍、修复与教训的**唯一入口**。细节在各分册，结论以本文为准。
所有数字都来自逐字节解包或零售引擎反汇编，标注了证据位置；未验证的推论单独标注。

---

## 0. 一分钟版

| 系统 | 机制定案 | 希望大厦现状 | 下一步 |
|---|---|---|---|
| 劳工 | bin+sink+规则；红字判据 = `TotalWorkers` vs `minWorkers` | ✅ 31/30 上岗、换班、走 path 进门 | 无 |
| 学生 | 同架构；闸门 = `SpotsForKids` 的 amount | 不适用（本资产非教育） | 教育资产按 §4 配对 |
| 观光 | 同架构；`AttractionLevel` + `TouristsAllowedW*` 配额 | ❌ 配额仅 10/10/10（Willis 150/75/50） | 提量 + 面板改绑 |
| 夜灯 | 内景控制器 + 内景图集第 1 列（alpha=自发光） | ❌ 仍黑（控制器/图集列未对齐） | 见 §3.3 |
| 电源按钮 | 与电量无关；有可关停班次才渲染 | ❌ 缺失 | 待查面板显隐条件 |
| 模型构建 | 与 Packager 对齐，LOD 载荷字节数一致 | ✅ 4×4,869,140B 完全相同 | 部署验证 |

---

## 1. 引擎机制（源码级事实）

证据树：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree`
（约 4.2 万个 CodeDumper 反编译函数文件，GB=EcoGame 模拟引擎，SC=图形/交通）。

### 1.1 通用事务模型

引擎里**没有"劳工/学生/游客"这些概念**。三者是同一套机器：

```
规则(Rule) ──mTransportID──> 事务(Transaction) ──交通层──> sink 交付 ──规则记账──> bin 变化
```

- 发车闸门：`cTransportTraffic::CanDeliverTransaction`（0x8341E0）要求规则
  `mTransportOps` 里**任一 sink entry 的路径池 `mTotalSinkCapacity > 0`**，否则一个 agent 都不发。
- 目的地 = 规则的 `mTransportOps` 列表（数据顺序），**不做"就近搜索"**。
- sink 容量 = `SinkCapacity`（0x616BF0）：`key bin capacity − amount`；
  `invert=true` 时取 `amount`；**无 key bin 且非 connected → 硬编码 fallback 10**。
- sink 绑定表在 bundle 上：`0x0A2725CE`（sink entry）/ `0x0A2725CF`（规则）/ `0x0AF38802`（资源）。
- bin 声明 `0x07E0AD23/24`；属性→bin 映射 `0x0A6FE749..4E`（`AddBundlePropOverrides` 0x644F20），
  **只对被引用 bundle 生效**，写在 Unit/Lot 自身的 core bundle 会被静默跳过。
- 应用规则 `ApplyBinRule` 0x64BBA0：unit 上不存在的 bin = **静默跳过**（不报错）。

### 1.2 劳工

| 事实 | 证据 |
|---|---|
| 红字 = `TotalWorkers`(0x1FD156B6) amount < `MinimumWorkersForProduction`(0x0A7917C8) | `template_registry.js:5855` |
| `TotalJobs`(0x0A7917C3) **引擎零引用**，纯 UI | 全树 grep |
| 岗位 `JobsW1/C6/C7` → bin 经 bundle 映射表；工人槽 `0x0E3F65A9/B3/B4` → `WorkersNeededWx` | 实测映射表 |
| 排班 8 列 `0x0D00853A..41` C++ 无读者，数据侧 labor 规则消费（全天 0 = 无班次） | s31 游戏内换班实证 |
| 引擎无财富档过滤；三档是独立资源 ID | 全树 grep |

### 1.3 学生

| 事实 | 证据 |
|---|---|
| 学生 agent 携带 `KidE2/KidE3` + `KnowledgeToken`（无 `StudentW*`） | transport entry `0D19BDC8/9` |
| 接收闸门 = sink `78B96709` 的 key bin `SpotsForKids`(0x05DAADA6) amount（invert） | `SinkCapacity` |
| `0x0AD7FAC9` **不是学生容量**，只喂 8 个 `StudentsNeeded_00..21` 槽的 capacity | `6C042367` 映射表 |
| **容量列与 bundle 族必须配对**：小学/中学 `6C042367`→`0AD7FAC9`+初值 `0B98804E..63`；社区/大学 `6C042366`/`3256C0A2`→`0AD93A77`+初值 `0B9880C6..DB`；全库 1214 unit **无一同时具备两列** | 19 教育单元普查 |
| `mSkills` 技能子系统本版休眠（0 unit 声明 SkillID/阈值） | 全库频次统计 |

### 1.4 观光

| 事实 | 证据 |
|---|---|
| `AttractionLevel`(0x54FC7890)、`TouristsAllowedW1-3`(0x96F3554C/4F/4E)、`TouristW1-3`(0x1A7B1745/46/47)、`TotalShoppers`(0xC1F569B9)、`OpenShoppers`(0xCE6E3D2D)、`SouvenirsW1-3`(0x6AE939A5/A6/A7) | FNV 校验 |
| **游客 bin 全在模板层**：19/19 地标 parent=`4E01E311`；模板 A4 带 `E15369A2`(AttractionLevel+TouristsAllowedW1-3) + `75D22FD0/1/2`(TouristW1-3+TotalShoppers+OpenShoppers+SouvenirsW1-3) | 地标普查 |
| 叶层 bundle（`CF116B9C`/`2B5A2BAF`/`362087BD` 等）**非共性非必要**：10/19 地标叶层 A5=0 照样观光 | 同上 |
| 每日配额属性 `0D8C29F6/FE/A01`：Willis=150/75/50（275/天），希望=10/10/10（公园 donor 继承） | 逐包解包 |
| 卡片 `369/390`：390=`kPropShopping_TotalShoppers`；369=`ShopperW1+2+3`=143+11+215 | UI 查询链 |

### 1.5 夜灯（最反复的一项，最终定案）

- **窗格灯不是几何，也不是 Unit 属性**：shader 用 `[q*d*s, q*s, s]` 三个阈值与随机房间数
  比较，决定内景图集（六槽 slot5）四列中的哪一列发光（`building-facade-engine-analysis.md` §2.1）。
- 内景控制器：`0x0081D020` 创建（无居民/工人来源时需图形资产 `0F169C46=true`）→
  `0x0081CD60` 初始化（读 `0F169C46`、`0E0BE186`）→ `0x0081CDF0` 更新（active?0.85:0）→
  `0x0081CC80` GetInfo → `0x00783BC1` 上传。
- **控制器为空 → 固定阈值 `(1,1,1)` → 永远第 0 列**；本仓库材质把夜景发光编在
  **第 1 列**（`state%4==1`，与包作者 `audit_window_night.py` 契约一致）→ 永久黑窗。
- 证据：`D:/ea-games/simcity_offline/entity/001/第八阶段…md`、`第九阶段…md`（零售引擎反汇编）。

---

## 2. 四样本对拍（数据事实）

| 样本 | parent | 岗位 W1/2/3 | minWorkers | 工人槽 | 学历 | 排班 | 吸引力 | 每日配额 | 实测表现 |
|---|---|---|---|---|---|---|---|---|---|
| **s31 大学** `1ea3cf9a` | `7E4B99E5` 教育 | 20/10/4 | 12（35%） | 10/0/0 | 10/5/4 | 6/7 族 | 1 | — | 500 学生满 + 劳工换班 + 公园吸引孩童 |
| **威利斯** `8D007745` | `4E01E311` 地标 | 150/150/60 | 60（17%） | 75/75/15 | 75/75/45 | 116 族 | 400 | 150/75/50 | 105 劳工 + 369 观光客 |
| **HTzhiyao** `97DF9619` | 无 | 5/2/0 | **0** | 0/0/0 | 5/2/0 | 全 0 | **1** | 缺 | 从不抱怨（零 wiring） |
| **希望大厦** `41da7118` | `7E4B99E5` | 30/0/0 | 5 | 30/0/0 | 15/0/0 | 116 族 | 400 | **10/10/10** | 31 劳工；灯黑；游客 0 |

**判据**：抱怨与否只由 `minWorkers / 可达到岗` 决定（可达 ≈ W1×0.86）；
到岗多少由 wiring 决定。HT 是"退出系统"的样本，不能当少写列的依据。

---

## 3. 构建器迁移状态（对 `reference/sc5-packager-0.8.0/`）

分册：`docs/re/port-gap-model.md`、`docs/re/port-gap-behavior.md`（含逐条改法与验收断言）。

### 3.1 模型侧（已全部完成）

| 项 | 内容 | 验收 |
|---|---|---|
| M1 | `scene.nightEmissionPolicy`：`night-only`/`preserve-source`，非法值 fail-closed | 编译通过，缺省与旧构建一致 |
| M2 | `compact_2048` 逐通道 BOX 降采样 | **图集 89,478,560B → 22,369,692B（4096²→2048²）**，省 201MB/包 |
| M3 | 退化/零面积三角形 → 构建失败 | 报 group + 三角形号 |
| M4 | 默认旋转按 LOTM `diagnostics.sourceFrontAxis`；缺字段保持旧行为 | `+Y`→0°、`-Y`→180°、缺失→沿用 |
| M5 | 图标一律 128×128 Lanczos 归一 | 1254×1254 已归一 |

**与 Packager 的实测对拍**（`scripts/diff_model_side.py`）：
LOD1-4 载荷 **4,869,140B ×4 完全相同**；碰撞 33,140B、阴影 16,777,588B 相同；
差异仅实例 ID 与共享 surface/normal 图（我们内嵌，Packager 依赖游戏内共享资源）。

### 3.2 行为侧（未完成，P0）

1. `native-park-w1-v1` 克隆配方：8 跳 parent 链（起 `40E0C000:59292C11`）+ 46 资源哈希 pin
   + 12 个效果列（`0E965A70..75`/`0E965BE0..E5` = `1.0`/`[2.5,2.5]`）；
   我们当前用静态 `tested-unit.bin`（= 基准 legacy 基座），该 12 列实测全为 `0.0`。
2. `verify()`：逐字段 recordHex 回读比对 + PARK 禁止 `00B2CCCB/0BF5D2E3/0BF5D2E4`。
3. `42E1C000` 图形单元补全：17 组 UnitLocation、`0C12EF24-2A` 路网键、`0CA5CF4A` state 键。

### 3.3 夜灯在构建侧仍缺的两条

- 图形资产当前只有 parent/bbox/模型引用/两个开关 → 需补 §3.2.3 的三组列；
- 控制器创建开关 `0F169C46` 已写（图形资产 + Lot），但**实例/检索键未实证**；
  下一步用 `cGraphicsUnitInteriors::FillFromProps` 的 `0CA5CF4A/B` 检索键校准。

---

## 4. 今天被推翻的判断（写在这里避免重犯）

| 曾经的结论 | 推翻依据 |
|---|---|
| 「需要劳工」= 岗位没填满 | 红字只比 `TotalWorkers` 与 `minWorkers`；Willis 29% 填充不红 |
| 财富/教育档不匹配是主因 | 引擎无财富档过滤；清零 W2/W3 后到岗比例不变 |
| 工人槽 10 是硬上限 | 实测 26 > 10 |
| `lightswhenactive` 是窗灯来源 | 它只决定"运营中整体提亮"，窗灯来自内景图集列 |
| parent 换地标导致熄灯 | Lot 仅差 180° 镜像、Unit 仅差 6 列；真正差异是模型槽位载荷 |
| 观光地图/面板开错 | 观光地图是正确判据；面板那三行读的是公园访客属性 |
| 电源按钮与电量有关 | 用户实证：所有 plop 建筑都有，与电量无关 |

---

## 5. 教训与已固化规则

**反复回归的统一模式**：未原样迁移 → 进游戏才发现 → 回头对拍 → 把没照抄的补上 → 一遍过。
已发生十余次（缺面、缺贴图、夜灯、观光、劳工）。

已写入 `AGENTS.md`（每会话生效）的六条：

1. 唯一基准是 `.py` 源码；迁移 = 逐函数照抄，能指出函数与行号；
2. 如无必要勿增实体：已验证正常的模块不改不重构；
3. 新功能新增模块，旧路径字节级不变；
4. 改动必须可归因（基准段落或已复现缺陷）；
5. 普适性判据：只对单栋楼有效的修法 = 没修好，应补进通用契约；
6. 每修一个缺字段/缺资源缺陷，同时补构建期硬闸门。

已落地的闸门：
- `package.rs` night-interior（Lot `0F169C46` 回读）；
- `menu.rs` 图形单元契约（`0F169C46` + `0E0BE186` 必须 true，缺则 fail）；
- `single_compatible` 房态契约（源有 emission 而夜景列全空 → fail）。

---

## 6. 部署台账（今日希望大厦）

| 哈希 | 内容 | 结果 |
|---|---|---|
| `125ed5c7` | /48 旧基准（学历分布缺失） | 0 人，需劳工 |
| `204c0738` | 学历分布 6 列修复 | 26 人（<minW 30 → 红） |
| `f2be3587` | minW 30→10 + 8 班 | 10→25 人 |
| `bc066263` | HT 14 坑 | 6 人 |
| `7ce0cd77` | 全 W1 14 | 6 人 |
| `03b056b3` | workers-only 直觉输入 | 6 人 |
| `23fafa95` | parent 换地标（**错误方向，已回滚**） | 灯黑 |
| `fe63897f` | 图形单元（仅 0E0BE186） | 灯仍黑 |
| `41da7118` | 图形单元补 0F169C46 | **31 人上岗 + 换班 + 进门；灯黑、游客 0** |

台账与归档：`D:/ea-games/simcity_offline/test-archive/MANIFEST.md`（每次部署/归档均有 SHA-256）。

---

## 7. 工具

| 工具 | 用途 |
|---|---|
| `scripts/diff_model_side.py` | 任意包的 LOD/图形单元/Lot 模型侧字段对拍 |
| `cargo run --example dump_property_blob -- <blob>` | RefPack 解压 + 属性表逐列打印 |
| `cargo run --example dump_unit -- <blob>` | 未压缩属性表打印 |
| `cargo run --example dump_tgi -- <pkg> <t> <g> <i>` | 按 TGI 直读包内资源 |

---

## 8. 待办（按优先级）

1. 行为侧 P0（§3.2）——决定劳工/观光/开关按钮的最终一致性；
2. 夜灯：图形单元补全 + `0CA5CF4A/B` 检索键校准；
3. 观光：配额三列提到 Willis 量级 + 菜单统计行改绑观光字段；
4. 部署 `8a6e591e`（含 M2 图集修正，包体 −201MB）并做多建筑同契约验收
   （希望大厦 + 学校 + 地标各一次，全部通过同一组闸门）。
