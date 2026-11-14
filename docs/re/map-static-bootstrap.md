# 静态地图预览：原始路网、城市资源与地下水

2026-10-12。当前安装的两份 RegionTerrain 包共 15 个区域、130 个城市，均可只读静态 package 完成预览。地图预览不再扫描 Documents、读取玩家 EGB 或要求“存档对齐”。

## 权威资源链

`SimCityDataEP1.package` 中，`cLocalTemplateManager` 使用组 `95F4AEF0` 的属性：

| 属性 / 类型 | 含义 |
|---|---|
| `46066280` | 原始 bootstrap 资源 Key 数组 |
| `46066281` | 与 Key 数组平行的 site UID 数组 |
| `F240A8CA` | 对应地图元数据 JSON Key |
| `08068AED` | gzip 压缩的零售版 EcoGame bootstrap |
| `0A98EAF0:*:4529F96F` | 地图元数据 JSON，包含名称、城市与伟工位 |

这里的 EcoGame 数据是随游戏发布的初始基线，编码与 EGB 相近；它不是玩家保存过的城市状态。过去“静态文件没有区域道路实例”的判断不适用于这个资源类型。

区域身份来自 site `0` 的 `environment`。每座城市同时校验 site UID、RegionTerrain 地块表的 environment group、城市数、128×128 网格尺寸及解析器的字段校验和。禁止用地块坐标相似度、资源笔刷中心或某份玩家存档猜测地图身份。

实现：`crates/sc-properties/src/region_bootstrap.rs`。解压最多 128 MiB；目录枚举仅解码每个区域的有界头部。数据不完整时返回缺失/校验失败状态，不制造替代矿藏。

## 名称与地图覆盖

下表名称来自当前安装的 zh-tw 原始语言包，保留游戏原文。中文界面文案另由 zh-CN i18n 提供。

| Region group | 模板 | 游戏名称 | 城市 |
|---|---|---|---:|
| BEAF0510 | Confluence | 白水谷 | 5 |
| B12DE348 | Gallia | 藍綠森林 | 16 |
| C2A9C48F | CapeTrinity | 三一岬 | 3 |
| 9E67572A | Caspiar | 寧靜群島 | 6 |
| C04182E4 | LittleGorge | 泰坦峽谷 | 16 |
| A0B60DDE | EdgeWaterBay | 愛華特灣 | 7 |
| DD1935B6 | SC_Desolation | 荒涼 | 7 |
| BC357A2B | Sawyer | 三角洲 | 16 |
| 9F735B20 | Oasis | 綿延不毛之地 | 14 |
| E0183D94 | Tutorial | 奮進島 | 2 |
| D01FA985 | Horizon | 地平線群島 | 11 |
| E41A82B8 | Reflection | 反影環礁 | 7 |
| 936ECB37 | SC_Hinsara | 翠綠叢林 | 8 |
| 9E9B1FF0 | CaspianLake | 大理石湖 | 10 |
| DB25018C | TwinCities | 追日灣 | 2 |

旧 `TEMPLATE_PRIOR` 中 Gallia/LittleGorge、Tutorial/TwinCities 曾互换。预览现按 bootstrap 环境组连接元数据，不再使用高度/坐标打分。已知模板名表仅供其他旧接口回退。

## 坐标、裁切与资源

地形可见边长来自区域属性 `51E7A18D` 的 `5E3D16DF`，以世界原点居中，实测 8192、16384 或 32768 m。地形、生态、路径和城市资源共享原生 XY 坐标，显示方向统一交给 `map-coordinates.ts`，不得单独翻转底图或某个资源层。

城市模拟域为地块中心 ±1024 m，128×128 格，每格 16 m。所有城市框四边闭合；网格按真实城市域和底图边界裁切。

| typed map ID | 图层 |
|---|---|
| D779C976 | 煤 |
| 76D10FF2 | 油 |
| D74AF22B | 矿 |
| 0F92DC56 | 地下水 |
| 1D52BE3E | 森林 |
| 28F896A4 | 生态土壤 |
| 6F9F6AB3 | 绝对高度，仅用于方向/配准研究，不替换预览地形 |

预览地下水分成两个独立图层：**区域地下水**（`waterTable`）在整个可见区域使用 ED byte 0，包括城市范围内；**城市地下水**（`cityWaterTable`）只显示各城市 `0F92DC56` 静态网格，城市域外透明，不回退到区域 ED。缺少城市静态数据时，城市地下水不可选，区域地下水仍可查看。城市详情中的地下水面积占比对应城市地下水。

此前离线导出的合成地下水图仍采用“城市外 ED、城市内 typed map”的规则，与这两个独立预览层区分。ED byte 1 为森林、byte 2 为土壤；森林与土壤预览仍按城市内外合成。离线 16-bit 输出中，ED 的 8-bit 值乘 257；预览颜色纹理按统一 0–65535 范围缩放为 8-bit。

城市域的来源掩码区分“有效零值”和“没有城市数据”：森林/土壤合成中零值也必须覆盖区域 ED；城市地下水的零值保持为零，不借用区域值。煤、油、矿的城市域外没有已确认的最终分布，不使用笔刷扩散生成圆形矿藏。资源纹理采用最近邻采样，防止跨城市边界插值污染。

## 2026-10-30 新样本：地图资源不是一张图，而是三层注册链

新的 PDB/source-tree 样本把“添加地图资源”落到了可编译的接口顺序：

```text
EcoGame scripts
  → cEcoGame::InitGame
  → cEcoMapEntry::FillFromProps
  → cEcoMap::FillFromProps + cEcoMap::Create
  → cEcoMap::InitFromImage / EGB Read-Write
  → cGraphicsMap::Create / Update
```

- `cEcoGame::InitGame`（RVA `0x60BDC0`）按 `NumMapEntries()` 创建每个模拟地图。地图
  条目从 Property 的 `0x07E0AE01` 取得 `maxValue`，`0x07E0AE05` 提供初值，
  `0x07E0AE06` 提供图像 Key；`cEcoMap::InitFromImage`（RVA `0x63E820`）先尝试
  `0x03E421F0` 的 16-bit 图像，失败再尝试 `0x03E421EC` 的 8-bit 图像，并按
  `maxValue` 缩放到 `mCellsX×mCellsY`。
- `cEcoMap::FillFromProps`（RVA `0x63F1F0`）把 `MapBounds()` 直接作为 `mCellsX/Y`，
  并从 `0x0839470B`、`0x0839470C` 分别读取模拟开关和 debug 开关。地图网格大小因此
  由脚本 MapBounds 与地图资源共同决定，不能只换一张 PNG。
- 图形侧 `cGraphicsMap::Create`（RVA `0x7CBC00`）从条目 Property `0x0AF3DAC7`
  读取纹理 Key，用同一 `cellsX/cellsY` 创建格式 24、用途 81 的 GPU 纹理；
  `Update`（RVA `0x7CBA10`）按 `mModCount` 将模拟值归一化写入纹理。模拟 EcoMap、
  图形纹理和区域 ED 是不同资源，编辑器必须分别记录 TGI、尺寸和来源。

这条链带来一个可执行的最小地图资源实验：先新增一个 map-entry Property 与一张
`0x03E421F0`/`0x03E421EC` 图像，再让脚本 `NumMapEntries` 能枚举它；随后验证 EcoMap
读回、`mMapSum`、GraphicsMap 纹理和 UI `BuildMapsJSValue` 可查询性。缺任何一层都应
报告“资源存在但未注册”，不能把预览纹理当作游戏地图已添加。

地图写回也已有原生对照：`cTerrainSystem::RegisterPackage`（RVA `0x405520`）通过
`CreatePackageSave` 建立保存包并注册 save area；`cTerrainEditor::WriteHeightMapToFile`
（RVA `0x40A850`）将 `cImageData16` 通过 `SaveNamedResource` 写入该区域。OpenSCP 的
overlay writer 应沿这条顺序写入并立即重读资源，而不是直接替换原始 package。

此前独立批处理对 130 城的煤/油/矿/地下水 PNG 逐值回读一致；129 城高度在八种方向候选中原生方向最吻合。CaspianLake 的 1027 城高度过平（标准差约 5.6 cm），另用地下水、森林、土壤与 ED 对比，三场均支持原生方向。方向证据不能简化为“全部高度都证明同一方向”。

## 道路与状态版本

只使用 site `0` 的曲线和区域 units。城市 bootstrap 中包含道路副本，叠加会重复绘制。道路外观沿原有 `PathEntry → extrusion → model/material` 链读取 Game/App/Graphics 包。全部 15 图的本次扫描无不支持的道路组件；这不等同于隧道、桥梁光照和游戏画面逐像素一致。

生产 `read_state` 支持 v17、v18、v20。v17/v18 在 ticks/nextUID 后有空扩展；v20 没有。实际 v17 使用 map v3 的 32-bit sum，v18/v20 使用 map v4 的 64-bit sum，必须按 map version 读取，不能按游戏版本直接猜测。

## 预览交互与城市词条

`map_panel_region_3d` 仅接收 packagePath/group，返回 `staticLayers`、`staticStatus` 和道路外观。相邻 EP1 缺失或无效时保留地形及区域生态预览，明确提示城市资源与道路不可用；Game 缺失时单独提示道路外观缺失。

右侧不列出全部槽位。点击地图中的城市后，高亮其边框并展示该城名称、介绍、可建设比例、风力、铁路/航运、中心坐标、高程、范围、资源组和资源信息。切换区域/包或重载会清除选择。

城市词条来自 Game JS 注册表的 `cityName`/`cityDescription` 语言键及标量字段，由 `region_city.rs` 只读解析，绝不执行脚本。介绍里的 0–3 资源等级与实际网格非零面积占比分开显示，两者均不是可开采储量。中文介绍由已安装 zh-tw 包解析；如果安装了相邻 `Locale/en-us/Data.package` 则提供英文介绍，否则英文界面明确显示该语言介绍缺失，不编造翻译。

切换资源时道路继续可见；植被和水面暂时隐藏以便检查。正常与全屏预览共享资源选项和选中槽位，面板高度受视口约束，城市详情自身滚动。

## 复现与产物

运行生产解析器的全图检查（只需本机合法安装的数据，不读取任何存档）：

```powershell
cargo run -p probes --example verify_static_maps -- "<SimCityData>" "tmp/map-static-analysis/app-integration"
npm test -- src/components/map
npm run check
cargo test -p sc-properties --lib
cargo check -p fluffy-open-scp
```

检查器枚举 RT0/RT1，校验 bootstrap 覆盖、130 城 UID/environment、六种资源网格和道路资产；可选输出目录生成生产数据 JSON 供浏览器预览验收。原始游戏数据、图片和大型压缩包不进入 Git。

本次验证：全图生产解析 15/15、城市 130/130，中文城市介绍 130/130；sc-properties 单元测试 71 项、地图前端测试 27 项通过；前端生产构建与 Tauri `cargo check` 通过。浏览器使用生产读取器导出的真实 JSON，替代 IPC 挂载实际地图面板，检查点击选择、清除选择、语言切换、全屏、1500×950/1100×700 视口及缺失静态数据状态，无页面或 WebGL 错误。这是组件/读取器集成验收，未声称启动打包后的桌面程序验收。

全局 `i18n-audit.test.ts` 的静态词条扫描仍把已有图表组件局部 i18n 的 `tickHint/chartSource/shareHint/requests/source` 当成全局词条，产生 10 条缺失误报。该审计的动态前缀及消息编译测试通过；本次城市详情另有实时语言切换组件测试。

本机研究产物全部集中在 `tmp/map-static-analysis/`：`all-maps-2026-10-12/` 保存全 15 图的独立图层、合成 PNG、总览和核验报告；`app-integration/` 保存应用读取器导出数据及界面截图。历史遗留的其他 tmp 文件未自动搬移或删除。

已解决的是“现有地图静态基线可独立预览”。修改资源后重建 bootstrap、创建游戏可接受的新区域、运行中模拟与写回仍是后续工作，不能把这次只读预览当作完整地图编辑器。
