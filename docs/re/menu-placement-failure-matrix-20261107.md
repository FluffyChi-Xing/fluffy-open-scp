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

## 7. Locale 同表遮蔽 + i32 造价 + enabled 死数据（2026-11-08 深夜，HopeTower 观光包）

症状三连：菜单名/描述显示模板默认 "OpenSCP Commercial Tower"；造价 50000 显示
「免费」；维护费 -1,500/小时 却正常。三个互不相同的根因层，同时命中：

1. **locale 表 TGI 冲突（资源层）**：`0A98EAF0:02FABF01:BF82F7CE` 是
   `menu.rs::LOCALE_TABLE` 固定常量（FNV-1("openscp/strings")——只保证与
   *原版* 无冲突，不保证与 *自家其它包* 无冲突）。Commercial_v7 与
   HopeTower 都写这张表 → 游戏解析到 v7 的行（"OpenSCP Commercial Tower"
   / "商业地标：.blend 原始工程 parent-free 路线"）。我们的包里
   「大连希望大厦」一直都在，只是被自家兄弟包遮蔽。修复：19a1b3a 起各路由
   locale 表 instance = 身份实例；menu-contract 第 8 号检查禁写共享表。
   **教训：自有"全局唯一"资源，唯一性必须按身份派生，"与 vanilla 不冲突"
   不等于"与自家包不冲突"。**
2. **造价位型（字节层）**：原版 SCUnit 成本全 Float32（s31/v7/vanilla 实测
   一致）；parent-free 路线 `build_unit_leaf` 写 Int32。Int32:50000 按 f32
   位读 = ~2.1e-41 ≈ 0 → 免费。同包维护费 i32:1500 显示正常纯属该字段解析
   宽容——不可作为"类型无所谓"的证据。修复：/36 两路由统一 Float；
   menu-contract 第 7 号检查 fail-closed。**教训：跨引擎字段先抄 vanilla
   的位型，再抄数值；解析值对拍看不见位型。**
3. **enabled=false 死数据（工作台层）**：工程面板保存 `editor.workbench.menu`
   从不写 `enabled:true`（MenuBindingForm 压根没有开关），而
   `workbench_logic::compile` 对 `enabled!=true` **静默返回 None**——用户在
   面板里填的名称/描述/造价整套不进构建。本次部署包是手写 schema 绕过工作台
   构建的，值是"抄"进去的而非"编译"进去的。修复：面板补「启用菜单绑定」
   开关（i18n zh/en）；编译门对"有内容而未启用"fail-closed 点名报错；
   前端 menuBindingIssues 同步提示。**教训：UI 写入的数据若编译器有静默
   跳过分支，必须在跳过前判断"是否填了内容"。**

连带修正：/35 把 vanilla-shadow-strip 移到 delta-audit 后，layout-contract
（在 strip 之前跑）开始扫到游戏基类模板 9EC474FF 自带的 count=0 数组
（0CABB2FC）→ 每个部署构建必被 vanilla 数据击落。修复：layout-contract 按
strip 同款判定跳过游戏自有条目（它们的字节不归本契约管）。

部署记录：修复包 a42cf63e0c25fedb（/36，137 条目，身份 3CAAEFBC——防撞车
守卫把 3CAAEFBB 让给已部署旧包）；GraphicsCache 隔离 bak-20261108-
hopetower-fix；存档备份 Games-backup-20261108-hopetower-fix。
观察点：菜单名=大连希望大厦、描述=编辑器原文、造价 $50,000、维护
-1,500/小时；白模/白地面另案（材质 raster 已确认在包内，缓存隔离后复测）。

### 7.1 白模/无地面定案（2026-11-08 凌晨，§7 的"另案"收口）

§7 留下的"白模/白地面另案"已定案，两个独立根因，都与打包日志对上：

1. **白模 = 材质编译路线缺 `modelShape: "nh-atlas"`**。大学 schema 带
   nh-atlas（tint atlas 烧入 + UV 重排），大连 schema 漏写 → 落进自动
   nh-partitioned 路线 → 塔 4 个 LOD 的材质纹理行 slot1/2/3 指向 3 张
   **32 字节空 raster**（5FD589D2/1301B5DC/E3C56FA3，tint/palette/shader
   无像素）→ 白模。`insert-textures {"slots":0}` 与 materialGate 的
   `textureSlotInstances [0,0,0,0,0,0]` 就是日志里的明文，但当时把它当
   了指纹字段没当绑定字段读。修复：schema 补 nh-atlas → 22MB tint atlas
   入包，纹理槽绑真实 raster。新增探针 `model_texture_refs <pkg> 0 ALL`
   （扫描全部模型引用 ≤64B 空 raster 的材质）：旧包 4 中毒 → 新包 0。
   **教训：材质绑定验证必须做在编码产物层（RW4 纹理行 → raster 闭包），
   槽位指纹字段不是绑定证据。**
2. **无地面 = 参照 lot 是 40 字节 fixture**。Dalian schema 的
   reference.lot = `40E1C000:8D007745`（golden fixture 极简 lot，2 列），
   大学用的是 118 列真 lot（C264A268）。游戏本体 34CE7E14 地标模板只有
   2 列（纯 parent 存根）——挂它不带来地面。修复：参照 lot 换游戏真 lot
   `40E1C000:263E0569`（132 列，BoundingBox [-44,-60..44,60] 覆盖塔
   footprint，掩码 raster 17B7CCBD + 地面模型一并入参照包，
   `add_reference_lot` 工具合并写入）→ 新 lot 132 列全量克隆，
   menu-contract BoundingBox 过检，delta-audit 132/132 对齐。
   **教训：full 模式 lot 叶子的列 = 参照 lot 的列；参照是几列，产物就是
   几列。"挂模板"不等于"有数据"。**

产物 f13155434e47ab02（身份 3CAAEFBB，144 条目）。Tools：probes 新增
`model_texture_refs`（材质中毒扫描）、examples 新增 `lotm_slots`
（LOTM 材质槽对拍）、`scan_big_lots`（大地面 lot 普查）、
`add_reference_lot`（donor lot + 依赖并入参照包）。

## 8. 防复发体系（/37，2026-11-08 定稿）——同类问题不再靠实机试错

§7/§7.1 的五连根因（locale 同表/i32 造价/enabled 死数据/缺 modelShape/
参照 lot=fixture）逐条对应到构建期门禁，processor_version /37：

| 事故 | 构建期拦截 | 验证方式 |
|---|---|---|
| 白模（空 raster 绑定） | `material-binding-contract` 新阶段（vanilla-shadow-strip 后、materialize 前，fail-closed）：产物池每个 RW4 材质纹理行 slot0..5 → raster 必须非空（>64B，32B=空壳即 fail，报错点名 `scene.modelShape="nh-atlas"`）；缺失 raster=告警（原版自带悬空引用，实证 263E0569 链 EDD00202 全安装不存在而原版渲染正常） | 负向验证：去掉 modelShape 重建 → 3 秒被击落，48 违规点名到槽位级（含 4 塔 LOD 的 5FD589D2/1301B5DC/E3C56FA3） |
| 无地面（参照 lot=派生/fixture 变体） | `load-inputs` 参照 lot 主体契约（fail-closed，仅 40E1C000 lot 适用）：主体三要件（00F9EFBA BoundingBox/0CCB7FC8 LotSize/0CCB7FD5 LotMask）缺一即拒，错误指认"换主体变体"+提示 scan_big_lots；**继承链逐跳进台账**（每跳 instance+列数+是否在包内） | 单测 stage_tests::reference_lot_master_contract_*（主体过/派生拒/非 asset lot 跳过） |
| modelShape 漏写 | `insert-model` 台账新增 `modelShapeDeclared` 字段 + 漏写时 warnings 点名（写明多材质 LOTM 在自动路线会产空绑定） | 负向构建 warnings 非空 |

**元教训（用户指令 2026-11-08："不允许回归的问题不能每次花几小时实机调"）**：
每一个能被游戏渲染结果否决的层，都必须有一个对应的字节层门禁，在打包
3 秒内给出可执行的报错，而不是构建全绿、实机返工。判断标准：门禁的失败
信息必须包含"改什么"（如：声明 nh-atlas / 换主体 lot），而不只是"坏了"。

新工具：`model_texture_refs <pkg> 0 ALL`（材质中毒扫描，验收新包必跑）、
`add_reference_lot <game> <tgi> <ref> <out>`（donor lot+模型+raster 二级
闭包并入参照包）、`scan_big_lots`（大地面主体 lot 普查）、`lotm_slots`
（LOTM 材质槽对拍）。

## 9. lot 身份污染 + 地面形状 + 丢面（/38，2026-11-08 白天）

三项均由"克隆 donor 而不清洗语义"引发，全部改为**以用户编排为准**：

1. **lot 身份文本列**：`09FB78CB` 是 donor 的名字文本边（Text→locale 表），
   在白名单里 → 克隆后实机显示 donor 建筑名（「高级煤矿坑」）。剔除后
   产物 lot 零文本列（`grep -c 'text('` = 0）。**规则：lot 只承载地面与
   数值，任何 Text 列都属于身份层，不得随地面克隆。**
2. **地面形状**：LotSize 与遮罩都来自 donor（88×120 长方形），与编辑器里
   的 80×80 正方形不符。改为：`LotSize/lot_offset/placement` 以工作台文档
   为准；遮罩按该尺寸**合成**（满覆盖纯色 raster），不再克隆 donor 形状。
3. **丢面（关键）**：几何完整（`geometry_audit`：LOD0 = 109862 顶点 /
   50382 面，与 LOTM 源一致），但 `00F9EFBA` 沿用 donor 的 bbox
   （z max = 56m），而塔高 270m —— 超出包围盒的部分被裁掉，实机表现为
   "整段楼层消失/露内部"。改为从编译后的 LOD0 模型顶点并集计算真实
   bbox（本次 z 0→270.31903，与编辑器 270.7m 吻合）。
   顺带修 `property_edit::set_array` 的步长声明表（BoundingBox 24 /
   Vector2 8 / Vector3·ColorRgb 12），否则自写 bbox 会被 layout-contract
   以 item_size 0 击落（门禁正确拦下）。

**元教训**：任何"从 donor 克隆"的字段都必须先问"这是不是用户编排的"；
用户编排的（尺寸/形状/名字/文案）一律以文档为准，donor 只提供结构性
数据（父链、列布局）。processor_version /37 → /38（处理器行为变更必须
bump，本轮曾漏 bump 被发现并补）。
