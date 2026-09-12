# OpenSCP 迁移 Roadmap — SimCityPak (C#/WPF) → Rust + Tauri

> 迁移源：`D:\source-map\Simcitypak-v2`（altinctrl/SimCityPak 的 CLI-enabled fork）
> 权威上下文：源项目 `HANDOFF.md`（含大量已验证的格式结论，实现时须先读对应小节）
> UI 侧规划（源版 UI 盘点、菜单目录设计、U1–U5）：`docs/roadmap/ui.md`
> 本文档为本地规划文档（docs/roadmap 已 gitignore），随实现进展滚动更新。

---

## 1. 目标与总体判断

**功能目标**：先完成可靠的 P0 格式底座，再构建 P1 Modding Suite。P0 负责 DBPF/RefPack、RW4、属性表和可保真写回；P1 将 `modding.pdf` 所描述的外部 DCC → OBJ → 目标 RW4 Mesh → 保存 package → 游戏验证流程收敛为声明式 manifest、Validate、Preview、Build 和 deterministic overlay package。

**P1 重点**：OpenSCP 不再只是资源浏览/导出器，而是一个面向真实模组开发的工作流编排器：声明资产与 LOD、解析依赖、导入 OBJ、应用属性 patch、自动校验并生成可追踪构建报告。详见 `docs/roadmap/modding-suite.md`。

**非目标**：P1 不实现完整 3D DCC、不默认自动生成高质量 LOD、不注入游戏进程、不承诺真正游戏内 live reload、不建立远程 mod registry。

**性能目标**：文件解析等关键环节达到原版一个数量级以上的提升。原版的结构性瓶颈（已核实源码）：

| # | C# 瓶颈 | 源码证据 | Rust 对策 |
|---|---|---|---|
| P1 | 每次读资源重新 `OpenRead()` 打开包文件 | `DataBaseIndex.cs:109` → `Owner.GetData`（`DataBasePackedFile.cs:140`） | `memmap2` 打开一次，资源读取变为 `(offset, len)` 切片，零拷贝 |
| P2 | 顺序 FileStream 逐字段读索引，391MB 的 EP1 包打开秒级 | `DataBasePackedFile.Read`（:314 起，循环 `stream.ReadU32()`） | 索引区一次性切片 + `bytemuck`/LE 直读，批量构建 |
| P3 | XNA 4.0 x86-only，贴图解码依赖 GPU GraphicsDevice（WPF 互操作） | `Texture.cs:130,136`（`GraphicsDeviceService`）；纯托管 DDSLib 仅部分路径 | 纯 Rust DXT1/DXT5 块解码（CPU，无 GPU 依赖），x64 原生，rayon 按 4×4 块并行 |
| P4 | 解析模型即 WPF ObservableObject，UI 线程耦合；hex 视图全量拼字符串导致 OOM | HANDOFF §4「OOM crash fixed」；`ViewHex` 256KB 上限是补丁而非根治 | 解析库与 UI 彻底分离（已是 crate 结构）；hex/列表 IPC 全部分页（后端 API 层面根治） |
| P5 | 导出全串行 | `CliRunner.RunExportModels` 单线程循环 | `rayon` 按资源并行导出；DXT 按块并行 |

**测试策略（贯穿全程）**：与 C# 版做**差分测试**——用源项目已构建的 CLI（`SimCityPak.exe export-*`）在真实包（`Oppie_OFFLINE_CentralTrainStation.package`、`SimCity_DLC0`、`SimCityDataEP1`）上生成金样本，Rust 实现逐字节/逐字段比对。每个 crate 带 criterion 基准。

---

## 2. 模块映射（C# → crate）

| C# 源（Simcitypak-v2） | Rust 落点 | 说明 |
|---|---|---|
| `SimCityPak.Packages/`（DataBasePackedFile/Index/IndexData、StreamHelpers.RefPackDecompress） | `crates/dbpf` | 容器格式 + RefPack + mmap 访问层 |
| `Gibbed.Spore.Properties`（PropertyFile）+ `CliRunner.DumpProp/ExportCombinedPropsToFolder` + `TGIRegistry`（.s3db）+ `LocaleRegistry` | `crates/sc-properties` | 属性解析、hash→名称（rusqlite 读 database_main.s3db）、locale 名称映射、combine 聚合 |
| `SimCityPak/RenderWare4/`（RW4Model、RW4Mesh、Texture、Skeleton、Anim、Matrices…约 20 个类） | `crates/rw4` | section 树解析、顶点格式、材质槽、DXT 解码 |
| `RenderWare4/Exporters/`（WaveFrontOBJConverter、GltfConverter）+ `CliRunner` 各 export 命令 | `crates/sc-exporter` | glTF 用成熟库序列化（非手写 chunk）；OBJ/DDS/PNG/TXT/JSON |
| `MainWindow.xaml` + `Views/`（ViewRW4/ViewTexture/ViewHex/ViewVideo…） | `src/`（Vue）+ `src-tauri` commands | 页面开发直接基于 fluffy-design-pro 壳 |
| （无 C# 对应，新增）应用遥测/活动记录 | `crates/sc-store`（rusqlite） | 埋点持久化 + 异步消息存档，见 M4.5 |
| `Tools/vgmstream/`、`Tools/ffmpeg/` 调度逻辑 | `src-tauri`（sidecar/外部进程） | 沿用外部工具方案，不重写编解码 |
| `database_main.s3db` / `database_user.s3db` | 随应用分发 + `rusqlite` 只读 | TGI/属性名描述符库 |

---

## 3. 里程碑

### M0 ✅ 脚手架（已完成，2026-09-04）
workspace + 4 个空 crate 骨架 + fluffy-design-pro 应用壳 + Tauri 2 壳 + 图标占位 + README。构建/测试全绿。

### M1 — `dbpf`：DBPF + RefPack（P0 底座，已完成）
**C# 参考**：`SimCityPak.Packages/DataBasePackedFile.cs`、`DataBaseIndex.cs`、`DataBaseIndexData.cs`、`StreamHelpers.cs`

任务：
- [x] DBPF/DBBF 头解析（magic `0x46504244`/`0x46424244`，`Always3==3` 校验，含 DBBF 120 字节大头变体）
- [x] 索引解析：`indexHeaderValues` 位标志（bit0 typeId / bit1 group / bit2 unknown，4~7 值合法），逐条目 TGI + offset/size/compressed 标志（2026-09-04）
- [x] RefPack 解压：输出按索引中 `DecompressedSize` 预分配；命令分支/重叠拷贝/多流头与长尺寸头均对齐 C# `StreamHelpers.RefPackDecompress`（含 10 个手编码向量单测）
- [x] `Package` 访问 API：`open(path)` → mmap 常驻；`entries()` 只读条目数组；`read_raw` 零拷贝切片；`read` 惰性解压；`CachedPackage`（LRU，Mutex 版可跨线程）
- [x] 错误处理：`thiserror` 错误枚举，区分「不是包/保留字段损坏/索引头非法/压缩标志非法/截断/越界/解压失败」
- [x] 真实数据验证（2026-09-04，`docs/packages/app.package`，SimCity_App 316MB / DBPF 3.0 / 2489 条目）：索引解析 0.9ms；全量扫读 2489 个资源（1260 条真实 RefPack 流）全部解压到精确声明大小，总计 481,477,425 字节无差错；release 全量解压吞吐 ≈860 MB/s（目标 ≥300）。附带 `cargo run -p dbpf --example inspect -- <包>` 诊断工具与文件缺失自动跳过的真实包测试
- [ ] criterion 基准正式化（当前有 inspect + 测试内计时；可选）与 C# GetIndexData 输出逐字节比对（可选，需构建 C# CLI）

**验收**：✅ 打开大包索引 < 100ms（实测 0.9ms）；✅ 资源读取正确性（全量扫读零失败）；✅ 任意资源读取常量内存（mmap 切片）。9 个合成 fixture 集成测试 + 10 个 RefPack 向量 + 2 个真实包测试 + doctest 全绿，clippy 无警告

### M2 — `sc-properties`：属性表 + 名称解析
**C# 参考**：`CliRunner.cs` 的 DumpProp/combine 部分、`TGIRegistry`、`NameRegistry`、`LocaleRegistry`；HANDOFF §4 export-prop 全部小节

任务：
- [x] 属性值模型：Float/Bool/Key(TGI)/Vector2/3/4/Color/BoundingBox/Transform/String8/16 + 数组（2026-09-04）。全大端格式、flags 位语义（0x30 标量/变体、0x40 数组/空、0x100 String8 短长度）、Transform 12/3/4 矩阵计数与 flags==15 前置 float 均对齐 C# 实现；`Property::keys()` 支持引用遍历（后续 combine 用）
- [x] 真实数据验证（app.package 249 个属性表全量解析 3ms，2116 属性，14 种类型全部出现；非空 Key 引用 97% 命中包索引类型，确认 instance/type/group 字段顺序正确）
- [x] combine 聚合（2026-09-04）：同 InstanceId union + Model Details（`0x0975695F`）引用 union（数组与标量 Key 均支持），刻意不加其他 hash；app.package 实测 249 条目 → 122 组（39 个多成员组，最大 21，全部由实例共享解释）
- [x] `database_main.s3db` 名称解析 → **新 crate `sc-registry`**（rusqlite bundled，只读；6 张表 id/name/comments，main+user 覆盖语义对齐 C# `TGITable.LoadCache`；真实库验证：11321 实例 / 464 属性描述符，`0x0975695F` = "Model Details (PROP)" 确认在库）
- [x] locale 名称映射（2026-09-04）：`Locale`（type `0x0A98EAF0` JSON 字符串表，BOM 跳过、`//` 注释键）+ `collect_name_map`（名称属性 5 hash → Array[0] Text → locale 解析 → prop 实例 + 引用的 RW4 模型实例传播）
- [x] 差分测试协议层（2026-09-04）：`sc-exporter/prop_json` 与 C# `DumpProp(json)` 逐字段对齐（`name/hash/type/value`、按 hash 排序、空变体排除、Bool→`True/False`、Key/Text→空串、UInt32→小写 hex、数组→带前导空格拼接、类型名去 `Property` 后缀；自实现 .NET Framework G7 浮点格式化）。`export-prop` CLI 端到端 249/249 资源成功；契约测试 11 个 + 真实包不变量测试全绿；oracle 门控差分测试（`OPEN_SCP_CSHARP_EXPORT_PROP`）未配置时显式跳过——本机无 MSBuild/.NET SDK 无法构建 C# CLI，跨语言比对待提供 oracle 后运行

**M2 状态：解析与协议层完成；C# 真机差分依赖外部 oracle（环境门控已就绪）。**

### M2.1 — Property Editor 后端基础（2026-09-06）

- [x] `sc-properties` 保存 Property resource 的 `flags` 与数组 `item_size`，增加 `PropType` 文件 type id 反向映射
- [x] 增加受限解析入口 `PropertyFile::parse_with_limits`，防止异常 entry/array/string 计数造成无界工作量
- [x] 增加确定性的全大端 `PropertyFile::encode_canonical`，覆盖标量/数组/Empty、15 种值类型、String8/16 和 Transform；明确语义 round-trip，不承诺原始字节保真
- [x] 增加 `LotEditorDocument`，提取 LOD1 模型、LotMask、Lot 尺寸、摆放变换，同时保留未知属性
- [x] Tauri 增加 `read_lot_editor_session` 和受限 `patch_property_overlay`：仅修改已有 Property、保持类型/形态/数组长度，生成独立未压缩 DBPF overlay；输出使用临时文件 + sync + rename，禁止覆盖源包
- [x] 单测覆盖 Property 编码 metadata round-trip、非法类型/item size、解析上限、patch hash/shape 约束和领域文档保留未知属性

**M2.1 性能报告**：`cargo test --workspace` debug 构建总耗时约 23.0s（含编译）；Property 单元测试 19 项耗时 <0.01s；Tauri 后端 35 项耗时 0.04s；DBPF 真实包测试 3.15s。workspace 全量测试通过。

### M3 — `rw4`：RenderWare4 解析（P0 底座，✅ 全部完成 2026-09-05）
**C# 参考**：`SimCityPak/RenderWare4/` 全目录；HANDOFF §4 export-gltf 各 done 小节（含全部格式结论）

任务（顺序即依赖顺序）：
- [x] section 索引解析（2026-09-04，`crates/rw4`）：真实格式为**扁平 section index**（非树）——28B magic、Model/Texture 文件类型、头部固定块（H001–H181 全部 expect 校验）、类型表（前 5 项固定 + H300/H301/H302 一致性检查）、24B entry（Blob pos 以 index end 重定位）+ fixup 对。合成 fixture 单测 16 个（合法构建器 + 9 类损坏用例 + 截断 no-panic）全绿；真实包 sweep：app.package 538 个 `0x2F4E681B` 资源 1.4s 全部通过（526 Texture + 4 Model 容器；8 个 `0xCAFED00D` 占位桩与 C# oracle 同样失败，显式归类）
- [x] 顶点/网格解码（2026-09-04）：`vertex.rs`（D3DDECLTYPE/USAGE 全枚举、VertexFormat **混合字节序**解析：LE 计数 + BE stride/unknown、12B 元素表、9 种组件解码，不支持类型按 C# 工厂语义跳过）+ `mesh.rs`（Mesh 10×u32 ME001–ME006、TriangleArray TA000–TA010（u16×3 索引）、VertexArray VA000–VA102、`decode_mesh` 跨 section 组装、`vertex_section==0x400000` blend-shape 特例、BLENDINDICES ÷3 + joint_count 钳位（`blend_indices` 保留 `blend_indices_raw`）、NORMAL UBYTE4 解包、UV FLOAT2 优先 FLOAT4 回退（`TryGetUV` 语义））。合成端到端 6 测 + 声明元数据测试全绿
- [x] M3 收尾三包 sweep（`tests/m3_packages.rs`，样本 `docs/packages` + `docs/packages/m3`）：app 4 / DLC0 565 / EP1 **1025** 个 mesh 全量解码成功（EP1 数与 C# HANDOFF 记录的 1025 模型精确吻合）；192 万三角形 / 333 万顶点、DLC0+EP1 UV 覆盖 100%、位置全部有限。mesh 头部 ME0xx 不合规 section（DLC0 6 / EP1 3678）经核对 C# `RW4Model.Read` 为 `try{GetObject}catch{break}` **静默跳过**——Rust 侧同样归类为 oracle 对齐失败不阻断；TA/VA/VF/payload 类错误零容忍。下一步：材质槽位 → DXT/Texture → Skeleton/Anim
- [x] 顶点数组/三角数组：语义解析（POSITION/NORMAL/TEXCOORD FLOAT2 优先 FLOAT4 兜底/BLENDINDICES/BLENDWEIGHT），**BLENDINDICES ÷3**（关节索引 ×3 存储）——已随上行完成（见 `vertex.rs`/`mesh.rs`）

**M3 性能报告（最终）**（`cargo test -p rw4 --test m3_packages -- --nocapture`，debug 构建，含 mmap 读取 + RefPack 解压 + header/section + mesh + 材质 + 贴图顶层 mip + 骨骼 + 动画全解码）：

| 包 | RW4 资源 | 耗时 | mesh(可导出) | 三角形 | 顶点 | 材质 | 骨骼/关节 | 动画/通道/关键帧 | 贴图/像素 | DXT 吞吐 | oracle 对齐失败 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| app | 538 | ~1.9s | 4 (4) | 8,287 | 8,812 | 2+2 raw | 0/0 | 0/0/0 | 238 / 1,391 万 | 27.2 Mpx/s | mesh 0 / 贴图 26（T001）+ 8 占位桩 |
| DLC0 | 785 | ~4.6s | 565 (565) | 673,948 | 1,222,534 | 571 | 256/2,656 | 502/5,997/27,170 | 912 / 3,654 万 | 24.0 Mpx/s | mesh 6 |
| EP1 | 3,758 | ~11.4s | 1,025 (1,025) | 1,243,411 | 2,099,140 | 4,703 | 2,103/21,945 | 5,379/64,050/168,288 | 3,835 / 2,662 万 | 25.8 Mpx/s | mesh 3,678 |

合计三包 5,081 资源 / ~18s：**约 192.6 万三角形、333 万顶点、7,710 万贴图像素（RGBA8）、2,359 骨骼 / 24,601 关节、5,881 动画 / 70,047 通道 / 195,458 关键帧**全量解码，骨骼/动画零失败（debug 单线程；release + rayon 并行还有数倍空间）。辅助数据点：app.package 249 个属性表 JSON 转储 13ms（M2，含 registry 名称解析）。

- [x] RW4Material 槽位模型（2026-09-05，`material.rs`）：`Size` + 28B 头 + （模型含 VertexFormat 时）`24+12n` 顶点格式副本 + 扫描 `0x2D` shader-def 标记（不越 section 末尾，C# 防挂起）+ AdditionalData + 固定 6×24B 纹理引用（原始 slot u32 保留 + `slot_byte()` 低字节语义）+ Data；不可解析布局整体回退 `Raw`（C# `_rawSection` 行为）。真实包：DLC0 571/571、EP1 4703/4703 解码成功，槽值直方图显示 slot 0（23351）/4（每材质 1 条）为主，另有 0x100/0x400/0x3F800000 等标志值
- [x] Texture：DXT1/DXT5 纯 Rust CPU 解码（标准 4×4 块算法，无 GPU/XNA 依赖）+ pixFmt 21 raw BGRA→RGBA + textureType 116 调色板条识别 + DDS 直写（与 C# `SaveDds` 字节级同构，含其不写 caps 的怪癖）。真实包 app+DLC0+EP1 共 4,985 张贴图 / 7,710 万像素解码零失败；app 有 textureType 50（未知格式）若干，结构化报 `UnsupportedTextureType`；T001 严格 expect 失败（app 26）为 oracle 对齐（C# Texture 分支无 try/catch 同样抛）
- [x] Skeleton/Anim（2026-09-05，`skeleton.rs`/`anim.rs`）：Skeleton SK000–SK001（expect 0x400000 + 三引用）；HierarchyInfo HI000–HI020（三段指针**绝对文件偏移**重定位——真实数据发现，C# 逐文件流 seek 语义）；Matrices MS000–MS-count（数量防挂起护栏 + 绝对偏移兼容）；mat4 双类型码（0x7000B 枚举 / 0x70003 类常量，真实资源用后者）。Anim AN000/AN-channels/AN-names/AN-info（4096 通道上限 + 越界快失败）；关键帧 0x101 LocRot(36B)/0x601 LocRotScale(48B)/0x100 BlendFactor(8B)，stride 对齐补齐修正 C# `ReadKey` 32B 消费的逐 key 漂移；末通道时间回退截断。真实包：**2,359 骨骼 / 24,601 关节 / 5,881 动画 / 70,047 通道 / 195,458 关键帧全部解码零失败**（修复前 anim 仅 2,132 keys → 修复后 195,458）
- [x] 数学库（2026-09-05，`math.rs`）：列主序 Mat4 + `mat4_mul`/`mat4_inverse`（退化行列式回退单位阵，与 C# 一致）/`mat4_decompose_trs`（TRS → 平移/四元数 xyzw/缩放），逐位移植 C# `GltfConverter` 计算路径；Z-up→Y-up 旋转留在导出层
- [x] NRE 防御对齐（2026-09-05）：`DecodedMesh::is_exportable()`（null vertices/triangles 过滤，C# `ExportRw4Bytes` 语义）；blend-shape `0x400000` 哨兵 → 空顶点；无 FLOAT4/FLOAT2 texcoords 的 mesh 不再抛错（`uv()` 返回 Option）。真实包验证：DLC0 565/565、EP1 1025/1025 全部可导出
- [x] shell-rig 标记（2026-09-05）：`DecodedVertex::shell_marker()`（TEXCOORD index 1 UBYTE4）+ `is_rigid_shell_static()`（`(127,127,127,0)`=静态外壳）；分组质心→最近骨骼 pivot 的皮肤构建属导出层（M4）

**M3 验收**：DLC0 + EP1 全量模型解析成功率 ≥ C# 版（6 个 oracle 对齐失败除外）——✅ 已达成（565/565、1025/1025 可导出）；金样本字段级比对待 C# oracle（环境门控就绪）。

### M4 — `sc-exporter`：导出管线（进行中：核心格式、批处理和可选材质嵌入已完成；跨包解析尚未接入端到端导出；M4.5/M5 未开始）
任务：
- [x] glTF/GLB（2026-09-05，`gltf.rs`）：serde_json 生成 JSON + GLB 容器（JSON 块 0x20 填充/BIN 块 0x00 填充，与 C# `WriteGlb` 字节布局一致）；几何（POSITION min/max/NORMAL/TEXCOORD_0 V 取反/FLOAT4 ≤8 门控/退化面剔除）+ 皮肤/动画/shell-rig。保留向后兼容 `export_glb` API；PNG/JPG/TGA/DDS texture API 已补齐。
- [x] OBJ（2026-09-05，`obj.rs`）：对齐 C# `WaveFrontOBJConverter.Export`；DDS 直写及 PNG/JPG/TGA 编码 API/CLI 已补齐。
- [x] prop JSON/文本（2026-09-05）：`export-prop --format json|text`，默认 JSON 保持兼容；新增稳定文本 formatter。
- [x] rayon 批量导出进度模型（2026-09-05）：`export_batch` 按资源并行并回调完成数/总数；回调仅提供 crate 内进度模型，Tauri event 桥接属于 M5。材质跨包查找提供 `TextureIndex` 与冲突诊断 API，但尚未接入完整的“材质槽 → sibling package 纹理读取 → glTF 材质”端到端导出，故不作为完成项。
- [x] 金样本差分（2026-09-05，`tests/export_model.rs`）：**ec3eade0 真实模型断言通过**——1 skin + 17 关节 + 1 动画 + 34 通道（17×T+R）+ 时间 0..~1.83s，与 HANDOFF §4 记录完全一致（无 C# oracle 亦验证）；另含 GLB 容器字节布局/静态网格/蒙皮/shell-rig 合成测试 5 项

**M4 性能报告（GLB 全量导出）**（`cargo test -p sc-exporter --test export_model perf_export -- --nocapture`，debug 构建，含 DBPF 读取 + RefPack 解压 + RW4 全解码 + 皮肤构建 + GLB 序列化）：

| 包 | 可导出 mesh | 顶点 | 三角形 | GLB 输出 | 耗时 | 吞吐 |
|---|---|---|---|---|---|---|
| app | 4 | 8,812 | 8,287 | 4 个 / 376 KB | 1.44s | — |
| DLC0 | 565 | 1,222,534 | 673,948 | 565 个 / 60.7 MB | 4.36s | 130 mesh/s |
| EP1 | 1,025 | 2,099,140 | 1,243,411 | 1,025 个 / 108.5 MB | 11.79s | 87 mesh/s |

合计三包 1,594 个 GLB / **169.6 MB** / ~17.6s（debug 单线程；皮肤/动画全量构建含在内）。

### M4.2 — 二进制魔数识别（撞库）+ 格式对照表（2026-09-05 新增）

**动机**：前端遇到 unknown 资源时，调用后端**非阻塞**二进制识别（`sc-exporter::magic::detect_magic`，纯内存前缀匹配、无 IO 无分配，Tauri command 可直接调用）对未知文件魔数撞库：命中 → 提示「检测到 .xx 文件，暂不支持预览」；未命中 → 按原未知文件处理。

**撞库 API**：`detect_magic(bytes: &[u8]) -> Option<MagicFormat>`（`hex/ascii/extension/format/usage` 五元组）。规则库 `MAGIC_FORMATS` 覆盖通用游戏魔数（ZIP/GZIP/Zlib/PNG/JPEG/DDS/RIFF→WAV|WebP|AVI/OggS/ICO/XML/7z/RAR/BMP/TTF/OTF/SQLite/UnityFS）+ SCP 生态（DBPF `.package`、s3db）。规则按前缀长度降序取最长命中；RIFF 按 offset 8..12 细分。

**原 SCP 支持的解析/预览清单**（权威来源：`database_main.s3db` FileTypes 表 25 条 + ViewSelector 硬编码特例；viewer 字段驱动路由）：

| viewer | typeId | 名称 | 原 SCP 能力 | 本项目迁移状态 |
|---|---|---|---|---|
| viewRW4 | `0x2F4E681B` | RW4 File | section 树/网格/材质/贴图浏览 | ✅ M3 全解码 + M4 OBJ/GLB 导出 |
| viewRaster | `0x2F4E681C` | Raster File | DXT 解码预览 | ✅ M3 decode_texture / M4 PNG/JPG/TGA/DDS 导出 |
| viewPropertyFile | `0x00B1B104` | Property File | 属性表查看/编辑 | ✅ M2 解析 + JSON/TXT 导出（编辑器属 P1+） |
| viewPng | `0x2F7D0004`/`0x2F7D0007`/`0x3F8662EA`/`0x02393756` | PNG/GIF/JPG/Cursor | 图片预览 | ⏳ 前端通用 `<img>` 即可；Cursor 走 ICO 魔数 |
| viewTga | `0x2F7D0006` | TGA File | TGA 预览 | ⏳ 同上（导出已支持） |
| viewRawImage | `0x03E421EC/ED/F0` | Greyscale Map 8/16/32-bit | 裸灰度图预览 | ❌ P2 |
| viewJavascript | `0x0A98EAF0`/`0x67771F5C`/`0x0A4D8D09` | JSON/JS/BKHD | 语法高亮查看 | ✅ locale JSON 已解析（M2）；前端 Shiki 覆盖 |
| viewText | HTML/C++/CSS/未知 | `0x-229DCC2A` 等 | 文本查看（无高亮） | ⏳ 前端 Shiki 统一解决 |
| viewData | `0x0D9E5710`(WAV)/TTF/ER2/Effects Dir 等 7 条 | 二进制数据 | hex/data 查看 | ✅ 等价于 hex 视图（M5）+ 魔数识别 |
| （硬编码）ViewVideo | `0x376840D7` | EA VP6 视频 | 视频信息 + 导出 MP4 | ❌ M5（ffmpeg sidecar） |
| （硬编码）Advanced | instance `0xB185/0x1651/0x1652`/`0x8B7E` | DecalAtlas/Path | 高级编辑器 | ❌ P2 |
| （兜底）ViewHex/HexDiff | 其余全部 | 未知二进制 | hex 查看（256KB 上限补丁） | ✅ M5 分页 hex（根治 OOM） |

**扩展名图标**：`.docs/assets/file-exptand/`（33 个 SVG + `index.html` 预览页），覆盖上述全部格式 + 通用魔数表扩展名。

**一般游戏文件魔数匹配方向**（撞库规则库即按此表实现）：

| 文件头（Hex） | ASCII 字符 | 极有可能的格式 | 常见游戏用途 |
|---|---|---|---|
| 50 4B 03 04 | PK.. | ZIP（或 Unity .assetbundle） | Unity 游戏资源包、Android 数据包 |
| 1F 8B | (不可见) | GZIP | 压缩的纹理或关卡数据 |
| 78 9C / 78 DA | (不可见) | Zlib（Deflate 压缩） | 很多引擎的通用压缩块 |
| 89 50 4E 47 | .PNG | PNG 图片 | 贴图、UI 图标 |
| FF D8 FF | (不可见) | JPEG 图片 | 过场 CG 或高质量背景 |
| 44 44 53 20 | DDS | DDS 纹理（DirectDraw Surface） | 直接驻留显存的显卡纹理格式 |
| 52 49 46 46 | RIFF | WAV 音频 或 WebP 图片 | 音效或图片 |
| 4F 67 67 53 | OggS | OGG 音频（Vorbis/Opus） | 背景音乐、语音包 |
| 00 00 01 00 | (不可见) | ICO 图标 或 Cursor 光标 | 游戏内鼠标指针 |
| 3C 3F 78 6D | <?xm | XML 文本（明文配置） | 剧情文本、数值表 |
| 44 42 50 46 | DBPF | EA DBPF 容器 | SimCity/EA `.package`（本项目主格式） |
| 53 51 4C 69 74 65 20 66 6F 72 6D 61 74 20 33 | SQLite format 3 | SQLite 3 数据库 | SCP 描述符库 s3db |
| 55 6E 69 74 79 46 53 | UnityFS | Unity Asset Bundle | Unity 引擎资源包 |
| 37 7A BC AF 27 1C / 52 61 72 21 | 7z.. / Rar! | 7-Zip / RAR | 模组分发压缩包 |
| 00 01 00 00 / 4F 54 54 4F | (不可见) / OTTO | TrueType / OpenType 字体 | 游戏 UI 字体 |
| 42 4D | BM | Windows 位图 | 简单贴图 |

### M4.5 — `sc-store`：SQLite 遥测存储（新增，2026-09-04 规划）

应用级 SQLite 数据库（rusqlite + `bundled` 特性，编译期自带 SQLite 零系统依赖），为扫描/解压/导出等操作提供埋点持久化与异步消息存档。

- [x] `crates/sc-store`：应用库 `openscp.db`（Tauri app 数据目录），`PRAGMA user_version` 迁移
  - `operations` 表：kind（package_open/package_scan/resource_read/export…）、target、status、duration_ms、bytes_in（压缩）/bytes_out（解压）、detail JSON
  - `events` 表：level（info/warn/error）、topic、message、payload JSON——后台任务事件一边实时推前端（Tauri event），一边落库可回看
  - `packages` 表：打开过的包历史（路径/大小/条目数/版本/打开次数）
- [x] 埋点位置：**src-tauri 服务层**包住 dbpf 调用（打开/扫描/读资源/导出），`dbpf` crate 保持纯净零副作用；需要更细粒度时给 dbpf 加可选 stats 返回
- [x] commands：`activity_list_operations/list_events/list_packages`、`activity_clear`；实时通道 `activity:event`
- [ ] 前端「活动记录」页（详见 ui.md 工作区分组）：历史过滤表格 + 实时事件流面板

**M4.5 后端进度（2026-09-05）**：已完成 SQLite 存储、迁移、活动查询/清理 IPC、包操作埋点和实时事件发布基础；`sc-store` 测试 4 项通过，单独测试耗时约 1.77s（含增量编译）。

**注意**：rusqlite 为同步 API，重操作（全包扫描）用 `spawn_blocking`；sc-store 依赖 serde 反序列化查询结果。

### M5 — Tauri commands + 前端页面（P1 Modding Suite UI）
任务：
- [x] 后端 commands：`open_package`（返回首批分页 TGI 树）、`close_package`、`list_resources(offset,limit,filter)`、`read_resource_bytes(range)`、`resolve_name(tgi)`、异步 `export`（raw/OBJ/GLB/PNG/JPG/TGA/DDS + 进度 event）、`export_status(job_id)`
- [x] 后端 **OOM 防线（API 层）**：资源列表分页上限 1000；hex 单次最多 4KB；压缩资源声明解压上限 256 MiB；纹理像素/Blob 预算校验；大操作使用 `spawn_blocking`
- [ ] 页面：包打开向导 → TGI 树（FTree）→ 资源详情（hex 分页 / 文本 Shiki / 贴图预览 / 模型 3D 预览评估 `three.js` / 视频内嵌 `<video>` + ffmpeg 转码临时文件 / 音频播放）
- [x] 运行时媒体工具探测与导出接线：Wwise Vorbis → WAV（vgmstream bundled 优先、PATH 回退）、EA VP6 → VP6/MP4（ffmpeg bundled 优先、PATH 回退），固定参数数组、严格 RIFF/MP4 结构校验、超时、并发限制、临时文件清理；BKHD SoundBank 可解析 BKHD/DIDX/DATA 并按 media id 提取 WEM
- [x] 通用媒体导出：图片资源支持 PNG/JPG/GIF；Wwise/BKHD 音频支持 WAV/MP3/OGG/FLAC，替代格式采用 vgmstream 解码 WAV 后由 ffmpeg 转码
- [x] 运行时可靠性：Windows 媒体子进程使用 `CREATE_NO_WINDOW`；转码失败通过 job 状态、activity 和稳定错误码返回，不导致应用退出
- [ ] vgmstream/ffmpeg 二进制随应用分发、Tauri bundle/externalBin、许可证和 x64/arm64 发布验证（留至 M6）
- [ ] chat-assistant 网关：**未来功能，本阶段不建设；前端开发阶段暂时禁用**

**M5 媒体性能/稳定性报告（2026-09-06）**：媒体后端单元测试 40 项耗时约 5.08s（含增量编译）；BKHD 合成 chunk 解析与 WEM 提取在微秒级完成；外部媒体进程并发上限为 2，单任务超时为 10 分钟。已在真实资源上运行应用内部导出路径：Wwise→WAV 输出 5,915,792 bytes，约 99.8ms；VP6→H.264 MP4 输出 1,741,310 bytes，约 1.72s。当前工具来自用户 PATH 的 WinGet 安装目录，真实转码集成测试通过；发布 bundle 仍未携带媒体二进制。

- [x] 前端首版源文件解析工作区：目录树、当前目录 package 列表、多个 package Tab、资源分类 Tabs、默认空态和 5174 Mock 验证
- [x] 前端媒体预览与导出 toolbar：图片 PNG/JPG/GIF，音频 WAV/MP3/OGG/FLAC；默认格式、保存对话框、导出状态和失败 toast 已接入
- [ ] Tauri 真实目录扫描 command 与完整前端联调

### M5.5 — 开发者文档工作区（后端，新增，2026-09-05）

与 SCP/DBPF/RW4 解析无关，为未来 P1 模组开发者提供按模组、资产和组件文件夹组织的 Markdown/README 文档能力。一个资产可通过多级文件夹拆分为多个组件，各文件夹拥有自己的 `README.md`。

- [x] `sc-store` schema v2：在 `openscp.db` 中持久化用户选择的工作区根目录，以及文件夹到 README 的相对路径缓存；正文不存入 SQLite
- [x] Tauri workspace commands：`workspace_get`、`workspace_set_root`、`workspace_list`、`workspace_create_folder`、`workspace_read_markdown`、`workspace_write_markdown`、`workspace_create_markdown`
- [x] 安全边界：仅允许工作区内相对路径；拒绝 `..`、绝对路径、盘符/UNC、NUL、符号链接和越界路径；Markdown 单文件上限 4 MiB；写入采用临时文件 + sync + 原子替换；revision 使用 SHA-256 防止并发覆盖
- [ ] 前端 Notion 风格目录树、Markdown 编辑/预览、首次启动工作区选择向导
- [ ] 富文本协同、多用户编辑、资产数据库实体和组件关系图
- [ ] 程序 MSI/NSIS 安装目录选择：M5.5 只选择用户文档工作区，不修改程序安装位置
- [ ] chat-assistant：未来功能，当前后端不建设，前端接入阶段暂时禁用

**M5.5 后端进度（2026-09-05）**：已完成工作区配置持久化、文件夹/README 关系缓存、Markdown 创建/读取/写入、revision 冲突保护和路径安全校验。`sc-store` 6 项测试及 workspace 后端编译验证通过；前端编辑器与 chat-assistant 保持未建设。

### M6 — 发布
- [x] 后端应用设置与游戏目录探测：持久化 `game_data_path`（`sc-store` schema v3）、canonical 路径校验、默认候选 `C:\Games\SimCity\SimCityData` 和直接 `.package` marker 检查
- [ ] 前端首启向导：选择游戏数据目录并展示探测结果；设置页持久化接入
- [ ] `pnpm tauri icon` 正式图标；MSI/NSIS 打包
- [ ] bundled vgmstream/ffmpeg 二进制随应用分发、许可证核查、x64/arm64 发布验证
- [ ] chat-assistant：未来功能，当前后端不建设，前端接入阶段暂时禁用

**M6 后端进度（2026-09-05）**：已完成应用设置持久化、游戏目录候选探测和安全路径校验；`sc-store` 7 项测试、Tauri 后端 21 项测试及 workspace 全量测试通过。后端定向测试耗时约 2.69s（含增量编译）。sidecar 二进制分发、安装器、前端首启向导和 chat-assistant 不在当前后端范围内。

---

## 4. 性能专项清单（关键环节指标）

| 环节 | 指标 | 手段 |
|---|---|---|
| 打开大包（EP1 391MB） | 索引 < 100ms | mmap + 批量索引解析（P1/P2） |
| RefPack 解压 | ≥ 300 MB/s/线程，输出零重分配 | 预分配 + 紧凑循环；差分测试保证正确性 |
| DXT 贴图解码 | 1024² DXT5 < 10ms | rayon 按块并行，无 GPU |
| 全包导出（export-all） | 接近核数线性 | rayon per-resource，纹理查找表复用 |
| hex/大文本查看 | 恒定内存 + <16ms 首屏 | 分页 IPC，前端虚拟滚动 |
| 前端 TGI 树（10万+ 条目） | 滚动 60fps | 后端分页 + 前端虚拟化，树按 group 惰性展开 |

基准落在各 crate 的 `benches/`（criterion），CI 可选 `cargo bench` 对比。

---

## 5. 风险与已知坑（实现前必读 HANDOFF 对应小节）

1. **材质/颜色不可完全复原**：建筑无烘焙 albedo，颜色在 GlassBox shader 中合成；导出按「亮度灰度 facade」方案（已验证），勿再尝试 RGB 合成。
2. **facade 建筑 UV**：只有 FLOAT4 世界投影坐标，超范围（>8）禁用，否则贴图拉花。
3. **prop combine 边界**：仅 `0x0975695f` + 同 InstanceId；再加 hash 会产生跨家族 blob。
4. **0x2001a 链路是 Spore 遗留**，SimCity 不用，保持禁用。
5. **共享图集是正常的**：多模型共享区域/法线图集（EP1 有 18 个模型共享同一对），不是 bug。
6. **blend 索引 ÷3** 忘了做会导致蒙皮全塌到根骨骼。
7. **DBBF 大包变体**与压缩标志语义要在差分测试中覆盖。
8. XNA 数学是行/列主序混合约定——移植 Matrices.cs 时以 glTF 列主序 + xyzw 四元数为准，逐一验证。

---

## 6. 建议执行顺序与当前状态

P0 底座先行，P1 工作流随后接入：

```text
P0: M1 dbpf ✅ → M2 sc-properties ✅ → M3 rw4 → DBPF/property writeback + OBJ importer
                                                    ↓
P1: sc-modding manifest → validate → preview/watch → deterministic overlay build → UI
                                                    ↓
P2/P3: 高级编辑器、游戏联动、依赖生态
```

当前：**M4 导出实现已落地，正在补强契约验收：OBJ/glTF/GLB、PNG/JPG/TGA/DDS、PNG 材质嵌入、rayon 批量导出与进度/部分失败汇总均已实现；`TextureIndex` 已有确定性优先级与冲突诊断，但材质跨 package 解析尚未形成端到端 glTF 材质导出，不能宣称已完成。M4.2 魔数撞库识别已就绪（`magic::detect_magic` 非阻塞 + 33 个格式图标）。Tauri event 桥接属于 M5；M4.5/M5 尚未开始。**

**MVP 交付定义**：最小可交付产物 = 前后端 P0 阶段打通后的**可运行 EXE**（原 SCP 核心能力的 Rust + Tauri 迁移版）。达成条件：M3 完成 + DBPF/property 只读链路经 Tauri command 暴露 + 前端资源工作台/属性/资产页面可用（见 `docs/roadmap/ui.md` U0–U2）+ 异常隔离验收通过 + `pnpm tauri build` 出包。

P1 的 manifest、pipeline、diagnostics、dependency、preview/watch 接口和验收标准见 `docs/roadmap/modding-suite.md`。

---

## 7. P0 UI 收尾记录（2026-09-06）

本轮完成 UI 十项修正 + Package 三列 IDE 工作台（详见 `docs/roadmap/ui.md` U1/U2 进展记录）。后端同步落地：

- `package_browser`：目录树（含文件）扫描 / `.package` 列表；symlink 跳过、深度/数量上限、稳定排序
- `resolve_names`：按 TGI 批量语义名称，registry s3db 就近发现（package 目录与游戏目录向上 3 级），失败回退 TGI
- `workspace_rename` / `workspace_move`：root 校验、单一组件名、防覆盖、防移入自身子树，操作后刷新 folder cache
- Tauri dialog plugin（原生目录选择）；窗口恢复 resizable/maximizable（min 960×600）

验证与性能：

- 前端：`vue-tsc` / `eslint` / `vitest` 50 通过 / `prettier` 全绿；后端：`cargo test` 26 通过、`clippy -D warnings` 零告警
- 真实 `SimCity_DLC0.package`（93.4 MB）：DBPF 3.0，1806 条目，1536 RefPack；索引解析 ~0.7 ms；分类 643 Property / 785 RW4 / 55 Raster
- 5174 浏览器 Playwright 实测：三列布局拖拽、文件树展开与 `.package` 打开、分类 tab 切换、Hex/图片预览与缩放旋转、文档树右键全流程（创建/重命名/移动/根级创建）、Markdown Enter 换行不跳页
- 产物：`target/release/fluffy-open-scp.exe` + MSI + NSIS 安装包（内嵌 dist，不依赖 dev server）

已知边界：Registry s3db 未随包分发，用户游戏目录缺失时语义名整体回退 TGI；Tauri Raster/音视频/GLB 预览待后续 command；属性/资产 inspector 为下一批。

---

## 8. 文档管理与源文件解析缺陷修复记录（2026-09-06）

用户 EXE 手测报告 6 缺陷，本轮全部修复：

文档管理（`workspace.rs`）：

- 创建文件夹不再自动写入 README.md（空文件夹即合法条目）
- 列表模型重构：`WorkspaceFolder(仅 README)` → `WorkspaceEntry{relativePath, kind}`，递归上报**全部 .md 文件**（非 README 改名文件可见可编辑）；根目录级 .md 允许创建（`existing_parent` 允许空 parent）；前端文档树改为按 parent 映射递归渲染文件节点
- 保存 os error 32 根因：`ReplaceFileW` 要求目标 DELETE 权，索引器/杀软/同步盘以"无 FILE_SHARE_DELETE"句柄持锁即失败。改为 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` + 共享冲突退避重试（25/50/100/200ms）

源文件解析（`package_service.rs` + 前端）：

- `ResourcePage` 新增全量 `typeCounts`（按 TypeID 聚合整包条目，类型名走 registry FileTypes 表，缺失回退 8 位 hex）；`list_resources` 新增 `typeId` 服务端过滤参数
- 类型 tab 改为按全量 typeCounts 渲染（此前只统计首页 100 条且仅 5 个硬编码 TypeID）；app.package 实测 17 类型（与原 SCP 可识别类型数吻合）
- 命名对齐原 SCP：类型列显示 s3db 短类型名，资源名显示语义名或 `0x{instance:8x}`，完整 TGI 移入详情并附一键复制
- 文件列表改**前端分页**（每页 100，上一页/下一页/页码/总数），避免大数据量全量渲染；分类切换走服务端过滤后重置到第 1 页

验证与性能：

- 后端 `cargo test --workspace` 31 套件全绿（含新增 scan/根级 parent/type_counts 过滤用例）；前端 `vue-tsc` / `vitest` 50 通过 / `vite build` 成功
- 性能（release，`app.package` 316MB / 2489 条目）：`type_counts` 17 类型 **35.4µs**；首页 100 条 `resource_page` **8.7µs** —— 类型聚合为单次 O(n) 索引遍历，分页过滤开销可忽略
- 产物：`target/release/fluffy-open-scp.exe`（内嵌最新 dist）

已知边界：音视频（wav/vp6）转码预览与导出、其他类型转码导入为下一批（后端封装 ffmpeg/vgmstream）；Raster/GLB 预览 command 仍待接；语义名依赖 registry s3db 存在。

---

## 9. 手测反馈第二轮修复记录（2026-09-06）

第二轮 EXE 手测反馈 6 项，全部修复：

- **registry s3db 内置分发**（解决"类型字段显示 hex"根因）：`database_main.s3db`（493KB，社区描述符库）作为 bundle resource 打包（`src-tauri/resources/`），`package_registry` 升级为三级回退：包目录向上 3 级 → 设置的游戏目录向上 3 级 → 应用内置资源；`database_user.s3db` 与 main 同目录时自动叠加覆盖。本地实测 s3db 可解析 app.package 全部 17 类型（PNG File / RW4 File / Javascript File 等）
- **Hex 预览重叠**：hex 列原为 `minmax(0, 1fr)` 会被压缩与 ASCII 列重叠；改固定 `9ch / 47ch / 16ch` 三列 + 容器横向滚动，窄面板不再互相覆盖
- **类型 tab**：去掉图标（该尺寸看不清），仅"类型名 + 数量"；计数徽标 active 态反色
- **tab 条滚动**：隐藏滚动条，两端加 ChevronLeft/Right 图标按钮（滚动 220px/次），到尽头置灰；scroll + ResizeObserver + tabs 变化三路驱动状态刷新
- **选中高亮**：package tab active 加 accent 底 + 加粗；资源行 selected 加 3px 主色侧条 + 名称加粗
- **大小显示**：存储列改 B/KB/MB/GB 自适应（<10 保留 1 位小数）

验证：后端 `cargo test --lib` 30 通过；前端 `vue-tsc` / `vitest` 50 通过 / `vite build` 成功；产物经 `pnpm tauri build` 输出 EXE + MSI + NSIS（含内置 s3db 资源）。

遗留：裸 `cargo build --release` 不带 `tauri/custom-protocol` feature 会产出连 devServer 的坏 EXE（第一轮已踩），给用户手测一律走 `pnpm tauri build`。

---

## 10. 预览器按文件类型路由（2026-09-06 第三轮）

- **类型→语言映射**：`resource-types.ts` 新增 `textPreviewLanguage`——js(0x67771F5C)→javascript、css(0x2C978DB6)→css、html(0xDD6233D6)→html、c++(0x0469A3F7)→cpp、json/locale(0x0A98EAF0)→json；`TextPreview` DTO 增加 `language` 字段
- **文本预览高亮**：`TextPreview.vue` 二次封装 `FCode`（shiki 高亮、主题跟随、语言标签、一键复制），保留编码/截断元信息；非映射类型字节嗅探为可读文本时以 text 语言高亮
- **预览 tab 不再回退 hex**：`ResourcePreview` 仅接受 text/image 两类；hex/unsupported 一律显示「{类型名} 类型文件暂不支持预览」（类型名来自 registry，hex 数据仍保留供 Hex 标签页使用）
- mock 数据源同步 `language` 字段；i18n 新增 `previewUnavailableType`/`copy`，移除失效的 `unsupportedPreview`

验证：`vue-tsc` / `vitest` 50 通过；产物 `pnpm tauri build`。

---

## 11. 静态图片预览接入（2026-09-06 第四轮）

- **后端**：新增 `read_resource_data` command——整资源解压读取（`RESOURCE_DATA_MAX` 32MB 上限），base64 编码返回（避免大资源 JSON 数字数组膨胀）；注册进 invoke_handler
- **类型路由**：`resource-types.ts` 新增 `imageMimeForType`——PNG(0x2F7D0004)→image/png、JPG(0x3F86 62EA)→image/jpeg、GIF(0x2F7D0007)→image/gif；`tauriPreview` 优先走图片分支：base64 解码 → Blob → `URL.createObjectURL` → 复用现有 `ImagePreview`（缩放/旋转/棋盘底）
- **blob 生命周期**：切换资源、切换/关闭 package 时 `revokeObjectURL` 释放；竞态请求（快速连点）失败侧同样释放
- **签名验证**（env 门控测试，真实 app.package）：PNG 资源 = `89 50 4E 47`、JPG = `FF D8 FF`、GIF = `GIF8`——包内图片为标准文件，blob 直读成立；灰度图（0x03E421EC/ED）为裸像素非标准文件，仍走"暂不支持"
- mock 数据源图片分支同步覆盖 PNG/JPG/GIF 类型

验证：后端 `cargo test --lib` 31 通过（含签名用例）；前端 `vue-tsc` / `vite build` 通过。

---

## 12. Property / RW4 结构化预览（2026-09-06 第五轮）

对齐原 SCP 的两种组合类型表单预览：

**后端**（`read_property_preview` / `read_rw4_preview` / `read_rw4_section_detail`，统一经 `read_resource_with` 32MB 解压上限）：

- Property：`sc_properties::PropertyFile::parse` 解析 0x00B1B104 → `{hash, 语义名(s3db Properties 表，无则 null), 类型名, 值文本, 数组长度}`；数组值 join 后 512 字符截断
- RW4：`Rw4File::parse` → 全量 section 列表 `{number, typeCode, typeName(SectionType 表), size}`；section 详情按类型分流——Mesh 走 `decode_mesh`（三角/顶点计数、解码数量、可导出性、顶点包围盒），Texture 走 `decode_texture` + `export_texture` PNG（base64 内联预览，256MB 纹理预算），其余类型给前 128 字节 hex+ASCII dump
- 真实 app.package 集成验证：property 条目解析非空、rw4 section 列表与详情可解析（env 门控测试）

**前端**：

- `PropertyPreview.vue`：工具栏（添加属性/编辑/查看子项/高级编辑器/工具下拉，全部置灰）+ Property|Value 双列表格（语义名回退 hash hex，类型徽标带数组长度）
- `Rw4Preview.vue`：Number|TypeCode|Size 表格，行点击经 `read_rw4_section_detail` 懒加载后从右侧弹出 `FSheet` 详情；Mesh 详情含工具栏（导入/导出/贴图/网格工具下拉，置灰）+ 统计网格；Texture 详情内联 PNG；其余显示 hex dump
- 预览路由：property/rw4 不再落入"暂不支持"，仍保留 4KB 头部字节供 Hex 标签页使用；mock 数据源同步

验证：后端 32 测试通过；前端 vue-tsc / vitest 50 / vite build 通过；产物 `pnpm tauri build`。

---

## 13. 3D 网格预览与间距修正（2026-09-06 第六轮）

- **MeshPreview 3D 组件**（three 0.185 动态导入，独立 chunk 不增主包体积）：OBJ 数据由 `sc_exporter::export_obj` 生成、base64 内联于 `Rw4MeshDetail.objBase64`（8MB 上限，超限返回 null）；交互含指针拖拽旋转、滚轮缩放（0.3×–12× 包围球半径）、点光源方位/仰角双滑杆（PointLight decay=0 + 环境光）；加载即包围球居中取景，法线缺失自动补算，卸载完整释放 geometry/material/renderer
- **RW4 Sheet**：Texture 详情改用统一 `ImagePreview` 组件（缩放/旋转/棋盘底），数据 URL 直载；内容区包 `.sheet-body`（gap 12px + 顶部 14px），修复 header 与内容贴合
- **预览面板**：`.resource-preview` 加 14px 顶部间距，修复 property/rw4 工具栏与 hex/预览 tab 贴合
- 依赖：`three` + `@types/three`；mock 网格附带可渲染的微型 OBJ

验证：后端 32 测试、前端 vue-tsc / vitest 50 通过。

---

## 14. Property Editor M-PE1 只读会话 + 3D 中键平移（2026-09-06 第七轮）

按 `docs/design/property-editor.md` 第一里程碑落地只读 Lot 编辑会话（编辑/写回留 M-PE2）：

**后端 Unit 结构层**（`sc-properties::lot_unit`，hash 常量迁移自 C# `PropertyConstants.cs`）：

- 装配约定与 C# `PropertyFileObjectCollection.Load` 一致：每 hash 一列并行数组，Unit #N = 各列第 N 元素，容错不等长列；Light 14 列、Effect 5 列、Prop 前 14 分箱（Transform/Slot）、Decal 3 类别（Scale 藏 Transform.Unknown hack）、PathPoint、Spawner、pathPairs(0x0CAA6841)
- LightType/CullDistance 枚举存 Key.instance（Point/Spot/Line、Near/Mid/Far/Max）；flags 15 的 Transform 提取 unknown 为 Scale；矩阵非 12 floats → 摆位置原点 + 诊断
- `LotEditorSession` 扩展 `modelKey`/`units`/`pathPairs` 字段（camelCase DTO，tag="kind" 判别联合）；`read_lot_editor_session` 装配并合并诊断
- 单测 11 个：不等长列、Spot 字段映射、未知枚举诊断、decal Scale hack、bin hash 选择、pathPairs、短矩阵降级等

**前端 ThreeViewer 引擎**（`src/lib/three-viewer.ts` + `three-obj.ts`，MeshPreview 重构为薄壳）：

- 相机：`orbitTarget` 泛化——左键轨道、**中键拖拽或 Shift+左键平移**（原生 mousedown 拦截 WebView2 中键自动滚动）、滚轮缩放限幅保留；三灯布光（key 滑杆 + fill/rim + 环境）替代单头灯
- Z-up `world` 根组 + 图层组注册表（model/lights/decals/props/effects/spawners/paths）+ Raycaster 轻点拾取 + emissive 选中高亮 + `frameContent()` 包围球取中/构图/地面网格

**Property Editor 全屏 Sheet**（Blender 三区，FSheet width 100vw）：

- Outliner：六类分组树 + 计数 + 组/单项独眼可见性；视口：白模 + SCP 忠实 Unit 图形（Point 球/Spot 截锥 0.6 透明/Line 盒，按 LightColor 着色；Effect 金锥、Prop 红锥、Spawner 蓝锥、PathPoint 青球 + 折线、Decal 绿背矩形），WPF Matrix3D 行主序→three 列主序转置映射（unitMatrix 纯函数测试钉死）；右上角可见性开关 + 重置视角
- 只读 Properties 面板（summary 行 + Unit 消费 hash 的属性行表）；状态栏（选中项/模型状态/各类计数）；视口↔Outliner 双向选中
- 会话数据流：PropertyPreview 工具栏第 4 按钮（原"高级编辑器"改名"属性编辑器"）启用 → `readLotEditorSession` → 模型链复用 `readRw4Preview`/`readRw4Section`（filter 0x20009）；模型缺失/失败降级为仅 Unit 视图，不阻塞
- `PropertyPreview` DTO 补 `packageId`/`tgi`（对齐 Rw4Preview）；mock 数据源罐头会话；zh-CN/en-US 双字典 40+ 键

验证：后端 workspace 全测试通过（sc-properties 33 含新 11 例）；前端 vue-tsc / vitest 59（新增 unitGizmos 5 例）/ vite build 通过；改动文件 eslint 干净。

---

## 15. Unit 朝向校准与 Lot 地面矩形（2026-09-06 第八轮）

用户对拍原 SCP 发现锥体/带状灯/道具朝向错位。用真实包取证工具（`sc-properties/examples/lot_axes.rs`，打印模型包围盒与 Unit 原始变换）交叉验证，锁定约定：

- **数据帧 = Z-up**：模型 Z 跨度=楼高（Z[-1,128]）、地面矩形为 XY 平面贴地、Unit 平移第 3 槽位=离地高度（楼顶灯 z=127.67）；SCP 全链路零旋转渲染（`Positions.Add` 原样、`MatrixTransform3D` 原样），行向量 `p·R·M`
- **显示轴向约定**（证据 + 用户对拍逐轮校准后的最终版；SCP `CreateGeometry` 里的 R(±90) 为无效死代码，净效果即行向量 `p·M`）：
  - Spot 锥 = 开口沿 **M 第 2 行**（局部 +Y，无预旋转；门侧 M₁ 型 → (0,0,-1) 垂直朝下）
  - Effect/Prop/Spawner 标记锥 = 宽端沿 **+M 第 3 行**（几何预旋转 +90°X；恒等变换下尖锥朝下 ▼；初版 -90°X 曾上下颠倒，已由用户对拍纠正）
  - Line 盒 = 长轴沿 **M 第 2 行**（与 Spot 同用局部 +Y，无预旋转；初版取第 3 行曾横竖互换，已由用户对拍纠正）
  - 贴花矩形法线 = M 第 3 行（原实现已正确）
- **Lot 地面矩形**：会话 DTO 增 `lotSize`（0x0CCB7FC8 camelCase 拷贝）；视口按尺寸画地面细框+半透明填充锚定构图；LotMask 四色地面图待 Raster 解码（M-PE3）
- 取证工具 `lot_axes.rs` 保留（rw4 加入 sc-properties dev-deps）

验证：cargo 全测试、vue-tsc / vitest 59 通过。

---

## 16. 编辑器光照浮层与渲染模式占位（2026-09-06 第九轮，未打包）

用户反馈后的小迭代（本批未出 EXE）：

- **重置按钮 icon 修复**：`RotateCcw` 未注册导致 FIcon 回退问号占位；`src/lib/icons.ts` 注册 `RotateCcw` 与 `Lightbulb`
- **光照编辑浮层**：视口右上角新增 Light 按钮（Lightbulb icon），点击展开浮层——方位/仰角双滑杆（复用 `package.lightAzimuth/lightElevation` i18n），实时驱动 `ThreeViewer.setKeyLight`；补齐原 3D 预览器的光源调节能力
- **渲染模式开关占位**：编辑器 header 只读 tag 左侧新增「默认 | 精细」分段开关，当前置灰；后续启用后切换不同 LOD 模型、精细模式自动绑定贴图材质（slot0 调色板/slot2 法线）并把标记光源替换为对应类型真实光源
- **后续计划（记录待办）**：property 的 LOD 模型族切换（0x00f9efbb/bc/bd/be）、RW4 Material→贴图实例绑定、Raster(0x2f4e681c) 解码（M-PE3）

验证：vue-tsc / eslint（改动文件）/ vitest 59 通过；未执行 pnpm tauri build（按用户要求）。

### 16.1 灯带定位漂移修复（2026-09-06 第十轮，用户对拍）

Line 盒几何残留 Helix `Center(0,0,-len/2)` 偏移，去除 +90°X 旋转后该偏移经 M 映射为 -len/2·第 3 行——竖直灯带水平漂、水平灯带竖直漂，各漂半个长度（灯带间相对关系不变，与用户观察一致）。修复：偏移改为 `+len/2·局部+Y`，灯带从灯具原点沿发光方向延伸整段长度，与锥形灯"自原点展开"模式一致。vue-tsc / vitest 通过后重新出包。

**✅ 用户多 property 交叉比对确认（2026-09-06）**：Unit 朝向、定位、灯带横竖与漂移修复全部通过，M-PE1 只读会话渲染侧验收完成。

---

## 17. Raster(0x2f4e681c) 解码与预览（2026-09-07 第十一轮，分支任务）

用户判断验证：raster 是 RenderWare/D3D8-9 风格的未压缩纹理容器（`0x2f4e681c`），原 SCP 可导出 dds/png。

- **`rw4::raster` 新模块**：6×u32 大端头（rasterType/width/height/mipCount/pixelSize/pixelFormat）+ 逐 mip `[u32 blockSize][载荷]`；`pixFmt==21`（D3DFMT_A8R8G8B8）内存字节序 B,G,R,A → RGBA 重排（与 texture.rs raw 路径一致）；`decode_lot_mask_rgba` 复刻 SCP `RasterChannel.Preview` 四层阈值量化（≥128 选层，字节→LotColor 按 C# 调用序交叉映射 byte3→color4/byte0→color3/byte1→color2/byte2→color1）；DXT 压缩变体（pixFmt≠21）仅元数据（C# CLI 同样未实现）。5 个单测
- **`read_raster_preview` 命令**：工作区 raster 预览——返回元数据 + PNG base64（pixFmt21）；不可解时 `decodable:false` 回退通用"暂不支持"。前端 `tauriPreview` 新增 raster 分支，复用 ImagePreview（缩放/旋转/棋盘底）
- **PE 地面真图**：`read_lot_editor_session` 服务端解码 LotMask→LotColor1-4（0x0D02D586..89，缺省黑/红/绿/蓝）四色量化 PNG，会话 DTO 增 `lotMaskPng`；视口地面矩形异步贴图（SRGB，带重建代际守卫），原"LotMask 解码不可用"诊断移除（失败时转具体诊断）
- **真实包冒烟**：`rw4/examples/raster_probe.rs`——EP1 359 个、DLC0 55 个 raster 全部 pixFmt21、解析+解码 100% 通过零失败

验证：cargo（rw4 38 + sc-properties 28）测试、后端 check、vue-tsc / vitest 59 通过。

### 17.1 raster 字节序/模糊/跨包三修复（2026-09-07，用户对拍）

- **通道顺序**：用户同文件对拍发现我们解出的图案与 SCP 的 A 通道视图一致——raster 像素实为**顺序 R,G,B,A 直读**（对齐 C# RasterImage 读取器），去除照搬 RW4 内嵌纹理的 BGRA 重排（两容器字节序不同，已在模块文档标注）
- **模糊**：低分辨率 mask 用平滑插值放大所致；`ImagePreview` 增 `pixelated` 标志（`image-rendering: pixelated`），raster 预览启用像素风渲染，与 SCP 锐利边缘一致
- **PE 地面跨包**：探针显示 EP1 2194 个带 LotMask 的 property 仅 97 个在同包找到 raster（Key 的 type/group 多为 0，raster 在 graphics 包）——`PackageManager` 增 `all_packages()` 快照，`decode_lot_mask_png` 查找顺序改为当前包 → 所有已打开包（对齐 SCP"全部已加载索引"语义）；跨包时提示打开对应包

验证：cargo（rw4 38）测试、后端 check、vue-tsc / vitest 通过。

**✅ 用户多文件交叉比对确认（2026-09-07）**：raster 预览与 PE 地面 LotMask 贴图全部正常（字节序/锐利度/跨包查找三修复均通过），本分支任务验收完成。

---

## 18. RW4 mesh↔material↔texture 绑定关系调研（2026-09-07 第十二轮，精细渲染前置）

目标：为 PE「精细渲染」模式（unit model 绑定贴图材质 + 标记光源换真实光源）摸清 RW4 内部绑定结构。方法：C# SimCityPak 源码考古 + Rust 探针 `rw4/examples/material_bind_probe.rs` 在 EP1/Graphics/Game/DLC0 四包上逐字节验证。

### 18.1 C# 端现状（考古结论）

- **解析覆盖**：`RenderWare4/` 下 mesh/VA/TA/材质/贴图/调色板/DXT 解码齐全；glTF 导出器（`GltfConverter.cs`）能绑 baseColor/normal/specularTexture（`KHR_materials_specular`），metallic=0/roughness=1；OBJ 导出无 .mtl
- **C# 材质解析有错位 bug**：`RW4Material.Read` 把引用表第 0 条的 slot 字段（0x2D）当"marker"丢弃后只读 6 条记录 → 整表错位一条，其 `TextureInstanceId` 读到杂讯（EP1 实测如 0x20D=材质自身大小），下游 `ResolveTextures` 只能靠内容启发式猜（textureType 116=调色板/粉色=法线/最大非蓝=baseColor）
- **Spore 链路不适用**：`Mesh→MeshMaterialAssignment(0x2001a)→TexMetadata` 在 SimCity 被禁用（mesh 不引用 material；模型级 Material section 承载全部贴图引用）
- **RW4 内无光源 section**（`SectionTypeCodes` 无 Light 类型）；光源全部在 property 的 scLight* 平行数组（已被我们 `lot_unit.rs` 解析：color/radius/diffuse/length/cull 齐全，直接可驱动 three.js 真光源）
- 建筑真实 albedo 由 GlassBox deferred shader 运行时合成（mask→palette 混合+HDR tonemap），C# 逆向未完成，只能近似

### 18.2 材质引用表真实布局（逐字节验证，修复错位）

Material(0x2000B) payload = `Size` u32 → 28B 头 →（模型有 VF section 时）顶点格式副本 → 附加数据 → **7×24B 引用记录** → 尾数据。记录 = `(slot, unk, instance, unk, unk, unk)`，第 0 条 `slot=0x2D` 为 shader-def 引用（instance→shader 资源），随后 slot `0..=5` 为纹理槽。

**EP1 全量统计**：4654/4703 材质完全符合（余 49 布局异常回退 Raw）；四包联合查找下 **slot0-5 引用 100% 解析**。槽位语义（detail 0x63D180B9 + HANDOFF 交叉印证）：

| slot | 内容 | 容器（实测） |
|---|---|---|
| 0x2D | shader-def 资源引用 | 不在内容包（全局 shader 包） |
| 0 | 调色板条：RW4 包裹 Texture type 0x74（A32B32G32R32F）如 119×4，每列 4×f32 = (ColorBottom, ColorTop, Int1, Int2)/256 | RW4(0x2f4e681b) |
| 1 | 区域/分区遮罩（当 baseColor 用，共享图集） | raster 512×512 RGBA |
| 2 | 法线（粉色编码，specular 在 alpha） | raster 512×512 |
| 3 | 副遮罩 | raster 512×512 |
| 4 | 竖条纹理（512×16 raw type 0x15） | RW4 包裹 |
| 5 | DXT5 256×256 细节纹理 | RW4 包裹（内嵌 mip 链） |

0x1188B12E（slot1）/0xA3791E5C（slot2）为 18+ 模型共享图集，印证 HANDOFF。**跨包查找必需**：EP1 模型 slot1-5 约半数引用落在 Graphics/Game/DLC0（与 LotMask 同模式：当前包 → 已打开包）。

### 18.3 mesh 侧与贴图相关的顶点事实

- **D3DCOLOR 元素分段**：相邻顶点 D3DCOLOR 变化即切"元素"（C# `RW4Mesh.Read`）；实测 EP1 建筑 0x63D180B9 有 419 个元素，**G 通道 = 调色板列号**（73/111/97/…指向 slot0 的 119 列），B 通道 = 变体 RNG（C# 导入器写 B=84 的约定与实测不符，仅参考）
- **UV 两态**：FLOAT2 = 真 UV（glTF V 取反）；facade 建筑只有 FLOAT4 且是世界投影平铺坐标（max 值上万），范围 ≤8 才当 UV（C# 约定）
- 模型内 Texture section 是 2×2 占位，真贴图全在外部资源（raster 0x2f4e681c 大图集 / RW4 0x2f4e681b 包裹调色板与压缩纹理）
- app.package 538 模型仅 2 个带 Material 且引用为伪值（App 包是 UI/道具模型，无贴图绑定需求）；PE 场景的建筑模型在 EP1/Game

### 18.4 精细渲染实现要点（结论）

1. **服务端**（`package_service`）：会话返回新增 `modelTextures`——按 material 解析四包索引，slot0 调色板解码 f32 列、slot1 区域遮罩 PNG、slot2 法线 PNG（Unswizzle：R↔B 对调、alpha→specular 灰度）
2. **前端**：按 D3DCOLOR G 通道把三角形分组为元素，元素色 = 调色板列（ColorBottom/Top 按 B 通道 RNG 混合或 v1 取均值）→ `vertexColors`；区域遮罩做 baseColorTexture；无 FLOAT2 UV 的 facade 用世界投影 shader（v1 可先用元素色+遮罩灰度）；slot5 DXT5 细节图 v1 可忽略
3. **真实光源**：`lot_unit.rs` Light 已含 color([f32;3])/outer_radius/inner_radius/diffuse/length——Point→PointLight、Spot→SpotLight(angle=atan(outer/length))、Line→RectAreaLight 或双光灯管模型
4. `rw4::material` 已按 7 记录布局修复（含 fixture 更新，38 测试通过）；`material_bind_probe.rs` 保留作跨包绑定率冒烟工具

验证：cargo（rw4 38 + sc-properties 28）测试通过；EP1 4654 材质布局 98.96% 命中、四包联合引用解析 100%。

---

## 19. PE 精细渲染 v1（2026-09-07 第十三轮）

启用 header「默认 | 精细」渲染模式开关（此前为置灰占位）。精细模式：unit model 重新绑定贴图材质，标记光源替换为真实 three.js 光源；props/decals/spawners/effects 小圆锥保持不变。

### 19.1 后端

- **`rw4::texture::decode_palette_f32`**：A32B32G32R32F 调色板条（textureType 116）→ 逐像素 4×f32（行主序）；列=材质元素，row0=ColorBottom / row1=ColorTop（C# SCP- 协议语义）
- **`rw4::unswizzle_simcity_normal`**：法线解 Swizzle（粉色 ~255,128,128 = +Y 在 RED → R↔B 对调得 three 切线空间约定；alpha 保持含 specular）
- **`sc_exporter::export_obj_with_colors`**：OBJ `v x y z r g b` 顶点色扩展（three OBJLoader 原生解析为 `color` 属性）
- **新命令 `read_lot_model_meshes`**：一次返回模型全部网格 OBJ（顶点色已烘焙）+ 材质资源，取代前端 1+N 次 section 请求。材质链：Material 槽位引用 → **跨包查找**（当前包 → 已打开包）→ slot0 调色板（RW4 包裹 type 116，row0/row1 均值→逐列 RGB）→ D3DCOLOR.G 列号映射顶点色；slot1 区域遮罩红通道（=调色板查表索引，GlassBox 语义）→灰度 PNG；slot2 法线→解 Swizzle PNG；模型含 FLOAT2 真 UV 才发贴图（facade 世界投影 UV 不贴）
- `DecodedVertex` 增 `has_float2_uv()` / `d3d_color_g()`

### 19.2 前端

- PropertyEditor header 开关启用（`renderMode: default | refined`），变更触发视口重建
- 精细材质：`MeshStandardMaterial{ vertexColors, map, normalMap, roughness 0.82 }`，贴图 SRGB + 重建代际守卫
- 真实光源（`buildRealLightUnit`）：Point→PointLight(distance=2×outerRadius)、Spot→SpotLight(angle=atan(outer/length) 钳位、penumbra 0.5、锥轴=局部 +Y)、Line→PointLight(管中点) + 发光管（自原点沿局部 +Y 延伸，同标记盒约定）；均带发光球拾取代理，outliner 选择/显隐不受影响；强度 = diffuse×8、decay=1（观感近似值，待对拍校准）

### 19.3 地面纹理偏移结论（停止排查）

用户确认：**同一 property 文件在原版 SCP 中也存在相同的地面纹理偏移**（偏移量一致）——该现象为原版行为而非本次迁移缺陷，不再处理。已验证的技术事实留存：placement 语义 = 模型坐标→地块坐标（脚印在 +t），mask 像素 Y 轴与地块 Y 反向；EP1 725/2194 带 placement（183 恒等 / 542 平移）；0x0CCB7FC9 offset 属性多为 (0,0)。

### 19.4 性能

真实包计时（EP1 建筑 0x63D180B9：2409 tri / 4682 verts，含模型解析、网格解码、材质解码、slot4 512×16 raw 与 slot5 DXT5 256×256 跨容器解码、顶点色映射）：**release 全链 8.65ms**（debug 69ms）。前端改为单命令后 IPC 往返从 1+N 降为 1。

验证：cargo（rw4 40 + sc-properties 29 + sc-exporter 13 + 后端 41）测试、vue-tsc、vitest 59 通过。

### 19.5 第二轮调整（2026-09-07，用户对拍）

- **线光源不可见化**：删除发光管/发光球——线光源为氛围光照，本体不可见；沿灯带均匀布 2~6 个小范围点光（按长度自动分段，distance≈length×1.2、decay=1）近似条形照明；留透明拾取代理（opacity 0 + 不写深度，three Raycaster 不过滤透明对象，outliner 选择/显隐不受影响）
- **贴图判定扩展**：`has_uv` = 有 FLOAT2 真 UV，或（仅 FLOAT4 且全部 xy 范围 ≤8——C# GltfConverter 同规则，OBJ vt 即 FLOAT4.xy）。EP1 实测：2819 带材质模型中 FLOAT2 仅 28、FLOAT4≤8 有 301、世界投影大坐标 696 → 贴图覆盖率 1%→11%
- **下一阶段（用户明确）**：大部分建筑仍只有基础顶点色、无纹理/材质——需实现 **facade 世界投影 UV + 每元素 UV 裁剪窗**（SHORT4N TEXCOORD 分量携带 X=width/Y=height/Z=x/W=y 归一化裁剪域，见 §8.10 / HANDOFF），让 696 个大坐标 facade 也能正确采样区域遮罩与法线

验证：后端 41 + vue-tsc + vitest 59 通过。

### 19.6 第三轮（2026-09-07）：二进制 GLB 通道重做 + 灯光三项修正（用户反馈）

**问题**：精细模式加载 3~15s 且 UI 冻结。根因不在 GPU 渲染而在数据通道：后端导出冗长 OBJ 文本（约 96B/顶点）→ base64 ×1.33 → 塞 JSON 字符串；前端**主线程**逐字符 `charCodeAt` 解码（`three-obj.ts`）+ 同步 `OBJLoader.parse` 文本解析。

**方案（用户确认：Rust 出二进制 GLB，渲染仍 three.js 保留全部交互）**：

- `sc-exporter/gltf.rs`：新增 `export_glb_with_colors`（COLOR_0 VEC3 f32 顶点色，three GLTFLoader 原生映射 `color` 属性）；`export_glb`/`export_glb_with_textures` 改为委托。`export_obj_with_colors` 随 OBJ 通道退役删除（`export_obj` 恢复纯色）
- `read_lot_model_meshes` 改原始字节通道（`tauri::ipc::Response`，跳过 JSON/base64），负载为 "LOTM" 容器（小端）：`magic|version|mesh_count|每 mesh u32 len+GLB|u32 len+baseColor PNG|u32 len+normal PNG|has_uv u8`；贴图全地块共享一份不逐 mesh 复制；`ModelMaterialBundle` 存 PNG 字节（`encode_rgba_png_bytes`），base64 版仅留其他调用方
- 前端 `src/lib/three-gltf.ts`：容器解析（DataView 零拷贝切片）+ `GLTFLoader.parseAsync` + PNG 字节→blob URL（免 data:URL 再解码）；**GLTF 根节点 -90°X 旋转需剥离**（视口 world 组已做同款旋转，模型须与 Unit gizmo 共享 Z-up 世界）；`parseLotModelContainer` 有 3 项 vitest
- session 链路：`modelMeshes: string[]` → `modelPayload: LotModelPayload`（session 内即解析校验）；MeshPreview 的 OBJ 通道不动

**灯光三项**：

- 光源本体全隐藏：Point/Spot 发光球删除，三种光源统一透明拾取代理（Point=球 r=outerRadius、Spot=圆柱 r×0.5、Line=沿用盒），outliner 选择不受影响
- 强度 diffuse×8 → ×16；每个真实光源记录 `userData.baseIntensity`
- 新增「环境亮度（昼/夜）」滑条 0~2（默认 1）：`ThreeViewer.setEnvironmentBrightness` 缩放环境四灯（key 2.2/fill 0.5/rim 0.65/ambient 0.38 基准），viewport 遍历 lights 组按 baseIntensity 缩放真实光源

**性能数据（debug 构建）**：金样本 ec3eade0 彩色 GLB 196 verts → 11.9KB / 0.2ms；全量 GLB 导出 DLC0 565 mesh/1.22M verts 4.02s、EP1 1025 mesh/2.10M verts 10.62s（后端 release 全链此前实测 8.65ms 不变）。逐顶点负载 ~60B vs 旧 OBJ+base64 ~128B（约 2× 缩），且消除 JSON 转义与主线程文本解析——前端预期 <1s 不冻结，待手测确认。

验证：sc-exporter 15（新增 COLOR_0 断言 + 真实模型彩色 GLB + 容器字节搜索）、后端 41、vue-tsc、vitest 62（新增容器解析 3 项）通过。

#### 19.6.1 用户对拍补充（2026-09-07）

- **亮度滑条语义修正**：滑条此前把地块真实光源一起缩放（调 0 连灯都黑了）——改为仅缩放环境四灯，地块光源恒定常亮（夜间路灯依然亮，真昼夜模拟）；`baseIntensity` 机制随之删除
- **推近复杂建筑掉帧**：WebGL 前向渲染每片元评估全部光源，多灯地块片元数×光源数爆炸。修复：真实光源总预算 24（超限从强度最弱单元起摘除光源本体、保留拾取代理）、线光分段上限 6→4、`pixelRatio` 钳制 min(DPR,1.5)、WebGL `powerPreference: high-performance`
- **devUrl 错位修复**：tauri.conf.json devUrl 5174 → 5173 对齐 vite strictPort（此前 dev 模式无法启动）；注意 Windows 下停 pnpm 外层进程会残留 vite 子进程占端口

## 20. 文本预览：字符集/二进制帧真相 + 不截断虚拟滚动（2026-09-07 第十四轮）

### 20.1 "乱码"根因：不是字符集问题（探针逐字节取证）

用户报告 cpp 类型（0x0469a3f7，SimCity 2013 shader 源码）UTF-8 解码后乱码。对 app.package 全部 32 个 cpp 资源探针（临时 Rust 测试，结论留档后已删）：

- **0/32 是严格 UTF-8**——这些资源是**二进制容器**：NUL 结尾的名字（"NullVS"、"modelToClip"）+ 小端元数据块 + 内嵌 shader 源码交替出现；同类型下还有纯二进制查表资源（instance 0/1，`00 01` 重复）
- 实测字节例：`"modelToClip"\0 | 06 00 06 00 04 00 00 00 00 | 40 01 00 00 00 00 | 06 "NullVS" | ...`——0x40='@' 是可打印字符会打断二进制段（旧预览显示的 `modelToClip@NullVS` 即由此来）
- **C# SimCityPak 也没有该类型解析器**（全源码 grep 零命中，原版显示同样的噪声）；HANDOFF.md 无此格式记录 → 逐字节逆向属独立课题，本轮不做

### 20.2 修复：三级解码 + 二进制段可见化（`src/lib/text-decode.ts`）

1. BOM 识别（UTF-16LE / UTF-8 BOM）
2. 严格 UTF-8，失败退宽松解码
3. **关键判定**：解码结果含控制字符（NUL/C0/C1，Tab/LF/CR 除外）或 U+FFFD 即认定含二进制帧——**NUL 是合法 UTF-8，仅靠严格解码失败判定不住**（单测踩坑实证）；连续段折叠为 `⟦N B⟫` 可见标记，源码主体保持可读、二进制位置如实标注（encoding 显示 `utf-8+binary`）
- `looksLikeText`（未知类型嗅探）同步强化：全可打印 ASCII，或严格 UTF-8 且无控制字符——纯二进制表落回 hex 视图
- locale JSON（0x0a98eaf0）3 字节前缀保守剥离（前缀后须紧跟 `{`）

### 20.3 不截断预览：全量读取 + 行级虚拟滚动

- 截断根因：`previewResource` 只 `readBytes(0, min(4096, size))`，且 FCode 用 shiki 对**整段**内容高亮（放大限制直接卡死）
- 新命令 `read_resource_text`：`tauri::ipc::Response` 原始字节通道全量返回（上限 8MB，超限置 truncated）
- `TextPreview.vue` 重写：行级虚拟滚动（只渲染可视窗口 ±24 行，rAF 节流）+ shiki 仅高亮可视片段（token 守卫）+ 行数/编码徽标 + 全文复制按钮；预览上限从 4KB → 8MB（1.2MB shader 容器全量可读）

验证：vue-tsc、vitest 73（新增 text-decode 11 项，含 NUL-合法-UTF-8 回归）、cargo workspace 全绿。

## 21. RW4 texture/material 绑定机制研究（2026-09-7 第十五轮，LOD 切换附带产出）

### 21.1 重大发现：MeshMaterialAssignment（0x2001A）= mesh→material 绑定表

EP1 多 mesh 模型 0x41B1BAC0 的 section 布局：`#10 Mesh, #11 Mesh, #12 Material, #13 MeshMaterialAssignment, #14 Material, #15 MeshMaterialAssignment`。assignment 原始字节：

```
#13: 0B 00 00 00 | 01 00 00 00 | 0C 00 00 00   → mesh #11 ↔ material #12
#15: 0A 00 00 00 | 01 00 00 00 | 0E 00 00 00   → mesh #10 ↔ material #14
```

**格式 = 12B：mesh section 号 u32 + u32（恒 1，语义待定）+ material section 号 u32**。C# `RW4Model.cs` 的 RWMeshMaterialAssignment 被注释禁用（误判为 Spore 专用），实际 SimCity 文件普遍存在。EP1 全量统计（`material_bind_probe`）：

- meshes=4703 = materials 可解 4654 + Raw 49 —— **每 MESH 恰好一个 MATERIAL**
- 2819 带材质模型中 1835 个含多 MATERIAL；双 UV 通道（≥2 组 TexCoord）模型 376 个
- 六槽贴图跨包解析率：slot0 4649/4654…全部命中主包（slot0 调色板全本地）
- 由此：`resolve_model_material` 的"取第一个材质"应改为 **assignment 表逐 mesh 绑定**（下一轮实现，含 facade UV）

### 21.2 modding.pdf（docs/overview/modding.pdf）佐证

- **材质命名协议**（p240）：`SCP-(colorBottom)-(intOffset)-BottomLayer-(intTex)-(colTop)-TopLayer-(paddingX)-(paddingY)-(index)`，其中 **BottomLayer/TopLayer = clip range W,H,U,V**（两裁剪窗）——与 §19.5 SHORT4N TEXCOORD `X=w/Y=h/Z=x/W=y` 推测互证；尾部 index 0~255 且 **256 起回绕**（p244 TIP）= D3DCOLOR.G 调色板列
- **双 UV Map Channel**（p241）：Channel 1 = 底层贴图，**Channel 2 = 第二层贴图（如砖墙上的门窗）**；Opiie RW4 文档（p279）顶点格式含 "Interior UV texture coordinates" —— 第二 UV = 假内景投影
- **按材质分面片**（p252）：建模时把共用同一贴图的面 detach 成独立 object 并赋对应材质——解释了"每 mesh 一材质"的来源
- **导入链**：3ds Max(OpenCOLLADA) → SimCityPak 工具导入 LOD 层（p247）；**SimCityPak 只认 3ds Max 导出的 DAE 材质**（p257）——贴图不内嵌，经材质槽引用
- **Opiie: RW4 Model File**（p279-280）：Mesh 头字段序与我们的解析一致（40,4/tri_section/tri_count/1,0/tri*3/0/vert_count/vert_section）；Texture 头 = type/恒 8/unknown/W,H/mip info/0,0/data_section
- **Opiie: RASTER File**（p281）：File Type/Width/Height/Mipmap Count/Unknown/Pixel Format + 每 mip [block size + ARGB]；"**大多数用途把 ARGB 通道当调色板而非直接显示**"——建筑贴图/地面/贴花均语义化用通道，与遮罩红通道=调色板索引的结论一致

### 21.3 结论与下一步

1. mesh→material 绑定已实锤：**解析 0x2001A 表** → 每 mesh 拿到自己的 slot0-5 → 精细渲染器按 mesh 绑定材质（取代现"全局第一个材质"）
2. 材质双裁剪窗（W,H,U,V ×2）+ 调色板列 index 已在命名协议中闭环，SHORT4N TEXCOORD 裁剪窗可直接用该语义实现 facade/多层贴图
3. 双 UV 通道 = 第二层（interior/门窗）→ GLB 导出需加 TEXCOORD_1
4. shader-def（0x2D 指向的全局包资源）内容仍未知，可后置

### 21.4 simcity_material.pdf（SUGC Materials 章，Berl Newell/Ocean Quigley）官方语义

新文档 docs/overview/simcity_material.pdf（39 页，全图无文本层，已逐页精读）——不含命名协议/RW4 结构，但给出命名协议背后每个实体的**官方定义**：

- **Material Set = 3 张图 × RGBA = 6 张纹理**（p13 原文 "stores a total of 6 textures in the RGBA channels of the 3 images"）——**与 slot0-5 数量精确吻合**：
  - Color control map：RGB = tinting + 元素区域分割；A 双职责（Base Layer 定形状镂空 / Top Layer 控透明）（p15）
  - Normal map：RGB = 法线，A = ambient occlusion（p16）
  - Shader map：**B = Specularity**（p18 原文），A = 窗户位置/透明（黑 = No Interior，50% 可半透，p19）
- **调色板 = 512×16 纹理：256 列 × 7 行，1 采样点 = 2×2 像素**（p23-24 原文）；材质元素被分配到一列、**"Simcity assigns it a row"**（p26），同列各行提供该材质的色调变体，"255 blocks 整列重复"（p26）——slot0 取 row0/row1 均值的现有实现应按"列=材质、行=变体"语义校准
- **图层系统**：每个模型 section 分配 Base Layer + Top Layer（各配 material+color），窗户 section 另配 **Interior Map**（预渲染房间网格图，假内景，p03/p07-08/p32）——对应命名协议双 clip range 与双 UV 的 Interior UV
- **Material Info Texture = 数据文件而非图片**（p09），由 OpalePlus/SimCityPak 写出，把"section→层→材质→调色板列"翻译给引擎——即 RW4 Material/0x2001A 绑定链的内容侧
- Color control map 由引擎预处理加黑边分割元素（p28）；官方管线四步（p09）：section 定层 → 层取材质集 → 材质配调色板色 → 窗户配 interior map
- 渲染侧：同风格建筑复用同一套 facade 纹理集合批；**Palletizing = 同纹理不同建筑/部位不同色**（p31），Base/Top 各自独立 palletize（p32）；高频细节走 normal map（p35）+ relief mapping（p36-39）

### 21.5 阶段 1：slot0 调色板行语义校准（2026-09-07）

**取证**（`palette_probe` 新探针，EP1 全量）：2528 个 slot0 调色板**全部为 W×4**（宽 119~435，无 512×16 标准布局，2×2 采样点展开不需要）；**columns 200620 中 192283（95.8%）row0≠row1（任一通道差 >0.1）**——旧实现取两行均值在 96% 的列上洗色。

**校准**：`resolve_palette` 停用均值，顶点色烘焙改取 **row0（ColorBottom，基层色）**。row1（ColorTop 顶层）/row2-3（Interior）待阶段 2（按 mesh 材质）与阶段 4（合成着色器）接入。

**新开放线索**：模型 0x63D180B9 实测 D3DCOLOR 的 **B 通道逐元素变化（1~80）且远超行数**——不是行号；假设：顶层列号（基层列=G、顶层列=B）或其它编码，待与遮罩绿通道/原版截图对拍（阶段 4 线索）。

性能：烘焙成本不变（同等查表量）。

## 22. 阶段 2：0x2001A 逐 mesh 材质绑定 + LOTM v2 容器（2026-09-7 第十六轮）

### 22.1 实现

- `rw4::material::decode_mesh_material_bindings`：解析 0x2001A（12B = mesh_section + u32(1) + material_section），损坏条目跳过；真实包金样本测试（EP1 0x41B1BAC0 双 assignment 字节断言）
- `read_lot_model_meshes` 重构为 `build_lot_model_payload`（可测试）：按绑定把每 mesh 配到自己的 MATERIAL（绑定缺失/材质 Raw 回退第一个可解码材质，v1 行为）；**逐 mesh** 用其材质的 slot0 调色板烘 COLOR_0、slot1/2 解出 PNG；可贴图判定改为逐 mesh（`mesh_has_uv`）
- 容器 **LOTM v2**：`magic|version=2|mesh_count|每 mesh GLB|material_count|每材质 base/normal PNG|每 mesh material_index+has_uv u8`（材质按 section 号去重共享 PNG）；前端 `parseLotModelContainer` v2 + 视口按 materialIndex 分组应用贴图（hasUv 逐 mesh 闸门）；mock 同步 v2

### 22.2 重大既有缺口：蒙皮双变体建筑整体未渲染

EP1 模型结构分布（shape histogram）：894 个 0/0、**1025 个 1 mesh/1 材质、1839 个 2 mesh/2 材质**。全部 2-mesh 模型（EP1/DLC0 均如此）的 mesh **decode_mesh 失败**（ME100 expected 0x1CE found 0x216 / ME004 expected 0 found 0x56A——蒙皮顶点声明变体），即 **65% 的带材质建筑当前完全没有几何**。这解释了此前"每模型 1.67 材质"的错觉：静态建筑都是 1 mesh/1 材质。修复蒙皮解码（解锁 1839 栋建筑 + 多材质分组实战验证）列为阶段 2.5 最高优先。

### 22.3 性能

金样本 0x63D180B9（2409 tri/4682 verts，含 slot1/2 跨包解码与 PNG 编码）：**release 全链 11.36ms**（v1 8.65ms，+31% 来自逐材质槽位解析；单材质模型差异极小）。多材质开销 = 材质数 × 槽位解析，静态建筑均单材质，无感知。

验证：rw4 金样本绑定测试、src-tauri v2 容器测试（结构/分组/计时）、cargo workspace、vue-tsc、vitest 73（容器 v2 3 项重写）通过。

## 23. 阶段 2.5：共享池 mesh 头重解读——1839 栋双变体建筑解锁（2026-09-7 第十七轮）

### 23.1 头布局重解读（0x41B1BAC0 逐字节取证）

旧解析（对齐 C# ME001-ME006）把 mesh 头 [5] 当恒 0、[6][7] 当 u64 tri×3——仅在"独占 TA/VA 池"的静态网格上碰巧成立。金样本双变体模型 0x41B1BAC0 实测统一布局：

| 槽位 | 语义 | mesh#10（变体 A） | mesh#11（变体 B） |
|---|---|---|---|
| [2] | tri_section | 5（共享） | 5（共享） |
| [3] | triangle_count | 462 | 72 |
| [5] | **start_index**（TA blob 内 u16 索引偏移） | 0 | 1386（=462×3，紧接 A） |
| [6] | **index_count**（=triangle_count×3） | 1386 | 216（=72×3） |
| [7] | **min_vertex_index**（draw-range 信息） | 0 | 451 |
| [8] | vertex_count | 1068 | 581（=1031-451+1，与其索引切片范围精确吻合） |
| [9] | vertex_section | 20（共享 VA，池 1068 顶点） | 同左 |

**判定实验**：mesh#11 的 216 个索引切片范围 [451,1031]，max-min+1=581=vertex_count——索引为**池相对**（非局部+基址），min_vertex_index 恰为其下界。

### 23.2 实现（`crates/rw4/src/mesh.rs`）

- `parse_mesh_header` 重写：[5]=start_index、[6]=index_count（ME005 改为校验 index_count==triangle_count×3）、[7]=min_vertex_index
- `decode_triangles` 按 `[start_index, start_index+index_count)` 切共享 TA（ME100 改为切片越界校验）
- `decode_vertices` 解码整池（删除 ME200 相等校验——变体 mesh 的 vertex_count ≠ 池大小）
- `decode_mesh` 新增**池顶点重映射**：按索引首次出现顺序收缩顶点、重写索引（mesh#11 导出 581 顶点而非全池 1068）；越界索引报 ME300

### 23.3 效果与性能

- **EP1：4703/4703 mesh 全部解码**（此前 1025）；顶点 2.10M→**6.47M**、三角 1.24M→**3.56M**；DLC0 571/571；**1839 栋双变体建筑获得几何**
- 新解锁网格的构成：dual-UV 模型 376→**2099**、FLOAT4 大坐标 facade 696→**2532**——阶段 4（facade UV）的受益面扩大 3.6 倍
- 导出 sweep：EP1 7.12s/4703 mesh（661 mesh/s，release）；金样本单 mesh 全链 16.67ms（debug；纯几何链路开销与 v1 同量级）

验证：rw4 40 单测（含重写的 ME100 越界断言）+ 3 包 sweep、sc-exporter 9、后端 42、前端 73 全绿。C# 的 ME004/ME100/ME200 在变体网格上同样失败——本修复超越 C# 参照。

## 24. 阶段 3：shader map / AO 通道接入（LOTM v3，2026-09-08 第十八轮）

### 24.1 槽位语义取证（金样本 0x63D180B9 逐槽通道统计）

| 槽位 | 实测 | 官方语义对照（§21.4） |
|---|---|---|
| slot0 | RW4 包裹 type 116 f32 119×4 | 建筑自定义调色板（列=材质元素） |
| slot1 | raster 512×512 RGBA，RGB 多彩/高 A | color control map（RGB=元素区域、A=镂空）✓ |
| slot2 | raster 512×512，R≈247/G≈B≈128，**A≈249** | normal map（RGB=法线、**A=AO**）✓ |
| slot3 | raster 512×512，**B≈29、A≈244** | **shader map（B=spec、A=窗户/Interior 位置）**✓（哑光墙面 spec 低、非窗区占比高） |
| slot4 | RW4 type 21（raw BGRA）**512×16** | **官方标准调色板**（256 列×2px 采样块）✓ |
| slot5 | DXT5 256×256（A≈10） | 细节图（未用，阶段 5 候选） |

### 24.2 实现（LOTM **v3**）

- `resolve_material_resources`：slot2 新增 **AO 灰度 PNG**（取原始 alpha）；slot3 新增 **roughness 灰度 PNG**（B 通道反转——spec 高=粗糙低）；槽位 rgba 一次取用（省跨包查找）
- 容器 v3 = v2 + 每材质 4 张 PNG（base/normal/roughness/ao）；前端 `parseLotModelContainer` v3，材质应用 `roughnessMap`（存在时 roughness=1）与 `aoMap`（three 0.185 默认走 uv0，无需 uv2）
- 窗户透明（shader map A）留阶段 5——需 Interior Map 才有意义

### 24.3 性能

金样本 0x63D180B9：payload 627KB→**880KB**（+roughness/AO 两张 512² 灰度 PNG），全链 16.8ms（debug，v2 16.7ms——PNG 编码增量可忽略）。

验证：workspace 31 个测试二进制全绿；vue-tsc + vitest 73（容器测试更新至 v3 四贴图断言）。

### 24.x 阶段 4 前置取证（进行中，2026-09-08）

- **f4small 模型（~332 栋）：FLOAT4 已是最终 UV**。金样本 0xC2D6D136 实测 f0==f2、f1==f3（双通道同值），范围 [0..2]（含 wrap），v014 干净 (0,1)——当前管线直接采样即可，配合 LUT baseColor 已正确渲染
- **facade 大坐标（2532 栋）：FLOAT4 与世界坐标线性**（逐面片不同斜率 0.18~0.49 UV/m 量级），但穷举尺度搜索（fract(f/s)，s∈[1,1200]，对 slot1/slot5 遮罩红 vs 顶点 D3DCOLOR.G 校验）全部为噪声底（~3%）——**缺的不是尺度而是图集参数**
- 材质段 28B 头/附加数据/尾数据均无裁剪窗（全零/标志位）→ 裁剪窗与图集 cell 参数在 **shader-def 资源**（slot 0x2D 指向 0x259E950F 等，不在 EP1/app/DLC0，需定位全局 shader 包并解析）
- 工具：`facade_probe`（FLOAT4 分布 + 逐顶点 dump + 假设求解器/尺度网格搜索）；前提校验注意：0xC2D6D136 等部分模型顶点**无 D3DCOLOR**，此时 LUT baseColor 是唯一着色

### 24.y 阶段 4 取证修正（2026-09-08 续）

- **f4small/huge 是 mesh 级属性**：0xC2D6D136 主 mesh #5（4563 verts）实为 facade 大坐标（f4 含 ±1900），仅 20 顶点的 mesh #6 是小 UV——此前"模型级"统计有误导，真实贴图覆盖需按 mesh 重算（待做）
- shader-def（EP1 全部材质仅引用 **2 个**：0x259E950F/0x38869BDA）**不在任何游戏包内**（Game/Graphics/App/Locale/RegionTerrain 全查无）——疑似内嵌 SimCity.exe；工具 `shader_def_scan`
- 求解器修复 true modulo（Rust fract 保号 bug）后 facade 尺度搜索仍为噪声；oracle（遮罩红==顶点G）在 facade 上未验证成立，需要一个「已知 UV + D3DCOLOR + 同包遮罩」的阳性对照
- f2/f3 与 f0/f1 存在近常数倍率关系（~2.0-2.2，双通道=同投影不同世界尺度），支持"双 UV 通道 = 两层材质各自投影"模型

### 24.z 阶段 4 深挖（2026-09-08 续二）：VertexFormat 确认 + oracle 证伪

- **VertexFormat ground truth**（金样本 facade，`decode_vertex_format` 新公开方法）：`Position Float3(0) + Normal UByte4(12) + Tangent UByte4(16) + Color D3DCOLOR(20) + TexCoord Float4(24)`——**单个 TexCoord Float4 通道**（stride 40B），f0..f3 同属一层；双 UV 模型才有第二个 TexCoord 元素
- **Float4 = 双层世界投影 (u1,v1,u2,v2)**：斜率反推 cell 尺寸 f0/f1≈5.7×8.4m、f2/f3≈2.6×4.3m（两层 ~2× 关系）
- **oracle 证伪**：fract(f0),fract(f1) 以米为单位 mod 512 后，建筑只覆盖 color-control map 一个 ~60×60px 角落，逐 G 聚类采样点全部落在图的暗色/边界区（RGB 暗橄榄色，非亮色块区域）——**任意尺度的直接采样都不是映射**；映射必须经材质的 cell 矩形（shader-def）
- 目视 slot1 PNG（`slot_dump` 新工具）：确认为 color control map——饱和色块区域（绿/品红/青/红）+ 箭头/圆圈符号，与 modding 教程 256 平面网格完全对应
- **给 exe 逆向的请求**：在 SimCity.exe（ImHex/GHIDRA）中搜索 u32 小端 `0F 95 9E 25`（0x259E950F）与 `DA 9B 86 38`（0x38869BDA）——命中处即内嵌 shader 包/表，是打破 facade UV 瓶颈的钥匙

### 24.w ★★ facade UV 公式破解（2026-09-08，cpp 容器 = 建筑着色器源码库）

**决定性发现**：app 包的 32 个 cpp(0x0469a3f7) 资源 = **游戏 HLSL 着色器源码库**（含 `building4*` 全家族：SetupVS/DefaultVS/DefaultPS/ClipAndReliefMapPS/InteriorMapPS...，以「shader 名 + 源码」成对存储）。用户逆向确认 exe 只是 bootstrap、渲染全在数据包——与 cpp 容器发现互相印证。

**facade 渲染公式**（从 building4DefaultPS/SetupVS 源码逐字提取）：

```hlsl
// VS：uv = In.texcoord0.xy; uv2 = In.texcoord0.zw;（FLOAT4 原样两层 UV）
//     materialIndex = floor(In.color.r * 255)；materialInfoUV 索引 Material Info 贴图
float2 baseUv = frac(uv) * regionXform.xy + regionXform.zw;   // regionXform = 材质裁剪窗(scale.xy+offset.zw)
float4 baseTintValues = tex2D(tintMapSampler, baseUv);        // slot1 color control map
clip(baseTintValues.a - 0.5f);                                // A 通道 = 元素有效区判定（新 oracle！）
// 第二层：relief_tc = frac(uv2)*regionXform2.xy + regionXform2.zw（Top 层 + relief mapping）
```

- 采样器族 6 个 = `materialDataSampler, tintMapSampler(slot1), normalMapSampler(slot2), shaderMapSampler(slot3), tintPaletteSampler, interiorMapSampler`——与 slot0-5 对应关系待最终锁定
- `kPaletteSize = int2(256,8)` + `BuildingPaletteVS`（palU×255/256 移到 2×2 块角）+ `BuildingPaletteVariationVS(buildingType)`（行=建筑变体）——**slot4 (512×16) = 256×8 调色板**（2×2 块），目视 byte 值为灰阶 tint 色
- slot0 (119×4 f32) = **每材质参数表**（每列=材质，4 行 float4）：row0 含调色板 UV（0.352,0.344,0.125,0）、row3 含整数格坐标（(2,1,1,1)/(3,3,1,1)）——regionXform 的来源，行→Xform 映射用 tint.a>0.5 oracle 逐行验证
- EP1 材质仅 2 个 shader 变体值：0x259E950F（facade 大坐标，696+1832 栋）/ 0x38869BDA（小坐标，288 栋）——与 FLOAT4 语义精确相关
- D3DCOLOR 字节序注意：shader 读 `In.color.r`；我们的 {a,r,g,b} 解析与 HLSL 分量的对应需按 materialIndex∈[0,119) 校准

**下一步（实现路径已完全清晰）**：① slot0 各行按 tint.a>0.5 oracle 定位 regionXform/regionXform2 ② 后端烘焙：frac(f0)*X.xy+X.zw → slot1 采样（RGB→palette U、A=镂空）→ 256×8 palette 查色 ③ 双层（uv2）接 Top 层。覆盖率将直达 ~100% 带材质资产。

## 25. 阶段 4 实现完整烘焙链（2026-09-08 第十九轮）

### 25.1 regionXform 行 oracle（决定性）

`facade_probe` 双指标（tint.a>0.5 率 + per-G tint.rg 一致性）穷举 slot0 的 4 行作为 regionXform：

| 行 | alpha>0.5 率 | per-G tint.rg spread |
|---|---|---|
| row0 | 61.5% | 88.26 |
| **row1** | **96.0%** | **16.50** |
| row2 | 65.7% | 7.25 |
| row3 | 59.9% | 93.08 |

→ **regionXform = slot0 row1**（96% 顶点落在 tint 有效区）。最终色烘焙 oracle：xform=row1 时 per-G 颜色离散度 27-40（其他行 96-155）✓。

### 25.2 实现（`build_lot_model_payload` 烘焙链）

- `MaterialBake { params(f32 4行), tint_rgba, palette_rgba }`：resolve 阶段取 slot0 f32 参数表、slot1 原始 RGBA、slot4 原始 RGBA
- `bake_vertex_colors`：逐顶点 `materialIndex = D3DCOLOR.G` → xform=row1、palette 原点=row2 → `baseUv = frac(f0)×X.xy + X.zw` → tint 查表 → `palette[palUV + tint.rg×0.125 + InvSize×0.25] × (tint.b×2)` → **COLOR_0 = 最终 palette 色**（替代旧 row0 色彩误读）；无 bake 数据回退旧 palette-row0 路径
- 前端零改动（COLOR_0 语义升级，vertexColors 照常）；facade mesh 的 hasUv 仍为 false（贴图变换待后续着色器化），但顶点色已带完整 palette 材质色

### 25.3 性能

金样本 0x63D180B9：18.2ms（debug，v3 16.8ms——两次纹理查表 ×4682 顶点，开销可忽略）。

验证：workspace 31 个测试二进制全绿。待用户目检 facade 建筑颜色（此版本顶点色 = palette 查色链完整输出，含 tint.rg 空间渐变与 b 亮度）。

## 26. 阶段 4 完成：逐像素 tint 着色器（LOTM v4，2026-09-08 第二十轮）

用户目检：顶点色烘焙后建筑脱离纯色块，但缺逐像素纹理与法线 → 需要把 building4 公式下沉到片元着色器。

- **容器 v4**：每材质 6 张 PNG（base/normal/rough/ao/**tintRaw**/**palette**）+ **slot0 参数表 f32** + paramCols；每 mesh `u8 uv_kind`（0 无 / 1 常规贴图 / 2 tint 着色器）；GLB 增加 **TEXCOORD_1 = (materialIndex/255, 0)**（gltf.rs `export_glb_with_colors` 新参）
- **前端 tint 着色器**（`attachTintShader`，onBeforeCompile 逐像素复刻 building4）：`texelFetch(paramsMap, (matIndex,1))` 取 regionXform → `tUv = frac(vTintUv)×X.xy + X.zw` → tintMap 采样（**A<0.5 discard 镂空**）→ `paletteMap(palOrigin + tint.rg×0.125 + InvSize×0.25)` 查最终色 ×(tint.b×2)；法线图同 UV 重采样 + perturbNormal2Arb；paramsMap = DataTexture(cols×4, RGBA Float, Nearest)
- uvKind 路由：2 = tinted（vertexColors 关、材质色全由着色器算）；1 = 常规贴图链（LUT baseColor 等）；0 = 白模

性能：金样本 payload 858KB→1.2MB（+tint/palette PNG），22.3ms。覆盖率：**全部带 Float4 材质资产（含 2532 栋 facade）逐像素上色** —— 50% 目标达成且超额。

验证：workspace 31 + 前端 73 全绿（容器测试更新至 v4 六贴图 + uvKind）。

### 25.1 常量修正 + 行绑定最终版（2026-09-08，oracle 双指标定案）

- **kSubsampleScale 修正**：着色器源码实为 `kPaletteInvSize × 0.5`（半物理纹素）+ `kSubsampleOffset × 0.25`——此前误用 0.125 导致采样飞出 cell 64 纹素（绿/红色块根因）。palette 2×2 物理块 = 4 个颜色角点，tint.rg 为**双线性混合位置**、tint.b 为亮度 ×2
- **行绑定定案**（oracle：row1 alpha 率 96% + 逐 G 色打印目检）：regionXform = **row1**、palette cell 原点 = **row0**（(palU,palV) 语义 + 0.125=cell 纵向范围）。烘焙色目检：G=2→浅灰 (227) 大墙面、G=13/15/31→深灰细节、G=6/19/29→黑（窗）——与游戏内截图（灰白消防站）完全吻合
- slot0 四行 = [palette 原点, regionXform(base), regionXform2(top), 整数格参数]

验证：workspace 31 全绿。烘焙色待用户目检。

## 27. 阶段 4 修复：四症状根因清零 + info 诊断浮层（LOTM v5，2026-09-08 第二十一轮）

用户目检 v4 后报四症状：①发绿/发黑 ②墙体/房顶整块消失 ③无玻璃/砖/金属材质 ④看不到法线。本轮用游戏渲染器源码（tmp/shaders/src building4 全家族）逐行对照 + 双探针（`shader_verify_probe`/`shader_sweep_probe`）实证根因后修复：

### 27.1 已实证根因（探针数据见验证记录）

1. **GLB 把 facade UV 清零（症状②④主源）**：gltf.rs 对 Float4>8 的 TEXCOORD 写 (0,0)，而 mesh_uv_kind 用同一条件把这些 mesh 路由进 tint 着色器 → 前端 `frac(0)*xform.xy+xform.zw = xform.zw` **每材质只采样 tint 图一个纹素**，其 alpha 决定整 mesh discard（EP1 sweep 预测 5.4% 整块消失）。修复：**TEXCOORD_2/3 = Float4.xy / .zw（Base/Top 层世界投影 UV，原值直出不取反）**；GLTFLoader r185 支持 uv2/uv3 映射。
2. **发绿主源 = resolve_palette 把 slot0 参数表 f32 当 RGB**：row0=(palU,palU2,0.125,0) → "颜色"B 恒 32 → 绿/黄/红垃圾色喂给顶点色 + LUT。修复：**删除 resolve_palette/mesh_vertex_colors/LUT 上色**；slot0 = 参数表（paletteF32）；无参数表材质 slot1 走 simple diffuse 原图。
3. **raster pixFmt21 = BGRA 存储（翻案）**：pixFmt21=D3DFMT_A8R8G8B8=D3D9 内存 B,G,R,A。§17 时代"RGBA 直读"的依据（与 SCP A 通道视图对拍）不成立——alpha 两序同在 byte3，无法区分 R/B。旧解法把法线图读成全图粉色（数学上非法线图）。修复：**decode_top_mip_rgba 恢复 BGRA→RGBA 重排**；**删除 unswizzle_simcity_normal**（原为补偿错误直读，双重变换抵消）；LotMask 改为重排后直映射（R→color1/G→color2/B→color3/A→color4，优先级 A>R>G>B，与旧交叉映射逐字节等价，5 个单测不变全过）。
4. **tint 亮度通道错位**：源码 tint.b=byte0（BGRA），旧实现用 byte2——byte2<64 占有效区 71.3%（整楼近黑），byte0 仅 3.5%。随 27.3 自动修复。
5. **前端三分歧**：subsample `×0.125`（U 宽 64 倍）→ `×(1/512,1/16)+(1/1024,1/32)`；palOrigin 采 row2 → **row0**（v=0.125）；palV=row0.y → **buildingVariation 行（固定 0）**。tint/palette/normal 贴图 `flipY=false`（与后端 bake 同坐标系，原始 UV 不翻转）。

### 27.2 源码确认语义（决定性）

- `texcoord0 = float4(uv, uv2)`：xy=Base 层、zw=Top 层双 UV（§19.5 "w/h/x/y" 推测作废）
- `materialIndex = floor(In.color.r*255+0.1)`；D3DColor 内存 B,G,R,A → **我们解析的 g 字段 = 游戏 r = materialIndex（旧实现正确）**；a = 游戏 B = interiorTexData（4bit size+4bit index，解开"B 1~80"之谜）
- row0=(palU,palU2,interiorScale,interiorOffset)、row1/2=regionXform(2)、**palV=buildingVariation（实例数据 ×1/8，不在任何表内）**、surface 恒取末行 kSurfacePalV、specE=tint.a
- slot3 shader map 绿色主导（G≈255）、slot5 = 256×256 DXT5 relief（kFlatLevel=23/255 呼应）——Top 层/玻璃阶段输入
- slot4 调色板**非灰阶**（RGB 通道差最大 255，旧记录作废）

### 27.3 容器 v5 + info 诊断浮层

- **LOTM v5**：v4 布局 + 末尾 `u32 diag_len + UTF-8` 诊断文本（mesh↔material 绑定行 + 每 material slot0-4 的 instance/来源包/格式/尺寸）。后端 `find_resource_across_packages_named` 携带来源包名，`resolve_slot_texture` 统一解析+诊断。
- **PE 视口 info 浮层**：右上角 Info 按钮（仿光源调节）→ 诊断卡片，显示诊断文本 + **复制按钮**（Clipboard API + execCommand 兜底），供任意建筑复盘，不再依赖金样本。

性能：金样本 payload 965KB / **17.0ms**（v4 22.3ms，删 LUT 上色后更快）。验证：Rust 全部套件 + 前端 vue-tsc/vitest 74 全绿（容器测试升 v5 + 诊断解析用例）；eslint/prettier 16 项为存量欠账（HEAD 前后一致，本轮净修 1 项）。待用户目检：建筑颜色/完整度/法线凹凸。

## 28. 渲染器全源码调研五问 + 仰视穿透缓解 + 阶段 5 路线（2026-09-08 第二十二轮）

以 tmp/shaders 8 个 cpp 转储（4×8MB 字节码+CTAB 常量表、4×1.2MB HLSL 源码段）+ 26 个复原 building4 HLSL 为据，用户五问全部拿到源码级答案，结论沉淀 **`docs/rendering.md`**（管线总览/窗户内景/材质质感/raster 对位/天空/光源/家族清单/寄存器表）。要点：

- **窗户** = shaderMap.A 窗洞遮罩 + interiorMap 盒体视差投影 + 逐窗格 FastNoise 选房（interiorScale/Offset = D3DCOLOR.B 4bit size+4bit index，与 slot0 row0.zw 同源）；夜间亮窗 interiorMap.a×16，供电 interiorThresholds.z 一票关
- **质感无分支** = 4 标量：specE=tint.a³×2048+1、specStrength=shaderMap.b×2、reflectance=palette surface 行 A、gloss=saturate(a×specStrength)；EnvLighting=天空 LUT 解析合成（非 cubemap）
- **raster↔建筑对位不在着色器侧**（8 转储 footprint/lotRaster 零命中），由 lot 数据 + 引擎决定；偏移=原版行为（§19.3），勿再排查
- **天空** = cSunSkyInfo 11×float4（Perez A–E + mSkyColorTuning + mDynamicWeather 雾双层 + mZenith.w 黑阶）+ s11 压缩 LUT；**光源** = 太阳 + parallel[4]（各带填充光）+ shCoeffs[16] SH + deferredLight 点/聚/线（半角 cos、gel texCUBE、体积雾 ray-march、云影最低 0.45）
- **疑点**：源码 `uv = texcoord0.xy / abs(tileSize)` 后才 frac·regionXform（app 直取 frac）——阶段 5c 用 info 浮层实测 tileSize 定夺

### 28.1 仰视地板穿透：原版瑕疵定性 + 观察器缓解

- 症状：白模仰视正常；渲染后部分楼板消失透视内部（游戏同现，用户确认）
- 根因（新只读探针 `tint_underface_probe`）：building4Clip 的 tint.a<0.5 镂空模板按**外立面 UV** 创作，地板/底面共用 facade UV → 继承窗洞镂空。金样本：tint 窗口矩形内不透明率仅 64.0%，下向面顶点 37.1% 落镂空区（上向 50.0%/侧向 46.3% 同采样一张带洞模板）。游戏相机永在地面/屋顶之上，Maxis 从未处理——数据/管线固有瑕疵。用户补充：部分位置在**白模（几何）阶段亦被预剔除**，同一"玩家不可见"假设贯穿几何与贴图
- 缓解（对源码唯一偏离）：前端 tint shader object 法线 Z<-0.3 豁免镂空 + 跳过调色（白模观感）；墙面窗洞（法线水平）零影响

### 28.2 阶段 5 演进路线（5a → 5d，本轮决策）

| 阶段 | 内容 | 源码依据 | 验收 |
|---|---|---|---|
| **5a 材质质感** | slot3 shaderMap PNG 传前端（后端已解码）；specStrength=b×2、specE=tint.a³×2048+1、reflectance=surface 行 A、gloss=saturate(a×b)；Blinn-Phong-Schlick 太阳高光 + EnvLighting 常数天空近似 | rendering.md §2 | 玻璃锐利高光/砖石摊平；金样本计时 |
| **5b 窗洞+假内景** | 前置：interiorMap 纹理包内定位；shaderMap.A 窗洞混合；逐窗格 FastNoise 选房 + 盒体投影（0.5/0.5/0.9）；interiorScale/Offset；夜间亮灯 × 供电 uniform | rendering.md §1 | 窗内房间图 + 随机亮灯；楼板完整 |
| **5c Top 层 + relief** | Base/Top 双层 lerp（tint/normal/shaderMap，facadeTintValues.a 因子）+ outsideTile 裁剪；slot5 DXT5 relief（kFlatLevel=23/255；本编译版 reliefMap=恒等，按标准 cone/binary-step 补写）；tileSize 疑点实测 | rendering.md §2.4 | 檐口/屋顶几何感 |
| **5d 环境/模式** | parallel[4] → Directional/Hemisphere 近似、夜景亮窗；DataView 纯色渲染模式（可选） | rendering.md §3.2/§4/§5 | 日/夜氛围对比 |

顺序理由：5a 最小成本收尾症状③（质感）；5b 视觉收益最大（窗户是当前最显缺口）；5c 依赖 5a/5b 的双层采样框架；5d 锦上添花。每阶段收尾跑金样本性能报告（惯例）。

验证：vue-tsc + vitest 74 全绿；探针 cargo 编译运行通过。变更：PropertyEditorViewport.vue（缓解）、docs/rendering.md（新）、tint_underface_probe.rs（新）。**打包未重做**（用户指示，待阶段 5a 后一并出包）。

### 28.3 阶段 5a 完成：材质质感 spec 链（LOTM v6，2026-09-08 第二十三轮）

- **LOTM v6** = v5 + 每材质第 7 张 PNG：slot3 shader map **原始 RGBA**（源码语义
  B=specularity、A=窗洞/Interior 留 5b；kind1 simple 路径的 roughness 反转灰度保留不变）。
- **前端 spec 链**（building4DeferredPS 逐字）：`shaderMap.b×2 = specStrength`、
  `palette 色 a ×(tint.b*2) 立方 ×2048+1 = specE`、`palette surface 行（kSurfacePalV=0.875，
  末行）a ×tintMul = reflectance`、`gloss = saturate(色a × specStrength)`、`AO = normalMap.a`
  乘入 diffuse（源码 artistAO）。
- **SimCityLighting 注入**（lights_fragment_end）：太阳 Blinn-Phong-Schlick——半向量
  `normalize(L−V)`、能量归一 `(specE+2)/8`、Schlick 快速式 `exp2(−8.656170·cosLH)`、
  specHighlight 不经 tint 直加（directSpecular）；EnvLighting 常数天空近似
  `uSkyColor × gloss×0.75 × diffuseColor`（indirectSpecular）。豁免的下向面
  specStrength/env 归零（保持白模观感）。
- **占位光照参数**：`uSunDir=(0.35,0.8,0.45)`（Y-up，晴天正午近似）、暖白 uSunColor、
  蓝灰 uSkyColor——游戏为 cSunSkyInfo 日循环，观察器固定值，5d 再议。
- **缓存键**：customProgramCacheKey 区分 TINT_SHADERMAP/TINT_PARAMS define 组合。
- 性能：金样本 v6 payload 1.32MB / **19.7ms** release（v5 965KB/17.0ms，+2.7ms 为
  shader PNG 编码）。验证：src-tauri 42 全绿（v6 容器断言）+ vue-tsc/vitest 74 全绿；
  eslint 存量欠账不变。**待用户本地目检：玻璃/金属锐利高光 vs 砖石摊平、AO 深浅、
  仰视楼板完整**。打包仍未重做（小迭代，用户本地启动验证）。

### 28.4 5a 调试：specularity 实际在 G 通道 + 通道实验开关（第二十四轮）

**症状**：用户目检 5a"质感没有显著变化"。**根因（探针+目视实证）**：源码字面
`shaderMap.b`（SUGC PDF "B=Specularity"）在资产数据里接近全零——玻璃楼 0xCFEC0F84
双材质窗口区 B 均值仅 6.8/20.2，金样本墙面 14.5，specStrength≈0.05~0.16 → 全链无感。
`shader_map_stats --dump` 目视：**G 通道才是作者绘制的 specularity**——玻璃材质窗口
有对角高光笔触（G=159~186 结构化），通用材质有楼层带结构（G=168），金样本窗洞区
G=11（洞内无材质）；palette surface 行（row7）亦 G 主导（玻璃材质 G=157 vs R=1/B=0）。

**修正**：`uSpecG` uniform——`specStrength = mix(shaderMap.b, shaderMap.g, uSpecG)×2`，
默认 1（G=资产实测），0 回溯源码字面 B。这是继下向面豁免后**第二处有意偏离源码**，
依据=资产实证（源码可见段未读 .g，可能在截断部分）。

**UI**：精细模式下渲染开关右侧新增"通道实验"checkbox（specExperiment），开启后显示
`G·数据 / B·源码` 切换组（specChannelG）——watch 热切换全部存活 tint 材质的 uSpecG
uniform，免重建；rebuild 时清空引用表。

**工具链**：`shader_map_stats` 探针（全材质遍历 + clip 窗口逐通道统计 + `--dump`
逐通道窗口 PNG 导出）；新增 `shader-injection.test.ts`——锁定 three r185 注入点存在
（r167+ meshphysical 已改名 physical，`.replace()` 目标缺失是静默失效）。

验证：vue-tsc + vitest 76 全绿（含新注入点测试）。**待用户目检**：通道实验开启后
玻璃楼 0xCFEC0F84 在 G·数据 下应有明显天空反射与高光；B·源码 应接近无高光（对照组）。

**目检反馈（同日）**：G 通道玻璃反光生效，但部分建筑 G 显得画面偏白（gloss→env 项
偏强），部分建筑 B 更自然；样本量不足定论——**通道实验开关保留**，默认仍 G，
后续样本积累后再定默认通道与 env 强度。本轮按用户指示提交并重新打包。

## 29. 阶段 5b：窗洞 + 假内景（LOTM v7，2026-09-08 第二十五轮）

### 29.1 slot5 = interiorMap 定性（关键翻案）

用户目视 `tmp/slots/mat6_slot5.png` 发现"亮着灯的房子"→ **slot5 是 interiorMap
（预渲染房间图集），旧"relief 高度图"解读作废**。图内容 = 暗色房间网格 + 暖黄/冷青
亮灯房间，alpha = 逐窗灯亮通道。building4 六采样器与 slot0-5 一一对应闭合：
materialData(参数表)/tintMap/normalMap/shaderMap/tintPalette/interiorMap。
数据源确认：interiorScale/Offset = 参数表 **row0.zw**、tilePadding/roomInvSize =
**row3**（顶点无 TEXCOORD1-3 流，参数表为准——金样本/玻璃楼顶点盘点实证）；
interiorRandomSeed = D3DCOLOR.B（游戏 A，玻璃楼 81/金样本 1）。

### 29.2 实现

- **LOTM v7** = v6 + 每材质第 8 张 PNG（slot5 interior map 原始 RGBA）+
  **TEXCOORD_1 升 VEC4**（.x=materialIndex/255、.y=内景种子/255）。
- **前端内景链**（ClipAndReliefMapPS + InteriorMapPS 逐字）：
  `artistOpacity = shaderMap.a`（1=外观/0=透内景）→ `interiorUv = vTintUv ×
  roomInvSize`（源码 uv*regionXform.xy*roomInvSize 前两项相消形式）→
  `floor/fract` 逐窗格栅格 → `FastNoise(elem, seed)` 选房（seed<0.5 按模型位置
  munge）→ 4 变体×象限单元格（interiorThresholds v1 用四分位常数）→
  `interiorMap()` 盒体投影（0.5/0.5/0.9）→ `×interiorScale + (0,interiorOffset)`
  采 slot5 图集 → `finalColor = mix(interior, exterior, artistOpacity)`。
  夜灯：`roomTex.rgb × (1 + roomTex.a × uInteriorGlow)`。
- **偏离清单**（累计五处，均文档化）：①下向面豁免镂空；②specularity 取 G 通道；
  ③夜灯 16→uInteriorGlow=6（无 HDR tonemap）；④interiorThresholds 四分位常数
  （引擎值未知）；⑤eyeDir 对象空间近似切线空间。
- eyeDir：顶点阶段 normalMatrix 转置（GLSL1 手工转置）得对象空间视线。

性能：金样本 v7 payload 1.44MB / **19.6ms** release（v6 1.32MB/19.7ms，+117KB 为
interior PNG）。验证：src-tauri 42 全绿（v7 断言）+ vue-tsc/vitest 76 全绿。
**待用户目检**：金样本窗洞（shaderMap.a<128 像素约 4.7%）应透出房间图、部分窗格
亮灯；玻璃楼 0xCFEC0F84 的 shaderMap.a≈1 全关（数据如此，与游戏一致）；
无内景图材质不受影响。打包未重做（小迭代）。

### 29.3 目检修正一：specularity 逐像素双通道（窗玻璃=B/墙面=G）

首版目检：内景可见（晾衣架、空调外机 ✓）但**窗户无玻璃感**——统一取 G 后窗洞
哑光。数据复核：shaderMap 的 G/B 与窗洞掩码 a **互补分布**——金样本墙面 G=168/
窗洞 B=101.5，玻璃楼整体 a≈1 → G=186。语义修正：**B=窗玻璃高光、G=墙面高光**，
specularity 按掩码逐像素选取 `mix(b, g, step(0.5, a))`（此即 SUGC "B=Specularity"
的正确语境——B 只在窗像素上生效）。通道实验开关升级三态：自动（逐像素，默认）/
G·墙/B·窗，uSpecMode uniform 热切换。

### 29.4 目检修正二：内景平涂色块（REPEAT 包装）

用户报"部分楼正确、部分楼窗位出现平涂绿/紫矩形"（0x6DC32BF1 玻璃楼、
0xC4805FD7 砖楼）。根因：这类楼的房间选择偏移超出 [0,1]（如 0xC4805FD7
row0.z=**scale 1.0**、roomInvSize=(1,1) 整图模式，格偏移为整数；0x6DC32BF1 格
偏移至 1.875），游戏 interiorMapSampler 为 **REPEAT 包装**（整数偏移回绕到正确
单元格），而我们统一用 ClampToEdge → 越界采样钳到边缘纯色。修复：interiorTex
单独设 RepeatWrapping。顺带修掉诊断文本过期标签 "LOTM v5"→v7。

## 30. 公寓楼窗户全墙化根因：Top 层未实现 + slot0 列布局纠错（2026-09-09 第二十七轮）

用户四问（公寓楼无窗 / 内景房间变形 / 无立体感 / 材质趋同）重读 26 个 HLSL 后
逐项探针实证（`shader_map_stats` 扩展：参数表只解码一次 + 逐材质块 Base/Top
矩形 A/B 统计 + uv2 范围 + 顶点 materialIndex 分布 + Top 矩形 PNG dump）。

### 30.1 slot0 参数表列布局纠错（影响此前所有探针读数）

slot0 参数表纹理为 **80(材质列)×4(行)**。材质 m 的 row k = `flat[k*80+m]`，
**不是** `flat[m*4+k]`。此前探针的 "regionXform=row1" 实取的是别的材质列的
palU 值（恰好也在 0..1 内，目视难辨）。交叉验证：后端 bake `params[param_cols+m]`
与前端 `DataTexture(cols,4)` 列布局一致且调色链工作正常 → 列布局成立。
新探针 `block_rows()` 已改列布局。

### 30.2 窗户遮罩实证：在 Top 层（row2 矩形）的窗户 motif 里

0xF8FFC5F8 公寓楼（SimCity_Graphics.package，材质节 #11，80 材质块，
mesh#10 5006 顶点，TexCoord FLOAT4 = uv+uv2 均存在，uv2 范围 x -98..86 /
y -168..167 世界投影）：

- **Base 矩形（row1）几乎无窗**：主要材质 slot1/slot3 A<128 占比 ≈0%；
- **Top 矩形（row2）含窗户 motif**：多材质 slot3 A<128 占 36–53%（B≈133
  玻璃标记），slot1 A<128 占 17–34%（镂空/窗框裁剪）；
- motif 结构（暗带检测）：每 Top 矩形 1–2 个大暗矩形（单窗/双扇），
  **不是多窗网格** —— 立面 = Base 砖墙平铺 + Top 窗户 motif 按 uv2 逐格
  重复（`relief_tc = frac(uv2)·regionXform2`），与游戏截图"标准方格窗"吻合。

即源码 `shaderMapSampled = lerp(shaderMapBase@baseUv, shaderMapTop@relief_tc,
facadeTintValues.a)` 的双采样链：**公寓楼窗户只存在于 Top 采样域**，open-scp
从未消费 TEXCOORD_3/row2 → 整面墙。玻璃幕墙楼正常是因为其窗户在 Base 域
即 A<1（row1 矩形）。

### 30.3 修复路线（待实施）

1. 前端消费 TEXCOORD_3（uv2）+ 逐材质 row2 矩形（paramsMap V=0.625）；
   tint/normMap/shaderMap 按 `frac(uv2)·regionXform2` 双采样，
   `facadeTintValues.a`（tint@relief_tc）驱动 lerp；
2. clip 镂空判定补 Top 域（tint.a@relief_tc，源码 outsideTile 逻辑）；
3. 内景格栅 `interiorUv = uv·regionXform.xy·roomInvSize` 中 uv 与 uv2 的
   对齐关系需在实现时目检（窗户 motif 周期 = uv2 格，房间格应与之一致）。

### 30.4 其余三问定性（源码已证，无资产疑点）

- **房间变形/无立体感**：源码 eyeDir 为切线空间（`-mul(tangentSpace,viewPos)`，
  t1 插值器），盒体投影+前 0.9/后 0.5 缩放透视只有切线空间视线才成立；
  open-scp 用对象空间近似 → 投影盒被剪切。修法：顶点按 t3/t4(normal/tangent)
  建切线架变换视线。
- **材质趋同**：无材质分支，四标量参数化（见 rendering.md §2）；open-scp 缺
  Top 层法线（窗框/线脚凹凸在 uv2 域）与 EnvLighting 天空 LUT 的
  diffuse/spec 能量劈分；relief 视差在本编译版为恒等函数。
- **参数表 row0 语义复核**：列布局下 row0=(palU, palU2, interiorScale,
  interiorOffset) 与既有实现一致，未推翻 5b。

### 30.5 普查：Top 层窗户的普遍性（SimCity_Graphics.package 全包扫描）

新探针 `facade_survey`（逐模型：顶点实际使用的材质块 × Base/Top 矩形 A<128
占比 ≥2% 判窗；明细存 `tmp/facade_survey_graphics.txt`）：

- 解析 3496 个 RW4 模型，**1900 个含 facade 链**（slot0+slot1+slot3 齐备）；
- **1900/1900 全部含 uv2（FLOAT4 TexCoord）**，"Top 有窗但无 uv2" 异常 = 0；
- 按材质块计：仅 Top 有窗 623、Base+Top 双域有窗 1024、仅 Base 有窗 58、
  两域无窗 195（屋顶/构件等）；
- 按顶点加权（更抗 2% 阈值误判）：Top 窗顶点占比 >0% 的 1540 栋、≥20% 的
  328 栋；Base 窗 ≥20% 仅 49 栋（真·大面积 Base 窗 ≈ 玻璃幕墙 ≈ 25 栋）。

结论：**带窗建筑约 86%（1647/1900）窗标记出现在 Top 域**，纯 Base 域窗户
（玻璃幕墙式）只占 ~3%——Top 层双采样链不是边角案例而是立面渲染主体，
5b 只实现 Base 采样导致绝大多数建筑"窗户全墙化"与用户观察（玻璃幕墙楼正常、
公寓楼无窗）完全一致。

### 30.6 Top 层双采样实施（待目检，2026-09-09）

PropertyEditorViewport.vue tint 着色器补全源码双采样链（仅窗口渲染，内景
投影修复留待下一轮）：vertex 新增 `uv3`（TEXCOORD_3 = Float4.zw）→
`vTopUv`；fragment 取 paramsMap row2（V=0.625，regionXform2），
`topUv = fract(vTopUv)·xform2.xy + xform2.zw`，`scFacade =
tint.a@topUv`（motif 覆盖率），shaderMap / normalMap（AO+法线）/ palette
色行（Top 用 palU2=row0.y 列 + subsampleTop）/ surface 行 / 亮度 tintMul
全部按 scFacade lerp。xform2.xy≤0 的材质（无 Top 层）自动退化为原 Base
采样。vue-tsc + vitest 76 全绿。**预期**：公寓楼立面出现窗阵（玻璃 spec +
内景混合）；内景投影仍有已知变形（eyeDir 对象空间近似），属下一阶段。

### 30.7 内景修复：格栅分量纠错 + eyeDir 切线空间（待目检）

用户目检 30.6：窗户已出，但（a）单层建筑一窗多房（b）玻璃门出现虚拟房间。
(a) 根因 = 5b 格栅误用 row3.zw（tilePadding+w），源码 t3 =
float4(interiorRoomInvSize, tilePadding) → roomInvSize 在 **row3.xy**；已改
`interiorUv = vTintUv·xform.xy·scRoom.xy`（旧"前两项相消"注释作废）。
(b) 属源码行为（凡 shaderMap.a<1 均透内景，店面玻璃门本就显示内景），
格栅修正后观感应改善，保留。
同时把内景 eyeDir 从对象空间近似改为切线空间：用 vTintUv 屏幕导数重建
与立面 UV 轴对齐的切线架（t=du 方向、b=dv 方向、n=几何法线，view space），
视线向量点积进 (u,v,n) 基——等价源码 `eyeDir = -mul(tangentSpace, viewPos)`
（t1 插值器），盒体裁剪+前 0.9/后 0.5 透视自此成立。vue-tsc + vitest 76 绿。

### 30.8 row3 语义定谳：tilePadding.xy + roomInvSize.zw + outsideTile（待目检）

用户目检 30.7：玻璃幕墙楼（0xBA637D54，60 材质块）丢虚拟房间。探针见其
row3.xy=(79311,76926)（超 half 范围的"垃圾"值）、zw=(1.27,1.70) 合理——与
公寓楼 row3=(0.34,0.27,0.125,0) 恰好互补。回查 cpp_ frac 完整源码定谳：

```hlsl
float2 tilePadding         = In.texcoord3.xy;
float2 interiorRoomInvSize = In.texcoord3.zw;
```

（InteriorAndVariationSetupVS 的 `t3=float4(interiorRoomInvSize, tilePadding)`
注释顺序相反，系变体差异/笔误；资产数据支持 frac 变体。）据此：
- **玻璃楼 padding≈8e4 → `reliefSrc=frac(uv2)·(1+pad)−pad/2` 恒越界 →
  outsideTile>0 → Top 层整体禁用**（数值即语义），窗户+房间全在 Base 域；
- 公寓楼 padding=(0.125,0) → motif 居中 88% 生效。

实现修正（PropertyEditorViewport.vue）：①格栅 `vTintUv·xform.xy·row3.zw`
（30.7 的 .xy 是垃圾值，5b 原 zw 才对，但须乘 xform.xy——旧版缺此因子导致
一窗多房）；②eyeDir 缩放 `scRoom.zwz`；③Top 层补 tilePadding 边界收缩 +
outsideTile→scFacade=0（修复玻璃楼回归）。vue-tsc + vitest 76 绿。

## 31. 阶段演进排期 + 5d 日/夜与供电（2026-09-09 第二十八轮）

### 31.1 后续排期（本轮决策）

| 序 | 项 | 说明 |
|---|---|---|
| 1 | **5d 日/夜 + 供电**（本轮） | 时段滑杆驱动太阳方向/颜色/天空/内景夜灯；供电开关（断电=内景自发光全灭，源码 interiorThresholds.z hack 的观察器版） |
| 2 | Props 真模型渲染（替换红锥） | prop unit → 属性表解析 RW4 实例 → 复用 GLB 管线 + uvKind=1 diffuse 链（generic_static 家族，无参数表） |
| 3 | Decal 地面贴花 + raster flipY 修复 | decal=按 transform 铺 alpha quad（decalProject 家族平面投影近似）；raster=地面 quad+LotMask（定位在 LotPlacementTransform，非几何数据） |
| 4 | relief 视差自研 | 本编译版 reliefMap 恒等，无源码可抄；先探针高度图实际存放（slot2? normalMap 通道?）再投入 |
| 5 | EnvLighting 天空 LUT 能量劈分 / 玻璃 cubemap / impostor | 材质细化与远景（夜景亮窗真源），长期 |

### 31.2 5d 实现（观察器近似，全部前端）

- UI：PropertyEditor 头部（精细模式）时段滑杆 0–24h（默认 12）+ 供电开关；
- 太阳模型：`alt = sin((t−6)/12·π)`，方位角随时刻旋转；太阳色按地平线带
  橙→白、夜间月蓝；天空色 day(0.30,0.42,0.55)→dusk→night(0.015,0.02,0.045)；
- uniform 共享实例热切换（同 specUniformRefs 模式）：uSunDir/uSunColor/uSkyColor
  + 新 uDayLight（0..1）+ uPowered（0/1）；
- 着色：夜间接入 `reflectedLight.*×mix(0.2,1,day)`；内景
  `room.rgb×(mix(0.12,1,day) + a×glow×powered)`，glow=day 时 2.5 / 夜 16
  （源码 HDR 16 的 tonemap 近似）；断电 glow=0、房间仅剩微弱环境；
- key light 同步：setKeyLight(方位角, max(8°, alt·70°)) 与时段联动。

### 31.3 5d 实现记录（待目检）

- PropertyEditor 头部（精细模式）：时段滑杆 0–24h（步 0.5，HH:MM 显示）+
  供电复选；透传 viewport props timeOfDay/powered；
- Viewport `applySun()`：alt=sin((t−6)/12π)、方位角=时刻线速；太阳色
  地平线橙(1,.55,.28)→正午白(1,.97,.9)、夜间月光蓝(.14,.17,.26)；天空
  三段插值 day(.30,.42,.55)/dusk(.24,.18,.20)/night(.016,.022,.05)；
  dayLight=clamp((alt+.08)/.5)；glow=2.5+13.5·(1−day)；
- 共享 uniform 实例（envRefs，一次 rebuild 一组全材质引用）：uSunDir/
  uSunColor/uSkyColor/uDayLight/uPowered/uInteriorGlow 热切换；
- 着色：夜间漫反射 ×mix(.22,1,dayLight)（太阳高光/天空镜面由颜色变暗）；
  内景 `room.rgb×(mix(.12,1,dayLight) + a×glow×powered)`——断电灯全灭、
  夜间房间仅微光、亮灯窗 ×16 近似；环境四灯 ×(.22+.78·day)（亮度滑杆
  叠乘）。key light 方位未随时段联动（手动面板保留主导，偏离记录）。
- vue-tsc + vitest 76 绿；lint 无新增（仓库基线预存 15 条）。

### 31.4 props/decal 链路考证（结论：离线不可达，记录死路）

排期第 2/3 项的资产链取证（新探针 prop_chain_probe / find_instance /
reverse_key_search + sc-registry lookup）：

- **词汇定谳**（s3db）：`0x0C12EF2X = ecoUnitBinDrawBinIDs[1-13]`（每 bin
  一个 Key = prop 原型引用）、`0x0C12EF3X = Transforms`、`0x0C12EF4X =
  Slots`；decal = `0x0D109050 scUnitDecalIDs` / `0x0D109060 Transforms`。
- **死路实证**：BinID key（t=0,g=0，如 0x14984C68/69/6A、0x21F3A153）与
  decalID key（0x3DF339B4 等）在全套已装包（Game/Graphics/DLC0/App/
  DataEP1/RT0/RT1/Cache/Patches/UserData 全部 .package）中**均不存在任何
  类型资源**；s3db instances/properties 表亦无。反向搜索确认唯一引用者就是
  lot 属性文件自身。结论：prop/decal 原型描述子在游戏内部编译的 eco 数据
  表（非 DBPF），离线包浏览无法解析 slot→模型，红锥/绿矩形占位维持。
  可行的后续：社区数据集映射表或运行时抓取。
- **顺手修复**：LotMask 地面贴图补 `flipY=false`（与模型贴图同坐标系，
  消除遮罩南北镜像；rendering.md §3.1 遗留项）。

### 31.5 无 LotSize lot 的地面回退（EP1 0x5A6EC675 案例）

用户报该楼无 raster 地面。取证：该 lot 属性**缺 `0x0CCB7FC8 LotSize`**（前端
地面矩形以 lotSize 为前置 → 整个地面跳过），但 LotMask（0x11257BCB，
128×128 raw RGBA，Graphics 包）与父描述子 `0x0CCB7FD4 "Lot Textures"`
（0x5A59766F = 纯纹理容器，单张 1024² DXT5 地面纹理）均在。mask 尺寸↔LotSize
换算普查（lot_size_fallback_probe）：主流 0.75 m/px（64↔48、128↔96、
256↔192），少数 0.625/1.5（同尺寸多映射）→ 无法精确推导，回退取 0.75
（该楼 bbox 71×57 与 128px→96×96 相容）。

实现：decode_lot_mask_png/entry 返回 (png, dims)；session 组装时
`lot_size = document.lot_size.or(mask_dims × 0.75)`，回退时推诊断消息。
新探针：model_peek / raster_peek / lot_size_fallback_probe / prop_chain_probe /
find_instance / reverse_key_search、sc-registry lookup。

## 32. 资产开发长期路线立项（2026-09-11）

资产开发三方案（插槽式白模/蓝图/标定资产包）评估结论、公共底层能力清单、
Property Editor 模块化改造（注册机制 + 热键 + TransformControls + 复制粘贴）
MVP 计划与执行顺序，已沉淀为独立文档：

→ **`docs/roadmap/asset-development.md`**（长期目标，PE-重构 1-4 与资产 1-6 双主线）

关键结论：三方案为递进关系而非并列；单一最大缺口是 RW4 写回器（纯只读）；
Property Editor 六类 Unit 选中已实现，改造起点是 Viewport 底座/业务分离。

## 33. PE-重构-1：Viewport 底座/业务分离（2026-09-11）

按 `docs/roadmap/asset-development.md` §二 MVP 第 1 步执行，行为零变化
（props/emits/expose 与浮层 UI 均未动）：

- 新增 `useEditorViewport.ts`：视口底座组合式函数——viewer 生命周期、
  拾取→select、rebuild 骨架（清组/防竞态 token/贴图 URL 回收/光源裁剪/
  收尾构图）、可见性与选中应用、captureRender。业务场景装配通过
  `rebuild(assembly)` 回调（scene contributor 的最小形态）注入。
- 新增 `refinedRender.ts`：SCP 精细渲染模块（~600 行 tint shader 注入、
  5d 日/夜环境 uniform、tint 贴图预载、tint/顶点色材质装配、逐 mesh
  材质贴图绑定），纯 TS 无 Vue。
- 新增 `editorGround.ts`：LotSize 地面矩形、placement 逆矩阵、LotMask
  四色量化贴图。
- 新增 `sceneProfile.ts`：模块注册机制骨架——`PropertyEditorModule`/
  热键表/功能开关 + 三形态静态清单（preview / property-edit / rw4-edit），
  编译期组合不做运行时注册；热键与 command 执行器待 PE-重构-2/3 接入。
- `PropertyEditorViewport.vue` 1369 → ~880 行，重写为组合层（装配
  refinedRender + editorGround + unitGizmos + 浮层 UI）。

验证：vue-tsc、eslint（仅 PropertyEditor.vue 预存 warning）、
unitGizmos 5 测试、vite build 全通过。preview 形态（PropertyPreview）
与编辑形态共用同一 Viewport，行为不变。

### 33.1 PE-重构-2：session 本地编辑层（2026-09-11）

- 新增 `unitEditLayer.ts`：`createUnitEditLayer()` = transform override
  （unitId → WPF 行主序 12 floats，reactive Map）+ undo/redo command 栈
  （version ref 驱动 canUndo/canRedo/editCount）。`setUnitTransform` 在
  push 时立即生效；`mergeUnitOverrides` 生成「后端数据 + 本地 override」
  合并视图（无 override 的 unit 保持引用相等；pathPoint 无 transform 跳过）。
- 纯函数矩阵转换：`threeToRowMajor`（three 列主序 elements → 行主序 12
  floats，unitMatrix 的逆映射）与 `translateRowMajor`（世界空间平移，
  行向量约定平移在索引 9-11）。
- `usePropertyEditorSession` 接入：grouping/flatUnits 走合并视图，
  load() 时 `edit.reset()`，返回值新增 `edit`。UI 暂未消费（PE-重构-3
  手柄拖拽结束与 PE-重构-4 粘贴接入 command）。
- 新增 `unitEditLayer.test.ts` 8 测试：T·R·S roundtrip（写回命门）、
  decompose 等价、平移与 three 世界平移一致、命令栈 undo/redo/reset、
  合并视图引用保持。MVP 不写回 DBPF。

验证：vue-tsc、eslint、13 测试全过。

### 33.2 PE-重构-3：热键 + 工具栏 + TransformControls + 检查器三 tab（2026-09-11）

- **竖向 tab 检查器**：`PropertyEditorInspector.vue` 取代原只读属性面板——
  属性（原 PropertyEditorProperties）/ 坐标（TransformPanel：位置 XYZ 可
  编辑、旋转/缩放只读展示，编辑走手柄）/ 元数据（MetadataPanel：MVP 支持
  light 类型/RGB/内外半径/漫射/长度编辑；其余类型只读提示，decal 暂缓）。
- **编辑层扩展**：unitEditLayer 新增字段 override（`setUnitFields` patch
  命令，自动剔除 transform 字段防越权），mergeUnitOverrides 合并两层。
- **热键**：`useEditorHotkeys` scoped keydown（capture），输入框内不触发
  （Esc 除外）；G/R/S 切工具、Esc 降级（先退工具再清选中）、Ctrl+Z /
  Ctrl+Shift+Z 撤销重做。sheet 打开且会话就绪才挂载。
- **左侧工具栏**：视口左缘垂直工具条（选择/移动/旋转/缩放，Blender 形态）。
- **TransformControls**（three r185 examples，动态 import）：
  - `three-viewer.ts` 新增 `domElement` getter 与 `setOrbitEnabled` 输入门控；
    手柄 dragging-changed 挂起/恢复轨道相机，解决输入抢占。
  - helper 加 scene 根（相机空间朝向，attached 对象在 Z-up world 组内由
    TransformControls 负责父空间换算）。
  - 拖拽结束 compose(pos/quat/scale) → threeToRowMajor → emit
    commit-transform → 壳落 edit.setUnitTransform（一次拖拽一条命令）；
    起止矩阵相同不产生冗余命令。
  - `useEditorViewport` 暴露 unitObjects 与 rebuild revision（选中对象被
    重建后手柄按 [tool, selectedId, revision] 重挂）。
- **编辑状态徽标**：header 显示"本地编辑 N"（editCount>0）替代只读徽标。
- i18n zh/en 新增 17 键（tab/工具/坐标/元数据/本地编辑）。

验证：vue-tsc、eslint（仅存量 warning）、13 测试、vite build 全过。
手柄拖拽与轨道相机的实测对拍待用户验收（WebView2 输入协调为本步最大风险）。

## 34. 未知类型取证：State Script / Terrain 16-bit / EP1 ER2（2026-09-12）

新增 `sc-exporter/examples/tgi_peek`（TGI 通配抽头 + gzip 展开 + 字符串
扫描 + 类型清单）对真实安装包取证，结论与落地：

- **0x024A0E52**（官方误拼 "Uken File (Property/Spore?)"）：内容实证为
  游戏状态机脚本纯文本（app.package 3 条，官方 viewer=viewText）。
  → 命名 "State Script"；resourceKind 归入 text（高亮 ini）。
- **0x03E421F0 Greyscale 16-bit**：Game 包实例 0x2096ae79 头部破解为
  20B 大端头 + channel_code=**7** 的大端 u16 单通道（256²、
  131092=20+256×256×2 吻合）→ decode_greyscale 新增 code 7 解码
  （高 8 位灰度近似），16 位地形高度图可预览。EC/ED/F0 更名为
  Terrain Field/Height Map。新增解码测试。
- **0x08068AED / 0x08068AEE**（EP1 独有，s3db 无名）：AED = gzip 包裹
  2.2MB 稀疏数据表（哈希数组+计数字段）；AEE = 12 字节记录表（0x410/411
  序号 + 位模式字段）。类型号紧邻官方 ER2 Rule File（0x08068AEB/AC），
  按 ER2 系列 EP1 变体命名 "EP1 ER2 Rule Data (gzip)" / "EP1 ER2 Rule
  Table"；保持 hex 预览。
- 命名机制：package_service 新增 `verified_type_name`（实证名优先于
  s3db 注册表），stats.rs KNOWN_EXTENSIONS 同步；knowledge 文档更新。

验证：cargo check/test（greyscale 3 测试）、vue-tsc、17 前端测试、build 全过。

### 34.1 存档结构实证 + EcoGame 误判修正（2026-09-12）

用户离线开局产出真实存档，`Games\<GUID>\1\` 结构取证：

- `misc/MetaData`：**纯 JSON 区域清单**——boxes[]（uid 1026-1030 城市/
  大工程槽位、type=CITY、isClaimed、regionTemplateName "Confluence"、
  resources 含 simoleons/mayor_Rating）。
- `<uid>\state_file_<n>_<ts>.egb`：**gzip(3.4MB) EcoGame 城市状态快照**，
  解压头 `00 00 00 14 62 2b 9c d7 …` 与 EP1 0x08068AED 载荷同魔数
  ——**修正 §34 的 ER2 误判**：AED/AEE 更名 EcoGame State Data/Table
  （package_service/stats/knowledge 同步）。`0\` 目录为区域级状态。
- `1029\0x<hex>`：JSON 键值表（u32 hash → 计数）。
- `SLDelta{2,3,4,6}_1.mfs`：二进制增量（头部 `04 00 00 00 ff 02 03 00`，
  语义待定，疑地形/流式增量）。

结论：存档 = JSON 元数据 + gzip 二进制状态，无 DBPF 层（与
SimCityUserData 用户数据不同）；扫描器只需 JSON 解析 + gzip 展开。

### 34.2 地面渲染 v2：真实 Lot Textures 图集（2026-09-12）

按 shader 实证（34.1 节 + raster-lot-decal-analysis.md §2）落地：

- 后端：`LotEditorDocument` 新增 `lot_textures`（0x0CCB7FD4 "Lot
  Textures"）；session 新增 `lotSurfacePng`——跨包定位纯纹理 RW4
  （图书馆实例 0xA0DA9E6C，SimCity_Game 692KB）→ DXT5 解码 → PNG。
- 前端 `composeRefinedGround` v2：真实图集（4×4 tile）按 LotColor.A
  索引取样，mask 四通道软权重混合 Σ w_i·(tile_{A_i} × LotColor_i.RGB)；
  surface 缺失回退 v1 本地占位 tile。DTO 链（tauri/session/Editor/
  Viewport/editorGround）全通。
- 地面 fill 改 FrontSide + alphaTest 1/255（对拍"正反面"+ 引擎 clip
  语义）。
- 180° 翻转修复与 v2 同批待用户对拍。

验证：cargo check/test（54）、vue-tsc、17 前端测试、build 全过。

## 35. GlassBox 引擎五大子系统深扫（2026-09-12）

基于 docs/source-code（348 文件）五路并行深扫，产物 docs/overview/glass-box/
（README 概述 + eco-swarm / transport / zoning-lot / terrain / disaster 五文档）。
核心结论：

- **Agent 两形态**：交通侧显式实体（薄基类+池槽位承载状态，逐段续约无全程
  寻路）；经济侧无 agent（场 SwarmMap + beat 规则表承担，"agent 是数据"）。
- **交通算法**：三层管道池 + 容量-队列拥堵传播（min 沿邻接边扩散）+ LCG
  择向 + 信号组 mod-4 相位；11 模式 GUID 数据驱动注册。
- **驱动脚本**：行为参数 100% property 化（type tag 校验+默认值回退）；
  调参文件零代码；GCT 脚本在数据包（OpenSCP property 编辑器=模拟调参器）。
- **Terrain**：256×256=65536 实证、13 张 typed map、Eco/G 双副本共享持有。
- **Zoning/Lot**：lot=corner 对之间的带（非网格）；parcel 0x4C、lot 表 0x108
  步长（**修正**：所谓"0x5c lot 数组"实为 transport 站点数组）；成长=每帧
  带时间预算的随机抽取+废弃兜底。
- **Disaster**：switch case 1..13 实证；破坏=状态标记+计时+属性随机，无
  血量；触发面收敛于 FUN_00702770 单函数。
- **横切**：32 位属性 hash 全库通用（FNV/CRC32 不匹配，私有 hash 待破）；
  COM 双 vtable+侵入式引用计数；fourCC 子系统注册表连续区段。

## 36. Decal Dictionary（贴花图鉴）相册浏览（2026-09-12 第三十六轮）

原 SCP 能对 decal atlas 显示网格相册，OpenSCP 此前只有属性表。核对原 SCP v2
源码（`D:\source-map\Simcitypak-v2`）后确认机制并实现。

**机制（源码实证）**

- 「decal atlas」不是图片，而是普通 Property 资源（`0x00B1B104`），由
  **GroupContainer 低 16 位**区分：`0xB185` / `0x1651` / `0x1652`
  （`InstanceTypeIconConverter.cs` 的 `DecalAtlas{1,2,3}`；`ViewSelector.cs`
  据此选择 `viewDecalDictionary`）。
- 载荷为 **列式并行数组**（无条目计数字段）：字典级 `MaterialId 0x0CE5EF4E`、
  `TextureSize 0x0CE5EF4F`、`AtlasSize 0x0CE5EF60`；条目级 7 个数组
  `ID 0x0CE5EF50`、`AspectRatio 0x0CE5EF53`、`RasterFileID 0x0CE5EF58`、
  `Color1..4 0x0CE5EF5C..5F`，条目 *i* = 各数组下标 *i* 的元组。
- 每个条目的 `RasterFileID.InstanceId` 指向一个 `0x2F4E681C` Raster；预览用条目
  自带四色还原量化/SDF 图。通道映射与 LotMask 同源，可直接复用已验证的
  `RasterImage::decode_lot_mask_rgba`（`crates/rw4/src/raster.rs:106-121`）：
  传 `[Color1..Color4]` 即得原 SCP 结果。
- 颜色存的是**线性值的一半**（C# 侧 `X = color.ScR/2`、显示 `FromScRgb(1, X*2)`），
  故还原需 `linear_to_srgb(X*2)`；直接 `X*2*255` 会偏暗。

**真实数据验证（SimCity_Game.package）**

- 7 个 decal 字典：group 低 16 位覆盖 `0x1651` / `0x1652` / `0xb185`（后者完整
  group = `0xc67fb185`，与 C# `DecalDictionaryGroup` 常量一致）。条目数
  29 / 149 / 190 / 247 / 254 / 385 / 440，7 个数组全部等长。
- 其中 `0xfb661652-0xeefd390c`（material `0xe5390a98`、textureSize 32×32、
  atlasSize 512×512）与原始 SCP 截图完全对应：条目 0..4 依次为
  `0xb05945fb` / `0x224a5eba` / `0x23585701` / `0xc3921bf9` / `0x1b015691`，
  **ID 与显示顺序逐项一致**。
- 图片尺寸确实不一致：32×32 / 64×64 / 32×64 / 64×32 / 128×32 / 256×32 /
  64×128 / 128×128，aspect ratio 0.5–8，印证「原 SCP 无法保证每张一样大」。
- Raster 跨包分布：主要来自 `SimCity_Graphics.package`，其余在
  `SimCityDataEP1.package` / `SimCity_DLC0.package`。只开 Game+Graphics 时
  命中率仅 6–68%，把 SimCityData 主要包都打开后升至 99%+。
- 导出 PNG 目视比对：`0xb05945fb` → 金色 MODERN + 蓝色 INDUSTRY；
  `0x23585701` → 青色网纹 + 金色圆点；`0xc016bd89` → 深色工业塔 + 青色 MECH。
  与原 SCP 截图逐张一致，确认四色映射与 sRGB 转换正确。

**修正此前死路结论**

§27 与 `docs/overview/raster-lot-decal-analysis.md` 记录「decalID key 如
`0x3DF339B4` 在全部 .package 中无对应资源，原型不可达」。实测修正：该 ID 是
字典 `0xc67fb185-0x1813da18`（29 条目、仅含 Color1、material `0x4491de3a`、
atlasSize 1024×512）的**第 9 号条目**，条目本身可达可解析；不在基础安装中的是
它引用的 **Raster**（`0x657856f5`），该字典 29 条的 Raster 全部缺失。故「字典
可离线解析」成立，仅个别字典的纹理资源需 DLC 或运行时补全。

**实现**

- 新增 `crates/sc-properties/src/decal.rs`：`DecalDictionary` / `DecalEntry` /
  `is_decal_dictionary_group` / `looks_like_dictionary`。并行数组长度不一致时
  以最长数组为条目数、越界字段记 `None`（原 SCP 会直接抛异常；只读浏览选择
  更宽容，并上报 `uniform_arrays` 与各数组长度）。
- 新增两个 Tauri 命令（`src-tauri/src/package_service.rs`）：
  `read_decal_dictionary`（只解析 header，不解像素）与 `read_decal_images`
  （按下标批量解码，单次上限 256，逐条独立失败）。
- 前端：`DecalDictionaryGallery.vue`（元数据头 + 内部检索 + 自适应 4~5 列 +
  IntersectionObserver 懒加载 + 批量 8）、`DecalImageViewerSheet.vue`（外侧
  Sheet 复用 `ImagePreview.vue` 的缩放/旋转，隐藏资源导出）。
  `PropertyPreview.vue` 按 group 低 16 位自动切换相册/属性表，可手动切回。
- 探针 `crates/sc-exporter/examples/decal_probe.rs`：分布统计 + 逐条状态 +
  `--only=<group:instance>` 定位 + `--out=<dir>` 导出 PNG 供目视比对。

**测试与性能报告（Windows 11，release）**

- 单元测试：`cargo test -p sc-properties --lib` 37 通过（新增 6 项：group 判定、
  并行数组装配、长度不一致容错、空数组、线性→sRGB、坏载荷拒绝）。
- 前端：`pnpm check`（vue-tsc）通过；改动文件 `pnpm lint` 无错误。
- 计时环境：release 构建、探针镜像两个命令、4 个包已打开
  （Game / Graphics / EP1 / DLC0）。探针含逐条控制台输出，实际命令略快。

| 项目 | 耗时 | 说明 |
|---|---|---|
| 基线（仅开包 + 索引解析） | 150 ms | 应用内包常驻，不计入单次命令 |
| 元数据 440 条目 | 219–233 ms | 扣基线 ≈ 70–85 ms |
| 元数据 29 条目（Raster 全缺） | 123–189 ms | 无跨包查找，≈基线 |
| 缩略图 40 张（32×32–256×32） | 35–78 ms | ≈ 0.9–2.0 ms/张（含 PNG 编码） |
| 单批 8 张（前端 BATCH_SIZE） | 8–16 ms | 首屏 ~24 格 ≈ 3 批 |
| PNG 体积 | 均值 1223 B | 40 张约 49 KB |

- **热点**：跨包查找 `find_resource_across_packages_named` 为线性扫描包内索引，
  440 条目 ≈ 0.19 ms/条。字典更大或包内索引更大时线性增长；后续可用
  instance→entry 索引表（同 `sc-exporter/src/texture_index.rs` 思路）优化。
- 既有问题（非本轮引入）：`crates/sc-properties/examples/lot_mask_probe.rs:64`
  向 `decode_lot_mask_rgba` 传 3 元素数组，导致 `cargo test -p sc-properties`
  在编译 example 阶段失败；`--lib` 不受影响。已单独记录，未在本轮改动。

## 37. Raster 通道预览组件重做（2026-09-12 第三十七轮）

**问题**：OpenSCP 此前对 Raster 一律按原始 RGBA 直出（`decode_top_mip_rgba`）。
四层量化/SDF 类 Raster（decal、LotMask）的四个通道是**数据**而非颜色，直出后
α 常为 0 导致整图近乎透明，R/G/B 的 SDF 渐变又呈平滑彩虹——观感即「只剩一个
通道 + 整体模糊」。对照 0xb05945fb（MODERN INDUSTRY 贴花）实测确认：模糊不是
缩放造成（`image-rendering: pixelated` 一直生效），而是数据本身。

**原 SCP 机制**（`Views/ViewRaster.xaml` + `viewHelpers/RasterImage.cs`）：
底部 Display Channel 单选组提供 **Preview / All / A / R / G / B / Facade Color**
七种模式，且 `Stretch="None"` 按原始像素 1:1 显示。`Preview` 是默认项，即四层
量化：按 `A > R > G > B` 阈值 ≥128 命中 color1..color4，未命中处透明。

**重做设计**（6 视图，默认对齐 SCP 的 Preview）：

| 视图 | 语义 |
|---|---|
| 四层量化 `quantized` | 默认。黑/红/绿/蓝四色，硬边，decal/LotMask 可读 |
| 合成 `composite` | RGB 直出但 **alpha 强制不透明**（alpha 多为数据通道） |
| `r` / `g` / `b` / `a` | 单通道**灰度**（比原版的红/绿/蓝着色更易读） |

省略 `Facade Color`（原工具的建筑作者辅助分色，非通用预览需要）。

**颜色下标坑位**（沿用 §36 结论）：`decode_lot_mask_rgba(colors)` 的下标 0..3
依次是 SCP 的 color4 / color3 / color2 / color1，故默认配色常量
`DEFAULT_QUANTIZED_COLORS` 按 **[蓝, 绿, 红, 黑]** 存放，对应 SCP 的
color1..4 = 黑 / 红 / 绿 / 蓝。

**实现**：`crates/rw4/src/raster.rs` 新增 `RasterView` / `render_view` /
`DEFAULT_QUANTIZED_COLORS`；`read_raster_preview` 增加 `channel` 参数
（缺省 quantized，非法值报 InvalidArgument）并在响应中回显 `channel` 供前端
按视图缓存；前端新增 `RasterPreview.vue`（nav bar + 逐视图缓存 + 切换期间保留
旧图避免闪烁），`previewResource` 对 Raster 改返回 `kind: "raster"`，由
`ResourcePreview.vue` 路由。视图缩放仍复用 `ImagePreview.vue`（`pixelated`）。

**验证**：`cargo test -p rw4 --lib` 44 项通过（新增 4 项：合成强制不透明、
四通道灰度、四层量化交叉配色、视图名往返与非法名拒绝）。真实数据
（`SimCity_Graphics.package` 的 0xb05945fb，64×128 pixFmt21）六视图导出目视：
quantized 为硬边四色 MODERN INDUSTRY；composite 不透明且可见真实 RGB 数据；
R / A 各自呈灰度 SDF 层。探针：`cargo run -p sc-exporter --release --example
raster_views -- <package> <instance_hex> <out_dir>`。

**计时**（release）：6 视图合计 160–227 ms，其中绝大部分是开包（≈150 ms）与
6 次文件写；应用内包常驻，切换成本为单次 PNG 编码 + 一次 IPC，且已切换过的
视图在前端缓存，回切无请求。
