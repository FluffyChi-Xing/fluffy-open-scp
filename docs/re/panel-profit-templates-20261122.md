# 利润面板：原生账本依赖与通用模板

用户范围：既修复佩翠安纳制药测试建筑缺失的利润显示，也在工作台新增通用利润模板。
此前仅研究、不改该楼的限制已被本轮明确指令更新。其余库存、货物、住宅金钱本轮仍属
模板复用可行性说明，没有宣称已经注册这些控件。

## 实现

- `profit`：通用分组的每日利润模板，复用本机 `kBlockLandmarkProfit` 布局与计算。
  只请求 native update 实际使用的四项今日/昨日收入支出，去掉三个无用查询，避免无用
  查询的 null 将本来可显示的利润隐藏。显示的是财务账本净额。
- `profit-index`：相同视觉模板的商业/工业利润指标适配，读取本机 `ProfitToken`、
  `ProfitTokenLast` 的实际 amount，显示“当前/上次记录”。不标成 §/天，不冒充昨日金额。
  资源映射使用组件目录的可选 metric 元数据；自有文字通过同一份 zh-CN/en-US JSON
  提供给编辑器和运行时。
- 现有 `tourism-profit` ID、存储配置继续有效。schema 仍为可选独立 panel 文件；原有
  schema 枚举增加两个 ID，没有将配置写回 asset.lot.json 或改变继承流程。

## 当前建筑的缺失依赖

原版地标 Unit 模板 `00B1B104:48E0C000:4E01E311` 的 A4 含 `C14044C4`，与游客
bundle 并列。当前商业+观光 Unit 的 59 个启用项没有这套账本。游客收入与容量控件有
数据，不能据此断言每日财务账本也存在。

从本机游戏包读取 `00B1B104:40E0C700:C14044C4`，确认它声明：

| 资源 | ID |
| --- | --- |
| CasinoSpecialtyCurrentMonthIncome | A5B3BE12 |
| CasinoSpecialtyLastMonthIncome | CE32088F |
| CasinoSpecialtyCurrentMonthExpenses | 3DEAB6DC |
| CasinoSpecialtyLastMonthExpenses | 90E00435 |

ID 以实际解析的整数为准；常量命名含 Month，而原生面板的时间标签与结算规则应整体
沿用游戏，不根据名字自行转换时间单位。C14044C4 同时提供小时记账与 day 结转规则，
不是只添四个永久为零的 bin。

新增 `panel/ledger.rs`，只在配置 `profit` 或 `tourism-profit` 时由 panel-bind 调用：
读取本机原始 provider，检查四个 bin 和结转规则存在；合并 Unit 的有效继承字段，
仅在缺少原生账本 provider 时将 C14044C4 追加至 A4。已有 C14044C4/原生赌场账本
则不重复追加；显式排除报错，不偷偷覆盖。编码后再解析并校验 provider 已生效。
无自定义面板或不选每日财务利润时不补账本。单独利润指标不添加财务规则。

来源身份：实际离线安装与项目参照的 `SimCity-Scripts_287520926.package` 均为
SHA-256 `77DDCED4A23A818673E44C19649732390D8771BC5A1434583032E5841BAEDD04`，
C14044C4 的解析属性逐项一致。两个安装的 SimCity_Game.package 整包哈希不同，未宣称
它们整包等价；本次 provider 与规则来自已验身份的 Scripts 包。

## 验证与测试产物

- 前端 16 项测试：组件可选性、插入、语言切换及数据口径提示、四项必要财务查询、
  实际利润指标值和空值隐藏，以及既有布局回归。
- Rust 9 项 panel 测试及 1 项显式本机包集成测试通过。集成测试实际选中 common profit，
  从本机包解析 C14044C4 并验证 Unit 输出依赖。
- 完整本机 Game.js/ScrUI 的 Chrome 测试：同屏通用每日利润与利润指标，模拟输入
  2640−450=2190、2100−450=1650；空查询隔离、双反馈状态、五组动态宽度和实际字形
  居中断言均通过。此为模拟查询，不等于已在真实城市验证记账。
- 类型检查及相关 ESLint 通过。

定向包复用同一 ledger::ensure 和实际组件目录。建筑只在新菜单身份的 Unit A4 追加
C14044C4；其余属性不变，另外 20 个资源 payload 逐项一致，不重新编译模型或纹理。
构建/修复记录位于 `tmp/panel-native/profit-repair/ledger.json`。

建筑 SHA-256：`50F5095F3946ACCCE30ECF35AF122E6363292D299B60CE7337D6EB4C0B1B07F3`。
runtime SHA-256：`11F57D218609A76EA39873BB75ACAE2CE05C7DA2D7D9051E9359F3AA7959B619`。

账本从初始化后按原生规则记账，不反推此前存档中的历史。游戏由用户冷启动验收；
源码测试通过不代表已取得真实城市的利润金额。

已通过归档脚本移出旧建筑 b8114298、旧运行时 260045cb，部署上述 50f5095f、11f57d21，
部署读回完整哈希一致，MANIFEST 已记载。游戏未启动、未备份存档，沿用用户明确授权。
桌面工作台后端包含编译期组件目录与构建处理器，需要重启/重编开发版才能消费新增项。
