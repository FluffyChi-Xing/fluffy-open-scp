# Lot Unit 与建筑树模型资源链

这份说明对应 `dev/rendering-optimisation` 当前的 `sc-properties` 解析器，并结合渲染端对原版属性链的审计结果。目标是让编辑器从一个 lot property 文件追到正确的建筑附属物、树、特效和生成器；其中的 hash 是游戏属性列，不是可直接当作 DBPF 资源的文件名。

## 先给结论：建筑旁边的树来自哪里

建筑旁边的树是 **lot prop**，不是建筑模型的子 mesh，也不是 `spawner` 生成的临时 Agent。当前保存城市使用的四种树原型都来自原版 **`SimCity_Game` 包**，模型资源的提取文件如下（文件名格式为 `type_group_instance.bin`）：

| 树类型 | 原型定义键          | 树定义                        | 模型资源                     | 提取路径                                                |
| ------ | ------------------- | ----------------------------- | ---------------------------- | ------------------------------------------------------- |
| 0      | `ADA2724C:40E02D00` | `C602CD31:40002D00`，type `0` | `04B1B100:002DE040:4C72A2AD` | `extracted/SimCity_Game/04B1B100_002DE040_4C72A2AD.bin` |
| 1      | `ADA2724D:40E02D00` | `C602CD31:40002D00`，type `1` | `04B1B100:002DE040:4D72A2AD` | `extracted/SimCity_Game/04B1B100_002DE040_4D72A2AD.bin` |
| 2      | `ADA2724E:40E02D00` | `C602CD31:40002D00`，type `2` | `04B1B100:002DE040:4E72A2AD` | `extracted/SimCity_Game/04B1B100_002DE040_4E72A2AD.bin` |
| 3      | `ADA2724F:40E02D00` | `C602CD31:40002D00`，type `3` | `04B1B100:002DE040:4F72A2AD` | `extracted/SimCity_Game/04B1B100_002DE040_4F72A2AD.bin` |

`ADA2724C` 到 `ADA2724F` 是 runtime prototype key；它们本身不是 RW4 mesh 的 TGI。正确解析顺序是：

```text
lot property 的 prop bin
  -> 运行时资源 ID（例如 14984C6A）
  -> EcoGame resource definition
  -> graphics override（0x08E17ED7）或 graphics group
  -> prototype（0x0D8C29C3）
  -> static model（0x0D897169 / 0x00F9EFBB）
     或 tree definition（0x0C36D30D + 0x0D8C29CF）
  -> tree model list（0x0BD62577[type]）
  -> SimCity_Game 中的 RW4 mesh
```

因此不能把 lot 中的 `14984C6A` 直接当模型文件名；它只是在运行时资源表中选中一个树原型。例如导出的 lot 数据中，`14984C6A` 最终解析为 `ADA2724D:40E02D00`，即树 type 1。

树 mesh 是 `NonImpostorTree` 材质族：内部 raster slot 0 为 diffuse，slot 1 为 normal，slot 2 为 AO；AO 使用第二套 UV（导出时要把 `TEXCOORD0` 与 `TEXCOORD1` 合并到 vec4，渲染时采样 `vUV.zw`）。树出现黑色或只有白模时，通常是只加载了 slot 0，或丢掉了第二套 UV。对应实现见主工程的 `tools/render/export_surfaces.py` 与 `site/live/surface-shaders.js`。

## Props / bin：树、垃圾桶和其他地块装饰

`crates/sc-properties/src/lot_unit.rs` 将 lot property 中的并行数组装配为 `LotUnit::Prop`：

| 列            | Hash               | 语义                                          |
| ------------- | ------------------ | --------------------------------------------- |
| 资源 ID       | `0x0C12EF20 + bin` | runtime prop/resource 标识；不是直接 DBPF TGI |
| 变换          | `0x0C12EF30 + bin` | WPF `Matrix3D` 的 12 个 float                 |
| slot          | `0x0C12EF40 + bin` | 将资源分配到该 bin 的变换槽                   |
| 随机槽        | `0x0C12EF50`       | 原版槽位随机化开关                            |
| 百分比填充    | `0x0C12EF60 + bin` | 按资源容量计算填充数量                        |
| 原型 override | `0x0C12EF70 + bin` | 保存文件直接指定 prototype 时使用             |

OpenSCP 当前 `PROP_BINS = 14`，按原 C# `PropertyFileObjectCollectionArray(14)` 读取 bin `0..13`。主工程对发行数据的审计还发现 `0x0C12EF20..+14` 的第 15 组列，渲染端因此按 `range(15)` 检查。这里要保留这个差异：不要把 bin 中的 ID 当模型 TGI，也不要在没有真实包样本验证时盲目改变 OpenSCP 常量。

prop 原型引用的 `type/group` 通常为 0；原版引擎用运行时哈希表（反编译线索 `FUN_00787870 -> FUN_0058ec70`）解析它，离线扫描 DBPF 无法直接得到同名资源。编辑器可以展示该标识；要渲染模型，必须接入游戏定义包、graphics override、prototype 和 LOD 链。

`Transform.Unknown` 在 flags 为 `15` 时承载 scale，这条约定同时用于 prop 和 decal。`UnitTransform` 是行主序 12 floats：前三行是基向量，第四行是平移。渲染实例时先应用原型的 `modelScale/modelOffset/modelRotation`，再乘 prop instance matrix。

### 定位一棵具体的树

1. 从 lot 的 `0x0C12EF20+i` 找资源 ID，从 `0x0C12EF30+i` 和 `0x0C12EF40+i` 取变换与 slot。
2. 若存在 `0x0C12EF70+i`，先使用 override；否则用当前游戏依赖的 resource group 找资源定义。
3. 读取 `0x08E17ED7`，跳到 graphics override 或对应 graphics group。
4. 读取 `0x0D8C29C3` 得到 prototype。
5. prototype 有 `0x0D897169` 或 `0x00F9EFBB` 时直接取静态模型；没有时读取 `0x0C36D30D` 的 tree definition 与 `0x0D8C29CF` 的 type，再取 `0x0BD62577[type]`。
6. 用完整三元组解析 RW4 mesh，并按材质族加载 diffuse、normal、AO 与两套 UV。

可先用主工程导出的 `site/live-data/lots.json` 验证 prototype 链，再回到 OpenSCP 的 package/property 编辑器定位原始列；不要只搜 `ADA2724*` 文件名，因为它们是 runtime prototype key。

## Spawners

spawner 是 lot 中的生成器记录，不是静态 prop：

| Hash         | 字段                   |
| ------------ | ---------------------- |
| `0x0E1BAC61` | spawner ID（`Key`）    |
| `0x0E1BAC62` | spawner transform      |
| `0x0E715928` | 初始生成数量 `count`   |
| `0x0E715929` | 随机上限 `countRandom` |
| `0x0F0E2BF1` | Agent 引用             |

每列按同一个 index 并行装配为 `LotUnit::Spawner`。原版数量语义为 `count + rand % countRandom`；发行包里数量、随机上限和 Agent 列很少出现，列缺失不代表 spawner 无效。当前编辑器可显示 ID、transform、count、countRandom 和 agent，但 Agent 的完整运行时模型/行为解析仍需要游戏定义和模拟系统支持。

扫描发行包：

```text
cargo run -p sc-properties --release --example spawner_probe -- <package> [max]
```

探针会统计含 spawner 的 lot、三组可选列是否出现，并检查 Agent instance 是否能在同一包中命中。

## Path

这里的 path 是 lot unit 中的路径点数据，用于编辑器和生成器关联；它不等于道路网格，也不是 transport simulation 的完整道路几何：

| Hash         | 字段                                      |
| ------------ | ----------------------------------------- |
| `0x0CAA680D` | 点坐标 `Vector3`                          |
| `0x0CB00ED8` | 切线 `Vector3`                            |
| `0x0CAA6832` | 点索引 `Int32`                            |
| `0x0CAA6841` | `Int32` 点区间对（`LotUnits.path_pairs`） |

`PathPoint` 的前三列按 index 并行装配；`PATH_PAIRS` 是独立的 Int32 数组。绘制曲线时使用 point + tangent，并依据 pairs 连接区间；不要把点索引当 DBPF resource instance。

## Effects

效果记录的列为：

| Hash         | 字段                             |
| ------------ | -------------------------------- |
| `0x02A907B5` | effect 的旧式 ID 列              |
| `0x02A907B6` | effect transform                 |
| `0x02A907B9` | 原版保留列，当前作为 fields 保留 |
| `0x02A907BB` | `effectId` 引用 `Key`            |
| `0x02A907BC` | enabled                          |

OpenSCP 将这些列装配为 `LotUnit::Effect`，前端显示 transform、effectId、enabled 和原始 fields。`effectId` 仍是定义引用，不能直接当 mesh 或贴图；粒子、灯光、烟雾等实际效果还需要解析对应的 runtime effect/material 定义。当前仓库没有把所有 effect 定义还原成 GPU 材质，因此编辑器显示 marker 是诊断行为，不代表已完成游戏内效果渲染。

## 装配顺序、矩阵和诊断

`assemble_units` 顺序是 lights、effects、spawners、path points、props、decals，最后读取 `path_pairs`。数组列长度不一致时按最长列建立 unit，缺失字段为 `None`，异常矩阵写入 `diagnostics`；这与原版 `PropertyFileObjectCollection.Load` 的并行列行为一致。

建议先运行 Rust 单元测试，再用探针对真实包做引用链审计：

```text
cargo test -p sc-properties lot_unit
cargo run -p sc-properties --release --example prop_chain_probe -- <package> [instance]
cargo run -p sc-properties --release --example spawner_probe -- <package> [max]
```

`prop_chain_probe` 会扫描 type `0x00B1B104` 的 lot property，打印 `0x0C12EF30..` 和 `0x0C12EF40..` 列，适合确认 slot、transform 和 runtime ID 是否同时存在。模型/地块偏移诊断：

```text
cargo run -p sc-exporter --release --example footprint_project -- \
  <lot package> <model instance hex> [--out dir] [lookup package...]
```

## 当前边界

- prop ID 是运行时标识，离线 DBPF 扫描不能替代 graphics/prototype resolver。
- OpenSCP 的 14-bin 读取和渲染端发现的第 15 组列存在差异，需要更多真实 lot 包验证后再决定 API 是否扩展。
- 树的四个原版 mesh 已确定来自 `SimCity_Game`；正确外观还必须保留 `NonImpostorTree` 的 slot 1 normal、slot 2 AO 和第二套 UV。
- spawner 的数量随机化、Agent 行为和 effect 的最终材质仍依赖游戏 runtime 定义；lot unit 解析只负责读取结构化引用。

### 相关源码

- `crates/sc-properties/src/lot_unit.rs`：hash 常量、DTO、并行列装配和单元测试。
- `crates/sc-properties/examples/prop_chain_probe.rs`：prop slot/transform 探针。
- `crates/sc-properties/examples/spawner_probe.rs`：spawner/Agent 探针。
- `crates/sc-exporter/examples/footprint_project.rs`：模型与 lot placement 的脚印诊断。
- 主渲染器 `tools/render/lot_props.py`：resource → graphics → prototype → tree model 的 resolver。
- 主渲染器 `tools/render/export_surfaces.py`、`site/live/surface-shaders.js`：树的 diffuse/normal/AO 与 UV 通道。
