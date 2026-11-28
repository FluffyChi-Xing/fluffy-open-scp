# OpenSCP Rust SDK 与 Simoder 适配设计

2026-11-24 决策：SDK 等下一次合并 master 后由 master 切出 SDK 分支。UI 编辑器移植不纳入当前计划。当前工作见 [package 热重载对比](re/package-hot-reload-simoder-comparison-20261124.md)。

## 现有接入

基于 OpenSCP `e136b856` 工作树和 Simoder `b2e5ae4` 的本地源码。Simoder 已把 `dbpf`、`sc-properties`、`rw4`、`sc-format` 固定到 OpenSCP 的 `42fd54a9`。`simoder-formats` 重导出这四个库，`simoder-core::Tgi` 是 `dbpf::ResourceId` 的别名，`simoder-mods` 也重用了 `sc_properties::debug_tools` 的 overlay builder。该 pin 与当前 HEAD 存在分歧。两棵提交树中的 `dbpf`、`rw4`、`sc-format` 目录相同，四库范围内的源码差异集中在 `sc-properties`，应连同工作区依赖和锁文件验证升级。

证据：[Simoder Cargo.toml](../../Simoder/Cargo.toml)、[formats 公开接口](../../Simoder/crates/simoder-formats/src/lib.rs)、[TGI](../../Simoder/crates/simoder-core/src/lib.rs)、[debug-tools 复用](../../Simoder/crates/simoder-mods/src/native_host/tool_registry.rs)。

## Rust SDK 架构

| 层 | 归属 | 第一个稳定切面 |
|---|---|---|
| 二进制格式 | OpenSCP 的 `dbpf`、`sc-properties`、`rw4`、`sc-format` | TGI、DBPF 索引和解码、RefPack、PROP 解析与编码、RW4 读取、格式探测。将四个 crate 标记为同一批兼容发布，按已被下游使用的接口确定公开清单。 |
| 无界面工作流 | 先复用 OpenSCP 模块，等第二个调用者出现后再决定是否单独发 crate | package 检查与冲突扫描、资源差异报告、有限的发布验证。包路径、顺序、预算和输出目录由调用者传入。OpenSCP 的建筑构建器和发布闸门继续由现有入口负责。 |
| 格式适配 | Simoder 持有 schema 和运行期语义，OpenSCP 写导出适配器 | 先交换可离线校验的清单和诊断；随后使用经双方确认的 UI 描述契约。编译出的 DBPF 与运行期 MOD 产物有各自的元数据和验收。 |
| 应用与运行期 | OpenSCP 持有 Tauri、SQLite、编辑项目及建筑生成。Simoder 持有进程内 UI、脚本权限、MOD 激活、运行顺序和撤销 | SDK 依赖图从宿主指向格式库；两款宿主各自拥有状态。桌面项目 ID 与 MOD manifest ID 的对应关系放在显式导出元数据里。 |

稳定化先做发布约定和契约测试，再依据实际调用面缩减耦合。保留 `simoder-formats` 作为 Simoder 侧门面，首版沿用精确 Git rev 和锁文件，给四库的兼容组合打 tag、列 API 变更和支持矩阵。有独立的消费者冒烟测试之后，再决定是否发布 umbrella crate 或上 crates.io。临时离线工具可以复用 `sc-exporter`，先维持其与 `sc-registry` 的依赖边界。`sc-properties` 的 registry 是测试依赖，外部消费者的正常依赖检查应排除 dev 边；核对 x86/x64 消费构建的正常依赖树是否引入 Tauri 或 SQLite。32 位读取和解压的上限需要按进程预算明确。

- **身份**采用现有 `dbpf::ResourceId` 作为当前二进制路径的 TGI 类型。Simoder 的运行期 PROP TGI 使用 u32 instance。schema 3 的原始资源清单另有校验器，按 16 位十六进制文本保留 64 位 instance。未来如需将该清单映射到当前 DBPF/PROP API，应测试最大 u32、带前导零和高位非零输入，并在超出目标类型时明确拒绝。
- **字节边界**区分存储字节、解码 payload 和重新编码后的内容。SDK 文档逐 API 声明语义，保留未知或未编辑资源。公共报告给出稳定的错误码、TGI、来源、预期值与实际值，各宿主负责翻译和呈现。
- **包检查**复用 DBPF 读取与现有差异审计，生成有顺序和来源的重复 TGI 清单。比较解码 payload 的摘要可区分相同内容和冲突内容；启用后由谁生效仅在给定且已验证的加载顺序下计算。PROP 字段冲突与 UI 槽位冲突分别使用 Simoder 的语义。

证据：[OpenSCP workspace](../Cargo.toml)、[DBPF 身份](../crates/dbpf/src/index.rs)、[包读取](../crates/dbpf/src/package.rs)、[PROP 依赖](../crates/sc-properties/Cargo.toml)、[桌面包服务](../src-tauri/src/package_service.rs)、[建筑差异审计](../src-tauri/src/asset_build_m1/delta_audit.rs)、[Simoder manifest 资源合同](../../Simoder/docs/resource-projects.md)。

## Simoder 能解决的当前问题

| 当前问题 | 本地源码核到的能力 | 建议适配 |
|---|---|---|
| 多个自定义面板需要共享 `Game.js`，现有部署预检会挡下不同内容的同 TGI 覆盖 | `UiPatchRegistry` 按 view/slot 组合 patch，记录 `replace_slot` 争用，并按 MOD owner generation 撤销。EAWebKit 的 ES5 applicator 接收描述并回传交互事件；QuickJS 测试覆盖 DOM 和点击。 | 优先把独立诊断按钮、面板或弹层放进已确认的扩展槽位，观察两个 MOD 能否并存。若需复用 Studio 编辑体验，导出器为新增路径，对无法表达的判定或数据绑定给出诊断。保留现有 LOT/Asset 面板和它的发布预检。 |
| Studio 工程状态与游戏里的启用状态脱节，修改后难确认生效的是哪版 | Manager 区分 Inactive、Active、Changed、Missing 等状态，比较候选 fingerprint，显式 Reload，记录启用顺序。PROP registry 报告同 TGI 同字段的多 MOD 冲突。 | 先在 Studio 发布前显示目标、来源、能力、冲突和候选摘要。运行中启用状态由 Simoder 提供。只有对方提供稳定只读接口后才接实时状态，Studio 不写 `state.toon`。 |
| 营业成本或利润显示问题，容易混淆输入字段、结算与界面副本 | 已支持有限的标量 float PROP 操作，按原始快照重算，跟踪受影响资源。其 Sanitizer 取证中模拟和选择器更新，已放置建筑详情仍显示旧值。 | 用单属性实验定位输入，分别记录原始 PROP、游戏实际日账和面板读数。Simoder 的 bridge observer 可作为只读取证候选。任何游戏内刷新范围都以视图或子系统的证据为准。 |
| 自制建筑、模型、夜灯与贴图想作为可管理 MOD 交付 | schema 3 加载器可验证目录、TGI、大小、SHA-256 及能力声明；源码明确阻止 `resource.register/resource.replace` 激活。 | 把 schema 3 作为可选的离线交接格式。建筑 `.package` 继续用 OpenSCP 构建与受控部署。任意资源热注入、材质热更、在运行中切换建筑包仍需各自的游戏验证。 |

UI 的第一个样例建议是只读诊断区域，用 `zh-CN`、`en-US` 的翻译键和一个只读动作展示 MOD 与候选版本。契约需约定运行时如何返回当前 view、缺槽位、事件 owner 与 generation，以及切语言如何刷新。使用 `UiNode` 已有的受限节点，验证打开、关闭、撤销、重入和双 MOD 冲突，记录实际可用的视图和版本。Simoder-Base 的 applicator/脚本热更测试与任意第三方游戏面板的热更属于不同覆盖范围。

证据：[OpenSCP 面板 schema](../src-tauri/src/asset_build_m1/panel/spec.rs)、[Game.js 预检](../src-tauri/src/game_launch/panel_preflight.rs)、[Simoder UI registry](../../Simoder/crates/simoder-mods/src/ui_patch.rs)、[EAWebKit 协议](../../Simoder/crates/simoder-d3d9/src/ui_patch_host.rs)、[UI 测试](../../Simoder/crates/simoder-d3d9/tests/ui_patching_and_base_mods.rs)、[MOD 生命周期](../../Simoder/docs/mod-lifecycle.md)、[字段冲突](../../Simoder/crates/simoder-mods/src/patch_registry.rs)、[Sanitizer 记录](../../Simoder/docs/resource-loading.md)、[raw activation 闸门](../../Simoder/crates/simoder-mods/src/manifest/mod.rs)。

## 实施顺序与验收

1. **锁定下游升级基线。** OpenSCP 提供四库的候选 tag/rev、公开接口清单和可再分发的合成样本。Simoder 在分支单独运行其正常 x64 和 i686 构建，校验 TGI、PROP 编解码、RW4 读取、overlay 回读、debug-tools builder、依赖树和锁文件。把候选与现有 pin 的差异分成 API、包字节、解码内容三类。双端通过再推广。
2. **做离线发布预检。** 先交付读取清单及 package、确认 payload SHA-256 和列出冲突的只读入口。样本覆盖多个包声明同 TGI、相同 payload 与不同 payload、schema 3 无法激活、高位 TGI、路径越界及构建后二次修改。用同一 fixture 对照 Simoder 的接受与拒绝结果。
3. **接一个 Simoder UI 样例。** 先确认支持的 view/slot、脚本入口及版本合同，再增加 Studio 的可选适配器。在 ES5 harness 验证结构与事件，再由用户实机确认画面、切语言、冲突和撤销。涉及已有建筑包时按 ledger → MANIFEST/实际 SHA-256 → payload 的顺序，核对模型、纹理、材质、Graphics 及 Asset/Unit 的渲染控制与生命周期。按既有归档和测试城市规则部署，保留完整构建闸门，交由用户启动并完成日夜、假内景和统计柱等相关验收。
4. **用独立实验验证运行期诊断。** 跑通一个已验证的 scalar float 字段，从候选、启用、观察、禁用到恢复记录有效值，检查复制到其他游戏对象的数据何时失效。任意资源注册等 Simoder 的独立合同具备验证后再安排。

## 本次验证范围

OpenSCP 四库的本机 x64 单测 194 通过、1 忽略。Simoder 的离线 Cargo 解析因本地 Cargo Git 缓存没有固定的 OpenSCP revision 而中断，当前环境也仅安装 x64 Rust target。Simoder 的测试内容和支持边界来自本地源码，跨项目构建、32 位和实机验收待执行。本次为架构设计和源码评估。
