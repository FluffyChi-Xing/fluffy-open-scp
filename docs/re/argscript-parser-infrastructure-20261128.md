# ArgScript 解析基础设施：开发版反编译契约

2026-11-28。本轮依用户要求，优先恢复基础设施，暂停新的零售版字段对拍与运行时捕获。代码是独立的离线 Python 研究实现，尚未接入产品编辑器、Actions、生产编译器或写包路径；没有声明完整 ER2 编译能力。

## 已实现的入口与调用顺序

`probes/runtime/argscript_parser.py` 的 `Parser.parse_line` 对应开发版 `cParser::ParseCommandInternal` 的可恢复主体：

```text
原始物理行 → 去注释与两端空白（保留跨行块注释状态）
  → 若元命令栈非空：整行与顶层结束标签比较，不区分 ASCII 大小写
      → 匹配：通知结束并弹栈，本行停止
      → 不匹配：交给顶层元命令 Parse；若消费，本行停止
  → SubstituteVariables(bracketVarsWithoutBraces=true)
  → SplitIntoArguments
  → 按第一个主参数查找命令
  → Chomp：只移除第一个主参数，保留选项
  → Execute
```

元命令消费前没有通用参数切分或变量替换。比如定义体中暂存 `emit $later (`，此阶段既不应报未知变量，也不应报括号不闭合。具体的 define/if 处理器还没有迁移；测试中的 Collector 是验证回调边界的宿主，不是这两个命令的实现。

`Parser.parse_arguments` 单独对应原生 `ParseArguments`：直接查命令、Chomp、执行，不补做元命令处理、注释处理或变量替换。

命令名按 ASCII 小写查找：从最内层块开始，逐层尝试外层块，再查全局命令表。注册同名命令替换既有绑定。块、元命令通过显式宿主 API 注册；目前未模拟原生引用计数、OnRegister/state 指针，也未隐式安装未经恢复的内置命令。

`parse_stream` 逐物理行调用文本入口，保留 CRLF 原文和偏移；结束时分别比较块栈、元命令栈与进入该流前的深度。这是相对入口深度的检查，嵌套流不一定从空栈开始。失败即抛错，保留已发生的回调及部分 trace，不自动回滚或继续后续行。流内错误保留阶段、文件、行和原始偏移。

`finish(error=True)` 对应 `ParseEnd` 的错误清理子集：先清理元命令，再清理块命令；随后清空命名空间位置及块注释状态，但不删除变量表。正常结束不自动调用仍在栈中的结束回调。文件目录状态尚未实现。

## 变量与命名空间契约

`probes/runtime/argscript_variables.py` 使用纯函数和不可变状态值；状态变化由调用方显式接收，不使用隐藏全局状态。

| 操作 | 已恢复语义 |
| --- | --- |
| PushNamespace | 保存当前 scope 长度，追加原名及 `:`；不是把字符串按冒号重新拆层 |
| PopNamespace | 按保存的长度恢复 scope；不删除变量 |
| SetVariableNoSubstitution | 以 `scope + name` 的小写形式存键，原样存值 |
| SetVariable | 对值做一次 `SubstituteVariables(false)`，再按上一行存储 |
| Variable，普通名称 | 当前 scope 优先，按真实 push 偏移逐层回退至根作用域 |
| Variable，前导 `:` | 去掉一个前导冒号，仅查该绝对局部键；失败仍走全局后备 |
| 全局后备 | 独立的全局变量表按传入名称精确查找；再请求外部变量集 |
| SetGlobalVariable | 原名键、原值；传入空指针删除。空字符串与不存在不同 |

外部变量集在研究实现中以显式输入的 bindings 快照代替原生 provider 回调。局部表会做大小写归一化，全局表没有相同的归一化调用，不能合并为一张不区分大小写的字典。

原生局部 setter 无条件加 scope，**不把前导冒号当作绝对赋值**；它与 lookup 的规则不对称。实现保留此差异。空栈 pop 在原生 API 中没有这里可见的防护；研究宿主显式报错，并标注为 host guard，不宣称是原生诊断。

## 替换规则与来源映射

```text
x 的值为 a b
命令文本 emit $x       → emit (a b) → 一个参数 a b
命令文本 emit ${x}     → emit a b   → 两个参数 a、b
命令文本 emit ${x}_end → emit a b_end
```

替换扫描原始输入一次，不重扫插入值；引号和反斜杠不屏蔽 `$`。当 bracket 参数为 false，未加花括号的变量也不额外包括号。此函数只替换文本，不求值表达式。

变量名在已实现的 ASCII 范围内：首字符为字母、下划线或冒号，后续再允许数字。分别保留 missing variable name、bad variable name、missing `}`、Unknown variable 等错误类别。只有存在 `$` 才触发原生 2047 字节输入限制；插入值导致输出超过此长度不是同一检查。非 ASCII 替换涉及原生编码及 ctype 表，当前明确拒绝；无替换的 Unicode 原文不由此函数改写。

替换结果用 pieces 保存“生成区间 → 原始引用区间”。Token 的 `generatedSpan` 与 `sourceReferences` 分开：插入的整个值归因于 `$x`/`${x}` 引用，不伪造值中各字符在源文件的位置；跨注释形成的 token 保留不连续源区间。变量定义位置、宏调用栈和 include 栈尚未加入该映射。

重要顺序：替换值中插入的 `#` 不再次经过注释剥离；替换值中的括号、引号、选项标记则会经过随后的参数切分。

## 证据提取与实现对应关系

源根目录：`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/EA/ArgScript/`。这些是开发版反编译输出，不是原始 C++；地址不是零售版探针地址。

| 开发版函数 | 地址 | 本轮使用 |
| --- | --- | --- |
| cParser/ParseCommandInternal | 0x8FCB90 | 文本入口顺序 |
| cParser/ParseArguments | 0x8FB390 | 已切分参数入口 |
| cParser/SubstituteVariables | 0x8FCE20 | 替换、包括号、单遍扫描和错误 |
| cParser/Variable | 0x8FB450 | 作用域及后备查找 |
| cParser/SetVariable | 0x9010F0 | setter 的替换模式 |
| cParser/SetVariableNoSubstitution | 0x900680 | 局部键构造 |
| cParser/SetGlobalVariable | 0x900770 | 全局原名键及删除 |
| cParser/PushNamespace、PopNamespace | 0x8FF780、0x8F94C0 | 真实 push 偏移栈 |
| cParser/CommandLookup、RegisterCommand | 0x8FAAF0、0x8FFD90 | 分层查找、同名替换 |
| cParser/ParseStream、ParseEnd | 0x9001F0、0x8FCA80 | 流深度检查、错误清理 |
| cArguments/Chomp | 0x8A41D0 | 只删除首个主参数 |
| cParser/Init | 0x8FE730 | 内置命令注册清单，尚不代表命令语义已实现 |

可复现提取命令（仓库根目录执行）：

```powershell
python probes/runtime/argscript_native_inventory.py --source 'D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/EA/ArgScript' --out tmp/argscript-infrastructure-20261128/native
python -m unittest discover -s probes/runtime -p 'test_argscript_infrastructure.py'
```

证据工具原样复制 21 个函数文件，并在 `manifest.json` 保存逐文件 SHA-256、开发版地址和签名；从 Init 提取到 33 项直接命令注册，包括 include、变量类型命令、namespace、if、宏命令、目录栈和 version。清单不推导各命令的完整参数契约。

## 验证、影响范围和剩余工作

后续进展：参数规范的消费层和执行闸门已新增，累计 57 项离线测试通过，详见 [cArgumentSpec 参数消费契约](argscript-argument-spec-20261128.md)。下列初始剩余清单中的第一项已部分完成；数值转换和完整声明字符串编译仍未实现。

定向测试 27 项通过，覆盖成功与失败路径、scope 回退、局部/全局大小写差异、替换边界、延迟文本、元命令整行结束匹配、块查找、入口深度检查、错误停止、清理及源映射。既有 H&V 测试 49 项也通过；其中涉及历史对照夹具，因此不把这次回归记为新的零售版调查或运行期验收。后续本阶段定向验证不依赖零售 AEB 或游戏。

调用范围为新增离线模块及其测试。旧的 H&V 专用 Expander、UnitRule 领域解析器、编辑器与生产构建路径保持原实现；没有迁移现有调用方，既有持久化图、包数据契约与游戏资源不受新增入口影响。

未完成项：

1. 原生 cArgumentSpec 的类型、参数个数、选项处理与未处理选项诊断。
2. define/redefine/create 参数替换、if 条件消费与嵌套、namespace 命令处理器；当前只有宿主基础接口。
3. cExpression 的词法、优先级、函数注册、整数与浮点转换；不能借用 Python eval 冒充。
4. include/sinclude 的实际路径规则、目录栈、错误与循环行为；尚未进行文件解析。
5. 错误包装、宏定义位置与展开调用栈、队列执行以及完整 ParseBegin/ParseEnd 生命周期。
6. 与 UnitRule 领域 AST 的接入；在前述语义闭合前不替换已验证的专用展开器，也不启用写包。

这些边界是后续开发入口，不依赖继续捕获游戏行为。
