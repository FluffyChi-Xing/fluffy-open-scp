# 自定义地标资产的最小回写元数据清单与资产→菜单链路缺口（2026-10-27）

> 样本：用户自制 219 m 高塔模型（已导入 openscp asset 工作台），目标 = 做成新的
> **地标建筑**；对标样本 = 威利斯大厦（菜单 `74168A83` / Unit `8D007745`，
> 见 `willis-tower-landmark-mechanism-20261027.md`）。
> 用户约束：只绑定车辆/树/垃圾桶 prop bin + light + spawner + path，
> **不含 decal/effect**。本文回答两件事：①前端只做 schema JSON 搭建时，引擎
> 转译层必须回写哪些元数据才能让它成为"活的地标"；②资产→菜单链路还缺什么
> 才能跑通闭环（地图编辑另一闭环，不在此文）。

## 0. 结论速览

1. **最小绑定集足以构成合法地标**——威利斯 Lot 本身就是"纯模型 + 锚点、
   零 effect 键"的形态；决定"是不是地标"的不是视觉绑定，而是
   **一条 Unit 模板继承边 + Unit/Lot/Menu2 三张记录 + 三个资源件**。
2. **前提修正**：菜单层 `parent` 只挂 plop 工具根 `AF042E9A`（全维度共享、
   不承载行为）；真正的父类继承发生在 **Unit 模板层**
   （`0x00B2CCCB → 48E0C000:4E01E311` TemplateToyBase_landmark）。
   工作台要补的是"Unit 选模板 + Lot 挂同 instance 的 Lot 模板"，
   不是"菜单条目继承父类"。
3. 回写原则 = **先模板后叶子**：模板给"一类基本行为"（解锁/限额/费用科目/
   类别绑定），叶子只写特化覆盖（造价/维护/吸引力/数值族/bundle 增删）。

## 1. 回写元数据清单

### A. Unit 层（`40E0C000:<新instance>`）

| 写什么 | 哈希 | 说明 |
|---|---|---|
| **模板继承边** | `0x00B2CCCB → 4E01E311` | 一行继承：解锁 bin（`kResourceIDLandmarksUnlocked`）、全城 3 座限额（`0x0E51C90F`/`0x0CC8BE51`）、吸引力默认 500、观光收入系数、Tourism 费用科目（`0x0AD65F07/08`）、吸引劫匪——全部不写 |
| 造价/维护 | `0x0A8A41DA` / `0x09AE19D7` | 叶子覆盖 |
| 地标吸引力 | `0x0E542F39` | 模板 500 → 按定位覆盖（威利斯 400、悉尼 800） |
| 地价加成 | `0x0E585AE0`+`E1` / `0x0E585B00`+`B01` | int32 伴财富档 |
| 水/电/垃圾族 | `0x0AFB9882`/`0x0B88904E`/`0x0C09DAA0` 等整族 | **整族一致**：Capacity=3×Amount，各带一个同 Capacity 伴生哈希 + 一个恒 0 伴生哈希；不写则继承 Toy 层默认 |
| 岗位族 | `0x0A7917C3..C8` | 按设定写 |
| **bundle 挂载** | `0x0A0209A5 → 40E0C700` | **没有 bundle = 死建筑**（无规则、无资源 bin）。M1 最小路径 = 克隆威利斯 15 个 bundle 组再删减，或启用模板 `0x0A0209A4` 的 12 个候选 |

### B. Lot 层（`40E1C000:<同instance>`）——用户点名的四件全在这层

- `0x00B2CCCB → 40E1C000:4E01E311`——**与 Unit 模板同 instance 的 Lot 模板
  （双注册）**。只按 instance 寻址必然写错层（消防局异常的头号嫌疑机制，
  见 `menu-asset-editing-insights-20261027.md` §4.2）。
- 声明 bbox `0x0F9EFBA`（219 m 塔写真实 z；它同时是地面 quad UV 原点回退值
  与 `GB::UnitIsFullyInsideWorld` 摆放校验输入，见
  `dev-dump-lot-placement-and-script-resources.md` 与 2026-10-27 边界对拍）
  + Frontage/Depth `0x0BBBCDED/0x0BBBCE2A`。
- 模型引用 → `2F4E681B` 多 LOD RW4 key 数组。
- **prop bins**：`0x0C12EF20` 族列，值 = 脚本资源表 resourceID（离线解析配方：
  EcoGame 包 G `40E0C100`；车辆两分支 `0x0D8C29C3` 显式 key / `0x00F9EFBB`
  self-key，见 dev-dump §3.4）。
- **light**：`0x049FC837..83A` 附件灯族（强度/颜色/尺寸/偏移数组）+ 窗口光/
  vignette 参数键（威利斯 5 键含 `kVignetteID`）。
- **spawner/命名位置**：`0x0E1BAC61/62` Key+Transform 成对表（门口/车位/塔尖
  锚点；agent 生成点只是用途之一）。
- **path**：`0x0CAA6841/680D/0ED8/6832` 之外，**`0x0CC8FE7A`（path entry）、
  `0x0D9A4E54`（scope）、`0x0D9AA2BB`（road connect）三列必须齐**——缺
  entry 列整条路径被引擎跳过（dev-dump §3.7）。
- 不含 decal/effect 是合法形态，不要以"缺 effect"为由改 Lot 结构。

### C. Menu2 层（`09878A01`，Graphics 包）

`0x0DB9FC63 uiCategories=[3F43E038]`（唯一归属边 → 文化/地标子栏）+
`0x00B2CCCB parent=AF042E9A` + `0x0975695F menuUnit` + `0x0DC1E3E0` 排序 +
**解锁三件套 `0x0DE84DC9/DD3/DDC` 成对写**（地标 = 市政厅 + 2 级 +
旅游模块 `C93FD4B5/895DD8CE/BF800BE4`）+ 统计行 `0x0EB1FC05/FC22`
（`unitEffectParameter` 直引 Unit 属性哈希，写 `[0x0E542F39, 0x0E585AE0]`）。

### D. 资源件

图标 PNG（`2F7D0004:40E02400`）、rollover JPG（`3F8662EA`）、locale 表
`0x0A98EAF0`（菜单名 + 解锁文案，zh-CN/en-US 双语，符合仓库 i18n 约束）。
显示价按"menuUnit + 子单元一层求和"回显，不显示写入裸值。

## 2. 链路缺口（资产 → 菜单跑通闭环）

唯一真缺口仍是 **Property 表写回 API**（`mod_flow.rs:923` 克隆+overlay 能
产出真实 DBPF，但 `PropertyFile` 序列化不存在，见
`mod-workbench-asset-semantics-pipeline-20261027.md` §2）。其余按依赖序：

**引擎/转译层**

1. Property 表序列化 diff 回写（Unit/Lot/Menu2/bundle 四表）；
2. TGI + 领域 group 约束寻址，废弃 instance-only/“版本数字最大”候选
  （同 instance 跨 group 跨包是常态：`8D007745` 在 Graphics=Menu2、
  Game=rollover JPG；消防局在 Scripts=Unit、Game=Lot）；
3. 双注册消歧 + 共享模板写保护（`40E0C000/48E0C000/40E1C000` 三组同
  instance 语义不同；经济字段只在 Unit 域解析、`40E1C000` 域禁写经济字段；
  命中共享模板默认阻断并列受影响建筑）；
4. 继承展平器复用——读侧已有（`package_service.rs:2983` 引擎口径 Parent 链
  展平），写侧"写叶子保 parent" + 编辑 Sheet 标注生效来源（模板默认 vs
  叶子覆盖，典型：`0x0EE2A50D` 模板 15→叶 11、`0x0E542F39` 500→400）；
5. 数值族联动校验（Rate/Amount/Capacity+伴生哈希整族、解锁三件套成组、
  容量数组按下标成组）。

**菜单工作台**

6. **Unit 模板继承边支持（本文核心缺口）**：`menu-export.ts` 目前只写
   Menu2（`PLOP_MENU_PARENT` 硬编码 `AF042E9A`），没有"选服务模板"步骤；
   需把模板选择做成一等步骤，且克隆包必须带上模板对
  （`48E0C000:4E01E311` + `40E1C000:4E01E311`）或显式引用原包；
7. menuUnit 双向校验：菜单→Unit 存在性、统计行参数哈希 ↔ Unit 属性一致性
  （`tmp/willis-survey/menu-survey.json` 24654 条可当兼容性检查表）；
8. 图标/rollover/locale 三件套生产（`menu-export.ts` 已有骨架常量）+ 两类
  "乱码"分诊（占位符未绑定=数据链断 vs 文本真损坏）。

**资产工作台**

9. Contract v2（`reference` 克隆参照 + `semantics` 九族 + `provenance`）——
   `assetBinding.ts` 在冻结区 `property-editor/` 内，按 FROZEN.md 约束
   **必须在平行新目录做 v2**，不能原地改；
10. 模型编译或换皮捷径：GLB→RW4 未接入前 M1 用"图形 TGI 指向现存模型"
    换皮；bbox/坐标（z-up、米）先行入 schema；
11. 脚本资源表解析（prop bin 的 resourceID→模型链）是 prop bins 落地的
    硬前置（dev-dump §3.1–3.4 配方已闭环，尚无消费代码）。

## 3. M1 验收（威利斯做参照）

选参照 → 工作台列出九族语义节点（带 TGI）→ 按 §1 填最小集 → build 产出
.package → 游戏内六条：

1. 菜单出现在文化/地标子栏（`uiCategories=[3F43E038]`）；
2. 市政厅旅游模块就绪前不解锁（三件套生效）；
3. 可建造、显示价 = 求和口径；
4. 全城限 3 座（模板限额继承生效）；
5. 吸引力/地价进统计行（`unitEffectParameter` 绑定生效）；
6. 夜灯亮 + 车辆/树/垃圾桶 prop 落地（light/prop bin/位置表生效）。

全程 provenance 面板区分 cloned/modified；不触碰冻结区装配/坐标/取景
（frameInvariants 全绿）。

## 4. 证据

- `docs/re/willis-tower-landmark-mechanism-20261027.md`（结构模型、五层
  模板、23 成员普查）；`docs/re/menu-asset-editing-insights-20261027.md`
  （守卫清单 §5、双注册/乱码机理）；`docs/re/mod-workbench-asset-semantics-
  pipeline-20261027.md`（现状盘点、M1-M3、Contract v2 草案）；
  `docs/re/dev-dump-lot-placement-and-script-resources.md`（prop/path/location
  列族配方）；`docs/re/building-count-limits-20261020.md`（限额读取端）。
- 代码现状：`src/pages/packages/components/property-editor/assetBinding.ts`
  （Contract v1 = source+六槽+menu 仅校验）、`src/lib/game-ui/menu-export.ts`
  （Menu2/图标/locale 骨架 + `PLOP_MENU_PARENT` 硬编码）、
  `src-tauri/src/mod_flow.rs:923`（克隆+overlay build）、
  `src-tauri/src/package_service.rs:2983`（Parent 链读侧展平）。
