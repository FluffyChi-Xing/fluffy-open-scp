# 半海湾 8192 米单城市原型

`OpenSCPHalfCoast` 是独立的地区模板。西侧 4096 米为水，东侧 4096 米为陆地。海岸在世界 X=0，高速在 X=2048 由南至北连接两个区域边缘。区域可见面积为 8192×8192 米。唯一城槽在 (0, 0)，城市 F0 为 1024²（8 米/像素），城市 EcoMap 为 512²（16 米/格）。煤、原矿、原油和地下水四层的每格均为 65534，静态城市卡四项均为 3 档。高速在区域和城市初始状态中各为同一条由 32 个相连的 256 米分段组成的路。区域 F0 保留原生 32768 米 backing field，并在可见区外延伸边缘像素。

## 构建及 IDE 接口

`sc_properties::region_authoring` 可直接作为库使用。IDE 提供 `CoastalRegionSpec` 和明确的 `RetailSources`，调用 `build_coastal_region` 取到 DBPF 字节，写盘后调用 `verify_coastal_region` 获取可序列化的报告。预览工具也可单独调用纯函数 `generate_coastal_region` 获取高度、地面、水深和高速中心线。调用方负责文件路径和发布，库本身只读传入的包。

```powershell
cargo run -p sc-properties --example coastal_region --offline -- "<SimCityData>" "<SimCityData>/Locale/en-us/Data.package" en-US "tmp/OpenSCPHalfCoast.package"
```

语言有 en-US、zh-CN 和 zh-TW。调用时选与本机有效语言表相同的源包。当前安装用 zh-tw 的 locale 资源作为两个中文实验包的结构来源。每个语言版本带相同的资源 ID，只选择与测试对应的一份。

## 选区与认领路径

1. 离线启动器请求 `RegionTemplates`。本地模板管理器枚举 `95F4AEF0`，新增的属性表以 `F240A8CA` 指向新 metadata，以 `46066280/81` 将 site 0 与 1026 绑定到独立的 v17 bootstrap。按零售 Cape Trinity 样本保留脚本与 skill 版本、全域 zone 及尾部状态，城市全域 zone 扩为 512 行。
2. 启动器以 metadata 的 `key` 登记模板，从 `map.cities` 显示城市数量并根据位置绘制板块。认领前 `RemapTemplateToRegion` 要求 `boxes` 与 `map.cities` 等长，按索引附上 UID。`CreateRegion` 提交对应的 `regionType` 与 `boxUid`。启动器和游戏的 registry 均有同名模板、同一城市 UID 及本地化词条。按游戏路径哈希写入等距底图、选区缩略图和加载图。
3. 入城时，地形加载器从城市环境的 `C1949C4D` 找区域描述，由描述的 `4EE97F5B` 取数字区域名，按 `FNV1("00000000_citylist_" + regionName) | 0x80000000` 定位 CityList（开发版 `cTerrainExporter::MakeCityListGroupID`，用零售三一岬 `F0E2E39C` 对拍）。半海湾 CityList group 为 `A53A3CF6`，其唯一行指向新环境，环境的 `1B53D525` 为 0。
4. 新模板的路由表为空。游戏 registry 的贸易回调沿用嘉納谷 r3 的已定位分支，为该模板的唯一城市初始化空连接表。其他模板按原来的判断执行。

## 已验证和待验证

- 真实零售源包构建后，读取器逐格核对城市 F0、区域中心裁切、地面和水深。读取两份 bootstrap 的道路和资源图，核对 CityList 原生查找键、两个 bootstrap 环境和唯一槽位的关联、图块条目数和两份脚本的 UID。
- Node 回放从候选包抽取真实启动器脚本、metadata 与词条。在该脚本的 `SetRegionTemplates → ShowTemplateDetails → ShowClaimCity` 上验证显示 1 城、0 伟工，认领回调传出 1026，切换中英文词条后标题更新。底图与缩略图也从包内回读。
- 游戏运行时仍要让新城市环境的实例使用 512²、±4096 米及 1024² 地形，再验证碰撞、道路控制图与 parcel 查询。原版城市初始化的 128² 与本包的 512² 会在 EGB 格数校验处冲突。区域 site 0 继续使用原版的 128² 契约。候选包用于项目内验证，游戏内选区与首次认领均待录证。
- 当前安装已有 `000_LauncherTweak.package`，占用同一启动器脚本 `67771F5C:40464200:696447EA`。首次游戏内试验需核对谁提供生效脚本。保存隔离、查哈希、包台账和冷启动沿用项目规则，由用户启动游戏。

相关证据见 [UI 消费链](map-consumers-ui-bridge-camera.md)、[首次写盘](region-bootstrap-and-egb-generation.md) 和 [4096 米地表及地块复盘](citybox-load-and-spatial-contract-20261115.md)。


## 实机首轮和 r2 修订

首轮主菜单、区域选择与认领页可用，UID 1026 已写为 claimed。区域、城市初始 EGB 分别与部署包内对应 bootstrap 的 SHA-256 相同。区域有 16384 格，城市有 262144 格。原版 EXE 读城仍按 16384 格初始化，实机报无法载入。四条区域/城市词条均显示星号，首次认领把该占位符写入了测试档的城市名。

r2 改用新 locale 表 `0A98EAF0:02FABF01:477A301C`（`OpenSCPHalfCoast.json`），按已实机验证的新表编码使用 BOM、CRLF 和缩进。轮播、已有游戏卡与加载图使用同一份基于原始高度场的透视景观渲染，右侧选区地图继续使用等距图。两份 bootstrap 与 r1 和旧测试档逐字节一致。旧测试档只把 UID 1026 的城市名修为「東岸」。

`probes/runtime/half_coast_stage.py` 使用经 SHA-256 校验的零售 EXE 和嘉納谷已定位的入口，生成独立启动文件。仅环境 `E11F2425` 启用 512² 模拟网格、1024² 物理高度图、±4096 米水平范围，并同步碰撞、区域贴片、道路控制图投影及阻挡网格。48 项独立 x86 用例通过。地表着色器和 parcel 查询还需实机验证。r2 已部署，需手动启动游戏目录下 `SimCity/SimCity-HalfCoast-8192.exe` 并重载 `new_map_test`。

## r3 CityList 读取崩溃

21:43:34 的 Windows Application Error 1000 记录 `SimCity-HalfCoast-8192.exe` 在 RVA `007FD451` 访问异常 `C0000005`，同一 PID 稍后以 `C000041D` 退出。RVA 对应 `cTerrainExporterCity::Init` 中的 `mov ecx,[eax+4]`，它读取 CityList 的城市名数组，发生在渲染与地块查询之前。

零售 `cTerrainRegion::Init` 的 `BF531B` 调用 `BEB5D0`，以 `%08x_citylist_%s` 构造版本 0 的组名，然后通过 `435960` 归一化资源组。该函数在 FNV1 之后明确执行 `or eax,0x80000000`。三一岬未参与暴露此差异，因为它的原始哈希 `F0E2E39C` 已置位。半海湾的原始哈希 `253A3CF6` 未置位，r2 资源在此组，实际读键为 `A53A3CF6`。r3 只改 CityList 资源键，保留表内字节、bootstrap 和启动文件，构建器以零售算法生成及核对组号。

## r4 入城实测和外圈缺陷

用户新建半海湾并进入城市，生成 slot 5 城市 tick 607 EGB（SHA-256 `67d5c5f0a5b7419e974dbc8a39be70f4096f9150627dc24b4e4d9eca37e640ec`）。安装包 SHA-256 `f2edeb0adfc7434188b5c5457215616f039f307bcfb3d6a68e8a7fd5c2cf1797`。EGB 解码为 v20，环境 E11F2425，36 图层、87 路径样条。工作区回读在 `tmp/half-coast/r4/validation`。

- 煤、矿、油、地下水每图 262144 格均为 65534，每图有 245760 个满值格在旧 2048 米边界外。现装 App 的 `0469A3F7:40212002:00000002` 同时包含 `worldPos.xy / 2048.0` 控制图坐标及 `abs(worldPos.x) < 1024 && abs(worldPos.y) < 1024` 资料图着色限制。两者都需要从实际活动城市范围取值。
- 地面 EcoMap 和水深 EcoMap 与新城 bootstrap 逐格一致，西半 131072 格保留水深。近景裸露海底发生在绘制或其输入链。历史原版实测控制纹理 512²，半海湾物理图设置 1024²，零售 `UpdatePhysicsMap` 整除会令 512/1024 得到零步长。须先采集半海湾运行时控制纹理尺寸及近景水面绘制状态，区分这一风险与水面裁剪/深度覆盖。
- 煤电厂 94C243EB 保存于 (2139.64,-4021.85)，244F51DB 保存于 (3178.19,-4034.30)。用中心代替 parcel 最小角回放 256 米区块计算，分别落在 (24,0) 和 (28,0)，旧插入上限使二者都夹到 (6,0)。RCI 的 PointQuery 与道路/直放重叠检查的 PolyQuery 均受旧 7×7 地块索引限制。保存的单位位置无法证明正式 parcel 是否存在。需要同步调整容量、插入、点查和多边形查询，保持 256 米区块及 32 米子格的原有粒度。

### r4 发展存档与整地对照候选

城市 tick 3210（SHA-256 `924794c852e8933ee5a81301f94d054ee3346b0b4da6a019758ca819bc87d9dc`）相对 tick 607，路径样条 87→133，单位记录 24→90。逐格相减，旧框外有 372 个煤格、377 个矿格、730 个地下水格、1072 个空气污染格和 72 个地面污染格改变；原油未变。这是格网状态变化证据，单凭两份快照无法确定每笔变化的模拟规则。

`probes/runtime/half_coast_flattening_stage.py` 在 `0x661251` 仅为 E11F2425 选择新的 slot 730 默认向量，保持已经提供显式向量的调用。半海湾的水平逆宽为 1/8192，其他城市及空指针保留原向量。x86 fixture 验证半海湾、区域、嘉納谷、空 wrapper 和空 terrain 五项，并检查 EDX、标志及向量对齐。候选 EXE 在 `tmp/half-coast/r4/validation`，SHA-256 `3ecf0ffd11ebd4e132d287bb0b330727bf2d9d1fb81af0667b3eb28b5dfc8670`。未部署；尚需验证近景水面与地坪在活动管线的表现。

`probes/runtime/half_coast_control_raster.py` 只读快照从零售 TextureManager 查找已加载 controlMapRT，前后核对环境及 raster 指针。离线 fixture 覆盖 512 和 1024 宽、拒绝其他活动城市。当前没有半海湾进程可测，水面故障原因和资料图动态范围仍未闭合。

### 修正验收目标

用户明确确认道路在原 2048 米框外一直贴地。此处道路仅作分区和占用对照，默认整地 EXE 不再是这轮验收对象。实际缺陷为建筑落地后缺少正式地坪、重叠放置未拦截、RCI 分区，以及资料图按旧边界裁切。

`probes/region/half_coast_contour_candidate.rs` 生成仅改资料图的工作区对照包。零售 App 的两个组共八个变体，每个只改 bicubic 资料图采样为世界 XY / 8192 + 0.5，并将资料图遮罩扩至 ±4096。八个资源原始长度和其余字段保持一致，重新打开 DBPF 逐项对拍通过。候选使用共享材质资源，未部署。正常安装应为活动 CityBox 提供尺寸，不应全局写死 8192。

开发版 `cZoningGrid` 构造函数直接在对象内构造 49 个 36 字节 chunk，`mUnplacedParcels` 紧跟数组。仅改上限会越过数组污染对象。容量扩展需同时覆盖构造/析构生命周期及 `AddOrUpdateParcel`、`RemoveParcel`、`ParcelQuery`、`PointQuery`、`PolyQuery` 对布局和索引的使用。

### 按城市启用的资料图修复

`cGraphicsDataView::CombineEcomapsForActiveLayer` 可能将两张 EcoMap 混入共享 `combinedContourMapRT`。所以不能用贴图分辨率猜城市大小。零售 `0xBF0A86` 写入 slot 658 的实际纹理宽度，后续三项是高度及两项倒数。新启动文件只在活动环境为 E11F2425 时给宽度分量置符号位，其余三项照常写入。着色器先用符号选择坐标范围，再以真实纹理尺寸的绝对值做 bicubic。半海灣用 `worldPos.xy/8192+0.5` 与 ±4096，普通城市沿用原 UV 与 ±1024。仅资料图路径改动。

`tmp/half-coast/data-view` 有配对的 `SimCity-HalfCoast-8192-dataview.exe`（`2e55c0bd0847b56a65e343ea2e689048d68d6165b35ef104a596e43fee3fb162`）和 `000_OpenSCP_HalfCoast_DataView.package`（`9498680d291ac89e6742109aefb508afa809bb33da7f5ef4fc1a10fa961b19bc`）。五项独立 x86 用例通过，确认半海灣以外、空 wrapper 和空 terrain 维持原值；八个 shader 变体都重开 DBPF 回读。尚未运行游戏内 shader 编译和显示验收，未部署。

### 零售地块索引的真实入口

以已校验的零售 EXE 反汇编交叉检查调用和字段访问。区块数组位于 `cZoningGrid+0`，每个 chunk 为 0x24 字节。49 项结束处 `+0x6E4` 是未索引地块表，边界在 `+0x708..0x714`，区块边长在 `+0x718`，活动标记在 `+0x71C`。在这个对象内扩大数组会覆盖其他状态。

| 操作 | 零售 VA | 确认依据 |
| --- | --- | --- |
| 构造、初始化、关闭 | 85B0F0 / 85B040 / 85B0C0 | 构造传 49×36；初始化及关闭均以 36 字节步长循环 49 次 |
| 插入或更新 | 85BE50 | ParcelQuery 准备暂时查询对象时调用 |
| 拆除 | 85AD30 | 读取 parcel +44/+48，按 chunk ID 和 slot 寻址并释放 |
| 点查 / 多边形查询 | 85ADA0 / 85B410 | zoning 调用提供同一 grid 和 2048 个输出槽 |
| 地块对地块查询 | 85C8F0 | 可暂时插入查询地块，85C98C 按当前 chunk ID 取记录并除以 7 还原区块坐标，再清理临时记录 |

因此如果扩成 32×32，插入和清理的 chunk/slot 必须共用同一套地址解析，ParcelQuery 还要同时改线性句柄的编码与解码。保留原对象和相邻字段，扩展区块存储应外置并绑定 grid 生命周期。此处是已核对的接口范围，尚无可部署的地块修复。

### 资料图 r2 改用零售预编译字节码

用户实测上一版的地下水图仍只显示小块蓝色区域。风图层的箭头不能作为等值着色覆盖范围的证据。零售加载器在 `0x4F81D7` 按材质表选数据库，随后以同一个数据库对象读取 Shader 表和片段表。原版 `0469A3F7:40212004` 的四个实例各带四个 D3D9 contour 像素程序。反汇编确认 16 条记录均用原控制图 UV 取样，且都有 -1024 米裁切常量，内容去重后是 8 个程序。零售缓存读取器在 `0x548DEB` 调 `0x422280`，后者直接调用 D3D9 CreatePixelShader。

`probes/runtime/half_coast_contour_bytecode.py` 校验四份零售表的 SHA-256，并在该调用前按字节长度和完整旧程序内容匹配后提交新程序。它用之前的尺寸符号分城市，仅在半海灣将 UV 改成 `worldPos.xy / 8192 + 0.5`，并将可见范围扩至 ±4096 米。普通城市保留原 UV 和裁切。8 个程序的汇编往返、x86 精确匹配及实际 D3D9 CreatePixelShader 调用均验证通过。同长度但字节变化、长度短一字节的输入保持原指针。

`SimCity-HalfCoast-8192-dataview-r2.exe` 已作为独立启动文件部署，SHA-256 `c1ff066b5fb64bb288f4c0b1a29a17f9ef163a72ce10a725b1aa2c2089d0018b`。2026-11-19 用户在既有半海灣存档实测，地下水着色已覆盖原 2048 米框外。地面污染等图层仍待观察。此截图验收的是地下水资料图，近景海面仍须在正常视图验收。

### r5 近景水面分块裁切

开发版 `cTerrainRegion::LODSelect` 调用 `FillOutMinMaxHeight`，当节点最低高度高于海平面时跳过水面。高度范围来自 `cTerrainSVT::FillFromProps` 读取的 `heightMapMinMax` 资源。已部署 r4 的表从三一岬复制。半海灣西半 8 个可见的最细块里有 3 个被旧表错误判为干地，例如图块 (7,7) 的最低高度原为 -856.10 米，实际是 -900.72 米，海平面为 -870 米。含水的上级节点能够画水，细分到错误的最细节点时会剔除这些水面。

`water_bounds.rs` 按 `cTerrainExporter::ComputeMinMaxValues` 和 `cTerrainSVT::MipXYToIndex` 的单块范围与索引顺序，从各层已编码的 F0 重建 341 对高度。构建器回读时逐项核对高度表和 F0。r5 SHA-256 `3b59384784eaa4d5e11b4f244a07f1d9a6e04f3d88750db600338d2e0a1c1254`。对照已部署 r4 的 713 条解压资源，唯一变化为 `00B1B104:6BEC5DFF:6E741C4C`。可见西半 8 个块都能通过最低高度判定，东半 8 个块均高于海平面。候选在工作区，近景实机表现待验证。

### 地块索引的运行时取样

零售 `0x6F124B` 以 `cZoningGame+0xF8` 调用 `0x85B040`，把 PlayArea 的四个水平边界和原版 256 米区块宽度送入地块索引。开发版的插入按区块宽度除以 8 写位图，点查保留乘 1/32 的换算。仅放大区块并保留 49 个记录会使两侧取不同的位。 `probes/runtime/half_coast_parcel_index.py` 可在启动 r2 后只读采集边界及每块占用。离线覆盖正常记录、无效记录长度和非半海灣环境三个场景。仍需纳入外置区块方案的完整增删查生命周期，并验证地坪、贴路分区及重叠拦截。

### 2026-11-19 实机测试收尾

按用户要求停止实机测试。当前 34 个地图及道路测试存档文件备份至 `Documents/SimCity/Games-backup-halfcoast-20261119-003858`，逐文件核对 SHA-256。半海灣地图及资料图包、格蘭港地图资源包和道路测试包已按哈希归档。恢复 `Games-isolated-road-20261117-223127` 对应的六个建筑包和 121 个存档文件，逐文件核对 SHA-256，保留启动器包。游戏关闭，待用户本人启动。r5 近景海面候选保留在工作区，外圈地块扩容和污染图层实测留待恢复测试时继续。
