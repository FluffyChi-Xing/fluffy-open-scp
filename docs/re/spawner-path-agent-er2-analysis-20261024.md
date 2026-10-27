# Spawner、路径、Agent 与 ER2 关系（2026-10-24）

> 目的：为 Property Editor / Mod Studio 的资产建模提供可复核的运行时边界。
> 本文只记录静态源代码和现有运行时资料的研究结果，不修改当前实现。
>
> 主要证据：`docs/source-code/SimCity-dev-ida.c` 的 dev beta 反编译（带符号）、
> `docs/re/unit-props-effects-spawners-paths.md`、
> `docs/re/props-effects-spawners-paths-engine-flow.md`、
> `docs/re/elementary-school-mechanism-20261022.md` 以及小学资源快照。

## 结论先行

1. `0x0E1BAC61/0x0E1BAC62` 在 `SC::cUnitModel` 中被装配为
   `mUnitLocations: vector_map<locationId, cSPTransform>`。它是 Unit 的具名局部位置表，
   不能仅凭列名证明“一个点生成一个人”或“一个点生成一群人”。点位可以被当作入口、出口、
   车辆/停车位、停机点、vignette 演员位置或其他脚本锚点。
2. 真正的交通 Agent 由 ER2/UnitRule 创建 transaction，再由 Transport handler 加入 Agent
   容器。`cTransportTraffic::AddTransaction` 对每个 transaction 调用一次 `AddAgent`，所以
   一个 transaction 的默认粒度是一个行人/车辆 Agent；多人组不是由 UnitLocation 数组长度
   直接决定的。
3. 行人组是运行时行为。`cTransportPedestrian::FormGroup` 把两个相邻 Agent 放入同一个
   `cPedestrianGroup`，`UpdateSingleAgentGroupState` 依据方向、距离、速度和 mood 决定是否
   结伴或聊天。因此一个建筑出现“一圈学生”可以是多个 transaction 在运行时组成的组。
4. 一个主 Unit 也可以通过 `mSecondaryUnits` 派生多个次级 Unit。每个次级项有 UnitEntry ID、
   局部 transform、`chance` 和 `connectToPrimaryUnit`；`CreateSecondaryUnits` 按概率创建，
   并可调用 `ConnectUnits`。学校的教师、教室、校车或附属功能更应先从这条链和 ER2/Transport
   资源查找，不能从 spawner 点数猜身份。
5. Path 点之间的线是实际的 `EcoPathSegment`。`CreatePathsFromProps` 读取点区间、索引、
   tangent 和 path entry，调用 `AddPoints`/`AddPath` 建立可供行人、车辆或其他 Transport
   handler 路由的曲线段。它不是只供编辑器显示的辅助线。
6. shader 不负责 spawner 或 path 的生成。小人外观属于 `cGraphicsInstancedSim`，使用
   body/head/outfit 组合、随机种子、速度和 scale 生成渲染数据；路径和点位由模拟/交通系统
   消费。建筑材质 shader 与这些位置表没有直接输入关系。

## 1. UnitLocation 与“spawner”名称的边界

### 1.1 引擎实际装配

`cUnitModel::FillFromProps`（反编译约 958480 段）构造 `kUnitLocationTable`：

```text
0x0E1BAC61  → Key 数组 unitLocationIDs
0x0E1BAC62  → Transform 数组 transforms
                 ↓
        mUnitLocations[Key.instance] = Transform
```

`GetUnitLocation`（`SC::cUnitModel::GetUnitLocation`，地址 `0x78C3B0`）按 ID 二分查找，输出
`modelTransform ∘ localTransform`；`GetUnitLocationLocal` 只返回局部 transform。源码中没有
把这两个列直接传给 `GB::cEcoGame::CreateUnit` 或 `CreateSecondaryUnits` 的调用。

这意味着现有 Property Editor 的“Spawner”标签是方便作者理解的兼容名称，底层数据模型应
改成“UnitLocation / 命名锚点”，再给它叠加用途标签。用途标签必须来自脚本、Transport、
Vignette 或运行时捕获，不能由 ID 哈希本身推断。

### 1.2 已观察到的消费者

`cUnitModel::GetUnitLocation` 的直接调用点包括：

- `SC::cVignette::DestinationIsAnActor`：判断位置是否是 vignette actor 目的地；
- `SC::cVignette::GetWorldLocation`：将 actor/sink 的局部位置转换到世界空间；
- UI/效果位置查询：例如 `0x141EED53` 位置用于效果或气泡偏移。

因此 UnitLocation 至少可以是 vignette/效果锚点。它也可以被 Transport 的脚本路径系统以
位置 ID 间接引用，但现有 dev dump 没有发现“按固定列自动生成 Agent”的函数。

## 2. 一个点是一个人，还是一群人？

### 2.1 明确存在的多 Unit 派生

`GB::cEcoGame::CreateSecondaryUnits`（地址 `0x60D740`）遍历
`unitEntry->mSecondaryUnits`，每项执行：

```text
IndexFromUnitEntryID(secondary.mID)
随机数 <= secondary.mChance
secondary.transform 后乘 primary transform
StartCreatingUnit → FinishCreatingUnit
secondary.mConnectToPrimaryUnit → ConnectUnits(primary, secondary)
```

因此一个建筑 Unit 可以带多个附属 Unit，它们能有不同的 graphics group、规则、资源 bin 和
路径。`mChance` 是是否创建的概率，不是“这个 spawner 生成几个人”的计数器。

### 2.2 交通 Agent 的粒度

ER2/UnitRule 执行到需要 Transport 的规则时，`GB::cEcoGame::CreateTransaction`（约
`0x60DD50`）根据 rule 的 TransportID、资源操作和目的地建立 transaction。

随后链路为：

```text
cEcoGame::CreateTransaction
  → cTransportBase::AddTransaction
       → 初始化 sink、transport entry、速度、初始位置
  → cTransportTraffic::AddTransaction
       → 检查 sink/transport 有效性
       → cTransportPedestrian::AddAgent(transactionIndex)
```

`cTransportBase::NumAgentsPerTransport` 与 `NumAgentsPerSink` 也是按 transaction 数统计，
这支持“一个 transaction 对应一个运行中 Agent”的粒度判断。它没有读取
`mUnitLocations` 的数组长度。

### 2.3 行人组是动态形成的

`cTransportPedestrian::FormGroup`（地址 `0x759850`）把两个 Agent 的 attach slot 加入同一
`cPedestrianGroup`，并计算平均位置/平均速度。调用者
`UpdateSingleAgentGroupState` 会先检查方向、side、mood、随机速度，再用 bonding radius
判断 walk-together 或 chat。

所以“学校一个点刷出老师和一圈学生”有三种候选解释，证据等级不同：

| 候选机制 | 当前证据 | 说明 |
|---|---|---|
| 多个 Transport transaction 后动态组队 | **高** | `AddTransaction → AddAgent` 与 `FormGroup` 已闭合；每个 Agent 仍独立创建 |
| 主 Unit 的 `mSecondaryUnits` | **高** | 可创建多个附属 Unit，并可 `ConnectUnits`；是否为教师/学生需查具体 UnitEntry |
| 单个 UnitLocation 自身带人数/agent 模板 | **未证实** | dev `cUnitModel` 只保存 ID+transform；没有直接 count/agent 消费证据 |

`0x0E715928/0x0E715929/0x0F0E2BF1` 在 dev 的 **Unit 非模拟属性**读取点是
`mBuildingVariation`、variation count 和 variation influence bin，不应当当作 lot spawner
人数列。发行包中含 `0x0E1BAC61` 的 lot 对这三列的静态命中也为零。旧知识页和
`lot_unit.rs` 中的兼容命名应视为待修订事项。

## 3. Path 点和点间连线的真实语义

### 3.1 Lot 属性到路径段

`SC::CreatePathsFromProps`（地址 `0x7E3A50`）的实际流程：

```text
0x0CAA6841  path pair 数组 (start, end)
0x0CAA680D  Vector3 点数组
0x0CB00ED8  与点等长的 tangent 数组
0x0CAA6832  点索引重映射
0x0CC8FE7A  path entry ID
0x0D9A4E54  path scope（缺省 local）
0x0D9AA2BB  road connection flag（缺省 none）
```

每一对 `(start, end)` 取含端点的点区间；引擎先把点加入 `GB::cEcoPathSet`，再调用
`AddPath(pathEntryIdx, numPoints, pointIndices, outSegmentIndices)`。每个相邻点对应一条
segment，tangent 被写入 segment 两端，供曲线位置、速度和距离计算。

### 3.2 Path entry 是行为入口

`pathEntryID` 通过 `gameScripts->IndexFromPathEntryID` 转为脚本 path entry。创建时会按
scope 过滤；通过后，`cTransportTraffic::AddSink` 读取 UnitPath，并按 path entry 将 sink
位置挂到路径上。行人侧 `GetPathEntryIndex` 会从 UnitPool segment 或 UnitPath segment 取
entry index，之后用它读取 sidewalk width、offset 等行为参数。

因此 path entry 决定“这段路径属于哪类入口/道路/人行道/车辆接驳语义”，而 pair/点/tangent
决定几何和连续运动。仅显示点线而不保留 entry/scope/roadConnect，资产编辑器无法保证
进出建筑的行为正确。

## 4. ER2、Unit、Transport 和路径的关系

推荐把关系理解成四层：

```text
UnitEntry / connected Units
  ├─ NonSim/Graphics：模型、UnitLocation、UnitPath、SimGraphics
  ├─ UnitRule / ER2：资源条件、applyCount、AgentTests、TransportID、成功/失败链
  └─ SecondaryUnits：附属 Unit、局部 transform、chance、ConnectUnits

UnitRule 满足条件
  → CreateTransaction(source, rule)
  → sink + transport entry
  → 一个或多个 transaction 进入 Transport handler
  → Agent 沿 EcoPathSegment 移动
  → GraphicsGame 根据 transport entry 选择模型/Sim/Impostor 外观
```

ER2 是行为和资源规则层；它不绘制 path，也不把 UnitLocation 直接变成 mesh。Transport
handler 负责把 transaction 变成 agent、选择 path pool、处理 sink 和运动。Unit 的图形层只
负责为 Agent 提供可渲染资源和外观参数。

## 5. shader 是否处理 spawner？

没有找到这种证据。dev 源码把小人渲染分成三种 `cGraphicsInstanced` 类型：

| type ID | 处理对象 | 证据 |
|---|---|---|
| `0x0D897503` | 普通 transport model instance | `TransactionCreate/UpdateTransports` 创建模型实例 |
| `0x0D897504` | `cGraphicsInstancedSim` | `GetSimData` 用速度、随机种子、body/head/outfit、scale 写入实例数据 |
| `0x0D897505` | agent impostor | `GetImpostorInfo` 产生公告板数据 |

`cGraphicsInstancedSim::GetSimData`（约 `0x785900`）和
`GetSimPaletteData`（约 `0x785AF0`）只决定外观组合、姿态标志和高度缩放；
`cGraphicsGame::UpdateTransports`（约 763700 段）把 Transport handler 返回的 Agent transform
写入渲染缓冲。代码中没有 spawner/path hash 作为 shader 参数，也没有 shader 参与 Agent
生成、分组或路线选择。

因此 Property Editor 可以把“模拟锚点预览”和“建筑材质 shader 预览”分开：spawner/UnitLocation
使用可标识的 gizmo，Agent 预览使用 SimGraphics/transport entry 资源；不要把 UnitLocation
坐标写进建筑材质或 decal shader。

## 6. 学校案例的当前结论

小学静态和运行时资料已经确认 `Student`、`TotalStudents`、`SpotsForKids`、
`StudentStudyingE2` 等是教育资源状态；这些资源的增减不能证明一个 spawner 的人数。
现阶段最稳妥的学校解释是：

1. 建筑 Unit 的教育 ER2 规则根据资源状态和 AgentTests/TransportOps 决定是否产生交通
   transaction；
2. 校车、学生、工作人员可能使用不同 TransportEntry 或 connected/secondary Unit；
3. 多个独立 Agent 到达校园后，行人 Transport 可因空间和速度形成动态 group；
4. Vignette actor 还可以在 UnitLocation 上演出脚本化的内部活动，这与真实交通 Agent 是
   两套系统，不能混为“一个 spawner 的人数”。

现有证据仍不足以给出“某个学校 location ID = 老师/学生圈”的确定映射。需要下一轮把
小学 UnitEntry、secondary Unit ID、path entry、TransportEntry、AgentTests 和 vignette
actor entry 做同一份索引，再以运行时 transaction/agent 日志对拍。

## 7. 对 Property Editor / Mod Studio 的直接要求

在实现下一版编辑模型前，应保留以下分层：

```text
UnitLocation
  id, localTransform, inferredUses[], evidence[]

Path
  entryId, scope, roadConnect, pointRange, remapIndices, points, tangents

Agent/Transport binding
  transportEntry, sinkEntry, ruleId, agentTests, sourceUnit, destinationUnit

SecondaryUnit
  unitEntryId, localTransform, chance, connectToPrimary

Visual provider
  model/impostor/sim, graphics key, body/head/outfit ranges, shader-independent
```

编辑器的验证顺序应是：

1. UnitLocation ID 在 Unit/lot 的 transform 表中闭合；
2. 每条 Path 的 pair、点、tangent、remap 数量和 path entry 闭合；
3. path entry 能在脚本表中解析，scope/roadConnect 合法；
4. ER2 的 TransportID、AgentTests、sink 和 UnitEntry 索引闭合；
5. secondary Unit 的目标 UnitEntry、概率和连接关系闭合；
6. 最后才把 Agent visual 绑定到模型/Sim/Impostor 资源。

这样可以避免把“看起来像 spawner 的位置点”错误地当成人数、角色或 shader 输入，也能让新
住宅的门口、车位、行人路径和功能规则分别校验。

## 8. 未闭合问题和下一轮证据计划

- 为小学和住宅分别建立 `UnitLocation ID → path entry/sink/transport` 交叉索引；
- 解码具体学校 UnitEntry 的 `mSecondaryUnits`，确认是否存在教师、学生或校车附属 Unit；
- 解码相关 ER2 UnitRule 的 AgentTests、TransportOps、TransportID 和 HandlerID；
- 记录运行时 transaction、Agent transform、transport entry 与目标 sink，确认人数来源；
- 区分 vignette actor 与真实 pedestrian Agent，确认“内部一圈学生”是否是脚本演出；
- 修订 `src/assets/knowledge/file-types.*` 和 `crates/sc-properties/src/lot_unit.rs` 的旧
  “spawner count/agent”注释前，先保留兼容字段并增加作用域标识，避免破坏已有包读取。

本轮没有把未验证的学校身份映射写入实现，也没有将 shader 解释扩展到 spawner/path。
