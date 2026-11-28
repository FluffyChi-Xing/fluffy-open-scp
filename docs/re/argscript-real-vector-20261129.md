# ArgScript 浮点与向量转换：静态指令校核和离线实现

2026-11-29。新增 `probes/runtime/argscript_real.py`，实现显式安装的浮点与向量转换模型；新增 `argscript_real_evidence.py`，归档固定开发版 EXE 的静态指令和常量。继续暂停零售版对拍与运行捕获。本轮没有执行目标代码、连接进程、修改游戏或接入生产构建。

## 指数调用与浮点存储疑点已经缩小

使用的文件是开发版 `SimCity.unwrapped.exe`，SHA-256 为 `03045b870a3b3c405af72e0cd21c07aed1dfa9f08c0b8dd12aece4bcbf1f056e`。地址基址为 0x400000，PE machine 为 x86。先固定身份，再使用 Capstone 只读反汇编。

StrtodEnglish 的反编译省略了 pow 的实参；指令表明：

- 0x89F016 从 0xC34118 读入 xmm0，常量字节解码为 double 10.0。
- 该寄存器在指数调用前保持底数；0x89F0DB 将有符号指数 edx 转 double，放入 xmm1。
- 0x89F0DF 调用 0xA2AFC8。负指数文本的符号由随后 1.0/result 路径处理。
- 同时核验 double 1.0、0.1 及 SSE 余数路径的 float 8388608、符号掩码和 1.0。

这些证据支持十进制指数计算，未证明宿主 libm 与开发版 libm 在所有输入上的最终位值一致。

EvalRealTerm 在起始及每次乘除后使用 `fstp m32` 存储中间结果；EvalRealExpression 每轮把左值 `fst m32`，运算仍经过 x87；括号及向量分量也有 float 存储。实现按这些已确认位置显式执行 float32 转换，而非仅在输出端转换。

最初尝试读取另一个 `Simcity Development Files Including PDB Debug files/SimCity.exe`，其哈希为 `fe4651fdc9c322947c42217d36148bd4a1e61eb1dd48596995b97a3b2ea185bd`，相同地址处的字节不对应上述函数。该尝试不作为代码证据；工具现对正确解包 EXE 的 SHA-256、架构、常量和完整指令消耗设置硬检查。不同文件不因同名而共用地址。

## 已实现的读取与表达式行为

`read_real` 按 StrtodEnglish 的顺序逐位累积整数与小数，返回 double 模型值与结束位置。不能直接用 Python float(text) 替代：原生小数位按 factor*=0.1 累积；例如 0.3 的中间值可能与标准库一次十进制转换不同。

原始读取允许尾部文本，指数部分也不要求至少一位数字：`1e+` 消耗至末尾并保留数值 1。没有数字时不会像整数读取那样必然恢复起点；符号、点或指数字符仍可能推进位置。NaN/inf 单词没有添加为原始数字语法。指数累积按 signed int32 建模，超长指数保留字回绕。

`RealExpression` 另行实现加减乘除、右结合乘方、负号与括号，顶层 `parse_real` 要求完整消费，返回供 float 目标写入的 float32 值。除零、溢出、无效根号等按常见 IEEE 默认非陷阱环境建模为 inf/NaN；宿主复杂度和编码防护继承现有表达式入口。

已实现的函数分支包括 sqrt、abs、floor、ceil、round、fmod、frac、remainder、pow。实数 `%` 按开发版 SSE 的 divss、加减 2^23、cmpnless、乘减序列建模，不能与 fmod 混用：-7%3 得到 2，fmod(-7,3) 得到 -1。

保留原生函数检查顺序与 Compare 的前缀行为，未自行修正名称冲突。特别是 **实数 sqr(4) 会先匹配 sqrt 分支，结果为 2；整数 sqr(4) 则为 16**。本轮起初的测试按通常平方含义期待 16，随后核对 Compare 的反编译及 0x8C8F60 静态指令后更正了预期；这不是已经执行原生函数得到的实测结果。该差异必须在未来教程中标明开发版与证据层。

## 向量与 RGBA 入口

支持 ParseVector2/3/4 及 ParseRGBA 对应路由：

- vector2/3/4 只有一个分量且后续到达 EOF 时，将首值复制到其它分量。
- 若出现第一个逗号，就要求对应的所有分量；例如 vector3 的 1,2 不会自动补最后一项。
- RGBA 必须先有 R/G/B；省略 A 时默认为 1.0，没有添加颜色范围钳制。
- 每个分量使用实数表达式并写成 float32。
- 原生向量函数最后一个分量之后没有统一 EOF 检查，因而 1,2,3,extra 可由 vector3 路径返回前三项。实现保留此入口差异，不能将顶层 parse_real 的完整消费规则套在所有向量入口上。

ParseRGB 的独立实现尚未定位，继续保持 ConversionRequest；没有把它凭名字替换为 vector3。pi/e、三角函数、注册回调及其它未恢复分支明确报缺口。

## 参数层接入与兼容范围

`numeric_converters()` 显式合并此前整数转换器及这些新增路由：

```text
StrtodEnglish.float32 / StrtodEnglish.float64
ParseFloat
ParseVector2 / ParseVector3 / ParseVector4 / ParseRGBA
```

旧的 `integer_expression_converters()` 保持原范围，默认解析器也不自动安装新转换器。double 参数仍走原始 StrtodEnglish 路由，即使提供 parser；同一文本 2+3，float 参数可得 5，double fallback 只读取前缀 2。

数组复用上一轮参数层的集合语义。错误仍保留此前参数写入、转换路由、原始参数位置和部分状态；失败不进入消费者。新增实现未改生产编译、持久化图、Actions、包写回或游戏资源。

**精度边界**：这是静态证据约束下的可移植语义模型。它显式保留已确认的 binary32 存储点及 SSE 逐步舍入，但使用宿主 binary64/math 实现中间算术和 libm，未完整模拟 x87 80 位精度、FPU/MXCSR 状态、NaN payload、非默认舍入和全部极端输入。因此 `fully_converted` 表示这条宿主路由已完成，不表示已证明原生逐位等价；尚不用于生产 ERZ 数值编码验收。

## 验证与复现

```powershell
python -m unittest discover -s probes/runtime -p 'test_argscript_*.py'
python probes/runtime/argscript_native_inventory.py --source 'D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/EA/ArgScript' --out tmp/argscript-real-20261129/native --scope real
python probes/runtime/argscript_real_evidence.py --binary 'D:/ea-games/simcity_dev/[WIN] SimCity 2013 (Dev Beta) (v253660365) [2013-01-14] (PDB)/SimCity.unwrapped.exe' --out tmp/argscript-real-20261129/dev-instructions.json
```

累计 105 项离线测试通过，其中新增 22 项覆盖原始读取、指数、double/float 差异、中间舍入、余数、函数顺序、IEEE 特殊值、向量广播/缺参/RGBA、末项尾部、数组、变量到向量消费者及失败状态。静态证据归档 17 个反编译函数、6 个指令区间和 6 项常量；EXE 与字节区间均保存 SHA-256。旧整数与参数基础设施回归一起通过。没有运行零售对拍测试或目标原生代码。

后续优先恢复宏、条件和 include 等预编译处理器；浮点仍需单独解决上述精度及函数覆盖缺口，再考虑生产编译接入。

后续进展：已新增宏/条件及显式来源表 include，累计142项离线测试通过，见 [预编译处理器](argscript-preprocessor-20261129.md)。不改变本文的浮点精度边界。
