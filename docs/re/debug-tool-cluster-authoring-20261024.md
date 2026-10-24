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

开发样本的 `SC::CreateTools` 会从工具脚本组 `0x08F570BD` 枚举 Property，读取 `0x08F672C7`，再调用 `SC::CreateTool(actionInstance)`。旧开发样本反编译文本只呈现 10 个 action；本机发售版二进制复核后，factory 实际保留 12 个 action，新增的两个是高塔/空中桥梁工具：

```text
134CDAC9 -> cToolPathPlacer
1943CB96 -> cToolPlop
2EA8FB98 -> cToolDefault
476A98C7 -> cToolUFO
6FE8D729 -> cToolMetaLotZone
70930C64 -> cToolDisasterPlop
9730F3A0 -> cToolBulldozer
9A2BAEE8 -> cToolTornado（factory action；隐藏 Menu 的实际 action 是 70930C64）
C741981F -> cToolPathSelector
EF089B06 -> cToolRegion
22D4ACD5 -> cToolAddTowerLevel
ECC08133 -> cToolSkybridge
```

因此当前证据是：

1. 三个 authoring Menu 的 action 都指向自身；
2. 自身 ID 不在 `CreateTool` factory 中；
3. `CreateTools` 对未知 action 返回 null，不会把该条目放入可执行工具列表；
4. 把 Parent 改到可见建筑工具的 Parent，只能改变菜单数据继承/分类，不能凭空创建缺失的 C++ tool 实例。

这解释了为什么“显示在菜单里”和“点击后真的进入 Unit 编辑器”必须分开验收。当前不能把这三个条目标成已恢复的可执行编辑器。

还有一条旁证：开发样本 PDB 路径清单包含 `sctoolunitconnecter.h`，但 `sctool/source` 中没有对应的 `sctoolunitconnecter.cpp`；同样，`sctoolpathedit.h` 与 `sctoolpathgraphicsedit.h` 也只有头文件路径。发售版 `SimCity.exe` 中三个 authoring action 的原始字节与对应 RTTI 也不存在，因此它们更像是曾经保留过接口/实验分支、发行时移除执行侧的工具。这个证据不能证明所有历史版本都没有实现，但足以说明当前发行版不能仅凭菜单标题恢复它。

## 对照：同一隐藏 Parent 下的其他条目

`AD947E95`、`A4C5D581`、`EB627DE3`、`A5800DC3`、`E48FCA6B` 等条目的 `ecoGameToolAction` 都是 `0x70930C64`，对应已确认存在的 `cToolDisasterPlop`。它们是灾害放置变体，而不是 authoring 工具。`C741981F` 的 action 是 `C741981F`，闭合到 `cToolPathSelector`；`9A2BAEE8` 虽然 instance 与 `cToolTornado` 相同，但其 Property action 实际是 `0x70930C64`，仍走 `cToolDisasterPlop`。

## 覆盖包实验

为避免把验证入口放进教育/大学子菜单，当前诊断包改用已经在道路工作台
overlay 中验证过的同级菜单字段：

```text
Parent       = 0xAF042E9A
工具分类     = 0xC710B6E9
UI 分类      = 0xAC12C904
uiPosition   = 110 / 111 / 112
```

包名为 `000_OpenSCP-debug-authoring-road-action-probe.package`，位于
`SimCityData`。它还把三个缺失的 action 临时映射到已存在的
`0x1943CB96 (cToolPlop)`，只用于验证“菜单数据已命中、CreateTools 能否创建
实例”这条链；这不是 authoring 工具实现，也不会把 cToolPlop 伪装成 Unit
Prop/Snap/Connecter 编辑器。

道路菜单实测后发现，三个原始 debug Property 只有 8 个字段，没有完整图标状态
和 hard gate，游戏 UI 会在菜单组装时将其过滤。当前包的诊断模式因此复制已经能
显示的 `0x85A8C58D` 道路条目作为外壳，再替换三个 debug 条目的标题、描述和
位置。`enable_debug_tools` 通过 `--shell --shell-package=<OpenSCP-ui-overlay.package>`
生成该模式；外壳只用于验证可见性，不代表已经恢复原生 authoring 行为。

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
工具分类      3D752D6A -> C710B6E9
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

## 实机验证补充（2026-10-24）

用户已在重新启动游戏后确认：道路菜单中 100 槽之后的 110、111、112 三个槽位均已显示。该结果确认当前 `000_OpenSCP-debug-authoring-road-action-probe.package` 的目录优先级、Property 外壳字段、道路 UI 分类和 `uiPosition` 设置均已被游戏读取。

截图中显示的是完整道路建筑外壳，三个入口共用建筑图标和普通建筑详情页；标题仍显示为 `***`，说明当前诊断 Property 的本地化标题串尚未正确接入。该验证也没有改变此前的 native factory 结论：菜单出现不等于原生 Unit Prop、Snap Point 或 Unit Connecter 编辑器已经恢复。

后续实机记录应分别点击 110、111、112，按以下四个状态验收：菜单出现、点击有响应、出现对应编辑预览、能够提交并写回资源。只有最后一项闭合后，才能把该入口标记为可执行 authoring 工具。

若菜单可见但点击无反应，下一步应追查开发版/工具 DLL 是否曾注册 `sctoolunitconnecter.h` 相关类，或将现有 `cToolPlop` 的 snap/prop 逻辑在 Mod Studio 内重建。不能把 `CreateTool` 中已有的 `cToolPlop` 强行映射给三个条目后宣称恢复了 authoring 工具，因为它们的输入状态、提交和写回语义不同。

