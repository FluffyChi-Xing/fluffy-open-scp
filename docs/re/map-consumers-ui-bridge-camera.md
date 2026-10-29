# 地图消费者、UI 通信桥与摄像机边界

2026-10-12 静态追踪。没有启动游戏、注入 Frida、修改游戏包或 EXE。本轮进程检查未发现 `SimCity`，因此没有实时抓取结果。

## 证据范围

- 零售 UI 转储：`tmp/js/region_res_67771F5C_696447EA.bin`（launcher）、`tmp/js/40464200_2093765772.js`（游戏 UI，instance `7CCC548C`）。两者都定义了 `kRegionalDefaultCityInfo`，但负责不同界面。
- 零售原生反编译：`docs/source-code/_legacy_ghidra_dump/SC_cGameCommandPlugin.c`、`SC_cGameEventQueuePlugin.c`、`GB_cLocalTemplateManager.c`。
- 带符号开发版反编译：`docs/source-code/SimCity-dev-ida.c`。以下开发版函数名、行号和地址只用于定位语义，不能直接作为当前零售 EXE 的补丁地址。反编译中的临时变量类型和虚表偏移可能不准确。
- 已验证的零售静态 bootstrap 读取链见 `map-static-bootstrap.md`。

## 城市数量与名称：之前的判断需要修正

不能把 Game registry 统一称为城市计数或认领列表的权威源。不同页面消费不同数据。

| 页面/行为 | 实际消费者 | 输入 |
|---|---|---|
| 创建区域页的城市数量 | launcher `CreateTab.prototype.ShowTemplateDetails` | `mCurrentTemplate.map.cities.length` |
| 小地图城市板块 | `IsometricMapController.prototype.DrawCityPlates` | `map.cities` 的 x/y/uid；伟工另读 `map.greatWorks` |
| 模板转成可展示区域 | `cMyGamesUIHandler.RemapTemplateToRegion` | 要求 `boxes.length == map.cities.length`；按索引将 city UID 赋给 box |
| 未认领城市名称/介绍 | `GetCityStubDescription`；游戏 UI 的城市详情消费点 | `kRegionalDefaultCityInfo[template].cities[uid]` → `ParseLocaleString` → `SetLocaleText` |
| 已创建区域的槽位与计数 | 游戏 UI `cAPIServerDataHandler.ReceiveRegionData` | 遍历返回的 `a.boxes` 构造 `mBoxes` 并累计 `mTotals.boxes`；已认领项另外累计 cities |
| 当前城市默认说明 | 同一个 `ReceiveRegionData` | 按 `a.template.name` 选择 `mCityDefaultData`，不以该表替代 boxes |

launcher 的 `DoPreload` 对模板配置了两个入口：联网 URL `/region/templates.json`，以及离线 `datarequest:["RegionTemplates"]`。`cLoader` 在 `mOfflineMode == 1` 时调用 `scrui.RequestGameData([scrui.BuildRequestQuery(f.datarequest)], ...)`。因此离线新建页应观察 `RegionTemplates` 返回值，而不是假设网页必然访问远端服务器。

零售 `GB_cLocalTemplateManager.c` 的 `FUN_00c17af0` 枚举 group `95F4AEF0` 并生成 `regionTemplates`；下游 `FUN_00c17620`/`FUN_00c17750` 的完整消费链仍需继续定位。静态读取器已验证该组属性连接 metadata 与 site/bootstrap，但这不能代替游戏运行时实际返回值。

对 target1 的修正：

1. v1 改地块表不能改变 `ShowTemplateDetails` 直接读取的模板城市数量。
2. v2 同步修改 `map.cities`/`boxes` 有明确消费者依据；若数量仍不变，优先核对有效模板资源与 `RegionTemplates` 实际返回值。
3. 两份 JS 都含默认城市说明，但删除 registry 中的 `1026` 不会自动从模板或返回的 boxes 中删除站点；还可能造成槽位存在而默认说明缺失。
4. 名称覆盖是否生效仍需观察实际语言表；不能用城市数量失败解释词条失败。
5. 没有运行时响应和有效 TGI 证据，不能宣称“补齐另一个 registry 就一定修复”。

## 资源在引擎中的消费

应区分 UI 评级、初始化字段和正在模拟的字段：

- registry 的 `water/coal/rawOre/crudeOil` 等是城市介绍数据；UI 显示它们不代表修改了可开采资源。
- EP1 `08068AED` bootstrap 提供已有区域/城市初始 EcoGame 字段，当前读侧已按 site UID/environment 对齐。
- 开发版 `GB::cEcoMap::Create`（690579 行）读取 `0x07E0AE06` 初始图像 key；先用当前 environment 覆盖 group 尝试加载，失败后回退原 key group 或属性所属 group（690647 行）。这说明写出正确 instance 但绑定错 environment/group 仍可能读不到目标字段。
- `InitFromImage`（689900 行）请求资源、检查图像尺寸，将 u16 样本按 `(mMaxValue * sample) >> 16` 转入 `mMapData` 并重算 `mMapSum`。这是该开发版图像初始化分支的证据，不能直接视为零售 bootstrap 的通用量化公式。
- 后续模拟规则更新字段；`LogMapRule` 写入 delta 流。零售新建时 bootstrap 反序列化与 `Create` 初始化的先后、哪些覆盖会被初始状态替换，仍需动态对拍。

下一步最有辨别力的实验是只改一个已知城市、一个已知资源格，比较源资源、bootstrap、认领后字段和保存重载字段，避免只观察资源介绍卡片。

## UI ↔ 原生：两条传输路径

后续已针对用户实际安装的 EXE/包核对协议，见 [retail-ui-command-event-protocol.md](retail-ui-command-event-protocol.md)。有 `window.Client` 时默认使用原生回调与直接事件推送，不是默认轮询 `RequestGameEvents`；该入口只是可选队列路径。

已确认嵌入式 UI 使用 EAWebKit。早期文档把整条桥笼统称为 Coherent UI/内嵌 HTTP，表述不准确。

开发版 `cGameUI::RegisterGameJavaScriptCallbacks`（942882 行）明确注册：

| JS 回调 | 原生消费者 | 含义 |
|---|---|---|
| `RequestGameData` | `cGameDataPlugin::OnCallback` → `BuildGameDataJSValue` | 读取请求对应的数据，返回 JS 值 |
| `PostGameCommand` | `cGameCommandPlugin::OnCallback` → `ExecuteGameCommand` | 命令字符串及可选数据字符串进入原生命令分发 |
| `RequestGameEvents` | `cGameEventQueuePlugin::OnCallback` → `ProcessEventQueue` | 按 token 拉取 UI 事件 |

另外有 URL transport 分支；当前安装包 scrui 的批量查询实际使用 `gamedata/batch/`，命令/事件使用 `gamecommand/`、`gameevents/`。不能把旧资料中的 `gamedata_batch` 名称直接当成请求 URL。`RegisterWebkitGameTransportPlugin` 与 `RegisterHTTPDataSourceFromTransportPlugin` 同时注册（945708 行），后者只有 `SP::HTTPServer()` 非空才注册（1112443 行）。零售 command/event plugin 转储也保留 URL 分派与队列逻辑。

因此：JS 原生回调、WebKit 内部 URL transport、实际监听端口的 HTTP 服务不能混为一谈。只抓网络可能完全看不到原生回调。

开发版 HTTP 线索：

- `CreateHTTPServer`（270041 行）内置 `Spark HTTP Server / 1.01.00`、`Port 8088`、`ErrorLog ErrorLog.txt`、`LogLevel warn`。
- 启动参数 `noServer` 跳过创建（273418 行）。
- `StartHTTPServer`（272444 行）使用 `DataDir/Server/` 和 `DebugDir/ServerLog/`，且明确关闭日志 flags 1、4、8。默认不等于记录所有请求。
- 当前零售版是否保留/开启服务、端口归属、实际 DebugDir 和允许的路由均未运行验证。不能把开发版默认值当成已可用的零售接口。

## 实时指令/日志能抓哪些

| 层级 | 候选观察点 | 能回答的问题 | 边界 |
|---|---|---|---|
| UI 语义 | `scrui.PostGameCommand`、`scrui.RequestGameData` 及现有回调返回值 | 点了什么、请求了哪个数据、拿到哪些模板/槽位 | 需验证 scrui 桥实现和观测补丁加载；不覆盖全部原生输入 |
| UI 事件 | 既有事件分发结果，或独立订阅 token | 引擎给 UI 的状态通知 | 不等于全部模拟事件 |
| 原生分发 | 版本匹配后的 `ExecuteGameCommand`、数据查询分派 | UI 命令是否进入原生，查询由谁处理 | 需先完成零售地址/签名映射 |
| 模拟变化 | `cEcoGame::LogAction`、`LogMapRule`、`LogPathCreate` 与 delta | 规则和路径状态变更 | 写入二进制 `mDeltaStream`，不是现成文本操作日志；需要协议解码 |
| 文件/HTTP | 已有日志、确认启用的服务 | 加载错误和特定请求 | 文件不存在/没输出不能证明没有执行 |

事件队列不是无副作用的历史查询：`ProcessEventQueue`（1113586 行）复制 token 对应事件后删除队列节点（1113640 行）。不可复用游戏 UI token 另开轮询，否则可能抢走 UI 事件。优先观察 UI 原有读取结果；独立 token 订阅须核对创建/清理和广播行为。

Frida 历史失败已经有具体记录：大量候选 hook、把内部 COM/垃圾对象当标准 D3D9 对象并调用 `GetFunction`、运行后尝试创建 D3D 设备，都产生过崩溃/死锁。见 `tools/dynamic/README.md`。记录中的“只读安全”“≤10 钩子”等是历史经验，不能作为不会崩溃的保证。

本任务优先观察语义桥，无需进入 D3D9。若未来需要原生探针，应先确认当前 EXE 指纹和调用约定，只选一个低频分发入口，限制读取长度/捕获时间，批量回传；不主动调用猜测的 NativeFunction。当前尚未制作或安装运行时探针。

## 摄像机限制：已定位多层，不能只改缩放

开发版 `cCameraControllerBase` 的证据：

1. `ConstrainTarget`（959721 行；开发版 VA `00791830`）取当前 Game 的边界，将 `mLerpSubject.mTarget` 的 x/y/z 夹在最小值与最大值之间。反编译此处仍显示虚调用，需要在零售版确认具体接口。
2. `Update` 的 `mRelativeMove` 非零分支（962754 行）先夹到 Game 边界，又对 x/y 检查 ±1024，越界后设为 ±1020（962840 行）。随后通过虚调用执行 `ConstrainTarget`（962875 行）。这是特定移动分支，不能把 ±1020 说成所有控制器统一限制。
3. `CameraEyeConstraints`（961064 行）按 `PlayArea` 内外选择地面检测，界外调用 `PointSampleHeightInRegion`。它承担防穿地等高度处理，不能整段禁用。
4. `ConfigUpdated`（964311 行）读 zoom/FOV/near/far/pitch/edge-scroll 等参数。`EdgeScrollMargin` 是屏幕边缘触发区，不是世界城市边界；扩大 zoom/far clip 也不解除 XY 限制。
5. `UpdateCameraSettings`（942939 行）从 NonSimProperties 的 `0x08E48E67` 选择相机 key，默认 instance `F3487AFE`。存在 WASD、Region 等其他控制器，但还未证明零售版可只靠切控制器得到保留正常游戏交互的自由相机。

`GB::cEcoGame::StartGame` 读 SimProperties `0x07E0AC05` 到 `mPlayArea`，并用范围与 `mMapBounds` 计算 `mPlayCell`（646370 行）。所以为了镜头自由而直接放大 PlayArea 会触及模拟坐标，不是纯镜头修改。

建议的相机专项路线：

1. 在当前零售 EXE 定位上述限制和活跃控制器，记录城市局部坐标与区域世界坐标变换。
2. 先验证现有控制器/配置能否提供区域漫游；检查进入城市、切换区域、跟随对象时是否重置。
3. 若必须原生修改，只改变相机使用的 XY 约束来源：改用换算到当前坐标系的有效区域边界，同时处理 relative-move 的硬编码限制；保留高度检测、插值和其他镜头行为。
4. 检查拖拽、键盘、屏幕边缘滚动、缩放、跟随、城市切换和重载，避免仅一种输入解锁。
5. 区域外地形采样、裁剪、远裁面与 LOD 单独验证。镜头跨城不会自动加载/模拟其他城市。

目前可以指出限制机制与候选修改面；没有当前零售补丁地址、可直接安装的相机解锁包或运行成功证据。

### 2026-10-12：从实机摄像机事件反查零售代码

第四轮原生桥采样（`tmp/ui-native-v4`）捕获 `kMessageGameCameraMovingStart=205419630` 和 `kMessageGameCameraMovingStop=205419631` 各 26 次，来自已验证的引擎→页面派发调用点。通知本身只携带数字 ID，无坐标或边界参数。

在当前 EXE 静态扫描这两个数值，找到明确的 `push imm32` 发送指令：start 为 RVA `0x25C09A`，stop 为 `0x25BF9F`。均调用消息服务虚表 +0x14；对象字段 +0x581 参与移动状态转换判断。所属候选更新函数入口为 RVA `0x25B6E0`，片段反汇编保存为 `tmp/ui-native-v4/camera-update-disasm.txt`。这些是事件发送/状态判断的位置，**不是已确认的相机边界钳制位置**，不应通过禁用通知来尝试解除限制。

这给出了在零售代码中继续追踪活动控制器及其基类约束的起点。尚未在运行时观测该控制器对象和坐标，也未修改任何相机字段或指令。

### 零售版约束候选（静态核对）

当前零售 EXE 中 RVA `0x259D80` 是一个明确的三轴边界夹取函数：先从输入对象的虚表 `+0x34` 取得六个边界值，随后分别对 x/y/z 做 min/max，结果写回同一坐标结构。函数末尾返回前，x 使用 `[bounds.minX, bounds.maxX]`，y 使用 `[minY, maxY]`，z 使用 `[minZ, maxZ]`。这是比开发版名称迁移更可靠的零售候选，但所属控制器对象和边界来源仍需运行时确认。

RVA `0x25B6E0` 是当前相机 glide/update 候选；它在不同状态分支更新目标值，并通过 `0x259D80` 同类的边界处理链。开发版反编译还显示 relative move 分支会先按 Game 边界夹取，再进行高度检测；零售版不能直接假设保留开发版的 ±1020 常量。解除区域边框的最小安全修改面应是确认 `0x259D80` 输入边界的来源后替换相机专用 bounds，保留高度检测、插值和裁剪；不能扩大全局 PlayArea，也不能只禁用移动开始/停止事件。

目前没有写入补丁或运行时修改；已有样本中用户操作期间相机移动事件正常产生，观察器注入几乎无感且无明显卡顿。

### 2026-10-30 新样本：`regionedit` 只改变相机模式，不解除边界

新样本的 source-tree/PDB 把“区域编辑相机是否天然自由”回答得更具体：

1. `kRegionEditCameraID` 的初始化函数（RVA `0x79D030`）只是对字符串
   `regionedit` 做 FNV1 hash。`cTerrainSystem::SetRegionAuthoringActive`（RVA
   `0x405140`）激活时调用 CameraManager 的切换接口，并关闭 `SCMain` renderer 的
   `dof` 与 `motionBlur`。函数没有清空 bounds、替换 controller 或跳过约束。
2. `cToolRegion::FillFromProps`（RVA `0x47C580`）从 Property `0x08E48E67` 读取
   `mRegionCameraID`，缺省值是 `0x2D89FF4A`；`Activate`（RVA `0x47AF30`）保存
   原相机、切到这个 ID，并在退出时恢复。也就是说这是一个可配置的 controller 选择，
   不是“解除限制”开关。
3. 基类 `cCameraControllerBase::Update`（RVA `0x395B90`）在每帧更新末尾调用
   虚函数 `ConstrainTarget`。基类实现（RVA `0x391830`）从 `Game()->PlayArea()`
   取得六个边界值，对 `mLerpSubject.mTarget` 的 x/y/z 做 min/max 夹取；切换到
   `regionedit` 并不会绕过这条调用。
4. `Update` 的 `mRelativeMove` 分支在调用 `ConstrainTarget` 前已经做了一次 Game
   bounds 夹取，随后对 x/y 再执行 `[-1024,1024]` 检查，越界值改为 `±1020`。因此只
   放宽 `ConstrainTarget` 仍可能在键盘、拖拽或边缘滚动时卡在第二道限制。
5. `CameraEyeConstraints`（RVA `0x393530`）使用 `PlayArea` 判断视线采样点是否在
   城市盒内；界外改用 `PointSampleHeightInRegion`，并据此调整相机高度/俯仰。这是防
   穿地逻辑，不能为了越界而整段禁用。

这带来一个可操作的解除方案分层：

| 层 | 解除动作 | 必须保留 |
| --- | --- | --- |
| 活动相机 | 只验证/选择 `regionedit` 或自定义 controller | CameraDirector 的切换、恢复和过渡 |
| XY 目标约束 | 将 `ConstrainTarget` 的 bounds 源换成相机专用有效区域 | z 约束、插值、跟随和事件状态 |
| 相对移动 | 将 `mRelativeMove` 的 ±1024/±1020 改为同一 bounds 契约 | 高度检测和 `CheckForGroundFromPoint` |
| 视线/地面 | 允许 `CameraEyeConstraints` 继续采样区域地形 | `PointSampleHeightInRegion`、pitch 修正 |
| 视觉参数 | 通过 camera Property 调整 zoom/FOV/near/far/pitch 数组 | 不把 zoom/far clip 当作 XY 解锁 |

最小验证应在同一城市内记录每帧 `targetBeforeClamp`、`bounds`、`targetAfterClamp`、
`relativeMove` 和地面采样结果。四种输入（拖拽、键盘、边缘滚动、跟随/过渡）都必须
通过相同 bounds；否则只能称为“部分解锁”。

## 接下来优先抓取的最小证据

1. 模板选择：`RegionTemplates` 响应里的模板 key、name、`map.cities[].uid`、boxes 数量和对应关系。
2. 词条：当前页面使用的 JS 资源、locale 表/string ID、实际显示值。
3. 区域创建/认领：实际命令和返回的 region/box UID；已有区域 `ReceiveRegionData` 的 boxes。
4. 资源：一个城市的 environment、一个资源字段初始化前后及重载后的值。
5. 镜头：活跃 controller、目标坐标、约束前后值；全局 PlayArea 保持不变的相机专用实验。

先形成可重复的加载/消费报告，再制作地图覆盖或镜头补丁，避免继续把“资源存在”当成“此页面/此运行阶段消费了它”。

### 创建区域运行时证据（2026-10-12）

稳定创建区域页面采样已确认：`CreateRegion` 返回 `map.cities` 五项坐标与 UID，`boxes` 数量与模板响应一致；每个 box 还携带 `regionTemplateName` 和初始化资源。该链路可用于验证单城大地图解锁包是否同时覆盖模板、槽位和初始化资源。名称乱码只出现在观察器对部分 flat string 的复制结果，尚不能归因于游戏词条或资源本身。
