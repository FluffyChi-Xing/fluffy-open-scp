# ER2 规则语言与 EcoGame 脚本机制

> 2026-09-25 建立。基于 EcoGame 脚本包全量取样（patch 287520926）、
> SimCity_dump_SCY.exe（脱壳）Ghidra 逆向、39 份 AEC 文本规则源码 +
> 73 份 AEB 编译规则库取证。
> 解析实现：`crates/dbpf/src/erz.rs`（AEB 结构解析，三文件精确消耗验证）。
> 姊妹文档：[`glassbox-engine.md`](../glassbox-engine.md)、
> [`saves-exploration.md`](../saves-exploration.md)、[`file-formats.md`](../file-formats.md)。

---

## 1. 结论速览

| 问题 | 结论 |
|---|---|
| EcoGame 脚本包是什么 | 服务器端模拟规则下发缓存：`SimCityUserData\EcoGame\*-Scripts_<版本>.package`，标准 DBPF |
| 包内容构成 | N 条 `0x00B1B104` Property（SCUnit 单位数据，kPropSCUnit* 键面）+ 恰 1 条 `0x08068AEB` ERZ 编译规则库 |
| ER2 文本规则 | `0x08068AEC`（"ER2 Rule File"），UTF-8 源码，`unitRule/globalRule/define/create` 语法 |
| ERZ 二进制规则 | `0x08068AEB`（"ER2 Binary Rule File"），文本规则的编译产物，大端 u32 流 |
| 两者关系 | AEC 是源码（仅 DLC 补丁带少量），AEB 是编译产物（每包必有）；`include "....er2"` 指向源码树路径 |
| 游戏如何加载 | exe 请求抽象类型 `0x08068AE9`（资源管理器注册 AEB/AEA 为其实现），`Setup Scripts` 按清单装载，组 ID 基址 `0x40800200` |
| 与存档关系 | `.egb` 状态快照头 `00 00 00 <ver> 62 2b 9c d7`——`622B9CD7` 就是 SimCity 主规则库 AEB 的 instance（同一个哈希常量），存档 = 规则库的状态投影 |

## 2. 两种 ER2 资源（魔数/类型对照）

| 类型 ID | 注册名 | 形态 | 分布（patch 287520926 全量扫描） |
|---|---|---|---|
| `0x08068AEC` | ER2 Rule File | UTF-8 文本（CRLF） | 39 条：H&V 23、Sandbox 2、其余 DLC 少量，group `40800201` |
| `0x08068AEB` | ER2 Binary Rule File | 二进制（见 §4） | 82 条：每个脚本包恰 1 条；SimCity 主库 13 份版本（instance `622B9CD7`，6.0MB）+ 各 DLC/模式库（group `40800201`） |
| `0x08068AE9` | （注册表无名） | 抽象类型 | exe 内为请求类型；AEB/AEA 注册为其实现（`FUN_005c9320`：`{0x8068aeb, 0x8068aea}` → vtable+0x24 `registerImpls(0x8068ae9, …)`） |
| `0x08068AED` / `0x08068AEE` | EcoGame State Data/Table | gzip 状态 / 12B 记录表 | EP1 包 145+15 条；与 `.egb` 同魔数（见 saves-exploration.md） |

文本样例（`HeroesAndVillains AEC 8F06F716`）：

```text
## Villain Base
set VillainGarage_VillainPointsNeeded 3000 #villain points to plop vu garage
```

## 3. ER2 规则语言语法（AEC 文本源码全集普查）

语料：39 份 AEC 共 2728 行，全部指令 token 如下（按频次）：
`global(375) unitRule(227) end(234) applyCount(142) local(119) create(91) agent(90)
successEvent(89) onSuccess(69) chain(59) options(46) onFail(42) include(36)
connected(32) maxApplyCount(28) set(26) flags(20) endif(19) timeTrigger(17)
define(14) enddef(14) globalFlags(13) else(11) rate(10) priority(8) globalRule(8)
if(6) testAgent(5) map(2) elseif`。

### 3.1 顶层结构

```text
unitRule <Name> … end          规则块（单位级，最常见）
globalRule <Name> … end        全局规则（城市级触发器，如 Sandbox 解锁）
define <Name>( params ) … enddef   宏定义（编译期展开）
create <Name>( args… )             宏实例化（一份定义多次展开）
set <Name> <Value> #comment        常量定义（编译进 ERZ 常量表）
include "../../../Core/Eco_Crime/Rules/CrimeTuning.er2"   源码树 include
```

编译期插值：宏参数 `&{param}`（名字替换）、常量引用 `${Name}`（值替换）；
编译期条件 `if( match(&{param}, "Literal") ) / elseif / else / endif`。

### 3.2 规则体指令（GlassBox 记号流语义）

GlassBox 模拟是记号（token）流引擎：规则消费/生产各作用域的命名资源，
满足条件即"点火"，并按路由把 agent 记号发送到后续规则。

```text
# 条件/生产（核心三段式：<scope> <Name> <verb> [value]）
global X is 0            条件：全局资源 X 等于 0
global X in 1            消费：从全局池取走 1 个 X
global X out 1           生产：向全局池注入 1 个 X
global X out !X@global   生产"取反值"（@global 作用域限定）
local  X atLeast 1       条件：本单位局部资源 ≥ 1
agent  Criminal in/out 1 agent 记号进出（模拟代理）
connected MaxisMan in 1  连接槽位资源（建筑间管道）
map    Crime out ${N}    向城市地图层写入数值

# 控制流
applyCount 1             单次模拟节拍内最多点火次数
maxApplyCount            上限（可与时间窗组合）
priority 10              同拍竞争时的点火优先级
rate 61                  每小时点火率（rate rules）
timeTrigger Hour 0.5 -variance 0.5   时间触发器（Hour/Day，-fromCreate -count N）
flags -if OpenForBusiness ActiveBuilding   位标志条件（可多个）
globalFlags -ifNot SandBoxActive -set SandboxActive   全局位标志：条件+置位

# 路由
onSuccess <Rule>         点火成功后链
onFail <Rule>            条件不满足时链
chain <Rule>             无条件后续
options -sendTo X -or Y -resendTo Z -switchTo W <n> -createAgent A -perApplication
                         agent 记号的多路发送/重发/换名/生成
successEvent uiEvent <Name>                            UI 事件
successEvent telemetry SC_RULE_MISSION_STARTED -resource global X   遥测
testAgent <Name> …       agent 存在性测试
```

### 3.3 语义锚点（文本 ↔ 编译产物互证）

- AEC `successEvent telemetry SC_RULE_MISSION_STARTED` 的常量字符串
  **原样出现在对应 AEB 尾部池**（HV 库：`SC_RULE_MISSION_STARTED/ENDED`、
  Sandbox 库：`SC_CHEAT_MONEY`，主库：`SC_RULE_PLAYERBANKBALANCE` 等 1.5MB 符号池）。
- AEC `set Name Value` ↔ AEB 常量表 `(hash, value)` 对（见 §4.1 尾段）。
- AEC `include "../../../Core/Eco_Crime/Rules/CrimeTuning.er2"` 证明：
  EC 库（`40800200`/`622B9CD7`）= 官方源码树 `Core/` 全部 `.er2` 的编译产物；
  零售盘不带 Core 源码，只有编译结果。

## 4. ERZ 二进制规则库（0x08068AEB）格式规格

证据链：SimCity_dump_SCY.exe 序列化器 `FUN_005fc7a0`（写入端）逐段反编译 +
三个真实库**精确消耗到文件尾**的逆向验证（SimCity 5,947,232B / HV 67,229B /
Sandbox 11,053B，patch 287520926，全部 `exact`）。全部整数**大端**（与
`.egb`、`0x08068AED` 状态表同族——同一条 `write_u32_be` 原语族，
`FUN_008ebab0/b9c0/d60` 只是 C++ 重载不同）。

```text
u32   5            布局主版本（旧补丁包=4 → 布局不同，OpenSCP 拒绝并回退 hex）
u32   2            布局次版本
u32   14           规则记录布局版本（旧包=13）
u8    flag         布尔（样本全 0）
u32   ruleCount
ruleCount × RuleRecord            ——见下
u32   5；u32 nA；nA × 18×u32                 ——段 A（tag 5）
u32   8；u32 nB；nB × (10×u32 + 4×u64 + 8×u32)  ——段 B（tag 8）
u32   2；u32 nC；nC × 变长记录               ——段 C（tag 2）
u32   pairCount；pairCount × (u32 hash, u32 value)   ——常量表
u32   outerD；outerD × (u32 inner, inner × 20B)      ——D 段
u32   outerE；outerE × (u32 inner, inner × 20B)      ——E 段
u32   blobLen；blobLen 字节                  ——字符串/符号池
```

RuleRecord（内存步长 0x9C，序列化器 `FUN_005fc1c0`）：

```text
u32  规则名哈希         （GlassBox hashifier；查名走 s3db Properties 表）
u32  伴随哈希           （模块/作用域归属，推断）
u32  f2
5 × (u32, u32)          条件/动作对（p!=0 时成对出现）
4 × u32                 g13
10 × u32                h17..h26（含 3 个哈希位形字段）
3 × u32                 v3（f32 三元组候选）
u32 h30；u32 f31；u32 f32
u32 subCount；subCount × (u32 hash, u32 a, u32 b)   ——12B 子条目
u32 h37；u32 h38
```

段 C 记录：`u32 hash ×3` + `u8` + `2×u32` + 三个子数组
（`hash,u32,f32` × n1、同 × n2、`f32,f32` 对 × n3）。
段 D/E 条目（20B）：`hash` + 8B 子对象(2×u32) + `hash` + `u16` + `u8` + `u8`。

实证计数（SimCity 主库）：rules=28441、A=56、B=573、C=95、常量=1716、
D=1716 对象/2651 条目（**D 段对象数与常量表条目数相等——按序耦合**）、
E=573 对象（=B 段数）0 条目、池=1,517,060B。
D 段外层计数在流中出现两次同值（1716）非巧合：常量与 D 段一一对应。

### 4.1 字段语义对照（AEC 源码 → AEB 字段，推断）

| AEC 指令 | AEB 对应（推断） |
|---|---|
| `unitRule <Name>` | RuleRecord.name_hash |
| `set <Name> <Value>` | 常量表 (hash, value) |
| `successEvent telemetry <Const>` | 尾部池中的常量字符串 |
| `rate/applyCount/priority` | 条件/动作对与 f32 字段（待逐位对照） |
| `onSuccess/onFail/chain` | 子条目数组（hash+2×u32） |

哈希函数未破解：FNV-1/FNV-1a/CRC32/Jenkins/djb2 全家桶对已知规则名
**不命中**（含 s3db `kRuleID*` 表——注意该表 ID 是顺序分配与哈希混排，
`kRuleIDSCPlayerRuleOnExitBox=0x83FF7538` 恰为 FNV-1(小写) 属巧合）。
exe 内 `hashifier: %x` 字符串无静态引用（死代码/仅调试）。
查名唯一现实路径仍是 s3db Properties 表 + property 键面。

## 5. EcoGame 脚本包加载机制（exe 证据链）

1. **资源注册**：`FUN_0041caf0(0x8068aea, 0x8068aeb, 0x8068ae9, 0)` +
   `FUN_005c9320` 把 AEB/AEA 注册为抽象类型 `0x08068AE9`（EcoGame 规则集）
   的实现；处理器对象 0x188B（`FUN_005c9e90` ctor，vftable @0xD1B018/0xD1B02C）。
2. **存在性检查**：`FUN_008a10d0`（Setup Scripts）：
   `if (resMgr.contains(0x622b9cd7)) loadGroup(0x40800200)`——按规则库
   instance 哈希探测脚本包是否存在，再装载其组。
3. **脚本清单**：`FUN_005a6d10` 从配置资源
   `(0x0CC26860, 0x20, 0x0CC26861)`（运行时虚拟 TGI，包内不存在）读清单，
   12B 条目 `{instance, ?, group}`，**group=0 回退 0x40800200**；
   日志 `INFO: Setup Scripts Info (from Config)`。
4. **TGI 组装**：`FUN_005c6d70`：`group = DAT_00df56c0 + dlcDelta`
   （基址 0x40800200，DLC +1 → 0x40800201），与包内 group 完全吻合；
   装载时以 `SP::cKeyFilter(0x00B1B104)` 过滤 property 条目。
5. **资源到达**：`FUN_005c9980`（vtable slot 12）：
   `param_5 == 0x8068ae9` → 分配 0x15C 字节规则系统对象（`FUN_005c98f0`
   ctor，`EA::ResourceMan::Resource`/`SP::cResourceBase` 派生），
   TGI 存 +0x14C，转交加载。
6. **载荷消费**：`FUN_005c92c0` 校验资源子类型 `0x08068aeb` 后把数据指针
   交给 `FUN_005fc7a0`（即 §4 的反序列化器）；运行期开关
   `includeRuleNames` / `excludeRuleNames`（`FUN_005c93d0`）可过滤规则。
7. **网络协议**：客户端↔服务器以 `\r\nData:\r\n` / `\r\nScripts:\r\n`
   分节交换模拟状态与脚本（`FUN_005b10c0`，`GB_EcoGame` 命名空间），
   落盘即 `.egb`（gzip）+ `SLDelta*.mfs` 增量（见 saves-exploration.md）。
8. **常量伴随**：规则类 vftable 后紧跟 f32 常量表
   （π、2π、π/2、FLT_MAX、1/3…），为规则引擎数学求值所用。

版本语义：文件头布局版本随游戏补丁演进（SimCity-Scripts 包在本机留存
patch 257365692→287520926 共 13 份，老包 (4,2,13)、新包 (5,2,14)）；
游戏只读最新补丁包，旧包为残留。

## 6. OpenSCP 支持（本轮落地）

| 项 | 位置 | 说明 |
|---|---|---|
| AEB 结构解析 | `crates/dbpf/src/erz.rs` | `erz_parse_summary`：完整布局校验（精确消耗），layout 14；旧补丁布局（13）降级为仅文件头（`layout_supported=false`） |
| AEB 预览命令 | `read_erz_preview`（package_service） | 结构摘要 DTO：版本三元组/各段计数/常量表样本 256 条/池内符号名样本 200 条/`exact` 标记 |
| AEB 预览 UI | `ErzPreview.vue` | 结构统计卡 + 常量表 + 符号名候选列；解析失败回退 hex |
| AEC 文本预览 | `resource-types.ts` | `0x08068AEC` 注册 `er2` 语言（文本预览管道直读） |
| 单测 | `dbpf::erz::tests` | 最小 fixture 精确解析 + 旧布局拒绝 |

## 7. 后续工作

1. **字段语义定名**：以带 AEC 源码的 H&V 库为 Rosetta 石（46 个已知规则名
   + 完整源码），对 rule record 的 5 组 (p,q) 对与子条目数组逐位定名
   （rate/applyCount/flags/onSuccess…）；顺带破解 hashifier。
2. **AEB → ER2 反编译**：D/E 段与 rule record 齐备后可重构规则图。
3. **`.egb` 状态表**：与 ERZ 同族序列化，破解 ERZ 后状态表可期
   （存档改钱/改资源的路径）。
4. **脚本包编辑**：Property 面已有 `patch_property_overlay`；
   AEB 写回需完整重序列化器（同 crate 可逆实现）。
