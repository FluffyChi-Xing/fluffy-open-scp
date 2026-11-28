# ArgScript 整数与布尔表达式：数值读取、递归下降及接入

2026-11-28。新增离线模块 `probes/runtime/argscript_numbers.py`、`argscript_expression.py`，从开发版反编译恢复整数读取与整数/布尔递归下降主体。数字转换器通过显式参数接入上一轮 cArgumentSpec；不改变默认转换器配置，也不替换生产构建或 H&V 专用展开器。本轮没有零售包对拍或游戏运行捕获。

## 三个入口不能混用

| 入口 | 已实现语义 |
| --- | --- |
| read_integer | EA::StdC::StrtoU64Common 的 ASCII 整数读取，返回位值、结束位置、溢出和是否读到数字 |
| read_int32/raw_integer_converters | 模拟本研究中 AtoI64/StrtoI32 的 **32 位消费者边界**，不声称恢复所有 ABI 返回值 |
| parse_expression | cExpression 的 int/uint/bool 顶层入口，要求完整消费表达式 |

底层读取可接受数字前缀并留下尾部，如 `12suffix`。表达式入口则检查剩余文本，对同样输入报 `Garbage at end of integer expression`。不允许用一种“严格数字正则”代替所有入口。

## 整数文本读取

StrtoU64Common 的主体使用 64 位累积，支持前导 ASCII 空白、正负号、2～36 进制及 base=0；base=0 按前缀选十进制、八进制或十六进制，base=16 可跳过 0x。没有读到任何数字时，结束位置回到原始起点，包括此前跳过的空白和符号。

累计超过 uint64 范围时仍继续读取数字；无符号分支钳制到 UINT64_MAX，有符号分支按正负钳制至 INT64_MAX 或 INT64_MIN，并报告溢出；负号最终按 64 位取负。实现返回溢出标志，不写宿主进程 errno。

本轮看到 StrtoI32 的反编译返回类型与 mangled 声明不一致，AtoI64 又转调该函数。这里按已有 cArgumentSpec 写入 DWORD 或判定低 DWORD 非零的实际消费者恢复 `read_int32` 与 bool 转换，不把函数名当作“严格 32 位范围报错”的证据。例子：

| 文本 | 本轮 read_int32 的消费者值 |
| --- | --- |
| `2147483648` | -2147483648 |
| `4294967296` | 0 |
| `9223372036854775808` | 先钳制 INT64_MAX，再取低字，得到 -1 |
| `true` | 未读到数字，得到 0；这不是表达式 ParseBool 的结果 |

这些是静态反编译支撑的模型与测试预期，未执行原生函数。ABI、反编译类型冲突与原始机器指令的进一步核验仍是独立证据层。

## 整数表达式语法与顺序

```text
Expression → Term { ('+' | '-') Term }
Term       → Factor { ('*' | '/' | '%') Factor }
Factor     → SignedNumber [ '^' Factor ]
SignedNumber → '+' SignedNumber | '-' SignedNumber | Number
Number     → integerLiteral | '(' Expression ')' | supportedFunction
```

实现对应 EvalIntExpression/Term/Factor/SignedIntNumber/Number 的调用结构，而非借用 Python eval。结果按 32 位有符号值建模；加减乘和乘方的机器字回绕使用显式 int32。除法向零截断，余数为 `lhs - trunc(lhs/rhs)*rhs`。

| 表达式 | 模型结果 | 原因 |
| --- | --- | --- |
| `2^3^2` | 512 | 右结合乘方 |
| `-2^2` | 4 | SignedNumber 先处理负号 |
| `2^-3` | 1 | 原生乘方循环只在指数大于零时执行 |
| `-7/3`、`-7%3` | -2、-1 | 向零截断，与 Python //、% 的负数规则不同 |
| `010` | 10 | 表达式读取前跳过部分前导零 |
| 原始 read_integer(`010`, base=0) | 8 | 没有表达式层的前导零预处理 |

数字 literal 经无符号 64 位读取，再按 int/uint 模式检查上限。int 模式中 `-2147483648` 会在负号应用前因正 literal 超范围而失败；`-2147483647-1` 则可得到最小值。uint 模式放宽 literal 到 UINT32_MAX，内部运算仍依源代码的 int 变量建模，并非把整个表达式换成 Python 无符号算术。

支持已直接恢复的 abs/sqr 整数函数。Compare 函数只比较 identifier 区间长度，因此存在前缀匹配，如 a(-3) 进入 abs 分支。其它函数、注册回调及整数 true/on/false/off 常量暂未迁移：这些常量在 EvalIntNumber 的若干反编译调用点出现 b/e 区间颠倒，不能直接照文本认定真实行为；遇到时报告未恢复分支。

## 布尔表达式

布尔层按原生调用关系实现：or → and → 递归 not/! → 关系运算 → bool atom 或 flags=2 的整数表达式。

- and/or 都会先解析右侧，即使左侧已经能确定真假。因此 `true or 1/0` 和 `false and 1/0` 都到达除零防护。
- 关系表达式右递归，所以 `3>2>1` 被建模为 `3>(2>1)`，不能套用 Python 的比较链规则。
- `not`、`and`、`or` 按原生字符匹配，区分大小写；没有添加词边界，也没有添加 &&、|| 语法。
- bool atom 中的 true/on/false/off 只比较已扫描 identifier 的长度，存在 t/tru/f/of 等前缀。此处与 EvalIntNumber 的可疑常量分支分开恢复。
- flags=2 时整数括号内部转入布尔表达式，例如 `(1<2)+1==2`。

原生错误文本如 empty expression、Bad integer expression、Out of range、Expected ')'、Garbage at end ... 被分别保留。非 ASCII、内嵌 NUL、表达式复杂度、除零或 INT_MIN/-1 等使用明确的宿主防护错误；它们不是声称原生会抛出同样的 cError。对于可疑或未实现函数，同样保留“未恢复”而非宣称语言不支持。

## 接入与失败可见性

`integer_expression_converters()` 返回 ParseInt、ParseBool 和已有原始整数消费路由的显式映射。cArgumentSpec 调用方选择是否安装它，默认未安装时仍产生 ConversionRequest。

```python
parser.register('number', checked_command(
    ArgumentSpec(arguments=(ArgInfo('n', 2, destination='n'),)),
    consume,
    converters=integer_expression_converters(),
))
```

文本入口仍先做 ArgScript 变量替换，再切分参数，再按规范转换。因此 `$n` 可以保留整体表达式参数，随后由整数递归下降求值。

转换器抛错时，参数层现在记录 conversion route、argvIndex、此前的写入与旗标。执行闸门将错误归因至原始参数区间，另存参数内部的 relative span；Parser 保留失败 invocation，后续行不继续，消费者不运行。没有伪造替换值内部逐字符来源，也没有静默用其他转换器重试。

## 验证与证据

```powershell
python -m unittest discover -s probes/runtime -p 'test_argscript_*.py'
python probes/runtime/argscript_native_inventory.py --source 'D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/EA/ArgScript' --out tmp/argscript-numbers-20261128/native --scope numbers
```

累计 83 项测试通过：此前 57 项，加本轮 26 项数值、表达式和接入测试。覆盖原始读取边界、溢出、参数错误、前导零、优先级、负数除余、字回绕、布尔右侧求值、比较结合性、复杂度、变量替换到消费者，以及失败时保留部分状态。未运行历史零售对拍测试。

numbers 证据范围复制 32 个原样开发版反编译函数并记录 SHA-256，包括：

| 函数 | 开发版 EA |
| --- | --- |
| StrtoU64Common | 0x89F130 |
| StrtoI32、StrtoU64、AtoI64 | 0x89F6D0、0x89F710、0x452CD0 |
| cExpression/ParseInt、ParseUint、ParseBool | 0x8CB240、0x8CB2C0、0x8C9500 |
| EvalIntExpression、Term、Factor | 0x8CB030、0x8CAF90、0x8CAF10 |
| EvalIntNumber、SignedIntNumber | 0x8CAAF0、0x8CAEC0 |
| EvalBoolExpression、Term、Factor | 0x8C9480、0x8C9400、0x8C93A0 |
| EvalRelExpression、EvalBool | 0x8C91E0、0x8CB0B0 |
| anonymous/Compare | 0x8C8F60 |

这些地址仅用于开发版静态定位。

## 当前仍未完成

后续进展（2026-11-29）：十进制指数实参已由固定开发版解包 EXE 的静态指令核清；新增浮点与向量的可移植模型，累计 105 项离线测试通过。详见 [浮点与向量转换](argscript-real-vector-20261129.md)。下述初始缺口中的指数定位已完成，但 x87/libm 逐位等价、RGB 和部分函数仍未完成。

浮点、double、vector 和 color 路由继续保留 ConversionRequest。StrtodEnglish 的指数路径在现有反编译文本中丢失 pow 调用实参；实数运算还涉及 float 中间存储、x87/libm 和 SSE 余数计算，不能直接用 Python float/pow 冒充逐位等价。此次已经取证对应函数，下一步专门核清这些缺口。

其它待办包括完整数学内置函数、表达式函数注册及回调、整数常量反编译疑点、宏和条件处理器、完整 ConstructSpec 声明编译、include 与目录栈。以上均不影响继续按离线基础设施顺序推进。
