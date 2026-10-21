# 社区 Mod Package 抽样分析（2026-10-21）

本次抽样目录：

`D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityUserData\Packages`

分析目标是按模组工作台的 Code Mod、Gameplay Mod、Assets Mod 三类，确认社区已经跑通的 package 形态，以及这些形态对 OpenSCP 当前机制研究和工作流设计的帮助。

## 抽样方法与判定口径

- 先解压目录中的 28 个 RAR/ZIP，按内容而不是中文文件名分类。
- 对每个 package 运行 DBPF 条目统计（type、条目数、压缩后大小）。
- 对 `0x00B1B104` 属性资源运行语义级 property dump/diff，记录字段值、Parent 和 TGI 引用。
- 对 `0x2F4E681B` 资源按 RW4 模型处理；它们不能仅按图片或普通二进制理解。
- “覆盖”只在 TGI 层判断。当前项目已有证据表明用户目录 mod 以新增 TGI 被加载；直接覆盖官方 TGI 的工具属于改基础包的另一条路径。

## 1. Code Mod：规则库脚本包

### 代表案例

**`SimCity-Scripts_287520926.package`**（目录原文件：`城市快乐度增加Mod.rar` 解压后的主包）

| 指标 | 结果 |
|---|---:|
| DBPF 条目 | 10,368 |
| `0x00B1B104` Property | 10,367 |
| `0x08068AEB` | 1，约 1.0 MiB |
| Property 压缩后总大小 | 985,898 B |

它与同目录随附的 `SimCity-Scripts_272391411.package`（`外围区建造Mod.rar` 的 EcoGame 版本）可以直接做同 TGI 对拍：

- mod 版 10,368 条，参照版 10,155 条；
- 213 个 TGI 只出现在 mod 版；
- 428 个同 TGI Property 内容不同；
- 代表性的脚本元数据 `40800200:622B9CD7` 中，`0x0EE2A50D` 的版本从 `272391411` 变为 `287520926`，若干引用数组长度由 23/41/45 增加到 38/43/50；
- 多个 `40E0C000` 规则 Property 同时出现数值和引用变化，例如 `0x0AD6A725: 200 → 2000`、`0x0A7917C8: 1 → 16/31/50`，并反复追加 instance `9A1BFB2F`。

### 机制结论

1. Code Mod 的可运行形态是“脚本包中的规则资源集合”，不是一个可独立识别的 C#/Lua 源文件。`0x08068AEB` 是脚本容器/编译产物旁证，真正可解释的行为参数仍落在大量 `0x00B1B104` Property 上。
2. 一个 Code Mod 可以同时新增 TGI、修改已有规则 Property，并更新规则库元数据；因此仅扫描新增资源不足以解释行为。
3. 规则库的依赖通过 Property 内的 Key/TGI 数组表达。`9A1BFB2F` 这类追加引用说明“改一个规则”可能实际启用一组新资源。

### 对 OpenSCP 的帮助

- **包识别**：工作台需要把 `SimCity-Scripts_*`、EcoGame 路径和 `0x08068AEB` 标成“规则/脚本包”，避免把它误报为普通 Gameplay overlay。
- **规则 diff**：应提供同 TGI 的语义 diff，至少展示标量、数组长度、Key/TGI 增删和 Parent，而非只显示字节差异。
- **依赖图**：对 `0x00B1B104` 中的 Key 数组做跨包解析，才能回答“这个 Code Mod 具体把哪个单位/资源接入了规则”。
- **风险诊断**：脚本包的版本元数据、规则库大规模变更和 EcoGame 安装位置应成为校验项；不能把它们当作可安全合并的单资源 overlay。

## 2. Gameplay Mod：属性表驱动的参数改动

### 追加案例：全建筑解锁 / 全成就解锁包

**`AllAchievementUnlock.package`**（`沙盒模式（全建筑解锁）【修正Mod】.rar`）

这是本次最有价值的机制样本。它不是 `0x00B1B104` Property overlay，而是一个单条目 JSON 型资源：

| 指标 | 原版 `SimCity_Game.package` | Mod |
|---|---:|---:|
| TGI | `0A98EAF0:00000000:36BDFDB6` | 相同 |
| 解压大小 | 85,808 B | 58,371 B |
| 成就/解锁记录 | 122 | 83 |
| `Enabled` | 122 条为 1 | 83 条为 1 |

Mod 与原版不是简单把“每座建筑的菜单入口复制出来并改成 unlocked”，而是替换同一个 `0A98EAF0` 资源中的解锁规则记录：

- 保留 83 个相同的 `ID` / `resourceRuleID`；
- 删除 39 条 EP1/资料片及部分区域/旅游解锁记录，例如 `ACGeneralEP1MegaTowers*`、`ACGeneralEP1Omega*`、`ACLaunchArcology*`、`ACTourismAirship*`；
- 对保留的 83 条记录，把原本有实际门槛的 `Tests` 全部替换成同一条恒真测试：
  `Type=Local, Resource=WaterProductionPerHour, Operator=>=, Value=0`；
- 例如原版 `ACBudgetHighExpenses` 的条件是 `Expenses >= 15000`，mod 中变为 `WaterProductionPerHour >= 0`；`ACCivicExpandedCityHall` 原来需要 6 个市政厅模块，mod 中 6 个测试全部变成同一恒真条件。

这说明作者选择的是**解锁判定层旁路**：菜单、建筑 Property、模型和服务规则仍由原版资源提供，mod 只让一部分 achievement/resource rule 立即满足。它没有复制建筑入口，也没有把建筑的模拟字段改成任意值，因此比“批量复制建筑 Property”更不容易破坏单位、服务和 UI 依赖。

但它也不是严格意义上的“全建筑解锁”：39 条记录被删除，尤其是 EP1/资料片内容。其“全建筑”效果依赖目标版本、已有成就系统如何把规则映射到菜单，以及被删除记录是否正好对应作者不需要的 DLC。这个包是经验型的、版本绑定的旁路，并不能证明所有建筑的解锁条件都已经被系统性理解。

#### 对“为什么不崩溃”的直接解释

从静态证据可以确认的稳定性来源有三点：

1. **资源身份不变**：仍覆盖原版同一个 `0A98EAF0:00000000:36BDFDB6`，消费它的代码和引用无需寻找新 TGI。
2. **记录结构不变**：保留的每条记录仍有 `Enabled/ID/resourceRuleID/Icon/Category/Name/Desc/Type/Tests` 等字段，只替换 `Tests` 内容，避免破坏成就/规则解析器的 schema。
3. **测试类型不变**：所有测试仍是 `Local`，只把资源和阈值换成一个在正常城市中必然满足的值；没有删除规则对象、改变 `resourceRuleID` 命名或修改建筑 Property 的复杂引用图。

静态分析不能证明所有运行时行为都安全：删掉的 39 条记录可能导致对应 DLC 建筑仍锁定、菜单缺项或成就显示异常；也无法仅凭 package 证明 `WaterProductionPerHour >= 0` 在所有模拟状态下都恒真。应把它标成“解锁规则替换，部分 DLC 记录删除，需运行时验证”。

#### 对 OpenSCP 的帮助

- 需要支持 **JSON/规则型资源** 的结构化 diff，而不能只按 `0x00B1B104` 分类。
- 解锁编辑器应显示 `ID → resourceRuleID → Tests → 受影响菜单/建筑` 的反向关系；不能把“建筑解锁”误判成建筑 Property 编辑。
- 校验器应报告删除的原版记录、修改后的条件数量、资源哈希和恒真条件风险。
- 最小安全写回案例可以是“只替换一个 `Tests` 数组并保持 schema/ID 不变”，比直接复制建筑资产更适合验证规则编辑管线。

#### 菜单编辑的后续方向（已记录）

这个样本确认：菜单中的“锁定/解锁”不是只能在前端通过隐藏按钮实现，数据侧存在可编辑的规则记录。因此菜单编辑器未来可以把解锁条件作为一等编辑对象，至少覆盖三种操作：

1. **修改已有条件**：保留 `ID`、`resourceRuleID` 和记录结构，只调整 `Tests` 中的资源、运算符、阈值或 `Local/SumInRegion` 作用域；
2. **移除已有条件**：将 `Tests` 替换为空条件或恒真条件，但必须根据运行时 schema 选择安全表达方式，不能未经验证直接删除字段；
3. **为无条件入口人为创建条件**：复制规则记录的结构和引用关系，生成稳定的新 `resourceRuleID`，并将它挂接到对应菜单/建筑入口；这需要同时追踪菜单资源、建筑 Property 和规则资源之间的反向引用。

第一阶段应只开放第 1 类和经过验证的第 2 类，使用 `AllAchievementUnlock.package` 作为结构化 diff 和写回 fixture。第 3 类属于“创建新解锁规则”，需要先完成规则到菜单入口的反向引用解析，不能只凭 UI 上的建筑名称推断。

### 代表案例 A：住宅增长密度

**`middensityfast_growth.package`**（`低中高住宅幸福密度Mod.rar` 中的中密度样本；同目录还有低密度/高密度单包）

| 指标 | 结果 |
|---|---:|
| DBPF 条目 | 8 |
| 类型 | 全部 `0x00B1B104` |
| Group | 全部 `0x48E0C000` |

一个 Property（instance `9EC474FF`）有 57 个字段、7 个 TGI 引用。可直接读出的行为参数包括：

- `0x0BA1A421..424 = 30`；
- `0x0BE225A0 = 1000`；
- `0x0C10838C = 90`、`0x0C10838E = 0.5`；
- `0x0E127533 = 0.2`、`0x0CE5E67F = (1.25, 1.25)`；
- Parent `0x00B2CCCB → 593B61D2`，并通过 `0x08E17ED7` 指向对应单位 Property。

这说明“快速增长/密度”不是新建筑模型，而是对现有住宅单位规则的字段级调整。低、中、高三个包分别只带一组相同结构的 Property，适合做最小可验证的 scalar overlay 案例。

### 代表案例 B：全套服务强化包

**`mod-airport.package` … `mod-waterplant.package`**（`强化Mod.zip` 与 `外网超强Mod，全面增强你的城市.zip` 重复包含）

15 个小包的条目数为 1–多条，主体仍是 `0x00B1B104`；例如 `mod-tradeport.package` 8,507 B，`mod-waterplant.package` 2,180 B。拆成服务粒度说明社区作者按“一个功能域一个 overlay”组织发布，方便启用/停用和排查冲突。

### 机制结论

1. Gameplay Mod 已经证明 OpenSCP 目前的 Property 解析、Parent 展平、Key/TGI 引用和确定性 overlay writer 是正确的目标抽象。
2. “改变模拟”通常只需要少量哈希字段；字段语义可以从值域、Parent 和被引用单位反推出，但哈希到人类名称仍依赖注册表/样本对照。
3. 同一功能可以拆成多个小 package，冲突检测应以 TGI 为粒度，而不是以文件名或 mod 名为粒度。

### 对 OpenSCP 的帮助

- 工作台可以先交付 **Property-only Gameplay Mod**：选定资源 → 展平 Parent → 修改白名单 scalar/array → 写出单资源 overlay → 重读对拍。
- UI 应同时显示“字段 hash、当前值、Parent 来源、Key/TGI 目标”，这样用户能看出改的是参数还是依赖。
- 对低/中/高住宅三个包做 golden fixture，可验证数组稳定性、浮点/整数保真和多包冲突提示。

## 3. Assets Mod：RW4 模型 + 入口 Property

### 反例：中央火车站不是“新资产入口”

**`Oppie_TrainStation.package`**（`大气的中央火车站Mod.rar`）

| 指标 | 结果 |
|---|---:|
| DBPF 条目 | 5 |
| `0x2F4E681B` RW4 | 4，约 1.50 MiB |
| `0x00B1B104` Property | 1，约 9.9 KiB |

这个包**不应作为“新资产入口闭环”样本**。它有 4 个 `0x2F4E681B` RW4 和 1 个 `0x00B1B104` Property，但入口 Property 的 instance `8357B87F` 是原有火车站单位身份的复用；其作用更接近替换原站点已经使用的模型链。包内复制的 Property/模型也不能证明作者理解了每一条 LOD、材质或功能字段。`0x00F9EFBB..BE` 引用 4 个 RW4，只能证明该入口需要一组模型资源，不能证明这是一个从零创建的新可放置建筑。

因此它在本报告中只保留为 **“原入口复用的 RW4 模型替换 / 经验型资产 Mod”**：适合验证模型替换、依赖闭包和无效资源复制检测，不适合用来设计“新资产创建向导”。

### 代表案例：车辆模型

**`Oppie_Train_VIRM_Regiorunner.package`**（`荷兰DD-IRM高速列车Mod.rar`）

- 13 条目：1 个 `0x00B1B104` 入口 Property + 12 个 `0x2F4E681B` RW4；
- 入口 Property `40E1C600:4FC6CD5B` 只有 810 B，但它承担把车辆身份接到 12 个模型资源的作用；
- 12 个模型合计约 16.3 MiB，说明“资产包大小”主要来自 RW4 payload，Property 只是连接图。

### 边界案例：桥/道路资产

**`Oppie_Bridge_Tram.package`** 与 **`Crossover All roads.package`** 都只有 `0x00B1B104` Property（分别 2 条、7 条），没有新的 RW4。它们更接近“已有模型/道路单位的功能参数和连接规则”，应归入 Assets/Gameplay 交界，而不能仅按文件名归为纯模型 mod。

### 机制结论

1. 对真正的新资产，最小闭环才是 `入口 Property → LOD/模型 TGI → RW4 资源`；中央车站说明同一结构也可能只是复用原入口做模型替换。
2. RW4 资源通常是新增 TGI；入口 Property 也通常是新增 TGI。它们验证了 overlay package 的“新增资源 + 引用闭包”模型。
3. 资产作者可以复用现有贴图/调色板/材质资源，不能假设每个 RW4 都携带完整独立纹理；依赖解析必须跨包进行。

### 对 OpenSCP 的帮助

- Assets 工作台应以 **资源图** 为中心显示 Property、LOD、RW4、纹理和注册入口，而不是把 package 平铺成文件列表。
- 构建校验至少需要：入口 TGI 存在、LOD 引用完整、RW4 type 正确、跨包依赖可解析、引用 instance 不碰撞。
- `Oppie_TrainStation` 适合作为“原入口复用/模型替换”固定样本，用来检测入口身份、原版 TGI 对照、LOD 链和无效复制；不适合作为新资产创建样本。
- `Oppie_Train_VIRM_Regiorunner` 适合压力测试大模型包、分页读取、模型依赖图和 glTF 导出；它不适合作为第一条写回案例。

## 4. 混合安装包对路径机制的证据

`外围区建造Mod.rar` 解压后同时出现：

- `SimCityData\*.package`：数据/资产类包；
- `SimCityUserData\EcoGame\SimCity-Scripts_*.package`：游戏规则脚本包；
- `SimCityUserData\Packages\1_Universal_Street_Script.package`：普通用户包；
- `OverplopModules\...`：安装器/回滚辅助目录。

这不是一个可直接装进单一目录的普通 mod。它确认工作台需要把 **安装目标路径** 作为 manifest 的一等字段，并在校验时阻止把脚本包、数据包和普通包混投。安装器的 `.bat`、回滚文本和重复包还说明社区发布物常常带有操作层，不应把这些文件误认为游戏资源。

## 5. 对当前机制理解的增量判断

### 已被社区样本进一步验证

- DBPF 条目与 TGI 是唯一可靠的资源身份；文件名只是发布层标签。
- `0x00B1B104` 是跨 Code/Gameplay/Assets 都会出现的行为与入口资源，必须做语义解析。
- 建筑解锁还有独立的规则/JSON 资源层（本样本为 `0x0A98EAF0`）；不能把所有菜单锁定都归因于建筑 Property 字段。
- `0x2F4E681B` 是资产模型主干；Property 通过 Key/TGI 数组连接模型、LOD、纹理和功能数据。
- Mod 组合的基本单位是“新增 TGI + 引用闭包”，而不是简单文件覆盖。
- EcoGame 与 Packages、SimCityData 的路径差异会改变加载语义。

### 仍未完全理清

- `0x08068AEB` 脚本容器的内部编译格式、代码与 Property 之间的精确运行时调用边界；目前能确认其存在和版本/规则关联，不能声称已还原脚本执行器。
- Property 哈希到所有人类语义的注册表覆盖率；像 `0x0AD6A725`、`0x0A7917C8` 的含义需要更多原版/社区成对样本和运行时日志。
- 资产入口 Property 中各数组（LOD、spawner、light、prop、路径）的完整字段语义与顺序约束；现有解析器能保真读取，通用安全写回仍需白名单。
- 游戏实际加载同 TGI 冲突时的排序/拒绝细节；“mod 只增不覆盖”已由现有引擎证据支持，但对基础包修改工具的行为不能套用到用户目录 overlay。
- Code Mod 与 Gameplay Mod 混合安装时，脚本包版本、规则 Property 和新增资源之间的依赖闭包还没有自动验证器。

## 6. 建议的工作台落地顺序

1. **Gameplay M1**：以 `middensityfast_growth.package` 做单资源 Property overlay，完成字段白名单、Parent 展平、重读对拍。
2. **Unlock M2**：以 `AllAchievementUnlock.package` 做单条 JSON 规则资源的结构化 diff 和 Tests 白名单替换；把删除的 39 条记录作为显式风险。
3. **Assets M3**：以 `Oppie_TrainStation.package` 做“原入口复用的模型替换”依赖图和完整性诊断，先不写 RW4。
4. **Assets M4**：以 4 RW4 + 1 Property 的样本验证确定性打包、跨包纹理解析和 glTF/OBJ 导出。
5. **Code M5**：把 `SimCity-Scripts_*` 识别为规则库包，先做版本/条目/TGI/语义 diff 和风险提示，再考虑任何写回。
6. **安装 M6**：把 `SimCityData`、`EcoGame`、`Packages` 目标路径纳入 `openscp.mod.toml`，对混合包输出安装计划和冲突报告。

本次抽样没有证明“可以安全自动生成脚本包”或“可以通用编译 RW4”；它证明的是 OpenSCP 可以围绕 TGI、Property、RW4 依赖和确定性 overlay 构建一条与社区已验证实践相容的工作流。

## 7. 摄像机专项：BOC 自由镜头线索与 Isometric Camera

用户提供的 BOC 图片是宣传/安装提示：它只说明“Free Roam Camera”在 BOC 的新菜单中有入口，并没有附带自由镜头 package，也没有给出修改字段。因此图片本身不能证明自由镜头如何突破 City Box；它的价值是确认社区把自由镜头作为独立工具/菜单功能发布，而不是普通建筑 Property。

本目录中的 `000_ONLINE_SUGC_Team1_20140718_1_0_Isometric_Camera.zip` 可以直接解析。ZIP 内只有一个 package，包含 2 条 `0x00B1B104`：

- `40410100:D9FEA4B9`：9 个字段，像是相机控制/曲线的辅助资源；
- `40410100:9E5A6813`：64 个字段，是主要等距相机参数资源。

将它与 `SimCity_App.package` 中同 TGI 的原版资源做语义 diff，得到：

```text
9E5A6813
  00C7C4FB       2500       → 2632
  00C7C4FC       20         → 45
  00C7C4FD       25         → 37.5
  00FC5228       [75 × 26]  → [150 × 25]
  00FC6857       [35 × 26]  → [20 × 25]
  00FC71FC       [14]       → [37.5]
  00FC7205       [80]       → [37.5]
  00FC78E7       [270,315,360,45,90,135,180,225]
                 → [37.5,127.5,217.5,307.5]
  0F17C26D       3          → 0

D9FEA4B9
  新增 00C7C4FC = 20
  新增 00FC6857 = [35 × 26]
  新增 00FC71FC = [14]
  新增 0F17C26D = 3
```

### 能确认的摄像机突破口

1. **近距离景深确实有数据侧入口。** `9E5A6813` 的 `0F17C26D` 从 3 改为 0，且相机曲线数组同时变化。这与“移除近距离景深/改变近景表现”的 Mod 说明相符，但仅凭哈希还不能把 `0F17C26D` 绝对命名为 DOF 开关；需要运行时或原生字段注册表确认。
2. **相机不是只有一个缩放值。** `00C7C4FB..FD`、`00FC5228`、`00FC6857`、`00FC71FC/7205/78E7` 组成多组标量和曲线/采样数组。Mod 同时改动它们，说明缩放、俯仰/旋转采样、近远距离阈值或插值曲线之间存在联动。只改最大缩放值很可能造成跳变或近景裁剪异常。
3. **City Box 限制不在这个 package 里。** 该 Mod 只有相机 Property，没有 `0x2F4E681B`、地形或城市模拟资源，也没有自由镜头 controller。现有零售反编译证据仍显示 XY 边界由原生 `cCameraControllerBase`/边界 clamp 处理；所以这个包能解除或改变景深/镜头曲线，不能证明它能突破 City Box。
4. **BOC 自由镜头应走原生控制器或脚本包路径。** 已有 BOC 安装包把主要改动放进 `SimCity-Scripts_*.package`，而图片所示入口是 UI 菜单。下一步应追踪该自由镜头实际 package/脚本中的 controller 切换、约束来源和命令，而不是从图片推断字段。

### 对 OpenSCP 的实际建议

- 在相机/菜单编辑器中新增“相机配置资源”视图，识别 group `40410100`，展示标量、数组长度、曲线采样和 `0F17C26D` 等未经命名的字段。
- 把 Isometric Camera 作为第一个 **相机参数 overlay fixture**：先支持语义 diff、备份、回滚和单字段实验；不要直接把所有哈希暴露成已命名的“焦距/景深/边界”选项。
- City Box 自由镜头需要单独的原生专项：确认当前零售 `RVA 0x259D80` 边界夹取函数的 bounds 来源，再只替换相机专用 bounds，保留高度采样、插值、远裁面和 LOD；不能通过扩大全局 `PlayArea` 解决。
- 运行时验证顺序应是：`0F17C26D` 单字段 → 相机曲线单组 → 近景景深观察 → 输入/缩放稳定性 → 最后才测试 XY 越界。这样可以把“景深参数”与“城市边界约束”分离。

### 当前结论边界

这个 Isometric Camera 包提供了一个明确的**数据侧相机参数突破口**，尤其是近景效果和相机曲线；它没有提供突破 City Box 的证据。自由镜头能否突破边界，仍需要找到 BOC Free Roam 的实际脚本/控制器改动或对零售原生 clamp 做受控运行时验证。

### 用户运行时观察（2026-10-21）

实机观察显示：安装 `000_ONLINE_SUGC_Team1_20140718_1_0_Isometric_Camera.zip` 后，镜头拉到最远时的活动范围比原版更远；但拉近后仍被限制在 City Box 内。

这条观察把相机参数拆成两个独立层：

- **远距/投影层**：很可能由 `00C7C4FB..FD` 和 `00FC5228/00FC6857` 等距离、采样曲线字段控制，Mod 可以扩大最远可视或可移动范围；
- **近距目标约束层**：仍由原生相机控制器的 XY bounds clamp 控制，至少没有被这个 package 改掉。

因此不能把“最远处看得更远”解释为已经突破 City Box。后续实验应分别记录：相机 eye distance、target XY、near/far clip、以及 clamp 前后的 target 值；只有 target XY 能越过 bounds，才算真正解除 City Box。

