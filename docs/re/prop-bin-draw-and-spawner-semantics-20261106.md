# Prop Bin 绘制与 Spawner 锚点语义（实锤版）

日期：2026-11-06。目标：解释"工厂货架堆货、写字楼停车场出现车辆"为什么此前失败；
以及 spawner（小人/道具锚点）的内部语义与资产开发者的选取方法。

证据：反编译 `SC::cUnitBinDraw`（FillFromProps 0x7908E0 / Update 0x78DF40）、
`SC::cUnitModel::FillFromProps`（spawner 列）、`SC::cChoreographyAnimations`；
实包转储 `tmp/s14-full-dump.txt`、`tmp/spawners-game.json`（472 个 lot / 257 个
spawner id）；工作台注册表 `database_main.s3db` 名字表。

## 0. 结论速览

1. **"货架/停车场的 prop"不是普通模型，是 bin-draw 实例**：`cUnitBinDraw` 按
   **unit 资源 bin 的数量**每帧增减实例。资产必须有 ①bin-draw 三列
   （资源 ID / transforms / slot 索引）②对应的资源 bin ③真正把它填起来的
   规则/交通流。缺任何一层，货架永远空、停车场永远空。
2. **spawner = cUnitModel 的 UnitLocation（id + transform）**，id 是 32 位
   **名字哈希**（与资源同 FNV 命名空间），不是 DBPF 资源键，运行期查表。
3. **锚点不编码职业**：小学与警察局共享完全相同的 18 个锚点，但一个刷老师
   学生、一个刷警官——职业由建筑的岗位/agent 资源与规则决定。锚点编码的
   是**位置类别**（边界/临街/停车/屋顶…）。
4. 所以资产开发者的正确做法不是"发明 spawner"，而是**按功能克隆参照建筑
   的锚点集**（vanilla 自己也是这么做的：9 个通用锚点出现在 443–455 个 lot 上）。

## 1. Prop Bin（bin-draw）机制：源码定案

`SC::cUnitBinDraw::FillFromProps`（0x7908E0）：

- 固定 **15 个绘制组**（set 0..14），每组三列平行：
  | 列 | 属性 | 内容 |
  |---|---|---|
  | 资源 | `0x0C12EF20 + set`（`202567456+set`） | Key[]，本组要绘制的资源 ID（如 CarW1/W2/W3） |
  | 位置 | `0x0C12EF30 + set`（`202567472+set`） | transform[]，每个实例的落点 |
  | 槽位 | `0x0C12EF40 + set`（`202567488+set`） | int[]，实例→槽位索引 |
- 对每个资源 ID：`IndexFromResourceID` → `unit->mResourceBins.find`；**找不到
  资源 bin → 该资源整组跳过**；`drawCapacity` 取该 bin 的 **capacity**。
- `SC::cUnitBinDraw::Update`（0x78DF40）：每帧取 bin 的 **amount**；
  若 `mUseBinPercent` 则按 `amount / capacity` 折算；与当前可见实例数比较后
  **增减实例**。→ 绘制数量 = 资源数量（或百分比），不是静态摆件。

**实包验证（小学 C264A268，lot 列转储）**：
- set 0 资源 = `0AB4DFE1/E2/E3` = **`kResourceIDCarW1/W2/W3`**；transform 5 个，
  位置在东侧 (28.2,32)/(30.4,27.2)/(30.4,22.7)/(30.4,18.4)/(28.2,14) ——
  **就是 5 个车位**。bin 里的 CarW1/2/3 由通勤规则填充 → 停车场的车。
- set 1 资源 = `BF2C1022` = **`Tent`**；5 个 transform 与 5 个 UnitLocation
  锚点（`16C1AE2x`）**位置逐一重合** → 道具锚点 + bin-draw 联动。
- **工厂同款验证（炼油厂 A50FC927）**：lot 同样有 bin-draw 三列——
  set 0 = `CarW1/W2/W3`（职工停车），set 4/9/12 =
  `2F207D6B`/`14984C6B`/`14984C68`（产品/物流资源，注册表未命名）。
  即"货架堆货"与"停车场出车"是**同一套 cUnitBinDraw**，只是绑定的资源不同：
  前者绑产品资源（由生产规则填充），后者绑 CarW1/2/3（由通勤规则填充）。

**为什么此前失败**（对照）：
| 缺失层 | 现象 |
|---|---|
| 没有 bin-draw 三列 | 模型永远不出现（编辑器只放了静态 prop） |
| 有列但资源 bin 不存在 | `find` 失败 → 整组跳过（静默） |
| 有 bin 但 amount 恒 0 | 每帧绘制 0 个（生产/通勤规则没写或没启用） |
| 资源未在资源组注册 | `IndexFromResourceID = -1` → 整组丢弃（静默） |

## 2. Spawner（UnitLocation）语义：源码 + 实包

`SC::cUnitModel::FillFromProps`：
- `0x0E1BAC61`（`236694625`）= id 数组（Key[]，type/group 全 0，仅 instance）
- `0x0E1BAC62`（`236694626`）= transform 数组
- 成对读入 `mUnitLocations: vector_map<u32, cSPTransform>`；`0x0E715928/9`
  在本函数里实际是 `mBuildingVariation` 系列（不是 spawner 数量，旧探针注释有误）。
- id **不是 DBPF 键**：472 个 lot 的 7761 条锚点 id 在脚本/Game/Graphics/EP1
  里全部查不到资源；它是 32 位名字哈希（同一命名空间下 `student` 的 FNV-1 =
  `0x2C1DA112` = `kResourceIDStudent`），运行期在引擎表里解析。

**锚点的频次分层（472 lot / 257 id）**：

| 层 | 典型 id | 出现 lot 数 | 位置特征 | 语义（推断，证据=位置+频次） |
|---|---|---:|---|---|
| 通用边界锚 | `125D19FA`/`980F070E`/`684C5FCA`/`700770E0`/`141EED53`/`AAB85CBB`/`1909178F`/`10B3AC30`/`33A4F764` | 443–455 | lot 四角 + 中线 | 行人/车辆进出地块的边界点 |
| 临街锚 | `3B8ADFE0/E2/E3/E5` | 366–373 | 临街一侧 | 人行道/入口 |
| 道具/车位锚 | `16C1AE28/2A/2B/2C/2D` | 9–11 | 与 bin-draw 车位重合 | 道具生成点（小学=帐篷位） |
| 屋顶工作锚 | `C3D8D690/91/92/97` | 13 | z≈29.9 | 水塔顶的工人位置 |
| 建筑专属 | 其余 ~235 个 | 1–37 | 各异 | 各功能专用锚 |

**关键否定证据**：小学与警察局的 18 个锚点**完全相同**（9 通用 + 4 临街 +
5 车位），水塔只额外多 4 个屋顶锚。→ **锚点不编码职业**；老师在草地、警官
在警局，是建筑的岗位/agent 资源与规则决定的，锚点只提供"在哪里出现"。

**动画层**：`0x0DA7CAD4/5`（`cChoreographyAnimations`）存"锚点→动画"；
小学上解析出 `B8F33895`=**Cycle**、`2A72B9A8`=**run** 等动画名。即：
锚点（位置）+ 动画（动作）+ 岗位（谁）三层组合出"老师站着、学生围一圈"。

## 3. 对资产编辑器的直接设计

### 3.1 Prop-bin 能力（货架/停车/堆货）

```
PropBinSet {
  resource: <已注册资源 ID>      // 如 kResourceIDCarW1、产品资源
  transforms: [..]              // 每个可见实例的落点
  slots: [..]                   // 实例→槽位
  mode: amount | percent        // mUseBinPercent
}
```
- 编辑器必须先做**资源存在性校验**：资源未注册 → 构建失败（否则静默丢弃）；
- 必须确认该资产的 bundle/规则会**填充该资源 bin**（否则永远 0）：
  工厂=生产规则写入产品资源；写字楼=通勤规则写入 CarW1/2/3；
- 数量语义是"资源量"，UI 应显示"当前 bin 量 → 预期可见实例数"。

### 3.2 Spawner 能力（锚点）

- **禁止发明 id**：id 是名字哈希，运行期表不可枚举；新 id 不会解析；
- 采用** donors 锚点模板**：从同功能参照建筑克隆锚点集
  （通用 9 + 临街 4 + 功能专用：车位/屋顶/码头等）；
- 编辑器展示每个锚点的：位置、所属层（边界/临街/车位/屋顶）、
  全局使用频次（可信度）；
- 职业不放在锚点上：由资产的岗位/agent 资源与规则决定——这与"行为闭环"
  的 bundle 配置是同一层，锚点只管位置。

### 3.3 验收矩阵（新增）

| 目标 | 可观测 |
|---|---|
| 工厂货架 | 产品资源 bin amount 上升 → 货架实例数上升 |
| 写字楼停车 | 通勤时段 CarW1/2/3 amount > 0 → 车位车辆出现 |
| 学校草地 | 临街锚点 + Cycle/run 动画 + 岗位资源 → 老师/学生出现 |
| 水塔屋顶 | 屋顶锚点 z 高度正确 + 工人岗位 → 工人在塔顶 |

## 4. 未决（明确边界）

1. 257 个 spawner id 的名字无法静态反解（哈希）——需要运行时 hook
   `mUnitLocations` 的消费点或引擎名字表；目前只能按频次/位置分层。
2. 15 个 bin-draw 组中，set 0/1 已验证（CarW1-3、Tent）；其余组的资源名
   未全量解析（部分未注册），可按同一方法继续补。
3. `mBuildingVariation`（0x0E715928/9）与外观变体的关系未展开。
4. 工厂货架的"产品资源"具体 ID 未取（需选一个工厂 lot 做同款转储；
   方法与小学一致）。

## 证据文件

- 探针：`probes/lot/spawner_probe.rs`、`spawner_names.rs`（472 lot 锚点导出）
- 数据：`tmp/spawners-game.json`、`tmp/spawner-ids.txt`、`tmp/spawner-names.txt`
- 源码：`src/SC/cUnitBinDraw/{FillFromProps,Update}.c`、
  `src/SC/cUnitModel/FillFromProps.c`、`src/SC/cChoreographyAnimations/FillFromProps.c`
- 相关：`docs/re/plop-behavior-closed-loop-census-20261106.md`（闭环表）、
  `docs/design/behavior-capability-composition.md`（能力组合架构）
