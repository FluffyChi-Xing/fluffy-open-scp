# Dev Beta (RL) vs 零售 (PL) vs dump：三方二进制与代码对比（2026-10-04）

> 三篇分析之二。样本真伪判定见 `dev-beta-sample-inventory.md`；
> 对 OpenSCP 未决问题的价值见 `dev-beta-openscp-leads.md`。
> 对比对象：
> ① dev beta `SimCity.exe`（RL 分支，CL 707103，2013-01-14）
> ② 零售 `SimCity.exe`（Cities of Tomorrow 版，PL 分支，CL 839109，2014-04-22）
> ③ Ghidra 脱壳 `SimCity_dump_SCY.exe`（= ②的内存转储+导入表重建）
> ④ `docs/source-code/`（从 ③ 选择性导出的 347 个类反编译）

## 一、PE 层面对比

| 项 | dev beta (RL) | 零售 (PL) | dump_SCY |
|---|---|---|---|
| 文件大小 | 10,109,616 | 10,971,552 | 10,979,328 |
| PE 时间戳 | 2013-01-15 05:25:17 UTC | 2014-04-22 01:19:07 UTC | 同零售 |
| 入口点 | 0xa93830（壳）/ 0x62ae02（脱壳 OEP） | 0x8e7fe2 | 同零售 |
| .text vsize | 0x8269c7（8.15MiB） | 0x8e6f87（8.90MiB） | 0x8e7000 |
| .data vsize | 0x16141c（1.38MiB） | 0x269adc（2.41MiB） | 同零售 |
| 段数 | 6（含壳 stub 段） | 5 | 6（含 `.SCY`） |
| RSDS PDB 路径 | `c:\BF\CM\SimCity_RL\...` | `c:\BF\CM\SimCity_PL\...` | 同零售 |
| RSDS GUID | `e291cc5d-…` | `cb3004aa-…` | `cb3004aa-…` |

**判读**：

1. **RL vs PL 是两条分支**，不是同一分支的两个构建——PDB 路径分支名
   不同、GUID 不同、版本字符串 `RL 707103` vs `PL 839109`。
   15 个月间 .text 增长 **+9.2%**，.data 增长 **+74.9%**
   （大量新增全局状态：新系统、新调参表）。
2. **dump_SCY = 零售 exe 的 Scylla 转储**：时间戳、入口、RSDS 全同，
   仅多 `.SCY` 段（0x3000，导入表重建产物）且段尺寸对齐取整。
   与"Ghidra 脱壳得到"的履历一致，作为 PL 分析底本可靠。
3. dev 脱壳对（原/unwrap）除入口与 stub 段外逐字段一致，见真伪篇证据 3。

## 二、代码规模与符号面

| 项 | dev (RL) 反编译 | docs/source-code (PL) |
|---|---|---|
| 形式 | 单文件 71.8MB / 1.90M 行 | 347 个文件（按类切分） |
| 函数体 | **39,321 个**（含重载/模板实例） | 4,600 个 `FUN_xxxxxxxx` 体 |
| 限定名 | **17,346 个**（真实符号） | 0（仅 vftable 名 280 处引用） |
| 类 | **1,462 个** | 347 个（选择性导出） |

**方法论警示**：`docs/source-code` 是从 PL dump **按需导出**的子集
（约占全量函数的一小部分），因此下文的类级 diff 中
"仅 dev 有"方向被选择性导出严重放大，只有
"**仅零售有**"方向（123 个）是高置信度的**真新增**。

## 三、类级 diff（归一化后）

归一化：文件名嵌套分隔 `_` 与 C++ `::` 歧义通过去分隔符比对消除；
模板参数剔除。产物：
`tmp/dev-sample-analysis/out/classes_{only_in_dev,only_in_retail,common}_norm.txt`。

- 共有类：**224**
- 仅零售 (PL)：**123** —— 发售后才有的系统，高置信
- 仅 dev (RL)：1,238 —— 受选择性导出放大，不能直接当"RL 独有"读

### 3.1 PL 新增类分组（123 个，按子系统）

**离线模式与本地存档（Update 10，2014-03）**：
`SC::cOfflineModeCheck`、`SC::cNetSwitchOfflineFlow`、
`SC::cNetOriginSeriousOfflineShutdownFlow`、`SC::cNetFreeTrialShutdownAfterEndFlow`、
`SC::cNetUploadDeltasOnQuitFlow`、`SC::cLocalSaveManager(+_cLocalGame)`、
`SC::cLocalTemplateManager`、`GB::cLocalSaveManager`、`GB::cLocalTemplateManager`、
`SC::tMessageLocalSaveManagerGameIdChanged`。

**Delta 同步基础设施重构**：
`GB::cDeltaChunk/cDeltaStream/cDeltaUploadTask/cBackgroundStreamsTask`、
`GB::cServerDeltaChunk/cServerDeltaDownloadTask/cServerDeltaWriteTask`、
`GB::cStateNetTransaction`、`GB::cStreamXOR`、
`GB::cUpdateManager_c*JobData/ThreadResult`（7 个任务数据类）、
`GB::cTelemetryBatchTransaction/cTelemetryBlobTransaction`。

**Cities of Tomorrow 资料片（2013-11）**：
`SC::cSpaceRobot`（巨型工程）、`SC::cToolAddTowerLevel`（精英塔加层）、
`SC::cToolSkybridge`（空中连廊）、`SC::cAvatarHelper`。

**地形系统重写**：
`SC::cTerrainWater`、`SC::cTerrainRock`、`SC::cTerrainTextureSet`、
`SC::cTerrainMapUint8/Uint16/Vector4`（模板化地形图）+
`SC::UcShaderDataTerrainForestVS/PS`、`…TerrainHeightPS`、
`…TerrainInfoPS`、`…TerrainNormalsPS`、`…TerrainRegionVS` 等。

**Shader-data 体系换代（27 个 `SC::UcShaderData*_cMIDataT` 类）**：
PL 把 RL 的"注册 ID + 上传函数"表（见线索篇 §4）重构成
逐 uniform 的常量类（`UcShaderDataBatchColors/BatchDatas/GrassVS/
SimPalette/WaterChoppy/WaterNormals/WaterHeight/WaterStrataPS/
LotInstanceInfo/SingleModelBatchData/UnitImpostorInstances/
AbandonedBuilding/CustomClipPlane/EffectOverride/FFT*/MapContourInfo…`）。

**相机与表现层**：
`SC::cGameCameraBaseModule/cGameCameraCinematicModule/cGameCameraSkeleton`、
`SC::cIGameCameraController/cIGameCameraFollow/cIGameCameraCinematic`、
`SC::cWASDCameraController_cKeyBlocker`、`SC::cPathDistanceHighlight`、
`SC::cRadialVisualTuning`、`SC::cTradeVisualTuning`、`SC::cPathCongestionTuning`、
`SC::cSunSkyLightingWorld`、`SC::cSimpleAnims`。

**其他系统**：
`SC::cNetShardSelectionFlow/cNetShardStates/cNetStates`（服务器分片）、
`SC::cNetDLCMigrate*`（DLC 迁移）、`SC::cPlumpManager`、`SC::cSCIME`（输入法）、
`SC::cSendJsonToUIMessage`、`SC::cPerformanceTelemetry`、
`SC::cPhysicsGame_c*`/`SC::cPhysicsWorld_c*`（物理回调细分）、
`SC::cTransportPipe_cPool*`（运输管线对象池）、
`GB::cEcoSwarmMap/cEcoSwarmLerpMap`（EcoGame 地图抽象）、
`GB::cIStateFlow`、`GB::cNetUserManager/cNetServerConfigManager/cNetTaskBase`。

### 3.2 对"是否早期开发版"的判定补强

123 个 PL 新增类的内容——离线模式（发售 1 年后才补）、CoT 资料片
（晚 10 个月）、地形重写、shader-data 换代——与公开更新史逐项吻合，
进一步确认 dev 样本 = **发售前功能尚未冻结的 RL 候选构建**，
而非零售换皮。同时注意：**RL 并非"功能更多的完整版"**——
它没有离线模式、没有 CoT、地形是旧管线；它的价值不在功能，
而在**符号**与**较早、较简单的实现形态**。

## 四、与 docs/source-code 现有结论的关系

1. **底本确认**：`docs/source-code` 的函数地址（如 `FUN_00bf27b0`、
   `DAT_00df513c`，地址域 0xc00000+）与 PL dump 的 0xc70000 镜像尺寸吻合，
   确认现有 347 类反编译自 **PL 分支（CL 839109）**——此前文档中未显式
   标注分支，现补齐。
2. **互证关系**：RL 反编译带符号，可对 PL 的 `FUN_` 逐函数按
   "同名函数在两分支的反编译形态"锚定——`SC::cVolumeDecalManager` 等
   核心类在两分支同名同构（详见线索篇），PL 侧既有分析结论因此获得
   第二独立来源背书；反过来 PL 的新增类（§3.1）在 RL 无对应，
   不可去 RL 里找。
3. **命名嫁接**：RL PDB 符号 → PL 的 FUN_/DAT_ 重命名需按
   "反编译形态指纹 + 字符串引用"逐函数人工/半自动锚定，
   不能按地址偏移平移（两版本 .text 布局完全不同）。
   `SimCity.pdb` 本身只对 RL exe 有效，**不能直接喂给 PL dump 用**。

## 五、诚实清单（本对比的局限）

- "仅 dev"清单受 docs/source-code 选择性导出污染，未逐类核实；
  如需"RL 有而 PL 删了"的准确清单，应对 PL dump 做一次全量导出再 diff。
- 零售 `SC_cSimCityApp.c`（246 行）为局部导出，不含 `--version` 分支，
  故 PL 版本输出代码未直接核对（字符串 `PL 839109` 已 grep 证实）。
- 类级 diff 不含"类内函数增删"粒度；共同类 224 个的函数数对比
  （`classes_common_norm.txt`）仅作规模参考。
