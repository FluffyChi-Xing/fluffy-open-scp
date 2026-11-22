# 医疗建筑弹窗缺失：部署依赖核查

2026-11-22，只读核查；未部署、未启动游戏、未改存档或缓存。

## 参照身份

- 项目：`D:/openscp/mods/佩翠安纳制药`
- 构建：`c65eb70c781c2a4d39905a5a0b55dad11196e58200fef155e0026802b79c3849`
- 台账：`.build/ledger/<构建>.json`
- 产物：`.build/<构建>/packages/000_OpenSCP_asset.package`
- 部署：`D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData/000_OpenSCP_medical_building_dialog_test.package`
- 两者 SHA-256 均为 `89A59E54011835DAFD38CCF7DBAE659C86A998B38855493409634253321F1BD6`，字节身份一致。
- archive MANIFEST 未找到该 dialog test 的记录；旧 medical 包 E4ECDBD2… 不是本次参照。

## 资源证据

用本项目 dbpf/sc-properties 读取部署包，Asset `00B1B104:40E1C000:9A3C0589` 已写入新布局及 hover/click 自定义 ID。
hover 为 maintenance、shoppers、feedback；click 为 tourism-profit、tourism-revenue、tourists、feedback、maintenance。

递归扫描当前安装的 SimCityData 和 SimCityUserData 内所有 `.package`，检查脚本 TGI
`67771F5C:40464200:7CCC548C`。仅 SimCityData/SimCity_Game.package 提供该资源，
3382891 字节，没有 `OpenSCPPanel_` 和 `OpenSCPPanelRuntimeVersion`。

构建台账的 panel_runtime 指向 `.build/<构建>/dependencies/openscp-panel-runtime-v1.package`。
此文件存在，SHA-256 为 `922B9C45EC5B7D92E239DE2C3621F45B2B3C06038A744F7D00E1AF7F06AF3ED0`。
它提供同一脚本 TGI，3389448 字节，包含自定义控件注册和版本标识；当前安装未包含该资源版本。

## 原生加载链与判断边界

本机原始 Game.js 中 `cRolloverUI.Init` 调用 `GenerateRolloverItemDataBindingMap`，
后者枚举 RolloverItems 并把名称转成 ID。`InitItems` 按资产的 ID 列表查询该 map，
缺失会触发 `no match found for rollover item ID` 断言，后续代码还要读取 binding.layoutPath。
因此仅安装建筑包无法初始化这些自定义控件；这是明确的部署缺失，与用户观察吻合。
尚未从本次实际运行日志取得断言记录，补齐依赖后是否另有问题仍待用户冷启动验证。

## 交付修正

构建 Actions 原本只展示资源数和摘要 hash，虽然 result.panelRuntime 已有依赖路径，界面没有展示。
新增独立 PanelRuntimeNotice 显示该必需依赖的完整路径和中英文说明，无面板依赖时不显示。
现有通用 deploy_game_mod 仅复制 artifact，没有部署 panel_runtime 的能力；本轮没有扩展或调用它。
手动安装也必须走归档台账流程，游戏关闭、备份存档并在一次性城市验证；不能在健康测试城继续改变包集合。

## 第二次反馈：补齐运行时后仍无弹窗

用户明确授权不备份存档并接受测试建筑失效风险；运行时依赖已通过归档脚本安装，
随后用户反馈仍不显示。重新核对两个部署文件哈希保持上述值。
因此“缺少运行时”是一个已确认的必要依赖缺失，不能当成全部故障的解释。

新发现：panel::processor::clean_zot 用 `v["children"].as_array_mut()` 递归。
serde_json 的可变索引会为没有 children 的叶节点插入 null。构建布局有 24 处
`children: null`；scrui.cControlBase.DeserializeFromObject 原生实现按字段是否存在
判断，并直接读取 children.length，导致整个布局反序列化失败。
旧预览直接加载原始布局，未覆盖编译后资源的完整 ShowRollover 入口。

修正抽成 panel/layout.rs，用 get_mut 访问，并在清理前后强制检查 children 若存在
必须为数组。panel-layout/2 加入配置指纹，令旧配置重新构建时不复用旧布局缓存。
无 panel 的继承路径保持不变。

定向修复包使用相同的 prepare_shell 函数从本机原包重新生成唯一的布局资源
`67771F5C:0B074E5A:9844B030`，断言与旧布局的差异仅为移除这 24 个 null children。
DBPF 共 21 个资源，另 20 个资源的解码 payload 逐项完全相同。不是全量重新编译模型。
修复产物 SHA-256：`439C33AAF0A8EAAF57246BD3D16DE63F7E67C24ED047EC680CC0C6A6425FE474`。

验证：7 项 panel Rust 测试通过；另显式运行本机原包解析集成测试 1 项通过。
应用测试原产物缺少 Common Controls manifest，启动报 0xc0000139；按既有验证方法
复制到 tmp 并给副本嵌入 Common Controls 6.0 manifest 后运行，未改应用构建配置。
在隔离 happy-dom 中加载完整本机 ScrUI/Raphael/已部署 Game.js，使用模拟游戏查询，
执行原生 ShowRollover→InitItems→UpdateItems：旧编译布局稳定报 null.length，
修复布局的 hover/click 入口均通过（单 maintenance 控件、空动作列表的最小复现）。
该验证证明布局加载缺陷已修复，不等于真实城市全部统计和按钮的运行时验收。

已通过 archive_test_package.py 将旧包归档为
`000_OpenSCP_medical_building_dialog_test-89a59e54.package`，保留故障样本；
修复包以原测试文件名部署，部署读回哈希为 439C33AA…5FE474，台账已记录完整哈希、
故障原因和待验收状态。runtime 包保持 922B9C45…6AF3ED0。不备份存档、不启动游戏，
遵循用户本轮明确授权。等待用户冷启动和真实城市弹窗反馈。

## 第三次反馈：控件越界与详情默认文字

用户确认原生 popup 已创建，hover 购物者在左侧越界，click 出现 Title/Left1/Right1、
Why am I unhappy?、未替换的 maintenance_cost 与 population_amount。
再次核对部署：建筑 439C33AA…5FE474，runtime 922B9C45…6AF3ED0。

原生 Private_BuildDataBindingResults 在任一不允许空值的绑定返回 null 时会取消整个
更新回调。隔离完整 Game.js/ScrUI、保留原生 GenerateDataBindingQueries/RequestDataBindingData/
Private_BuildDataBindingResults/ShowRollover/InitItems/UpdateItems，以当前实际 hover 三项和
click 五项组合模拟一项 incomeYesterday 查询返回 null，完整复现用户截图的整页默认文字。
这是可复现的失败路径；尚未截获该城市实际返回 null 的具体查询，不能将模拟选择的
incomeYesterday 当作实机日志结论。

runtime 修正：自定义绑定 allowNullResults=true，收到 null 的模块保留
info.panelMissingQueries 并单独 minimize，不用虚构的 0 代替缺失数据；其他模块照常
更新，数据恢复后模块可恢复。初始化先隐藏内容，避免首次查询未返回时显示模板文字。
闲置人口头改用 OpenSCPPanel_headerNone，在 init 阶段立即隐藏，不依赖整批首次更新。
未绑定的原生第二标题（例如利润模块 RightTitle）隐藏。

原生模块按四栏混排，部分模板宽 309/317，而当前外壳内容区宽 265；购物者图标原始
坐标 -8/-11，且 WebKit scale 围绕中心缩放。自定义槽改为内容区完整行，容器对齐
原生白色内容区的 left/top/width，购物者为缩放前图标盒预留空间，文本与图标不重叠。
原生反馈背景/分隔线有意越出行的装饰部分只在内容区裁剪。仅对带 panelSlot 的
自定义绑定启用位置扩展，继承面板仍走原始 UpdateItemPositions。

验证：9 项前端定向测试、类型检查及相关 ESLint 通过；完整原生脚本的 Chrome 浏览器
模拟查询验证全数据与单项 null 两种情况。hover/click 都实际执行，断言可见文字无
Why am I unhappy、Title/Left/Right、维护费/人口参数；统计行在白色内容区内，购物者
缩放后 DOM 矩形在行内；缺数据模块隐藏而维护费可见。浏览器截图仅作本地验证，
图标加载与模拟名称不作为最终游戏画面的证明，仍待实际游戏验收。

定向产物只改建筑 Asset 的 0D7EADAC 人口头绑定，其余属性及另外 20 个资源 payload
逐项相同；更新共享 runtime 的自有 glue，原始游戏脚本前缀保持不变。
建筑 SHA-256：B811429828E1A083D1740D059CBF72D2DC0EB9AEA05190A354EDD0E0A12C79CD。
runtime SHA-256：0B8D9C173FFC659272BBCB88B17DB4B5F32B723C11E3FAFBFA56007C5912ECFD。

已确认游戏未运行，通过归档脚本撤出旧建筑 439C33AA 和旧 runtime 922B9C45，
部署上述 B8114298 / 0B8D9C17 新产物，完整哈希读回通过；MANIFEST 已记载故障原因、
完整哈希及待验收状态。未备份存档、未启动游戏，沿用用户明确授权。

## 第四次反馈：统计列没有跟随面板宽度

确认外层行已满宽，但原生统计模板宽 309，其内容框 left=3,width=267，保留了 39
的右侧空白。这是旧四栏模板的预留，不是当前弹窗应使用的边距。仅改变行宽会保留
这份空白；内部右侧值因此与实际内容区边缘不对齐。

stretchNativeContent 在 init 时根据模板自身的 left 与原始占宽识别宽内容框，移除
旧侧边预留，建立左右相同的原生 inset，再使用 ScrUI kPinStretch。容器与整行也设
为 kPinStretch，因此后续仅改变弹窗父宽度即可自动伸缩，不依赖重新加载或再次调用
UpdateItemPositions。子级的原生右锚定值继续保留自身边距。未写入固定弹窗宽度。

Chrome 原生脚本验证：完成初始 hover/click 后，同一实例宽度连续变化
230→278→360→480→278，不再次调用排版方法。完整查询与一项查询为 null 两场景
均通过；内容框左右 inset 均为模板来源的 3，右侧统计与白色内容区边缘的距离始终为
14（测量结果，非代码常量）。同时保留越界、占位符、空查询隔离回归检查。
前端定向测试共 10 项通过，类型检查及 ESLint 通过。

本次建筑包保持 B811429828E1A083D1740D059CBF72D2DC0EB9AEA05190A354EDD0E0A12C79CD。
仅更新 runtime，产物 SHA-256：964099E9C995E87187E14D2693FDD2872803606FCEA464ED1E23901C40BD8892。

## 第五次反馈：绿色反馈文字仍偏左

原生 RolloverHappinessText 的正常/异常反馈使用两条不同嵌套路径：正常分支外层 left=-1，
内层文本框另有 side area；前次只处理顶层非负 left 的宽内容框，未消除这条嵌套路径
的剩余侧空白。文字虽使用 center 样式，居中参照仍是偏左的小文本框。

centerFeedback 仅处理 feedback 类型控件，沿两条已发现的原生文本控制路径，从外到内
对齐到所在行，并将文本框左右内边距对称、使用原生 kPinStretch 与 kHAlignCenter。
不设置固定弹窗宽度，不改变统计表的左右对齐规则或反馈内容/颜色/状态判定。

验证：12 项定向测试、类型检查、ESLint 通过。完整原生脚本 Chrome 验证覆盖 hover 和
click、两种反馈状态、完整查询与单项 null、230→278→360→480→278 的动态宽度。
通过 DOM Range 测量实际文本字形（不仅是文本控件盒子），其水平中心相对内容区的
偏差均为 0 px；保留统计右列与越界回归断言。实际游戏效果仍待用户验收。

仅替换 runtime，SHA-256：260045CB4250872CE88FF12520AE65CE80590EAAA6CDD7B347873B98C1457D63。
建筑包保持 B811429828E1A083D1740D059CBF72D2DC0EB9AEA05190A354EDD0E0A12C79CD。
