# 下一阶段逆向分析路线图

本路线图以当前源码树、可解析 package/存档样本和已有 UI bridge 证据为基础，目标是把“能定位字段”推进到“能解释生命周期、能验证修改结果”。每个阶段都要求保留源文件路径、函数/属性 ID、样本哈希和未知项，避免把静态推断当成运行时事实。

## P0：闭合城市坐标与边界生命周期

### 目标

- 找到 `GB::cIEcoGame::PlayArea()` 的初始化来源、默认尺寸和存档覆盖点。
- 证明区域模式、城市模式和城市切换时是否存在隐式旋转/平移。
- 找到“边框解锁”实际改变的字段集合，而不是只修改可见 mesh。

### 入口与证据

- `cTerrainGfx::Init`、`cTerrainRegion::Init/FillFromProps`、`cTerrainExporterCity::Init`。
- `cGameAppMode::LoadBoxesFromRegionInfo`、EcoGame box load/post-load 回调。
- `cGraphicsCityBoxBoundary::StartGame`、`cCameraControllerBase`、`cZoningGame::StartGame`。
- `GB::cEcoGame` 中 PlayArea、world bounds、city box 初始化函数及 `0x7E0AC05` 的全部引用。

### 验收

使用一个原始城市和一个扩大边界样本，逐项比较：PlayArea min/max、城市槽位位置、道路端点、lot corners、地形高度采样点、摄像机约束。只有在同一锚点差值保持为零时，才能确认没有重复平移。

## P0：闭合道路跨区域连接

### 目标

- 解释 region path 与 region connection road 的脚本 bitset、path entry index 和 city path 的对应关系。
- 确认跨城市道路两端在 region box 与 city box 中是否共享坐标、如何处理 z 高度和加载顺序。

### 入口与证据

- `cGraphicsPathNetwork::Init/IsRegionPath/ConnectedToRegionPaths`。
- `cGraphicsPathPart::SetPathFlags/PathInPlayArea`、`SC::IsSegmentInPlayArea`。
- `cRoadManager::PathCreate/UpdatePaths/SetPathEntry`。
- `cUnitPathCreationObj`、region connection 相关脚本属性和已有道路样本。

### 验收

从同一条跨界道路导出 region 端和 city 端的端点、path entry、flags、PlayArea 判定和加载时间线，确认端点误差、方向和高度误差的容忍范围。

## P1：建立模拟规则可观测性

### 目标

- 从 `cEcoBinSet`、rule handler、resource/bin 更新到 UI 状态读回，建立“动作→规则→资源变化”的最小闭环。
- 区分状态验证失败、规则失败、资源不足和 UI 未刷新。

### 入口与证据

- `GB::cEcoBinSet::RunRule/CanRunRule/ApplyRule/OnRuleFail/OnRuleSuccess`。
- `GB::cEcoGame::StateValidationFailed/LogGlobalRule`。
- `EA::Trace::TraceHelper/Server/Reporter` 及 `SP::cCheatManager::AppConsole`。
- 已有 `RequestGameData`、`PostGameCommand`、`ReceiveGameEventFromClient` 样本。

### 验收

选择一个可重复动作（修路、消费、保存），记录时间戳、城市/box/UID、资源值、事件 ID、请求返回和磁盘变化；若没有 native trace，只能把规则内部步骤标为未知。

## P1：确定 package 覆盖与完整性边界

### 目标

- 把 package overlay 的资源优先级、Preload 特例、EcoGame 版本/Property group 要求与实际启动结果关联。
- 确定哪些资源允许覆盖、哪些资源必须成组更新，哪些修改会导致模板/城市状态不一致。

### 入口与证据

- `RegisterPackages → AddPackageIfValid → ResourceMan::RegisterDatabase`。
- `GB::InitGameVersions/AddGamePackage`。
- package manifest、DBPF entry、Property parent/inheritance、地图模板 JSON 和 locale。

### 验收

为每个改动生成依赖闭包和前后哈希；在离线 parser、流程引擎和实机加载三层分别验证。任何缺失 parent、group/version tag、locale 或模板索引的 package 都必须被标记为不可交付。

## P1：完善 UI bridge 事件语义

### 目标

- 把已观察的命令、查询、返回、事件和 console 消息映射成稳定的语义 schema。
- 识别绑定对象复用、页面切换、限流丢弃和请求/返回配对失败。

### 入口与证据

- `native_agent.js` 的 `invoke`、`onLeave`、`ReceiveGameEventFromClient`、`addMessageToConsole`。
- `retail-ui-command-event-protocol.md` 中的事件 ID、请求名和已有时间线。

### 验收

每条记录都带 session、page、binding、thread、sequence、source 和 dropped 计数；对保存、加载、创建区域、改名、消费五类动作分别做专项采样。结果必须明确“观察到”“未采到”“被限流丢弃”三种状态。

## P2：建立 native 日志观测实验

### 目标

- 先外部采集 `OutputDebugStringA`，再评估指定 `EA::Trace::Server::Trace`/reporter 的只读 hook。
- 验证 `AppConsole` 是否在当前零售构建中包含可用的 engine trace。

### 限制

不得 hook 共享 `ret 4` 的 DebugLog 空函数；不得把页面 console 当成模拟日志；不得在未确认 ABI、模块哈希和入口字节前注入 reporter。

### 验收

同一动作同时保留 UI bridge、DebugView/native trace、存档文件快照三份证据，并说明每条证据的覆盖范围和丢失可能。

## P2：运行时验证与版本矩阵

为每个关键结论维护构建指纹：`SimCity.exe`/`EAWebKit.dll` SHA-256、RVA、入口字节、样本版本、操作系统和 mod 集合。至少覆盖原始零售构建、当前离线目录和一个已安装 UI/package overlay 的对照，避免把单一构建的地址或属性解释外推到其他版本。

## 暂不优先的方向

- 继续扩大无语义的全量 UI 参数抓取；当前限流和绑定生命周期已经证明它不是无损模拟流。
- 直接修改渲染边界 mesh 来实现可放置区域解锁；源码证据显示道路、分区和 lot 受 PlayArea/世界坐标约束。
- 在没有运行时回读的情况下批量改写城市槽位、道路端点或 unit transform。
