# Effect（EA Swarm 粒子系统）预览——机制分析与实现路线

## 当前实现（2026-10-14）

### EP1 建筑 `B468821F` 的黄色圆锥（2026-10-14）

用户截图中的模型 `B4688210` 对应 Property `B468821F`。原始属性的 8 个
Effect `enabled` 全为 `false`，而预览 composable 和视口此前都以此字段过滤，
因此开启“渲染 Effect”仍不发出任何效果请求，也不会替换标记锥。预览开关现
在会显示所有已放置 Effect；`enabled` 仍作为属性值保留，代表游戏运行时状态。

这 8 个单元对应 5 个不同 effectId，均导出于 App 包集合 `622B9CD7`：
`660BC390` 有 3 个候选（2 个直接纹理可解码）；`4D98C606`、`E09EDE1F`、
`E6BCECD8`、`99DA8CAB` 分别有 2、5、5、1 个候选。这些候选具有正发射率、
非零尺寸和 alpha 曲线。此前将 `drawMode=15` 的主资源 ID 当纹理查找，因此
错误地判为缺贴图并画成无贴图小球。截图上方的 decal 缺失提示与 Effect 数据链
无关。

### 2026-10-14 材质与图集勘误

`SP::cEffectsRenderer::CreateTextureParticleSet` 对 `drawMode=15` 以主资源 ID 查
`GetEffectMaterial`/`MaterialManager`；`SP::cEffectsResources::AddMaterial` 将材质
参数 ID 0–7 的纹理引用交给 `TextureManager`。Swarm 集合辅助资源 `type=1`
保存材质 ID、shader ID、format 和变长参数表；参数类型 6 是 64 位纹理 ID。
新增边界检查解码器后，EP1 集合 `622B9CD7` 有 223 个材质，623/623 个
`drawMode=15` 粒子都能链接到含纹理参数的材质。实证映射：

| 材质 ID | 参数 slot 2 纹理 ID | Graphics 包纹理 | 粒子图集 |
| --- | --- | --- | --- |
| `684A2E36` | `C0D189FE` | 128×2048 | 1×16 |
| `93DC33AD` | `A148458F` | 256×256 | 1×1 |
| `FA57D08F` | `D7E56466` | 64×1024 | 1×16 |

三张纹理都能解码，PNG 体积分别约 204/79/42 KiB，低于预览层 512 KiB 上限。
Property Editor 现在先按材质参数解析纹理，再以四边形按 `mTileCountU/V`、
`mFrameStart/Count/Speed`、尺寸、宽高比、旋转曲线采样。引擎
`StreamerSetup`/`GenerateQuadInfo` 证明 `frameCount=0` 用完整图集循环、
尾帧回绕，并允许按粒子年龄或生命周期驱动；HLSL 片段进一步证明相邻两帧
`frameDelta` 插值以及 camera-facing 四边形。当前实现尚未还原材质 shader 的
混合/法线/soft-particle 深度淡化，`frameRandom`、宽高比和旋转的随机分布也
只做近似。电弧可能使用独立 beam/ribbon 组件，仍需从具体效果 ID 回证；不能
把电弧视作普通 sprite。以上测试验证了解码和数据链接，尚非游戏与 Tauri 的
同机位截图对拍。

### 嵌套效果递归验证（2026-10-14）

Property Editor 的资源层现在会有界展开 VisualEffect block `type=0`，以及
MetaParticle 的 `type=0` 子 VisualEffect；沿路径检测循环、限制四层深度和每个
效果 32 个 emitter，并组合 block 的位移、旋转、缩放。子效果的粒子纹理、颜色
与生命周期来自真正的 Particles 组件。MetaParticle 嵌套路径暂用父 emitter 的
发射参数代理每个子效果实例，响应中的 `fallback` 和 `unsupportedBlocks` 标明
该结果为近似预览；多层 MetaParticle 的独立发射时序尚未实现。

真实 `SimCity_App.package` 回归测试遍历 3264 个导出效果：修复粒子段版本 2
变长 speed curve 解码后，旧逻辑仍有 1661 个无候选 emitter，递归后其中 450 个
首次有候选，至少 291 个仅用 App 包内纹理且正发射率就满足现有渲染条件。
这些是按三档画质集合实体统计的覆盖下限，不代表 291 个独立建筑，也未证明
Three 画面与游戏一致。其余 159 个需要继续对位纹理包、
材质和资源引用；仍无候选的效果还包括 Distribute、外部组件等类型。

下一轮应选含上述 221 个效果的具体建筑，在 Property Editor 和游戏中截取同
一效果、相机与时间点，对比发射位置、速率、尺寸、颜色及贴图。同时分别统计
缺失纹理、零发射率、仅非粒子组件、全局 emitter 配额耗尽等原因，避免把所有
黄色锥体归为同一种解析失败。

逆向 `src/EA/Swarm/ReadDescription__2.c` 显示 Particles 版本 2 在 rate cycles
与 speed scale 后按顺序保存变长 speed curve 和 variation。此前的固定 6/14
字节猜测使两个集合（实例 `622B9CD7`、`BC2C0BE9`，各三档画质）只解出
639/1281 与 11/107 个普通粒子。改为按变长字段读取后，App 包粒子段解析失败
数从 6 降为 0；剩余空效果的顶层 type 1 block 从 1347 降为 0。这是本次比
嵌套递归更大的可见性修复，应优先对拍这两个集合中的建筑效果。

修复后的空效果顶层 block 计数（一个效果可有多个 block，仍按三档画质重复）：
MetaParticle `type 2` 为 770，model `type 8` 为 170，外部 `type 37` 为 132、
`type 32` 为 129，嵌套 visual `type 0` 为 120，Distribute `type 13` 为 81。
下一轮先按 MetaParticle 的 child type 和解码结果拆分 770 个 block，再查
`type 0` 的深度/循环限制；随后取 Distribute 与外部类型的具体游戏样本。
音效、相机等非粒子 block 不应单凭出现次数判断为预览解析失败。

### 2026-10-14 勘误：MetaParticle 子组件类型

`new-cource/include/global/tFXComponentType_ord432.h` 证实 `type=0` 是
`kFXCmptVisual`，表示 MetaParticle 的子组件是另一个 VisualEffect，**不是**
普通粒子 `type=1`。上一轮 `0 -> 1`（以及 `11 -> 12`）映射已撤回；直接拿
同索引普通粒子会误画不相关的纹理和参数。对 App 包的 627 个 `type=0` 子引用
与 DLC0 的 108 个子引用逐一检查，全部落在有效 VisualEffect 索引；其中分别
330 和 72 个子效果直接带粒子 block。因此“多数锥体仍无粒子”的主要结构性
缺口是递归展开 VisualEffect 子效果，并正确组合 MetaParticle 的发射时序、
位置和子 block 变换。主集合里曾观察到 147 个此类引用；该数字只覆盖一份
集合，不能代表整个 App 包。

### 2026-10-14 对拍分层

同一 `tFXComponentType` 枚举给出了 VisualEffect block
类型表：`1/2` 是 Particles/MetaParticles，`3..11` 依次为 decal、sequence、
sound、shake、camera、model、screen、light、game，`12` 是 FastParticles，
`13` 是 Distribute，`14..16` 是 ribbon/animate/beam。由此可把当前的
`unsupportedBlocks` 分成“非屏幕粒子组件”和“尚未接入的屏幕粒子组件”，避免
把音效、相机、模型等正常行为误报为粒子解析失败。

对 `SimCity_App.package` 的统计（新增 `effect_component_stats` 探针）显示：
5103 个 VisualEffect、4757 个普通粒子 block、1372 个 MetaParticle block，
MetaParticle 子引用中 627 个是嵌套 VisualEffect、702 个是 type 32 外部组件；
另有 6 个 FastParticles、372 个 Distribute block 和 501 个 type 37 外部 block。
DLC0 有 1131 个普通粒子、105 个 MetaParticle，无 Distribute block，但有
18 个 type 37 外部 block。上述数字按三档画质集合实体计数，不是独立效果数。
下一轮对拍优先级应为：

1. **嵌套 VisualEffect（type 0）**：按子索引递归展开，限制循环与深度，
   将 MetaParticle 的粒子发射位置/时序与子 block 变换组合；先验证直接带
   Particles 的 330+72 个子效果，再处理多层嵌套。
2. **Distribute（type 13）**：读取 `cDistributeDescription` 的 density、
   sourceType/radius、cluster 和 surface 字段，先做静态采样，补齐喷泉/地面
   散布类效果。
3. **MetaParticle 外部组件**：type 32/47 等需要分别调查外部组件的画面语义，
   不能直接当成 screen sprite 或一概称为 terrain Brush。
4. **FastParticles（type 12）**：`cFastParticle` 没有普通粒子属性，现有通用
   fallback 只能保证可见性，需从 `cFastParticlesDescription` 和纹理 renderer
   的 vtable 调用点恢复真正的采样参数。
5. **粒子行为细节**：surface bounce/slide/death、map repulse、random walk、
   attractor、wiggle、death effect，以及 `cITextureParticleRenderer` 的 billboard
   facing、动画 tile 和 soft-particle 深度衰减。

- Property Editor 精细模式的 Effect 开关按需解析集合；默认模式保留黄色锥体。
- `sc-properties::swarm_particle` 解普通粒子与 MetaParticle 的发射参数和子组件引用；Tauri 资源层按实际 TGI 查 RW4 纹理并解码 PNG；前端独立的固定步长解算器与 Three 点精灵渲染器限制粒子总量和帧开销。
- 安装包主集合的 37 个普通粒子及 250 个 MetaParticle 已按 section 边界验证；首个普通粒子的纹理键 `0x4D272BC5` 已在游戏包中解码为 PNG。主集合的 MetaParticle 多数指向地形 Brush（type 32），这类组件不作为屏幕粒子渲染。
- App 包的其他集合存在同版本字段布局差异。后端保留已验证的组件前缀并返回诊断；快速粒子（type 12）现在会使用明确标记的通用粒子参数降级显示，缺失纹理及非粒子类型仍显示标记锥，这部分还需继续对拍。材质型粒子（draw mode 15）现按集合辅助材质参数解析图集纹理，走面向相机的四边形预览。模拟目前覆盖基础发射、生命周期、方向力、重力、阻尼和确定性的风力扰动，尚未覆盖 Swarm 的表面碰撞、随机游走、子粒子死亡效果等行为。

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
