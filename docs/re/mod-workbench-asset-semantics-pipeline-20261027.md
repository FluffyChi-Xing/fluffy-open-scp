# Mod 工作台资产语义管道重审报告

日期:2026-10-27  
前置研究:`building-animation-runtime-design-20261027.md`(动画/建造/Effect 三
路径已源码闭环)、`school-property-er2-runtime-model-20261027.md`(Property →
ER2 → 运行时投影)、`effect-swarm-preview-design.md`(Effect 预览)  
用户命题:可视化搭建隐藏了背后的复杂逻辑。资产 mod 要"活",除了导入资产与
绑定 prop bins / effect / light / spawner / path / lot / 菜单之外,**必须在
后台完成相关语义的解析与回写**,否则产物仍是死的。

## 1. 结论

1. **工作台今天只做了"皮"**:导入(GLB/LOTM/blend)→ 视觉预览 → 六槽
   贴图绑定 → 菜单 schema 校验 → 项目内 `asset.lot.json` 持久化。它产出
   的不是资产包,是一份设计文档。
2. **"活资产"= 一组 Property 表 + 资源的正确组合**,与视觉模型无关:
   Unit(资源 bin/规则/子件)+ Lot(图形/挂点)+ Menu2(入口)+ bundle/
   ER2(行为)+ 五张动画表(§动画报告)+ ConstructionSite(建造)+
   Effect attachment / GraphicsLight / Spawner / Path。缺失任何一组,
   建筑就是静态雕塑。
3. **写回地基已存在且被低估**:`mod_flow.rs:923 build()` 已能产出真实
   DBPF——"克隆原生包全部条目(building-asset)+ 按 TGI 叠加静态资源
   (static-resource)"的 overlay 模式。**克隆参照建筑 + 叠加改写的
   Property 表 = 语义回写的最短路径**,且不依赖尚未接入的 RW4 编译。
4. 属性级写回 API(把编辑后的 Property 表序列化回 DBPF 条目)是唯一
   真正的缺口,其余都是既有能力的编排。

## 2. 现状盘点(代码钉死,2026-10-27)

| 能力 | 状态 | 位置 |
|---|---|---|
| 导入 GLB/LOTM/blend + 视觉预览 | ✅ | `useWorkbenchAsset.openDraftAsset` |
| 六槽材质绑定(贴图→槽位) | ✅(预览级;RW4 写回未接入) | `draftMaterialSlots` |
| 菜单绑定 schema | ⚠️ 仅校验(`AssetBindingContract.menu`),无生产 | `assetBinding.ts` |
| 文档持久化 | ✅ 项目内 `asset.lot.json`(openscp.lot-asset/1) | `useWorkbenchAsset:1162` |
| DBPF 构建(克隆+overlay) | ✅ 但工作台未使用 | `mod_flow.rs:923` |
| GLB→RW4 模型编译 | ❌ 未接入(`validate` 明示) | `mod_flow_assets.rs:517` |
| Property 表级解析(参照包语义) | ⚠️ 只读会话有(readLotEditorSession 等),无通用表解析器 | `package_service.rs` |
| Property 表级写回 | ❌ 无 API | — |
| 资源 bin/动画表/建造表/触发器解析 | ❌(引擎机制已研究闭环,无消费代码) | 见前置研究 |

## 3. "活资产"语义清单(九族)

以一个"活的 RCI 建筑"为基准,逐族列出语义面、现状与来源研究:

| # | 语义族 | 作用 | 现状 | 来源研究 |
|---|---|---|---|---|
| 1 | Unit Property | 资源 bin 容量/初值、规则挂载、子件 | 会话内可读可编辑,无写回 | school-property §2 |
| 2 | Lot Property | 图形引用、UnitLocation/Path/PropBin/Light/Effect attachment | 会话内可读(PE 视口),无写回 | 同上 §2 |
| 3 | Menu2/menuUnit | 入口、价格、图标、父菜单 | 仅 schema 校验 | menu-mechanism 系列文档 |
| 4 | bundle + ER2 | 行为规则(教育/垃圾/火灾…) | 只读研究闭环 | school-property §3 |
| 5 | 资源 bin 语义 | amount/capacity 驱动一切比例动画与阈值 | 研究闭环,无消费 | 动画报告 §3.4 |
| 6 | 动画五表(AnimModel/AnimFXFloat/Vector3/Active/ControlFX) | 模型时间/参数/启停 | 研究闭环(ID 家族 0x9F3E6B3..F1) | 动画报告 §9.1 |
| 7 | ConstructionSite | 建造动画(进度 bin + cap 层) | 研究闭环(§10) | 动画报告 §10 |
| 8 | Effect attachment / Light / Spawner / Path | 挂点、夜灯、Agent 生成、路径 | 挂点/会话可读;Effect 预览有设计 | effect-swarm 设计 |
| 9 | choreography / triggers | 事件序列(卷帘门/防盗窗) | 研究闭环 | 动画报告 §3.6 |

**判定规则**:1-4 缺任一 = 不可放置或不可购;5-9 缺 = 可放置但"死"
(不建造、不动画、不亮灯、不出人)。

## 4. 管线设计:参照克隆 + 绑定图 + 差异回写

### 4.1 三段管道

```text
[parse]  参照原生 .package(选一个行为最接近的原版建筑)
         → 枚举其全部 Property 表(Unit/Lot/Menu2/bundle/ER2/五动画表/
           ConstructionSite/attachment/Light/Spawner/Path)
         → StaticAssetGraph(每张表 = 一个语义绑定节点,带来源 TGI)
[bind]   工作台 UI 逐节点编辑:
         - 模型引用 → 指向导入资产(编译后)或暂留原模型(M1)
         - 贴图槽位 → 已有六槽绑定
         - 资源 bin 初值/容量、规则参数、动画表行、cap 表、阈值
         - 菜单名/价格/图标
         → binding graph(带 provenance:克隆值 vs 修改值 vs 未解析)
[write]  mod_flow.build 已有能力 + 新增 property-serialize:
         - building-asset schema 克隆参照包全部条目(行为语义的底座)
         - overlay 被编辑的 Property 表条目(同 TGI 覆盖)
         - overlay 模型/贴图资源(M2 起依赖 RW4 编译)
         → 产出 .package
```

### 4.2 AssetBindingContract v2(草案)

```text
contract v2 = v1(source + 六槽 + menu)
  + reference: { packagePath, unitTgi, lotTgi, menuTgi }   # 克隆参照
  + semantics: {
      unit:      { resourceBins[ {resourceId, capacity, initial} ],
                   bundles[], ruleOverrides[] },
      lot:       { modelRef, propBins[], effects[], lights[],
                   spawners[], paths[], constructionSite? },
      animation: { animModel[], animFxFloat[], animFxVector3[],
                   animFxActive[], controlFxActive[] },        # 五动画表
      choreography?: { sequences[], triggers[] }
    }
  + provenance: 每节点 = cloned | modified | added | unresolved
```

`asset.lot.json` 升 v2:`semantics` 段落盘(与视觉段并列),加载时
重放 parse→merge,保证参照包更新后可重放差异。

### 4.3 解算器接线(受控)

`src/lib/buildingRuntime/` 纯函数解算器(动画报告 §10.4)消费
`semantics.animation/construction` + 运行时快照 → 投影;工作台预览与
游戏各跑各的解算,**共享同一份绑定图**——预览"活"了,写出的包在游戏里
才是活的;两者不一致时以绑定图为准回查。

## 5. 里程碑(与 RW4 编译解耦的务实路径)

- **M1 语义克隆包(不依赖 RW4 编译,价值即成立)**:
  参照包克隆 + Property 表 diff 回写 + 菜单改名/价格/bin 初值——
  产物是一个**行为完整**的克隆建筑(用原模型)。验收:放入游戏可
  建造、可运行规则、夜间亮灯。
- **M2 模型替换**:RW4 编译接入(GLB→RW4 model/Mesh)或先走
  "图形 TGI 指向其他现存游戏模型"的换皮捷径;六槽贴图随模型写回。
- **M3 语义创作**:五动画表/建造表/触发器的创作 UI + buildingRuntime
  预览联动;从"克隆改值"升级到"从头组装"。

## 6. 边界与风险

- M1 的克隆包含原建筑的**全部语义**,作者必须审查许可与套用范围
  (克隆原生 EA 资产仅限本地研究;发布需替换全部 EA 派生内容)——
  工作台应在 build 时展示 provenance 清单并强制作者确认;
- ER2 规则编辑是深水区:M1 只暴露参数值编辑(初值/容量/速率),
  规则拓扑编辑延后;
- overlay 的覆盖优先级依赖游戏加载顺序,构建器必须用"参照包条目 +
  同 TGI 追加覆盖"并在产物自检中验证 TGI 冲突集;
- 五动画表的正式 schema 命名仍未回填(合成名 + 原始 ID 并显)。

## 7. 验收(M1)

- 选定参照建筑 → 工作台列出其全部语义绑定节点(九族齐全,带 TGI);
- 修改菜单名/价格/某资源 bin 容量 → build 产出 .package;
- 游戏内:新菜单项出现、可建造、资源容量生效、建筑行为与参照一致;
- provenance 面板能区分 cloned/modified;`asset.lot.json` v2 重放一致;
- 全程不触碰冻结区装配/坐标/取景逻辑(frameInvariants 全绿)。
