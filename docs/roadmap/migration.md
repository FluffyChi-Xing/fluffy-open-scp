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
- [x] 运行时媒体工具探测与导出接线：Wwise Vorbis → WAV（vgmstream bundled 路径）、EA VP6 → VP6/MP4（ffmpeg bundled 优先、PATH 回退），固定参数数组、输出签名校验与临时文件清理（对齐 C# `FindFfmpeg`）
- [ ] vgmstream/ffmpeg 二进制随应用分发、Tauri bundle/externalBin、许可证和 x64/arm64 发布验证（留至 M6）
- [ ] chat-assistant 网关：**未来功能，本阶段不建设；前端开发阶段暂时禁用**

**M5 后端进度（2026-09-05）**：已完成包句柄生命周期与容量保护、包摘要与资源分页、4KB 字节范围读取、Registry 缓存名称解析、raw/OBJ/GLB/PNG/JPG/TGA/DDS/WAV/VP6/MP4 异步导出、骨骼/动画接线、媒体工具运行时探测、原子临时文件落盘、activity 埋点和 `export:progress`/job 状态查询。后端单元测试 19 项通过；workspace 全量测试通过。前端页面、sidecar 二进制分发、完整多 Mesh/多材质跨 package glTF 与 chat-assistant 保持未建设。
- [x] 前端首版源文件解析工作区：目录树、当前目录 package 列表、多个 package Tab、资源分类 Tabs、默认空态和 5174 Mock 验证
- [ ] 前端多格式详情预览：文本/代码 FCode、图片、音频、视频、Three.js 模型预览
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
