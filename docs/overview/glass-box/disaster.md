# SC::Disaster — 灾难系统深扫笔记

> 深扫笔记（2026-09-12）。扫描文件：SC_cDisasterGame.c、SC_cToolDisasterPlop.c、SC_cToolTornado/UFO.c、SC_cTornado/UFO/UFORaid/Meteor/MeteorShower/ZombieNight/Robot/SpaceRobot/Monster.c、SC_cGraphicsUnitVandalism.c。
> 置信度：【高】= 代码直接可证；【中】= 强邻近性/模式推断；【低】= 合理猜测。函数地址为 exe 内 VA。
> 定位说明：本 dump 属 **app/表现层**（SC::cGraphicsGame 体系），模拟服务端的 resource field 写回不在 dump 内（第 4 节有诚实边界）。

## 0. 总体定位（先说结论）

本 dump 是游戏的 **app/表现层（SC::cGraphicsGame 体系）**，不是 GlassBox 模拟服务器本体。所有灾难类都是 COM 风格（`EA::COM::IRefCount` / `EA::RefCountTemplate` vtable）的"灾难活动对象"（disaster actor），它们：
- 由 `cDisasterGame` 管理器统一登记/更新/销毁；
- 通过 city 接口（`param_1+0xc` 处对象的虚表）查询 lot/building/transport 数据并施加破坏；
- 真正的 resource field 写入（幸福度/经济）不在本 dump 可见范围内（见第 4 节的诚实边界说明）。

证据级别说明：【高】= 代码直接可证；【中】= 强邻近性/模式推断；【低】= 合理猜测。

---

## 1. 灾难类型枚举还原（13 类）

核心证据：`SC_cDisasterGame.c` 的 `FUN_00702770`（触发分发函数，`__thiscall(this, param_2=属性包, param_3=参数)`）中一个 `switch(*puVar4)` 覆盖 **case 1..13（0xd）**，每个 case 调用不同的工厂函数把灾难对象加入活动列表【高】。另有 `FUN_00bdd200` 直接 `return 0xd`（13），疑为 "灾难类型总数" getter【中】。

工厂函数与灾难类文件的对应关系依据**地址邻近性**（同一编译单元函数地址连续）还原：

| case ID | 工厂函数 | 归属 TU（地址区间证据） | 还原灾难名 | 置信度 |
|---|---|---|---|---|
| 1, 3 | `FUN_00892330` | 0x8922c0–0x892700 = `SC_cMeteorShower.c` 区间（`FUN_008922f0` 析构、`FUN_00891e40` 接口查询同时接受 0xdef1434 与 0xdef1510） | 陨石雨 / 陨石（分享同一工厂，case 1/3 共用分支，触发前会先遍历活动灾难查询接口 `0xdef1510`，取已有灾难的位置 `+0x64/+0x68/+0x6c` 作为出生点） | 中 |
| 2 | `FUN_00893210` | 0x892700–0x893200 = `SC_cMeteor.c` 区间（`FUN_00892700` 为陨石更新/撞击） | 单颗陨石（Meteor） | 中高 |
| 4 | 无工厂调用，只写入 GUID 对 `DAT_00dfe3f8/DAT_00dfe3fc` | — | 无本地灾难实体的类型。候选：**地震**（地面形变类，无需表现层 actor）或纯 sim 端灾难 | 低 |
| 5 | `FUN_0088ef40` | 0x88eb40–0x88f020 = `SC_cUFO.c` 区间（`FUN_0088f020` UFO 状态机） | UFO | 高 |
| 6 | `FUN_008902e0` | 0x890240（`SC_cTornado.c` 析构）之后 | 龙卷风（Tornado） | 高 |
| 7 | `FUN_0088d540` | 0x88d540 紧邻 `SC_cUFORaid.c` 的 `FUN_0088d750`/`FUN_0088d980` | UFO 掠袭（UFO Raid，多机编队） | 高 |
| 8 | **直接 fall-through，无任何操作** | — | 保留/被砍类型（占位）。无法还原身份 | — |
| 9 | `FUN_008898c0` | 0x888d50–0x889b70 = `SC_cRobot.c` 区间（`FUN_0088ace0` 机器人状态机） | 机器人（Robot，Vu 的机器人） | 中高 |
| 10 | `FUN_008842f0` | 0x8842f0 紧邻 `SC_cZombieNight.c` 的 `FUN_00884370`/`FUN_008843a0` | 僵尸之夜（Zombie Night） | 高 |
| 11 | `FUN_00881e60`（多传一个 bool：`*(param_1+0x41)=='\0'` 即"灾难系统是否未暂停"） | 0x881a80–0x883e90 = `SC_cMonster.c` 状态机函数区间 | 怪兽（Monster） | 中 |
| 12 | `FUN_0087edd0` | 0x87edd0，紧邻 `SC_cMonster.c` 内辅助块起点 `FUN_0087ee50`（0x87ee50）；也可能是 SpaceRobot TU 尾部 | 怪兽变体（候选：**大脚怪 Bigfoot** 与 case 11 共用 cMonster 类、不同工厂/参数） | 低 |
| 13 | `FUN_0087e430` | 0x87e430 紧邻 `SC_cSpaceRobot.c` 的状态变更 `FUN_0087e490` 与更新 `FUN_0087e520` | 太空机器人（Space Robot） | 高 |

辅助枚举/接口 ID（32 位 hash，即 MUUID 风格 ID）：
- `0xdef1434`：所有灾难单元共享的接口/类型 ID（每个灾难文件都有同一个 `FUN_00891e20`：`hash != 0xdef1434 && hash != 0xEE3F516E` 则 mask 掉）【高：代码存在；中：语义="IDisaster 基接口"】。
- `0xdef1510`：仅 MeteorShower 的 `FUN_00891e40` 额外接受；`FUN_00702770` case 1/3 用它向活动灾难查询位置 → 语义="可提供出生位置的灾难"（陨石雨）【中】。
- `0xAE9CB0FA`（代码中写作 `-0x51634f06`）：`FUN_008a6080`（ToolDisasterPlop）与 UFORaid/SpaceRobot 的接口查询 `FUN_008ab2a0` 共用 → 疑为 "IDisasterActor" 共享接口【中】。
- `0xEE3F516E`：出现在相机、雨云等大量非灾难文件 → 通用对象接口（如 IGraphicsObject）【中】。
- case 4/6/7/9/10/11/13 各自把一对 32 位常量（`DAT_00dfe3xx/DAT_00dfe3xx+4`，.data 段，dump 未含初值）作为 **GUID 对** 存入活动记录 → 即"每个灾难有自己的 MUUID 对"，但具体值需运行时 dump【高：结构；低：数值】。

与社区口径的对照：还原出的独立表现类共 9 个（Meteor、MeteorShower、Tornado、UFO、UFORaid、ZombieNight、Robot、SpaceRobot、Monster），加上 case 1/3 拆分与 case 4/8 两个无实体项，总数恰好对上 13【中】。**未发现 Fire/Earthquake 专属文件**（全目录 grep 无 fire/quake 命名文件）；火灾大概率走建筑燃烧状态（本 dump 未含），地震对应 case 4 的猜测见上。

---

## 2. 灾难生命周期

### 2.1 管理器结构（`SC_cDisasterGame.c`）【高】

`cDisasterGame`（本文件主体）字段（偏移还原）：
- `+0x14/+0x18`：活动灾难记录 vector（16 字节/项：`{ disasterObj, id, key, guidPair... }`）
- `+0x28`： intrusive list 头（`+0x2c/+0x30/+0x34`），`+0x38` 计数
- `+0x40`：缓存布尔（游戏速度状态，见 2.4）；`+0x41`：暂停标志；`+0x42`："本帧触发了灾难"标志
- `+0x0c/+0x10`：city / 辅助接口指针

关键函数：
- `FUN_00702770` = **TriggerDisaster(typePropBag, param)**：校验参数包类型 `*(short*)(param_2+0x12) != 9` 则退出（9 = 变体类型 u32；同文件 `==1` 为 bool、`==0xd` 为 float，见 ToolDisasterPlop）【高】；按 case 分发创建灾难对象；若 `+0x41`（暂停）则立即对新对象调用 vtable+0x10（暂停入口）；然后构造记录 vector push：id 来自 `FUN_005cbb10(&param_3)`（hash→索引），key 来自 city vtable+0x84（按 key 取记录），GUID 对写入【高】。
- `FUN_007023f0` = **RemoveDisaster(id)**：查到后 swap-pop 删除，释放对象，调用 `FUN_008eadc0/008eb0c0`（外部登记表），最后经全局游戏对象 vtable+0x174 发通知【高】。
- `FUN_007017c0` = **ClearAll**：遍历销毁全部记录、清 overlay 链表、复位标志【高】。
- `FUN_00700ce0` = **PauseAll**：`+0x41=1`，对每个活动灾难调用 vtable+0x10【高】。
- `FUN_007018a0` = 消息处理：`param_2==2` 时按 key 定位记录并对对象调用 vtable+0x20（销毁请求）【中】。
- `FUN_007047b0 / FUN_007047d0`：5 参数包装器，转调 `FUN_00702af0(param_1, param_5, param_2, param_3, 1/0)` —— 参数布局（type, x, z, flag）极像 **脚本/控制台/网络触发的 "TriggerDisasterAt(type, x, z, true/false)"** 入口【中】；`FUN_00702af0` 本体未 dump。

### 2.2 触发路径（三种）【高，结构层面】

1. **Plop 工具**：`SC_cToolDisasterPlop.c` —— 玩家放置"灾难 plop"（Dr. Vu 风格）。工具从 plop 的属性包读取（`vf6`，属性 hash → 缓存）：
   - `0xe950ce7`（u32→+0x00）：**灾难类型 GUID**（对应第 1 节枚举的 MUUID）
   - `0xd47adb9`（u32→+0x08）
   - `0xe950d75`/`0xeaa434b`/`0xeab367a`/`0x1074a983`/`0xf1fa4aa`（bool→+0x0c..+0x10）：行为开关
   - `vf10`（放置校验）：地形射线取点 → 若 +0x0c 则对齐地形法线 → `FUN_008ab2c0` 可达性/放置检查 → `FUN_007bd250`（重叠→error=2）、`FUN_007a91a0`（无效→invalid）、`FUN_007bd2b0`（城市检查→error=1）；错误码存 `+0x20`（0=OK,1,2,3），有效性 `+0x12`【高】。
   - `vf15`：ghost 渲染 + 放置音效 `0xc8d1d793`【中】。
   - 解锁条件（`FUN_008a6400`，三个工具共用）：lot 资源 `0xe72d8eb`>0、`0x96cb206c`>0 置标志 +0xa61/+0xa62；当前城市名（`FUN_00a0e940`）必须命中列表属性 `0xf90e380`（fallback `0xf90e381`）之一（`FUN_008e6dc0` 比对）——即 **"允许触发灾难的城市白名单"**【高】。
   - 配额/阈值：`FUN_008a6720`（资源 `0xbf5d2e3` 计数 vs 属性 `0xbf5d2e4` float 阈值，超出则禁止）、`FUN_008a6290`（lot+0x210 资源 `0xa1c4a200` 当前值>0 且 `0x9f5b9909` 上限未满）、`FUN_008a6320`（资源 `0x933277bb`，返回 -1 若无）、`FUN_008a66a0`（资源 `0xe531ef8`）；**作弊覆盖**：属性 `0xebb61b7` 为 bool 时直接放行（经 `FUN_00405cb0` vtable+0x68 作弊服务）【高：结构；中：各 hash 语义】。
   - `FUN_008a63e0`：判断当前工具 id == `0x134cdac9`（灾难 plop 工具自身 ID）【中】。
2. **随机/脚本/网络触发**：走 `FUN_00702770` 本体（见 2.1）；`FUN_007047b0/d0` 包装器暗示存在直接按 (type,x,z) 的触发 API【中】。
3. **区域级事件**：`SC_cUFORaid.c` 的 `FUN_0088d980` 按中心 `+0x28/+0x2c/+0x30` 以 cos/sin 圆环布点批量生成 UFO 实体（原型 GUID `0x476a98c7`），`DAT_00d04464` 秒后给所有子机置 `+0x84=1`（离场标志）【高】。

### 2.3 传播/进行时【高】

- **龙卷风**（`SC_cTornado.c` `FUN_00890670`，逐帧 `param_2=dt`）：
  - 游走：`rand()%200-100`（两次）生成随机转向，经归一化与 `+0xa8..+0xb0` 历史速度混合，再与目标方向 `+0x98` 权重插值；"吸力阶段"由 `+0xc4/+0xc0` 计时，强度曲线含 `+0xb4..+0xbc` 平滑【高】。
  - 吸起/抛掷：维护被吸对象表（`+0x10c` 起，16 字节/项：状态 0..3、对象 key、**存活计时 float +0xc**）。每帧计时 `-dt`；到 0 时经 `FUN_00641fa0` 拿 building 记录，置 `+0x54=2`（=破坏/摧毁状态）和 `+0x48=DAT_00cf2ce4`，然后从表中 swap-pop 删除【高】。路径段表（`+0xfc`，stride 0x1c）每段含子表（stride 8：lot id + float），`+0x14` 状态 1/2/3 流转；状态 2 时给随机冲量 `(rand()%200-100)*DAT_00d1d698` 调用对象 vtable+0x18 —— **被卷起建筑的抛飞**【高】。
  - 吸取范围查询：`FUN_007bac30(city, pos, radius, &list)`（半径 = sqrt(`+0x58`²+`+0x5c`²)），对命中的对象取位置（vtable+0x1d8），距离阈值 `+0xdc` 判定：近者走 city vtable+0x138（携带 `0xe13dfa8`，疑为"标记摧毁/伤害"属性）【中】，远者调用对象 vtable+0x90（`id,1,1`，伤害/强制状态调用）【中】。
  - 结束条件：`FUN_0088f530` 查询（外部条件，疑为时长/强度耗尽）与 `+0xe8` 上一帧值比较做沿触发；`+0xe0` 计时超过 `DAT_00cf1e4c` 直接结束；音效对象 `+0x84` 按需创建/销毁（hash `0x337421f3` / `0x5a26c273` 两个音效事件）【高：流程；中：语义】。
- **UFO**（`SC_cUFO.c` `FUN_0088f020`）：状态机 `+0x4c` ∈ {0,1,2,3,4,5,6,8,9,10,0xb}，各状态独立 handler（`FUN_0088ee30/ee50/e370/e7b0/e870/ea30/ed50/d790`）；`+0x50` 状态计时、`+0x164` 总倒计时，归零或 8/9 状态超时（`DAT_00cf2ce8`）→ 进入 `0xb`（离场），离场完成 return 1（对象自杀）【高】。事件处理 `FUN_0088eb40`：收到 `0xfeaa4f9`（绑架目标请求，arg 为实体 key）→ `+0xd4` = 目标 key 或默认 `0x87dc71d6`，状态 2/6 → 8（发 `0x6a023407`）或 8/9 路径 → 9（发 `0x72981776`）；收到 `0x45283303` → `FUN_0088e060(3)`（打断，如被击落？）【中】。每帧把光束/音量参数 `0xccc36600` 按高度差比例（`DAT_00d269c4` 系数）写到客户端实体 vtable+0x74【高：代码；低：语义="光束缩放/音量"】。
- **Robot / Monster / SpaceRobot**：同为大型状态机（Robot `FUN_0088ace0` 状态 0..0xe 共 15 个；Monster `FUN_00883e90` 状态 0..0xe；SpaceRobot `FUN_0087e520` 状态 0..0xd）。共同特征：
  - 行走状态（Robot 4/5/7；Monster 4/6）做向下射线/范围查询（`FUN_00823110`），命中点生成 **脚印尘埃特效** `0x515d48d4`【高】；
  - SpaceRobot 有"自毁计时"：`+0x11c` 累加 `speed*dt`，达 `+0x120` 阈值且状态 <0xb → 强制转 `0xb` 并 `FUN_0087b660()`【高】；
  - Robot 读目标建筑的调参属性：`0xeb0be5b`/`0xeb0bf1f`/`0xeb0be6f` 与 `0xeb0d44c`/`0xeb0d450`（float，type 0xd），用 `rand()%100` 在 min/max 间插值生成动作时长【高：代码；低：具体是"挖掘时长/停留时长"】；
  - Robot 事件 `0x5060d92f` → 直接转状态 0xc 并更新目标点（`+0x274→+0x278`）【中】；
  - SpaceRobot 还从城市数据 `*0x128(lotId)*0x128 + 0x8c` 取记录，检查 `bit12 of [1]`（疑为"是否可拆"），按全局速度状态（`0xadea19f/0xadea1a3/0xadea1a6` 三值之一，见 2.4）从 `+0x108` 数组选系数【中】。
- **Zombie Night**（`SC_cZombieNight.c` `FUN_008843a0`）：对事件 `0x4fe806a9` 响应——遍历城市 transport 列表（city vtable+0x1cc 计数 / +0x1d0 可见性 / +0x1d4 取对象），筛出 lot（`+0x31`）属于本城的 transport，经 `0xdc0d28f` 查询其路径/乘员数据（该 hash 同时被全部 `SC_cTransport*.c` 使用 → 是 transport 共享接口），然后以 `0xabb04d5d` 在路径点位置生成僵尸单位 → **"把路网上的-agent 位置转化为僵尸刷出点"**【高：流程；中：语义】。
- **Meteor**（`SC_cMeteor.c` `FUN_00892700`）：直线飞行（速度 `+0x8c`，指向 `+0x18..+0x20` 目标），逐帧射线 cast（对象 vtable+0xa4，模式 3）；命中建筑/地面后：
  - 按模式 `+0x94`（==1 与否）选撞击特效 hash `0x48ae51e3` / `0xe411ec09`，以及飞行啸声 `0x63284990` 或 `0x64ACB176`（`+0x9d` 标志二选一）【高】；
  - 对附近建筑 `FUN_00648d90` 取变换 → 计算冲击方向冲量（`DAT_00d23490`）调 vtable+0x94 —— **物理击飞**；再发特效 `0xee6b0b89`（爆炸）【高：代码；中：语义】；
  - 模式 1 时走 `FUN_007a7d90`（city 级建筑摧毁查询）或 `FUN_006373b0`+`FUN_00707e10`（另一条摧毁路径），否则统一 vtable+0x90(id,1,1) 伤害调用；置 `+0x90=1` 完成【高：流程】。
- **MeteorShower**（`SC_cMeteorShower.c` `FUN_00891e70`）：持全局标志 `DAT_00ff29a8`（由 `FUN_00891ef0` 置位），经 city vtable+0x34 → vtable+0x90(`0x933befb1`) → vtable+0x168 取目标（疑为"选择随机着弹 lot"）【低：语义】。

### 2.4 结束与全局收尾（`FUN_007045e0` 管理器更新）【高：流程；中：细节】

1. `FUN_00891f00(city)` / `FUN_00891ec0()` 包住更新（共享 `FUN_00891e20` 接口查询所需的线程/上下文登记）【中】。
2. 若 `+0x42`（本帧有灾难被触发）且游戏未暂停（全局对象 vtable+0x1c），调用游戏对象 vtable+0x174(`0xf14fd2b`, 0) —— 疑为"灾难发生"通知（镜头/存档/UI 钩子），随后清标志【低：语义】。
3. 遍历活动灾难：调用每个对象的 vtable+0x1c(dt)（=Tick，返回"已结束"），返回真则 `FUN_00695910(0, entry)` 入待删【高】。
4. 读取城市"速度资源"（`DAT_00dfe3e8` 经 vtable+0x58 + vtable+0x10c），判断数量≥1 → 缓存到 `+0x40`；变化时经游戏对象 vtable+0x154 发 `0xadea1a3`。`SC_cGameUI.c` 中 `0xadea19f/0xadea1a3/0xadea1a6` 是三档**游戏速度状态**（switch 设置 UI 速度值），`SC_cToolRegion.c` 也会发 `0xadea1a3` → 即管理器在灾难期间参与**速度协调**（很像"灾难时锁定/恢复游戏速度"）【中】。

---

## 3. 破坏模型

没有发现"血量(HP)数值"式的模型；证据指向三种机制【高：观察；中：命名】：

1. **状态标记模型（主）**：building/lot 记录是城市侧的图形记录（`FUN_00641fa0/FUN_00648010` 由 key 取记录），破坏 = 写状态字段：记录 `+0x54 = 2`（摧毁态）与 `+0x48 = DAT_00cf2ce4`（时间戳或破坏者标识）；路径/子对象用 `+0xc = 1/2` 三态（完好/受创/毁）。对应 Tornado `FUN_0088fa20`（清场时把全部被吸对象置毁）与 `FUN_00890670` 主循环【高】。
2. **计时衰减模型**：龙卷风给每个吸起对象一个 float 存活计时（`+0xc`），每帧 `-dt`，归零即毁 —— 相当于"按灾难种类决定的持握时长"，而非建筑血量【高】。
3. **概率/随机化调参**：Robot/SpaceRobot/Tornado 大量 `rand()%100`（或 `%200-100`）在 min/max 属性间插值（Robot 的 `0xeb0be5b` 系列属性、龙卷风冲量 `DAT_00d1d698`、方向抖动）→ 破坏强度由**属性表调参 + 运行期随机**决定，具体数值在 .data/property 文件中，非本 dump【高：机制；低：数值】。
4. **伤害派发统一出口**：非吸附类灾难对目标统一调用 city/对象 vtable+0x90(id,1,1) 或 city vtable+0x138(id, `0xe13dfa8`)（Tornado 命中建筑、Meteor 撞击）→ 存在一个"对 lot 应用灾难伤害"的城市级接口，参数 `0xe13dfa8` 应是伤害/摧毁属性 ID【中】。
5. **物理表现**：Meteor 冲量 `DAT_00d23490`、Tornado 抛掷冲量、被吸对象脱离后落地记录（`+0xf0` 链表逐个处理：对 lot 子对象置 `+0xc=2` 毁，对 transport 建新记录）【高：流程】。

---

## 4. 与 GlassBox 回路的耦合

先划边界【高】：本 dump 属于客户端 app 层；GlassBox 的 resource field（土地价值、幸福度、R/C/I 需求等）由模拟服务端计算，客户端通过 **city 数据接口 + lot 属性资源** 读快照。可观察到的耦合点：

- **资源读取原语**：`FUN_00606340(city, dataId, resourceHash) -> record`，取 `record+4`（int 值）与 `+8` —— 工具与管理器全部用它在指定 lot/city 数据上查询资源字段（如 `0xe72d8eb`、`0x96cb206c`、`0xbf5d2e3`、`0xa1c4a200`、`0x9f5b9909`、`0x933277bb`、`0xe531ef8`）【高】。`FUN_008a6320` 展示了完整链：lot id → `city vtable+0x128(lotId)` → 数据块 `+0x210` → 资源 hash 查询【高】。
- **破坏写回**：灾难对建筑状态的写（第 3 节 `+0x54=2` 等）与 city vtable+0x138/0x90 调用，是客户端把"视觉摧毁"上报给模拟层的通道；真正的资源后果（人口死亡、幸福度下降、土地污染）应在 sim 端由建筑摧毁事件驱动，本 dump 不可见【高：边界；中：推断】。
- **速度协调**：灾难活动期间管理器参与游戏速度消息（`0xadea19f/1a3/1a6`）【中】。
- **RMS/存档同步**：活动灾难记录含 key（city vtable+0x84 分配），与 `GB_cDeltaManager/GB_cDeltaChunk.c` 的 delta 上传体系同层 —— 灾难状态属多人同步的状态块【低：推断，未见直接引用】。
- **教程/成就钩子**：`0xf14fd2b` 通知（2.4-2）【低】。

---

## 5. Vandalism（破坏/犯罪表现）系统

`D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cGraphicsUnitVandalism.c`（仅 55 行，只有析构 `FUN_0081d620` 与通用引用计数 helper）：
- `SC::cGraphicsUnitVandalism::vftable` 确认这是 **cGraphicsUnit 派生的表现单元**，继承 `EA::COM::IRefCount`/`EA::RefCountTemplate<int>`（析构中先置 `EA::RefCountTemplate` vftable 再置 IRefCount，标准 COM 弱引用布局）【高】。
- 成员：`[0xc]` 一个引用计数数组/对象、`[2]` 另一个引用计数容器 —— 符合"持有 mesh/状态列表"的图形单元【中】。
- 全目录仅此一处引用其类名 → 它是**数据驱动**的单元（由 building 状态机/属性表 spawn，不在灾难代码里硬编码）。结合游戏行为，其角色应是建筑受破坏/骚乱（vandalism）状态的视觉载体（涂鸦、破损、暴徒表现）【低：具体形态】。
- 关联发现：ZombieNight 把 transport 路网上的 agent 位置转为僵尸刷出点（2.3），说明"市民级破坏"走 transport/agent 通道，与 vandalism 表现单元互补【中】。

---

## 6. 对 OpenSCP 的可复用结论

| # | 结论 | 证据 | 置信度 |
|---|---|---|---|
| 1 | **灾难触发器编辑器可行**：整个触发面收敛于 `cDisasterGame::FUN_00702770(typePropBag, param)`，输入只需 (灾难类型 u32 变体, 参数)。做一个注入层/内存补丁直接调它即可实现"任意灾难随叫随到" | FUN_00702770 的参数校验与 switch | 高 |
| 2 | 更便利的入口疑似存在：`FUN_007047b0/FUN_007047d0` 是 (type, x, z, flag) 5 参包装器，签名就是现成的"指定位置触发灾难"API | FUN_007047b0/d0 反编译 | 中 |
| 3 | **灾难类型表可做成枚举资产**：case 1..13 与各工厂函数已定位；但每个灾难的 GUID 对存于 .data（`DAT_00dfe3f0..dfe42c`），dump 无初值 → 需运行时/静态文件补一次常量提取，编辑器 UI 即可按名出 13 项 | switch + DAT 对 + FUN_00bdd200 返回 13 | 高（结构）/ 低（数值） |
| 4 | **解锁条件可配置**：白名单属性 `0xf90e380/0xf90e381`（城市 GUID 列表）、阈值 `0xbf5d2e3/0xbf5d2e4`、配额 `0xa1c4a200/0x9f5b9909`、作弊开关 `0xebb61b7` 全是 lot/plop 属性 → mod 改 property 即可开放/限制灾难，不必碰代码 | ToolDisasterPlop FUN_008a6400/6720/6290/6600 | 中高 |
| 5 | **Plop 灾难是"属性驱动"的**：类型 GUID `0xe950ce7` 从 plop 属性读入 → 新增自定义灾难 plop = 现有类型 + 新属性组合，无需新代码路径 | ToolDisasterPlop vf6 | 中高 |
| 6 | 三种专用放置工具（Tornado/UFO/DisasterPlop）共享同一工具骨架（`+0xaa8` 状态块、error 0-3、ghost 渲染）；复刻 OpenSCP 工具时可抽象成统一 `IToolDisaster` | 三文件同构代码 | 高 |
| 7 | UFO 原型 GUID `0x476a98c7`、龙卷风 `0x9a2baee8`、脚印尘埃 `0x515d48d4`、僵尸刷出 `0xabb04d5d`、事件 `0x4fe806a9`/`0xfeaa4f9`/`0x5060d92f` 是可直接复用的 spawn/消息 ID 常量 | 各文件字面量 | 高（值）/ 中（语义） |
| 8 | 破坏模型基于"状态标记 + 计时 + 属性调参随机"，无建筑血量表 → OpenSCP 复刻时不必设计 HP 系统，做"摧毁状态 + 概率/时长参数"即够 | Tornado/Meteor 破坏路径 | 中高 |
| 9 | 客户端与 GlassBox 模拟的耦合点已定位为：`FUN_00606340` 资源读原语 + city vtable+0x128（lot 数据）+ vtable+0x90/0x138（伤害写回）+ vtable+0x84（记录 key）。若要接管模拟后果，需在 sim 端（property/脚本层）下功夫，本 dump 无幸福度/经济写入代码 | 全文 | 高（边界结论） |
| 10 | 风险提示：case 8 为空、case 4 无实体 → 枚举表做 UI 时应允许"未实现/保留"项；Fire 与 Earthquake 的实现不在本 dump，勿在文档中断言其结构 | FUN_00702770 case 4/8 | 高 |

### 附：关键文件与入口函数速查
- `D:\rust\packages\fluffy-open-scp\docs\source-code\SC_cDisasterGame.c` — FUN_00702770（触发分发/13 类 switch）、FUN_007045e0（管理器 Tick）、FUN_007023f0（移除）、FUN_007017c0（清空）、FUN_00700ce0（全暂停）、FUN_007047b0/d0（位置触发包装）
- `SC_cToolDisasterPlop.c` — vf6（属性读入）、vf10（放置校验）、vf15（ghost+音效）、FUN_008a6400（解锁/白名单）、FUN_008a6720/008a6290（阈值/配额）
- `SC_cTornado.c` — FUN_00890670（移动/吸附/破坏主循环）、FUN_0088fa20（清场置毁）
- `SC_cUFO.c` — FUN_0088f020（11 态状态机）、FUN_0088eb40（绑架/打断事件）
- `SC_cUFORaid.c` — FUN_0088d980（编队生成）、FUN_0088b840（编队中心）
- `SC_cMeteor.c` — FUN_00892700（飞行/撞击/击飞/摧毁）
- `SC_cZombieNight.c` — FUN_008843a0（transport→僵尸刷出）
- `SC_cRobot.c` / `SC_cMonster.c` / `SC_cSpaceRobot.c` — 各自 14-15 态行走灾难状态机（FUN_0088ace0 / FUN_00883e90 / FUN_0087e520）
- `SC_cGraphicsUnitVandalism.c` — 仅析构，确认 COM 图形单元身份