# ER2 UnitRule 的能力边界

本文以 [V 博士大楼的 Property 与 bundle 链](./er2-vu-tower-20261113.md) 为入口。运行机制来自带符号反编译和零售 AEB 字节，游戏内时序尚未采样。

## ArgScript 的两层语义

| 写法 | 求值时机 | V 博士样本 |
|---|---|---|
| `include "file.er2"`、`set NAME value`、`$NAME`、`${NAME}` | ArgScript 读取、变量替换 | `8F06F716` 定义点数门槛并包含 `VillainBase.er2`、`VillainLab.er2` 等。 |
| `define Name(args)` … `enddef`、`create Name(values)`、`&{param}` | ArgScript 展开规则文本。宏体中的 `if(match(...))` 决定生成哪些行 | `573AD901:82` 起按犯罪类型展开规则及其 ID，纵向复用同一逻辑。 |
| `unitRule Name` … `end`、`rate`、`timeTrigger` | 解析时产生规则定义，挂载后由模拟调度 | `573AD901:32` 的 `rate 61` 对到 AEB 的 `mSimRate=61`。`3EF3DF2D` 有创建后一次性触发和每日触发。 |
| `local/global/connected/nearby/map/agent`、`in/out/is/atLeast/atMost`、`!Name@scope` | 运行时计算资源约束和数量 | `573AD901:37` 的 `Temp@global` 从模拟 bin 取数量，`map Crime out ${Villain_CrimeMapFromHenchmen}` 的数值来自解析期变量。 |
| `flags/globalFlags -if`、`-set/-clear` | 运行时测试或修改各作用域控制位 | `573AD901:63–77` 为概率位的读取端。`A3369E07:37` 和 `766099D1:50` 分别设置和清除 `CrimeInProgress`。 |
| `onSuccess/onFail/chain`、`successEvent` | 运行时规则转移或向引擎处理器发事件 | `573AD901:310` 发送 `DrVuEscaped` UI 事件。 |

通用 ArgScript 的命令表由 `cParser::Init` 注册，包括 `seti/setf/setv2`、`redefine`、`arrayCreate` 等。具体用途由各自 parser 注册的命令和块决定。引号、注释、宏、元命令栈和块作用域应保存在文本 AST 中，以便显示原文和展开来源。运行规则图应指向展开后的定义和 AEB 规则 ID。通用命令注册证据见 [开发版 cParser::Init](D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/EA/ArgScript/cParser/Init.c:115)，查表和展开顺序见 [ParseCommandInternal](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:1260573)。

## 概率标志由谁生成

在零售主库 `08068AEB:40800200:622B9CD7` 中，以下四个 ID 同时出现在 UnitRule 与 Global/BinSetRule 段。ID 的原始名称未恢复。

| 规则 ID | 调度概率 | 局部 SetMask → SetValues | 全局 SetMask → SetValues |
|---|---:|---|---|
| `203F0385` | 1 | `0x70 → 0` | `0xC0000020 → 0` |
| `6AE5FC34` | 0.25 | `0x10 → 0x10` | `0x20 → 0x20` |
| `6BE5FDA4` | 0.5 | `0x20 → 0x20` | `0x40000000 → 0x40000000` |
| `6DE6008F` | 0.75 | `0x40 → 0x40` | `0x80000000 → 0x80000000` |

局部组的 `mSimRate=120`，全局组的 `mSimRate=60`。清除规则的 `mSimPriority=1871`，置位规则为 1872。调度器用模拟计数器和 `mChance` 控制入队，执行端按掩码清位或置位。单位计数器属于已挂载的规则状态，不能据此假定每栋建筑的刷新相位一致。依据见 [单位调度](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:638379)、[全局调度](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:635506)、[局部标志写入](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:697240) 和 [全局标志写入](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:692146)。

V 博士犯罪链用三个后续规则读取已有位。晚版 H&V AEB 的 `mControlFlags` 数据中，`AD153635` 测试局部 `0x10`，`27810D63` 测试局部 `0x40`，`F3BCC941` 测试全局 `0x40000000`。三个条件各自命中后进入对应犯罪分支，未命中时走 `onFail`，最后一条的失败分支为破坏类。同一刷新窗口内重复读取可能得到相同结果，全局位还被不同单位共享。把每个判断都模拟成独立的掷骰会改变行为。规则读取见 [CanRunRule](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:698532)。

## 执行能力和依赖

| 能力 | 已确认的边界 |
|---|---|
| 调度和状态机 | 常态规则、定时触发、引擎事件入口、sink 入口及成功/失败链。链式转移有连续结果计数、目标规则 ID 和延时。调度概率决定是否入队，未入队不会执行失败分支。 `ApplyRule` 的成功路径由 `CanRunRule` 决定，即使 `RunRule` 的返回值没有触发规则日志，仍会调用 `OnRuleSuccess`。 |
| 资源运算 | 检查、消费、生产及从 bin 替换数量；可涉及本地、全局、邻近、连接、地图、事务及区域上下文。范围由编译字段和实际目标决定。引擎结合各范围的可执行次数与 min/max 决定应用量。 |
| Property | 常规资源操作使用 bin。规则显式选择 Property 型 `mApplyResource` 时，运行时从 Unit 属性读取整数作为 maxApply，未取到再尝试 SimProperties。另有加载阶段的 PropOp 专用化，可改写编译记录中的支持字段。为建筑增加一个任意 named 属性本身不会注册动作。 |
| Agent 与物流 | V 博士巡逻用 `-createAgent`、`testAgent … -canReach`、`-sendTo … -or …`，回程 sink 接收 Agent 携带的状态。运输选择、目的地及可达性由引擎和已注册的 Agent、sink、路径及 handler 决定。 |
| Unit 生命周期 | 实验室 `-createUnit DrVu_RoboTank` 调用引擎创建注册好的 Unit；运行器还有升级、失败状态、销毁等内置分支。机器人自身的创建和离开规则设置/清除 `DisasterActiveRobot`，把持续状态写回全局 bin。 |
| UI、图形、音频、灾害 | 规则可选择图形状态并把事件交给已注册处理器。V 博士逃脱发 UI 事件，机器人创建发灾害遥测和消息。新的 UI 行为、灾害实现、音效与图形资源仍需对应的消费者与资源配置。 |
| 扩展边界 | UnitRule 记录提供固定的模拟操作与引擎动作。任意运行时表达式、任意 Property 读写、文件或网络访问、注册新 handler 均未见可由 UnitRule 字段表达的路径。ArgScript 的解析期文件 include、变量和表达式与这些运行时能力分属不同阶段。MapRule、GlobalRule、ZoneRule 各有自己的记录和执行器。 |

证据入口为 [ApplyRule](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:698906)、[OnRuleSuccess](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:697573)、[OnRuleFail](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:697770)、[Property 型上限](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:698557)、[PropOp 专用化](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:667630)、[动态资源量](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:701205) 和 [Unit 创建与生命周期分支](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:697700)。GlassBox 的整体模型可参照 [Andrew Willmott 的 GDC 演讲](https://www.andrewwillmott.com/talks/inside-glassbox)。

## 对 OpenSCP 的实现约束

1. **可视化**以实际挂载点为入口显示调度、事件和 sink。规则节点显示作用域、读写、阈值、动态数量来源及失败原因；沿成功/失败边显示计数、延时、目标 ID。随机标志显示生产规则、位掩码和刷新周期。预览需要接收当前 flag/bin 或模拟刷新过程。
2. **建筑编辑**可先支持引用已有规则、查看继承和排除后的有效 bundle、编辑已验证的 bin 与专用化参数。构建检查应验证规则、资源、目标 Unit、Agent、sink 及所需 handler 的引用能解析，并保留不识别的二进制字段。
3. **玩家脚本**需要同时解决文本到 AEB 的编译、RuleSet 的注册和建筑入口绑定。零售 `cEcoGameScripts::Init` 用 game ID 与 add-on ID 分别请求 `AE9:40800200` 和 `AE9:40800201`，资源工厂加载 AEB。把 AEC 或任意新 ID 的 AEB 放进包，尚无证据会自动挂入当前游戏。独立 add-on 注册、规则 ID 冲突优先级和未知字段的无损编译需继续验证。见 [主库请求](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:662620) 和 [附加库请求](D:/ea-games/simcity_dev/SimCity.unwrapped.exe.c:662822)。

## 复核位置

主库从安装版 `SimCity-Scripts_287520926.package` 重新抽取，包 SHA-256 `77DDCED4A23A818673E44C19649732390D8771BC5A1434583032E5841BAEDD04`，AEB SHA-256 `A388E3C15080A4721E7D0047DA180F6EE9F38292060C67892FA34B952564C381`，与本地分析样本一致。主库 UnitRule 四条记录起点为 145793、145937、146081、146225，全局规则为 4281725、4281829、4281933、4282037，数据池起点 4430172。H&V AEB 重新抽取的哈希与[首批记录](./er2-vu-tower-20261113.md)一致，数据池起点 51069，三条分支的局部/全局控制块偏移为 `0x2248`、`0x2288`、`0x22C8`。偏移以字节计。记录头与全局规则内联掩码按大端读取，引用的数据池中的模拟参数与控制块按小端读取。
