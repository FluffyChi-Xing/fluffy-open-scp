# 实际离线安装的 package 加载对拍

后续纠正：单 TGI 对照成立，但旧 writer 多条条目缺少 size 高位标记，游戏索引读取会错位。该缺陷已通过 cape 原包/修正副本实机对照确认并修复，见 [索引标记根因报告](package-cape-index-sizeflag-20261017.md)。本报告不可作为旧 writer 多条包的兼容性保证。

日期：2026-10-17（香港时间）。三个启动实验已经完成。结论范围为该 EXE、该安装配置、一个未改内容的 Property overlay；没有进入或保存城市，没有验证任意 Mod 的游戏行为。

## 身份与证据

实际 EXE：`D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCity/SimCity.exe`。
SHA-256：`2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`，与用户备份 EXE 一致。运行时基址 `0x400000`，8 个观察入口各 12 字节与此文件匹配后才安装 hook、恢复进程执行。

可复核事件摘录及原始日志摘要：[evidence/package-runtime-20261017.json](evidence/package-runtime-20261017.json)。每个 sequence 对应相应运行的 JSONL；摘录保留全部 register、scan、被观察目录和目标 TGI 事件。原始数据保存在本地 `tmp/package-runtime-{baseline,userdata,data}-20261017/{trace.jsonl,status.json,config.json}`，摘录含原始 trace 的 SHA-256。

| 实验 | PID | 窗口 | 事件数 | 回调读错误 / 队列丢弃 | 结果 |
|---|---:|---:|---:|---|---|
| baseline | 59940 | 75 秒 | 696 | 0 / 0 | 不新增包；目标来自 App |
| userdata | 61532 | 35 秒 | 209 | 0 / 0 | 最小包注册成功；目标仍来自 App |
| data | 54300 | 35 秒 | 207 | 0 / 0 | 最小包注册成功；目标来自最小包 |

## 1. 实际目录与注册链

以下相对路径以安装根目录为基准。baseline 观察到：

1. seq 1：BaseData ID `06CBED39` 返回 NULL；本环境没有这一可选目录的扫描。
2. seq 2：扫描 `SimCityData/`，priority 0。成功注册顺序：Oppie_TrainStation、SimCityDataEP1、SimCity_App、Audio_Banks、Audio_MusicStreams、Audio_SpeechBanks、Audio_Streams、DLC0、Game、Graphics、RegionTerrain0、RegionTerrain1（后十个均为 SimCity_ 前缀）。
3. seq 42–43：Config ID `00A0214F` 返回 `SimCityData/Config/`，散文件数据库以 priority 10 注册。该数据库不是 packed-file 类型，日志不按 packed 的偏移猜它的路径。
4. seq 50：`SimCityData/Locale/en-us/Data.package` 以 priority 1 注册。
5. seq 58：公共用户目录 ID `00A0214B` 是 `C:/Users/FluffyCHi_Xing/Documents/SimCity/`。seq 73：私有目录 ID `00A02151` 是安装内 `SimCityUserData/`。两者不能混同。
6. seq 89：扫描 `SimCityUserData/Packages/`，priority 0，晚于普通 Data。
7. seq 555–559：LocalSettings、TerrainForServer、UpdaterData 等用户数据包注册 priority 100；前面也有专用数据库注册，详情见 JSON。
8. seq 560–562：EcoGame ID `08F03C82` 先 NULL，后为 `SimCityUserData/EcoGame/`；备用 ID `08F03C83` 为 NULL。本环境走默认 EcoGame 路径。
9. seq 646–696：所选 EcoGame 脚本版本陆续以 priority 100 注册，包括 `SimCity-Scripts_287520926.package`、Sandbox 及 DLC。枚举所有候选版本和最终挂接版本是不同阶段。

本次未观察到 priority 2 的 Preload 注册，也未直接捕获 BaseLocale 路径查询；不能把静态可选分支写成实际执行。本观察入口也不保证所有目录访问都经过 GetDirFromID（调用可内联或直接读字段），因此实际 scan/open 路径比“没有某 ID 事件”更可靠。

该卷实际枚举呈现按名称排列的现象，`000_...` 测试包也在 Data 最先注册。但静态链没有额外文件名排序器；这不证明游戏实施“按首字母排序”，更不证明跨目录时名称能压过 priority/注册顺序。

现有 `cape-trinity-clearwater.package` 在 Data 和用户 Packages 中均观察到 Open=false。本轮没有修改、修复或删除它；失败具体原因尚未诊断，不能仅由 Open=false 断言文件损坏。补充离线核验：项目 `inspect` 可打开 Data 中此包，报告 DBPF 3.0、4 条未压缩资源、index 120 字节 @0x60；这再次说明工具能解析索引不等于游戏能打开。

## 2. 单 TGI 最小包的真实命中

目标：`00B1B104:00000000:4483E856`，原始来源 `SimCityData/SimCity_App.package`。使用项目现有 `dbpf::write_uncompressed_overlay`，完整复制解压后的原始 payload，未改任何 Property 值。

- 一个 TGI，payload 1,237 字节；完整 package 1,369 字节，即 `132 + payload`。
- 没有 Property/group `40404000` 的 package-info，没有复制官方签名 metadata。
- 重新打开输出，验证条目数、TGI 与解压内容逐字节相等。
- package SHA-256：`729306cf4e0c7c65db718837c1f680d79389c684406d9f306818d92b0c3d7ef6`。
- 临时名称：`000_openscp_loader_probe_20261017.package`。同一时刻只部署一份。

| 放置位置 | 打开 / 注册 | 最终选择 | 创建 Record |
|---|---|---|---|
| 用户 Packages | seq 57–59，true，priority 0 | seq 65，SimCity_App.package | seq 66，materialize=true / result=true |
| SimCityData | seq 3–5，true，priority 0 | seq 63，测试包 | seq 64，materialize=true / result=true |

用户目录实验中，查询发生在测试包注册之后；可见 Locale、Oppie、EP1 的存在性探测失败，App 探测成功，随后 FindRecord 返回 App。因此这里是实际查询选择了先注册的基础包，不能解释成“尚未扫描用户包”。Data 实验可见 Locale 探测失败后测试包成功、FindRecord 返回测试包，再创建 Record 成功。

这已证明现有 writer 的单 TGI、无 metadata 普通包可以被此游戏注册并选作资源来源。OpenRecord(materialize=true) 是成功创建资源记录的证据，不等于已经捕获工厂解码完成或视觉行为正确；本实验用完全相同 payload，避免了行为变更，也不声称验证所有资源类型的依赖闭包。

## 3. 观察器覆盖与限制

工具：[runtime_capture.py](../../tools/dynamic/package-loader/runtime_capture.py)、[runtime_agent.js](../../tools/dynamic/package-loader/runtime_agent.js)。Frida 17.19.0，挂起启动 → 验证指纹与入口 → 安装 8 个 hook → flush → ready → resume。不是事后 attach；三个实验均捕获到首轮 Data 扫描。

观察层：GetDirFromID、RegisterPackages、AddPackageIfValid、RegisterDatabase、Packed Open、FindDatabase、FindRecord、Packed OpenRecord。只读取参数/返回值，不主动调用游戏 NativeFunction、不修改返回值；Interceptor 本身会修改进程代码，不能称为零侵入。

ABI：thiscall 从 ECX 取 this；栈参数从 ESP+4 开始；bool 只读取 AL。TGI 内存顺序为 instance/type/group。只有确认 packed vtable RVA `0x929E20` 后才读取 db+0x20 的路径指针。OpenRecord 的输出指针为 NULL 时记 materialize=false，避免将存在性探测写成消费。

日志不是全量 I/O：

- baseline 对查询按 kind/TGI 去重并限量；同一目标的早期存在性探测可能占掉后续 materialize 记录，不能拿 baseline 的记录缺失判失败。两个目标实验对指定 TGI 不去重。
- dropped=0 仅表示进入输出队列的事件未因容量丢弃，不包括有意过滤的查询。baseline 实际 OpenRecord 命中 36,336 次，只输出有界样本。
- 不观察缓存命中、ReadFile 字节、资源工厂返回、所有数据库子类型、DLL 初始化前文件打开或采集窗口外事件。NULL path 不是文件不存在的证据。
- db 指针可能复用，历史分析使用事件中的路径快照，不能拿最终 databases 字典回填所有历史事件。
- 未用截图验收主菜单；捕获结束进程仍存活，baseline/data 的窗口为 SimCity™ 且 Responding=true。这不能替代视觉或城市加载验收。

## 对 OpenSCP 的落地结论

可以复用现有 writer 构建最小 overlay，并将检查报告分成结构完整性、准入、注册、实际命中、资源语义/游戏行为五层。不要自动附加官方 metadata；编辑字段后仍需序列化完整资源。新增 TGI 还要有消费引用。

安装服务不能默认用户 Packages 中的同 TGI 会覆盖 Data，也不能只靠 `000_` 名称承诺覆盖。应记录安装位置、实际 priority/注册顺序、目标命中源；未运行时观察就显示“未验证”。普通 overlay 采用重启验证，不承诺热加载。

本专题三个 P0 在上述构建和样本范围内关闭。剩余 BaseData/BaseLocale 配置来源、特殊 EcoGame 路径设置和缓存热切换、各类型的语义闭包，分别保留在对应专题；已有 cape 包的 Open 失败是独立的具体文件诊断项，不改变这次有效对照结果。

收尾：三次自建进程均发送正常关闭请求，并确认最终无 SimCity 进程。两个位置的测试包均在核对 SHA-256 后删除并确认不存在。没有修改已有 mod；本轮没有打开或保存城市，游戏自身启动/退出可能更新设置或日志。
