# 和平饭店与瓦科面板共享运行时

## 实装身份与根因

按 MANIFEST 先核对实装文件 SHA-256：

| 实装文件 | 包 SHA-256 | Game.js payload SHA-256 |
| --- | --- | --- |
| 000_OpenSCP_peace_hotel.package | a9f2d5b63b29938b39500492dac9c1786a3fdcdb2daff6b4a22e3673da674785 | 无 |
| 000_OpenSCP_Waco_UtilityPanelFix_20261124.package | cf63ee00d994dc298b3dc87c6458e5b48f8f1399d8be9a3d577e99f0d123a2a5 | 无 |
| 000_OpenSCP_peace_hotel_panel.package | 275b2c308bbb30bdd961d42452a64228ebb624da8ed46a74748f29c1dd3bb03b | edb782c6422f55e76bd780ea0bb99c7d6bc29536ccb3a9c3bdfa63a2e2409065 |
| 000_OpenSCP_Waco_UtilityPanelFix_20261124_panel.package | 8661589275743ae255eeecb707ba857d1cfe6845300b46a64868020f078d6d84 | 4183ca597731ddbb5662d319794f7deb571a86cfca63bf75d038d31d1ae3faff |

两份面板依赖争用 `67771F5C:40464200:7CCC548C`。建筑自身的布局按 Asset 身份独立生成，本次没有发现它们在争用同一份自定义布局；真正重复的是完整 Game.js 的不同版本。旧部署预检要求逐字节相等，无法表达“新版提供旧版需要的能力”。

另有原版来源错误：旧工程依赖 `D:/ea-games/SimCity/`，实际安装是 `D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/`。前者 Game.js 长 3,383,719 字节，SHA-256 `64663354fabd086c7dd8e02cc2f9ad317f0a1de5332ec0fddd173bf283771f37`；后者长 3,382,891 字节，SHA-256 `683c66fe0d0146b9b630bbe59e1df89848fc05d907f3028147c01ff4f407431b`。本次重打包在隔离工程中把六个 UI 来源全部重新固定到实际安装，并核对 SHA-256。另将和平饭店和瓦科两份实际工程的 dependencies.panel.sources 同步更新，保留逐字节原文件备份，其他 JSON 配置不变。

## 从 Simoder 迁移的部分

依据本地 `Simoder/crates/simoder-mods/src/ui_patch.rs` 的 `register_patch`、`revoke_generation` 和 `compose_sorted`，在 `panel/runtime_registry.rs` 实现构建期的 owner/generation、view/slot、依赖、确定性组合和归属撤销。不同写入者占用同一独占槽位直接失败，不能靠包文件名碰运气选择赢家。测试覆盖组合顺序、同槽位冲突、跨归属/代际撤销隔离、前置依赖和顺序错误。

这里迁移的是登记与组合语义。未接 Simoder DLL、Luau、DOM applicator 或运行中的 MOD 热卸载。既有详情面板仍由原生 Asset 选择器、自有布局和已验证 ES5 适配器负责。

共享运行时只含一份实际游戏的原版 Game.js，随后按固定顺序组合六段：catalog、tower-operations、bindings、container-tourists、container-shoppers、component-queries。不把两份旧 Game.js 拼接，也不在每栋建筑中输出建筑专属脚本。

## 可演进的兼容合同

`panel/runtime_bundle.rs` 在脚本末尾输出不执行的 JSON 注释清单：原版长度与哈希、正文哈希、每段范围和哈希、归属/代际、视图/槽位、ABI、能力版本与绑定 ID。清单检查同时验证连续范围、唯一名称和槽位、完整正文。构建指纹包含新组合器版本，避免重放旧输出。

发布预检逐一检查所有覆盖 Game.js 的包：相同脚本保留既有路径；不同脚本必须有有效清单、相同原版来源与 ABI，且已安装者提供全部所需能力和绑定。不能只因文件名含 OpenSCP 就接受。缺能力、损坏正文、陌生无清单脚本或未知 ABI 继续拒绝。未来增加可兼容能力可由同一运行时服务旧建筑；破坏性变化必须使用新的 ABI/能力标识。

已部署的两份旧脚本没有清单，迁移入口只认可上表的两个完整 payload 哈希。另用 ES5 AST 比较核对了两处原版中所需的 34 个 native binding，全部一致，因此仅为这两处已核对原版哈希提供旧面板迁移能力。旧来源脚本只作为“需求”识别；真正安装的候选仍必须以当前实际原版开头。不会因此放行未知游戏版本或让错误原版覆盖当前游戏。

`openscp-build --panel-runtime-check <stock.package> <shared-runtime.package> <required-runtime.package>...` 提供只读检查，报告属性为离线合同验证；不将其标为游戏运行通过。

## 保留与验证边界

和平饭店继续使用原设计的住宅、观光、绿化和地价配置，以及上一轮 v7 的公共 plop 基底和住宅迁出/补入住规则。瓦科建筑包不重建、不修改；通过统一运行时服务它已有的选择器。

审计分别核对实际原版前缀、v7 自有 ES5 扩展字节、模型/材质/贴图的解码 payload、Graphics 父项重绑，以及两个实际建筑包中的所有自定义控件选择器。生产脚本在带原生接口桩的 VM 中验证绑定登记与交替建筑查询隔离；这不代替实际游戏渲染、点击、人口/游客和日夜灯光验收。

输出、原始来源哈希、34 个 native binding AST 比较和详细台账位于 `tmp/panel-coexistence-20261125/`。未修改 SimCityData、未改存档、未启动游戏。交付时必须用统一运行时替代两份旧面板依赖，不能把第三份不同 Game.js 直接叠加进去；部署继续遵守存档隔离及归档台账规则。

最终交付在 `tmp/panel-coexistence-20261125/delivery/`，新菜单 `BDDF8C86`。27 项相关 Rust、20 项前端通过；真实模型完整生产 API 与最终 CLI 输出字节一致，最终 CLI 直接验证两份实装旧运行时需求均可由新统一运行时提供。完整哈希与分层验收见交付 MANIFEST.json。未部署，未标游戏运行通过。

Simoder 迁移参照的 MIT 许可随附于 `docs/re/third-party/simoder-ui-patch-LICENSE.txt`。
