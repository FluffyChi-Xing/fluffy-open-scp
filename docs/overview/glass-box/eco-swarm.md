> 深扫笔记（2026-09-12）。材料：docs/source-code 下 Eco/Swarm/Stream/Delta/Beat 相关约 20 个文件 + tmp/rtti_classes.txt（脱壳 RTTI 全量清单）。
> 置信度标注：**[实证]** = 有 vtable 符号/字符串/结构偏移直接支撑；**[推测]** = 由调用约定与代码同构推断。函数地址均为 exe 内 VA，可在 Ghidra 二次定位。

# GB::Eco/Swarm 经济回路深扫笔记

> 2026-09-11。材料：`D:\rust\packages\fluffy-open-scp\docs\source-code\` 下 348 个 IDA 伪 C 文件中的 Eco/Swarm/Stream/Delta/Beat 相关约 20 个 + 交叉引用 grep + `D:\rust\packages\fluffy-open-scp\tmp\rtti_classes.txt`（脱壳 RTTI 全量清单）。
> 置信度标注：**[实证]** = 有 vtable 符号/字符串/结构偏移直接支撑；**[推测]** = 由调用约定与代码同构推断。
> 函数地址均为 exe 内 VA，可直接在 Ghidra 中二次定位。

---

## 0. 先修正一个命名层误会（重要）

RTTI 实证（`tmp/rtti_classes.txt`，grep `eco|swarm`）：

```
.?AVcEcoSwarmMap@GB@@           .?AVcEcoSwarmLerpMap@GB@@
.?AUcEcoGameDescription@GB@@    （U = 纯结构体，无 vtable）
.?AVcEcoGameEffect@GB@@         .?AVcEcoGameHandlerBase@GB@@
.?AVcIEcoGameHandler@GB@@       .?AVcIEcoHeightField@GB@@
.?AVcIEcoStreamHandler@GB@@     .?AVcIEcoTransformHandler@GB@@
.?AVcIEcoTransportHandler@GB@@  .?AVcIEcoZoneHandler@GB@@
----- namespace Swarm@EA (13 类) -----
cIMap / cMapBase / cISurface / cSurfaceBase / cIComponent / cComponentBase
cDescription / cParticlesDescription / cLightDescription
cParticlesEffect / cIDecalManager / cIModelParticleStreamer / cITextureParticleStreamer
.?AVcSwarmGameHeightField@?A0xf5e3c6ee@@   （匿名命名空间桥接类）
```

结论：
- **Swarm@EA 是 EA 的粒子/效果中间件**，其 `cIMap/cMapBase`（宽度×高度的 int 网格）被 GB 复用为"场（field）"的数据平面。GB 的 `cEcoSwarmMap/cEcoSwarmLerpMap` 是 Swarm map 的世界空间包装器。**[实证：RTTI + 下文结构]**
- **GB "Eco" 家族 = 5 通道资源接口层**：HeightField（地形高度）、Stream（流）、Transform（变换）、Transport（交通）、Zone（分区）+ 一个总口 `cIEcoGameHandler`。与 GlassBox 文献中"field/stream/transform 三类处理单元"对应关系为：field↔Swarm map/HeightField，stream↔cIEcoStreamHandler，transform↔cIEcoTransformHandler。**[实证：RTTI 存在性；[推测]：语义对应]**
- 字面量 `resource→module→agent→effect` 类名**不存在于二进制**；见 §4 的替代证据链。

---

## 1. 类清单与角色

| 类 | 文件 | 职责（证据） | 继承/双 vtable |
|---|---|---|---|
| `GB::cEcoSwarmMap` | `GB_cEcoSwarmMap.c` | 世界 AABB 内的 int 标量场，**最近邻采样** | `{vf@0, vf@4, refcnt@8}` + 内嵌 Swarm map 指针 **[实证/推测混合，见 §2]** |
| `GB::cEcoSwarmLerpMap` | `GB_cEcoSwarmLerpMap.c` | 同上但**双线性插值** | 同布局，仅采样函数不同 **[实证]** |
| `GB::cEcoGameDescription` | `GB_cEcoGameDescription.c`（14 行） | 纯数据描述结构体（RTTI 为 `?AU`），唯一导出是 `EA::COM::RefCountTemplate<int>` 析构 | EA::COM 引用计数 **[实证 L6]** |
| `GB::cEcoGameEffect` | `GB_cEcoGameEffect.c` | Swarm 组件薄包装：Begin/Update/End 播放一段"EcoGame 效果"，带属性 setter | `EA::Swarm::cComponentBase` + `EA::COM::RefCountTemplate<int>` **[实证 L261-262]** |
| `GB::cEcoGameHandlerBase` | `GB_cEcoGameHandlerBase.c`（索引计 43 个虚函数） | Eco 处理器公共基类：持有 target 对象 + 其 ID | 大 vtable 基类 **[实证]** |
| `GB::cIEcoStreamHandler` / `cIEcoTransformHandler` | 各 52 行 | **纯接口**：全文件只有 4 份 `purecall`（4 个纯虚槽） | 无实现导出 **[实证]** |
| `SC::cIOnBeatRules` | `SC_cIOnBeatRules.c` | 节拍观察者纯接口（purecall ×4） | **[实证]** |
| `SC::cBeatRuleHandler` | `SC_cBeatRuleHandler.c`（43 个虚函数） | 节拍规则引擎：规则表掩码匹配 → 广播/入队 | 子系统 fourCC `0xe556413` **[实证 L36]** |
| `SC::cTerrainEcoMap` / `cTerrainEcoMapCombiner` / `cTerrainEcoMapList` | `SC_cTerrainEcoMap*.c` | 地形生态图层组合（本 dump 只含析构 + 共享块，逻辑未导出） | RTTI 存在 **[实证存在性/逻辑缺失]** |
| `SC::cEcoGameUIEventHandler`（匿名 ns `FA446B77`） | `SC_cGameUI.c` L74-83 | UI 侧 EcoGame 事件观察者：0x18 字节对象，双 vtable + `EA::RefCountTemplate` + `+0x14` 回指 GameUI | **[实证]** |

### 1.1 共享引用计数块（IDA 切块伪影）

`FUN_006f92a0`（`+8` 计数 +1）与 `FUN_00878b20`（-1，归零时置 1 并 `vf2[0](1)` 删除）逐字出现在 `GB_cEcoSwarmMap.c:1-27`、`GB_cEcoSwarmLerpMap.c`、`GB_cEcoGameEffect.c:223-249`、`GB_cEcoGameHandlerBase.c:1-27`、`SC_cBeatRuleHandler.c:1-27`、两个 `SC_cTerrainEcoMap*.c`。对象布局 `{vftable@0, vftable@4, refcnt@8}`，即 EA::COM 二基类侵入式计数。**[实证：代码同构 7 处；布局归因推测]**

### 1.2 子系统 fourCC 注册表（`GetSubsystemID` 槽，各文件 L36 附近同构函数）

| fourCC | 子类 | 证据 |
|---|---|---|
| `0xe55640f` | GraphicsZone | `SC_cGraphicsZone.c:36` |
| `0xe556410` | GraphicsGame | `SC_cGraphicsGame.c:36` |
| `0xe556411` | DisasterGame | `SC_cDisasterGame.c:36` |
| `0xe556412` | AudioGame | `SC_cAudioGame.c:36` |
| `0xe556413` | **BeatRuleHandler** | `SC_cBeatRuleHandler.c:36` |
| `0xe556414` | GraphicsGameImpostor | `SC_cGraphicsGameImpostor.c:36` |
| `0xe556415` | VolumeDecalManager | `SC_cVolumeDecalManager.c:36` |
| `0xe556416..19` | Physics/Road/Layer/Zoning | 各文件 L36 |
| `0xe556522/523/524` | TransportShared/Telemetry/AvatarHelper | 各文件 |
| `0xe556570` | TerrainGame | `SC_cTerrainGame.c:36` |
| （空缺 `0xe55641a..0xe556521`） | **EcoGame 应落在此区间** | 未被 dump **[推测]** |

子系统获取统一走 `GameManager->vf[0xe8](fourCC)`（`SC_cToolPlop.c:258-259`、`SC_cTransportRail.c:1263` 等）。**[实证]**

---

## 2. SwarmMap 数据结构

### 2.1 布局（`GB_cEcoSwarmMap.c` / `GB_cEcoSwarmLerpMap.c` 合并还原）

```
cEcoSwarmMap (包装器)
 +0x00 vftable            (RTTI: cEcoSwarmMap@GB)          [推测槽位内容]
 +0x04 vftable2           (EA::COM::RefCountTemplate<int>)  [推测]
 +0x08 refcount int
 +0x0c map*  → Swarm map 数据对象 {                 [实证：三个采样函数共同访问]
 |    +0x08 int width
 |    +0x0c int height
 |    +0x10 int* data      (row-major, 4 字节/格, 索引 y*width+x)
 |    +0x28 int ?          (FUN_00c1e7d0 返回，含义未知)
 +0x10 float scale         (采样结果统一乘子)            [实证]
 +0x14..+0x28 float AABB[6]: minX,minY,?, maxX,maxY,?       [实证：contains 测试]
```

- `FUN_00c1e9a0`（Map L77-87）：2D contains，`x∈[+0x14,+0x20) && y∈[+0x18,+0x24)` → 证明 0x14/0x18=左上、0x20/0x24=右下。**[实证]**
- `FUN_00c1e970`（Map L63-73）：导出全部 6 个 float = GetBounds。**[实证]**

### 2.2 采样语义

- **最近邻**（`FUN_00c1e9e0`，`GB_cEcoSwarmMap.c:103-145`）：
  `u=(x-minX)/dx`, `v=(y-minY)/dy`（分母为 0 保护，`DAT_00da06e0`≈epsilon）→
  `cx=round(u*width)`, `cy=round(v*height)`，越界方向 +1（`DAT_00cf2ce4`=1.0f 的 ceil 修正）→
  返回 `(float)data[cy*width+cx] * scale`。**[实证]**
- **双线性**（`FUN_00c1eb00`，`GB_cEcoSwarmLerpMap.c:103-166`）：
  同上但坐标先减 `DAT_00da307c`（恒为 0.5f，同常量出现在共享地形内核 `FUN_00800000`，即取格中心），
  4 邻角 `data[(y±1)*w+(x±1)]` 用 min/max 钳到 `[0,w-1]/[0,h-1]`，
  横向插值→纵向插值→`*scale`。**[实证]**
- 两类都是**世界坐标 → 格值**的只读查询；写路径不在 dump 内（推测在 Swarm map 原生类或未导出的 update 函数）。**[推测]**

### 2.3 与 256×256 地形的关系

- 没有任何代码硬编码 256；地形分辨率证据来自另一路：`SC_cGraphicsGame.c` 的 `cTerrainMapUint8/Uint16` 与子系统文档中的常量 `0x10000`(=65536=256×256)。
- SwarmMap 用 **float AABB + width/height** 表达覆盖范围，因此可对齐任意网格密度（包括 256×256 的地形、或更粗的经济网格）。二者是"同一世界坐标系下的不同分辨率层"，而非同构复制。**[推测，中置信]**
- `SC::cTerrainEcoMap/Combiner/List`（RTTI）是地形侧的生态图层；dump 中两个文件仅含析构（`FUN_00bf75a0`→`FUN_00be9020`，`FUN_00bfbc30`→`FUN_00be8fb0`，地址相邻证明二类同源）+ 与其他文件共享的 `FUN_00800000` 地形修改内核（遍历格区域→写多个子层，见 `SC_cTerrainEcoMapCombiner.c:87-214`）。**结论：EcoMap→地形纹理/属性的合成链在 dump 外，尚无法还原。**[实证缺失]

---

## 3. Stream/Transform handler 接口与执行时机

### 3.1 cIEcoStreamHandler / cIEcoTransformHandler

- 纯抽象（`purecall @ 00a90870` ×4，`GB_cIEcoStreamHandler.c` 与 `GB_cIEcoTransformHandler.c` 全文）；导出索引各 4 个虚函数。**[实证]**
- 具体实现在 dump 中不存在 → 无法给出输入输出签名。按 GlassBox 文献与 `cIEcoHeightField/ZoneHandler/TransportHandler` 并列关系推测：Stream=资源在处理器间的流动通道，Transform=资源场之间的映射算子。**[推测，低置信]**
- 注册方式：EcoGame 侧统一持有 target+id（见 3.3），且 `SC_cGameUI.c:74-83` 的 UI 观察者模式（分配 handler→替换 GameUI `+0x14` 槽，旧指针 Release）说明 handler 是**槽位挂接**而非全局广播。**[实证结构]**

### 3.2 执行时机：beat 驱动（实证链最完整的一环）

`SC_cBeatRuleHandler.c`（fourCC `0xe556413`）：

- 规则常量：`FUN_0058ec10 → 0x37`(=55)（L41-47，与 EcoGameHandlerBase/TelemetryManager/Impostor 共享同一 stub）；`FUN_006d1220 → 0x6001`(=24577)（L51-57）。**[实证；语义推测：规则数/规则 id 空间]**
- **匹配路径**（`FUN_006d17a0` L272-339 与 `FUN_006d18c0` L343-410，双胞胎=两个节拍相位）：规则表位于节拍数据 blob 的 `+0x50/+0x54`（另一相位 `+0x60/+0x64`），每规则 12 字节步长的 ushort 掩码；命中 `0x8000` 位 → 立即对已注册监听者数组（`+0x28/+0x2c`）逐个调用 `vf[+8]`（相位 A）/`vf[+0xc]`（相位 B）——即 `SC::cIOnBeatRules` 的两个纯虚回调。**[实证]**
- **队列路径**：未命中则压入 `+0x14..+0x1c` 的 0x14(20) 字节事件队列，容量硬上限 `0x2711`=10001 条，超过立即冲刷（`FUN_006d1070`）——**队列即背压阀**。**[实证]**
- **消费**（`FUN_006d0fa0` L78-104）：逐条 0x14B 记录，调 target 的 `vf[+0x98]` 取规则对象，对象 `+0x1bc/+0x1c0` 是 0x1c(28)B/条的数组，记录第 3 字 `entry[2]` 是索引；越界则重置为默认值 `DAT_00dfcd9c/da0`。**[实证]**
- 时基：target 对象 `+0x18=+0x14`（写指针回退=清队，`FUN_006d1020/1050`）。不同子系统共享同一 float 时钟、各自 divisor。**[实证；divisor 数值在 dump 外]**

**对 Eco 回路的意义**：Eco handler 的 tick 由该 beat 机器驱动（cEcoGameEffect.Update 的计时累加见 §4），"每 beat 重算资源场→触发依赖规则"的模型成立。**[推测→与 §4 证据互洽]**

### 3.3 Handler 公共骨架（`GB_cEcoGameHandlerBase.c`）

- `FUN_00c20eb0`（L69-80）：SetTarget —— `+0xc = obj; +0x10 = obj->vf[0x34]()`。与 `SC_cBeatRuleHandler.c` 的 `FUN_006d1020`（L61-74）**逐指令同构**（同样的 `+0x24=-1`、`+0x18=+0x14`、`vf+0x34`）。→ 基类布局 `{+0xc target, +0x10 targetId}`，`vf+0x34` 是"取对象 ID/接口"。**[实证：同构；语义推测]**
- `FUN_0058ec20`：ClearTarget；`FUN_0058ec30 → (0,0)`；`FUN_0058ec40` nop；`FUN_0058ec50 → FUN_0058f240`（未导出）。**[实证]**
- `GetPoolName/AK::MemoryMgr` 与成排 `FUN_00ace2c0/FUN_007df9c0/FUN_00c1e7c0` 空 函数 = IDA 邻居污染伪影，非本类逻辑。**[实证：伪影]**

---

## 4. EcoGame 回路：resource→module→agent→effect 的证据链

字面类名不存在，但以下五环构成可辩护的替代证据链。

### 4.1 resource（资源=数据行+描述）

- `cEcoGameDescription` 是 `?AU`（纯结构）且引用计数（`GB_cEcoGameDescription.c:6`）→ 资源描述 = 数据 + COM 生命周期，无行为。**[实证]**
- 资源场数据 = §2 的 Swarm map（int 网格 + scale + AABB）——资源量在场上，不在 agent 上。**[实证]**
- property 文件驱动：规则/属性键是 32 位哈希，存于数据 blob（BeatRuleHandler 的 `+0x50` 偏移表、`SC_cBeatRuleHandler.c:289-316`）；`kRuleID/kResourceID` 常量在 exe 0 命中（既有文档 `glassbox-engine.md §4` 已证）→ **表驱动**。**[实证]**

### 4.2 module（=五个 IEco*Handler + 子系统注册）

- `cIEcoGameHandler` + Stream/Transform/Transport/Zone/HeightField 五接口（RTTI），经 GameManager `vf[0xe8](fourCC)` 定位，record 表 0x140B/条（`GB_cGameManager.c` FUN_00c1a370：`*(record_table + idx*0x140 + 4)`）。**[实证]**

### 4.3 agent（极薄数据行，经济侧无独立 agent 类）

- 经济回路 dump 内**没有**经济 agent；`SC::cAgent` 仅 21 行哨兵初始化（`SC_cAgent.c:3-18`：`+0x44/+0x48=-1`、`+0x54=-1`），交通 agent（Ped/Vehicle/Rail）只是容量-流量对。经济行为由场+规则承担——**"agent 是数据、场是机器"在经济侧同样成立，只是机器是 map+beat 而非 pipe**。**[实证（缺失本身即证据）]**

### 4.4 effect（`GB_cEcoGameEffect.c` 全解，270 行）

布局与生命周期（`cEcoGameEffect`，基类 `EA::Swarm::cComponentBase` L261）：

```
+0x0c description*（构造传入）
+0x10 bool playing
+0x14 float elapsed（每 tick += dt）
+0x18.. 参数块（传给 Begin）
+0x50 effectMgr*（由 description 的 owner 经 vf[0x34] 解析 0x87abfd1 得到）
+0x54 int effectId（= description->+0xc）
+0x58 8B 效果句柄 {int,int}
```

- **构造/绑定** `FUN_0058d380`（L1-12）：`mgr = owner->vf[0x34](0x87abfd1)`；`effectId = *(desc+0xc)`。→ `0x87abfd1` 是 EcoGame 效果管理器的接口/命令 ID。同一 ID 出现在 `GB_cGameManager.c:1182`：`gameObj->vf[0x30](0x87abfd1, record+4)` —— vt+0x30/vt+0x34 = **SendCommand(id,payload) / Query(id)** 的 COM 对。**[实证]**
- **Begin** `FUN_0058d3b0`（L26-46）：`if(!playing && mgr) { elapsed=0; handle=mgr->vf[0x74](effectId, params@+0x18, 0); playing = handle>=0; }`。**[实证结构；vf+0x74=Begin 推测]**
- **End** `FUN_0058d410`（L50-62）：`mgr->vf[0x78](&handle)`，句柄复位为 `DAT_00df2d3c/40`（无效值）。**[实证]**
- **Update** `FUN_0058d510`（L76-90）：`elapsed += dt; if (mgr->vf[0x8c](&handle)) return;  // 仍在播
  else this->vf[0xc](1);  // 通知宿主完成`。→ **效果是逐 tick 步进的**，完成即回调宿主（与全局引用计数的 `vf+0xc(1)` 完成语义同构）。**[实证]**
- **属性设置** `FUN_0058d450`（L128-169）：property id `6` = **按 ID 设效果**（`mgr->vf[0x74]`），`7` = **按索引设效果**（`mgr->vf[0x84](idx)`）；切换前先 End 旧句柄。→ 编辑器路径：改属性表即改播放的效果。**[实证结构]**
- **目标应用** `FUN_0058d710`（L94-114）：`mgr->vf[0x88](&handle)` 取状态 + `mgr->vf[0xd8]()` 取目标列表，对每个成员 `member->vf[0xc](status, params)` —— 效果→受体的分发点（"agent 受效果影响"的最近证据，成员是轻量对象）。**[实证结构；语义推测]**
- 管理器接口 vtable 复原（slot = 偏移/4）：`0x74`→29 Begin(id,…)，`0x78`→30 End，`0x84`→33 BeginByIndex，`0x88`→34 Query(handle)，`0x8c`→35 Advance(handle)→bool，`0xd8`→54 GetTargetList。**[实证调用点；语义推测]**

### 4.5 UI 回路

`SC_cGameUI.c:74-83`：GameUI 初始化时分配 `cEcoGameUIEventHandler`（0x18B：双 vtable + `EA::RefCountTemplate<int>@+4` + `+0x14`=GameUI this）替换 `param_1[5]` —— 模拟侧 EcoGame 状态向 UI 单向推送的观察者。**[实证]**

---

## 5. 与流式下载系统（Delta/BackgroundStreams）的关系

### 5.1 结论先行

**"Stream" 一词两套系统、一个抽象基座**：
- Eco 侧的 `cIEcoStreamHandler` 是**模拟资源流**接口（纯虚，无实现导出）；
- IO 侧的 `EA::IO::IStream` 装饰器栈（XOR/RateLimited/ChildWrapper/MultiFileStream/MD5）是**字节流**，被存档（`.egb`）与云端 delta **共用**。二者仅共享命名，不共享类型。**[实证：类型层级]**

### 5.2 EA::IO::IStream vtable 复原（slot = 偏移/4）

证据：`GB_cStreamXOR.c`（转发 thunk L83-253）、`GB_cStreamRateLimited.c`（L77-216）、`GB_cStreamChildWrapper.c`、`GB_cMultiFileStream.c:501-713`。

| 偏移 | slot | 语义 | 置信 |
|---|---|---|---|
| +0x08 | 2 | Release（析构中释放子流） | 实证 |
| +0x24 | 9 | GetPosition（XOR Seek 后 `pos % keylen` 重置相位） | 实证 |
| +0x28 | 10 | SetPosition(offset, whence)（0=begin；DeltaChunk 频繁调用） | 实证 |
| +0x2c | 11 | GetSize（DeltaChunk 用它做容量校验；ChildWrapper 本地算 end-pos） | 实证/实证 |
| +0x30 | 12 | Read(buf,size) | 实证 |
| +0x38 | 14 | Write(buf,size) | 实证 |
| +0x10..+0x20 | 4-8 | 只读属性 getters（AccessFlags/Format/CanRead…） | 存在实证，语义推测 |
| +0x34 | 13 | getter（CanRead/GetAvailable） | 推测 |

类 ID getter（COM 风格）：StreamXOR `0x10d15699`（`GB_cStreamXOR.c:75-79`）、RateLimited `0xd661c50`（`GB_cStreamRateLimited.c:67-73`）、ChildWrapper `0x3472233a`（`GB_cStreamChildWrapper.c:48-54`）。**[实证]**

### 5.3 三个装饰器要点

- **cStreamXOR**（`GB_cStreamXOR.c`）：`+0x0c` key、`+0x10` keylen、`+0x14` 相位；Read 在子流 Read 后逐字节 `^key[phase++ % keylen]`（L194-207），Write 先异或再写（L226-254）；Seek 后相位 = `child->GetPosition() % keylen`（L163-175）。**.egb 存档与 delta 流的解密即此**。**[实证]**
- **cStreamRateLimited**（`GB_cStreamRateLimited.c`）：子流在 `+0x18`，Read/Write 前调 `FUN_005c6200(size)`（令牌桶预算，函数未导出）再转发 `vf+0x30/+0x38`（L183-216）。**[实证结构]**
- **cStreamChildWrapper / 子流视图**（`GB_cStreamChildWrapper.c` L123-245）：`{parent@+0xc, base@+0x10, pos@+0x14, end@+0x18}`，Seek 支持 whence 0/1/2，GetSize=剩余量 —— 区间流。**[实证]**

### 5.4 Delta 系统（`GB_cDeltaManager.c` 944 行 + Chunk/Stream/Task）

- **EA::Messaging 订阅**：`FUN_005ba7f0/8290`（L31-74）经 `vt+0x24`/`vt+0x2c` 注册/反注册消息 `0x1dd7bda9`（附带 `0xffffd8f1`）；同模式遍布 `GB_cGameManager.c:41,67`、`GB_cNetUserManager.c:42`、`GB_cTelemetryManager.c:51` 等 → `0x1dd7bda9` 是全局消息类别。**[实证]**
- **OnEvent 分发** `FUN_005b8d30`（L893-930）：`if(id==0x1dd7bda9)` 再按 `msg+0xc == 0xc329b77`（路径 A：`FUN_005b27a0(msg+0x10,…)`）或 `0xd83e017`（路径 B：`FUN_005b2690/2680/7aa0`）处理。**[实证]**
- **任务记录**（vector@`+0x58/+0x5c`，元素 `{+0x0 obj, +0x0c conn(int,-1 哨兵), +0x10 gameId}`）：状态字段 `+0x4b8`（0=pending，2=已合并 `FUN_005bb540` L239，3=running），标志位 `+0x4b4`（0x40|0x100 组合检查，`FUN_005b7a10` L283）；连接级对象 `+0x450` bool、`+0x454` stream*。记录体 ≥0x4bc 字节（每城一条）。**[实证]**
- **流获取 = XOR 装饰**：`FUN_005bb1f0`（L522-575）`FUN_005b86a0(conn, 0x400)` 取流；若 `+0x454` 空则分配 0x2cB 并 `FUN_005c60b0(stream)` + `FUN_005c6140(conn)` —— 两个地址都落在 `GB_cStreamXOR` 函数簇内（0x5c6060 写 / 0x5c6120 类 ID / 0x5c6130 析构之间）→ **delta 传输流被 cStreamXOR 包裹，密钥来自连接对象**。**[实证（地址聚类）/细节推测]**
- **状态汇报** `FUN_005b7b20`（L422-518）：填充 0x38B 状态块 —— `+0x00` u64 总量、`+0x0c/+0x10` 上下行速率、`+0x1c/+0x20` 剩余字节、`+0x30` ETA（=剩余/速率，速率 0 时写哨兵 `DAT_00d1a784`）。这是 UI 上的云同步进度条数据源。**[实证]**
- **cDeltaChunk**（`GB_cDeltaChunk.c`）：版本化布局表是四个静态描述符 `DAT_00df513c/148/154/160`（各 `+4`=段大小、`+8`=段指针）；版本字节在 `+0x14`：<2 读 2 个 64 位序号（`+0x1c/+0x20`），≥2 读 3 个（`+0x18/+0x1c/+0x20`）——双 64 位序号支持乱序合并；`FUN_005c1620`（L129-175）按 `-1` 哨兵字段惰性定位段起点；`+0x09` 状态字节 2/4 触发 seek 重放（`FUN_005c1710`）。**ServerDeltaChunk**（`GB_cServerDeltaChunk.c`）有 4 套布局（`DAT_00df513c..0x184`），版本 0/1/2+ 字段数不同，多带 `FUN_008ebb10` 写的 (值,长度) 对。**[实证结构]**
- **cBackgroundStreamsTask**（`GB_cBackgroundStreamsTask.c`）：StateFlow 风格状态机，状态字 `+0xc`，`switch(state-3)` → 状态 3/4/7/8；`+0x120` = **DeltaManager 指针**（其 `+0x58/+0x5c` 即 §5.4 任务 vector），配置键 `0xe2e5503`=最大并发、`0xe2e5504`=超时秒（`vt+0x1c(key)` 查存在 + `FUN_004090f0(key)` 取值，L52-68）；状态 8 按 `限额 - (状态3数 + 状态0数)` 调 `FUN_005beae0(+0x128, 配额)` 补启动下载，状态 1 时逐个 kick 状态 0 的任务（`FUN_005b7140`）。**[实证]**
- **存档侧同一基座**：`GB_cLocalSaveManager.c:2060` 格式串 `u_s_state_file_0__lld_egb`（`state_file_0_%lld.egb`），随后 0x22cB 流对象 `FUN_008e3820(name)` → `vf+0x4c(2,2,1,0)` 打开 → `vf+0x18(stream,-1)` 挂接 → 读写 → `vf+0x24` 关闭。与 delta 共用 IStream 族。**[实证]**

---

## 6. 对 OpenSCP 的可复用结论（只读 eco 查看器/编辑器路线）

### 6.1 必须还原的结构（按优先级）

1. **SwarmMap 三元组** `{AABB[6]f32, width/height, int32 网格 ×scale}` —— 语义完整、11 个函数已还原（§2）。任何 eco 可视化（热力图/等值线）只需实现 `FUN_00c1e9e0/c1eb00` 的最近邻/双线性两把采样器。
2. **EcoGameEffect 属性协议**：`{mgr=QI(0x87abfd1), effectId=desc+0xc, Begin(id,params)/End/Advance}` + property 6/7 —— 做"效果播放器"编辑器的最小接口面（§4.4）。
3. **Beat 规则表**：blob 偏移 `+0x50/+0x54`（及相位 B `+0x60/+0x64`）、12B 步长 ushort 掩码、`0x8000` 立即广播位、20B 事件记录、28B 规则条目 —— 规则查看器直接按此解析运行时 blob（§3.2）。
4. **DeltaChunk 版本表**：静态布局描述符 `DAT_00df513c/148/154/160/16c/178/184`（Ghidra 中直接读这 7 个结构体即可得到 0/1/2+ 三版字段表）+ 双 64 位序号 —— 若要做云存档/增量检查工具。
5. **IStream vtable 槽表**（§5.2）—— 读写 `.egb`（需先过 StreamXOR，密钥在连接/存档对象中）的前置。

### 6.2 接入方式建议

- **没有 eco 专属文件格式**：eco 场是运行时对象，package 侧只有 property/规则表。只读查看器有两条路：进程外内存读取（按 §2 布局扫 `cEcoSwarmMap` vftable → 遍历实例）；或 hook 两个采样函数 `0x00c1e9e0`（最近邻）/`0x00c1eb00`（双线性）做旁路记录——后者能顺带拿到采样点语义。**[实证地址/推测方案]**
- **编辑路径**：安全入口是 property 6/7（按 ID/索引换效果）与 beat 规则掩码位；直接写网格值可行但写路径未还原，需先在 Ghidra 找 `cIMap::Set`（Swarm 原生类，dump 外）。**[推测]**
- **需要回避的坑**：`FUN_00800000`、`GetPoolName`、成排空函数是 IDA 切块伪影，不要据其建模；`cTerrainEcoMap*` 组合链、`cIEco*Handler` 实现均缺 dump，二期需补 `(0x00be8fb0-0x00bf7xxx)` 与 `cSwarmGameHeightField@?A0xf5e3c6ee` 两个区域。

### 6.3 一句话架构结论

GlassBox 的"经济"不是交易网络，而是：**规则表（property hash + ushort 掩码，beat 驱动、队列背压）→ 写/读 int 网格场（Swarm map，世界 AABB + scale）→ 场变化经 property 协议触发效果（cEcoGameEffect：Begin/Advance/End，0x87abfd1 接口）→ 效果目标列表回调受体 → UI 观察者消费**。OpenSCP 若做查看器，按"场快照 + 规则表转储 + 效果日志"三件套即可覆盖 90% 可观察性。