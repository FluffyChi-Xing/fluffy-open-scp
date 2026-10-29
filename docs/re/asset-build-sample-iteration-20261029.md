# 构建系统样本迭代：大学与商业建筑资产 mod

日期：2026-10-29

本轮只使用工作台已经创建的两个资产 mod 做构建方案验证，不接入资产工作台，也不修改这两个用户项目。

## 样本事实

| 样本 | 当前内容 | 对构建方案的意义 |
|---|---|---|
| `C:/Users/FluffyCHi_Xing/AppData/Roaming/com.fluffy.openscp/mods/大学-综合实验楼-资产项目` | `package.json`、8 个节点 schema、`asset.lot.json` v1、一个 26.6 MB 的 LOTM；当前文档有 12 个单元但六个 texture input 和 building 节点的 `config.asset/document/property` 仍为空 | 这是“只有模型输入”的最小样本。它可以验证资源快照、LOTM 身份和缺失绑定诊断，但不能直接进入 Property 写回 |
| `C:/Users/FluffyCHi_Xing/AppData/Roaming/com.fluffy.openscp/mods/商业地标资产开发` | `package.json`、`asset.lot.json` v1、一个 45.99 MB LOTM、15 个 PNG 和 5 个参数 JSON；文档有 17 个单元，包含 lot、prop、light、path 等单元 | 这是“有场景语义和六槽候选输入”的复杂样本。它可以验证文档闭合、资源依赖清单和不支持字段阻断，但仍没有参照 package、Unit/Property TGI 或输出包 |

两个样本的 `building-asset-7.json` 都只是 v1 节点配置，`asset.lot.json` 也仍是 v1，尚未成为 M1 所需的 v2 Property schema。当前不能把“存在 LOTM/贴图”误判为可构建的游戏资产。

## 方案调整

M1 的第一个样本目标从“直接构建新建筑”调整为“样本审计 + 原生行为克隆”。每个样本先生成一份外部的 build fixture（放在项目 `tmp`，不写回用户 mod）：

```text
sample-fixture/
  source/                  # 样本原始文件的只读快照和 SHA-256
  asset-v1.json            # 原始工作台文档（若存在）
  binding-report.json      # 节点、槽位、LOTM、语义单元的诊断
  m1-schema.json           # 仅在 reference.package 和 Property TGI 齐全时生成
```

fixture 阶段只做纯读取和诊断：

1. 校验 `package.json` 节点图、节点 schema 引用和六槽连接。
2. 读取 `asset.lot.json`（商业样本）或标记其缺失（大学样本）。
3. 为 LOTM、贴图、参数 JSON 记录路径、内容摘要和识别出的格式，不把它们假设为 RW4/Property 可写资源。
4. 将每个语义单元标为 `cloned`、`modified`、`unresolved` 或 `unsupported`。
5. 只有存在项目内参照 `.package`、明确 Unit/Lot/菜单 TGI，并且字段 patch 可表达时，才生成 M1 v2 schema；否则只生成阻断报告。

## 两个样本的预期结果

- 大学样本第一轮应得到 `missing-reference-package`、`empty-building-asset` 和 `missing-six-slot-inputs` 诊断；它的 `asset.lot.json` 可作为场景文档快照，但不能代替参照包。它适合验证“最小输入不能静默产包”。
- 商业样本第一轮应识别 lot 尺寸、路径点、prop、light 和 LOTM，但对 `scene.model`、六槽贴图写回、Unit/菜单/ER2 依赖报告为 `unsupported` 或 `unresolved`。它适合验证“复杂文档不能被 M1 丢字段”。
- 两个样本都不应调用 `asset_build_m1_build` 产出可安装包，除非另备一个包含参照建筑资源的 package 副本，并补齐目标 Property 的完整 TGI。

## 后续实现顺序

1. 在后端新增独立的 sample fixture/diagnostic 纯函数和测试，输入为项目根目录下的文件快照，输出稳定的 `binding-report.json`；不改工作台入口。
2. 用商业样本覆盖“lot/prop/light/path 被发现但阻断写回”的测试，用大学样本覆盖“仅 LOTM 仍不能构建”的测试。
3. 准备两个临时参照 package（大学和商业各一个），只选一个可确认的 Property 标量字段作为 M1 patch，验证重读和 overlay 报告。
4. 再扩展到 Unit/Lot/菜单依赖闭包；在此之前不实现六槽贴图或 GLB/LOTM → RW4 写回。
5. Raster/RW4 编译器仍排在 Property-only 样本闭环之后，且不把工作台 v1 schema 直接升级为可写契约。

## Property-only fixture（2026-10-29）

已在 `tmp/asset-build-fixtures` 建立临时、可删除的回读 fixture。参照包来自本机
`D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package` 的只读副本，两个样本分别
命中真实 Lot Property：

| 样本 | Lot Property TGI | 原始 LotSize | fixture patch |
|---|---|---:|---:|
| 大学 | `00B1B104:58F0C000:2063CCDD` | `[192, 96]` | `[193, 96]` |
| 商业建筑 | `00B1B104:41F0C000:0483239E` | `[80, 80]` | `[81, 80]` |

两个 patch 都带 `expected` 前值条件，只验证 Property 编码、DBPF overlay、资源重读和
源包不变。测试命令为：

```powershell
$env:SC_ASSET_FIXTURE_ROOT = (Resolve-Path 'tmp/asset-build-fixtures').Path
cargo test -p fluffy-open-scp real_asset_sample_property_fixture_readback --lib -- --ignored
```

fixture 只在 `tmp` 下生成，不写回 AppData mods，也不接入工作台。它证明了两个样本
已经可以进入 Property-only 构建验证；模型替换、六槽写回和 Unit/Menu/ER2 仍未进入
这个 fixture。

已补充 raster-only overlay fixture 测试：每个真实 Lot Property 都生成一个新的
raster instance，修改顶层 RGBA 的一个像素，重新编码为 pixFmt21+mip 链，并把
`0x0CCB7FD5` LotMask 引用指向新 TGI。overlay 重读后，Property 引用和 raster
像素均与预期一致；源 `SimCity_DLC0.package` 的 SHA-256 前后一致。fixture 输出只在
`tmp/raster-overlay-fixtures`，不写入游戏目录或 AppData mods。

处理逻辑已抽到 `asset_build_m1::raster`：它是纯函数式的 raster 编码器，负责解码、
单像素变更、重新编码和语义回读；`ensure_target_free` 在写入前拒绝已有 TGI，避免
覆盖官方资源。包打开、Property patch 和 overlay 写盘仍由上层 fixture 负责。

样本扫描现在同时输出 lot、prop、bin、path point、path 和 path entry 计数，并把
`paths[]` 的缺失或非法 `entry`、缺失 `source` 标为阻断诊断。当前 AppData 中两份
JSON 都声明了一个 `paths[].entry`：大学为 `675B61B1`，商业为 `418A18B1`。这说明
扫描器此前把“有路径点”与“路径语义闭合”混在一起的问题已经拆开。下一步会在
引用包可用时继续校验 `0x0CC8FE7A` 的 per-path Key 和 `0x40E0C400` PathEntry
资源是否实际存在；仅有 JSON 字符串不会被视为合法道路。

跨 `SimCityData` 全部 package 的闭包测试已加入：它按 source TGI 找到路径 Property，
检查 `0x0CC8FE7A` 中是否绑定当前 entry，再检查对应的
`00B1B104:40E0C400:<entry>` 资源。当前两份 AppData 样本均通过这三层检查；如果
商业样本在另一份工作区状态中确实没有 PathEntry，这个阶段会产生
`path-source-property-missing`、`path-entry-not-bound` 或 `path-entry-resource-missing`
并阻止 package 构建。该结果也说明当前文件扫描得到的商业 JSON 与“完全没有 entry”
的描述不一致，需要以实际 schema/package 文件为准继续定位差异。

这套校验已接入 M1 `build_from_schema` 的 staging 前 gate：只要项目内存在
`scene.document`，路径语义会在任何 Property patch、输出目录创建或 package 写盘
之前执行。缺 entry、source Property、per-path Key 或 PathEntry 资源都会直接返回
错误，并保证不会留下 `.build` 产物。没有场景文档的旧 Property-only fixture 会继续
保持兼容，直到它声明了实际场景文件。

大学的真实 package fixture 已进一步验证：DLC0 中的源 path Property 绑定
`675B61B1`，但对应 PathEntry 资源实际位于 `SimCity_Game.package`，TGI 为
`00B1B104:40E1C400:675B61B1`。构建前把这一个资源合并到临时 DLC0 副本后，M1
成功生成 package，并在输出中重读到该 PathEntry。这里的 group 不能硬编码为
`40E0C400`；Property Key 与资源实际 group 需要按 instance 和 `0xC400` 组族解析。

## Raster 依赖闭合（2026-10-29）

Lot 与建筑贴图按游戏原生 raster 处理。两个样本的 `sourceLot` Property 均通过
`0x0CCB7FD5` 引用 `0x2F4E681C` raster，实际资源位于只读的
`D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package`：

| 样本 | LotMask key | raster TGI | 原始大小 | 编码结构 |
|---|---|---|---:|---|
| 大学 | `00000000:00000000:1CF819C1` | `2F4E681C:00000000:1CF819C1` | 174,816 B | pixFmt 21、可解码、含 mip |
| 商业建筑 | `00000000:00000000:DEDC4095` | `2F4E681C:00000000:DEDC4095` | 87,436 B | pixFmt 21、可解码、含 mip |

`asset_build_samples::inspect_project_with_raster` 现在会输出 LotMask key、解析出的
raster TGI、包路径、SHA-256、尺寸、mip 数和 `decode_supported`，并用
`RasterImage::decode_top_mip_rgba` → `build_raw_bgra` → 再解码做像素级 roundtrip
检查。真实样本 ignored 测试已通过；测试只读映射游戏包，不会写入游戏目录或 AppData
mods。建筑六槽中的 PNG/JSON 仍只作为预览输入，不能当作原生 raster TGI。

这一步闭合了 LotMask 的 raster 依赖和编码验证，但没有宣称模型 LOTM/建筑材质已经能
转换为 RW4，也没有把 raster overlay 接入 M1 Property-only 构建器。下一步应在临时
fixture 中复制 raster、做可逆像素变更并 patch LotMask 引用，再验证 overlay 的资源
重读和源包哈希不变。

## 当前明确边界

- 不接入资产工作台，不增加构建按钮，不改变现有 v1 schema。
- 不修改 `C:/Users/FluffyCHi_Xing/AppData/Roaming/com.fluffy.openscp/mods/大学-综合实验楼-资产项目` 或 `C:/Users/FluffyCHi_Xing/AppData/Roaming/com.fluffy.openscp/mods/商业地标资产开发`。
- 不把 LOTM、PNG、参数 JSON 当成已经可写的游戏资源。
- 不生成使用 EA 原生资源的可发布包；样本只用于本地研究和回读验证。
