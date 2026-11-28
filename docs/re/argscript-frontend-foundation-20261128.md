# ArgScript 解析基础：原生契约、源码保留与展开后的规则 AST

2026-11-28。接续UnitRule第三课。本轮新增独立的只读研究工具，没有改变已有宏展开器、生产构建器、工作台、Actions、包写入或运行探针。

后续已补一批领域类型与二进制核对，见`unit-rule-domain-contracts-20261128.md`。本文保留初版解析基础及当时的验证范围；当前实现仍未允许包编译。

## 本轮产物

- `probes/runtime/argscript_syntax.py`：保留原始文本、换行、token及来源区段的扫描器。
- `probes/runtime/unit_rule_frontend.py`：对**已展开**的H&V规则建立AST，资源操作、动态读bin、引用与双边chain有明确结构。
- `probes/runtime/unit_rule_frontend_audit.py`：批量扫描语料，并把资源AST重新表达后与晚版AEB字段比较。
- `probes/runtime/test_hero_villain_frontend.py`：原生词法例子、错误诊断、整个语料和数量作用域变更的回归检查。

这提供了可运行的解析基础，不是完整EA::ArgScript重实现。`domainValidation=partial`、`packageCompilationAllowed=false`保存在AST结果中；无诊断不等于已经证明可以编译游戏包。

## 1. 从原生代码确认的解析顺序

依据本地开发版反编译`docs/source-code/SimCity-dev-ida.c`，不是零售版运行时解析器的实机差分结果。提取的源码片段、原始行号与哈希保存在：

```text
tmp/hero-villain-language-20261128/frontend/native/
```

`cParser::ParseCommandInternal`的顺序是：

1. `StripCommentsAndWhiteSpace`，携带跨行blockCommentDepth。
2. 若处于meta命令中，先检查关闭标记、调用meta Parse；被meta消费的内容不会直接执行领域命令。
3. `SubstituteVariables`。
4. `cArguments::SplitIntoArguments`。
5. CommandLookup，再执行相应命令；找不到命令时报错。

`InstantiateMacro`还会按小写化的宏名查找定义，检查实参数量、替换参数、恢复定义路径/行号，再重新进入解析。

因此不能先把原始宏文本当成普通UnitRule块检查。例如条件分支可以分别生成不同的规则头，随后共享正文与end；在未经条件展开的文本上推断领域嵌套，会把合法生成逻辑误报。原始扫描结果用于保留与查看；领域结构检查只消费展开后的有效流。

## 2. 不能照搬 shell 的词法

`GetToken`只是一个按传入分隔符切片的辅助函数，不是整个ArgScript词法的定义。实际参数行为应查看`SplitIntoArguments`。

| 形式 | 已核对的原生行为 | 当前工具 |
| --- | --- | --- |
| `# 普通注释` | 截断该行后续内容 | 保留raw，clean去掉注释 |
| `#< ... #>` | blockCommentDepth支持跨行、嵌套；多余关闭标记报错 | 保留开启/关闭标记位置及深度 |
| `"a b"` | 作为一个参数，外层双引号不进入参数值 | quoted token保留raw与value |
| `(a nested(x,y))` | 括号深度分组，外层括号不进入参数值 | group token，内部内容原样保留 |
| `Foo(a b)` | 名称与括号组分成两个参数 | `Foo`、`a b` |
| `'a b'` | 注释清理阶段会保护成对单引号中的#；参数拆分阶段不把单引号当分组 | 两个普通参数`'a`、`b'` |
| `-or A -or B` | option记录是有顺序的列表 | 两个or，不能字典覆盖或去重 |
| `-3`、`-.5` | 第二字符不是ASCII字母，不属于命名option | 普通参数 |
| `"-or"` | quoted参数，不是option | 保持普通参数 |
| 单独`-` | option has no label错误 | AS005 |

原生双引号参数扫描寻找下一个双引号，没有这里常见的shell/C风格反斜杠转义循环。注释清理函数也把成对反斜杠当作一类屏蔽区段处理。当前扫描器按该证据保留相关行为，不额外解释`\n`或`\"`。非ASCII ctype行为与复杂路径边界仍需专门验证。

注释删除不会自动插入空格：

```text
foo#< comment #>bar
```

清理后是`foobar`。它的一个token来自两个不连续源码区段，因此token同时记录`sourceSegments`与原始包围范围；只记录一个“删除注释后的列号”会把编辑器定位弄错。

## 3. 源码位置约定

原始文档保留完整source和各行raw，包括CRLF；位置采用Unicode码点，line/column从1开始，start/end从0开始且end不包含。只把CR、LF、CRLF当物理行边界，垂直制表符仍是同一行内的空白。

对于展开后的AST：

- generated位置指向按序拼接的展开文本，路径标记为`<expanded>`。
- origin单独保留源文件、原始行、宏定义与调用链。
- 不把生成文本的列号伪装成原始宏调用中的精确列号。精确的变量/宏替换片段映射尚未实现。

扫描器接收的是解码后的文本；CLI使用utf-8-sig读取时会去掉BOM。批量审计另外记录原文件字节SHA-256，不用文本哈希冒充原文件字节身份。

## 4. 领域 AST 的第一批类型

当前解析`unitRule`和`globalRule`块，识别19种命令名：语料实际使用18种，另保留已知nearby资源作用域。识别名称不等于完整验证每一种原生参数文法。

资源操作例：

```text
local CriminalHome out Criminal@local
```

形成的核心结构为：

```json
{
  "kind": "resourceOperation",
  "scope": "local",
  "resource": "CriminalHome",
  "verb": "out",
  "quantity": {
    "kind": "binRead",
    "resource": "Criminal",
    "scope": "local",
    "remainingCapacity": false
  }
}
```

`!Space@local`则记录remainingCapacity=true，不当作布尔取反。整数、读bin、外部变量分别分类。外部变量即使出现在暂时保留原文的rate/timeTrigger中，也会列入unresolvedVariables，不能因该命令暂未详细建模而丢失。

`chain B`生成success和fail两条显式引用。`onSuccess -createUnit PanicMarker (0,3,0)`没有命名规则后继，仍须保留其option动作，不能误判成丢了目标。

UnitRule和GlobalRule符号空间分开。仅本次语料外的规则引用标记`external-unresolved`，不直接判定不存在；最终需要链接主游戏及其它注册RuleSet才能决定。

应用次数、flags、时间条件、事件与options的完整领域文法尚未全部降低为执行IR。其中保留原始参数/token，不能自行补默认值，也不允许据此写ERZ。

## 5. 诊断范围及兼容性边界

词法诊断：AS001多余块注释关闭、AS002缺双引号、AS003多余右括号、AS004缺右括号、AS005空option、AS006未闭合块注释。

领域诊断区分：规则嵌套、头参数错误、同种规则重名、end不平衡、未展开meta/未知顶层、未知命令、资源参数错误、超出已验证范围的数量语法、后继参数错误、应用次数形式不支持等。未知命令节点仍保留在输出里，同时令success=false。

这些诊断不是完整原生兼容性声明：

- AS006对EOF未闭合注释及同种规则重名检查是当前工具的明确约束，不能仅凭此称为已经验证的原生报错行为。
- 原始宏体或未激活分支中的扫描问题可能属于延后处理的文本；不能将原始扫描诊断直接等同于原生编译失败。
- 全部命令的合法上下文、参数类型、重复声明语义、表达式函数、作用域变量查找、include和宏执行仍有未实现部分。
- 既有H&V Expander未修改。它是受限研究工具，不能因本轮增加扫描器就宣称它已支持通用ArgScript。

## 6. 复跑结果

```powershell
# 扫描单个原始文件，保留文本与token位置
python probes/runtime/unit_rule_frontend.py tmp/aec-extract/HeroesAndVillains-Scripts_257365692_573AD901.aec --out tmp/hero-villain-language-20261128/frontend/villain-source.json

# 展开语料、解析并核对已支持的二进制字段
python probes/runtime/unit_rule_frontend_audit.py --source-dir tmp/aec-extract --binary tmp/rosetta/hv_277156632.aeb --out tmp/hero-villain-language-20261128/frontend

python -m unittest discover -s probes/runtime -p 'test_hero_villain_*.py'
```

结果：23份原始脚本无扫描诊断；展开331条UnitRule、3条GlobalRule，无结构诊断。对331条UnitRule，资源操作从新AST的作用域、资源、动词和数量类型重新表达，再进入已有独立字段核对：3732项检查，3730一致。

两项差异仍为`VillainBase_SendSkill_E2/E3`的transport目的地/时限，属于此前已经确认的早期AEC与晚版AEB版本差异，没有静默修正。GlobalRule的二进制字段、未建模的UnitRule字段不计入本轮一致性结论。

35项相关Python测试通过，包含新增15项前端检查。测试还故意把`CriminalHome out Criminal@local`的读源改成global，确认二进制核对会发现新增差异，避免“AST有类型但验证仍只看原文”的空验证。命令行错误路径检查确认返回非零，并先写出可审查的诊断及原始来源位置。

## 后续可以直接接续的工作

下一步恢复Eco领域命令注册与Execute参数契约，逐个把应用次数、flags、timeTrigger、options和事件从原文保留提升成受验证的AST节点；再实现变量作用域、include和通用宏语义。编辑器与生产构建接入应在这些契约明确后进行，继续区分解析成功、链接成功、二进制等价和游戏运行验收。
