# 资产工作台小人运动路径（Path）便捷规划研究（2026-10-27）

> 命题：按 path 的真实引擎语义，设计资产工作台里"便捷规划小人运动路径"的
> 交互与数据模型。本文为研究+设计，不改实现。
> 语义基础：`docs/re/spawner-path-agent-er2-analysis-20261024.md`（消费链）、
> `docs/re/unit-props-effects-spawners-paths.md`、`docs/re/dev-dump-lot-placement-
> and-script-resources.md` §3.7（列族配方）。
> 新增实证：威利斯大厦 lot（`40E1C000:8D007745`）path 列族全量 dump +
> PathEntry 脚本表（G `40E0C400`）解析（2026-10-27）。

## 0. 结论速览

1. **路径的一等实体是"路径"，不是"点"**。引擎把 `(pairs, points, tangents,
   remap, entry[], scope[], roadConnect[])` 七个列族读成若干条 EcoPathSegment
   曲线；点只是路径的采样顶点。工作台今天按"扁平 pathPoint 单元"建模是
   便捷性不足的根源。
2. **真实作者模式（威利斯实证）**：22 点 = 8 对 = **4 条路径**——6 条"门口
   两桩短线"（门内一点 + 街边一点）+ 2 条多点人行道长跑；门口短线共用一个
   path entry，长跑用另一个；切线几乎全部 (0,−1,0)（朝街）；scope/roadConnect
   全部缺省。**便捷规划 = 让作者"挑门口 → 画到路边"，而不是手摆 22 个点。**
3. **path entry 是行为开关**：entry 不可解析（IndexFromPathEntryID → −1）整条
   路径被引擎跳过；entry 决定 sink 挂载与 sidewalk width/offset 等行为参数。
   编辑器若只画线不绑 entry，产物在游戏里是死线。
4. 工作台现状缺口：DTO 已有 `pathPairs` 但前端未消费；pathPoint 无 gizmo
   （编辑层直接跳过，点位不可拖）；无 entry/scope/roadConnect 任何 UI；
   折线按 pointIndex 排序单条连接（多路径会串线）。

## 1. 语义链（引擎侧，全部有锚点）

### 1.1 列族 → 路径段（`SC::CreatePathsFromProps`，dev 0x7E3A50）

| 列族 | 语义 | 缺省/校验 |
|---|---|---|
| `0x0CAA680D` Vector3[] | 采样点（威利斯 22 点，z 几乎全 0；商店 lot 见 z=0.94 坡道点） | 与切线**等长否则整批返回 1** |
| `0x0CB00ED8` Vector3[] | 段两端切线（曲线位置/速度/距离计算输入） | 威利斯 22 条中 19 条 ≈(0,−1,0) 朝街 |
| `0x0CAA6841` Int32 对 | `(start,end)` 含端点的点区间；**对数=路径数** | 奇数个=数据错误 |
| `0x0CAA6832` Int32[] | 点索引重映射（威利斯=恒等 0..21） | 可选 |
| `0x0CC8FE7A` Key[] | **per-path** entry ID（下标=路径序号 j） | 不可解析 → 该路径跳过 |
| `0x0D9A4E54` | per-path scope（缺省 local；被拒路径只推进切线索引） | |
| `0x0D9AA2BB` | per-path road connect（缺省 none） | |

### 1.2 PathEntry（G `40E0C400`，脚本表）——行为入口

威利斯门口短线 entry `418A18B1` 实测：

```text
0x00B2CCCB parent → 48E0C400:DE38BA78（entry 模板链，注册表无名）
0x0D6498E8 允许通行类型 Key[] → E0CD3122 = "Pedestrians"（注册表可读名！）
0x0BD81A2D..31  道路几何参数族（Key[]/int[]/float[]×2；BoC 取证同族，
                对应 sidewalk width/offset/车道数类行为参数）
```

→ **entry 选择器的显示名 = 允许通行类型解析名**（Pedestrians/Vehicles/…），
   辅以父模板链与几何参数摘要；条目枚举 = 已加载脚本包 G `40E0C400`。

### 1.3 消费链（谁在小人身上用这些线）

ER2/UnitRule → CreateTransaction → Transport handler → Agent 沿 EcoPathSegment
移动；`cTransportTraffic::AddSink` 按 path entry 把 sink 挂上路径；行人
`GetPathEntryIndex` 取 entry 后读行为参数。**UnitLocation（门口命名锚点）与
path 是两套数据**，门口短线只是作者习惯让路径起点落在门口附近。

## 2. 工作台现状盘点（代码钉死，2026-10-27）

| 能力 | 现状 | 位置 |
|---|---|---|
| pathPoint 放置 | ✅ 单点拾起（预设拖拽），无切线、无路径归属 | `useWorkbenchAsset.placeUnit` pathPoint 分支 |
| 折线渲染 | ⚠️ 按 pointIndex 排序**单条**连接（"pathPairs 语义未定，先 best-effort"）——多路径会串线 | `PropertyEditorViewport.vue:2282`、`unitGizmos.ts:246` |
| 点拖拽 | ❌ pathPoint 无 transform，gizmo 系统直接跳过（`unitGizmos.ts:156`） | |
| pairs 消费 | ❌ DTO 有 `pathPairs`（`tauri.ts:682`）但恒空/未用 | |
| entry/scope/roadConnect | ❌ 无任何 UI 与序列化 | |
| 检查器 | ❌ Inspector 无 pathPoint 分支 | |

## 3. 设计：以"路径"为一等实体的便捷规划

### 3.1 编辑态数据模型

```text
Path (编辑态一等实体)
  id, entryId?, scope=local, roadConnect=none
  points: [{ position: [x,y,z], tangent: [x,y,z] }]   // 有序

序列化 ↔ lot 列族（roundtrip 即校验）：
  points(拼接) → 0x0CAA680D / 0x0CB00ED8（等长）
  各路径点数前缀和 → 0x0CAA6841 (start,end 含端点)
  可选 remap → 0x0CAA6832；entry[] → 0x0CC8FE7A；scope/roadConnect → 0x0D9A4E54/0x0D9AA2BB
```

承载方式二选一：①schema 文档（openscp.lot-asset）增加 `paths` 段 +
`pathPoint` 兼容互转（推荐——蓝图/Monaco/保存三方同源已由 schemaDocument
承接）；②扩展 pathPoint 单元加 `pathId` 字段。①改动面更大但模型干净。

### 3.2 交互（按便捷性排序）

1. **连续画线（Draw 模式）**：工具条新增"路径"工具；点击地面依次加点，
   最后一点→光标实时橡胶带预览；Enter/双击收笔、Backspace 撤点、Esc 取消。
   一次收笔 = 一条 Path；entry 默认沿用最近使用值。z 吸附 0（可调，坡道点例外）。
2. **门口短线模板（核心便捷点，威利斯模式直译）**：选中 UnitLocation（门口
   锚点）→ 一键"生成门口路径"：起点=门口、终点=吸附到已有路径最近点或
   lot 边缘朝街点，entry 预选该门口类型的常用值。作者不该手摆门口桩。
3. **点拖拽 + 切线自动重算**：给 pathPoint 实现位置 gizmo（补 `unitGizmos`
   的跳过分支）；拖动/增删点后，端点切线自动指向邻点方向、长度=段长
   （威利斯切线量级与段长相关的实证），并保留手动覆盖。
4. **Path 检查器**：entry 选择器（枚举已加载脚本 G `40E0C400`，显示
   allowlist 解析名如 "Pedestrians" + 几何参数摘要）、scope 下拉、
   roadConnect 开关、点位列表（点选联动视口高亮）。
5. **校验面板（编辑闭环的硬门）**：点/切线等长；pairs 端点闭合；**entry
   缺失或不可解析 → 红色错误**（引擎整条跳过）；孤立点（不属于任何路径）
   警告；scope 与 allowlist 冲突提示。
6. **可视化分级**：折线按路径分色（或按 entry 类型分色）、行进方向箭头、
   切线手柄微件（默认收起）；选中路径高亮 + 其余淡出。

### 3.3 分期

- **P1 数据与校验**：schema 文档 `paths` 段 + 列族 roundtrip（含
  0x0CC8FE7A/0x0D9A4E54/0x0D9AA2BB 三列——PE 旧 F5 缺口一并关闭）+ 校验
  面板 + 多路径分色渲染（替换 best-effort 单折线）。
- **P2 绘制效率**：Draw 模式 + 点拖拽 gizmo + 切线自动重算 + Draw 的
  橡胶带预览。
- **P3 语义便捷**：entry 选择器（40E0C400 扫描 + allowlist 解析名）+
  门口短线模板 + UnitLocation 吸附。

## 4. 边界

- 运动行为（速度/成组/ mood）在 Transport handler 运行时决定，编辑器
  不承诺预览（`cGraphicsInstancedSim` 外观与路径无关）；
- entry 的几何参数族（0xBD81A2D..31）语义未逐字段命名——选择器先显示
  allowlist 解析名，参数编辑延后；
- 树 impostor 类"沿路径摆树"等装饰用法（若存在）未考证，本期只对齐
  Transport 行人语义。

## 5. 证据

威利斯 lot 全量 path dump（prop_hash_scan 0x0CAA680D/41/32/0CB00ED8/
0x0CC8FE7A，SimCity_Graphics.package 40E1C000:8D007745）；PathEntry
418A18B1 prop_dump（EcoGame 40E0C400）；allowlist 名 "Pedestrians"
（database_main.s3db E0CD3122）；工作台缺口：PropertyEditorViewport.vue:2282、
unitGizmos.ts:156/246、tauri.ts:682、useWorkbenchAsset.ts:956。
