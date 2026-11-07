# 菜单/渲染失败矩阵：症状 → 根因层 → 构建期拦截

日期：2026-11-07。起因：s24→s25 多次出现"菜单项在、放置只有锚点/丢 lot 模型"，
与 r10 症状相同但根因层不同。本文把已证实与仍在调查的因果分开记录，并对应到
构建闸门，避免每次靠实机试错。

## 1. 验收主题（不要搞错）

**行为闭环与菜单/继承解耦**：公园入口可以上学、地标可以住人、警察局可以产水。
s24 的配方就是 **公园菜单 + 教育最小闭环**（`07049250` 入口 + `95A9D6FD` Unit
行为类别 + 教育 bundle 集）。因此：

- **不要把"公园入口 + 教育行为"当成错误去改成教育菜单**——那正是要验证的
  解耦配方；
- 菜单类别决定"在哪个面板出现/用哪个放置工具"，行为由 Unit 的 `0AFE27B1` +
  bundle/bin/sink/规则链决定；两者正交（s17/R1、s24 实证）。

## 2. 症状矩阵

| 症状 | 已证实根因 | 仍待证/其它可能 | 构建期检查 |
|---|---|---|---|
| 菜单项出现，放置只有锚点 | ①新菜单身份合成了 donor 没有的 `0CA5CF4A` 自映射（`clone_donor_identity` 旧行为）；②工作台文档整体重写丢列（r10） | ①bin1 只写 3/5 列或原型 id 重复；②路径列被改写后的结构问题；③GraphicsCache 陈旧 | menu-contract（自映射）、delta-audit（列漂移）、缓存隔离 |
| 菜单/面板标题 `***`/乱码 | Menu2 `0A09F5FA/FB` 的 Text 指向的 locale 表/行不存在或为空；locale 表 JSON/BOM 坏 | locale 表 id 与其它包冲突（同表被覆盖） | menu-contract locale 行解析 |
| 菜单不加载/条目消失 | ①包本身坏（Property 编码/解析失败）；②Menu2 缺 `0975695F`（menuUnit）；③uiCategories 为空；④PathEntry 索引/资源缺失；⑤同 TGI 被更高优先级包遮蔽 | 菜单容器位次/容量冲突 | menu-contract（menuUnit/类别）、validate-paths、资源闭包 |
| lot/模型丢失 | ①整 lot 重写（r10）：未声明列被剥掉；②Unit→Asset 断链/Asset 缺 BoundingBox/模型/LotMask；③合成自映射 | ①bin 列结构；②路径列；③GraphicsCache 陈旧 | delta-audit、menu-contract 渲染字段 |
| 菜单同时出现旧/新条目 | full 包包含参考包的 Menu2/locale（不是残留文件）；两个身份各一条 | — | 构建清单/资源对拍 |
| 旧包卸载后原地仍有建筑 | **已放置单位在存档里**，不在包里；移除包不会删除已建单位 | — | 流程：DISPOSABLE 城市/拆除 |
| 代码改了但产物没变 | `processor_version` 未 bump，构建缓存按版本命中 | — | M7：任何处理器/契约代码变更必须 bump |

## 3. 已证实的案例（按层）

### 3.1 r10：整 lot 重写丢列（数据层）

工作台文档只建模了 lot 的一部分列；构建把文档当权威整体重写，锚点 22→9、
灯光附属列、percent-fill 开关、per-prop 资源全部丢失 → 模型+地面消失。
修复：`scene.writes` 列族 delta + `delta-audit`（最终 lot vs 参照 lot 逐列对拍）。
本次 s25 重建已确认 Asset 只改 paths + props:1，其余列与 s24 字节一致。

### 3.2 新身份合成 `0CA5CF4A`（身份层）

`clone_donor_identity` 无条件给新 Asset 写自映射；s24/Dalian 都没有该字段。
menu.rs 自己的注释写明"合成 self-map 会改变运行时继承图，可能只显示放置锚点"。
修复：不合成，donor 有则只做实例重定向。

### 3.3 缓存（环境层）

- **构建缓存**：改代码未 bump `processor_version` → 构建 hash/产物完全复用旧包
  （s25 第一次"修完没变化"即此）。现为 `/26`。
- **GraphicsCache.package**：跨会话持久缓存；包数据正确但渲染坏时，先隔离
  `SimCityUserData/GraphicsCache.package`（改名 `.bak`）再测。s25 本轮已隔离。

### 3.4 菜单/行为解耦（语义层，正面案例）

s24 = 公园入口 + 教育最小闭环，实机可放置并可入学。**这是要验证的配方**，
不能因为"在公园面板"就改菜单类别；否则验证目标被偷换。

## 4. 构建期闸门（已实现）

### 4.1 menu-contract（新增阶段，/26）

在 `behavior-bind` 之后、`delta-audit` 之前，fail-closed：

- Menu2：`uiCategories` 非空、`menuUnit` 指向本身份；
- 身份链：Unit→Asset 指向本身份且 Asset 在池；
- 渲染字段：`00F9EFBA` BoundingBox、模型引用（`00F9EFBB..BE`/`0D0082F1` 至少
  一个可解析到 RW4）、LotMask 可解析；
- 标题：Text → locale 表/行存在且非空（包内表才校验，基础游戏全局表跳过）；
- 自映射：donor 无 `0CA5CF4A` 时新身份不得有；
- 公园入口 + 非公园 Unit：**警告**（解耦配方，不拦截）；
- 报告进入阶段台账（Actions 页可见）。

严格模式（新菜单身份）对 Unit→Asset/模型引用 fail-closed；frozen/legacy 身份
只给警告（历史包可能沿用 Dalian 旧约定）。

### 4.2 delta-audit（/23 起）

最终 Lot vs 参照 Lot 逐列；未声明的 changed/added/removed = 构建失败。
菜单身份另对 identity donor lot 对拍（自映射/标题链接列白名单）。

### 4.3 流程闸门

1. 参照必先验 SHA-256（配方 schema/场景/参考包/基线包）；
2. 部署前归档旧测试包 + 备份存档（`Games-backup-<ts>`）；
3. 冷启动菜单 oracle（卡菜单 = 坏包，不进城）；
4. 只在新/另存为的 DISPOSABLE 城市测试；旧城已放置单位不会随包卸载消失；
5. 包数据正确但渲染坏 → 先隔离 GraphicsCache.package；
6. 任何处理器/契约代码改动 → bump `processor_version`。

## 5. 复盘清单（出现症状按顺序查）

1. **先确认参照身份**：构建用的 schema/场景/参考包哈希 vs 基线台账；
2. **看构建台账**：menu-contract 的 checks/warnings、delta-audit 的 declared/
   drift；有没有 `[menu-contract]`/`[delta-audit]` 失败；
3. **看包内容**：`dump_package` 对拍基线；model/ground TGI 是否字节一致；
4. **看身份链**：Menu2 menuUnit → Unit → Asset → BBox/model/LotMask；
5. **看环境**：GraphicsCache 是否陈旧；是否在新城市；构建是否命中旧缓存；
6. **最后才做字节对拍**（且先核参照哈希）。

## 6. 未决（s25 锚点仍在调查）

s25（同身份重建：4 点路径 + bin1 两桶 + W1=20）在隔离缓存/新城市前的实机仍
只显示锚点。已排除：
- 整 lot 丢列（Asset diff 仅 paths/bin1；其余 17 资源字节一致）；
- 合成自映射（已删除）；
- 菜单/身份链（menu-contract 全绿；Menu2/Unit/Asset 均在包内）；
- 菜单类别（公园+教育是 s24 验证配方）。

剩余重点怀疑：
1. **bin1 编码**：s24 的 bin 原型列是"少量原型 id + 多实例 transform/slot"
   （bin0：3 ids / 9 transforms / 9 slots；bin12：2 / 10 / 10），而 s25 bin1
   写的是 2 个**重复原型 id** + 2 transforms + 2 slots，且缺 randomize/
   percent-fill 列（`0C12EF51/61`）。若引擎要求原型 id 去重或五列齐备，
   整个 Asset 可能解析失败 → 锚点。
2. **路径列**：4 点服务道（roadConnect=First）+ 行为追加的行人道；需与
   s24 的 9 点双路径结构逐列比对。
3. **GraphicsCache/旧城污染**：已隔离缓存，待新城市复测。

下一步隔离实验（一次只改一个变量）：
- A：只 W1=20（无 paths/bin1）→ 能否渲染；
- B：W1 + paths（无 bin1）；
- C：W1 + bin1（无 paths）；
- D：全量。
A 能渲染而 D 不能时，B/C 二分定位。定位后把 bin1 编码修正（原型去重/补齐列）
或暂缓垃圾桶，先保证建筑渲染。

## 5. s25/s26 lot 不加载定案（2026-11-07 追加，vanilla 地面真值扫描）

症状：s24 GOLDEN（5df84860）实机完全正常；s25（4c7c3f89）与 s26（8a7fb8ab）
lot/模型均不加载（工具缩略图棋盘格、放置只剩锚点）。三者字节对拍后差异收敛到
lot 的 bin1 道具写入。对本体 SimCity_Game.package 全量 lot 扫描
（examples/scan_lot_patterns.rs）取得地面真值：

- `0CAA6841` = per-path **闭区间对 [start,end]**，指向 `0CAA6832`（per-path
  点序列拼接表）。4 个 vanilla 多路径 lot 全部自洽闭合（如 AFEFBF8F
  3+5+7+4=19）。s24 `[0,8][9,10]` 与 s26 `[0,3][4,7]` 均结构合法 → **path 列洗清**。
- bin0 "keys 少于 transforms"（如 3 keys/7 transforms）是 vanilla 正常态
  （9C4C3711）→ **bin0 调色板模式洗清**。
- **全部 vanilla lot 的 bin1 列严格 1/1/1（单资源单实例），无任何"同一 key
  写两遍"先例**。s25/s26 的 bin1 = [54CA89F0,54CA89F0]（双实例同 key）是
  全包唯一无 vanilla 先例支撑的结构 → 头号嫌疑。
- `418A18B1`（Pedestrians）在所有游戏包中**不存在为 DBPF 资源**——它是引擎
  内部语义 id；scene 显式声明该 entry 时 validate-paths 会拒绝（行为层
  ensurePedestrianPath 的镜像道在 validate 之后追加所以能过）。不要伪造
  PathEntry 资源。

**s27（5734e10d）= 单变量隔离测试**：s24 + 单个 vanilla 形状垃圾桶
（bin1 严格 1/1/1）+ W1 20 + 词条改名；path 列零差异。s27 仍不加载 →
"双实例同 key"嫌疑被推翻，真根因见 §6（Transform 列声明步长 0）。
本节对 bin1 的 vanilla 观察仍有效：原版 bin1 = 单实例 1/1/1（multi 实例
同 key 无先例），双桶后续用稀疏 bin2 层（364C2CEE 证明 20/21/22 三层并存
是 vanilla 正常态）。

## 6. 根因修正（2026-11-07 深夜，raw-byte 取证定案）

§5 的"bin1 双实例同 key"嫌疑**被 s27 推翻**（s27 = 单 bin 严格 1/1/1、path 零
差异，仍然不加载）。raw-byte 取证找到真根因：

- **`scene.rs::array_property` 对新建 Transform 列声明 `item_size=0`**
  （注释自称"Transform 变长所以不声明"）。而游戏加载器**信任声明的步长**
  （native_profile::array 的既有注释就写着"the game loader validates the
  declared step"——知识一直在，scene.rs 漏抄了）。
- vanilla 实测（raw_probe 对 s24 灯光列 0CAA8F12）：**声明 stride=56，每项
  实际 50 字节（flags-14），尾部松弛**——"声明 56 + 每项 50"就是 vanilla
  约定；本编码器每项输出恰为 50/54 字节，值语义无错，错的只有声明。
- s25/s26/s27 三连死 = 三个构建都带场景 prop 写入（新建 Transform 列）；
  r9 及之前从未新建过 Transform 列，所以从未触发。
- 修复：`array_property` 补 `Transform => Some(56)`；**processor_version /26
  → /27**（M7）。产物 s28（f88f1371）四门禁（TGI/列 diff/布局/raw 步长）全绿。
- 教训：**离线门禁必须包含"新列布局对拍 vanilla 同类型列"**（type/flags/
  count/item_size），解析值对不等于字节层对。

### 4.4 layout-contract（新增阶段，/28）

s25/s26/s27 三连死的构建期拦截（§6 根因）。behavior-bind 之后、staging 之前，
fail-closed 扫描输出池**所有** 00B1B104 属性文件（lot/unit/Menu2/PathEntry/
bundle 一网打尽）：

- 每个数组列声明步长 ∈ `legal_declared_stride` 合法表——表来自本体全量标定
  （examples/scan_strides.rs：2385 文件、31653 个 Transform 数组，**每类型
  恰好一个合法步长、零例外**：Bool 1 / Int32·UInt32·Float 4 / Key·Vector3·
  ColorRgb 12 / Vector2 8 / Vector4·ColorRgba 16 / String8 16 / Text 520 /
  **Transform 56** / BoundingBox 24）；
- 空数组（count=0，vanilla 从不存在）同拒；
- 实证：坏 s27 被精确击落（`stride 0 for hash 0c12ef31`），s24 GOLDEN 与
  s28 全过；unit test `declared_stride_gate_*` 钉住表与三种毒性编码。

教训固化：解析值对拍（delta-audit/列 diff）看不见声明元数据——**凡新增
"写属性文件"的代码路径，门禁加在编码产物的字节层，而不是作者层**。
processor_version /27→/28。
