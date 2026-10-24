# 建筑资产 authoring debug 工具簇：action 闭环与覆盖包验证

日期：2026-10-24。

本轮优先分析三个最直接服务于新建筑资产制作的菜单条目：

| Menu instance | 名称 | 目标 |
| --- | --- | --- |
| `0x53B77423` | Edit Unit Prop Tool | 调整 Unit 上的 prop 装配 |
| `0xA2AA4268` | Edit Snap Points Tool | 编辑 Unit 的 snap point |
| `0xB65E1F2E` | Unit Connecter Tool | 手动连接 Unit |

## 零售包中的真实字段

从 `SimCity_Game.package` 逐项读取 `0x00B1B104 / 0x09878A01` Property 后，三项的关键字段如下：

| Menu | Parent `0x00B2CCCB` | `ecoGameToolAction` `0x08F672C7` | 分类 `0x0975695E` | UI 分类 `0x0DB9FC63` |
| --- | --- | --- | --- | --- |
| `53B77423` | `BE5744E0` | `53B77423` | `3D752D6A` | `A2A33338` |
| `A2AA4268` | `BE5744E0` | `A2AA4268` | `3D752D6A` | `A2A33338` |
| `B65E1F2E` | `BE5744E0` | `B65E1F2E` | `3D752D6A` | `51E7A18D, A2A33338` |

这三个条目共享 `0x48E1EB00:BE5744E0` 空壳 Parent；该 Parent 在零售包中是 0 属性。它们的标题和描述仍完整存在于 Locale：

```text
Edit Unit Prop Tool
Edit Snap Points Tool
Unit Connecter Tool
```

## 引擎侧 action 闭环

开发样本的 `SC::CreateTools` 会从工具脚本组 `0x08F570BD` 枚举 Property，读取 `0x08F672C7`，再调用 `SC::CreateTool(actionInstance)`。`SC::CreateTool` 的 switch 只有以下 10 个 action：

```text
134CDAC9 -> cToolPathPlacer
1943CB96 -> cToolPlop
2EA8FB98 -> cToolDefault
476A98C7 -> cToolUFO
6FE8D729 -> cToolMetaLotZone
70930C64 -> cToolDisasterPlop
9730F3A0 -> cToolBulldozer
9A2BAEE8 -> cToolTornado
C741981F -> cToolPathSelector
EF089B06 -> cToolRegion
```

因此当前证据是：

1. 三个 authoring Menu 的 action 都指向自身；
2. 自身 ID 不在 `CreateTool` factory 中；
3. `CreateTools` 对未知 action 返回 null，不会把该条目放入可执行工具列表；
4. 把 Parent 改到可见建筑工具的 Parent，只能改变菜单数据继承/分类，不能凭空创建缺失的 C++ tool 实例。

这解释了为什么“显示在菜单里”和“点击后真的进入 Unit 编辑器”必须分开验收。当前不能把这三个条目标成已恢复的可执行编辑器。

还有一条旁证：开发样本 PDB 路径清单包含 `sctoolunitconnecter.h`，但 `sctool/source` 中没有对应的 `sctoolunitconnecter.cpp`；同样，`sctoolpathedit.h` 与 `sctoolpathgraphicsedit.h` 也只有头文件路径。它更像是曾经保留过接口/实验分支的开发代码，而不是当前样本中已链接的工具实现。这个证据不能证明所有版本都没有实现，但足以说明不能仅凭菜单标题恢复它。

## 对照：同一隐藏 Parent 下的其他条目

`AD947E95`、`A4C5D581`、`EB627DE3`、`A5800DC3`、`E48FCA6B` 等条目的 `ecoGameToolAction` 都是 `0x70930C64`，对应已确认存在的 `cToolDisasterPlop`。它们是灾害放置变体，而不是 authoring 工具。`C741981F` 和 `9A2BAEE8` 则分别闭合到 `cToolPathSelector` 与 `cToolTornado`，是隐藏 Menu → factory 的阳性样本。

## 覆盖包实验

为避免把验证入口放进教育/大学子菜单，当前诊断包改用已经在道路工作台
overlay 中验证过的同级菜单字段：

```text
Parent       = 0xAF042E9A
分类         = 0xAC12C904
UI 分类      = 0xAC12C904
uiPosition   = 110 / 111 / 112
```

包名为 `000_OpenSCP-debug-authoring-road-action-probe.package`，位于
`SimCityData`。它还把三个缺失的 action 临时映射到已存在的
`0x1943CB96 (cToolPlop)`，只用于验证“菜单数据已命中、CreateTools 能否创建
实例”这条链；这不是 authoring 工具实现，也不会把 cToolPlop 伪装成 Unit
Prop/Snap/Connecter 编辑器。

早期使用现有的 `sc_properties::debug_tools` 补丁逻辑生成过教育分类的独立覆盖包：

```text
D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityUserData\Packages\OpenSCP-debug-authoring-overlay.package
```

这份初始包的 DBPF 结构和回读都正确，但实机没有显示菜单。随后结合已有的 retail 加载对拍确认：普通数据库按 priority 降序查找，同 priority 先注册者优先；UserData/Packages 会晚于 SimCityData 注册。因此普通同 TGI overlay 放入 UserData/Packages 不能自动覆盖已经注册的基础资源，文件名 `OpenSCP-...` 放入 SimCityData 也会排在 `SimCity_Game.package` 后面。

随后追加过按数据目录枚举顺序优先的测试副本；当前可扫描的诊断包是：

```text
D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\000_OpenSCP-debug-authoring-road-action-probe.package
```

`000_` 前缀来自已有运行时实验：该目录的 `000_...package` 会在普通基础包之前注册，从而成为同 TGI 的首个命中候选。该命名只适用于当前离线安装的普通资源链，不能推广到 UserData/Packages 或 EcoGame 脚本包。

该包包含 4 条资源：三个 authoring Menu 加一条标题对照项。道路诊断包的三个目标各自改写：

```text
Parent       BE5744E0 -> AF042E9A
菜单分类      3D752D6A -> AC12C904
UI 子分类     A2A33338 -> AC12C904
菜单位置      100/200/300 -> 110/111/112
```

`B65E1F2E` 原有的两个 UI 子分类都被收敛到道路分类，两个位置数组元素也都设为 112。其余 Property 字段逐字节保留。覆盖包已重新打开并回读校验，目标字段均正确；原始 `SimCity_Game.package` 没有被修改。

同时加入了一个阳性对照：可见的 `0x75A90B66`（宿舍）标题改成同栏的“商務學院”。如果游戏中出现两个“商務學院”，说明覆盖包确实被加载；随后应分别点击三个 debug 条目，记录“菜单出现 / 点击有响应 / 预览出现 / 能否提交”四个结果。

## 当前判定与下一步

| 项目 | 判定 |
| --- | --- |
| 数据层重新显示三个菜单 | 已可生成、已回读验证 |
| 进入真实 Unit Prop/Snap/Connecter 编辑器 | 未闭合；factory 缺少 action |
| 通过覆盖包恢复缺失 C++ 实现 | 不可行；package overlay 不能注入 native class |
| 是否值得保留该覆盖包做实机测试 | 值得；可区分“菜单门控”与“运行时实现缺失” |

若菜单可见但点击无反应，下一步应追查开发版/工具 DLL 是否曾注册 `sctoolunitconnecter.h` 相关类，或将现有 `cToolPlop` 的 snap/prop 逻辑在 Mod Studio 内重建。不能把 `CreateTool` 中已有的 `cToolPlop` 强行映射给三个条目后宣称恢复了 authoring 工具，因为它们的输入状态、提交和写回语义不同。

