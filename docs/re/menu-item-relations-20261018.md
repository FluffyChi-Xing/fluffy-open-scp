# 菜单项关联数据全景：容器、归属、元数据与 package

日期：2026-10-18。范围：对 `public/game-ui` 逆向产物（tools.json 649 条 Menu/Menu2、
370 张 locale 表、replica data）做全量关系解析；把结论落进工作台代码（副本编辑、
拷贝条目、大图三态、真实容器 id）。未在本轮对游戏执行新实验；字段级定案沿用
[菜单编辑闭环](menu-editing-closed-loop-20261017.md)。

## 1. 一句话结论

**一个菜单条目 = 一条 Menu/Menu2 property（0x00B1B104）**；它出现在哪个菜单由
`uiCategories`（0x0DB9FC63，Key 数组）决定，它继承自哪个功能家族由 `parent`
（0x00B2CCCB）决定，其余字段全部是「指针」：文案指向 locale 表、图标/大图指向
图片资源、造价/维护在 GlassBox 模拟侧的 Unit property 里。编辑任何一面的正确
姿势都是：**复制或覆盖这条 property + 它指向的资源，绝不改动原始包**。

## 2. 三个 UI 表面及其数据源（全部实证）

| 表面 | 数据源 | 容器 instance |
| --- | --- | --- |
| 城市（city） | SimCityData Menu/Menu2 + GlobalUI2 布局 + ToolCategories locale | 14 个一级分类（AC12C904 道路、3193C0BE 分区、8E8B4E10 电力…） |
| 城市发展（development） | 0C298BDD 顶层产业分类（2026-10-20 更正） | 采矿 AFA8707D、钻油 3A798ED7、贸易 74CD2EB0、电子 BEE1C73E、赌博 0BFA0784、文化 94E412ED、超级巨塔 895E329B、研究中心 1D23D2FD、欧米伽 692D1886 |
| 区域（region） | `Layouts/Region/RegionUI.js` 布局；按钮组由**运行时填充** | 静态包内无容器 instance（大奇观/通勤者/交通/公用/服务/礼物 6 组） |

大学不是城市发展菜单。9D2EF585 仅为大学建筑子菜单，其成员的双重验证：tools.json 里 `uiCategories` 含 9D2EF585 的条目恰好是上面
8 个 instance；`workbench.json` 的 university 工具列表也是同样 8 个 id。
14 个一级分类本身**不在** Menu/Menu2 导出集里（被 649 条的 uiCategories 引用、
自身无条目）——它们的显示名走 ToolCategories locale 表，按钮由 HUD 运行时装配。

**两个特例：公共運輸 CBC27856 与公園 899BEA70 的调色板没有静态成员。**
按钮本身真实存在——全分类按钮序表 = `0x57572DAC`（组 0x09A4BA02）的
0x09A4A024 Key 数组（D8F2B788, AC12C904 道路, 3193C0BE, …, CBC27856,
899BEA70, 9B7C811F 災難, C51657E5…，共 25+ 个按钮，含调试分类）；但
tools.json 里没有任何条目的 uiCategories 指向这两个 id（两处安装、
reverse_key_search 全查过）。它们的面板由运行时按模块/单位分类装配。
工作台的补齐方案：`tools/generate-extra-containers.cjs` 按可验证的容器
关系聚合——公共運輸 = uiCategories ∈ {2A99C945 公車, 4B277752 客運火車,
DC8602C9 碼頭, A63DDC77 電車, 6AFE622F 渡輪}（16 条）；公園 = parent ∈
{C725C506, F1035727, 4EEB801B, F970A822} 四个公园子菜单根（66 条），
产出 `public/game-ui/tools/extra-containers.json`（含从包里补提取的
大图/槽位图标，`menu_asset_extract`），loader 只回填空分类。

## 3. 两条正交的关系边（解析的关键）

对 649 条做全量统计：`uiCategories` 取值共 **165 个容器 id**；`parent` 取值
主要是 AF042E9A（一级 plop 容器，485 个直接子条目）、12E3214B、6B28471C、
BE5744E0 四个根 + 若干模块容器。

两条边语义不同，不能混写：

- **归属边 `uiCategories`（0x0DB9FC63，数组）**：这条目「显示在哪些菜单」。
  一个条目可以挂多个菜单——`7DD8A627 低密度泥路 ∈ {5E0AC22E, AC12C904}`、
  `41158FE5 高密度街道 ∈ {5E0AC22E, AC12C904, 4A052724}`。SCP 教程里的
  "Parent Menu 数组元素 [0]" 就是它。**移动 = 替换选定元素；多挂 = 追加元素。**
- **继承边 `parent`（0x00B2CCCB，单值）**：这条目属于哪个功能家族/模板树。
  实证：`9A38E382 人行道` 的 parent 是 `12E3214B`（道路家族根，其子条目还有
  低密度泥路、低密道街道、電車軌道、Railroad…），而它的 uiCategories 是
  `9D2EF585`（大学菜单）——**功能上是路、显示在大学**。拷贝条目时 parent 保持
  不变（继承原家族），只动归属边。

其余容器事实：5E0AC22E 与 AC12C904（道路）成员完全相同（7 条），是平行容器；
12E3214B/6B28471C 是无标题的裸菜单根；容器自身的 titleTextId 在 370 张 locale
表里 **0/165 命中**——子菜单容器的显示名是运行时数据，静态层面只能靠成员
反推（工作台因此用「大学」等已核对名，其余容器暂不进 UI）。

**调色板每页容量**（对拍游戏规则）：二级面板条横向拉伸，引擎把槽位从左信息
块右侧排到右翻页钮（`page left`/`page right`）为止——**每页容量 =
⌊(右箭头左缘 − 行首 x) / 槽位 pitch⌋**，超出翻页、不换行。各分类布局的左块
宽度/箭头位置不同 → 每页数量随菜单不同（同一布局在不同舞台分辨率下拉伸后
也不同）。palette.js 的 14 个分类布局共用 pitch=121.6 / startX=245，但右箭头
落点各异；工作台 `palette.ts` 已按此规则推导，不再写死 8。

## 4. 单个菜单条目的元数据组织

以 `7DD8A627 低密度泥路` 为例（replica 提取物 + ui_tool_dump 字段定案）：

| 元数据 | property 键 | 指向 | 包侧 |
| --- | --- | --- | --- |
| 名称 | title 0x0A09F5FA | Text{表 0x6C969DEE, id} | locale 包（提取物 `public/game-ui/locale/6C969DEE.json`） |
| 描述 | desc 0x0A09F5FB | Text{表 0x50AA0BEA, id} | 同上 |
| 解锁提示 | unlockString 0x0DE84DDC（+unlockTargetAmount 0x0DE84DD3） | Text{表 0x4B54417A, id} | 同上 |
| 槽位图标 | toolIconKey 0x0977AA8F / iconNormal..0x09756955 六态 | PNG 0x2F7D0004 组 0x40E02400 | SimCityData（128×128） |
| rollover 大图 | marquee 0x0DDEEE56 | JPG 0x3F8662EA | SimCityData（454×263） |
| 菜单内排序 | uiPosition 0x0DC1E3E0 | int32 | 本条 property |
| 归属 | uiCategories 0x0DB9FC63 | Key 数组 → 容器 instance | 本条 property |
| 继承 | parent 0x00B2CCCB | Key → 家族根 | 本条 property |
| 锁定 | hardGate 0x0975695F | Key（语义按模板确认） | 本条 property |
| 弹窗统计行 | unitEffectTitle 0x0EB1FC05 / unitEffectParameter 0x0EB1FC22 | 模板文本（~amount~ 运行时填充） | 本条 property + locale |
| **建筑造价/维护**（plop） | 建造价 **0x0A8A41DA**（float）+ 维护 **0x09AE19D7**（/小时） | **EcoGame 脚本包**（`SimCityUserData/EcoGame/*-Scripts_*.package`）组 **40E0C000/48E0C000** 的 Unit 条目；菜单条目经 **0x0975695F（menuUnit）** → 单元配置 instance → EcoGame **同 instance** Unit 条目；子单位 0x07E0AD0A 一层求和（cToolPlop::FillFromProps 反编译定案） | 已对拍：診所 §20000/§400、鎮公所 §0/§200、風力發電廠 §3000+子单位=§8000/§40 |
| **道路/步道造价** | **0x0E76A7E2**（float 单价） | 0xD13BB4A6 指向的**分段定义**（组 40E1C400）上的该键；无键 = §0（cToolPathPlacer::GetPaletteRolloverCost 反编译定案）——**SimCity 2013 道路铺设免费**（街道/大道分段无键即免费）；部分步道菜单条目自带该键（=1） | 提取覆盖 27 条带价步道，其余道路显式 roadCost=0 |
| **资产配置 Cost**（模块/交易价） | **0x0C8EE92F**（float） | 组 **40E1C000**（本体）/ **61F0C000**（CoT）资产配置；与菜单无键引用，按 6C969DEE 名称字串联查 | menuUnit 链无命中时的兜底（107 条） |

以上三条链已实现为 `menu_cost_dump`（crates/sc-properties/examples），产出
`public/game-ui/tools/costs.json`（format 2：cost/maintenance/roadCost/configCost
+ unit/piece/config 溯源），loader `mergeMenuCosts` 只回填空值。再生成命令：

```
cargo run -p probes --release --example menu_cost_dump -- \
  public/game-ui/tools/costs.json \
  <SimCityData>/SimCity_Game.package <SimCityData>/SimCityDataEP1.package \
  <SimCityData>/SimCity_DLC0.package \
  <SimCityUserData>/EcoGame/SimCity-Scripts_<最新版本号>.package
```

注意：**大图资源 instance ≠ 条目 instance**。replica 资产名 `tool_<instance>.jpg`
里的 instance 才是资源真身——`EB0271D7` 的大图就是 `tool_AF256395.jpg`（共享）。
图标同理（`tool_icon_<instance>.png`）。凡「引用原图」的覆盖都必须按资产路径
反推 TGI，而不是默认指向条目自身（`menu-export.ts: marqueeInstanceFromAsset /
iconInstanceFromAsset`）。

## 4a. 统一数据模型：为什么"变体"这么多、格式却只有一个

所有菜单条目 = **同一条 0x00B1B104 property**，没有第二种格式。变体感来自
**可选指针的不同组合**——每条 property 只有两类东西：

1. **显示字段**（都指向 locale 表）：title 0x0A09F5FA、desc 0x0A09F5FB、
   unlock 0x0DE84DDC、统计行模板 0x0EB1FC05（文本如 `<B>公車數量</B>：~amount:number~`）；
2. **行为指针**：归属 uiCategories、继承 parent、menuUnit 0x0975695F（→
   单元配置/EcoGame Unit）、道路分段 0xD13BB4A6、图标 0x0977AA8F、
   大图 0x0DDEEE56、排序 0x0DC1E3E0。

弹窗的每一行都有确定出处（BuildToolJSValue / cToolPlop 反编译定案）：

| 弹窗内容 | 出处 | 公車站 79F3ED13 实测 |
| --- | --- | --- |
| 名称/描述 | locale 表（基础 6C969DEE；**DLC 用独立表**，如 CoT 7B45C298——之前显示 TGI 的根因是 dump 只记 instance 没记表 id） | 公車站 ✓ |
| 统计行 | unitEffectTitle 模板 + **unitEffectParameter（0x0EB1FC22）= Unit 条目上的 property 哈希**，值 = menuUnit 指向的 EcoGame Unit 条目上的 float | 0x0D8ACE92=3（公車數量）、0x0ECAEED3=40（公車容量）✓ |
| 造价 | menuUnit → EcoGame Unit 0x0A8A41DA，子单位（0x07E0AD0A）一层求和 | 13000+7000=**§20,000** ✓ |
| 维护/小时 | 同结构求和后**取负** | -(200+100)=**-300** ✓ |
| 槽位图标 | toolIconKey → PNG（transit 类条目 icon instance 恰与 menuUnit 同 instance） | tool_icon_0403377D.png ✓ |

道路/步道变体 = 带 0xD13BB4A6 分段指针、无 menuUnit（造价走分段 0x0E76A7E2，
无键=免费）；文件夹变体（公共交通/公園）= 只在按钮序表（0x57572DAC/0x09A4A024）
里出现、无静态成员，子分类标签（公車/路面電車/…）是运行时 UI，静态不可得。
公園的价格档标签（基本($)/運動($$)…）同理，工作台本轮以单列+分页呈现。

**locale 语言坑**：破解版安装的 `Locale/en-us/Data.package` 实际装的是繁中文本
（目录名对调），提取以内容实测为准。

## 5. 操作语义（数据层面 → 工作台落点）

**拷贝条目（例：低密度泥路 → 大学子菜单）**

1. 数据层面：完整复制 7DD8A627 的 property，保留 Type 0x00B1B104 / Group、
   parent 12E3214B、图标/大图/locale 指针；分配新 Instance；uiCategories 写
   `[9D2EF585]`。原图、原文、原功能全部共享——SCP 第 80 页的"只改 Instance"。
2. 工作台落点：Sheet「菜单条目」步的 **复制到菜单**（`duplicateToMenu`）。
   副本快照源条目当前有效视图（含已有编辑），获得新工作台 id
   （`COPY-<源id>-<目标>`）；编译时非游戏 id 走 FNV 分配新 instance，大图/图标
   键从复刻资产路径反推原资源 instance → 包内只写键、不复制图片字节。
3. 副本语义：所有编辑器**只编辑副本**——store 基础复刻数据只读，一切修改落在
   `edits` / `added` / `removed` 三张覆盖表；Sheet 打开时拷贝草稿、应用时写
   diff。`edits` 图片字段三态：`undefined` 不改 / `null` 明确移除 / 字符串替换。

**移动条目（例：大学人行道 → 道路一级菜单）**

替换 uiCategories 中选定元素、保留其余（已验证可行，见闭环文档 §4A）。
工作台当前把「移出 + 加入」表达为 `removed[menuId]` + `added[menuId]`，
编译时按**有效归属**（复刻反查 − 移出 + 新增）写 uiCategories。

**移除大图（本轮新增）**

Sheet 大图右上角 ✕ → 草稿 marquee=null → 应用后 `edits.marquee=null` →
面板/舞台立即无图；编译时不写 marquee 键（旧行为会写悬空键）。重新添加 =
点击大图区上传并裁切（本轮同时修复了裁切弹窗未挂载导致的上传链路断裂）。

## 6. package 关联：已确认与缺口

一个菜单条目相关的 package 至少四类：

1. **来源包**：承载这条 Menu/Menu2 property 的 SimCityData 包（同 TGI 覆盖即
   生效）。现状缺口：`ui_tool_dump` 枚举多包但不记录来源，tools.json 无
   per-entry 来源字段——需要带 provenance 的重扫或运行时 TGI→包索引。
2. **locale 包**：三张文案表（0x6C969DEE / 0x50AA0BEA / 0x4B54417A）。
3. **图片资源包**：槽位图标 PNG、大图 JPG（可与来源包不同包）。
4. **经济数据包**：SimCityData 的 40E1C000/61F0C000 配置 + **EcoGame 脚本包**
   （40E0C000/48E0C000 Unit 条目，含建造/维护），提取链见 §4 表。

游戏目录以实际可玩安装为准（本轮：`D:\ea-games\simcity_offline\SimCity：Cites
of Tomorrow\SimCityData`）。costs.json 再生成：
`cargo run -p probes --release --example menu_cost_dump -- public/game-ui/tools/costs.json <SimCity_Game.package> <SimCityDataEP1.package> <SimCity_DLC0.package>`

## 7. 本轮代码落点

| 文件 | 内容 |
| --- | --- |
| `src/lib/game-ui/workbench.ts` | ToolEdit 三态语义（applyEdit null=移除不回落） |
| `src/lib/game-ui/menu-export.ts` | marquee 键按需写入；MenuExportEntry.marqueeInstance；资产路径反推 instance；空 entries 不崩 |
| `src/stores/uiWorkbench.ts` | `effectiveEntries`（面板/舞台/导出同一有效模型）；`duplicateToMenu` + `copyTargetMenus`（跨视图）+ `membershipsOf`；发展视图用真实容器 9D2EF585；buildGameExport 只导变更集、归属=有效归属、整删除显式警告 |
| `MenuFormBasics.vue` / `MenuFormTip.vue` | 大图/槽位图显示 + 右上角移除；归属菜单 chips；复制到菜单（FSelect） |
| `panels/ui.vue` | 草稿携带完整图片值；应用=精确 diff；FImageCropper 接回模板（修复上传裁切） |
| 测试 | `menu-export.test.ts` 新增（键按需写入/资产反推/空包）；store 测试覆盖拷贝、归属、有效视图导出、三态移除 |

## 8. 边界与未决

- 覆盖包无法表达**整删除**（不写 = 游戏保留原条目）——工作台显式警告，不静默。
- 编译器从展示字段**重建** property 会丢未知字段/原字符串 id；字段级最小补丁
  （读原包→改单键）是下一轮 Tauri 侧工作（闭环文档 §4A）。
- 区域视图容器与全部子菜单容器名（0/165）是运行时数据；静态可编辑面 = 城市
  14 容器 + 大学 9D2EF585 + 自定义分类。
- hardGate 锁定键语义、六态图标的状态映射仍按模板/类型逐例确认。

## 9. 追加定案（2026-10-19 实机闭环）：词条交付与功能绑定补全

### 9.1 词条交付三定律（全部实机验证）

1. **位置无关**：游戏是扁平 TGI 表，包放 SimCityData / SimCityUserData/Packages
   只影响优先级。此前「locale 必须镜像进 Locale/<lang>」是误判（混杂变量），
   伴生包镜像逻辑（deploy_ui_locale_companions）已删除。
2. **格式决定成败**：locale 表必须是仿原版宽松排版（BOM + \r\n + 缩进 +
   `"键" : "值"`）；单行紧凑 JSON 会被整表解析丢弃（表内已有键也 ***）。
   buildLocaleResource / serialize_locale_items 均已改为宽松排版。
3. **自定义词条进自有表**：原版表 ID（6C969DEE/50AA0BEA/4B54417A）被 Locale
   数据先注册，其他目录的重定义整表不生效；全新表 ID 任意目录可用。
   OpenSCP 自有表 = `0xBF82F7CE`（FNV-1("openscp/strings")，前端
   OPENSCP_LOCALE_TABLE / Rust sc_properties::locale::OPENSCP_LOCALE_TABLE），
   先例：SimRollerCompletePack（裸包 + 自有表 01111109）。

### 9.2 属性编码第二格式坑

Text 数组的 `item_size` 是游戏加载器可见元数据：原版一律 **520**（文件内每项
仍 8 字节）。写成 8（此前 normalize_array_item_sizes 的"规范化"）会让游戏解析
多元素 Text 数组失败——菜单统计行渲染 *** 的直接原因。
`encode_canonical` 对 Text 数组一律声明 `TEXT_ARRAY_ITEM_SIZE=520`；
修复后原版属性 解析→编码 逐字节无损（prop_roundtrip 实证）。

### 9.3 功能绑定链补全（对拍 地铁Mod / 消防局 / baseline 人行道）

- **道路/步道**：菜单条目 → `0xD13BB4A6` 分段定义（如 B718847E）+
  `0x0E76A7E2` 单价；parent=12E3214B 道路家族。无 Unit。
- **Plop 建筑**：菜单条目 → `menuUnit 0x0975695F` → **EcoGame 脚本包**
  Unit（40E0C000；维护 0x09AE19D7、造价 0x0A8A41DA、子单位 0x07E0AD0A）
  → `0x0A0209A5` 数组 → **建筑定义 40E0C700**（本次新发现的第四层）；
  Unit 另有 `0x08E17ED7` → 40E1C000 资产配置。
- **地铁Mod 的新菜单姿势**（新菜单绑定已有功能的模板）：新菜单条目
  menuUnit → 自己的 Unit/资产配置（内容拷贝自 EP1）；六态图标自带包内；
  **title/desc 允许 Text{FFFFFFFF,FFFFFFFF} 空引用**（游戏容忍，渲染空白），
  名称经 `0x09E91B9C` 属性指针 → 车站定义属性（0x09B711C3 名称、
  0x0CB8E249 Unit 列表）；uiCategories → 包内自带的新分类容器。

### 9.4 探针沉淀（probes/ 目录）

locale_companion_build / locale_own_table / text_ref_hunt / prop_layout /
prop_bytes_diff / prop_roundtrip / pkg_merge——本轮全部结论的复现工具。


## 2026-10-20 城市发展九菜单接入

分类定义统一存于 `src/lib/game-ui/development-menus.json`，由生成器与工作台共同读取。
顶层容器和图标经 `ui_menu_survey` 扫描原版 SimCity_Game.package 与 SimCityDataEP1.package 核对：
顶层类为 0C298BDD；文化通过 0F978DAC 子分类聚合，超级巨塔通过 101B50A0 子分类聚合。
文化还包含 DLC 宗教场所 E286FFA5。条目来自 tools.json 的 uiCategories，而非建筑模块的 parent。

- 文化：地标 3F43E038、体育场馆 546FD9CE、景点 0653C631、宗教场所 E286FFA5。
- 超级巨塔：普通 E996DFCF、精英 41E4831D；楼层模块属于建筑子菜单，不混入顶层。
- 采矿 0707E659 和钻油 25173F4B 是建筑模块容器；旧待办中的这两个 ID 不可作为顶层分类。
- 九类条目数分别为 5、3、3、3、6、37、7、2、2，共 68 条；大学不在此视图。
- 聚合条目保留原始 uiCategories，编辑导出不会把文化/巨塔的分组 ID 替代真实成员归属。
- 新分类及子分类名称通过 zh-CN/en-US i18n；游戏资源文案仍采用安装包提取的语言。

验证：28 项菜单/store/loader 测试通过，vue-tsc 检查通过；浏览器已核对九入口、文化和巨塔子分类、
打开面板时中英文切换。未执行游戏内注入验证。
