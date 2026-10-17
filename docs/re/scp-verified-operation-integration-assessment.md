# SCP 已验证操作与 OpenSCP 集成评估

日期：2026-10-17

本评估只覆盖 `scp.pdf` 中与 SimCityPak / Property / 菜单和资源编辑有关的操作，
以及 `D:\source-map\Simcitypak-v2` 中可直接核对的 C# 实现。日志追查暂不作为本轮前置条件。

## 结论

可以开始集成，而且最适合从“真实菜单 Property 的最小 overlay 编辑”开始。原因是：

1. SCP 文档给出了已经在游戏中验证过的闭环：查找菜单 Property、修改 `Parent Menu` 的 Instance、导出单资源 package、保存、进游戏观察菜单位置。
2. 原始 C# 已明确实现了 Property 大端读写、Parent 继承展平、菜单 Property 的 TGI 复制，以及 icon / marquee / unit / LOD 依赖的递归复制。
3. OpenSCP 已经有 DBPF 读取、Property 解析、规范化编码、单 Property overlay writer、菜单层级索引和前端菜单工作台。
4. 缺口集中在“把语义编辑连接到真实资源写回”，不是重新逆向 DBPF 或 Property 格式。

因此可以先做一个受约束、可复验的菜单迁移器，不需要等待日志系统完成；但当前不能把前端工作台的“新增/删除/改图标”称为已可用于游戏的 mod 功能。

## `scp.pdf` 中已经确认的可行机制

`scp.pdf` 第 75–80 页（PDF 页码 2–7）描述了一个完整案例：把行人路径从大学菜单移动到道路菜单。

| 文档动作 | 真实作用 |
| --- | --- |
| 打开 `SimCity_Game.package` | 读取原始 DBPF 资源 |
| 过滤 Property 并找到目标（Pedestrian Path） | 目标是一个 Menu/Menu2 Property，而非 UI 截图或布局文件 |
| 查看 `Parent Menu` | 读取一个 Key/TGI 引用；它决定条目归属的父菜单 |
| 复制原 Instance 值 | 保留原菜单条目本身，只切换父菜单关系 |
| 将目标 Property 导出为新 package | 生成只覆盖这一条资源的 overlay |
| 在新 package 中粘贴 Instance | 修改复制出的资源引用 |
| 保存 package | 使 overlay 成为可安装的 mod 文件 |
| 发现原菜单项消失后修改 TGI | 避免 overlay 与原资源使用相同 TGI，改为新增资源并保留原条目 |
| 再保存并进游戏 | 以游戏菜单实际出现情况验收 |

这个案例的关键不是“把一项加到数组”，而是 **Property 的 TGI 身份、Parent Menu Key 和 package 覆盖规则共同决定结果**。

同一文档后续章节还展示了车辆纹理流程：先从 PROP 的 `Vehicle Models` 数组取 RW4 模型，再定位对应材质/纹理资源。这说明“语义 Property → 依赖资源 → 修改 → 写回”的模式也适用于模型资产，但纹理章节的完整写回成本高于菜单迁移。

## 原始 C# 已确认的机制

### Property 编码与继承

`SimCityPak/PackageReader/Properties/PropertyFile.cs` 已确认：

- Property count、hash、类型和 flags 按大端读取；数组保存 count 和 item size。
- `Write()` 按 hash 排序后重新编码，避免 Dictionary 顺序导致的不稳定包。
- Parent 使用 `0x00B2CCCB`，`ViewPropertyFile.xaml.cs` 的 `GetParentProperties()` 逐级读取并展平父链。
- 编辑窗口支持已有标量编辑、数组项插入、删除、复制/粘贴、flatten parent。

### 菜单资源复制

`ViewPropertyFile.xaml.cs` 的菜单复制流程（`mnuCopy_Click`）已确认：

- 只对 `Menu2` 类型执行菜单复制。
- 为菜单 Property 分配新的 Instance ID。
- 若存在 `menuIcon = 0x0977AA8F`，复制被引用的图标资源并重写 Key。
- 若存在 `menuMarqueeImage = 0x0DDEEE56`，复制大图并重写 Key。
- 若存在 `menuUnit = 0x0975695F`，展平 unit Parent，复制 unit 的 display model 和各级 LOD，并逐级重写 Key。
- 最后将复制后的资源集合写入新 package。

`PropertyConstants.cs` 同时给出了菜单字段和 lot/unit 字段的稳定 hash；这与 OpenSCP 当前的 Property 解析模型可以直接对接。

## OpenSCP 当前状态

### 已经可以复用的部分

| 能力 | 当前证据 | 状态 |
| --- | --- | --- |
| DBPF 资源读取 | `crates/dbpf`、`src-tauri/src/package_service.rs` | 可复用 |
| Property 解析与规范化编码 | `crates/sc-properties` | 可复用；已有 round-trip 测试 |
| Parent / 语义预览 | `src-tauri/src/prop_models.rs`、`crates/sc-properties/src/semantic.rs` | 已接入读取/预览 |
| Menu/Menu2 层级模型 | `docs/re/game-ui-menu-relationships.md`、`public/game-ui/replica/data/*` | 已逆向并用于复刻 UI |
| 单 Property overlay 输出 | `patch_property_overlay` | 可写已有字段 |
| 前端菜单编辑草稿 | `src/stores/uiWorkbench.ts`、`src/pages/studio/panels/ui.vue` | 可视化可用，但当前只保存 JSON |
| 资源依赖保留 | DBPF overlay writer | 对未编辑资源可保留；尚未有菜单复制编排器 |

### 仍未关联或未确认的部分

| 缺口 | 影响 |
| --- | --- |
| 工作台 `edits/added/removed` 没有映射到真实 Menu/Menu2 TGI | UI 改动不会进入游戏 |
| `patch_property_overlay` 只能替换已有字段，数组长度不能变化 | 不能直接实现新增菜单项、复制资源或删除数组成员 |
| 没有“新 Instance/TGI 分配 + 引用重写”服务 | 按 SCP 文档复制条目会覆盖原项或失去原项 |
| 没有 icon/marquee/unit/LOD 依赖递归复制 | 复制菜单项时会引用原资源或产生断链 |
| 没有把 Parent/Menu2 语义与 locale/layout 资源一起校验 | 能生成包不等于游戏菜单可见、文字和图标完整 |
| 工作台有 `cost/upkeep/unlock` 等字段，但文档和静态 Menu Property 未证明这些值的统一来源 | 这些字段只能保持 unknown，不能伪造写入 |
| 尚未完成游戏内安装与验收 | 当前只能宣称格式/重读写验证，不能宣称游戏行为验证 |

## 建议的最小集成顺序

### M0：真实资源识别（先做）

1. 从已打开 package 的索引中筛选 `0x00B1B104`，识别 Menu/Menu2 的 instance type。
2. 用已知字段解析 `Parent Menu`、`uiCategories`、`uiPosition`、`menuIcon`、`menuMarqueeImage`、`menuUnit`。
3. 在工作台条目上保存源 package ID、完整 TGI、Property TGI 和字段 hash，禁止只用前端 instance 字符串。
4. 对每个条目显示“已解析 / 未解析 / 仅静态复刻”状态。

### M1：先实现文档中的“移动现有条目”

这是风险最低的真实 mod 验证：只改目标 Menu2 Property 的 Parent Menu Key，导出一个单资源 overlay。

- 不复制 icon、marquee、unit 或 LOD。
- 不改变数组长度、不新增 TGI。
- 写出后重新打开 overlay，逐 hash 对拍原值与新值。
- 提供 package 安装路径和游戏内验收记录。

这一步完成后，OpenSCP 就能具备第一个“文档已验证、游戏可验证”的 mod 开发能力。

### M2：菜单条目复制

以原始 C# 的 `mnuCopy_Click` 为行为基准，实现一个后端编排器：

1. 给 Menu2、icon、marquee、unit、display model、LOD 分配新 Instance。
2. 重写所有直接 Key 引用。
3. 保留源包中未涉及的资源。
4. 生成一个 overlay package，并在写出后重新解析所有资源引用。
5. 对复制资源建立依赖清单，缺失依赖时拒绝导出。

这一步才支持“原菜单项保留，同时新增到另一个菜单”的文档案例。

### M3：数组与结构编辑

再扩展 `patch_property_overlay`：支持受白名单约束的数组长度变化、插入、删除和 Property 新建。所有新增字段必须有明确的 PropType 与原始 C# / 游戏样本证据，不能把 UI 工作台的任意字段直接写入 Property。

## 当前判断边界

- **可以开始**：菜单 Parent Menu 移动的真实 overlay；Property 标量/Key 的受约束编辑；重读写和引用完整性验证。
- **暂缓**：任意菜单新增、删除、复制；依赖资源复制；任意车辆纹理重编译；把 UI 工作台所有字段都写成游戏属性。
- **尚未证明**：OpenSCP 输出的菜单 overlay 已经在真实游戏安装中成功加载。必须完成一次固定样本的安装、启动和菜单截图/行为验收后，才能把 M1 标为游戏验证通过。

## 证据文件

- 用户材料：`C:\Users\FluffyCHi_Xing\Desktop\scp.pdf`，相关页 75–80（PDF 页 2–7）。
- 原始实现：`D:\source-map\Simcitypak-v2\SimCityPak\PackageReader\Properties\PropertyFile.cs`。
- 原始实现：`D:\source-map\Simcitypak-v2\SimCityPak\Views\ViewPropertyFile.xaml.cs`。
- 原始常量：`D:\source-map\Simcitypak-v2\SimCityPak\Views\AdvancedEditors\LotEditor\PropertyConstants.cs`。
- OpenSCP 菜单关系：[game-ui-menu-relationships.md](game-ui-menu-relationships.md)。
- OpenSCP 当前资产写回边界：[asset-flow-integration.md](asset-flow-integration.md)。
- OpenSCP 写回入口：`src-tauri/src/package_service.rs` 的 `patch_property_overlay`。
- OpenSCP 前端工作台：`src/stores/uiWorkbench.ts`、`src/pages/studio/panels/ui.vue`。

