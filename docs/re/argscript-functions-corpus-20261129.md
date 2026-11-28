# 类型化变量、表达式函数与 H&V 源脚本闭环

2026-11-29。本轮实现 `probes/runtime/argscript_functions.py`，新增源代码审计入口 `argscript_corpus_audit.py`。原始 AEC → 新预编译器 → 既有领域 AST 的离线链路首次在本处理器组合下完成。没有读取 AEB/ERZ 做对拍，没有连接游戏进程、写包或部署。

## 结果与实际边界

从23份 `HeroesAndVillains-Scripts_257365692_*.aec` 读取两个入口 8F06F716、F6ED959D：

| 检查 | 结果 |
| --- | --- |
| 严格模式 | 在 HeroVillainRules.er2 第5行因 ThirtyGameMinutes 未定义而停止 |
| 符号保留模式 | 完成，输出2287行领域文本 |
| 宏 | 13个定义、66次成功调用，1723条宏体重放记录 |
| 领域 AST | 334条规则：331 UnitRule、3 GlobalRule；解析诊断0 |
| 外部变量 | 8项，均显式保留名称，不填猜测数值 |
| 写包许可 | 始终 false |

缺失的外部变量为 ThirtyGameMinutes、GameHour、GameDay、SixGameHours、Hour8、Hour19、Hour7、TenGameMinutes。这批 DLC 源文没有提供其所需的全局环境，不能将这些变量擅自替换成游戏分钟、实时时间或某个约定倍数。

符号模式以 `${名称}` 作为显式后备值，利用已恢复的单遍替换保留引用；每次发现新变量都从新宿主重新解析，避免重复保留前次尝试的副作用。它不尝试猜测布尔条件：若未知变量进入条件求值，仍会停止。严格模式的原始失败、包含链与发现过程在报告中单独保留。

0诊断仅指现有领域解析子集没有报错，不代表完整原生静态语义、全部引用链接、二进制编译或运行行为已验证。当前 AST 中的外部符号、既有 unknown/partial 契约和 packageCompilationAllowed=false 继续有效。

## 类型化变量赋值

`install_meta_extensions(host)` 显式安装 seti、setu、setb、setf；没有更改原 Preprocessor 的默认命令集，也没有更改旧整数/浮点转换器。

- seti：ParseInt → I32toa(base10) → SetVariableNoSubstitution。
- setu：ParseUint → U32toa(base10) → SetVariableNoSubstitution。
- setb：ParseBool → 字符串 1 或 0 → SetVariableNoSubstitution。
- setf：ParseFloat 返回值 → double 实参 → Snprintf(char[32], "%f") → SetVariableNoSubstitution。

赋值之前普通命令入口已经完成变量替换，不能再次调用带替换的 setter。求值失败不覆盖旧变量，也不执行后续行；错误保留参数内部相对位置，并归因到源参数引用。

setf 的静态指令在 0x900E65/0x900E68 明确以 `sub esp,8; fstp qword ptr [esp]` 传入 double，不是先加一次最终 float32 存储。新 FunctionEnvironment 的 real 返回入口保留表达式返回模型值，和已有供 float 目标写入的 parse_real 分开。例子：表达式 `16777216+1` 在此返回模型中可供 setf 存成 `16777217.000000`；旧 parse_real 的 float 写入边界得到16777216。

格式实现采用可移植的六位小数和31字符截断模型；非有限数的原生 printf 拼写尚未恢复，明确失败。没有声称已完全迁移 EA 自有 printf、x87扩展精度、非默认舍入或极端数值下的逐位表现。本轮默认 H&V 语料实际上只使用 set 赋值；类型化命令由独立定向测试验证，不混称真实语料已覆盖它们。

## 表达式函数注册

RegisterFunction 以原始名称为键，没有 MakeCaseInsensitive；同名覆盖，传 null 移除。FunctionEnvironment 保留该规则，和命令表/宏表的大小写归一化分开。

整数和布尔入口可调用显式注册的 ExprFunction；boolean 回调省略时按 integer 结果非零判真。注册函数负责从当前游标消费参数，表达式顶层仍检查完整消费。实数入口使用独立 real 回调，结果按 EvalRealNumber 的 float y 存储点转换。原生内置分支优先，注册不能覆盖 abs 等更早命中的内置语义；已有反编译疑点不因注册而被绕过。

显式安装的函数子集：

| 名称 | 已恢复行为 |
| --- | --- |
| match(string,pattern) | 不区分 ASCII 大小写的原生通配符规则 |
| eq(lhs,rhs) | 不区分 ASCII 大小写的完整字符串相等 |
| varExists(name) | 调用已有变量查找，空字符串值也算存在 |
| defExists(name) | 实时查询宏定义表 |
| commandExists(name) | 仅查全局命令表，不调用分层 CommandLookup |
| minVersion()/maxVersion() | 只有调用方明确提供两个边界时才安装，不猜默认值 |

这些布尔函数用于 real 路径时没有默默转成0.0/1.0，而是保留 boolean function 错误边界。注册对象与 Python 回调是研究宿主接口，不模拟 C++ 引用计数、析构或函数 vtable ABI。

## EvalString 与通配符的两个细节

EvalString 和普通参数切分不同：它支持递归括号包裹、双引号字符串；不带引号时停止字符是右括号、ASCII空格或逗号。开头会跳过一般空白，但内部 tab 不作为同一终止字符。没有加入 shell 转义。

WildcardMatch 先转小写，再调用 MatchPattern。后者只有 `*`、`?` 通配规则，方括号是普通字符；没有借用 Python fnmatch 的字符组语法。

实现还保留原生的分支顺序：只有“单个末尾星号”在空输入检查前直接成功。故空字符串匹配 * 为真，匹配 ** 为假；字符串 a 匹配 a** 也为假。不能先把连续星号折叠，再声称等同原生。离线实现使用动态规划和显式工作量防护，避免复制原生的递归爆炸，但保留这些语义。

## 来源绑定与领域观察器

AEC 文件以实例ID命名。审计工具将脚本字面 include 名称的 stem 经过已使用的 ASCII lowercase FNV-1，显式绑定至对应实例源文，记录 request、targetInstance、effectivePath 和原始 AEC SHA-256。这是对提取语料的可审阅别名绑定，不是声称恢复了开发机真实目录。缺失或冲突绑定硬失败。

领域观察器只注册明确列出的 UnitRule/GlobalRule 头及已有领域命令集合；未知命令停止，不从语料任意自动接受关键字。它记录执行后的文本与来源，并用宿主块栈验证规则边界。它不是恢复出来的 Eco C++ 命令注册表。

报告中的宏生成行继续使用生成坐标，附调用/定义链；普通原文行保留逻辑来源身份，原始文件身份通过 sourceFiles/includeAliases 映射。当前宏链可重建，但尚未整理为编辑器可直接消费的统一诊断位置契约。

## 验证和复现

```powershell
python -m unittest discover -s probes/runtime -p 'test_argscript_*.py'
python probes/runtime/argscript_corpus_audit.py --sources tmp/aec-extract --out tmp/argscript-functions-20261129/corpus-audit.json --ast-out tmp/argscript-functions-20261129/domain-ast.json
python probes/runtime/argscript_native_inventory.py --source 'D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/EA/ArgScript' --out tmp/argscript-functions-20261129/native --scope functions
```

累计164项离线测试通过：此前142项，新增17项函数/类型化赋值测试和5项作者自建隔离语料测试。覆盖大小写、覆盖/删除、回调消费、字符串/通配规则、实时存在性、版本显式输入、double返回差异、失败保留旧值、源文→宏条件→领域AST、外部符号保留、条件不猜测、未知领域命令及缺失include绑定。

另外单独运行上述真实23文件审计，得到表中结果。源证据范围新增22个反编译函数；固定开发版 EXE 的静态证据以 `argscript_real_evidence.py --setters` 增加 setf 指令区间，旧默认提取范围不变。

核心定位：SetB 0x900D50、SetI 0x900DA0、SetU 0x900DF0、SetF 0x900E40；RegisterFunction 0x8CA890、EvalString 0x8CA180；match 0x8FA570、eq 0x8FA3D0；WildcardMatch 0x8DD620、MatchPattern 0x8DD5C0。地址仅属于已固定身份的开发版。

## 后续优先级

基础预编译链已有真实源文闭环。接下来优先补齐源依赖清单与统一来源诊断，把8项外部时间变量追溯到真实定义，再完善领域命令签名和引用链接。include 根路径/磁盘契约、screate、完整 ConstructSpec 和浮点精度仍分别保留待办；在它们和领域契约闭合前不接入生产写包，不以当前0诊断放开编译。
