# 最小 Mod package 与完整性检测：证据和设计边界

后续运行时更新：2026-10-17 已完成实际安装的单 TGI 双目录对照，1,369 字节无 metadata 包在 Data 注册并命中；用户 Packages 注册但未覆盖。见 [实机报告](package-runtime-verification-20261017.md)。下文“尚未验收”描述的是静态调查阶段。

日期：2026-10-17。此次仅分析与修订文档，未修改产品代码、游戏包或正在运行的游戏。dev 根目录为 `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src`；这是带符号反编译代码，非原始 C++。下面 dev 地址不是 retail 地址，本轮已完成下述 retail dump 验证分支的机器码对拍；尚未进行游戏运行时验收。

## 1. 最小普通包不要求 package-info

`SP/AddPackageIfValid.c`，dev VA `0x4A2C20`，第 71–88、250–275 行：对 Property type `0x00B1B104` 与 `kPackageInfoGroup_1` 做过滤，按匹配数量分支。

| 匹配 metadata 条数 | dev 路径 | 对导出的意义 |
|---|---|---|
| 0 | `LABEL_52` 取调用方 priority，进入 `RegisterDatabase` | 普通 overlay 无须额外生成官方 package-info |
| 1 | 打开 metadata，调用签名校验，读 Property，执行条件授权检查 | 不应盲目复制官方 metadata 到自制最小包 |
| >1 | `LABEL_68` Shutdown 并返回 false | 多份官方 metadata 拼包可能触发直接拒绝 |

另一个观测边界：dev `RegisterDatabase` 的返回值未检查，随后直接 return 1；retail `0x45F133` 调用后也未据返回值分支。因此 AddPackageIfValid=true 仅能作为此函数接受/尝试注册信号，不能替代 Attach 成功或最终命中证据。

函数输入已是打开的数据库；路径重载 `SP/AddPackageIfValid__2.c`（`0x4A3690`）先 Init、SetLocation、Open 后才调用本函数。因此“0 metadata 可注册”不是无视 DBPF 完整性的通行证。

1 条 metadata 分支的字段：package ID `0x06EF59E7`（Uint32 或 Key instance）；entitlement 开关 `0x06EF59EB`；capabilities 数组 `0x06EF59E6`（非空全局 capabilities 时检查至少一项匹配）；Bless 开关 `0x06EF59E8`，可进一步读取签名 Bless Property、产品/注册表数据与磁盘指纹。合法 metadata 的 `0x06EF59E5` 若为 Int32（tag 9），会覆写调用方 priority。普通 overlay 不应伪造此结构来请求更高优先级。

`SP/PropertyListHasValidSignature.c`（`0x4A29F0`）检查首 Property ID 为 0 的签名字节，SHA-1 + RSA verify，随后复位流位置。签名覆盖此 Property 流的剩余 payload，不能称为“整个 package 每条资源的完整性签名”。反编译调用参数有错位伪影，dev 单独阅读难以确认 `allowNoSignature` 实参；下面 retail 机器码已确认对应调用传 false。

### Retail dump 已核验分支和 group 实值

样本 `D:/ea-games/simcity_offline/SimCity_dump_SCY.exe`，SHA-256 `06d681444011f1297a6cf53bc233ee5f138d41786b451e6f18dc02bf273ff569`。主调查保存的机器码反汇编见 `tmp/package-loader-20261017/validate-disasm.txt`，本子调查已读取核对；不是声称 IDA 自动分析或反编译已完成。

- 对应入口 VA `0x45EC40` / RVA `0x5EC40`；`0x45EC53` 读取全局 VA `0xDE1C68` 作为 group，type 常量 `0x00B1B104` 写于 `0x45EC9B`。
- 独立按 PE section 将 `0xDE1C68` 映射到文件 offset `0x9E0A68`，读得 `0x40404000`。因此这个 retail dump 的 package-info 筛选键为 **Property type 0x00B1B104 + group 0x40404000**，instance 通配。
- `0x45ECA7 je 0x45F11C` 为 0 条 metadata 分支，使用调用方 priority 注册；`0x45ECB0 jne 0x45F1C6` 为非 1 条拒绝分支。确认 dev 的 0/1/>1 三分规则在此 retail 样本保留。
- 1 条分支在 `0x45ECE5`、`0x45ECE6`、`0x45ECEB` 依次压入 0、128、公钥地址，然后取得流，`0x45ECF3` 调用 `0x45EA80`。与 dev 已知签名对应，确认此调用 `allowNoSignature=false`。

后续主调查用独立 PE 映射确认两个指定 EXE 的该函数有界区间与 group 值逐字节相同，记录见 `tmp/package-loader-20261017/verified/manifest.json`；这不代表运行中内存未被修改。IDA 9.2 定点反编译亦已完成（`ida-0045EC40.c`），与上述分支一致。`kEcosystemGroup` 实值仍须单独闭合。

收尾补证：retail Ecosystem group已确定为0x40800200/+1；Header末尾可疑返回分支已机器码证实，不能把游戏的Header接受结果等同严格完整性。详见 `package-ecogame-resolution-closure.md` 与 `package-loading-open-questions.md`。

## 2. EcoGame 的 10 是类型标签，不是版本号

`anonymous/AddGamePackage.c`（`0x5ED2B0`），第 39–88 行：

1. Open database，依次找 `kEcosystemGroup`、`kEcosystemGroup+1`，首个非空 group 停止。
2. 取首个匹配 key，`gameID = key.mInstance`，不是从 metadata 字段读取 game ID。
3. 读 Property `0x09EEB11C`（166637852）；`*(variant+18) == 10` 检查 Variant 类型。
4. `operator<unsigned int>` 才取真正版本值，存入 `mVersions[version]`，若较大则更新 `mLatestVersion`。

`crates/sc-properties/src/model.rs:31` 同样把 tag 10 映射 UInt32。没有 `version == 10` 限制。该函数不检查唯一 metadata：它消费首个匹配条目。因此生态包 profile 的多个候选应诊断为选择歧义，不可照搬普通 package-info 的多条拒绝规则。相同 gameID/version 的后一次赋值会替换版本表中的数据库引用；这不是普通 ResourceMan 的同 TGI 查找规则。

## 3. 已有 writer 足以构造最小候选包

后续修复：旧 writer 的 28 字节条目未在 stored-size 设置 `0x80000000`，导致游戏多条解析错位。2026-10-17 已修复 Rust/TS 两侧，输出尺寸公式不变。项目 parser 能读旧错误输出不代表实机兼容，见 [cape 对照](package-cape-index-sizeflag-20261017.md)。

`crates/dbpf/src/writer.rs` 已输出确定性未压缩 DBPF：96 字节头，8 字节索引前导，28 字节/条索引，随后资源内容。理论输出长度为 `104 + 28*N + sum(payload lengths)`；单条资源是 `132 + payload length` 字节。无需复制整份源 package。writer 按 TGI 排序、拒绝输出内重复 TGI，固定 major 3/minor 0，index key type/保留字段为 3。

“只改几个字段”仍应重建这些 TGI 对应的完整资源字节，不应把字段差分直接当资源写入。`src-tauri/src/package_service.rs:4300` 的 `patch_property_overlay` 已读取源 Property、应用 patch、序列化整条 Property，并只导出一条 TGI；`mod_flow.rs:1062` 已写包、重新打开、逐 TGI 比较内容并记录摘要。因此后续工作应统一构建/校验服务，复用已有 writer，而不是再造 DBPF 写入器。

最小候选策略：修改的完整 TGI + 本次新增且必须随包的依赖；保留可解析的基础游戏外部依赖，并记录其来源/摘要。不要把全部依赖递归复制成“大包”，不要自动复制 package-info。新增 TGI 必须有消费引用；同 TGI 修改必须验证实际优先级及缓存。源包完整性、结构通过、注册通过、最终命中、行为正确是不同状态。

## 4. 当前解析器不等于严格完整性校验器

| 层级 | 已有代码行为 | 建议新增诊断 |
|---|---|---|
| Header | header.rs 检查 magic/长度/key-type 字段，读取版本 | profile 兼容性；注意当前代码并未验证 major/minor |
| Index | index.rs 解析共享字段与 compression flags | 限制 entry 数量/内存预算；解析必须受声明 index_size 约束；checked offset/size；重复 TGI |
| Payload | Package::read_raw 访问时检查文件范围，read 解压 | open 后全量扫描范围；payload 与 header/index 重叠；条目互相重叠作为诊断，区分合法别名与损坏 |
| Resource | 已有 Property/地图/Locale/RW4 等解析器 | 完整解码预算；资源类型与实际格式；Property claimed_count 与实际条数；未知资源保持 opaque |
| Dependencies | 编辑器已有部分语义关系 | unresolved 与外部基础包依赖分别报告，不能一概视为缺失 |
| Admission | 无通用游戏注册检查报告 | metadata 0/1/>1 分支；单独 EcoGame profile；retail 未确认应 unknown |
| Resolution | override_scan 为工具候选模型 | 记录注册 priority/顺序、命中源、缓存状态，禁止直接以工具 wins 宣称游戏命中 |

具体缺口：index.rs 以不可信 `header.index_count` 直接 `Vec::with_capacity`；Reader 的界限是整个文件，不是声明 index 区间；header.rs 的“负 count 当 0”注释与实际读取 u32 不一致；DBPF offset 通过 i32 转 u64，超过 2 GiB 的 offset 会扩成巨大值。严格 validator 应单独定义无符号格式与安全预算，不应把这些宽容/迁移行为照抄成格式规范。

Property parser 为兼容历史样本容忍 EOF 提前结束（lib.rs 的 claimed_count 说明），所以成功 parse 仍不足以宣布 strict-valid。写出器确定性/roundtrip 测试只能证明工具自身协议自洽，不能代替游戏加载验收。

## 5. dev 容器完整性链的边界

`DatabasePackedFile/ReadHeaders.c`（`0x8A0280`）→ ReadHeaderRecord / ReadIndexRecord。

`VerifyHeaderRecordIntegrity.c`（`0x8A0CB0`）可见 magic、major <= 3、entry count < 0x07FFFFFF、索引位置/大小范围和 key type 非 0 条件。由此不能声称必须 major=3，更不能把 writer 的“major1 可能被忽略”注释当已证实事实。函数末尾高低 position 一致性分支可能绕过此前 v2，且有 32/64 位恢复疑点，必须机器码复核后才可抽成精确规范。

`ReadIndexRecord.c`（`0x8A2610`）只读声明 index 大小并传入 ReadFromMemory，随后调用 VerifyIndexRecordIntegrity；存在 `mbEnableReadOnCorruptFiles` 宽容分支。`VerifyIndexRecordIntegrity.c`（`0x8A0F10`）委托索引 Validate；`PFIndexModifiable/Validate.c`（`0x8F5FA0`）检查非空 payload 的 position/size 位于有效范围，空记录跳过。本轮未证明所有 index 实现具有完全相同校验。

OpenSCP 可采用比游戏容错更严格的发布门禁，但须将“产品保守策略”与“游戏硬性规则”分列。

## 6. 服务契约建议与后续验收

建议 `inspect_package(profile, source)` 产出结构化 findings：severity、code、TGI、offset、expected/actual、证据等级、解析器版本；`build_minimal_overlay(changes, baseline, profile)` 返回产物、SHA-256、TGI 清单、外部依赖和同 TGI 冲突候选。输出暂存文件先重新打开、逐字节核对，成功后原子发布；失败不激活构建记录。用户可见消息通过 zh-CN/en-US i18n，机器诊断 code 保持稳定。

下一轮优先：继续补齐 EcoGame group/版本路径的 retail 对拍；独立测试环境做一个无 metadata 的单 TGI 包，分别验证扫描/注册/最终命中；然后做文件名、复制顺序、目录与缓存的单变量实验。不可仅凭“游戏打开了文件”把报告标为 loaded/winner。此次没有新增或运行游戏验收实验。
