# ArgScript 参数规范：cArgumentSpec 的参数消费与转换边界

2026-11-28。继续开发版反编译支撑的离线基础设施；本轮未运行零售版 AEB 对拍或连接游戏探针。

新增 `probes/runtime/argscript_argument_spec.py`：规范化的 ArgInfo/OptionSpec/EnumSpec、参数消费器、类型转换请求、部分写入日志，以及接入上一轮 Parser 的执行闸门。现有 H&V Expander、领域解析器和产品构建路径没有迁移。本文中的规范声明属于 C++ 命令注册元数据，不是新增 ER2 关键字。

## 两种参数视图必须并存

开发版 `cArguments::AsArgV` 返回完整的 `mArguments`；`Chomp` 只调整主参数视图的起点与长度，不修改这份存储。`cArgumentSpec::Parse(cArguments*, parser)` 先取回完整 argv，再标记所有选项已处理，然后调用另一 Parse 重载。非零返回值会包装为 cError 抛出，包括请求帮助。

因此 Parser 的 invocation 现在保留 `argv`（含命令名）和已有 `arguments` 视图。新的参数消费器读取 argv，从索引 1 开始；不会从 arguments 视图重建或重排输入。

| 输入 | 参数视图与规范消费的区别 |
| --- | --- |
| `cmd "-help"` | 分词时是带引号的主参数；AsArgV 的值已无引号，规范消费器再次按 `-`＋字母识别为选项 |
| `cmd -x optionValue mainValue` | 分词视图将两项都归到 x；若 x 的规范只读一个参数，mainValue 随后仍可填主参数 |
| `cmd first -x optionValue second` | 规范消费器可在选项前后继续填主参数，不把整个尾部固定归入最后一个选项 |

选项名按 ASCII 不区分大小写比较。`-3`、`-.5` 不属于 `-`＋字母选项；未声明的选项返回 5。没有自动添加 Unix 风格 `--` 终止符语义。

## 依赖参数、集合和重复选项

ConstructSpec 将每个参数记为 `mIsDependent`，并非简单给每项标注 required/optional：

```text
required [a b]
对应 dependent：true, false, true
只有 required：通过
required 与 a：缺少 b，失败
required、a、b：通过
```

原生通过可选括号的深度与前一参数记录的深度比较来生成依赖位。`dependency_slots` 按该过程从显式 token/ArgInfo 恢复依赖位；类型和目标位置由宿主显式提供。尚未把完整的 C++ 变长参数 ConstructSpec API 迁移为声明字符串编译器。

两个集合标志不同：

- `0x4000`：一个参数内部调用 SplitIntoArguments，再逐项追加，包含该次嵌套切分识别为选项的 token。这里的 NumArguments/operator[] 遍历完整存储。
- `0x8000`：最后一个位置可重复消费外层参数；直接调用 ParseVectorArgument，不再次切分每项。ConstructSpec 的末尾 `...` 会给前一项同时加上 `0xC000`。

例如外层一个值为 `a b` 时，string[] 得到两项；尾部 `...` 的直接追加路径保留一项 `a b`。末尾重复项的空值检查还有差异：主参数的重复末项可以没有值；选项下 dependent=true 的重复项若一次也没消费，返回缺参。

重复出现的选项逐次消费：标量目标会产生多次覆盖写入，vector 目标逐次追加。实现保存每次写入及 argvIndex，不把它们提前合并成字典而丢失顺序。

参数/选项旗标是 OR 操作，解析调用开始时清零；选项旗标在参数校验前就设置，失败也保留。普通 ParseArgument 会先设置参数旗标；末尾重复项直接调用 ParseVectorArgument，因此跳过这个参数旗标设置。实现及测试保留了这个不对称。

## 类型转换的实际分流

| 类型 ID | 无 parser 的转换 | 有 parser 的转换 |
| --- | --- | --- |
| 1 bool | 标量 AtoI64；vector StrtoI32(base10)，再判非零 | ParseBool |
| 2 int | 标量 AtoI64；vector StrtoI32(base10) | ParseInt |
| 3 float | StrtodEnglish，再写 float | ParseFloat，再写 float |
| 4 double | StrtodEnglish | 仍走无 parser 的 fallback，不调用表达式 ParseFloat |
| 5 cstr、6 string | 原始参数值 | 同左 |
| 7/8/9 vector2/3/4、10/11 RGB/RGBA | 本函数不支持，bad spec | 对应 ParseVector/ParseRGB(A) |
| 12 起枚举 | 按枚举表查 token | 同左 |

本轮可靠实现了字符串和枚举值；枚举先查帮助表，再查普通表，不区分 ASCII 大小写。未知枚举返回 6，并保留之前的写入和旗标。数字/向量/颜色转换保留为带有准确 route 的 ConversionRequest，等待对应 StdC/表达式实现；可由宿主显式提供已验证的转换函数。

这点避免把原生的文本数值转换与表达式求值混为一谈。比如同为文本 `2+3`，ParseInt 路径和 StrtodEnglish 路径不能共用 Python eval。原生函数不检查转换尾指针，未来不能自行加入严格整串匹配并宣称等同原生。

对于 null mLocation，多数标量/集合分支直接跳过转换；实现以 destination=None 表达这一点。原生标量枚举成功分支则直接写指针，宿主对此使用明确的防护错误。未模拟原生内存别名、C 字符串生命周期、浮点写入舍入和数值溢出。

## 返回状态与执行闸门

从开发版类型头恢复的 tArgError：0 成功、1 帮助、2 缺参、3 过多主参数、4 bad spec、5 未知选项、6 未知枚举。

`parse_argv` 返回 SpecResult，包括原始状态码、消息、旗标、部分写入、消费位置和未完成转换。`native_code == 0` 只说明这一轮结构消费未返回原生参数错误；必须同时没有 unresolved 才能得到 `fully_converted == True`。这是研究宿主对转换缺口的明确表示，不是假设原生引擎也存在 ConversionRequest。

`parse_invocation` 提供保留返回状态的低层适配，选项在失败时也标记已处理。`checked_command` 则模拟原生 throwing wrapper 的执行控制：

1. 保存参数结果。
2. 非零原生状态抛 ASC008，保留原始 native_code。
3. 仍有转换缺口抛 ASC009，区别于原生语法错误。
4. 只有完全转换成功才调用消费者。

Parser 在回调抛出 ArgScriptError 时，现在也保留 invocation。因此未知选项、转换缺口及部分写入可以在失败 trace 中检查，后续行不执行。错误引用回到原始 token 或 EOF；尚未增加嵌套 vector 内部的逐子项位置映射及宏/include 调用栈。

## 验证与复现

```powershell
python -m unittest discover -s probes/runtime -p 'test_argscript_*.py'
python probes/runtime/argscript_native_inventory.py --source 'D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/EA/ArgScript' --out tmp/argscript-arguments-20261128/native --scope arguments
```

57 项离线定向测试通过：上一轮 27 项加本轮 30 项，覆盖依赖组、参数与选项交错、集合两条路径、重复选项、帮助优先级、失败部分状态、数值分流、枚举、带变量替换的文本入口、执行闸门及原始错误位置。测试不依赖零售包或游戏进程。

证据工具的 arguments 范围归档 19 个原样反编译函数和 3 个类型头，记录 SHA-256；默认 pipeline 范围继续保留上一轮 21 函数，不改变原调用方式。核心函数地址：

| 函数 | 开发版 EA |
| --- | --- |
| cArgumentSpec/Parse、Parse__2 | 0x8B1AE0、0x8B1CD0 |
| ParseOption、ParseOptionArgs | 0x8B1980、0x8B1830 |
| ParseArgument、ParseArgument__2 | 0x8B1300、0x8B1580 |
| ParseVectorArgument、ParseVectorArgument__2 | 0x8B0E40、0x8B10F0 |
| ConstructSpec | 0x8B1F90 |
| cArguments/AsArgV、NumArguments、operator[] | 0x8A4420、0x8A41E0、0x8A41F0 |
| ParseEnum 普通表、帮助表 | 0x8A4AA0、0x8A4B10 |

## 下一层开发

后续进展：原始整数读取及 int/uint/bool 表达式主体已新增，可显式安装到转换路由；累计 83 项离线测试通过。见 [整数与布尔表达式](argscript-integer-expression-20261128.md)。浮点和向量转换仍保留缺口。

继续恢复表达式入口与 StdC 数字读取，填补 ConversionRequest；然后实现宏和条件处理器。ArgTypeFromName/FindNameAndTypeFromOption 已取证，但类型缩写、格式串和完整 ConstructSpec 尚未实现，不能把本轮 normalized schema 当作完整原生声明语法。文件 include、目录栈、队列及生产领域 AST 接入仍待后续。
