# 地图工作台：现状与缺口（原有地图编辑 + 新地图创建）

> 2026-09-25 只读调查，未改任何代码。回答三个问题：①现有地图预览的缩放机制
> 是什么、如何改为"放大即提高渲染精度"；②在原有地图上做编辑缺什么；③从零
> 创建新地图缺什么。事实底座见 [capability-census.md](capability-census.md)。

---

## 1. 结论速览

| 维度 | 现状 | 判定 |
|---|---|---|
| 区域渲染 | 后端全精度 4096²@8m PNG（`region_map.rs:465`）+ 前端单 `<img>` | ✅ 数据层已到位 |
| 缩放机制 | CSS transform 拉伸单张 PNG，0.05–32 倍（`MapViewer.vue:211,91-93`） | ❌ 放大即插值，无精度提升 |
| 地图编辑 | 只读：无高度图回写、无画刷/地块表编辑 | ❌ 零写回 |
| 新地图创建 | 无噪声生成器、无灰度导入切割、无 ED/画刷编辑 | ❌ 全缺 |
| 复用资产 | 锚点缩放/平移、像素画布状态机、overlay 保存链、changeset | ✅ 齐备 |

## 2. 现状：预览链路与缩放机制

**数据链**：`map.vue:100` → `map_panel_render_region`（`map_panel.rs:67`）→
`render_region_png`（`region_map.rs:279-474`）：341-tile 金字塔拼合 → 4×4 box
滤波水陆判定 → 坡度光照 + 草地/岩石/水着色 → **全分辨率 PNG**（宽高 = 全幅
4096px，`meters_per_pixel` 恒 8.0，`region_map.rs:465-468`），附地块/画刷像素
坐标与资源图层 PNG（`RegionRenderDto`，`map_panel.rs:31`）。

**前端渲染**：`src/components/map/MapViewer.vue`——base64 喂 `<img>`
（:39-41,214-230），资源图层同尺寸 `<img>` 叠加（:222-230），地块框 SVG
`<rect>`（:231-249）。缩放 = `transform: translate() scale()` 作用在容器上
（:211），zoom clamp 0.05–32（:91-93），**无任何按级别重渲染/换源逻辑**。

**问题定位**：当前渲染已是数据层的最高精度（8m/px），`zoom > 1` 时浏览器对
已原生分辨率的图做双线性拉伸——放大不会"更精确"，只会"更糊"；米制标尺的
mpp 恒为 8（:23,275-277），与 zoom 无联动。

## 3. 缩放改造设计："放大即提高渲染精度"

**先校准预期**：8m/格是引擎地形粒度，任何插值都造不出真细节。正确设计是
**换数据源 + 矢量化 + 格子化编辑**三件事，让"放大"切换到更接近编辑真相的
表示，而不是放大像素：

| 级别 | 触发 | 数据源 | 渲染 |
|---|---|---|---|
| L0 远景 | fit ~ 1:1 | 现有全幅风格化 PNG | 现状不变 |
| L1 近景 | zoom > 1 | **视口窗口局部重渲染**：新命令 `map_panel_render_region_window(origin_world, size, detail)` 返回窗口内高保真 PNG（跳过远景降采样、保留逐格光照） | 渐进加载，PNG 按 `image-rendering: pixelated` 保持格锐利 |
| L2 编辑级 | 视口进入某地块 | 该城市 **256² 原始高度场**（F0 直读）+ 8m 格网线 + hover 高程读数 | "编辑渲染"替代"艺术渲染"；地块框/路网/画刷轮廓全部 SVG 矢量（任意缩放锐利） |

改造点：
- 前端：`MapViewer.vue` 的 `setZoom`/`onWheel`（:91-93,:153-170）接入级别状态机
  与窗口缓存（LRU）；标尺/HUD 的 mpp 随级别显示（L2 为"1 格 = 8m"）；交互骨架
  （锚点缩放、平移、fit）原样保留。
- 后端：`map_panel.rs` 增加窗口渲染命令（复用 `region_map.rs` 金字塔，无新解析）；
  L2 复用 `read_image_preview` 已有的 F0 灰度解码（`package_service.rs:3168-3220`）。
- **笔刷坐标系**：L2 下绘制以"格子"为单位（8m），而不是屏幕像素——放大改善的是
  落格准度，符合 8m 精度上限（0.75 m/px 是 Lot raster 惯例，见
  full-map-playable.md §4.3，与地图域无关）。

## 4. 原有地图编辑：缺口清单

| # | 缺口 | 现状 | 需要做的 | 量级 |
|---|---|---|---|---|
| E1 | 高度图回写 | `tile_arrange` 仅探针（重排 mosaic 可逆已验证，region-and-map.md:590-592） | 库化：f32 场→u16 量化→金字塔重排→F0 tile 字节→overlay 包；Tauri 命令 `map_panel_write_heightmap` | 大（核心） |
| E2 | 水位编辑 | 只读覆盖源已有（`region_water_level`，`region_map.rs:654-666`） | 写回区域 desc `0x51E7A18D`/`0x0E16BE1A`（`patch_property_overlay` 可承载） | 小 |
| E3 | ED 场编辑（含路网 b0） | 解码+渲染已有（`:643-650`） | ED tile 编辑器 + 回写 | 中 |
| E4 | 资源画刷编辑器 | 读取已有（`resource_brushes`，`:249-274`） | 清单 property 增删改 UI（`encode_canonical` 写出） | 中 |
| E5 | 地块表编辑器 | 读取已有（`plot_positions`，`:211-246`） | 地块 CRUD + 城市模板 Key 绑定（`0x9F2F9B65`→三层层栈模板） | 中 |
| E6 | 安全写与版本 | atomic_fs + changeset 已有 | 地图域接入 version_console（大 blob 注意 32MiB 上限，`sc-store/lib.rs:11-24`） | 小 |

## 5. 新地图创建：缺口清单

| # | 缺口 | 说明 | 依赖 |
|---|---|---|---|
| N1 | Perlin/fBm 生成器 | 全库无噪声实现；新增 `sc-mapgen`（或 sc-properties 模块）：fBm → 4096² f32，参数（频率/八度/种子）对齐区域 desc 种子 `257368037` 的用法 | E1 |
| N2 | 灰度 PNG 导入切割 | `read_image_rgba` 已可导入 4096² 上限 4096（`raster_edit.rs:24`）；补灰度→u16 场→金字塔切割 | E1 |
| N3 | 区域模板骨架 | 新区域 = 区域 desc（34 键，含半幅/全幅/水位/种子常量）+ 13-14 张 typed map 槽位 + 341 F0 + ED 组（region-and-map.md:120-126,82-88） | E1-E5 |
| N4 | 城市地块摆放 | 复用既有槽位模板（Reddit 实测：区域内城市摆放复用既有城市槽位模板） | E5 |
| N5 | 资源补齐 | 画刷清单条目批量生成（stamp 由引擎按种子程序化生成，无需位图） | E4 |
| N6 | 区域道路绘制 | ED b0 路网通道笔刷（复用 RasterCanvas 笔刷状态机） | E3 |
| N7 | 打包导出 | `write_uncompressed_overlay` 已具备；500MB 级大包的 IO/内存策略需专项（分片写入或直写 DBBF 64 位变体） | E1 |

## 6. 工作量估算

| 项 | 估时 |
|---|---|
| E1 高度图回写库化 + 命令 + 单测（真包回归） | 2–3 人日 |
| §3 缩放分级（L1 窗口渲染 + L2 地块编辑渲染 + 前端状态机） | 2–3 人日 |
| E2/E4/E5 property 侧编辑（复用 patch_property_overlay） | 2–3 人日 |
| E3 ED 编辑器（含 N6 道路笔刷） | 2 人日 |
| N1-N3 新区域生成与打包（含大包 IO 专项） | 3–5 人日 |
| 游戏内验收（出界行走/资源/接缝/偏移四项，对照 full-map-playable.md §6 FM4） | 1–2 人日 |
| **合计** | **12–18 人日** |

## 7. 风险与开放问题

1. **大包写回性能**：RT0 级 504MB 包的 overlay 直写需实测 mmap 拷贝耗时；
   必要时引入 DBBF（64 位偏移）变体（dbpf 已支持解析，`header.rs:23-24`）。
2. **游戏侧兼容**：自制区域包需过游戏启动校验（LoadGroup 引用计数成组加载，
   glassbox-engine.md:44-48）；区域 desc 34 键中未定名键逐一对齐是 N3 的
   主要不确定性。
3. ~~1px=0.75m 口径~~ 已解决：0.75 是 Lot raster 官方惯例（官方主流 0.75、
   少数 0.625/1.5，raster-painter.md:69），被需求稿误带入地图语境；地图域
   数据与面板代码均以 8m/格为准（无代码 bug，MapViewer 历史 -S "0.75" 零命中）。
4. **L1 窗口渲染的并发**：窗口命令应无状态（现 `map_panel.rs:1-5` 即无状态
   设计），避免前端缓存与后端会话不一致。
