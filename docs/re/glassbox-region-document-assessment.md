# GlassBox 与 Region 文档对 OpenSCP 的补充评估

日期：2026-10-17

本报告只记录两篇附件的机制说明，并与现有逆向结果和代码证据对照。附件中的制作经验不等于 OpenSCP 已经具备对应的写回能力；日志追查仍暂缓。

## 两篇文档分别补足了什么

`glassbox-info.pdf` 解释了游戏运行时如何组织模拟对象：资源（resources）、单元（units）、地图资源场（maps）、全局资源（globals）和规则（rules）共同组成 GlassBox；unit 带有本地 resource bins、simulation footprint 和 rules，agent 携带资源沿 path 移动，box 负责脚本、Property、unit/map 类型、globals、状态和位置，region 通过 boxes 传递 package。交通部分进一步说明 path graph、segment 权重和拥堵值由运行时路由计算产生，图形部分说明 façade/interior/impostor/texture/SDF 等资源由 graphics handler 消费。

`simcity_region.pdf` 是区域设计师的过程记录，而非区域文件格式逆向。它确认了几个实际创作输入：区域构思阶段先做可丢弃的俯视 mock-up；地形从 Photoshop 生成的 512×512 黑白 height map 开始，再经过侵蚀滤镜和手工调整；地块位置、地形障碍、道路和水域被设计成玩家可选择的城市画布；区域主题还会改变纹理、水量和环境挑战。文档中的污染、辐射、水资源、城市间资金/研究合作，是游戏模拟结果和内容设计，不是一个单独可编辑的“区域图片”。

## 与现有逆向结果已经对上的部分

### GlassBox 机制

OpenSCP 的 Property 语义层已经按 GlassBox 资源家族区分了 Unit、Agent、Resource、Network、Path、MapLayer、Menu、TerrainBrushTemplate 和 EnvironmentLink。`crates/sc-properties/src/semantic.rs` 中，`heightmap` 名称加分辨率被作为地形画笔/地图模板结构判据，`environment` 开关加 REGION Key 被作为城市环境绑定判据。这与 GlassBox 的 maps、units、globals、boxes 以及 region 传递模型相互吻合。

现有 Menu/Menu2、Lot/Unit、LOD、Raster、路径和地图图层分析也说明我们已经把“Property 是资源关系入口”这一点摸清了。原始 C# 中的 Parent 展平、Unit/LOD 依赖复制与 GlassBox 的 unit/box 资源组织可以互相解释；它们不是互不关联的 UI 字段。

### Region 机制

区域文档对现有实现的直接证据主要是读取侧：OpenSCP 已能识别 `0xE000` Map Layer 语义、地形画笔模板的 heightmap/resolution 特征，以及 environment→REGION 的 Key 关系。也就是说，我们可以把区域相关 Property 作为“资源索引和关系图”展示出来，而不是把它们误当成普通建筑 Property。

但文档没有提供以下可写回所需的证据：区域 DBPF 中各资源的完整 TGI 清单、height/terrain/water/biome 图层的编码格式、图层与城市地块的精确坐标映射、region save JSON 与 DBPF 的边界、区域包生成器的校验规则。因此不能据此声称 OpenSCP 已能创建或修改真实区域。

## 当前 OpenSCP 的能力边界

| 机制 | 已有证据 | 当前结论 |
| --- | --- | --- |
| GlassBox Property/Parent/资源关系读取 | `sc-properties`、语义判据、Property 预览 | 已关联，可继续用于索引和对拍 |
| Menu/Menu2、Lot、Unit、LOD 关系 | 既有三份文档与原始 C# 对拍 | 可先做受约束 Property overlay |
| Map Layer、Terrain Brush、Environment Link 识别 | `semantic.rs` 的结构判定 | 读取和分类可用，写回未证实 |
| 高度图/区域地形可视化 | 区域文档中的 512×512 height map 过程 | 可做外部图像预览或实验输入，不能直接生成游戏区域包 |
| 纹理、水量、污染、辐射等环境图层 | 文档只展示设计结果和游戏内 map | 机制可解释，格式和写回边界未知 |
| path graph、agent 路由、拥堵和跨城市 package | GlassBox 运行时说明 | 属于游戏运行时，不应伪装成静态 Property 编辑功能 |
| 区域创建/修改 | 文档明确写到当时不能由社区直接创建或修改区域 | 暂不纳入当前集成里程碑 |

## 对菜单/Lot 路线的影响

两篇文档没有推翻先前路线，反而说明为什么要先做小而真实的资源写回。推荐路线保持为：

1. 菜单 `Parent Menu` 单字段 overlay；
2. Property-only Lot 标量（Lot Size、颜色、单个 light 参数）；
3. 单个 prop/light 数组成员；
4. Lot Raster 替换并验证引用；
5. Menu2 复制及 icon/marquee/unit/LOD 依赖复制；
6. 最后才评估区域图层和 RW4 编译/写回。

GlassBox 文档还要求我们在后端数据模型中区分三种东西：静态资源描述（Property/Map Layer）、运行时状态（resources/globals/agents/拥堵/污染结果）和区域容器/存档。UI 可以把它们关联展示，但不能因为能显示某个 map 或 Property 就提供同名的写回按钮。

## 可以开始的最小验证

可以立即开始固定样本的菜单 Parent overlay 和 Property-only Lot overlay。两者都有原始 SCP 操作、C# 写法和 OpenSCP parser/writer 证据支撑；写出后可重读、逐 hash 对拍，并进入游戏验收。

区域方面目前只建议做只读的“区域关系检查器”：列出 MapLayer、TerrainBrushTemplate、EnvironmentLink 资源及其 Key/TGI，标记未知的 height/water/biome 依赖，并禁止导出为声称可用的 region mod。要进入区域写回，必须先获得真实区域样本、完整资源清单和至少一次游戏内对拍。

## 证据

- 用户材料：`C:\Users\FluffyCHi_Xing\Desktop\glassbox-info.pdf`，GlassBox 的 resources/units/maps/globals/rules、boxes/regions、graphics 和 traffic 章节。
- 用户材料：`C:\Users\FluffyCHi_Xing\Desktop\simcity_region.pdf`，PDF 页 119–129；其中页 120–121 描述 512×512 height map 和侵蚀/手工调整，页 122–123 描述城市地块和地形障碍，页 124–129 描述新区域主题、纹理、水量、污染/辐射和跨城市挑战。
- OpenSCP：`crates/sc-properties/src/semantic.rs` 的 TerrainBrushTemplate、EnvironmentLink、MapLayer 判据。
- 既有对照：`docs/re/scp-verified-operation-integration-assessment.md`、`docs/re/simcity-modding-document-assessment.md`。
