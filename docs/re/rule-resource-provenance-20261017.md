# 规则资源的 package 来源：加载链采集

日期：2026-10-17。接续 [规则身份报告](rule-resource-identity-20261017.md)。已完成实际离线 EXE 冷启动后的加载链采集，并将随后城市规则日志中的规则集对象关联到实际 package 来源。分段窗口与丢事件限制见下文。

## 为什么当前规则对象不能直接给出 package 路径

零售 `cEcoRulesResource` 构造函数 0x5C98F0 初始化规则集、位于 +0x144 的 Resource 基类及 release callback。工厂 0x5C9980 分配 0x15C 字节对象，复制 Record 的 instance/group，将资源 type 设成 AE9。核对范围内没有复制数据库指针或包路径。

工厂 `ReadResource=0x5C9450` 检查 Record 类型，AEB 分支取 stream 并调用 `cEcoRuleSet::Read=0x602420`，成功后递增规则集 mModCount。来源 Record 由 ResourceManager 管理，后续 Flush/Release；不能把规则对象后的任意内存解释成来源指针。

源码证据来自 dev 带符号反编译树：`GB/cEcoRulesResource/cEcoRulesResource.c`、`GB/cEcoRulesFactory/CreateResource.c`、`ReadResource.c` 与 `EA/ResourceMan/Manager/GetResourceInternal.c`。零售相关 6 个函数区间均已三样本逐字节核对，见 [指令证据](evidence/rule-resource-provenance-20261017.json)。

## 必须核对最终返回对象

`Manager::GetResourceInternal=0x911760` 先尝试缓存；缓存未命中时选择数据库、OpenRecord、CreateResource。创建成功之后还会在锁内再次检查缓存。如果另一个对象已经进入缓存，新建对象会被 Release，最终返回旧缓存对象。

所以“OpenRecord 成功”和“工厂创建成功”都不足以独立证明最终对象的来源。新探针按线程的嵌套请求关联：

1. 请求抽象规则键 AE9，group 40800200 或 40800201。
2. `DatabasePackedFile::OpenRecord=0x8EA530` 返回具体 Record 指针，记录真实请求键 AEA/AEB 与数据库位置。
3. `cEcoRulesFactory::CreateResource=0x5C9980` 消费相同 Record，成功返回 Resource 指针。
4. ResourceManager 最终成功返回的 Resource 必须与该新建对象相同，才能标记 `created-from-record`。

缓存返回标为 `cache-hit-origin-unknown`；没有完整匹配链标为 `origin-unobserved`；失败请求没有来源归属。非目标嵌套请求形成关联隔离边界，避免把子调用的证据串到父请求。已打开的其他数据库类型只保留指针/vtable，不猜包路径。该版本只观察同步 Manager 请求内的 PackedFile/规则工厂路径，异步或其他加载实现不冒充覆盖。

## 实现与当前验收

`tools/dynamic/rule-logs/capture.py --scope provenance` 支持附加，以及显式 `--launch-exe` 冷启动。启动前校验 EXE 摘要；安装全部 hook 前校验全部入口；已有游戏进程时拒绝再启动。清理时卸载并 detach，不终止游戏。启动初始化失败时尝试恢复新进程，避免遗留挂起进程。

每次采集最多 120 秒、2000 条记录，队列 256 条。`status.json` 保留错误、丢弃、命中和停止时尚未结束的目标请求数 `incompleteRequests`。工作台分别展示 `rule-resource-record/create/cache/request`，不把候选打开与最终来源混为一条“成功加载”。

对 PID 18848 做了 10 秒附加验证：四个 hook 成功安装，窗口内所有入口命中均为 0，记录 0、错误 0、丢弃 0，已 detach。此结果仅说明该窗口未执行这些入口；既不能证明不存在来源，也不能验证真实加载路径的参数关联。

以上为空闲阶段历史结果；用户保存退出后，已带探针启动真实 EXE，PID 7424。所有采集结束后均已 detach，没有关闭游戏。

## 实机闭环

真实来源为 `D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityUserData\EcoGame\SimCity-Scripts_287520926.package`。

进城采集 request 1 的事件序列 6、7、9 分别证明：OpenRecord 成功取到具体 AEB Record，工厂消费相同 Record 创建 Resource，ResourceManager 最终返回同一个 `0x2e722284`。抽象请求为 `08068AE9:40800200:622B9CD7`，磁盘条目为 `08068AEB:40800200:622B9CD7`。

随后同一进程的城市日志显示 game=`0x2dc4f520`、scripts=`0x2e7219b0`、ruleSet=`0x2e722140`。规则集内嵌 Resource 地址 `ruleSet + 0x144 = 0x2e722284`，与 request 1 完全相等。全局规则 stateIndex 315 → index 380 → ID `0x739D42FF`，单位 slot 392/stateIndex 12 → index 10447 → ID `0xF01B0EB5`；两者 applyCount 均为 1，均指向该规则集。

同一进城窗口也加载了旧包 `257365692`：request 27 返回 `0x40b56484`，request 29 返回 `0x41b29664`；request 28 从新版包返回另一个对象 `0x409fbba4`。因此同 TGI 的首个/最后一个加载事件不能替代城市对象关联。指针关联仅限本次 PID；没有持续追踪对象释放及地址复用，不能据此建立跨会话身份。

## 质量边界与可复核材料

- 启动窗口 120 秒：片头期间 0 条目标记录，错误/丢弃均为 0。
- 进城窗口 120 秒：475 条记录，读取错误 0，丢弃 186。离线校验得到 21 条完整来源链及 1 个不完整事件组。停止时 `incompleteRequests=0` 表示没有仍在执行的请求，不代表历史事件无丢失。
- 城市规则窗口 15 秒：6 条记录，读取错误/丢弃均为 0。三个窗口分开采集，之间有空档，不能称为连续全启动覆盖。

完整 status、trace 摘要、校验结果和 6 条城市日志保存在 [JSON 证据](evidence/rule-resource-provenance-20261017.json)。来源包采集后磁盘 SHA-256 为 `77ddced4a23a818673e44c19649732390d8771bc5a1434583032e5841baedd04`，不是加载时内存内容摘要。工作台真实回归样本来自 request 1，只有安装目录被替换为测试路径。

`verify_provenance.py` 独立检查同请求内三阶段顺序、Record/Resource 指针、数据库及键一致性；缺失链或缓存替代不会被验证为来源。旧队列依赖 100ms 定时发送，连续同步加载曾填满 256 条队列。现改为满队列时立即发送有限批次，仍保留总记录上限；同时非目标 Record 调用先过滤再读参数。650 条同步模拟突发验证了 600 条上限、分批及 50 条超限计数。此修复没有重新进行实机冷启动验收，不改写本次丢失证据。

本阶段解决了实际来源问题；脚本名称、单位 stamp、输入输出 bin 语义及主动触发引擎日志仍未完成，不能将主机标记或观察入口命中描述为这些能力。
