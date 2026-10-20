# 建筑可建数量：V 博士与地标（2026-10-20）

## 结论与范围

核查原版 UI JavaScript、游戏 DBPF property、EcoGame 脚本属性及现有零售反汇编材料。随后已接入菜单编辑 sheet 和后端资源写回；尚未在运行中的游戏城市验收新上限。

数量接口不是单个 `availableCount`：原版通过 `tools/<toolID>/toolCountUsed` 读取计数对象。地标需要同时表达整组最多 3 个、每种最多 1 个；V 博士大楼额度为 1。编辑态独立存放在工作台 `countLimits`，按模拟建筑和共享资源标识管理，不伪造当前城市已建数。

## 读取端复核与更正

此前仅凭数值推断 `0x0E854247`、排除 Module Limit 的结论不充分。零售镜像 `tmp/dev-sample-analysis/ref/SimCity_dump_SCY.exe` 的 `0x0066AAA3–0x0066AC27`（本次摘录 `tmp/tool-count-retail.asm`）明确：

- 读取目标 Unit 的 `0x0E51C90F`，以对应全局 bin 的 capacity 作为 `toolCountLimit`。
- 读取 `0x0DF1467C`，在 16 项 `kModuleResourceIDs` 中找到下标，再读取 `0x0CC8BE51 + 下标` 的 **Float**，转换成整数供 `toolIndividualLimit` 使用。
- 地标和 V 博士的该引用均为 `7D11699A`（下标 0），所以单项字段是 `0x0CC8BE51`，值为 Float 1。不能写成 Int32 标量。
- `0x0E854247` 未作为本次写回目标。

开发版反编译 `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c:773580` 有同源流程，可辅助理解，零售材料用于核对当前读取路径。两者都存在以 `HasBeenPlaced` 生成 0/1 使用量的非编辑器分支，因此静态限额编辑不能被宣称为已实现任意城市运行态计数显示。

## 原版 UI 字段

证据：`tmp/js/template_registry.js:1540` 的 `UpdateToolLimitsAndGrouping`；同文件 1693 行 rollover 的 `toolCountInfo` 查询。常量在 777、781 行定义。

| 返回字段 | UI 使用语义 |
| --- | --- |
| `toolCountUsed` | 当前工具的已使用数量 |
| `toolIndividualLimit` | 当前工具的单项上限；普通模式已使用数大于 0 且等于上限时显示“已放置”并禁用 |
| `toolCountShared` | 共享额度的已使用数量；上限为正且此值达到上限时锁定 |
| `toolCountLimit` | 当前生效的额度上限，与共享计数比较；也用于分组计数显示 |
| `toolModuleLimitIndex` | 数量分组标识；相同非零值的工具在 UI 内归组，不能当成数量 |
| `toolLimitOverrideBinSpecified` | 是否使用额度覆盖资源；原版对其有独立的“已放置”判定分支 |
| `toolIsHQReliant` | UI 单项分组处理中使用的 HQ 依赖标志 |

这些是运行时返回字段，而非可以直接写回菜单 property 的属性名。已有历史实机查询样本 `tmp/ui-log-samples/v5/queries.json` 证明返回对象包含这些字段；该样本不代表本次两个建筑的实时计数。

原版分组数字显示的是 **已使用 / 上限**（`kTxtModulesUsed` / `kTxtModuleLimit`），不是剩余可建数。分组时仅对 `toolCountShared != -1` 的后续工具累加；不能把 sentinel `-1` 当作普通数量。

普通有限额建筑的“剩余可建”可以在单项和共享两层都有效时计算为：

```text
max(0, min(单项上限 - 单项已建数, 共享上限 - 共享已建数))
```

这是这两个案例的业务解释，不是覆盖所有工具的通用替代实现；覆盖额度、升级、HQ、解锁条件仍需分别处理。

## 静态资源定义

以下三个属性是按相同下标配对的数组。当前注册表没有其正式符号名，因此保留 hash，不编造 `kProp...` 名称：

| 属性 hash | 类型 | 本次数据中的作用 |
| --- | --- | --- |
| `0x0A8DF590` | `Key[]` | 资源 ID 列表 |
| `0x0A8DF591` | `Int32[]` | 对应资源容量 |
| `0x0A8DF592` | `Int32[]` | 对应初始量；本次条目均为 0 |

### V 博士大楼

- 菜单工具 `BC45E41B`：`SimCity_DLC0.package`，TGI `00B1B104:41F0C900:BC45E41B`。
- `0x0975695F` 指向模拟单元 `AFA6EB12`。
- `HeroesAndVillains-Scripts_277156632.package` 中该单元继承 `13C05239`（两者 group 均为 `41EFC000`）。
- `13C05239` 的 `0x0BF5D2E3`（`kPropUnlock_Bin`）指向资源 `2B23B20E`，`0x0BF5D2E4`（`kPropUnlock_BinAmount`）为 `1`。
- 同包 TGI `00B1B104:41EFC700:B80ECEAE`，上述三个数组的第 17 项（零基下标 16）为 `2B23B20E / 1 / 0`。
- 同单元还有 Float `0x0CC8BE51 = 1`，由 `0x0DF1467C -> 7D11699A` 选择，作为单项 UI 限额。写回覆盖在叶子 Unit `AFA6EB12` 上，保留父引用，不改共享模板。

### 地标建筑

共享定义位于 `SimCity-Scripts_287520926.package` 的 TGI `00B1B104:40E0C700:318E8443`：

| 资源 ID | 已知名称 / 对应建筑 | 容量 | 初始量 |
| --- | --- | ---: | ---: |
| `4B427C97` | `kResourceIDLandmarksUnlocked` | 3 | 0 |
| `3B6116BF` | `kResourceIDHasLandmark` | 3 | 0 |
| `4A73D81E` | 帝国大厦的单项计数资源 | 1 | 0 |
| `A535ED72` | `kResourceIDHasLandmarkTokyoTower` | 1 | 0 |
| `31054835` | 自由女神像的单项计数资源 | 1 | 0 |

该表共 24 项：前两项容量为 3，其余 22 项容量为 1。容量不等于当前可用量；初始量为 0 也不表示游戏永远不可建，额度会由模拟规则改变。

继承链与引用：

- 帝国大厦工具 `C0E5BE77` 位于 `SimCity_Graphics.package`，TGI `00B1B104:09878A01:C0E5BE77`，`0x0975695F` 指向 `118282E9`。
- 模拟单元 `118282E9`（`40E0C000`）继承 `4E01E311`（`48E0C000`，`TemplateToyBase_landmark`）。
- 共享模板 `0x0BF5D2E3 -> 4B427C97`，`0x0BF5D2E4 = 1`，另有 `0x0E51C90F -> 4B427C97`。
- 帝国大厦 `0x0EC5D62E -> 4A73D81E`；东京塔 `084DB48F` 的同属性指向 `A535ED72`；自由女神 `9F35D0EE` 指向 `31054835`。三者均继承地标模板，且 `0x0E854247 = 1`。
- `0x0EC5D62E`、`0x0E51C90F` 的正式符号名仍保留 hash；后者的容量读取映射已由零售读取端核实。单项编辑还同步修改前者引用的专属计数资源容量。

因此已有帝国大厦时，即使地标总数只有 1，帝国大厦自身也不能再建；另外两种可用地标仍可建。第三种落成后，共享额度满，所有尚未建造的地标也应受限。不能让每种地标各自获得 3 个额度，亦不能用整个“文化”菜单计数代替地标共享计数。

## 不应混淆的字段

- 菜单属性 `0x0DE84DC9 / 0x0DE84DD3 / 0x0DE84DDC` 是解锁条件资源、目标值、文案，不是上限数组。
- `kPropToolLimitOverride` 为 `0x0F7FA2A8`。本次两个菜单工具及其直接公共模板没有该字段，不能把它说成这两个案例的默认上限定义。
- `0x0CC8BE51` 等 Module Limit 属性既被模块路径使用，也被主建筑路径使用；必须结合 `0x0DF1467C` 和读取上下文确定语义。

## 复核方式

使用现有 `tool_info.exe`、`prop_dump.exe` 读取上述 package / TGI；`reverse_key_search.exe` 追踪 `4B427C97,2B23B20E` 定位容量数组。临时脚本按数组下标配对并断言地标共享容量 3、三种地标单项容量 1、V 博士资源容量 1，全部通过。原始类型 dump 留在 `tmp/landmark-capacity-evidence.txt`。

## 编辑与写回实现

- `game_count_limits_read` 从安装包中的菜单 → 模拟 Unit → 继承链发现能力。无法完整解析的限制不会显示为可编辑；带工具额度覆盖字段的复杂情况暂不暴露为普通上限。
- 浏览器预览使用 `public/game-ui/tools/count-limits.json`，它由同一后端解析器从安装包生成，非按名称/菜单硬编码。
- 单项编辑按源建筑保存；菜单副本仍引用同一个模拟 Unit，因而共享这个上限。共享编辑按 bin ID 保存，跨菜单和视角只有一个有效值。
- `game_count_limits_patch` 批量写回完整 PropertyFile：单项 Float 写在叶子 Unit；容量只修改 ID 数组对应的 Int32 数组元素，保留其他元素和初始量。
- 地标共享修改同时更新 `LandmarksUnlocked` 和 `HasLandmark` 两个容量；各地标专属容量与单项修改同步。V 博士的单项修改同时更新其 `2B23B20E` 容量。
- 写回接受本次导出的已有资源字节，因此同一资源的经济与数量编辑合并，不后写覆盖前写。冲突的共享值、非法整数、缺失或格式不匹配的资源会使编译失败，不能静默漏写后报告成功。
- TGI 新旧不是格式规则。此处使用源 TGI 是为了保留现有引用；编码通过 `encode_canonical` 保持 Float、Int32[]、Key[] 及数组步长。若将来复制模拟资源为新 TGI，必须同步重定向全部引用。
- 支持 1–16777216 的整数，确保 Float 表示精确；没有把 0 擅自定义为无限制或禁建。

验证包括类型检查、前端表单与共享状态测试、Rust 数组保留/冲突拒绝测试、本机游戏数据往返测试。浏览器实测 V 博士只显示单项 1，帝国大厦显示单项 1/共享 3；将共享改为 5 后东京塔显示单项 1/共享 5。浏览器不能生成缺少游戏资源的数量覆盖包，需在桌面应用编译。尚未部署到游戏或验证存档中已有 bin 是否即时更新，运行态城市验证需重启/加载游戏后进行。
