# Effect（EA Swarm 粒子系统）预览——机制分析与实现路线（2026-10-08 只读取证）

## 当前实现（2026-10-14）

- Property Editor 精细模式的 Effect 开关按需解析集合；默认模式保留黄色锥体。
- `sc-properties::swarm_particle` 解普通粒子与 MetaParticle 的发射参数和子组件引用；Tauri 资源层按实际 TGI 查 RW4 纹理并解码 PNG；前端独立的固定步长解算器与 Three 点精灵渲染器限制粒子总量和帧开销。
- 安装包主集合的 37 个普通粒子及 250 个 MetaParticle 已按 section 边界验证；首个普通粒子的纹理键 `0x4D272BC5` 已在游戏包中解码为 PNG。主集合的 MetaParticle 多数指向地形 Brush（type 32），这类组件不作为屏幕粒子渲染。
- App 包的其他集合存在同版本字段布局差异。后端保留已验证的组件前缀并返回诊断；未能确认边界的组件、快速粒子（type 12）、缺失纹理及非粒子类型仍显示标记锥，这部分还需继续对拍。材质型贴图（draw mode 15）现在会复用已解码的 TGI 贴图走点精灵降级路径。模拟目前覆盖基础发射、生命周期、方向力、重力、阻尼和确定性的风力扰动，尚未覆盖 Swarm 的表面碰撞、随机游走、子粒子死亡效果等行为。

> 目标：在 Property Editor 实现 effect 单元的 Swarm 粒子模拟预览。
> 数据源实证：SimCity_Game / App / DLC0 / EP1 / Graphics 五包只读扫描 +
> exe 反编译（dev-dump）+ source-tree 类结构。附真实数据样本与分阶段路线。

## 一、数据链全景（本轮回证）

```
Lot 属性 Effect 单元（0x02A907B5 keys 列）
  effect_id = { type:0, group:0, instance:FNV(效果名) }      ← 38208 条引用统计
    ↓ 按「名字」在已加载效果集合中查找（GetActiveEffectInfo(pattern)）
Swarm 效果集合资源
  type = 0xEA5118B0
  group = 0x40450200|效果等级字节（基础三档）/ 0x40804300|…（EP1 扩展组，
          来自 cAppSystem::mEffectExpansionGroupIDs）
  实例 = 集合 id（如 4CF9B596 主城集 284KB、622B9CD7 EP1 总集 1.18MB）
  实体位置：SimCity_App.package（基础全部）+ SimCity_DLC0.package（扩展）
    ——SimCityData 12 包全扫：Game/Graphics/EP1 内**零**效果集合
    ——lot effect_id 的 instance 在包条目层**零**直配（名字只在集合内部表）
  格式：编译态 cCollection 二进制（非 ArgScript 明文；名字以 FNV 哈希存
    导出表；主集合第一个组件段 count=37，VisualEffect count=347）
    ↓ cIEffectsManager::AddCollection
运行时
  EffectControl → SetEffectsRunning(enable, 1, externalId)   ← enabled 列
  cVisualEffect（Start/Stop/ApplyEffect；cDistributeEffect
  CreateSamples2D/3D 粒子分布采样）；着色器 = effect* 家族 131 片段
  （point/soft particle、光照 Dir/SC/SH 三变体，见 shader-fragment-census）
```

关键实证（本回新增探针，见 `crates/sc-properties/examples/`）：
- `effect_units`：38208 条 effect 引用，type 全 0（名字键），同效果 instance
  跨 lot 复用（F03864CC/309CFE3F/59A569A8…），enabled 列 = 火情/废弃/夜间开关
- `effect_collections`：0xEA5118B0 集合 81 个资源实体（App 基础+EP1 组、
  DLC0 扩展 4 集合）
- `effect_resource_find`：lot effect instance 在包条目层零直配（名字只在
  集合内部表）
- LOTM v9 载荷（用户提供建筑 dump）**不含 effect 字段**——effect 数据在
  独立集合资源，与 LOTM 管线并行，需新增一条解析线

## 二、实现路线（分四阶段，每阶段可独立对拍）

### 阶段 1：集合资源解码（Rust，源文件解析层）

- 新命令 `read_effect_collections`：扫包收集 T 0xEA5118B0 资源（组
  0x40450200/01/02 + 0x408043xx），按 group=画质档/扩展组分组下发
- 已实现 `sc_properties::swarm::SwarmCollection::parse`：版本 4、组件/资源
  段边界、变长 VisualEffect block、导出名字哈希与导入索引。主集合
  `effects_main.col` 284720 字节可完整解析，含 347 个 VisualEffect。
- 尚需解码各组件段的粒子参数和贴图引用，供后续模拟渲染使用。
- 阶段产出：`{效果名 FNV → 组件清单（类型/数量/资源引用）}`——足够做
  「效果卡片」UI（名字语义 + 组成），数据确定性 100%

### 阶段 2：效果单元 UI 升级（前端，低风险）

- 效果单元保留标记锥/挂点，加效果语义标签（FNV → 集合内反查；命不中
  显示 hex）——对齐 spawner 小人同款「标记+语义」哲学
- 可见性对齐 decal：effects 图层眼睛开关已存在；预览默认开/关由该开关
  统一控制

### 阶段 3：粒子模拟预览（前端，核心目标）

- three.js `Points` + 自定义 effect* 简化着色器（点精灵 + alpha 渐变 +
  加色混合备选）；发射分布按集合参数（cDistributeEffect 的 2D/3D 采样
  同构：CreateSamples 的分布形状/速率/生命周期）
- 参数缺失的效果按名字关键字语义降级预设：smoke→灰阶上升湍流、
  fire→加色闪烁、haze/fog→大粒子低透明、spark→快速衰减
- 帧预算：粒子总量上限 + 每效果预算（对拍游戏 cComponentStats 的
  budget 思想）；lot 内多效果按 enabled 列启停（对齐
  SetEffectsRunning 语义）
- 风险隔离：与 GLB Worker 教训不同——本项**只新增渲染对象，不触碰
  建筑/地面既有装配链**，且可按效果单元粒度开关

### 阶段 4（可选）：真粒子对拍

- 集合二进制里的纹理引用（cEffectResourceInfo）→ 对位 slot 贴图解析；
  与游戏截图逐效果对拍（烟雾形状/颜色/速率）

## 三、风险与未知

- 集合容器、VisualEffect block 和导出表已解析；各组件类型的参数载荷
  仍需逐型解析和游戏对拍。
- 部分效果含音/光组件：PE 预览只做视觉粒子，光组件可复用
  buildRealLightUnit
- EP1 特有效果可能引用 EP1 贴图（DLC0 内自洽，已实证存在扩展集合）

## 四、待办与决策点

1. 阶段 1 的 cCollection 二进制逆向深度（只解清单 vs 全解组件参数）
2. 预览粒子的保真度档位（象征性 vs 参数驱动）
3. 集合资源走「按需加载」还是「打开包时全量下发」（主集合 284KB、
   EP1 总集 1.18MB，量级可控，建议打开包时全量）
