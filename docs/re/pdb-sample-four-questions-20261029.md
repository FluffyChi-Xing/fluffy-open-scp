# PDB 样本四问题攻坚：hashifier / AEB→ER2 / trace 身份 / delta 流

> **2026-10-30 后续：hashifier 已破解。** GlassBox 规则名哈希 =
> **FNV-1(lowercase(name))**（seed 0x811C9DC5、prime 0x01000193，先乘后
> 异或）——与资源系统 `HashName32` 同一算法。方法：AEC 宏预处理器
> （include 图经 AEC 实例=FNV-1(文件名) 解析、define/create/`if(match)`
> 展开、嵌套规则递归解析）重建 339 条源码序规则，DFS 单调对齐到 H&V AEB
> 332 条记录（onSuccess/onFail/chain 交叉引用作硬约束，351 步收敛），
> 同期 AEB 324/332 直接命中（余 8 为补丁间改名）。`crates/dbpf/src/erz.rs`
> 已升级为双布局（14=144+12×T / 13=160+12×T）记录级精确解析 +
> `fnv1_name_hash()`，真实四文件集成验证主库 `exact=true`。
>Rosetta 工具链：`tmp/rosetta/`（aec_parse.py / align.py / name2hash.json）。

日期：2026-10-29。
样本：`D:\ea-games\simcity_dev\new-cource\Simcity Development Files Including PDB Debug files`
（SimCity.exe v253465305 retail + **SimCity.pdb 73MB 全量调试符号** + EAWebkit.pdb），
以及同目录上级的 `SimCity.unwrapped.exe.dec`（dev beta v253660365 的按函数反编译伪码，
**带 PDB 类型恢复的字段名/枚举名**）、`SimCity2013-source-tree`（41900 函数 / 13050 类型的
符号→RVA 索引 + GB 头文件）。retail 打补丁布局用 `D:\ea-games\simcity_offline\SimCity_dump_SCY.exe`
时代的 (5,2,14) 数据对拍。

方法学：**PDB/伪码给"名字"，数据精确消耗给"布局"**，两者互证后才写结论。

工具（本轮新增，均可丢弃）：

- `tmp/pdb-scan/all-runs.txt`：SimCity.pdb 全部 ASCII 符号名提取（556,017 行）
- `tmp/aeb-flag-scan/`：dbpf crate 临时扫描器（82 包 AEB 头遍历 + (5,2,14) 精确消耗验证 + 解压 dump）

---

## 1. hashifier：两条哈希，一破一定论

### 1.1 资源名哈希 = FNV-1(小写) —— 从推断升级为实锤

`SimCity.unwrapped.exe.dec` 中的反编译（VA 0x475BF0）：

```c
unsigned int HashName32(const char *s, unsigned int len) {
    unsigned int result = 0x811C9DC5;              // FNV offset basis
    for (; len != 0; ++s, --len)
        result = EASTDC_WLOWER_MAP[*s] ^ (0x01000193 * result);   // FNV prime, 先乘后异或
    return result;
}
```

同命名空间的 `EA::ArgScript::cHashFunction::EvalInt`（0x458610）直接调用
`EA::StdC::FNV1_String8(name, 0x811C9DC5, kCharCaseLower)`，其帮助文本：

> `"returns resource-system-compatible hash of string"`

即 ArgScript 表达式 `hash(Name)` 就是资源系统实例 ID 的生成口。与
`docs/overview/file-formats.md` 已有的 FNV-1 结论一致，本次补上了**函数级证据链**
（含 wchar 重载 0x8D92D0 与 `EA::ResourceMan::HashName32`）。

### 1.2 GlassBox 规则名哈希（AEB 内 mID）：运行时不存在该算法 —— 定论

新的硬证据（全部来自 dev beta 伪码）：

1. `GB::cEcoRuleBase::Read`：先读 u32 `mID`（大端），若 `namesIncluded` 再读
   长度前缀名字符串——**读进临时 `eastl::string` 后直接析构丢弃**。全流程无任何
   name→hash 调用。名字纯粹是调试信息，ID 以文件为准。
2. `GB::cEcoRuleSet::BuildIDMaps`：只把已读入的 mID 建排序索引，无哈希计算。
3. 本机全部 **82 个 EcoGame 脚本包（13 个主库补丁版本 + 全部 DLC）的 AEB
   `namesIncluded` 标志位 = 0**（逐包 RefPack 解压验证）。即官方数据从不含名字，
   文件内也不存在 (name, hash) 配对可收获。
4. `hashifier: %x` 字符串：retail exe、dev beta unwrapped exe、两份 PDB
   （retail 73MB + dev beta）**均不存在**。`er2-rules.md` §4.1 旧记录
   "exe 内 hashifier: %x 字符串无静态引用" 的出处无法在本机任何现存文件复现，
   应视为未证实（可能来自 9 月分析时的旧 dump 或误记）。

**结论**：规则名→u32 哈希只存在于 Maxis 的 AEB 编译工具（PC 侧，未泄露），
游戏运行时只消费哈希值。查名路径保持 `er2-rules.md` 的结论：s3db Properties
表 + property 键面。破解该哈希的现实途径只剩离线攻击：用 39 份 AEC 源码中的
已知规则名 × 对应 AEB 的 mID 集合做配对候选，再对新算法做差分——但配对本身
依赖 H&V Rosetta 对齐，见 §2.4。

## 2. AEB→ER2 反编译：布局定案 + 字段名落地

### 2.1 版本三元组的真实含义（修正旧文档）

`GB::cEcoRuleSet::Read`（dev beta，读 (4,2,13)）：

```text
u32 rulesVersion      dev=4, retail=5      ——文件布局主版本
u32 baseRulesVersion  =2                    ——cEcoRuleBase 字段布局
u32 unitRulesVersion  dev=13, retail=14     ——unit rule 记录布局
u8  namesIncluded     官方样本恒 0           ——名字跟写开关（对应 AEB 编译选项）
```

旧文档 §4 把三元组称为"布局主/次版本+规则记录版本"，前两个的定性
（主/次）不准确：2 是 base rule 子记录的版本号，与 unitRulesVersion 一样是
**按记录类型独立演进的版本**（mapRulesVersion 独立为 3..4，binSetRulesVersion=7，
zoneRulesVersion=1——同一文件里五个版本号并行）。

### 2.2 retail (5,2,14) unit rule 记录 = 144 + 12×T 字节（精确消耗证明）

对 `SimCity-Scripts_287520926.package` 主库 AEB（解压后 5,947,232 B，
与 `er2-rules.md` 记录一致）：

- 按 `144 + 12×T` 模型走 28,441 条 unit rule，**终点 4,275,389 与段 A 锚点
  `(tag=5, nA=56)` 严格重合**；T 合计 14,989（平均 0.527 条 transport/规则）。
- 记录尺寸直方图：144B×23,332 / 156B×237 / 168B×2,466 / 180B×982 / …（长尾）。

字段对位（dev-13 名字 ↔ retail-14 字节位，语义以 dev 为准、位置以 retail 为准）：

| u32 序 | retail-14 内容 | dev-13 字段名（伪码权威） | AEC 对应 |
|---|---|---|---|
| 0 | 规则 ID（哈希） | `mID` | `unitRule <Name>` |
| 1 | 位标志 | `mFlags.mWord[0]` | `flags -if/-ifNot …` |
| 2 | 数据池偏移 | `mSimInfo.mOffset` | （条件评估输入集） |
| 3..12 | 5×(off,cnt)，off=-1 表空 | `mLocal/mNearby/mMap/mGlobal/mFromBinOps` | `local/connected/map/global/agent in|out` 记号流 |
| 13..16 | -1,0,1,0 等 | `mControlFlags.mOffset, mFilterGroup, mFilterGroupMask, mRegionSelect` | |
| 17..26 | | `mMinApply, mMaxApply, mApplyResource, mSuccessCount, mSuccessRuleID, mSuccessRuleDelay, mSuccessEvents{off,cnt}, mFailCount, mFailRuleID` | `applyCount/maxApply … onSuccess/onFail/successEvent` |
| 27..29 | | `mFailRuleDelay, mFailEvents{off,cnt}` | `onFail` 尾段 |
| 30..32 | | `mUnitID, (mUnitCreateOffset 疑), mGraphicsState`（dev 此区共 7 个 u32，retail 只有 3——**layout 14 相对 13 删了 4 个 u32**，具体删哪些待 retail 读取端确认） | `create`/`options -createAgent` |
| 33 | T | transport 数组长度 | `options -sendTo/-resendTo …` |
| 34.. | T×(u32 + vec2) | transportOps | 路由多路发送 |
| 尾 2 | | `mTransportID, mHandlerID` | |

**条件语义修正**：dev-13 的 (off,cnt) 对是"off≠0 才读 cnt"（条件存储）；
retail-14 全部成对存储、用 `0xFFFFFFFF` 哨兵表空（record0 五对全为
`(-1, 0)` 实证）。`er2-rules.md` §4 旧 RuleRecord 表中"伴随哈希（模块/作用域
归属）"实为 `mFlags`（位标志），"f2" 实为 `mSimInfo.mOffset`（数据池偏移）。

### 2.3 段结构对拍：旧文档分段模型正确，且现在有名字

前向锚点验证全部通过：段 A `(5,56)`@4,275,389 → 段 B `(8,573)`@4,279,429 →
段 C `(2,95)`@4,339,029 → 常量表 `pairCount=1716`@4,354,252 → D 段 count=1716 →
E 段 → 池长 1,517,060 → 文件尾。与运行时 `cEcoRuleSet::Read` 的读取序对齐后：

| 旧文档段名 | 运行时对应（dev 伪码命名） | 说明 |
|---|---|---|
| 段 A (tag 5, 56) | **mapRules**（`mapRulesVersion=5`, `mMapRules`） | tag 5 就是 mapRulesVersion；每条经 `cEcoMapRule::Read`：扩散（mDiffuseThreshold/MaxAmount/ModifierMapID/Viscosity）、漂移（mShiftSpeed/Direction/SpeedResource/DirectionResource）、mRandomCount、mUnitCreateID+偏移、mChain* |
| 段 B (tag 8, 573) | **binSetRules / mGlobalRules**（binSetRulesVersion=8） | `cEcoBinSetRule::Read`：mResources/mFromBinOps 对、min/max/applyResource、**cControlFlagInfo（64 位 test/set 掩码×2，对应 `globalFlags -set`）**、fail/success 链 |
| 段 C (tag 2, 95) | **zoneRules**（zoneRulesVersion=2） | `cEcoZoneRule::Read`：mSampleType、mUnitZoneID、mSkillID/Min/Max、map 三元组（id,op,weight）、global 三元组、filterOps(slot,stateIndex) |
| "tag"数字 | 各子记录自己的 **version 号** | 不是类型标签；dev 代码里是 `mapRulesVersion-3>1` 这类范围检查 |

之后：`mBaseUnitRuleCount`、`mIDToUnitRulePropSetMap`（排序 (id,idx) 对）、
`mUnitRulePropSets`、`mGlobalRulePropSets`（每条 = n×16B PropOp：
`mPropID + vec2 mPropExpr + mData32 + mData16 + mData8 + mOpID`，
即 AEC `set Name Value` 与规则内 prop 运算的编译产物）、`mData` 字节池。

### 2.4 常量表首列语义 = 未决（旧"hash"定性存疑）

常量表 1716 对 `(u32, u32)` **按首列严格升序**（0xB3A8C < 0x322976 < …）。
首列若是名字哈希不应有序；若是池偏移，指向处并非字符串起始。池本身是
**字符串与二进制记录混排**（头部直接是 `SC_RULE_PLAYERBANKBALANCE\0…`，
中段出现 `999999(0xF423F)` + 哈希 + float 的记录）。首列真身待
retail 读取端（IDA）确认。这对反编译器很重要：若首列可定位字符串，
1716 个常量的名字可直读。

### 2.5 反编译器路线图（更新 er2-rules.md §7.2）

已具备：unit/map/binset/zone 四类记录的**权威字段名 + retail 精确字节布局** +
PropOp/FlagInfo/IDList 等原语。缺：① layout-14 在 30..32 区删减的 4 个 u32 的
身份（需 `SimCity_dump_SCY.exe.i64` 中读取端反汇编）；② 常量表首列语义；
③ 池内记录结构。拿到 ① 后即可在 `crates/dbpf/src/erz.rs` 重写精确解析
（现 erz.rs 仅做段计数），并按 AEC 语法模板输出规则文本（Rosetta 对齐用 H&V
的 39 份 AEC）。

## 3. trace 哈希身份绑定：从"待办"到可实施方案

`logging-and-simulation-findings-summary-20261017.md` §五/§六的缺口：
"status.json 的记录数一致不证明文件来源相同；现有用户选择关联未做 trace
哈希身份绑定"。本轮找到的绑定锚点：

1. **规则集身份（最关键）**：rule ID / stateIndex 只在特定 AEB 版本内有意义
   （26,572 规则@257365692 vs 28,441@287520926）。任何含规则索引的 trace
   必须绑定 `(0x08068AEB:40800200:622B9CD7, patch=287520926)`。
2. **引擎身份三元组**：`GB::cEcoKey = {tHistoryDataType mHdt, u32 mId, u32 mUnit}`
   ——EcoGame 状态图（facts）的 key 结构，是比 unitSlot/指针稳定的身份单位；
   unitSlot 是数组下标，不能跨生命周期引用（与旧报告结论一致，现在有结构名）。
3. **会话锚**：`cDeltaManager::cGameJob.mUId`（delta 作业唯一 ID，`EndDeltaStream`
   实证）+ `cEcoGame::mSimTotalTicks/mLoggedSimTicks`（tick 基准）+ 城市 UID/
   environment（ctx 已有结论）。
4. **内容身份**：每份 trace/status/delta 副本记 SHA-256；游戏 EXE 记 SHA-256
   （retail 2b0aff52…）。

落地：诊断工作台导出会话时写 `identity{artifacts:{path:sha256}, exe_sha,
aeb_tgi, aeb_patch, city_uid, job_uid, tick_base}`，导入时校验哈希并把
rule 索引解释限定在该 AEB 版本。这一步不需要新采集，纯元数据规范。

## 4. delta 流解析：动作层完全定案

### 4.1 `GB::DecodeActionHeader`（0x6376D0）——4 字节内嵌头

```text
u32 LE header:
  bit 0..15   data1 (u16)
  bit 16..23  data2 (u8)
  bit 24..29  actionID（6 bit）
  bit 30..31  哨兵 = 0b11（否则拒绝）
逃逸：data1==0xFFFE → 追读 i32 BE；0xFFFF → -1
      data2 >= 62    → 63 表 -1，否则追读 i32 BE
合法 actionID < 0x2F (47)
```

旧文档"version 27 前后 0x3E/0x3F vs 0xFE/0xFF"的表述可精化为上表
（0xFFFE/0xFFFF 与 62/63 是同一机制的两种视角）。

### 4.2 `GB::tLogActionID` 全 47 个枚举值（PDB TPI 流提取，含 kLogActionUnitMove=10 补漏）

```text
0  SimTicks            1  UnitRule          2  Unused2            3  UnitRuleAttach
4  MapRule             5  GlobalRule        6  UnitCreate         7  UnitFail
8  UnitUpgrade         9  DestroyUnits     10  UnitMove          11  ZoneCreate
12 ZoneDestroy        13  ZoneUpdate       14  ZoneMerge         15  PathAddPoints
16 PathSetTangents    17  PathUpdatePoints 18  PathUpdateTangents 19  PathCreate
20 PathDestroy        21  PathUpgrade      22  GlobalRuleAttach  23  EventRuleAttach
24 DebugSetBin        25  ApplyServerDelta 26  AcceptServerMessage 27 AcceptServerPackage
28 DeliverTransaction 29  PathMerge        30  PathSplitSegment  31  MapRuleAttach
32 DestroyTransaction 33  RevertTransaction 34  SinkRuleAttach    35  PathGroupBegin
36 PathGroupEnd       37  CompactServerPackages               38  ClearTransactionBinding
39 SetTransactionBinding                  40  ZoneRemap         41  PathSplit
42 DebugModifyMap     43  ConnectUnits     44  RedirectTransaction
45 PathDowngrade      46  PathReconstruct
```

### 4.3 Log* 写入端语义（dev 伪码，retail 同构）

- 所有 `Log*` 先自动冲刷 `kLogActionSimTicks`（data1=tick 增量，**不是绝对 tick**；
  旧文档"delta ID 不是 tick"的警告在此层同样成立）。
- `LogGlobalRule(stateIndex)`：头 `(5, stateIndex, 0)` + i32 该全局规则 `mApplyCount`。
- `LogMapRule(mapSlot, mapRuleIndex)`：头 `(4, mapSlot, mapRuleIndex)` + u32 `mRandomCount`。
- `LogUnitFail(unitSlot, stateIndex)`：头 `(7, unitSlot, 0)` + i32 stateIndex。
- `LogUnitRule(unitSlot, stateIndex, runState)`：头 `(1, unitSlot, stateIndex)` +
  i32×3 (`mApplyCount, mMaxApply, mMaxPossibleMapApply`) + `EncodeIDList`
  （u32 count + count×u32 选中单位）。
- `LogUnitRule` 前置：单位 `mLogMoveDirty` 时先发 `kLogActionUnitMove(unitSlot,0)`
  + `cSPTransform::Write`（u16 flags BE，bit0=scale f32、bit1=rotation Matrix3、
  bit2=translation Vector3，写端恒置 bit3）——**单位位置就在 delta 流里**。

对旧捕获的重新解读（假设级）：65 秒采集里 "26→12→27→16→22→13" 重复组现在是
`AcceptServerMessage→ZoneDestroy→AcceptServerPackage→PathSetTangents→
GlobalRuleAttach→ZoneUpdate`——形似"服务器包接收+状态重挂"的序列化片段，
与"per-stream 状态记录"的旧定性一致但语义可读了；"action 1 带 data2=stateIndex"
即 `kLogActionUnitRule`。dev 枚举 `Unused2`（值 2）在 PDB 里真实存在，
旧文档"不得把 dev 枚举 Unused2 套 retail"的警告应收敛为：**编号以本表为准**。

### 4.4 仍未决

- 外层 `SLDelta*.mfs` 信封（`ff 02 03 00` + BE record_size/delta_id/sim_ticks/kind）
  的 `kind` 与 cChunkList/cChunk 的关系；SLDelta11 系列非标准头。
- `cStreamXOR`（key 128B，0xd1add0）在何种保存路径启用。
- retail exe 上对应 dev 函数的 RVA 映射（retail 无符号，需 IDA 特征匹配）。

## 5. 对既有文档的修正清单

| 文档 | 修正 |
|---|---|
| `er2-rules.md` §4 | 版本三元组含义、RuleRecord"伴随哈希/f2"实为 mFlags/mSimInfo.mOffset、(off,cnt) 哨兵语义、段 A/B/C=map/binset/zone 规则、"hashifier: %x"字符串不可复现应撤回、unit record=144+12T |
| `er2-rules.md` §4.1 | 常量表首列"hash"定性降级为未决（严格升序，非名字哈希特征） |
| `retail-delta-stream-analysis.md` 顶部纠正 | action 编号与名字以本文 §4.2 为准；"dev Unused2 不得套用"收敛；data1/data2 逃逸值精化 |
| `logging-and-simulation-findings-summary-20261017.md` §六.2 | "建立哈希绑定"的锚点方案见本文 §3 |

## 6. 后续工作

1. ~~在 `SimCity_dump_SCY.exe.i64` 反汇编 (5,2,14) 读取端~~ —— 2026-10-30
   已用纯数据路线绕过：Rosetta 顺序对齐 + 字段投票定出 14 布局关键字段
   （19=onSuccess、23=onFail/chain、15/16=min/maxApply、33=T；13 布局
   21/26/17/18）。30..32 区剩余字段与常量表首列仍待 IDA，但不再阻塞。
2. ~~离线攻击规则名哈希~~ —— **已破解：FNV-1(lowercase)**。未命中样本
   （8 条 `*_Destroy_CheckUnlock/Relock`）为补丁间改名，可再对齐收割。
3. ✅ `crates/dbpf/src/erz.rs` 已升级（双布局记录级解析 + `fnv1_name_hash`
   + 规则样本；主库 exact=true）。**待做**：binset-8/zone-2 变长记录的
   字段级解析（当前计数+定长跳过）、按 AEC 语法模板输出规则文本
   （反编译器本体）、池偏移追踪（local/global ops 的资源名哈希也在池里，
   可用同一 FNV-1 查名——资源键面即 property 名）。
4. 诊断工作台身份绑定 schema（§3 方案，纯元数据）——未动工。
5. ✅ delta 流重放已产出带名字时间线：60 秒捕获 31 流/3,689 写行 →
   337 个具名动作头（`tmp/rosetta/delta_replay_named.json`）。直方图
   ClearTransactionBinding×85、PathReconstruct×33、PathSetTangents×30、
   CompactServerPackages×28、AcceptServerPackage×20、UnitRuleAttach×18、
   AcceptServerMessage/ZoneDestroy/ZoneUpdate/ConnectUnits×15——与
   "保存/重载状态序列化"定性吻合，且旧观测 26→12→27→16→22→13 序列
   现在读作 AcceptServerMessage→ZoneDestroy→AcceptServerPackage→
   PathSetTangents→GlobalRuleAttach→ZoneUpdate。UnitRule 仅 ×5：
   该窗口几乎没有玩法点火，动作层解析有待一次"放置一条道路"窗口验证。
