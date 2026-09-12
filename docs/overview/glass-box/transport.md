# SimCity (2013) GlassBox > 深扫笔记（2026-09-12）。范围：SC_cTransport*（14 文件）、SC_cPathPool/SC_cVehiclePathPool/GB_cPathRouteInfo、SC_cAgent/cVehicleAgent/cRailAgent/cPedestrianAgent、GB_cTransportShared、SC_cGraphicsPath*、SC_cPathIntersection/CongestionTuning、SC_cRoadManager、SC_cGameAppMode（共 26+ 文件）。
> 置信度：★★★ = 多处互证直接证据；★★ = 结构/常量推断；★ = 单点证据。偏移为二进制内函数地址（FUN_ @ 0x…）。

交通系统（SC::Transport）逆向笔记

> 范围：`D:\rust\packages\fluffy-open-scp\docs\source-code` 下的反编译伪 C（IDA 风格）。
> 置信度标注：★★★ = 有多处相互印证的直接证据；★★ = 结构/常量推断，语义为合理猜测；★ = 单点证据，字段含义推测。
> 所有偏移均为二进制内函数地址（`FUN_ @ 0x…`）。反编译代码几乎无符号名，vtable 调用一律以 `+0x…` 偏移表示。

---

## 0. 文件清单与地址域映射

`ls | grep -iE "transport|traffic|vehicle|agent|path|commute|travel"` 命中的 26 个核心文件 + 周边文件：

| 文件 | 行数 | 主要函数地址域 |
|---|---|---|
| SC_cAgent.c | 21 | 0x007087b0 |
| SC_cVehicleAgent.c | 77 | 0x007234d0 |
| SC_cRailAgent.c | 86 | 0x00894a80 |
| SC_cPedestrianAgent.c | 93 | 0x007392f0 |
| SC_cTransportBase.c | 630 | 0x00894xxx–0x00896xxx（池底层公共代码） |
| SC_cTransportPipe.c | 845 | 0x00757xxx–0x00760xxx |
| SC_cTransportPipe_cBasePool.c | 140 | 0x00758ab0（+大量 purecall） |
| SC_cTransportPipe_cPool.c | 476 | 0x0075axxx–0x0076xxxx |
| SC_cTransportPipe_cDirectionPool.c | 435 | 0x00757xxx、0x0075bxxx–0x0075dxxx |
| SC_cTransportTraffic.c | 3732 | 0x00835xxx–0x0083bxxx |
| SC_cTransportVehicle.c | 5129 | 0x0071bxxx–0x0073xxxx + 0x00836xxx–0x0083bxxx |
| SC_cTransportSignal.c | 639 | 0x0074f4xx–0x0074f9xx |
| SC_cTransportRail.c | 2261 | 0x0070axxx–0x0071axxx |
| SC_cTransportAirplane.c | 2622 | 0x00760xxx–0x00768xxx |
| SC_cTransportHelicopter.c | 1686 | 0x00752xxx–0x00757xxx |
| SC_cTransportPedestrian.c | 5127 | 0x00739xxx–0x0074cxxx |
| SC_cTransportRadial.c | 1001 | 0x0074dxxx–0x0074fxxx |
| SC_cPathPool.c | 624 | 0x00c25xxx–0x00c26xxx（GB 核心） |
| SC_cVehiclePathPool.c | 505 | 0x00896xxx–0x00897xxx（与 GB 核心共用部分函数） |
| GB_cPathRouteInfo.c | 624 | 与 SC_cPathPool.c 内容大量重叠（0x00c26bxx/0x00c25exx 同地址） |
| GB_cTransportShared.c | 698 | 0x00c20xxx–0x00c21xxx |
| SC_cGraphicsPath.c | 976 | 0x007f3xxx–0x0080xxxx |
| SC_cGraphicsPathNetwork.c / _Part.c | 98/77 | 0x007f6c50 / 0x00805790 等 |
| SC_cPathIntersection.c | 198 | 0x007fxxxx–0x0080xxxx |
| SC_cPathCongestionTuning.c | 50 | 0x00830240、0x00775af0 |
| SC_cRoadManager.c | 959 | 0x006c3xxx–0x006d1xxx |
| SC_cPathPool.c 与 GB_cPathRouteInfo.c 开头函数完全同地址（0x00c26ba0/0x00c26b00/0x00c25ed0） | — | 说明二者是同一族路径池类的不同翻译切片 |

关键外围证据文件：
- **SC_cGameAppMode.c**（0x787–0x795 行）：交通"模式"注册表，带可读字符串（见 §7）。
- **SC_cZombieNight.c**：也引用 `0xdc0d28f`（agent 资源过滤常量），僵尸模式复用交通管线（★★）。

---

## 1. Agent 基本形式（cAgent / cVehicleAgent / cRailAgent / cPedestrianAgent）

### 1.1 内存布局（由构造函数偏移写序还原）

**cAgent 基类**（SC_cAgent.c `FUN_007087b0 @ 0x007087b0`，★-★★）：

```
+0x34/0x38/0x40 : int   = 0              // 三个计数/游标
+0x44/0x48      : id    = -1             // 成对 id（疑似 from/to 管道或链表前后向）
+0x4c/+0x50     : id    = DAT_00dfea04/DAT_00dfea08（默认资源句柄，来自只读数据）
+0x54/0x60      : id    = -1
+0x58           : int   = 0
+0x5c           : byte  = 0              // 布尔标志
```
派生类构造全部以同一段 11 行"序言"开头（仅 DAT 常量不同），可确认为公共基类初始化（★★★）。SC_cTransportPipe_cBasePool.c 中的 11 个 `purecall`（0x00a90870）表明该族有纯虚接口类（★★★）。

**cVehicleAgent**（SC_cVehicleAgent.c `FUN_007234d0 @ 0x007234d0`）：
- 序言后：`+0x9c` 起 12 个 `uint16 = 0xffff`（12 项 id 数组，-1 填充，★★）；
- `+0xd0 = +0xcc`（8 字节 double/int64 复制）；`+0x180` 起一组向量/默认值；`+0x1c8/+0x1ca = 0xffff`；`+0x1c4 = 1`；`+0x1d4 = 8`（★）。

**cRailAgent**（SC_cRailAgent.c `FUN_00894a80 @ 0x00894a80`）：
- 以 `stride 0x120` 遍历 `+0x9c..+0xa0` 数组逐个析构（0x120 = 288 字节/元素的列车编组记录，★★）；
- `+0x64` 处句柄若非 -1 则经 vtable+0x18 释放后置 -1；重置 `+0x68..0x90`、`+0x150..` 一族（★）。

**cPedestrianAgent**（SC_cPedestrianAgent.c `FUN_007392f0 @ 0x007392f0`）：
- 序言后写入大量 DAT 浮点：`+0x70..0x7c` 两组 vec3（min/max 对）、`+0x98..0xb0` 三对 vec3；
- 构造 8 组栈上 vec3 后调用 `FUN_007355e0`（用角点构造包围盒/变换，★★）——行人 agent 携带空间包围盒。

### 1.2 生命周期与状态机

- 本目录内**没有**显式 agent 状态机枚举；agent 的"状态"由**管道池槽位状态**承载（见 §2）：槽位 `+0x30/+0x34` 是否有效、`+0x38` 是否为 -1、方向池中是否有记录，即"在路段上/等待/离场"的隐式状态（★★）。
- 生命周期钩子：进入边 `FUN_0075f3f0`（SC_cTransportPipe.c）→ 事件入队；离开边 `FUN_0075cc60` → 移除槽位（★★★）。
- `FUN_0083a830`（SC_cTransportVehicle.c @ 0x0083a830）：AddAgent 入口——`FUN_00896090` 建槽；若槽 `+0x30` 或 `+0x34` < 0（两个方向池注册失败）→ `FUN_00895c00` 回滚并调用 network vtable+0x1ec（拒绝上报）；否则把 pipeIdx 压入 `+0x15..0x17` 待处理向量（★★★）。

---

## 2. Pipe 池体系（cTransportPipe / cBasePool / cPool / cDirectionPool）

### 2.1 三层结构

**cBasePool（抽象基座）**：SC_cTransportPipe_cBasePool.c `FUN_00758ab0 @ 0x00758ab0`
```
+0x04 : network 对象指针
+0x08 : network->vtable+0x34()   // 网络 id/版本句柄
+0x0c : network->vtable+0x17c()  // 映射表对象（下称 map）
+0x10..0x2f : 32 字节 key/GUID（param_4 原样拷贝）
+0xb0 : param_3（key）
之后 FUN_00758530(id, key) 完成注册
```
（★★★，纯虚函数表证明这是被 cPool/cDirectionPool 共用的基座。）

**cPool**（SC_cTransportPipe_cPool.c `FUN_00758d10 @ 0x00758d10`）：
- 基座构造后按 map 的两条向量容量 `resize`：
  - 边数 = `(*(map+0x70) - *(map+0x6c)) >> 2` → vtable+0x20 预留；
  - 点数 = `(*(map+0x14) - *(map+0x10)) >> 2` → vtable+0x2c 预留。
- 槽位记录 **stride 0x5c**（下述"路径槽"）。（★★★）

**cDirectionPool**（SC_cTransportPipe_cDirectionPool.c `FUN_0075ba00 @ 0x0075ba00`）：
- 基座构造 + `FUN_008972a0` 创建两个 0x14 字节记录（`+0x268/+0x278` 传入对儿）；再两次 `FUN_00453220(边数)` 预留两个向量（★★）。
- 0x14 记录 = `{ int 当前计数; vector<{句柄, 值} 8字节> }`——增删见 §2.3（★★）。

### 2.2 路径槽（0x5c 字节）——agent 在管道上的驻留记录

由 SC_cTransportBase.c `FUN_00896090 @ 0x00896090`（获取槽位）与读取方（SC_cTransportPipe.c `FUN_00894ff0 @ 0x00894ff0`）交叉还原：

```
+0x30 : int  方向池A索引（FUN_00c21390 注册返回，★）
+0x34 : int  方向池B索引
+0x38 : int  = -1（清理/暂存标记，FUN_0075cc60 置 -1）
+0x3c/+0x40/+0x44 : float vec3 位置（来自源对象 +0xb4..0xbc）
+0x48/+0x4c/+0x50 : float vec3 方向（读取时按 1/sqrt 归一化，★★★，FUN_00894ff0）
+0x54 : float 权重/耗时（若 agent 索引 == 尾元素则取 (+0xa4 + +0xa8)，否则 +0xa4；负值截 0，★★）
+0x58 : float 进度/时间片（FUN_00896090 中按剩余计数比例计算，★）
```
辅助查询（SC_cTransportBase.c）：`FUN_00894de0`/`FUN_00894e20` 分别统计 `+0x34 == idx` / `+0x30 == idx` 的槽位数 —— 即"某管道正向/反向当前 agent 数"（★★★）。

### 2.3 方向池的增删（容量语义）

SC_cPathPool.c / GB_cPathRouteInfo.c（同地址函数，0x00c26xxx）：
- `FUN_00c26ba0 @ 0x00c26ba0`（AddAgent）：
  `增量 = min(param_5, -当前计数)` → 计数 += 增量，总数(+0xc) += 增量，条目数(+0xd)++；再 `FUN_00829050` 把 `{句柄, 值}` **有序插入** 8 字节向量；`FUN_00896ef0` 上报；vtable+0x14 通知该边变更。
  → **容量由调用方 clamp**，方向池只是"agent 名册 + 计数"（★★★）。
- `FUN_00c26b00 @ 0x00c26b00`（RemoveAgent）：按句柄线性查找（stride 8），memmove 收缩，计数按 cap 反向钳制递减（★★★）。

### 2.4 与道路网络的映射

- SC_cTransportVehicle.c `FUN_0083b530 @ 0x0083b530`（交通侧的同步/脏检查，★★★）：交通池为每条边缓存 0x38 字节记录 `{起点node(+0), 终点node(+4), 起版本(+8), 终版本(+0xc), mapIdx(+0x10), 资源id(+0x14)}`，与 network 的边表（+0x6c 向量、边记录 +0x7c、stride 0x1c，其中 +0x18 为 mapIdx）比对：
  - 两端与版本都一致 → vtable+0xdc（原地重建边）；
  - 起点变 → vtable+0xd4（脏节点）；
  - 旧端失效 → `FUN_0070bd40` 销毁记录（+0xfeeeec）。
  → **管道槽位按 (node,node) 对惰性重建，网络不动则管道不动**。这是"道路即管道网络"的直接实现证据。
- `FUN_00894c80 @ 0x00894c80`（SC_cTransportPipe.c）：`return (param_2 != 0xdc0d28f) ? 0 : param_1;` —— 以资源 id `0xdc0d28f` 过滤 agent；该常量出现在全部 transport 文件与 SC_cZombieNight.c（★★，疑为"agent/单位"类资源 id）。

### 2.5 Pipe 主更新与信号/路口对象环

SC_cTransportPipe.c `FUN_0075fce0 @ 0x0075fce0`（构造末段）：`+0x124 = vtable+0xc0(0x767285c0)` —— 按 hash 取一个常驻对象；`+0x90/+0xc0 = -1`（两个环游标）。

`FUN_0075fdc0 @ 0x0075fdc0`（dt 更新，★★）：
1. 若 `+0x120 != *(map+300)`（版本变）→ `FUN_0075f910` 重建；
2. 两个对象环（游标 `+0x90`、`+0xc0`，容量 `FUN_00757bd0()/FUN_00757c70()`）：游标++取模、`FUN_00757c20/00757cc0` 取对象、`vtable+8(100)` —— 对象 tick(100)；
3. `+0x124` 指定对象恒定 tick；
4. 遍历两组**分块槽位数组**（stride **0x1e4**=484B 与 **0x288**=648B，FUN_0075d4d0 析构可证）：每块先 `vtable+0x20()`、再 `vtable+0x24(+0xc, &0xd4, &0xe4, &0xc4)`；
5. `+0x114 += dt`，`+0x11c = ROUND(+0x114) & 3` —— **mod 4 相位计数器**；
6. 二次遍历同两组数组：读 `slot+0x1e`（管道 idx），有效则 `FUN_007595b0(idx)` → `vtable+4(相位)`；再 `FUN_0075e7d0(slot, dt, 方向)`。

→ 推断：cBasePool 的两个分块数组是**路口/信号灯对象池**（两种尺寸 = 普通路口 vs 大路口），mod-4 相位即四相位信号周期（★★）。FUN_00758970（事件类型==6 时 `FUN_00757cc0/00757c20` + `FUN_00758530`）是信号对象随管段拆除的释放路径（★★）。

### 2.6 agent 进/出边的事件流

- 进：`FUN_0075f3f0 @ 0x0075f3f0` —— `FUN_00896090` 建槽；若 `network->vtable+0x124(obj)` 命中，`vtable+0x128(obj, 槽位+0x3c)` 取值并 `FUN_007a9a10(+0x48, …)` 注册到信号对象；槽 `+0x30 = 管道idx`；把 `{pipeIdx, -1, 0, +0x54<<32}` 压入 `FUN_0075aea0` 事件向量 → `FUN_0060c530` 派发（★★）。
- 出：`FUN_0075cc60 @ 0x0075cc60` —— `槽+0x38 = -1`；`FUN_0075c2e0(池对象, pipeIdx)`；`FUN_00895c00` 删槽（★★）。
- 下一跳：`FUN_00759c60 @ 0x00759c60` —— 管道对象内**三级后备队列** `+0x4a/+0x46/+0x42`（各为 vector），依次取首个非空者调 vtable+0x1c —— agent 过路口时从预排好的转向队列取下一段（★★）。
- 接纳判定：`FUN_00759d10 @ 0x00759d10` —— 遍历 map+0x40 查到的段表（+0x7c/+0x80），每段经 vtable+0xc0 得管道 idx，池中有效则调管道 vtable+8（是否接纳）；若记录 bit30-31 置位再走连通表后备（★★）。

---

## 3. PathPool / VehiclePathPool / GraphicsPath

### 3.1 核心路径池（GB 层，0x00c25xxx–0x00c26xxx）

SC_cPathPool.c（与 GB_cPathRouteInfo.c 同地址函数）：

- **拥堵传播** `FUN_00c25ed0 @ 0x00c25ed0`（★★）：
  `FUN_006094a0(边, 8, 邻表)` 取至多 8 个邻接边（编码 = `idx<<1|方向`）；对每个邻居：
  - `资源id = *(network+0xd0 + mapIdx*0x18)`（< 0x100 才有效），查 `param_1+0x38` 起的 **资源掩码位图**（≤ 0x100 类资源）——只统计关心的资源类型；
  - 容量 = `*(方向表[0/1] + idx*4) * *(param_1+0x80)`；非 per-lane 模式（+0x7c==0）时 `值 = (阻塞阈值(+0x6c) - 计数) * 系数(+0x70)`，否则对队列逐项 `条目*容量`（或 `容量-条目`）取 min；
  - 同时考虑**对向边当前值 + ε(DAT_00cfcfbc)**；
  - 取 min 写入 `边+0xc`（目标值），`边+8` 为现值；差值超 ε → FUN_005622a0/005622f0（变更通知），否则 FUN_00562260。
- **路口择向** `FUN_00c26410 @ 0x00c26410`（★★）：先在排队项中找最近可达（4 路展开扫描）；否则比较两条出边的 `当前值(+8) + 候选值` 与对向的对应值，再加 `*(param_1+0x74) * LCG` 抖动（种子 `+0x68`，`种子 *= 0x278dde6d`，★★）→ 返回 0/1。**拥堵 + 随机 = 转向决策**。
- **邻边收集** `FUN_00c266a0 @ 0x00c266a0`：按资源掩码过滤后输出两平行数组 `{边编码, 边+8 现值}`。
- **逐 tick 推进** `FUN_00c25e00 @ 0x00c25e00`：弹出 pending 队列（+0x16/+0x17）中 16 字节项，取 `min(item[2], item[3])` 逼近目标，调 vtable+0xc(边) 推进/销账（★★）。

### 3.2 车辆路径池（SC_cVehiclePathPool.c，0x00896xxx）

- `FUN_008968a0 @ 0x008968a0`：同 FUN_00c25e00 的双槽版本；溢出（上限 DAT_00d26dd8）时调 `FUN_00896440(对向边, idx, 方向)` —— **超员向对向车道溢出**（★★）。
- `FUN_00896a00 @ 0x00896a00`：沿 `FUN_006094a0(边,4,…)` 4 邻接传播；map+0x7c 的边记录 stride 0x1c `{+0x00 边A, +0x04 边B, …, +0x18 mapIdx}`（★★★，与 §2.4 一致）。
- `FUN_00896240 @ 0x00896240`：**方向裁决**：若 `*(+0xec + 1 + idx*2)` 禁行标志置位 → 常量 `+0x100`；否则比较两份代价表 `+0xcc/+0xdc` 的 `+8+idx*0x10` → 0 / 1 / 2（小于/大于/相等）（★★）。
- `FUN_008962f0 @ 0x008962f0`：收集未被禁行的出边及代价（★★）。
- `FUN_00897430 @ 0x00897430`：扩容 2 字节标志数组并逐项初始化（★）。

**拥堵计数来源**：SC_cTransportVehicle.c `FUN_007254d0 @ 0x007254d0` —— 遍历车辆路径池记录（0x10 步进）与待处理表（+0x24c），把每个有效槽（`槽+0x30`、`+0x34` 均 ≥ 0）计入 `+0xb8`（总数）与 `+0xbc + 方向*0x14` 的 `{count, vector<槽idx>}` 桶（★★★）。这就是 §3.1 拥堵值的输入。

### 3.3 GraphicsPath 三件套

- **SC_cGraphicsPath.c `FUN_008021f0 @ 0x008021f0`**（约 900 行，路面网格重建）：校验资源（`+0x3a4/+0x3a8` 对 network+0xc0/+0xd0）与 `+0x128` 资源位图；逐角点（×2）遍历路口臂表（`(0x2948-0x2944)/0xc` 项，12 字节/顶点）：中点 = `端点均值×0.5`（DAT_00da307c=0.5，★★★），叉积法线（角点偏移常量 DAT_00fec580..88），归一化、点积着色，`FUN_00706f50` 压入顶点缓冲 —— **道路可视网格直接由交通管段几何生成**（★★）。
- **SC_cPathIntersection.c `FUN_008046d0 @ 0x008046d0`**（路口求解器）：臂数 `< 5` 限制（`0x2948-0x2944)/0xc < 5`）；`FUN_008780c0` 求解两遍（第二遍带属性 `0xe2107072` 与由 +0x2a94 零值构建的空闲位掩码）；输出 **0x54 字节/臂** 记录，以 0x58 步长拷贝到 `+0x30` 数组；`+0x390/+0x394` 覆盖项；臂记录 +0x15 处放"是否最后一件"旗标（★★）。
- **SC_cGraphicsPathNetwork.c `FUN_007f6c50 @ 0x007f6c50`**（信号灯渲染）：`状态 & 0x10` → 取 0xc0376a3 对象；低 4 位 ==1 → (a,b)=(1,0)，==2 → (0,1)，`FUN_007f53f0(状态, 参数, 轴…)` —— 把信号相位映射到两个灯头（红绿轴）网格（★★）。
- 三者关系：PathIntersection 生成路口几何 → GraphicsPath 拉丝成网格 → GraphicsPathNetwork 只负责信号灯部件。**渲染层不参与交通逻辑，只是管段几何的消费者**（★★）。

### 3.4 RoadManager（SC_cRoadManager.c）

`FUN_006cc540 @ 0x006cc540`（构造）：
- 绑定 network（+0xc/+0x10，map 存 +0x148）；
- 读属性映射表：`0xd688453`（type 0x20 = map）+ 列 `0xd688467`、`0xd688474`（type 0xd = uint32[]）→ 建成 `+0xb0/+0xb4` 有序 map（12 字节项 `{key, v1, v2}`）—— 疑为"道路类型 → 参数"表（★★）；
- 属性 `0xe27bd82`、`0xe27bd86` → 全局 DAT_00dfcabc / DAT_00dfcac0；
- 按**边数**分配 `+0xa0` 数组（stride 0x50），逐边 `FUN_00609570(idx, &desc)` → `FUN_006c4ac0` 拷贝边描述；
- 第二组属性 `0xe04085c`(float)、`0xe040850/0xb/0x4/0x7`(map)、`0xef518b9` → `+0xe8` 向量（24 字节项 `{key, 4 值, 资源}`）（★★）。
- `FUN_006cfee0 @ 0x006cfee0`：`+0x110 = vtable+0x6c(0xe38de978)` 等运行期对象获取；`FUN_006d0680` 为对称析构（★）。

### 3.5 杂项

- SC_cPathCongestionTuning.c：仅 `FUN_00830240`（EA::RefCountTemplate 析构）与增减引用——**拥堵调参本体是纯数据（GCT 属性），代码无表**（★★★）。
- SC_cGraphicsPathPart.c `FUN_00805790`：部件重建（先清 `+0x33c/+0x348` 三级缓冲——按 `+0x390` 的 0/1/2 选择，★★）。

---

## 4. 交通算法：车辆怎么跑、信号怎么堵

### 4.1 主循环骨架（SC_cTransportTraffic.c / SC_cTransportVehicle.c 共享）

`FUN_00838e20 @ 0x00838e20`（Traffic 与 Vehicle 文件同地址，★★★）：
1. `+0x4d` 标志 → `FUN_006cf690`（视觉刷新挂钩）；
2. 遍历 `+0x15..0x16` 待销毁管道表：`network->vtable+0x1d4(pipe)+0x10` → 对象 `vtable+0x88` 释放；
3. 若 network 计数（`+0x2c` vs `+0x30`+动态值）变化 → `vtable+300()` **全量重建**；
4. `FUN_00835f20`（边统计刷新，内部即 FUN_007254d0 一族）；
5. `vtable+0x94(dt)`、`vtable+0x9c()` —— 派发给具体模式的 step。

`FUN_0083b530 @ 0x0083b530`（懒重建，见 §2.4）+ `FUN_0083aeb0`（teardown：摘除静态回调 `LAB_00835f10/00837c40`，收缩各向量，★★）。

### 4.2 车辆沿路径移动（插值/速度/间距）

- **车道几何** `FUN_00721300 @ 0x00721300`（★★）：每边 0x38 缓存记录 + 0x4c 渲染记录；读地块字节属性 `0xee147fb / 0xdb62aad / 0xdb74e1e / 0xf9b2e0f` 打包进 `+0x48` 低 4 位（疑为单向/车道数/停车线等标志，★）；对每边取两端点 vec3，求单位方向向量，向左右两侧偏移 `k*(间隔(+8)+基准(+0x10))`，**弯道用 `sin(偏移/半径)` 修正**（0x3157–0x3207 行两处 `__libm_sse2_sin`）—— 即左右车道样条偏移的生成（★★★ 的数学结构，★★ 的语义）。
- **间距/排队**：§3.1/§3.2 的容量-队列模型即车间距模型；`FUN_008360b0 @ 0x008360b0`（Traffic/Vehicle 共享）是"能否进入"：逐段 `vtable+0x84`，后备连通表里 `vtable+0xf4(seg,1)→+0x30 计数 > 0` 即判占（★★）。
- **到达/卸客** `FUN_00727ca0 @ 0x00727ca0`（★★）：找第一个有客管道（`vtable+0xf4(seg)` 计数>0）写入 `槽+0x30`；`+0x38 = -1`；`network->vtable+0x1d8/0x1dc` + 随机范围（FUN_00923480/009234b0，DAT_00d21460/68）→ `+0x19c`；`FUN_00727280` 记录位姿、`FUN_00724e90` 刷视觉；`+0x38 = idx*2|旗标`；命中锚点（DAT_00fe4b00..08）时直接把 `+0x3c..0x44` 设为目标坐标并置 `+0x40` bit2 —— **瞬移式到位**（★★）。
- **agent 位置查询** `FUN_007301d0 @ 0x007301d0`（Vehicle）/ `FUN_00741e90 @ 0x00741e90`（Pedestrian）：按半径近邻查找，vtable+0x74 取位姿，欧氏距离平方比较 —— 供拾取/高亮/UI 使用（★★）。

### 4.3 信号与拥堵（cTransportSignal）

`FUN_0074f6c0 @ 0x0074f6c0`（Signal dt 更新，★★）：
- 遍历 `+0x10..0x14` 槽表（0x5c 步长，与 §2.2 同结构）；
- 若 `*entry(+0x00 时钟) <= entry+0x58 周期`：
  - entry+0x30 管道有效 → `network->vtable+0x1d4(idx)+0x10` 拿地块对象 → `+0x8c` 取信号对象；对象有效（+0xc≠0）且未手动（+0x65==0）时：
  - `FUN_00819ea0(obj+0x10)` 取**信号组 id**；与 `+0x54/+0x58` 登记组一致时，`FUN_0082de10(obj+0x10)` 展开组内信号 id 列表；
  - 匹配 `entry+0xc` → `network->vtable+0x1e0(idx, a, b)`（试开连通）→ `vtable+0x1e4`（提交）→ `entry+0xc = -NAN`（0xffffffff：已触发），`+0x74` 计数++；
  - 无组 → `FUN_00c214f0(idx, network, entry, …)` 默认处理，`+0x78` 计数++；
  - 无论是否触发：`entry+0x58 += dt`（周期累积）。

`FUN_0074f870`（新信号注册进组：遍历已有槽的 +0x0c 表，凡 id 出现在本组列表即绑 `+0x30`）；`FUN_0074f950/0074f990`（信号 id 列表增删）。
→ **拥堵机制**：信号以"组"为单位在周期到期时批量切换连通性（vtable+0x1e0/0x1e4 即路口 two-way 连通开关），配合 §3.1 的容量-队列，红灯 = 出边容量按组归零，车队在方向池中积压（★★）。

---

## 5. 多模式交通（Rail / Airplane / Helicopter / Pedestrian / Radial）

### 5.1 模式总表（SC_cGameAppMode.c @ 0x774–0x796 行，★★★）

```c
FUN_00766130(param, 0xc086c6eb, 0,0)          // → Airplane 域工厂
FUN_0075d9e0(param, 0x6de71541, 0)            // → Helicopter 域
FUN_00756260(param, 0x8136c3aa, 0,0)          // → Helicopter 域（第二变体）
FUN_0074f650(param, 0x34f64dcd, 0,0)          // → Radial 域
FUN_0074ee40(param, 0x1a65986c, 0,0)          // → Radial 域（第二变体）
FUN_00748560(param, 0xe7565405, 0)            // → Pedestrian 域
FUN_007328e0(param, 0x54abdf8c, s_Vehicle_00d1ca04)   // "Vehicle"
FUN_007328e0(param, 0xcad37bd0, s_Drone_00d1c9fc)     // "Drone"
FUN_007328e0(param, 0x6afe622f, &DAT_00d1c9f4)        // Vehicle 第三变体（名字字符串 8 字节，未解码）
FUN_007112f0(param, 0xa63ddc77, s_Light_Rail_00d1c9e8) // "Light_Rail"
FUN_007112f0(param, 0x4b277752, s_Heavy_Rail_00d1c9dc) // "Heavy_Rail"
```
交叉印证：Rail 构造 `FUN_0071a380 @ 0x0071a380` 明确分支 `== 0xa63ddc77`（Light）读属性 `0x4cb1f193`+`0xe281d1a3`、`== 0x4b277752`（Heavy）读 `0x9a79f996`；Vehicle 构造 `FUN_00731240 @ 0x00731240` 分支 `0x54abdf8c / 0xcad37bd0 → 0x3106c2e`，否则（第三变体）`→ 0x38758f9c`（★★★）。**模式 = GUID + 名称字符串，全部数据驱动注册**。

### 5.2 Rail（SC_cTransportRail.c）

构造 `FUN_0071a380 @ 0x0071a380`：每条轨道元素建 **0x68 字节记录**（`+0x14c` 数组）：
- 公共 4 属性 `0xc52409e / 0xc74595f / 0xc52409f / 0xc745960`（全部模式共用的"四件套"，疑为 容量/速度/宽度/标志，★★）；
- `0xdbf68be`：名称字符串 → `FUN_0077eb40` 哈希成 id 存 +0xf；
- `0xdbf68c8/0xdbf68cb` → +0x10/+0x11；`0xdbf68d0`：**站台列表**（0xc 步长，每项名字哈希 + 常量 `0x2ca33bdb` 组成 16 字节项入 +0x13 向量，★★）；
- `0xdce1b90`（字节数组→+0x12 项 +0xc）、`0xe374265`（→+4）、`0xe37426b`（→+8）、`0xdbf68af/0xdbf68b4`（+0x16/+0x17）、`0xee69f33`（+0x18）、`0xf9a0eeb`（byte +0x19）；
- 颜色：`FUN_00431f80(rec, DAT_00f9efba, &col)` 失败则默认色 DAT_00cf24a8/00cfeccc（★★）。
- 容量数组：`+0xfc`（stride 0x38）、`+0x140`、0xac 分块数组、`+0xa0`（8 字节）均按边数扩容，并把节点资源 id 写入 +0xfc 记录 +0x14（★★）。
- 火车运行：`FUN_0071afe0`（~320 行）、`FUN_00714390`（~400 行）、`FUN_0070d950`、`FUN_007160a0`、`FUN_00710ef0` —— 未逐行还原，但从站台表结构可推断为"沿线段序列 + 站点停靠"的定点调度（★）。

### 5.3 Airplane（SC_cTransportAirplane.c）

构造 `FUN_00764e00 @ 0x00764e00`：每条航线对象建 ~0x5c 记录；除四件套外读 `0xc7866f03 / 0x8e4c9704 / 0x13ac275e / 0xbb935c68 / 0x53ce4ed0 / 0x6644c84f / 0x510c979c`（uint32）与 `0x29b62172`（float，type 9）；`record[0x11] = float属性 * 原值`（缩放）；最后取地图中心（network vtable+0xac）存 `+0x160`（★★）。语义推测为 起降间隔/巡航高度/速度 等航线参数（★）。

### 5.4 Helicopter（SC_cTransportHelicopter.c）

构造 `FUN_00754230 @ 0x00754230` 极短：仅 `FUN_00894d50` 重绑 + 清 `+0xb0..0xb8` + `map->vtable+0xb8` → `FUN_004f0c50` —— **直升机不建逐边几何，只有自由飞行池**（★★）。主要函数 `FUN_00755900`（~290 行）、`FUN_00755120`、`FUN_00753270`、`FUN_00752230`（★，未还原）。

### 5.5 Pedestrian（SC_cTransportPedestrian.c）

构造 `FUN_00746dc0 @ 0x00746dc0`：
- 基类 `FUN_0083bbc0`（与 Traffic 同构造，★★★）；随后 `FUN_00746d20(500) / FUN_0073e370(500) / FUN_00896b70(1000) / FUN_00c11100(500)` 四个池预留；
- `+0x1c8 = map->vtable+0x9c(0x418a18b1)`；
- `rdtsc/QueryPerformanceCounter` 初始化全局计时（DAT_00fe5018，★★）；
- `FUN_00734fa0(+0x1d4, +0x1d8)` 建**哈希桶表**（+0x290 缓存桶指针，桶内链 stride 0x58，★★）。

重建 `FUN_00745390 @ 0x00745390`：0x38 记录 `{+0=-1, +4/+8, +0x18/+0x1c 链表头尾, +0x20 计数, +0x24, +0x2c=3, +0x30=3}`；0x28 记录重置为 DAT_00fe4da0..a8 默认 vec3；0xac 分块数组销毁；`FUN_0083aeb0` 收尾（★★）。
近邻查询 `FUN_00741e90 @ 0x00741e90`（★★，见 §4.2）。
行人网络独立于车行道（`FUN_00896b70(1000)` 自有管道池），即在路段旁/穿地块的步道网（★★）。

### 5.6 Radial（SC_cTransportRadial.c）

- 构造 `FUN_0074dcb0 @ 0x0074dcb0`：仅重绑 + 清三个字段 —— 最轻量的池（★★）。
- 更新 `FUN_0074e140 @ 0x0074e140`：0x60 步长记录 `{+0xc 计数/+0xd 上限, 槽位向量}`；遍历槽位（0x5c 步长，引用 §2.2 的 map 槽）；当目标记录 `+0x30 != +0x34` 时把槽标 `*slot = -NAN` 并摘除（★★）。
- 推断：**Radial = 点对点直线"径向"行程池**（跨任意两点、不经路网），对应游戏内跨城/特殊派送类 agent（★-★★）。

---

## 6. 通勤回路（家 → 工作 → 商业）

反编译层能看到的"通勤"证据全部是**机制侧**；"为什么去哪"的数据不在二进制里（见 §7）：

1. **行程生成 + 目的地分配**：SC_cTransportVehicle.c `FUN_0083a8b0 @ 0x0083a8b0` / `FUN_00838900 @ 0x00838900`（Traffic/Pedestrian 亦有同地址副本）：
   - 校验发起者：`network->vtable+0x84` 查对象 → `+0x1f0` 边界 → `vtable+500` 有效性 → 位置不等于哨兵（DAT_00e08f4c/50）→ `vtable+0x1f8/+0x128` 取地块数据 → `FUN_006cfe10` 取建筑信息 → `FUN_00836780 / FUN_008368e0` 两级目标过滤（疑"能否步行到达/是否已满"，★）→ `FUN_00726500/FUN_00724fe0` 记录坐标；
   - 然后遍历 `param_1[0x1a]` 目的地表（**0x18 字节/项**），把总数均摊：`份额 = 总数/项数 (+首项补余数)`，逐项调 `vtable+0xc4(资源id, 份额, 记录指针, 目标句柄, 序号)`（★★★）。
   → 一次"需求"被拆成对多个目的地的 spawn 调用；目的地表由属性/资源系统提供。这就是通勤分派的引擎出口。
2. **覆盖/吸引范围标记**：`FUN_00839e90 @ 0x00839e90`（★★）：属性表 `0xd688453`(map) + `0xd688497`(uint32[]) + `0x9962f265`(引用数组，type 0x8000000d) 定义 `{资源, 半径, 参数}` 三元组；对每个三元组：
   - 从哈希桶（`unaff_EBX+0x16c`）收集半径内同资源对象；
   - `FUN_0060f7b0(圆心, 半径, …)` 查询附近管段；
   - 对每段做**线段-圆求交**（0x1044–0x1116 行的二次方程），得区间 `[t0,t1]` 后 `FUN_006ca9a0(段idx, t0, t1, 来源id, 资源, 半径, 参数)`。
   → "沿路网给建筑覆盖服务范围"的几何实现（公交站/商铺吸引力类的机制基础，★★）。
3. **agent 的目标端**：§2.2 的 `+0x34`（方向池B索引）与 `FUN_00894de0` 计数即"以某管道为目标的 agent 数"；`FUN_00759c60` 的三级下一跳队列说明 agent 在节点间**逐段续约**而非全程预计算（★★）。
4. **反向证据**：全部 348 个文件中 grep `commute|travel|home|work|shop` 无命中；无 "Sim" agent 字符串。通勤的目的选择逻辑（哪栋楼的人去哪上班）不在本二进制（★★★）。

---

## 7. 驱动脚本：agent 行为由什么驱动？

结论：**引擎（本目录）只提供"模式 + 池 + 管道"的执行机；行为参数与触发全部来自 GlassBox 数据（GCT 对象属性/资源），脚本本体在游戏数据包中而非二进制。** 证据：

1. **属性读取无处不在**：每个构造/更新都通过 `obj->vtable+0x28(hash, &out)` 读属性并检查类型 tag（`*(short*)(out+0x12)`：0xd=uint32 数组、9=float、1=uint8、0x20=map、0x8000000d=引用数组），且都带默认值回退（例：Vehicle ctor `+0x2b0=6000` ← 属性 `0xf10fcfc`、`+0x2b4=400` ← `0xf10fd02`、`+0x2b8=2` ← `0xf1102a6`，★★★）。
2. **模式注册表**（§5.1）由 AppMode 在启动时以 GUID+名称字符串装配 —— 加哪种交通、几种变体完全是数据/装配层决定（★★★）。
3. **拥堵调参零代码**：SC_cPathCongestionTuning.c 只有引用计数模板析构 —— 调参值必为纯数据（★★★）。
4. **脚本系统存在但独立**：字符串 `s_INFO__Starting_GameScript_index_…`、`s_UPDATER_SCRIPTS_UPDATE_START`（SC_cGameAppMode 等处）证明有 GameScript 下载/索引层；`GB_cStateFlow*`、`GB_cEcoGameEffect`、`GB_cEcoSwarmMap` 是规则/效果层的基础设施。交通引擎通过 §6.1 的 `vtable+0xc4`（spawn 到目的地）与 §2.6 的事件向量（`FUN_0060c530`）与脚本层交互（★★）。
5. **硬编码部分**：车道偏移数学（sin 修正）、信号 mod-4 相位环、拥堵 min-传播与 LCG 择向、0x1e4/0x288 两级路口对象尺寸 —— 这些是引擎内固化算法，参数（周期、容量、系数）来自属性（★★）。

---

## 8. 结论速查（按问题）

| # | 结论 | 关键证据 | 置信度 |
|---|---|---|---|
| 1 | Agent 基类 11 字段序言，三个派生各加自有池/表 | SC_cAgent.c 0x007087b0 等 | ★★★ |
| 2 | 三层池 = 基座绑定(net,id,map) → cPool(0x5c 槽×边数) → cDirectionPool(0x14 名册×方向)；按 (node,node) 懒重建 | 0x00758ab0/0x00758d10/0x0075ba00/0x0083b530 | ★★★ |
| 3 | PathPool = 邻接边容量-队列 + min 拥堵传播 + LCG 择向；GraphicsPath 只消费几何 | 0x00c25ed0/0x00c26410/0x008021f0/0x008046d0 | ★★ |
| 4 | 车辆 = 槽位占队（容量即车间距），信号组周期切换连通（mod-4 相位），拥堵 = 方向池积压经 §3.1 传播 | 0x0075fdc0/0x0074f6c0/0x00c25ed0/0x007254d0 | ★★ |
| 5 | 11 种模式 GUID 注册；Rail 按站台表调度、Helicopter/Radial 无逐边几何、Pedestrian 独立步道网+哈希桶 | SC_cGameAppMode.c 0x774–796；各 ctor | ★★★ |
| 6 | 通勤 = 属性表驱动的"目的地分摊 spawn" + 半径覆盖标记；目的选择数据不在二进制 | 0x0083a8b0/0x00838900/0x00839e90 | ★★ |
| 7 | 行为参数 100% 属性驱动（类型 tag 校验+默认值回退）；算法骨架硬编码；GCT 脚本在数据包 | 全部 ctor；s_INFO__Starting_GameScript | ★★★ |

**遗留未解**：`0xc52409e/0xc74595f/0xc52409f/0xc745960` 四件套属性的真实名称（需属性数据库/`property` JSON 才能命名）；`0xdc0d28f` 资源 id 的语义；Radial 第二 GUID `0x1a65986c` 对应的游戏内模式（疑 Ship/Ferry 类，纯推测）。