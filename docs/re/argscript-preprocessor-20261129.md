# ArgScript 宏、条件与 include：原生处理器的离线实现

2026-11-29。本轮新增 `probes/runtime/argscript_preprocessor.py`，将开发版反编译中的宏和条件处理器接入已有 Parser，并以调用方提供的源文本表实现 include 流程。没有读写游戏目录、运行游戏、连接探针或进行零售 AEB 对拍。

## 实际执行链

```text
含路径身份的来源表 → parse_source → Parser.parse_stream
  → 普通命令或当前元命令处理器
  → define 保存文本；create 替换参数后重入 parse_line
  → if/elseif/else 决定继续解析还是消费本行
  → include 递归进入新源流，沿用变量、宏、块和注释状态
  → 注册的领域命令消费者
```

这是基础设施接入点，不是完整 Eco 编译器。测试中的 emit 是宿主注册的记录回调，不是本轮对原生 output 的迁移。现有 H&V 专用展开器、UnitRule 领域解析器和生产写包链保持原实现。

## 宏定义和实例化

原生 cDefineCommand 要求两个主参数：名称和一个参数列表字符串。因此 `define M(x y)` 由已有分词器拆为 define、M、括号组 x y，参数列表再切一次。调用 `create M(a b)` 同理。

- 定义体由元命令 Parse 保存，位于普通变量替换和参数切分之前；保存的是去注释后的非空文本及原始行号。
- 遇到整行 enddef 才提交定义；错误清理不提交未结束定义。
- define 对已存在名称报 Already defined；redefine 可替换，并在正常 enddef 时生效。
- 宏名称在独立表内按小写存取；不自动加当前 namespace。
- 参数名按原文区分大小写，重名参数引用命中第一个。
- 调用时比较完整的参数 token 数，不只比较主参数视图；错误参数个数在重放宏体前失败。
- 宏体不是隔离函数作用域。它沿用调用方变量作用域和块栈，也可以打开一个由调用方后续关闭的普通块。

`SubstituteParameters` 的规则已作为纯函数实现：

```text
值 x 为 a b
&x      → (a b)
&{x}    → a b
&{x}_id → a b_id
```

参数名首字符允许 ASCII 字母或下划线，后续允许数字；不支持变量替换层所允许的冒号。引号不会屏蔽参数引用；单遍扫描不递归替换插入值。分别保留 missing parameter name、bad parameter name、missing '}'、Unknown parameter 错误类别。

参数替换后，宏体行重新进入普通命令入口，因此先重新去注释，再替换 `$变量`，再切分参数。比如宏实参插入 `# comment` 可以改变重放行的注释；这与普通变量在注释处理之后插入 `#` 的行为不同，不能合并成一次通用模板替换。

宏定义体的终止仍由当前元命令整行匹配决定，没有自行增加“嵌套 define 自动配对”的语义。undefine 与 sundefine 已实现，后者仅对不存在的定义保持静默。

## if 的两个状态位

实现按原生共享 cIfCommand 的栈记录 active 与 triggered：进入 if 时总是先求布尔条件，再结合父层 active；父层不活动时将 triggered 设为真，防止子层 else 激活。

原生元命令 Parse 的分支顺序已保留：

| 当前行 | 行为 |
| --- | --- |
| 以小写 if 开头 | 返回“不消费”，送回普通命令入口；不是 token 边界判断 |
| 整行小写 else | 未触发过子句才激活，否则停止当前子句；消费本行 |
| 以小写 elseif 开头 | 已触发过则跳过；否则替换变量、切分、去掉首个主参数并求一个布尔参数 |
| 其它行 | active 时放行，inactive 时消费 |

因此产生几个重要差异：

- `if false` 内的普通未知命令、缺失变量或坏括号可以被直接消费。
- 其中嵌套的小写 if 仍会经过替换和条件求值，`if $missing` 仍可报错。
- inactive 分支里的大写 IF 不匹配这个小写前缀检查，可能被消费；普通命令表的不区分大小写行为不能替代此检查。
- iffy 也符合 if 前缀条件，会返回普通分派并产生未知命令，而不会被 inactive 分支吞掉。
- elseifAnything 也进入 elseif 分支，原生没有在这里再次核对首个 token 的名称。
- endif 整行结束由 Parser 自己处理，按不区分大小写比较；与 else/elseif 的比较方式不同。

上述均为开发版静态分支的模型，不作为所有零售版本的运行结论。布尔表达式使用上一轮的 int/bool 子集；未恢复函数仍明确失败。

## namespace、set 与普通块

namespace/end 接入已恢复的变量 scope 栈。正常 end 按原生 cEndCommand 先弹块再调用结束回调；错误清理由 Parser.finish(True) 维持已有栈清理路径。namespace 关闭后不删除作用域中存下的变量。

一个明确的宿主防护差异：原生 namespace 在验证参数前先推块；研究实现先验证名称，以免缺名后错误清理触发未定义的 PopNamespace。正常输入路径相同，不能将这一防护称为原生错误路径的逐步等价。

set 的赋值形式使用两个主参数，并调用 SetVariableNoSubstitution：普通命令入口已经做过一次变量替换，不能再调用替换型 setter。零/一个参数的输出和检查形式本轮明确未实现；类型化 seti/setf 等未安装到这个处理器集。

## include：源文本表与目录栈

调用方显式传入 `{effective_path: source_text}`；没有隐式读取磁盘、搜索全局目录、规范化 `..` 或猜测扩展名。文件记录的 SHA-256 对象是 UTF-8 编码后的输入文本，不是声称持有原始文件字节。

已实现流程：

1. parse_source 以有效路径的父目录 PushDir。FindParentDir 保留最后一个斜杠/反斜杠；没有分隔符时使用 ./。
2. 普通相对 include 直接将当前 base 与请求名连接；字母盘符加冒号的名称不加 base。
3. PushDir 替换 base，不进行相对路径拼接，也不自动补分隔符；PopDir 恢复此前 base。
4. 子流共享宏、变量、普通块、元命令及块注释深度。子流结束时继续执行 Parser 的相对栈深度检查。
5. include 找不到来源报错；sinclude 找不到来源跳过，但已取得的源文本中出现解析错误仍传播。

宏展开只更新来源身份，不会切换到定义文件目录。因此宏体 include 使用调用时的目录 base；已有测试覆盖“宏定义于 defs/、调用于 city/、包含 city/child”。

根路径/UNC 的原生检测片段仍需核清，当前以 ASM015 明确拒绝；来源表也没有模拟 stat/open 权限或共享失败。循环包含、32 层深度、宏递归32层和累计10000行是研究宿主的显式防护，不是声称原生有相同限制。宿主通过 finally 做目录弹出和活动文件栈清理；没有宣称原生异常路径的目录状态已经完全恢复。

screate 依赖 InstantiateMacroWithResult 的异常包装，其现有反编译没有显示完整捕获行为。当前注册为明确的 ASM019 缺口，而不是擅自解释为“只忽略不存在的宏”或吞掉全部错误。

## 来源与失败传播

原 Parser 的错误包装在重入时可能用外层文件覆盖内层失败。本轮修正为保留最深层错误位置，并让各层 trace 保存本层 invocation；包含链和宏展开链另外记录。

宏体原文、参数替换结果 pieces、定义位置和调用位置分开保存。重放行使用 `<macro ...>` 虚拟来源，标明 generated；生成字符偏移不冒充原始定义文件中的字符位置。宏参数替换的错误也采用这一坐标边界。

普通 if 条件失败可归因至原始参数引用；elseif 分支目前缺少完整去注释/替换位置映射，故归因至整条原始行，保留表达式相对位置而不伪造列号。失败不撤销此前已经完成的变量赋值、定义或消费者效果；后续行停止。调用方仍需显式调用 finish(True) 做错误栈清理。

## 验证与证据

```powershell
python -m unittest discover -s probes/runtime -p 'test_argscript_*.py'
python probes/runtime/argscript_native_inventory.py --source 'D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/EA/ArgScript' --out tmp/argscript-preprocessor-20261129/native --scope preprocessor
```

累计 **142 项离线测试通过**，本轮新增37项。覆盖延迟定义、提交/替换/删除、参数规则、调用作用域、跨调用普通块、嵌套/递归、条件原生前缀、inactive 分支、elseif 跳过、include 共享状态、宏定义与调用目录、跨流块注释、缺失来源、栈平衡以及最深层错误链。旧变量、参数、整数和浮点模型一同回归；没有调用零售 AEB 对拍或游戏探针。

证据工具 preprocessor 范围归档31个反编译函数，记录原始文件 SHA-256，包括：

| 函数 | 开发版 EA |
| --- | --- |
| cDefineCommand::Execute/Parse/OnEndParse | 0x8FE470 / 0x8FE610 / 0x901730 |
| cCreateCommand::Execute、SubstituteParameters | 0x8F90F0 / 0x8FD0E0 |
| cParser::InstantiateMacro | 0x8FDAF0 |
| cIfCommand::Execute/Parse/OnEndParse | 0x8FE150 / 0x8FA790 / 0x8FA770 |
| cNamespaceCommand::Execute/OnEndBlock | 0x8F8FE0 / 0x8F9020 |
| cEndCommand::Execute | 0x8F8CF0 |
| cIncludeCommand/cSincludeCommand::Execute | 0x8FC5D0 / 0x8FC830 |
| PushDir/PopDir、FindParentDir | 0x8FF8C0 / 0x8FCD90 / 0x902650 |
| cArguments::MainArguments | 0x8A4200 |

尚未完成：完整 include 文件系统与根路径规则、screate 异常语义、队列模式、类型化 set 与表达式回调、完整 ConstructSpec、原生领域命令注册表及生产 AST 接入。后续应先补齐这些契约，再决定怎样替换旧的专用展开器，不用本轮子集宣布全体 ER2 已支持。

后续进展：新增显式类型化 set 与表达式函数扩展，23份 H&V 源文在符号保留模式下完成预编译并产生334条领域规则，累计164项离线测试通过。严格模式仍因外部时间变量缺失而停止；见 [函数扩展与真实语料闭环](argscript-functions-corpus-20261129.md)。
