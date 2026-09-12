# GlassBox 引擎五大子系统深扫概述

> 2026-09-12。基于 `docs/source-code/`（348 个 IDA 反编译伪 C 文件，5110 个虚函数、346 个类）的五路并行深扫：
>
> | 文档 | 子系统 | 核心结论 |
> |---|---|---|
> | [eco-swarm.md](eco-swarm.md) | GB::Eco/Swarm 场+流+变换经济回路 | 经济=「beat 规则表 → int 网格场（SwarmMap）→ property 协议触发效果（EcoGameEffect）→ 受体回调」；**经济侧没有独立 agent 类** |
> | [transport.md](transport.md) | SC::Transport 管流交通 + PathPool | **显式 agent 模拟**：三层管道池（BasePool/Pool/DirectionPool）+ 容量-队列拥堵模型 + 信号组 mod-4 相位；11 种模式 GUID 数据驱动注册 |
> | [zoning-lot.md](zoning-lot.md) | SC::Zoning/Lot 分区与地块成长 | 地块=「道路 corner 对之间的带」（非网格）；parcel 定长 **0x4C**、lot 主表步长 **0x108**；成长=每帧带时间预算的随机抽取+失败落废弃兜底 |
> | [terrain.md](terrain.md) | SC::Terrain 256×256 typed map 集 | **256×256=65536 常量实证**、13 张 typed map（getter 返回 13）、Eco/Graphics 双副本共享持有（引用计数非深拷贝）、Tessendorf 水面 |
> | [disaster.md](disaster.md) | SC::Disaster 13 类硬编码灾难 | switch case 1..13 实证；破坏=「状态标记+计时+属性随机」无血量表；整个触发面收敛于单函数 `FUN_00702770` |

每个文档均带证据地址（exe VA）、置信度分级与「对 OpenSCP 的可复用结论」。

---

## 1. Agent 的基本形式（跨子系统对比）——本概述最重要的一节

GlassBox 里 **"agent" 不是统一基类，而是两个截然不同的形态**：

### 1.1 交通 agent（显式实体，SC 侧）

`SC::cAgent` 基类极薄（构造 `FUN_007087b0`，11 字段序言）：

```
+0x34/0x38/0x40 : 计数/游标        +0x44/0x48 : id = -1（成对，疑 from/to 管道）
+0x4c/+0x50     : 默认资源句柄      +0x54/0x60 : id = -1
+0x5c           : 布尔标志
```

派生类各自扩展：`cVehicleAgent` +0x9c 起 12 项 uint16 id 数组（0xffff 填充）；`cRailAgent` 有 0x120 步长的列车编组记录；`cPedestrianAgent` 携带空间包围盒与独立步道网（自有管道池 + 哈希桶近邻查询）。

**关键：agent 本身几乎不带状态——状态由管道池槽位承载**。槽位 0x5c 字节：`+0x30/+0x34` 方向池 A/B 索引（是否在路段上/等待）、`+0x3c..0x50` 位置+方向 vec3、`+0x54` 权重、`+0x58` 进度。生命周期 = 进入边建槽（`FUN_0075f3f0`）→ 过路口换槽（三级后备队列 `+0x4a/+0x46/+0x42` 逐段续约，**无全程预计算寻路**）→ 离开边删槽。

### 1.2 经济侧没有 agent（GB 侧）

Eco 回路的 dump 内**不存在任何经济 agent 类**——`SC::cAgent` 只有 21 行哨兵初始化。经济行为完全由 **场（SwarmMap int 网格）+ beat 规则表**承担："agent 是数据、场是机器"。文献中的 `resource→module→agent→effect` 四段，在二进制中的对应物是：

| 文献概念 | 二进制对应物 | 证据 |
|---|---|---|
| resource | `cEcoGameDescription`（纯数据结构+COM）+ SwarmMap 场 | RTTI `?AU` |
| module | 5 通道接口 `cIEcoStreamHandler/cIEcoTransformHandler/cIEcoTransportHandler/cIEcoZoneHandler/cIEcoHeightField` + `cIEcoGameHandler` 总口 | RTTI 存在（实现未 dump） |
| agent | **无**——数据行即 agent | 缺失本身即证据 |
| effect | `cEcoGameEffect`（Swarm 组件：Begin/Advance/End + property 6/7 换效果 + 目标列表回调） | `GB_cEcoGameEffect.c` 全解 |

### 1.3 灾难 agent（第三形态：表现层活动对象）

`SC_cDisasterGame` 管理的 13 类灾难 actor 是 **COM 风格状态机**（UFO 11 态、Robot/Monster 14-15 态），管理器逐帧 Tick、返回"已结束"即回收。它不进交通管道池，破坏走"城市接口写状态字段"（`+0x54=2` 摧毁态），无血量表。

---

## 2. 交通算法（重点还原）

### 2.1 数据面：道路即管道网络

- **三层池**：`cBasePool`（绑定 network+32 字节 GUID key）→ `cPool`（按 map 边数/点数预留，**槽位 stride 0x5c**）→ `cDirectionPool`（每方向一个"计数+8 字节名册向量"）。
- **懒重建**：交通池为每条边缓存 0x38 字节记录 `{起点node, 终点node, 起版本, 终版本, mapIdx, 资源id}`，与 network 边表比对——**网络不动则管道不动**（`FUN_0083b530`）。
- 车道几何由管段直接生成（GraphicsPath `FUN_008021f0`：中点+叉积法线+`sin(偏移/半径)` 弯道修正的车道样条），渲染层只是几何消费者。

### 2.2 容量-队列拥堵模型（核心算法）

- 拥堵计数：遍历车辆路径池，把每个有效槽计入 `+0xb8` 总数与 `+0xbc+方向*0x14` 的方向桶（`FUN_007254d0`）。
- **拥堵传播**（`FUN_00c25ed0`）：对每条边取至多 8 个邻接边（编码 `idx<<1|方向`），按资源掩码位图过滤，容量 = 方向表容量 × 系数；非 per-lane 模式 `值 = (阻塞阈值 - 计数) × 系数`，per-lane 模式对队列逐项 `条目×容量`（或 `容量-条目`）取 min，再考虑对向边+ε，取 min 写回——**拥堵沿邻接边逐跳扩散**。
- **路口择向**（`FUN_00c26410`）：比较两条出边 `当前值+候选值` 与对向对应值，加 LCG 随机抖动（种子 ×0x278dde6d）——**拥堵 + 随机 = 转向决策**。
- **超员溢出**：车辆池溢出时向对向车道溢出（`FUN_00896440`）。
- **信号**：信号对象按"组"注册，周期到期批量切换路口连通性（mod-4 相位环，两级路口对象池 0x1e4/0x288 字节 = 普通/大路口）——**红灯 = 出边容量按组归零**，车队在方向池积压，再经传播模型向后扩散。

### 2.3 移动与到达

车辆不做全程寻路：逐段续约（节点处三级后备队列）+ 到达/卸客用锚点瞬移（`FUN_00727ca0` 命中锚点直接写目标坐标）。行人有独立步道网 + 0x58 步长链的哈希桶近邻查询。

## 3. 驱动 agent 的脚本：数据驱动，代码只留算法骨架

三个子系统给出**一致的结论**：

1. **行为参数 100% 走 property**：每个构造/更新函数都以 `vtable+0x28(hash, &out)` 读属性、校验类型 tag（0xd=u32 数组、9=float、1=u8、0x20=map、0x8000000d=引用数组），全部带默认值回退（例：Vehicle `+0x2b0=6000` ← 属性 `0xf10fcfc`）。拥堵调参文件 `SC_cPathCongestionTuning.c` 只有引用计数析构——**调参是纯数据，代码无表**。
2. **交通模式 = GUID + 名称字符串的注册表**（`SC_cGameAppMode.c` 实证 11 种：Vehicle/Drone/Light_Rail/Heavy_Rail/Airplane/Helicopter×2/Radial×2/Pedestrian/第三 Vehicle 变体），加哪种交通完全是装配层数据决定。
3. **beat 规则引擎**（Eco 侧时基）：规则表在数据 blob `+0x50/+0x54`（相位 B `+0x60/+0x64`），每规则 12 字节 ushort 掩码，命中 `0x8000` 位立即广播 `cIOnBeatRules` 两相位回调；未命中入 20 字节事件队列（容量 10001 = 背压阀）。**"每 beat 重算资源场→触发依赖规则"是 Eco 的 tick 模型**。
4. **脚本本体在游戏数据包**：字符串 `s_INFO__Starting_GameScript_index_…` 证明有 GameScript 下载/索引层；SimCity 的模拟规则（RCI 需求、资源产出）是 **GCT 对象 + property 表**，本仓库已有解析能力（sc-properties）。二进制内只固化算法骨架（车道 sin 修正、信号 mod-4、min 拥堵传播、LCG 择向）。
5. **Mod 意义**：改 property 即改行为（灾难解锁白名单 `0xf90e380`、密度阈值 `0xbf5d2e3`、交通参数 `0xf10fcfc` 族全是 property）——OpenSCP 的 property 编辑器因此就是事实上的"模拟调参器"。

---

## 4. 跨子系统横切结论

1. **hash 即接口**：跨模块常量全部是 32 位属性/消息 hash（0xe556413 beat、0x87abfd1 效果管理器、0xdc0d28f agent 资源、0x1dd7bda9 全局消息…），**FNV/CRC32 均不匹配**（已程序化验证）——Maxis 私有 hash，做存档/资源兼容前必须破解（或建彩虹表）。
2. **COM + 侵入式引用计数**：`{vftable@0, vftable@4, refcnt@8}` 双 vtable 布局遍布全部子系统。
3. **子系统注册表**：`GameManager->vf[0xe8](fourCC)` 统一定位；fourCC 连续区段 0xe55640f..0xe556570（GraphicsZone→TerrainGame），**EcoGame 落在 0xe55641a..0xe556521 空缺区间**（未 dump）。
4. **客户端/服务器分层**：E（Eco）=服务器模拟、G（Graphics）/FX=客户端渲染，两侧以"引用计数共享 + 静态描述符"连接（terrain 实证）；灾难系统的"视觉摧毁上报"（city vtable+0x138/0x90）是同一分层的另一例。
5. **IDA 切块伪影**：`FUN_00800000`、成排空函数、GetPoolName 是切块伪影，建模时须剔除。
