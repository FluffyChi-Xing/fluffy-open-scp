# EA::ArgScript 语法与校验规则——逆向全解（2026-10-01）

> 来源：SimCity.exe（10.7MB，已脱壳、Ghidra 友好）内置解析器的
> **错误信息字符串 + 内置帮助文本 + 命令注册表**（全部为静态字符串提取，
> 无需游戏运行）。用途：OpenSCP 内置 Monaco 编辑器的语法高亮/linter/
> 校验规则直接消费本文档。

## 一、ArgScript 是什么

EA 的文本命令语言（EA::ArgScript，RTTI 实锤
`.?AVcCommandBase@ArgScript@EA@@` / `cICommand` / `cError` /
`CommandLine@EA`）：**行式命令流**——每行一个命令 + 空格分隔参数；
块命令以 `end` 收尾；支持条件、宏、变量、命名空间、include。
SimCity 里两处使用：shader 容器（0x0469A3F7，片段定义）与
EcoGame 规则包（type 0x08068AEB，unit rule，SC_RULE_* 规则表）。

## 二、元命令表（exe 注册表按地址序，23 个）

| 命令 | 描述（exe 内置帮助原文） | 备注 |
|---|---|---|
| `color` | Sets a variable with a RGB or ARGB color value | set 子命令 |
| `vector4` / `vector3` / `vector2` | Sets a vector4/3/2 variable | set 子命令 |
| `end` | 块终止 | 独立串 ×3（多上下文） |
| `mode` | 模式切换 | ×2（begin/end 形态） |
| `set` | 变量赋值（×9 个注册变体） | 见 §三变量 |
| `version` | Set version of the current script | 引擎校验 min/max：`Script version more recent than code` / `no longer supported` |
| `include` | 文件包含 | 支持通配符 `*` `?`；配 `pushDir`/`popDir` |
| `define` | Create a definition, must be terminated with `enddef` | 宏定义 |
| （redefine 变体） | As per define, but will replace any existing definition | 关键字串待定位 |
| `output` | output the given string / Output the arguments of this command | 调试输出 |
| `popDir` / `pushDir` | Push current base directory… / Pop… | 目录栈 |
| `if` / `elseif` / `else` / `endif` | Start a conditional block | 条件编译 |
| `purge` | Purge variable definitions from the given namespace scope | 配 namespace |
| `namespace` | Begins a new namespace scope | `Bad scope '%s' specified` |
| `enddef` | 宏终止 | |
| `enum` | 枚举定义 | `Unknown enum` |
| `flag` | 标志 | |
| （instantiate 变体） | instantiate the named definition a given number of times | `create %s(%d %d)` |
| `error` | Cause an error … to be reported | 主动报错 |

## 三、语法模型

1. **行命令**：`命令名 参数...`；参数个数在解析器注册表中声明，
   违者报 `Wrong number of arguments` /
   `Not enough arguments: expecting at least %d more`；
2. **块命令**：命令后跟随若干行，以 `end` 收尾；不平衡报
   `Unbalanced block command in '%s': too many or two few 'end's.`；
3. **条件**：`if` … `[elseif …]` … `[else]` … `endif`；不平衡报
   `Unbalanced meta command in '%s': look for missing close tag (e.g. endif/enddef)`；
4. **宏**：`define 名称 参数...` … `enddef`；重复定义报 `Already defined`；
   实例化 `instantiate 名称(次数)`；宏内错误定位
   `in definition '%s', line %d`；
5. **变量**：花括号引用 `{name}`（错误：`missing '}'` / `missing variable
   name` / `bad variable name` / `Unknown variable: '%s'`）；赋值走
   `set` 系列（含 color/vector2/3/4 类型化变体）；作用域分
   **Globals / Locals**（调试转储格式 `%s = %s`，注释行 `# Globals` /
   `# Locals`）；namespace 作用域隔离；
6. **include**：`include 文件`（支持 `*` `?` 通配符；无命中报
   `No matches! You may want to include wildcards.`；文件缺失报
   `File not found: '%s'`）；目录栈 `pushDir "目录"` / `popDir`；
7. **注释**：`#` 开头行为注释（`# Plus unknown external globals`、
   `# Globals` 均为注释形态实证）；
8. **enum / flag / mode / version**：数据定义类元命令；
9. **报错格式**：`%s(%d): error: <msg>`（文件(行): 错误）、
   `%s(%d): %s`、`Expected '%c'`（期望特定分隔符）、`Unknown command`。

## 四、校验规则 → Monaco linter 映射（全部有 exe 错误串背书）

| linter 规则 | 引擎错误串 |
|---|---|
| 未知命令 | `Unknown command` |
| 参数个数不足 | `Not enough arguments: expecting at least %d more` |
| 参数个数错误 | `Wrong number of arguments` |
| 期望分隔符 | `Expected '%c'` |
| 块不平衡（end 多/少） | `Unbalanced block command in '%s': …` |
| 元命令不平衡（endif/enddef） | `Unbalanced meta command in '%s': …` |
| 重复定义 | `Already defined` |
| 未定义引用（宏/变量/枚举/参数） | `Unknown definition '%s'` / `Unknown variable '%s'` / `Unknown enum` / `Unknown parameter` / `Unknown definition %s` |
| 变量括号/名字 | `missing '}'` / `missing variable name` / `bad variable name` |
| include 文件缺失 | `File not found: '%s'` |
| 版本过新/过旧 | `Script version more recent than code…` / `Script version no longer supported…` |
| 输入流损坏 | `Bad input stream` |
| 位置格式 | `%s(%d): error: <msg>`（文件(行): 消息——Monaco marker 的标准形态） |

## 五、容器 token 流实况（shader 容器前段）

长度前缀字符串流依次为：HLSL 语句片段 → uniform 声明 → **片段名** →
下一段……（`NullVS` / `worldToClipSpaceVS` / `SimpleScreenQuadVS` /
`copyUV0` / `skinPosition1` / `SkinPosNormalTangent` 函数体……）。
即容器 = 「片段定义」的 ArgScript 序列，名字与源码交替；
完整提取见 `tmp/dynamic/all_blocks_index.txt`（6636 块）与
`docs/re/shader-fragment-census.md`（按域普查）。

## 六、Monaco 编辑器实现建议

1. **Tokenizer**：行首 token = 命令名（关键字表着色）；`#` 行注释；
   `{name}` 变量高亮；字符串/路径参数；
2. **Linter**：按 §四 表实现——已知命令集合（本文档 §二）+
   每命令参数个数声明（对照引擎注册表）+ 块/元命令配对栈 +
   `{}` 变量配对；错误以 `{file, line, message}` 形态输出（与引擎
   `%s(%d): %s` 对齐）；
3. **Snippet**：`define…enddef`、`if…endif`、`include`、`set` 系列、
   `pushDir/popDir`；
4. **EcoGame 规则包**：自有帧格式（null 分隔名字表 + 数据段），解析器
   待单独逆向（首样本 `tmp/boc/scripts_main.bin`，SC_RULE_×1035）——
   完成后 Monaco 增加 EcoGame 规则语言模式。

## 七、dump 方法（复现本文档）

- 关键字/错误/帮助串：对 SimCity.exe 做可打印段扫描，锚点
  `Create a definition, must be terminated with enddef`；
- 容器 token 流：`tmp/parse_container_tokens.py`（长度前缀字符串流）；
- EcoGame blob 样本：`tmp/boc/scripts_main.bin`（BOC 修改版，6.3MB）。
