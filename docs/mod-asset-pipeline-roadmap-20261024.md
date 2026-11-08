# Property Editor 与 Mod Studio 资产闭环计划

日期：2026-11-08（现状复核；原计划日期保留在文件名中）

本文记录从 Blender 烘焙模型到游戏内建筑验证的目标流程，以及当前实现边界和后续任务。计划以一个可重复构建的建筑样本为主线，逐步扩展到 decal、特殊建筑和 ER2。

## 目标链路

Blender 烘焙模型 → Mod Studio Assets 项目 → 模型 / 材质 / lot / decal / 附属单元 → Property Editor → openscp.lot-asset/1 → 资源编译、Property/Unit 写回 → DBPF package → Menu2 / Unit / EcoGame / ER2 → 游戏启动、放置、模拟和回滚验证

## 当前已具备

- .package、.lotm、.glb 导入和 LOTM/GLB 预览。
- building4 六槽材质输入：参数表、tint、normal/AO、shader map、palette、interior。
- Property Editor 的 lot、light、effect、decal、prop、pathPoint、spawner Schema 单元。
- lot mask、lot texture、decal profile 的 Raster Editor。
- 原生 package 的 TGI 保留、DBPF overlay 输出、重新打开和逐资源比较。
- Menu2 菜单移动、复制、分组、locale、图标、rollover 和 overlay 导出。
- Menu2 → Unit → bundle → resource → ER2 的静态研究基础。
- 小学、水泵等样本的规则资源和运行时 delta 采集原型。
- LOTM/GLB → 原生 RW4 的 M1 编译路径、LOD/包围盒/材质绑定和确定性 DBPF 输出。
- 新菜单身份注册、locale 按身份隔离、行为绑定和 full 输出的资源闭包。
- `layout-contract`、`menu-contract`、`delta-audit`、`material-binding-contract` 和
  参照 lot 主体契约；构建器当前处理器版本为 `openscp.property.m1/39`。

## P0：第一个可玩的建筑闭环

### Blender 导入契约

- 固定米制、Z-up、lot 中心原点、街道方向和朝向。
- 固定 LOD、UV、法线、切线、镜像绕序和三角形预算。
- 固定 building4 六槽烘焙规范和 shader family。
- 在 Schema 中记录源文件、导出工具版本、坐标系和单位。

验收：同一 Blender 文件重复导出后，尺寸、原点、朝向、LOD、材质索引和 Schema 结果一致。

### 模型编译

- 实现受支持的 GLB/LOTM → RW4/LOTM 写出。
- 写出顶点、索引、法线、切线、UV、LOD、包围盒和材质绑定。
- 分配新 TGI，保留未编辑资源，检查三角形和纹理预算。

验收：OpenSCP 可重新读取编译结果，游戏可加载并显示模型。

### Property/lot 写回

- 实现 Property 字段类型反编码和继承合并。
- 写回 lot 尺寸、颜色、边框、mask、地面纹理、道路入口、位置表和附属单元。
- 让 lot Raster Editor 与 Property Editor 共享同一份文档。
- 编辑失败时阻止生成可安装包。

验收：修改位置、light、lot、prop 后，重新读取字段和数组与编辑结果一致，未编辑字段保持不变。

### Decal 开发能力

- 增加 decal Schema 和项目节点。
- 支持投影涂鸦、霓虹、interior map、SDF、浮空 quad 和普通 RGBA。
- 实现 decal atlas、dictionary、entry、category/index 和 materialData 写回。
- 区分投影路径（depth reconstruction + clip + back clip）和浮空路径（NoClip）。
- 完成贴墙、透明度、SDF 抗锯齿、内景和夜间光照验证。

验收：在 Mod Studio 绘制 decal，绑定建筑，构建 package，游戏内正确显示；修改或删除后不残留旧 atlas 条目。

## P1：Property Editor 深度能力

- 建立 Menu2、UnitEntry、runtime unitSlot、ruleId、resourceId、UI TGI 的分离身份模型。
- 建立 Menu → Unit → Lot/Graphics → Bundle → Rule → Resource → UI Query 依赖图。
- 增加普通建筑、小学、水泵、地标、交通、Omega 模板校验器。
- 增加 effect、spawner、path、agent、transport 的语义校验和受控写回。
- 将 raw hash 编辑提升为带来源、继承和单位的语义编辑。
- 完善 openscp.lot-asset/1、openscp.raster/1、openscp.decal/1、openscp.menu/1、openscp.unit/1、openscp.er2/1 版本化 Schema。

## P1：Package 与 ER2

- 统一资源编译顺序：验证 → 依赖闭包 → 编译 → 写回 → 写包 → 重读 → 对比 → 报告。
- 建立 PackageValidationReport，区分容器、资源、依赖、profile、覆盖顺序和运行时状态。
- 区分 overlay、data-root、ecogame、addon profile。
- 记录加载目录、priority、TGI 冲突、缓存命中风险和重启要求。
- ER2 先支持已验证模板引用和已解码字段编辑，再支持新规则编译。
- 补齐 ER2/ERZ 编译、版本选择、rule index 重定位、EcoGame 加载和运行时对拍。

## P1：菜单与资产合流

- 将 Menu Schema 纳入 Mod Studio 项目。
- 菜单、资产、Unit、locale、icon、marquee 共用依赖图和 package 输出。
- 建筑分类必须由 Menu2、Unit、zone/resource、bundle、attached UI 和 ER2 证据共同判定。
- 保留菜单多归属和运行时聚合语义。

## P2：实机闭环与质量

- 固定一个包含模型、lot、decal、prop、light、effect、spawner、path、Unit、菜单和 ER2 模板的最小样本。
- 构建前通过静态资源闭合、ER2 索引闭合、UI 查询闭合、运行时对拍四道闸门。
- 记录游戏时间、UnitEntry、unitSlot、ruleId、resourceId、bin 前后值、applyCount、delta、丢弃量和未知字段。
- 每次构建生成 manifest、依赖清单、SHA-256、游戏版本和回滚包。
- 验证启动、菜单、图标、放置、模型、材质、lot、decal、Unit、ER2、UI 查询和删除回滚。

## 实施顺序

1. Blender 导入契约和一个受支持模型格式的编译器。
2. Property/lot 写回和原生 package 构建。
3. Decal Schema、绘制、注册、写回和实机验证。
4. Menu/Unit/ER2 依赖图和统一 package 输出。
5. 已验证特殊建筑模板和受控机制编辑。
6. 交通、Agent、复杂 spawner/path、Effect/Swarm 和任意 ER2 新规则。

## 当前主要风险

- 通用 RW4 任意 section 的编辑和原生材质重组仍不是公开稳定契约；当前稳定路径是
  资产工作台的 LOTM/GLB 输入和 M1 编译器。
- Property/LOTM 的语义编辑仍以受控 schema 和场景列族为边界，不能宣称任意属性
  保真重写。
- Decal、ER2 通用编译和完整游戏行为闭环仍需按样本逐类验证；已有的菜单/行为样本
  不能外推为所有建筑类型。
- ER2 通用编译和游戏加载验证尚未闭合。
- 普通 overlay 的写出成功不能等同于游戏内生效。
- 所有新增界面文案必须同时维护 zh-CN 和 en-US。
