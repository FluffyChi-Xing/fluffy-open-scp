# OpenSCP 能力普查（面向"预览工具 → 开发 IDE"转型）

> 2026-09-25 只读调查，未改任何代码。配套文档：
> [full-map-playable.md](full-map-playable.md)（全地图可游玩模组）、
> [map-workbench.md](map-workbench.md)（地图工作台）、
> [../design/blueprint-panel.md](../design/blueprint-panel.md)（蓝图面板）。
> 证据均为 `文件:行号`，来自对 `crates/`、`src-tauri/`、`src/` 的全量扫描。

---

## 1. 结论速览

OpenSCP 当前是"**读取面近乎全覆盖 + 三条局部写回闭环**"的预览工具。距"可产出的开发
IDE"，缺的不是底层解析（已很完整），而是**地图域写回、两个 stub 面板的编辑 UI、
节点化编排**三块。

| 层 | 已具备 | IDE 缺口 |
|---|---|---|
| Rust 库层 | DBPF 读写、Property 解析+写回、RW4/Raster 编解码、ERZ 解析、区域地形金字塔全破解 | 高度图回写、.egb 存档解析、地图生成器 |
| Tauri 命令面 | 约 90 条命令（`lib.rs:107-201`），地图/包/raster/workspace/version 全覆盖 | 无地图写入类命令、无存档类命令 |
| 前端面板 | map/raster/i18n/ui/versions 五面板可用 | property/asset 两面板 stub，蓝图编排不存在 |
| 写回与版本 | overlay 三链路 + atomic_fs + changeset 版本线 | 地图域尚未接入写回 |

## 2. Rust 库层盘点

| crate | 职责 | IDE 相关关键能力 | 证据 |
|---|---|---|---|
| dbpf | DBPF/DBBF 容器 + ERZ 规则 | mmap 读、LRU 缓存、RefPack；`write_uncompressed_overlay` 未压缩 overlay 造包（唯一"造包"入口） | `lib.rs:28-49`、`writer.rs:54-125` |
| rw4 | RenderWare4 模型/贴图 | Raster 编码器 `build_raw_bgra`+mip 链（绘制闭环）、DXT 解码、LotMask 六视图 | `raster.rs:37,114,215-290` |
| sc-properties | Property 核心解析 | `encode_canonical` 确定性写回；Lot/region_map/semantic/decal/inherit；LotEditorDocument | `lib.rs:163-202`、`lot.rs:28-92` |
| sc-registry | s3db 描述符名称解析 | 六表 HashMap、user 覆盖 main | `lib.rs:56-140` |
| sc-exporter | 导出 | OBJ/glTF/纹理/property JSON、rayon 批量 | `lib.rs:18-34` |
| sc-format | 格式嗅探 | 14 种签名零依赖探测 | `lib.rs:31-69` |
| sc-store | SQLite 持久层 | changeset 版本线（资源级前后字节、blob 32MiB）、ModProject/StudioConfig | `lib.rs:11-24,1027-1120` |

**区域地形（全地图模组的事实底座）**：`sc-properties/src/region_map.rs`——F0 高度图
tile（256×256 u16，341 tile 金字塔，`region_map.rs:286-292`）、ED 地面场
（128×128 u32，`:643-650`）、水位（`WATER_LEVEL=4928 raw ↔ -870m`，`:56-72`）、
地块表（`:211-246`）、资源画刷（`:249-274`）、PNG 渲染（`:279-474`）。
**无** .egb 解析、无高度图导入/回写、无噪声生成器。

## 3. Tauri 命令面（约 90 条，`src-tauri/src/lib.rs:107-201`）

| 域 | 代表命令 | 说明 |
|---|---|---|
| 地图（2 条） | `map_panel_list_regions` / `map_panel_render_region` | 区域枚举 + 全精度 4096²@8m PNG 渲染（`map_panel.rs:51,67`），**只读** |
| 包浏览 | `open/list_resources/read_resource_data/read_image_preview/read_rw4_preview/read_erz_preview/read_lot_editor_session...` | 含地形类型灰度图解码（`package_service.rs:3168-3220`） |
| Raster 绘制 | `read_image_rgba/save_raster_overlay/create_lot_overlay/register_decal_entry...` | 外部 PNG 导入→RGBA 画布→pixFmt21+mip→overlay 副本（`raster_edit.rs:113-828`） |
| 写回 | `patch_property_overlay` / `write_export_file` | property 补丁 → overlay 包；导出落盘 |
| 工作台 | `workspace_*`、`code_tree/read/write`、`version_record/capture_baseline/rollback` | 文档工作区 + 代码工作台 + 版本线 |
| 设置 | `settings_get/set_game_directory/game_directory_detect` | 默认 `C:\Games\SimCity\SimCityData`（`settings.rs:12`） |

**缺口**：无任何城市存档（`Games\<GUID>\*.egb`、MetaData）命令；无地图数据写入命令。

## 4. 前端面板盘点

路由注册：`src/router/routes/modules/studio.ts:4-115`。

| 面板 | 路由 | 状态 | 备注 |
|---|---|---|---|
| 地图 | `/studio/map` | ✅ 可用 | MapViewer 单 PNG + CSS scale（见 map-workbench.md §2） |
| Raster 绘制 | `/studio/raster` | ✅ 可用 | 像素笔刷/油漆桶/停车位/贝塞尔 + overlay 保存，超出设计文档 |
| i18n 文本 | `/studio/i18n` | ✅ 可用 | localeEditor store + baseline 快照 |
| UI 工作台 | `/studio/ui` | ✅ 可用 | uiWorkbench store + Shadow DOM 舞台 |
| 版本 | `/studio/versions` | ✅ 可用 | versionConsole store |
| 复写检测 | `/studio/diagnostics` | ✅ 可用 | overrideScan store |
| Property | `/studio/property` | ⬜ stub | 设计文档 `docs/design/property-editor.md` 已有 |
| 资产 | `/studio/asset` | ⬜ stub | 规划 `workspace-panels.md` §3.4/3.5（8 槽位材质/LOD 链） |
| 蓝图 | — | ❌ 不存在 | 见 blueprint-panel.md |

可复用交互资产：MapViewer/RasterCanvas 双份锚点缩放+平移实现、RasterHistory 撤销栈、
FSheet 抽屉、Resizable 三件套、FTree、FImageCropper（`src/components/...`）。
**无**任何 node-graph 依赖（`@vue-flow/*` 零命中）。

## 5. 质量工具链

- `pnpm test`（vitest + happy-dom）192 用例，含 `i18n-audit.test.ts`（静态扫描全部
  `t()` key 强制 zh/en 双语存在，`src/locales/i18n-audit.test.ts:1-20`）。
- `vue-tsc -b --noEmit`（`npm run check`）；eslint 10 + prettier。
- Rust：dbpf 14 测试、rw4 37、sc-properties 24+、sc-exporter 22+，含真实包回归
  （`crates/dbpf/tests/real_package.rs:22-48`）。探针 examples 约 100 个（开发用）。

## 6. 距离 IDE 的总体缺口（优先级）

- **P0 地图域写回**：高度图 PNG→u16 场→F0 tile 回写、画刷清单 property 追加、
  地块表编辑——这是"全地图模组"与"地图工作台"的公共底座（详见
  map-workbench.md §4）。
- **P1 两个 stub 面板**：property 面板（设计文档已定稿）、asset 面板（8 槽位
  材质/LOD 链，数据结构 `LotMaterialTextures`/`LotModelPayload` 已就绪，
  `src/api/tauri.ts:596-645`）。
- **P2 蓝图编排**：节点化开发范式（blueprint-panel.md）；.egb 存档解析
  （改钱/改资源，存档=规则库状态投影，`saves-exploration.md:40-44`）。
- **P3 发布工程**：Linux 打包（linux-packaging.md，2.5–3.5 人日）。
