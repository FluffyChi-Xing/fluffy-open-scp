# 新区域初始化与 EGB 生命周期

## 核心结论

新区域不是先读取一个已经存在的 EGB 再生成地图。游戏先根据脚本、区域模板和静态资源在内存中创建 `cEcoGame`，初始化 EcoMap、Unit、Zone、Path 和 handler，之后才把运行时状态保存为 `state_file_*.egb`。

```text
CreateGames
  → cEcoGame::StartGame
  → scripts / SimProperties / NonSimProperties
  → cEcoMap 初始化
  → PrecreateUnits / PrecreateZones
  → PathCreate / RoadManager
  → handlers 启动
  → 保存 live state 为 EGB
```

## 初始化顺序证据

`cGameAppMode::CreateGames` 创建 active/region `GB::cEcoGame` 对象。真正的地图初始化从 `StartGame(gameID, environmentID, addOns)` 开始：

1. 读取当前 Game Scripts；
2. 创建 map-entry 对应的 `cEcoMap`；
3. 从 `0x7E0AE06` 和图像资源初始化各 map；
4. 创建 handler、路径、区域单位和 zone 状态；
5. 启动 handlers；
6. 执行 `PrecreateUnits` 与 `PrecreateZones`；
7. 路径 handler 消费已创建的 `cEcoPathSet`。

这些结论来自 `docs/source-code/SimCity-dev-ida.c` 和 `docs/source-code/_legacy_ghidra_dump/GB_cLocalSaveManager.c`。

## 初始 Unit 与 Zone

`PrecreateUnits` 读取：

```text
0x0ADFE207 → Unit keys
0x0ADFE208 → cSPTransform
```

`PrecreateZones` 读取：

```text
0x0B4D6794 / 0x0B4D6795 → zone keys/transforms
0x0D4130F3 → path zones
```

当前脚本包中的通用两行 Unit 表是真实存在的，但没有包含 CapeTrinity 的完整道路网络和世界放置。因此不能把零变换的通用 Unit 表当作 CapeTrinity 道路的 transform。

## 初始道路

道路 Unit 的 `NonSimProperties` 由 `CreatePathsFromProps` 解析：

- `0x0CAA680D`：点；
- `0x0CB00ED8`：切线；
- `0x0CAA6841`：段范围/索引；
- `0x0CAA6832`：点索引；
- `0x0CC8FE7A`：PathEntry；
- scope/connect 字段：路径作用域和连接行为。

随后 `cUnitPathCreationLocal::CreatePath` 写入 `cEcoPathSet`，RoadManager 与 GraphicsPath 消费同一套段和切线。道路可视化不是另一个独立地形层。

## EGB 为什么可以不存在

存档管理器中用于读取的路径会打开 `state_file_0_<id>.egb`，读取失败时返回失败；这只是 load-existing-state 分支。新区域分支已经在内存中完成 `StartGame`，所以第一份 EGB 是 live object/delta state 的后续快照。

观察到的空存档也符合这一顺序：选中新地图并创建区域后，区域 EGB、城市 EGB 和 `misc/MetaData` 才出现。

## CapeTrinity 的实际限制

干净存档 `C:\Users\FluffyCHi_Xing\Documents\SimCity\Games\9a3b6720a2b84853\3` 的区域 EGB 有：

- 66 个 Unit；
- 118 条曲线；
- 两个 `30360939` 道路 Unit 实例；
- 三个城市 UID 和伟大工程坐标。

118 条曲线不是静态 Unit 的 22 条基础段直接复制，而是加入区域连接器、城市边界和交叉口后形成的运行时切分结果。

## 对 OpenSCP 生成空 EGB 的含义

要生成可被游戏接受的空区域状态，至少需要：

1. 正确的 region environment/template；
2. 所有 map-entry 和 image/resource key；
3. 初始 Unit/Zone key 及 `cSPTransform`；
4. 30360939 等静态道路 Unit 的世界实例；
5. 与游戏一致的 EcoPathSet 曲线、segment/path slot、PathEntry、scope/connect 数据；
6. 区域/城市边界、伟大工程和 metadata 的对应关系；
7. 与 EGB writer 相同的版本、容器和状态字段布局。

当前只完成了 EGB 读取、道路资源解析和参考区域加载，尚未完成 118 条运行时曲线及全部状态字段的独立重建。因此现有候选 EGB 不能宣称为游戏可接受的空存档。
