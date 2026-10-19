# probes —— 探针目录册

全项目一次性/取证探针（probe）的唯一存放处。每个探针是独立的小型
命令行工具，只读游戏资源、输出到 `tmp/`，用于逆向取证、数据链验证与
回归对拍；**不参与应用构建**（`publish = false`，无 lib/bin）。

## 运行

```bash
cargo run -p probes --release --example <名称> -- <参数...>
```

各探针的参数写在文件头注释里（`//!` 首行 = 用途，其后为用法/输出说明）。
输出统一落在 `tmp/<主题>/`，`tmp/` 是可丢弃的沙盒。

## 新增探针

1. 在对应分类目录下建 `probes/<分类>/<名称>.rs`，文件头写 `//!` 用途与用法；
2. 在 `Cargo.toml` 对应分类注释块下补一个 `[[example]]`（name = 文件名）；
3. 依赖只允许本 workspace crate 与既有通用依赖；探针保持独立、不共享代码，
   结论沉淀进 `docs/re/`，可复用的发现用 `ctx learn` 注册。

## 分类一览

| 目录 | 领域 | 数量 |
|---|---|---|
| [`package/`] | DBPF 容器 / 索引 / TGI 通用查找 | 32 |
| [`property/`] | 属性表（0x00B1B104）解析与对比 | 12 |
| [`locale/`] | 字符串表（0x0A98EAF0） | 11 |
| [`decal/`] | 贴花 / 地面覆盖 / 招牌 | 27 |
| [`lot/`] | 地块（LotMask / 合成 / prop 槽位） | 23 |
| [`menu/`] | 菜单 / 工具条目 / 分类 / 图标 / 造价 | 14 |
| [`region/`] | 区域 / 城市 / tile 金字塔 / 引导 / 状态 | 59 |
| [`road/`] | 道路 / 路径 / curve | 6 |
| [`terrain/`] | 地形 / 水位 / 画刷 / 地面纹理 | 23 |
| [`model/`] | RW4 模型 / 材质 / 栅格 / 着色器 | 38 |
| [`ui/`] | 游戏 UI 脚本 / 布局 / JS | 8 |
| [`effect/`] | Swarm / 特效 | 5 |
| [`zone/`] | 分区颜色与图标 | 4 |
| [`registry/`] | sc-registry 注册表查询 | 3 |
| [`runtime/`] | 游戏运行时注入 / 日志（python） | 2 |

## 迁移来源与改名

原 `crates/*/examples/` 已整体迁入本目录；`tools/` 下两个 python 运行时探针
迁入 `runtime/`。历史提交与文档中的旧调用方式对照：

`cargo run -p <crate> --example <名>` → `cargo run -p probes --example <名>`
（名字不变，仅换 crate）。两个跨 crate 重名探针改名：

| 原路径 | 现路径（example 名） |
|---|---|
| `crates/sc-exporter/examples/find_instance.rs` | `decal/decal_find_raster.rs`（decal_find_raster） |
| `crates/sc-exporter/examples/prop_dump.rs` | `property/unit_prop_dump.rs`（unit_prop_dump） |

运行时注入/钩子类探针子系统（Frida/D3D9/UI 观察器）保留在
[`tools/dynamic/`](../tools/dynamic/)，有独立 README 与协议文档，未迁动。
`tmp/` 下的一次性脚本（`decode_shader_container.py`、`compare_regions.py`、
`check-*.cjs` 等）与探针输出数据留在原处；其中可复用的脚本将来再按本目录
约定收编。

## package —— DBPF 容器 / 索引 / TGI 通用查找

- [`dbpf_fixture`]（dbpf）：冒烟工具：为前端 dbpf-writer 测试生成 write_uncompressed_overlay 十六进制 fixture。
- [`dump_one`]（sc-properties）：探针：按 TGI 转储单条资源。仅开发用。
- [`er2_collect`]（dbpf）：探针：收集目录下所有脚本包中的 0x08068AEC/AEB 条目并导出。仅开发用。
- [`erz_check`]（dbpf）：探针：对真实 ERZ 文件运行结构解析并打印摘要。仅开发用。
- [`find_by_type`]（sc-exporter）：冒烟工具：按 type_id 列出全部资源（计数 + 按 group 分布 + 抽样 instance）。
- [`find_group`]（sc-properties）：按 group 找资源（全类型）。仅开发用。
- [`find_instance`]（sc-properties）：按 instance 找资源（全类型）。仅开发用。
- [`find_instance_all`]（sc-exporter）：冒烟工具：全包全类型搜索某 instance 的资源（列 TGI+大小+RW4 材质槽）。
- [`find_instance_loc`]（sc-exporter）：冒烟工具：定位 instance 在哪个包哪个组（含尺寸）。
- [`find_raster_all`]（sc-exporter）：冒烟工具：在多个包里查找同名 raster 资源的全部副本（尺寸/格式对比）。
- [`find_tgi`]（sc-properties）：探针：跨包按 instance 搜资源（任意 type/group）。仅开发用。
- [`group_census`]（sc-properties）：按 group 统计 property 数（找区域组）。仅开发用。
- [`idx_type_scan`]（dbpf）：探针：跨包扫描索引中指定 type 的条目。仅开发用。
- [`inspect`]（dbpf）：打开一个 .package 并打印概况：
- [`inst_reuse`]（sc-properties）：检查同类型 instance 跨 group 复用情况。仅开发用。
- [`json_grep`]（sc-properties）：探针：扫包内 JSON（0x0A98EAF0）/文本资源，解压后按关键词过滤并转储命中。仅开发用。
- [`list_group`]（sc-exporter）：冒烟工具：列出某 group 的全部 B1B104 记录。
- [`package_dump_all`]（sc-properties）：探针：列出包内全部条目（TGI+尺寸），property 条目附完整内容转储。仅开发用。
- [`payload_grep`]（dbpf）：探针：在包内**解压后**的资源负载里搜 ASCII 子串（DBPF 条目多为 RefPack 压缩，
- [`pixel_sample`]（sc-exporter）：一次性采样工具：打印指定 PNG 指定点位的 RGBA（诊断用，可随时删除）。
- [`pkg_diff`]（sc-properties）：整包逐条目对比（TGI 集合 + 解压后内容字节比对）。仅开发用。
- [`pkg_diff_bytes`]（sc-properties）：打印两包中指定 TGI 解压内容的逐字节差异位置与值。仅开发用。
- [`pkg_extract`]（sc-exporter）：冒烟工具：按 TGI 抽取条目原文。
- [`pkg_survey`]（sc-properties）：探针：包类型直方图 + 按 instance 前缀统计。仅开发用。
- [`pkg_toc`]（sc-exporter）：冒烟工具：包 TOC 全量清单——按类型分组统计 + 全条目 TGI 列表（可选按类型过滤）。
- [`probe_engine`]（dbpf）：GlassBox 引擎痕迹探针：在指定 package 中解出目标 typeId 的资源，
- [`raw_id_scan`]（dbpf）：探针：在包内**全部资源**的原始字节里搜索指定 u32（LE/BE 两种字节序）。
- [`scan_preview_gaps`]（dbpf）：扫描真实包中预览缺口类型的样本：TGA/Cursor/Greyscale/TTF/0x024a0e52/ER2。
- [`single_tgi_overlay`]（dbpf）：Copy one complete resource unchanged into an exclusive-created minimal overlay.
- [`slot_compare`]（sc-properties）：对比两包 341-tile 组的 instance 集合。仅开发用。
- [`slot_compare2`]（sc-properties）：同包内比较两类型的 instance 集合。仅开发用。
- [`tgi_peek`]（sc-exporter）：冒烟工具：按 TGI 抽资源前 N 字节做 16 进制 + ASCII dump（类型取证）。

## property —— 属性表（0x00B1B104）解析与对比

- [`alert_audio_scan`]（sc-properties）：探针：验证警报 property(0x098A44F2) 是否关联可播放音频。仅开发用。
- [`instancetype_probe`]（sc-properties）：探针：按 InstanceType（group 低 16 位）统计 property 资源分布，并对
- [`key_ref_scan`]（sc-properties）：
- [`kv_survey`]（sc-properties）：只读分析：0x00B1B104:40E1E800 族 property 的结构与引用调查。
- [`prop_diff`]（sc-properties）：语义级 property diff：两包同名资源按 key 对比，输出值变化。仅开发用。
- [`text_ref_hunt`]（probes）：在目录下全部包中反查包含指定 Text 引用（表，串ID）的属性资源，输出 TGI 与命中 hash。
- [`prop_bytes_diff`]（probes）：两个属性资源解压后逐字节对拍，定位拷贝/改写引入的编码差异。
- [`prop_layout`]（probes）：逐条目打印属性资源布局（偏移/type/flags/数组头/Text 表:串），取证编码标志位。
- [`prop_roundtrip`]（probes）：属性资源 解析→encode_canonical→逐字节对比，验证编码往返无损。
- [`prop_dump`]（sc-properties）：探针：按完整 TGI 打印单个资源的属性（PropertyFile 调试视图）。仅开发用。
- [`prop_fixture`]（sc-properties）：冒烟工具：为前端 property-writer 测试生成 encode_canonical 十六进制 fixture。
- [`property_of_model`]（sc-properties）：冒烟工具：反查引用指定 RW4 模型（LOD1-4 键）的 property 实例。
- [`property_semantics_survey`]（sc-properties）：探针：property 语义子类型普查（排除 Unit 0xC000 后的特殊语义家族扫描）。
- [`reverse_key_search`]（sc-properties）：反向搜索：哪个属性文件的哪个 hash 里出现了指定 Key instance。仅开发用。
- [`semantic_gap_probe`]（sc-properties）：探针：语义 tag 漏检面普查。
- [`unit_prop_dump`]（sc-exporter）：Unit 属性全量 dump——解析 0xB1B104 属性表，输出语义分类、全部属性

## locale —— 字符串表（0x0A98EAF0）

- [`locale_dump`]（dbpf）：导出 locale 资源全部条目。仅开发用。
- [`locale_extract`]（dbpf）：提取 locale 资源中包含指定子串的条目（hash key → 文本）。仅开发用。
- [`locale_lang_probe`]（sc-properties）：探针：对比 zh-tw / en-us 语言包同表内容，确认语言分布。
- [`locale_mount_scan`]（sc-properties）：探针：扫描一批包，统计每个包内 0x0A98EAF0 资源的数量与表 id 冲突情况。
- [`locale_names_dump`]（sc-properties）：探针：转储 zh-tw locale 区域命名 JSON 全文。仅开发用。
- [`locale_names_probe`]（sc-properties）：Export English names: <Game.package> <output.json> <locale.package> [...]
- [`locale_probe`]（sc-properties）：探针：检查 zh-tw locale 包里重复表条目与具体字符串内容。仅开发用。
- [`locale_tgi_full`]（sc-properties）：探针：打印一个包内全部 0x0A98EAF0 资源的完整 TGI 与内容头部。仅开发用。
- [`locale_tgi_scan`]（sc-properties）：探针：列出 locale 包内 0x0A98EAF0 资源的完整 TGI 分布。仅开发用。
- [`overlay_locale_diff`]（sc-properties）：探针：对比 baseline 包与 OpenSCP 产出 overlay 包的 locale 词条资源字节与
- [`vanilla_locale_check`]（sc-properties）：探针：在原版 locale 包里查找 overlay 属性引用的字符串 ID，并对比
- [`locale_companion_build`]（probes）：生成 locale 伴生对照测试包（全量/部分 × 仿原版/紧凑排版），实机判定 LocaleManager 加载目录、排版敏感性与同名表合并语义。

## decal —— 贴花 / 地面覆盖 / 招牌

- [`decal_anchor_sweep`]（sc-exporter）：只读扫查：decal 原点与建筑表面的耦合统计（2026-10-01 引擎对齐验证）。
- [`decal_chain`]（sc-exporter）：只读探针：lot property 的 Decal 单元 → Decal Atlas 字典条目 → Raster 贴图
- [`decal_composite`]（sc-exporter）：decal 合成探针 —— 与 lot_composite 同模式的离线合成实验。
- [`decal_dict_survey`]（sc-exporter）：全字典普查（N7）：decal 字典 → material → 0x2D shader-def 实例 + shaderdef 内容字符串。
- [`decal_extra_fields`]（sc-exporter）：冒烟工具：打印 lot decal 单元的第二 Key(0x0D109090)/Int32(0x0D1090A0)/Float(0x0D1090B0)。
- [`decal_field_survey`]（sc-properties）：只读取证：lot decal 单元各字段在真实包中的存在率与取值范围。
- [`decal_find_lots`]（sc-exporter）：只读工具：按 decal 条目 instance 反查使用它的 lot。
- [`decal_find_raster`]（sc-exporter）：一次性探针：跨包查找指定 instance 的全部资源类型（decal raster 定位用）。
- [`decal_float_scan`]（sc-exporter）：只读扫描：为 holo/墙招牌判别找静态数据依据。
- [`decal_material_data`]（sc-properties）：动画招牌（decal unit materialData[1] > 0）的三元组全库分布取证。
- [`decal_material_key`]（sc-exporter）：冒烟工具：列出 decal 字典的 material Key（T/G/I）。
- [`decal_material_slots`]（sc-exporter）：冒烟工具：decal 聚类 material 资源（RW4）的贴图槽清单（含 0x2D shader-def）。
- [`decal_probe`]（sc-exporter）：探针：定位 package 中的 Decal Dictionary（贴花图鉴）并打印结构摘要。
- [`decal_projection_probe`]（sc-exporter）：只读探针：钉死 decal 投影盒的语义（`modelToTexture` 的盒中心与 Z 厚度）。
- [`decal_raster_stats`]（sc-exporter）：只读统计：decal raster 的通道形态取证（2026-10-01 边缘/暗部/色彩三问）。
- [`decal_render_probe`]（sc-exporter）：decal 全量重渲染探针（引擎公式离线软件渲染，2026-10-01）。
- [`decal_resolve_replay`]（sc-exporter）：冒烟工具：复刻 package_service::resolve_decal_textures 的分派逻辑，
- [`decal_roundtrip_probe`]（sc-exporter）：探针：对真实包内的 Decal Atlas property 做写回闭环验证。
- [`decal_scan`]（sc-exporter）：decal 特征扫描：全包解码全部 decal 条目，按像素签名过滤目标贴花。
- [`decal_sign_thumbs`]（sc-exporter）：只读工具：导出招牌字典（material 0x73684EFC）全部唯一条目的解码贴图。
- [`decal_trace`]（sc-exporter）：冒烟工具：单条 decal 的解码语义追踪——R/G/B/A 四通道独立成像 +
- [`decal_unit_dump`]（sc-exporter）：只读探针：打印指定 lot 的全部 Decal unit 静态字段
- [`dump_decal_4color`]（sc-exporter）：冒烟工具：按运行时口径（decode_lot_mask_rgba）解码指定 decal 条目。
- [`dump_decal_channels`]（sc-exporter）：冒烟工具：导出 decal raster 的四通道灰度图 + 指定扫描线剖面。
- [`graffiti_decal_dump`]（sc-properties）：涂鸦族（材质 0xE5390A98）贴图取证：权重掩码 vs 真彩图 定谳。
- [`hole_decal_dump`]（sc-properties）：破洞族（decalInteriorMap = 无 Color1-4 的 atlas 条目）贴图取证：
- [`scan_decals`]（dbpf）：扫描游戏 .package：查找 render script（含 "scDecals" 字节）与 renderGroup 魔法值 0x96AF4B50。

## lot —— 地块（LotMask / 合成 / prop 槽位）

- [`atlas_dump`]（sc-exporter）：冒烟工具：解码 "Lot Textures" 图集并切出 4×4=16 格 PNG。
- [`find_lot_by_model`]（sc-exporter）：反查探针：给定模型 RW4 instance，找出引用它作 LOD1~LOD4 的 lot property。
- [`footprint_project`]（sc-exporter）：只读取证：把建筑物模型的**地面脚印多边形**投影到其所属 lot 的合成图上。
- [`lot_anchor_probe`]（sc-properties）：锚点探针：对带 placement(平移) + 可解 LotMask + 模型的 property，
- [`lot_axes`]（sc-properties）：取证工具 v2：扫描包内 Lot 属性，按 Unit 类型统计并打印原始变换，
- [`lot_cavity_stats`]（sc-exporter）：统计检验：建筑中心是否显著落在 mask 空腔（未覆盖区）内（偏心假设）。
- [`lot_compose`]（sc-exporter）：探针：读取真实 building lot，按 `generic_lot` 像素着色器公式做多通道合成并输出图片。
- [`lot_composite`]（sc-exporter）：LotMask raster 批量引擎式合成 —— 严格按 `generic_lot` 像素着色器
- [`lot_diff_probe`]（sc-properties）：对比两个 lot property 的全部属性。仅开发用。
- [`lot_mask_alignment`]（sc-exporter）：冒烟工具：裁决 lot mask 与模型/prop 的坐标绑定方向（镜像取证）。
- [`lot_mask_probe`]（sc-properties）：冒烟探针：扫描包内带 LotMask 的 property,执行会话同款 LotMask 解码链路。仅开发用。
- [`lot_prop_offset_probe`]（sc-exporter）：只读取证：lot 的地面定位三要素（P=0xDB7FB17 / unitOffset=0x0CCB7FC9 /
- [`lot_property_scan`]（sc-exporter）：取证探针：列出同一 instance 下的全部 lot property 变体（不同 group），
- [`lot_render_props`]（sc-properties）：只读取证：dump 一个 lot property 的关键渲染属性值。
- [`lot_sibling_probe`]（sc-properties）：检查同 InstanceId 的 property 资源分布：变体 lot 是否有兄弟资源携带 placement。仅开发用。
- [`lot_size_fallback_probe`]（sc-properties）：推导 mask 光栅尺寸 ↔ LotSize 换算：扫包内同时带两者的 lot。仅开发用。
- [`lot_tree_scales`]（sc-exporter）：冒烟工具：实测 lot 树道具的放置缩放与推算实际树高。
- [`lot_uv_range`]（sc-exporter）：探针：打印模型各 mesh 的 texcoord0 范围。
- [`lotmask_probe`]（sc-properties）：冒烟探针：统计 EP1 property 的 LotMask raster 像素格式分布（阶段 3.1 取证）。
- [`mask_weights`]（sc-exporter）：检查 raw mask 指定区域的通道权重（诊断"变绿"根因：区域是否低于 0.5 阈值）。
- [`prop_chain_probe`]（sc-properties）：探针：找含 Prop 槽列（0x0C12EF40..）的 lot 属性文件，dump 全部 hash +
- [`refined_replay`]（sc-exporter）：离线重放前端 refinedGround 的合成算法（阈值+优先级链+frac 平铺），
- [`spawner_probe`]（sc-properties）：探针：扫描含 Spawner 列的 lot，dump spawner 全列值并尝试解析 agent 引用

## menu —— 菜单 / 工具条目 / 分类 / 图标 / 造价

- [`cost_group_census`]（sc-properties）：探针：三种造价/维护哈希在哪些 class（group）里、以及样例条目。
- [`eco_unit_cost_scan`]（sc-properties）：探针：EcoGame 脚本包（SimCityUserData/EcoGame/*.package）的 Unit 条目
- [`enable_debug_tools`]（sc-exporter）：实验工具：把三个 debug 工具（Edit Unit Prop Tool / Edit Snap Points Tool /
- [`icon_probe`]（sc-properties）：探针：警报/图层图标 PNG 实例在哪个包。
- [`menu_asset_extract`]（sc-properties）：生成器辅助：为 extra-containers（公共交通/公園等运行时装配分类）提取
- [`menu_cost_dump`]（sc-properties）：生成器：把菜单条目的经济字段从实际安装包提取成 costs.json，供 UI 工作台
- [`menu_survey`]（sc-properties）：探针：普查 Menu / Menu2（InstanceType 0xC900 / 0x8A01）工具定义的两个分类键，
- [`path_cost_scan`]（sc-properties）：探针：扫描菜单条目（8A01/C900）上的路径单价键 ——
- [`road_cost_probe`]（sc-properties）：探针：道路菜单条目 → piece 定义（0xD13BB4A6）→ 全部键值，
- [`tool_info`]（sc-properties）：探针：按 instance dump 一个 property 资源（0x00B1B104）的全部字段，
- [`ui_menu_survey`]（sc-properties）：探针：导出 Menu/Menu2 (0x00B1B104) 全部条目的摘要，用来还原底部工具条的菜单层级。
- [`ui_tool_dump`]（sc-properties）：探针：导出全部 Menu/Menu2 工具定义（0x00B1B104, InstanceType 0x8A01/0xC900）
- [`unit_config_name_scan`]（sc-properties）：探针：40E1C000（资产单元配置）条目的 名称文本id + Cost + 维护，
- [`unit_cost_scan`]（sc-properties）：探针：扫描 40E1C000（资产单元配置）组 property 的 Cost（0x0C8EE92F）等经济字段，

## region —— 区域 / 城市 / tile 金字塔 / 引导 / 状态

- [`cape_bootstrap_probe`]（sc-properties）：Build a non-destructive Cape Trinity bootstrap candidate. Unit transforms
- [`city_z_verify`]（sc-properties）：探针：全链路一致性——模板城市 z vs 高度图采样 z。仅开发用。
- [`desc_keys_resolve`]（sc-properties）：探针：区域 desc 14 个 Key → 组内 property 名解析（确认有无独立路网资源）。仅开发用。
- [`dto_json_check`]（sc-properties）：探针：确认 RegionRenderDto 序列化键名。仅开发用。
- [`ed_align`]（sc-properties）：ED 三通道 × 帧假设 × 水陆分类 检验。仅开发用。
- [`ed_channel_view`]（sc-properties）：探针：ED b0/b1/b2 通道灰度渲染 + b0 值直方图（找路网/铁路编码）。仅开发用。
- [`ed_frame_probe`]（sc-properties）：探针：ED 金字塔网格硬编码提取 + ED 相对帧朝向判定（草量 b1 × 平坦度相关性）。
- [`ed_grid_cross`]（sc-properties）：探针：ED 金字塔网格跨区域一致性 + ED/F0 槽位关系 + 水面子资源属性。
- [`ed_hist`]（sc-properties）：ED b2/b1 通道取值直方图（top 值）。仅开发用。
- [`ed_plot_contrast`]（sc-properties）：探针：ED 相对帧判定——地块框内外的 b0/b1/b2 对比度。
- [`ed_render`]（sc-properties）：ED 地面场金字塔拼合渲染：查森林区周期条纹。仅开发用。
- [`ed_road_channels`]（sc-properties）：探针：ED 场路网/桥梁/铁路通道语义深查——直方图 + 三通道灰度图 + 路网叠色图。仅开发用。
- [`ed_road_overlay`]（sc-properties）：探针：b0 路网判读叠合图：水=蓝 b0>=250=白 b0<=2=红 其余=灰。仅开发用。
- [`ed_road_split`]（sc-properties）：探针：b0<=2 按 0/1/2 分色（公路 vs 铁路?）+ 地块表 ids/positions 转储。仅开发用。
- [`ed_road_verify`]（sc-properties）：探针：b0 亮带/暗线 与 水陆掩码、坡度 的关系判定（公路 vs 河流 vs 铁路）。仅开发用。
- [`ed_road_zoom`]（sc-properties）：探针：白水谷中央区 b0 低值放大渲染（区分公路/铁路线型）。仅开发用。
- [`ed_stats`]（sc-properties）：ED u32 字段统计：各字节通道的非零分布 + 抽样值。仅开发用。
- [`final_render`]（sc-properties）：探针：用正式 render_region_png 管线（SHARED_ED_GRID + 数据驱动水位 + 资源图层）渲染验证。仅开发用。
- [`greatwork_hunt`]（sc-properties）：探针：定位区域 desc 未释读 Key 1437550736 + 全部地块表转储（Q4）。仅开发用。
- [`group_tiles`]（sc-properties）：区域组 tile 数 + 高度统计（含零 tile 计数）。仅开发用。
- [`gw_env_dump`]（sc-properties）：探针：environment 属性全文 + JS GreatWorks 常量搜索由外部完成。仅开发用。
- [`mini_thumbs`]（sc-properties）：探针：渲染全部未命名区域的地形缩略图（供目视消歧）。仅开发用。
- [`namehash_probe`]（sc-properties）：name-hash 穷举探针：多种名字模板/坐标域/mip 变体 × FNV1a，对照包内 instance。仅开发用。
- [`offset_check`]（sc-properties）：探针：验证地块/画刷 → PNG 像素换算是否在界内（复刻前端 toPx）。仅开发用。
- [`orient_check`]（sc-properties）：定向终判：ED 道路线网 × F0 陆地 × 地块框的四种帧组合打分。仅开发用。
- [`plot_pair`]（sc-properties）：区域 ↔ 地块表配对探针：同 instance 的地块表每区域一份（group 不同），
- [`plot_pos_join`]（sc-properties）：探针：BEAF0510 地块表（names+positions）与 Confluence 模板城市坐标精确配对。仅开发用。
- [`precreate_unit_scan`]（sc-properties）：Locate static PrecreateUnits tables used by a new EcoGame bootstrap.
- [`region_3d_probe`]（sc-properties）：探针：region_3d 全链实测（高度/地面 PNG + 地块名 + 伟工位）。仅开发用。
- [`region_bind`]（sc-properties）：区域 ↔ 城市组 ↔ 地块表 三方配对（显式引用链），并识别 5城+1伟工 的白水谷。仅开发用。
- [`region_bounds_probe`]（sc-properties）：Verify region-visible-area crops against every shipped RT0 region and plot.
- [`region_color`]（sc-properties）：区域彩色合成渲染：F0 高度 + ED 场上色（水/森林/材质）+ 资源画刷叠图。仅开发用。
- [`region_json_scan`]（sc-properties）：探针：全包定位 AutomatedRegionTemplates.json / AutomatedResources.json 资源
- [`region_list_probe`]（sc-properties）：探针：region_list_named 真名链验证。仅开发用。
- [`region_name_bridge`]（sc-properties）：探针：区域名称桥全链路。
- [`region_preview`]（sc-properties）：区域地形俯视预览渲染：金字塔匹配 → 4096² 拼合 → hillshade PNG。仅开发用。
- [`region_probe`]（sc-properties）：地图机制调查探针（只读）：
- [`region_ref_scan`]（sc-properties）：全包扫描引用指定区域 TGI 的属性（Key 的 type/group/instance 任一匹配）。仅开发用。
- [`region_state_maps`]（sc-properties）：探针：区域存档状态文件按 32776 B 步长切 128² u16 地图，渲染非零地图。仅开发用。
- [`region_state_probe`]（sc-properties）：Decode an already gunzipped retail state; outputs JSON to an explicit path.
- [`region_template_bind`]（sc-properties）：探针：区域模板 JSON（EP1 包 4529F96F）↔ RT0 地块表配对验证 + 伟工位输出。仅开发用。
- [`region_terrain_survey`]（sc-properties）：探针：RegionTerrain0 包 B12DE348（ED 金字塔求解失败）/ D01FA985（渲染异常）诊断。
- [`state_unit_resource_probe`]（sc-properties）：Match unit entry IDs from a decompressed region state to static property resources.
- [`strata_env_dump`]（sc-properties）：探针：dump 层栈模板（Game 包 D7EF2862 族）+ 区域 desc 全文 + 环境属性表。仅开发用。
- [`table_dump`]（sc-properties）：全量 dump 指定区域的地块表（自动配对）。仅开发用。
- [`target1_add_registry_overlay`]（sc-properties）：Add the Game registry JS replacement to target1.
- [`target1_add_template_overlay`]（sc-properties）：Add the authoritative EP1 Cape Trinity template replacement to target1.
- [`target1_build_package`]（sc-properties）：Build the first real target1 Cape Trinity overlay from retail resources.
- [`target1_registry`]（sc-properties）：Target1: dump Cape Trinity registry ids and locale resource contents.
- [`target1_template_probe`]（sc-properties）：Dump the Cape Trinity EP1 template resource TGI and map.cities entries.
- [`target1_verify`]（sc-properties）：Verify target1 overlay by reopening it and comparing expected semantic diffs.
- [`tile_anchor`]（sc-properties）：城市地块高度图 ↔ 区域背景 tile 锚定探针。仅开发用。
- [`tile_arrange`]（sc-properties）：tile 金字塔完整还原：child→(parent,quadrant) 图 → 树 → 16×16 mip0 排布。仅开发用。
- [`tile_diag`]（sc-properties）：诊断：区域内每 tile 最优父的误差分布。仅开发用。
- [`tile_grid`]（sc-properties）：按"包索引序 × 指定列宽"把区域高度图 tile 渲染成大图（BMP 24bit）。
- [`tile_grid_vote`]（sc-properties）：跨区域投票定稿 tile 排布：341 槽位全区域共享 ⇒ 排布唯一，多区域投票消歧。仅开发用。
- [`tile_orient`]（sc-properties）：拼合 16×16 mosaic，用城市地块（已知世界坐标）窗口比对：验证模型 + 定向。仅开发用。
- [`tile_pyramid`]（sc-properties）：tile 金字塔匹配探针：验证 341 = mip0(256)+mip1(64)+mip2(16)+mip3(4)+mip4(1)。
- [`z_diff_probe`]（sc-properties）：探针：BC357A2B 逐城场地 z vs 三张 16 城模板 z 的配对差。仅开发用。

## road —— 道路 / 路径 / curve

- [`path_prop_scan`]（sc-properties）：Census static property resources carrying starter path arrays.
- [`path_reconstruct_probe`]（sc-properties）：Reconstruct static road path endpoints from a region unit transform.
- [`probe_lane`]（sc-exporter）：一次性探针：从 SimCity_Graphics.package 解出指定 instance 的 raster，
- [`road_render_probe`]（sc-properties）：Read-only census of road extrusion links in the installed Game package.
- [`static_path_match_probe`]（sc-properties）：Match EGB curve entry IDs against every static Unit path resource.
- [`unit_path_dump`]（sc-properties）：

## terrain —— 地形 / 水位 / 画刷 / 地面纹理

- [`bay_probe`]（sc-properties）：采样火山群岛"半圆海湾"区域的高度分布，确定水位下限。仅开发用。
- [`city_map_histo`]（sc-properties）：探针：每城市组的 256² 高度图直方图——海洋格恒定值 = 引擎水位。仅开发用。
- [`city_prop_dump`]（sc-properties）：探针：转储全部城市组 36B property（0x1B53D525 int32 + 0xC1949C4D 区域回指）的 int 值。
- [`city_uid_prop`]（sc-properties）：探针：城市组 36B property 的 0x1B53D525 是否= uid。仅开发用。
- [`eco_brush_full`]（sc-properties）：探针：转储 coalEcoMapBrushes 全部键值。仅开发用。
- [`egb_scan`]（sc-properties）：探针：在 EcoGame 存档快照（解压后）中找 u16 常量长跑——水位面候选值。
- [`egb_water_hunt`]（sc-properties）：探针：城市 egb 中找非零 u16 常量跑（水位候选）+ 地图块结构盘点。仅开发用。
- [`find_terrain_ps`]（sc-exporter）：全包扫描：解压后含 groundColors+controlMap 的资源（terrain PS 容器）。
- [`find_watertable_res`]（sc-properties）：探针：在全部游戏包中按 instance 定位 watertableheightmap 引用的资源并解码。仅开发用。
- [`ground_tex_extract`]（sc-properties）：探针：定位引擎地面 cube 细节纹理（实际为 type ED + instance=FNV1_lower(名)、group=0）。
- [`gtex_probe2`]（sc-properties）：探针：GraphicsCache/Cache 条目数 + SimCity.par 是否 DBPF + 全包扫 0x03E421ED group 段。仅开发用。
- [`height_hist`]（sc-properties）：区域高度直方图（排除全零 tile），找水位面候选。仅开发用。
- [`map_layer_assets`]（sc-properties）：Inspect named map assets and optionally export the native shoreline foam.
- [`map_native_assets`]（sc-properties）：Export private local fixtures; game assets and saves must not be committed.
- [`map_unit_audit`]（sc-properties）：Read-only audit of regional unit definitions and bridge models.
- [`normal_atlas_probe`]（sc-exporter）：法线图案图集（0x60E7805D）边缘列/行解剖 —— 排查 PE 精细地面
- [`resource_brush_probe`]（sc-properties）：
- [`script_water_scan`]（sc-properties）：探针：全包扫描文本资源（JS 脚本等）中的 waterLevel / seaLevel / water level 引用。
- [`stamp_hunt`]（sc-properties）：探针：全类型、全包按 instance 搜 eco/terrain 画刷 stamp 资源。仅开发用。
- [`subresource_dump`]（sc-properties）：探针：区域子资源全清单——名称、画刷 Key、变换、目标 map、分辨率。
- [`terrain_color_probe`]（sc-properties）：Export real region fields for the terrain-color regression/visual harness.
- [`verify_static_maps`]（sc-properties）：Opt-in validation against an installed game's packages; no game data is committed.
- [`watertable_probe`]（sc-properties）：探针：定位并解码区域 waterTableEcoMapBrushes 引用的水位面高度图。

## model —— RW4 模型 / 材质 / 栅格 / 着色器

- [`building_material_probe`]（sc-exporter）：探针：对一栋建筑的材质链逐槽导出，并按 building4 公式做**平面（UV 域）合成预览**。
- [`building_vertex_cols`]（sc-exporter）：探针：逐三角形导出建筑网格的位置/UV/调色板列号（D3DCOLOR.G），
- [`dump_facade_glbs`]（sc-exporter）：手动取证（GLB Worker 贴图乱码，2026-10-08）：扫描包内带 facade 世界投影
- [`dump_model_glb`]（sc-exporter）：冒烟工具：导出模型 RW4 的全部可导出 mesh 为 GLB。
- [`dump_slot_texture`]（sc-exporter）：冒烟工具：解出指定 RW4 纹理包裹（2F4E681B）的顶层 mip 存 PNG。
- [`facade_probe`]（rw4）：冒烟探针：facade 世界投影 UV 数学取证（阶段 4）。
- [`facade_survey`]（rw4）：立面窗特征普查（只读）：遍历一个包内全部 RW4 模型容器，对每栋建筑按
- [`facade_uv_probe`]（sc-exporter）：只读取证：facade mesh 的 TEXCOORD 结构直方图。
- [`find_foliage_textures`]（sc-exporter）：冒烟工具：扫描包内绿色主导的纹理（树/植被图集特征）。
- [`find_tree_models`]（sc-exporter）：冒烟工具：按 BBox 形状找树模型（高/宽 > 1.3 且高 3-20m）。
- [`foliage_ref_scan`]（sc-exporter）：冒烟工具：全包扫 2F4E681B 模型的材质纹理引用，找引用 foliage 图集族的模型
- [`glb_attr_check`]（sc-exporter）：冒烟工具：导出一个模型的 GLB 并检查顶点属性（TANGENT 是否写入、NORMAL 是否非零）。
- [`hlsl_dump`]（sc-properties）：探针：转储 shader 源码容器（typeId 0x0469A3F7，ArgScript + HLSL 文本）。
- [`impostor_config_scan`]（sc-exporter）：取证探针：impostor class 配置表定位。
- [`material_bind_probe`]（rw4）：冒烟探针：验证 RW4 模型 mesh↔material↔texture 绑定链（仅开发用）。
- [`material_info_dump`]（sc-exporter）：MaterialInfo（材质信息表）解析——exe `SP::cMaterialManager::ReadMaterials` 的逆向对拍。
- [`model_peek`]（rw4）：单模型快速盘点：mesh 数/顶点/三角/属性/材质节。仅开发用。
- [`palette_probe`]（rw4）：冒烟探针：统计 slot0 调色板的尺寸分布与行间差异（阶段 1 校准前置取证）。
- [`raster_bytes`]（rw4）：字节序取证：打印 raster 顶层 mip 每个字节位置(0..3)的统计与前 8 像素。仅开发用。
- [`raster_peek`]（rw4）：单光栅尺寸。仅开发用。
- [`raster_probe`]（rw4）：冒烟探针：扫描包内 raster(0x2F4E681C) 资源，打印解析与解码结果。仅开发用。
- [`raster_views`]（sc-exporter）：冒烟工具：把某个 Raster 的全部预览视图各导出一张 PNG，用于目视比对
- [`shader_container_extract`]（sc-exporter）：定位并全量提取 shader 源码容器 0x0469A3F7（ArgScript HLSL 片段库）。
- [`shader_def_scan`]（rw4）：冒烟探针：收集材质 slot 0x2D（shader-def）引用并跨包定位 + 字节取证（阶段 4）。
- [`shader_grep`]（sc-exporter）：冒烟工具：在 shader 源（0x0469A3F7）里按关键词检索，打印命中片段。
- [`shader_map_stats`]（rw4）：slot1/2/3/palette 通道统计探针（5a 调试，只读）：
- [`shader_name_hash`]（rw4）：冒烟探针：从 cpp(0x0469a3f7) shader 容器抽名字，穷举哈希算法匹配 0x2D 引用值。
- [`shader_sweep_probe`]（rw4）：sweep 探针：全包统计 uvKind 分布 + 前端常数采样（TEXCOORD_0=0,0）整 mesh discard 预测。
- [`shader_verify_probe`]（rw4）：验证探针（2026-09-08 四症状分析取证，只读不改）：
- [`simm_material_probe`]（sc-exporter）：冒烟工具：sim 部件模型的材质段结构（Decoded/Raw、纹理引用、文件纹理清单），
- [`simm_model_probe`]（sc-exporter）：冒烟工具：GI_Simm_*（市民/顾问小人）instance 在游戏包中的资源形态定性。
- [`slot_dump`]（sc-exporter）：冒烟工具：导出指定模型材质的 slot1/3/5 贴图为 PNG（人工目检语义，阶段 4）。
- [`tangent_scan`]（rw4）：顶点格式普查：模型是否携带 TANGENT/BINORMAL 顶点流（法线贴图 TBN 的来源）。
- [`tint_underface_probe`]（rw4）：下向面（地板/底面）tint alpha 取证探针（2026-09-08，只读不改）：
- [`tree_model_check`]（sc-exporter）：冒烟工具：树模型候选（find_tree_models 同形状过滤）的纹理指纹定性——
- [`tree_model_dump`]（sc-exporter）：冒烟工具：树 impostor 模型定性——section 清单 + 内嵌纹理落盘 + GLB 导出。
- [`uv_model_scan`]（rw4）：冒烟探针：扫描「可贴图 + 带调色板材质」的建筑模型清单（阶段 3.1 测试样本）。
- [`vehicle_3d_probe`]（sc-properties）：探针：载具卡 3D 链路真实验证。

## ui —— 游戏 UI 脚本 / 布局 / JS

- [`js_dump`]（sc-properties）：探针：转储 Game 包主 JS 脚本（type 67771F5C）供检索。仅开发用。
- [`script_resources_probe`]（sc-exporter）：P1 取证探针：EcoGame 脚本资源表解析（dev-dump 文档 §3 配方的离线验证）。
- [`ui_assets`]（dbpf）：游戏 UI 资产盘点：扫描 SimCityData 全部 package，统计 CSS / 布局 JSON /
- [`ui_dump_tgi`]（dbpf）：探针：按完整 TGI 导出一个条目。用法: ui_dump_tgi <out> <type> <group> <instance> <package>
- [`ui_js_census`]（dbpf）：探针 v2：导出 group 0x40464200 全部 JS 模块 + 松散关键词统计。
- [`ui_js_dump`]（dbpf）：探针：按 FNV-1(小写去扩展名) = instance 的约定，从 UI 包里导出布局 JS 模块。
- [`ui_layout_census`]（dbpf）：探针：普查全部布局模块（type 0x67771F5C, group 0x0B074E5A），打印
- [`ui_observer_package`]（dbpf）：Read/extract/build the single scrui resource; never modifies an input package.

## effect —— Swarm / 特效

- [`effect_collection_dump`]（sc-properties）：手动取证：把 Swarm 效果集合资源原样落盘（供字符串/结构分析）。
- [`effect_collections`]（sc-properties）：手动取证（effect Swarm 预览）：扫描包内 Swarm 效果集合资源
- [`effect_component_stats`]（sc-properties）：Summarise Swarm component usage for render-preview comparison.
- [`effect_resource_find`]（sc-properties）：手动取证（effect Swarm 预览）：按 instance 反查 effect 资源实体——
- [`effect_units`]（sc-properties）：手动取证（effect Swarm 预览前期调查，2026-10-08）：扫描包内 Lot 属性的

## zone —— 分区颜色与图标

- [`zone_color_dump`]（sc-exporter）：精确提取 EP1 的 3 个 0x00b1b104 属性资源（instance 低 16 位命中 zone id）
- [`zone_color_dump2`]（sc-exporter）：dump 三个 EP1 属性资源全文（hex+可读），人工判读属性布局。
- [`zone_icon_probe`]（sc-properties）：探针：字节等价搜索 —— 在多个包里找与目标 PNG 完全相同的资源，定位其来源 TGI。仅开发用。
- [`zone_ids_probe`]（sc-exporter）：zone 资源 id 反查：BlendColors 活体提取的 16 位 id（zone 资源 instance

## registry —— sc-registry 注册表查询

- [`lookup`]（sc-registry）：instance/property 查名。仅开发用。
- [`name_lookup`]（sc-registry）：冒烟工具：从 s3db 注册表按 instance 查资产名。
- [`type_lookup`]（sc-registry）：type/group 查名。仅开发用。

## runtime —— 游戏运行时注入 / 日志（python）

- [`package_loader_probe`]（package_loader_probe.py）：复现 SimCity 包加载器的静态证据（只读 PE，--disassemble 需 capstone）。
- [`rule_logging_probe`]（rule_logging_probe.py）：从零售 PE 复现有界 rule-logging 证据（纯静态读取）。
