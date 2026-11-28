# UnitRule 领域契约第二阶段：应用次数、控制位、调度、运输与事件

2026-11-28。接续`argscript-frontend-foundation-20261128.md`。本轮保持只读研究，不改生产构建、工作台、Actions、游戏包或存档。

## 这次实际补了什么

新增`probes/runtime/unit_rule_domain.py`，将应用次数、控制位、调度、运输options、规则后继动作、testAgent及successEvent整理成类型明确的domain节点。23份H&V脚本展开后，原有791个资源操作之外，新增828个domain节点：

| 类型 | 数量 |
| --- | ---: |
| ruleEdge | 312 |
| applicationCount | 260 |
| transportOptions | 101 |
| successEvent | 88 |
| controlFlags | 36 |
| timeTrigger | 13 |
| rate | 8 |
| priority | 7 |
| agentTest | 3 |

数字包含331条UnitRule和3条GlobalRule。GlobalRule已解析，但二进制参数比较仍仅覆盖UnitRule。

`unit_rule_domain_compare.py`增加直接字段核对；`unit_rule_frontend_audit.py`会从类型化字段重建可比较形式。已知命令若丢失domain节点，审计直接拒绝，不能退回原文而假装新AST仍正确。

**证据边界：** 当前提供的反编译中已定位运行期执行器、调度器、构造器和二进制读取器；尚未获得完整Eco领域命令注册/Execute解析实现。这里的参数约束是“原始语料签名＋字段比较＋原生消费者”支撑的研究子集，不是声称已恢复原生完整文法。未知选项与超出子集的表达式会明确报不支持；合法原生语法也可能尚不在本子集中。

## 1. 应用次数：显式值与隐式值必须分开

已支持的原文形式：

```text
applyCount 1
applyCount 1 2
applyCount 0 90
maxApplyCount
```

显式数量记录为`explicitBounds`，保留minimum/maximum；maxApplyCount无参数记录为`resourceBounded`、rawBounds=[0,0]。命令未出现时不擅自增加节点。

原生CanRunRule只在mMaxApply>0时以它覆盖runState最大应用次数，还允许mApplyResource从map/Property/bin等来源计算上限。该动态来源的**原文参数语法尚未恢复**，因此不新增猜测语法。

特别不能把0一概当成“执行零次”。本轮进一步发现：

- C++ cEcoUnitRule构造器的mMinApply/mMaxApply均为0。
- 74条没有显式应用次数命令的源规则，晚版AEB中60条mMaxApply=1、14条=0。
- 例如HeroBase_CanCreate与HeroVillain_HeroLeave属于不同结果。

这说明编译后的隐式应用次数不能单靠运行期构造器恢复。当前审计将这74项明确列为`implicit-application-bound-not-recovered`，不根据猜测填0或1。即使某种规则形态与结果相关，也不能未经命令解析证据就把相关性写成通用默认策略。

## 2. flags和globalFlags：测试与写入不同，符号也依赖作用域

例：

```text
flags -if IsOnFire -clear IsOnFire Burning
globalFlags -if Chance50Percent
```

AST分别保存scope、test/set/clear操作及符号列表。不是把整行变成一个bool，也不是把flags当资源bin。

这份语料与AEB核对得到的位表如下，不推广为全游戏完整枚举：

| local标志 | bit | global标志 | bit |
| --- | ---: | --- | ---: |
| Chance25Percent | 4 | SandboxActive | 2 |
| Chance50Percent | 5 | CrimeEnabled | 10 |
| Chance75Percent | 6 | Chance50Percent | 30 |
| Burning | 12 | | |
| OpenForBusiness | 19 | | |
| IsOnFire | 20 | | |
| CrimeInProgress | 28 | | |
| ActiveBuilding | 29 | | |

同名Chance50Percent在local/global上并非同一bit。类型化符号的名字/哈希只是链接身份；真正的控制位编码要查作用域对应表，不能直接把FNV值当mask。

原生CanRunRule检查`(currentFlags & testMask) == testValues`；RunRule写入遵循`(old & ~writeMask) | setValues`。64字节pool块分别包含local/global两组测试及改写信息，当前全部331条UnitRule均做8个64位字段核对。

这也解释了Chance标志在此处只是“读取当前条件位”，不能仅凭名字把该命令改写成“执行时独立随机抽签”。这些位如何刷新属于额外运行逻辑。

重复、相互冲突的set/clear声明与完整原生合并策略仍未恢复。AST保留顺序，不能拿本案例中无冲突的位集合核对冒充全语言合并规则。

## 3. 调度：不是按脚本行睡眠

以下形式已类型化：

```text
priority 200
rate 61
timeTrigger Hour 0.02 -fromCreate -count 1
timeTrigger Hour 0.5 -variance 0.5
timeTrigger Day ${Hour8}
```

timeTrigger保存period引用、time数量及fromCreate/count/variance修饰。rate与priority独立保存，外部变量不求猜测值。

原生cEcoSimInfo构造器给出priority=0、rate=16、triggerTime=-1、variance=0、triggerCount=0等初始值。本轮核对的pool布局为68字节：priority+0、rate+4、triggerTime+48、periodID+52、count+56、variance+60；数据池内部是小端，不能与AEB大端记录头混读。

原生TimeTrigger先检查计数上限及当前周期时间，达到条件后增加triggerCount并推进nextTriggerTime。count为正才限制次数；0不能机械解释为执行0次。fromCreate使用规则flag bit0，不是多出一个独立规则事件。

SetNextTriggerTime处理variance，并依据creationSimTick选择随机偏移或由ruleID/slot/创建时刻混合得到偏移；随后落到周期坐标中。不能把variance写成每次调用都独立随机一次，也不能把Hour/Day直接当作墙钟秒。

更不能用一个时间条件保证角色在固定墙钟时刻出现在某建筑：还有资源、状态、调度、运输可达及角色在途情况。此前工业推广实机证明连续访问和回收，并没有测量出发时刻的概率分布。

当前对8个未解析外部时间/频率表达式的字段核对显式跳过，继续保留表达式。

## 4. options：引用对象必须带类型，重复候选必须保留

当前支持这份H&V语料使用的选项签名：

| 选项 | 参数 | AST含义 |
| --- | --- | --- |
| sendTo/resendTo/or | 一个sink符号 | 目的地引用 |
| switchTo | sink及非负时间值/外部变量 | 后续目的地阶段 |
| createAgent/via | 一个transport符号 | 运输资源引用，不是Unit创建 |
| using | 一个handler符号 | 原生运输消费者引用 |
| dieAfter | 时间值/外部变量 | 运输生命周期控制项 |
| perApplication/resend/ignoreStability | 无参数 | 独立修饰，仍保留作用顺序 |
| region | 一个原始区域值 | 保留，尚不解释完整区域枚举 |

重复or与多段switchTo保留为有序列表；不字典覆盖、不去重。应用范围需要以整条规则的多个options节点为单位，不能要求createAgent和perApplication必须写在同一行。

直接核对了运输ID、显式handlerID及以下规则位：sendTo对应0x00800000；resend/resendTo对应0x01000000；perApplication对应0x00010000。测试同时核对“存在”和“应不存在”，去掉perApplication也能检测到差异。

**不要把0x00100000叫ignoreStability。** 实验室SendRobot没有写该option也带这个bit；原生OnRuleSuccess正是用它进入mUnitID创建路径。RobotKills同时有ignoreStability和创建PanicMarker，单靠共现会误命名。该bit现在核对为createUnit动作；ignoreStability本身的独立编码明确列为`encoding-not-yet-isolated`。

testAgent的handler和canReach sink已有独立类型；本轮核对AgentTests数组数量，尚未宣称恢复了每个测试记录的全部字段。

## 5. 创建Unit与创建Agent不是同一件事

```text
onSuccess VillainBase_CheckRelocks -createUnit DrVu_RoboTank (0, 3, 0)
```

本子集支持已观察到的onSuccess创建动作：规则后继是UnitRule引用，创建目标是Unit引用，偏移是三分量向量。当前未把onFail/chain带createUnit的其它组合当作已恢复的合法语法。

`mUnitID`及三分量偏移已经逐字段比对。原生OnRuleSuccess还会将该偏移与PlayCell大小、目标occupation radius及宿主变换结合后创建实体。因此`(0,3,0)`不能直接教成“向某轴移动3米”。

这些命令编译成不同字段和数据段，由CanRunRule、RunRule、OnRuleSuccess等阶段消费。AST保存源文顺序，不意味着原生执行器逐行像Python一样解释它们。

## 6. 事件的类型、旗标与载荷不能混为一个ID

已覆盖的事件类别：uiEvent、telemetry、vignette、alert、history。AST分别保留类别、事件符号、参数/资源来源与修饰。

当前核对cTriggerInfo的12字节头：uint16类型、uint16类别专属flags、uint32 ID、int32额外载荷offset。数据池仍为小端。

| 类别 | 本语料类型值 | 本轮核对范围 |
| --- | --- | --- |
| uiEvent | 0x84 | 类别、符号ID、flags；带参数的完整载荷未降低 |
| vignette | 0x86 | 类别、符号ID、start/stop flags |
| alert | 0x02 | 类别、符号ID、clear flag |
| history | 0x8C | 类别、history符号ID、flags；5个资源载荷未降低 |
| telemetry | 0x06 | 名字字符串、类别、resource作用域旗标及资源ID |

telemetry尤其不同：事件名是extra数据指向的字符串，mID在带resource时用于资源ID，不能统一把所有事件目标都做FNV然后写同一个字段。原生TelemetryManager::RuleEvent分别读取global/local/connected来源；本子集参数语法仍只接受本语料已使用的global/local形式。

任何事件头匹配都不代表已证明UI、动画、历史或遥测下游效果生效；消费者及其注册仍是依赖。

## 7. 验证结果及保留的未知

运行：

```powershell
python probes/runtime/unit_rule_frontend_audit.py --source-dir tmp/aec-extract --binary tmp/rosetta/hv_277156632.aeb --out tmp/hero-villain-language-20261128/frontend-domain
python -m unittest discover -s probes/runtime -p 'test_hero_villain_*.py'
```

- 23份原始脚本、334条展开规则仍无诊断。
- 原3732项比较仍3730一致，两个已知版本差异仍是SendSkill_E2/E3运输字段。
- 新增领域比较7665项全部一致，包括声明缺失时可确认的缺省字段，不能仅靠删除命令来删掉检查。
- 88项明确跳过：74个隐式应用次数上限、8个外部变量字段、5个history额外载荷、1个ignoreStability编码。
- 49项相关Python检查通过；包含14项新增领域测试，故意修改控制作用域、触发锚点、创建偏移、事件目标、perApplication及删除整个timeTrigger，确认能检出差异。领域节点丢失则审计硬失败。

原生证据片段与哈希在`tmp/hero-villain-language-20261128/frontend-domain/native/`。结构化结果为同目录`expanded-ast.json`、`audit.json`。

当前仍保持`domainValidation=partial`、`packageCompilationAllowed=false`。隐式默认策略、完整命令注册/参数表达式、重复声明策略、所有payload、变量/宏作用域和最终资源链接仍需补齐。本轮没有游戏内新验收，不以离线字段匹配替代运行行为验证。
