# 全地图可游玩模组调研：运行时地形/资源修改的可行性与技术路线

> 2026-09-25 只读调查，未改任何代码。起因：Reddit 帖
> ["13 Years Later SimCity 2013 Can Run Fully Playable 4096×4096 Cities"](https://www.reddit.com/r/SimCity/comments/1w3mrkx/13_years_later_simcity_2013_can_run_fully/)
> （u/silkiokas，2026-08-31，r/SimCity）——作者在游戏内加了 debug 菜单、地形笔刷
> 与动态资源添加。本文回答"这可能吗"，并给出 OpenSCP 侧的全图模组技术方案。
> 引擎结论引自本地逆向文档 `docs/overview/glass-box/`（terrain/region-and-map/
> eco-swarm/er2-rules）与 `docs/overview/saves-exploration.md`、
> `analyze/01-game-architecture.md`。

---

## 1. 结论速览

**可能，而且是引擎原生能力，不是黑魔法。**"地图信息只在首次进入游戏时加载"是误解：
进图时加载进内存的是**活数据**，GlassBox 的模拟在游玩全程持续改写它们；游戏自带
的建城地形工具、debug UI、以及 EcoMap 画刷机制，全都是"运行时修改地形/资源"的
官方实现。Reddit 作者做的事情 = 把这些引擎内建机制通过 debug 菜单暴露出来 +
扩展引擎常量。

| 子问题 | 答案 | 依据 |
|---|---|---|
| 游戏中能否实时改地形？ | ✅ 引擎原生支持（effect 消息 → 服务器 cTerrainGame 落盘内存） | terrain.md:92-95 |
| 能否动态添加资源？ | ✅ EcoMap 画刷机制，清单 property 可增删 | region-and-map.md:248-287 |
| debug 菜单如何实现？ | UI 菜单分类是 property 门控，overlay 包即可重开（本项目已有探针验证） | `sc-exporter/examples/enable_debug_tools.rs:1-30` |
| 城市能突破 2048×2048m 吗？ | ✅ 但要改引擎核心（silkiokas 已实证 4096²），且性能坑明确（单核模拟、agent 路径扫描） | 帖子评论 + region-and-map.md:374-376 |
| OpenSCP 走哪条路？ | 数据工程为主（不改 exe）：三件套 + 地图生成器；游戏内解锁为辅 | 本文 §4 |

## 2. Reddit 案例还原（silkiokas，2026-08）

帖主在 Mac App Store（Aspyr）版上完成，计划发布 GitHub 安装器（"modifies many
core systems for the larger maps"）与自定义区域包：

1. **4096×4096m 城市完全可玩**（默认城市箱 2048×2048m，面积 4 倍）；修复了 Aspyr
   版分辨率问题、被毁/弃建筑黑块 bug，重开了 OpenGL 加速。
2. **游戏内区域编辑器**（早期版）：可导入自己的高度图、把更大的城市地块放置在
   区域任意位置——与 OpenSCP 地图工作台的"新地图创建"目标直接同源。
3. **模拟优化在研**：1 个 agent 代表 10 名工人/购物者/水量，削减 GlassBox 实体数。
4. **评论区的 Maxis 原开发者 MaxisGuillaume 亲述内部教训**（重要性能情报）：
   - agent "pipe" requester 按 2K×2K 城市调参，4K 需提速 requester，但低帧率时
     会跳过 sink；
   - 车辆速度按小城调校，4K 城通勤时长会超出游戏允许；
   - **主模拟严格单线程有序**（渲染/音频并发），4096 = 路径扫描 ×4 尚可，
     8192 = ×16 模拟扛不住——瓶颈是路径扫描次数而非人口；
   - 另一评论者指出区域内城市摆放复用既有城市槽位模板（实测样本复用 Cape
     Trinity 的槽位）。
5. 未完成：区域选择菜单的"3D 渲染风"地图图像尚未逆向。

历史先例（2013 年代）：SimCity Offline Mod 的 debug UI 即可"出城建造 + 地形工具"
（[GameFAQs 讨论](https://gamefaqs.gamespot.com/boards/958716-simcity/65926166)、
[YouTube 演示](https://www.youtube.com/watch?v=PwMt-kCdf4E)）；debug 菜单在
sandbox 模式可开（[IGN wiki](https://www.ign.com/wikis/simcity/Cheats_and_Secrets)）。

## 3. 机制解答：为什么"运行时修改"是引擎原生能力

GlassBox 是**客户端-服务端架构**：模拟在服务器侧（单机模式为本地进程），渲染在
客户端，两侧按 E/G 边界切开（`analyze/01-game-architecture.md:220-231`）。
"修改地图"因此有三条引擎内建通道：

1. **地形 = effect 消息机制，非静态资产**。`cTerrainLayer::DrawLayer` 按 32 位
   hash 分派 14+ 种 effect 消息（成对 hash 疑为 do/undo），编辑器把操作写入
   effect map，DrawLayer 变成绘制调用，最终落点是**服务器侧 cTerrainGame**；
   高度修改后经三条属性管线通知法线/区域/info 重算
   （`glass-box/terrain.md:92-95,126`）。城市地形 256×256 格、格边长 8m
   （terrain.md:28,53；region-and-map.md §4.2）。**"笔刷创建新陆地"就是这个
   机制**——工具（cTool 子类）经 hash 消息总线发布模拟变更，参数来自 RCD 数据
   块（`glassbox-subsystems-deep.md:158-161`，注意：无撤销/重做）。
2. **资源 = EcoMap 画刷**。资源分布由画刷清单 property 驱动：清单名 `0x00B2CCCA`
   （`*EcoMapBrushes`）+ `0x02A907B6` Transform 数组（世界摆放 + 强度），目标
   map 索引 soil=1/watertable=2/forest=3/oil=4/coal=5/ore=6/radiation=10/
   groundPollution=11/desirability=8,9（`region-and-map.md:248-287`）。stamp
   位图**不在包内**，由引擎按区域种子程序化生成——所以"动态添加资源"既可以在
   运行时加画刷（游戏内路线），也可以在数据侧往清单 property 追加条目
   （OpenSCP 路线，`encode_canonical` 写出能力已具备）。
3. **debug 菜单 = property 门控的 UI 可见性**。本项目探针已实证：把 3 个 debug
   工具的菜单分类键 `0x0975695E=0x3D752D6A` 与 UI 子分类键 `0x0DB9FC63=
   0xA2A33338` 重指到可见分类，生成未压缩 overlay 放入 `SimCityUserData/Packages/`
   即可重开（`sc-exporter/examples/enable_debug_tools.rs:1-30`，带阳性对照实验
   设计）。silkiokas 的 debug 菜单属于同类思路（或叠加二进制补丁）。

另两个关键事实：

- **没有 eco 专属文件格式**：eco 场是运行时对象，package 侧只有 property/规则表；
  编辑安全入口 = property 与 beat 规则掩码位（`glass-box/eco-swarm.md:246-250`）。
- **存档 = 规则库的状态投影**：`.egb` 是 gzip 的 EcoGame 状态快照，魔数即主规则库
  instance `622B9CD7`（`saves-exploration.md:40-44`）——运行时改的东西最终以状态
  表形式存档，二者是同一套数据。

## 4. OpenSCP 的两条路线

### 4.1 路线 A：游戏内运行时修改（silkiokas 路线）

改 exe/注入 DLL + debug 菜单 + 调 DrawLayer。能力上限最高（4096 城已实证），但
需要深度二进制逆向、维护成本高（随游戏版本/平台漂移）、性能坑明确（§2.4）。
OpenSCP 是 Rust 桌面工具，不直接走这条；但其**数据侧成果可直接喂给路线 A**：
silkiokas 计划"individual packages for custom regions"——自定义区域包正是
OpenSCP 擅长产出的东西。

### 4.2 路线 B：数据工程三件套（OpenSCP 主路线，不改 exe）

BoC（出界游玩）取证已给出界外缺陷三根源与对策
（`region-and-map.md:424-440`）：

| 根源 | 症状 | 对策 |
|---|---|---|
| 界外 eco map 恒 0 | 区域外资源数据为空 | 补画界外画刷：往 `*EcoMapBrushes` 清单 property 追加 stamp 条目 |
| 格网定义域外漂移 | 建筑摆放偏移 | 地块表扩展 + 城市域隐式定义的对齐（城市范围 = 256×256 高度图域的隐式定义，region-and-map.md:128-132） |
| 区域 tile 与城市高度场两种高度源不一致 | 道路/地形接缝错位 | 高度源对齐：区域 4096² 金字塔与城市 256² 场从同一源生成 |

加上**地块表扩展**即三件套：
1. 界外 eco map 补画（画刷清单 property 追加，`encode_canonical` 已可写出）；
2. 高度源对齐（区域 tile 重生成，`tile_arrange` 已验证可逆，
   region-and-map.md:590-592）；
3. 地块表扩展（property instance `0x2B9C480C`：城市 id `0x16B7B1EF`、位置
   `0xF01DE4B1`、模板 Key `0x9F2F9B65`；"只保留一个可选地块 + 区域外映射"即
   编辑此表，`region-and-map.rs:211-246` 已有读取）。

**"只保留一个可选区域、区域外映射进当前城市坐标"的落地含义**：地块表删到只剩
1 个入口城市（或全部保留但只开放 1 个），城市 2048m 域之外的世界由区域背景
（4096²@8m 高度场 + ED 场 + 画刷资源）承担，坐标换算
`tile = (regionExtent/2 + world) × 1/2048`（Ghidra FUN_00beb730，
region-and-map.md:225-235）保证城市与背景共享寻址。
**注意硬上限**：不改引擎时城市箱仍是 2048×2048m（UI JS 自证
"city box is limited to 2048x2048"，region-and-map.md:374-376）；4096 城市属于
路线 A。

### 4.3 地图生成器（需求：柏林噪声新地图）

- **尺度口径修正（出处已定位）**：需求稿写"1px = 0.75m"，该值实为 **Lot raster 的
  官方惯例**（米/像素 = LotSize ÷ mask 宽高，官方主流 0.75、少数 0.625/1.5，
  `docs/design/raster-painter.md:69`；停车标线实测同证 `tools.ts:219`），被需求稿
  误带入地图语境。地图域的真相是 **8m/格**：城市地形 256×256 格覆盖 2048m
  （terrain.md:28,53），区域高度图 4096²@8m 全幅（`region_map.rs:465` 恒定
  `meters_per_pixel=8.0`）。地图面板代码无 0.75 残留（MapViewer MPP=8、标尺
  梯子 8–4096 系，git 历史 `-S "0.75"` 零命中），**无需修代码，只需修口径**。
- **生成管线**（全部缺失，无 Perlin 实现，image crate 已就绪
  `Cargo.toml:22`）：Perlin/fBm → 4096² f32 高度场 → u16 量化
  （`raw = (z + 1024) × 32`，z=raw/32−1024，region-and-map.md:109-112）→
  金字塔重排（5 级 mip：16²+8²+4²+2²+1 tile）→ F0 tile 回写 → overlay 包
  （`write_uncompressed_overlay` 已具备，`writer.rs:54-125`）。
- **灰度图切割算法** = 上述金字塔重排的逆过程库化（tile_arrange 探针已验证
  可逆性）；外部灰度 PNG 导入同管线入口（水位用区域 desc `0x51E7A18D` 的
  `0x0E16BE1A` 覆盖，region_map.rs:654-666）。
- **资源绘制笔刷** = 画刷清单 property 的编辑器（读取已有
  `resource_brushes`，写出走 `encode_canonical`）。
- **区域道路绘制** = ED 场 b0 通道（植被/路网）编辑：ED tile 128×128 u32、
  字节语义 b0=路网（`region_map.rs:643-650`；模块文档 region_map.rs:8-9）。

## 5. 风险与开放问题

1. **性能天花板**：路线 B 不改 exe，默认城市箱 2048m；4K 城市必须路线 A，且
   Maxis 开发者亲述的 requester/通勤/单核三坑会复现（§2.4）。
2. **32 位进程**：主程序 32 位，超大区域数据包（RT0 504MB F0）在工具侧无碍，
   在游戏侧加载内存需验证。
3. **无撤销**：引擎 cTool 管线无 undo（glassbox-subsystems-deep.md:158-161），
   游戏内笔刷误操作依赖 OpenSCP 侧 changeset 版本线兜底（存档层面）。
4. **silkiokas 尚未开源**：其引擎常量改动清单（4096 城市箱、agent 1:10）待
   GitHub 发布后对照，届时可校准"路线 A 所需补丁面"评估。
5. ~~1px=0.75m 出处待证~~ 已解决：出处为 Lot raster 官方惯例
   （raster-painter.md:69），系需求稿误带入地图语境；地图域以 8m/格为准。
6. **`.egb` 状态表未解析**：运行时修改后的存档级校验（改钱/改资源）依赖
   AEB→状态表逆向（er2-rules.md:215-224 已排期）。

## 6. 里程碑建议

| 阶段 | 内容 | 依赖 |
|---|---|---|
| FM0 | debug 工具 overlay 包产品化（enable_debug_tools 探针 → 一键命令） | 已验证 |
| FM1 | 高度图 PNG 导入 → F0 tile 回写 → overlay 包（灰度切割库化） | map-workbench MW1 |
| FM2 | 地块表编辑器（单可选地块 + 区域外映射）+ 画刷清单编辑器 | FM1 |
| FM3 | 柏林噪声地图生成器（fBm → 4096² → 金字塔 → 打包） | FM1 |
| FM4 | 游戏内重建验证：出界行走、区域外资源、道路接缝、建筑无偏移 | FM0-FM3 |
| （远） | 路线 A 对接：silkiokas 开源后评估 4096 城市箱补丁的兼容 | 外部 |
