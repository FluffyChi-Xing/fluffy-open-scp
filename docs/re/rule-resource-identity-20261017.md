# 规则日志到资源身份：实机与包内对照

日期：2026-10-17（Asia/Hong_Kong）。目标仍为用户运行中的离线 SimCity，PID 18848，模块 SHA-256 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`。两次各 10 秒被动采集均已卸载并 detach，未调用游戏函数、未关闭游戏。完整字段、指令及摘要见 [证据 JSON](evidence/rule-resource-identity-20261017.json)。

## 已验证的关联

| 范围 | 状态索引 | 集内规则索引 | 规则 ID | 包内记录字节偏移 |
|---|---:|---:|---|---:|
| global | 315 | 380 | 0x739D42FF | 4318957 |
| unit，slot 392 | 12 | 10447 | 0xF01B0EB5 | 1546973 |

两个目标均为 ruleSetIndex=0。最终采集各保留 2 条，共 4 条，读取错误及丢弃均为 0。运行中的 game 指针已从上一阶段的 `0x2b8be678` 变化为 `0x2b8bffa0`；全局同一规则的 stateIndex 从 318 变成 315。因此 stateIndex、slot 和指针不能当成跨城市或跨会话的规则身份。

1. 从日志入口取 game、stateIndex、unitSlot，使用上一阶段验证的数组/deque 路径取得 rulePointer。
2. rulePointer+0 是规则 mID。零售单位/全局序列化函数 `0x5FC1C0` / `0x5FC520` 都先写该字段；dev `cEcoRuleSet/BuildIDMaps.c` 显示 mID 到集内索引的建表关系。
3. 零售 `IndexFromUnitRuleID=0x5F79D0`、`IndexFromGlobalRuleID=0x5F7BE0` 使用 set+0x74 / set+0x9C 的 8 字节 ID/index 表。采集器有界二分查找，再检查返回索引处的规则 ID 相同。此为同一规则集内核对，不冒充原函数跨 add-on 的优先级查询。
4. `EcoRulesFromResource=0x5C9290` 将 Resource 指针减去 0x144；Resource 的 key 从 +8 开始，依次 instance/type/group。实机两个 set 均读到抽象键 **08068AE9:40800200:622B9CD7**。零售 Init 的 core/add-on 请求分别使用 type AE9、group 40800200/40800201，与 dev 请求链一致。

以上 8 个相关函数/指令区间在真实安装、备份 EXE、dump EXE 中逐字节一致。证据保存的是原始指令；语义名称来自 dev 带符号反编译树与零售指令对照，不是零售符号。

## 磁盘候选对照，不等同来源胜出证明

检查明确指定的 `SimCityUserData/EcoGame/SimCity-Scripts_287520926.package`，导出其中 **08068AEB:40800200:622B9CD7**。资源 SHA-256：`a388e3c15080a4721e7d0047da180f6ee9f38292060c67892fa34b952564c381`。

`resolve_ids.py` 只接受已验证的 5/2/14、namesIncluded=0 布局，检查段版本、边界、尾部完整消耗；该样本有 28441 条单位规则、573 条全局规则。两个 ID 都出现在与运行态相同的集内索引。保留所有重复 ID 候选，不选择第一个冒充唯一来源。

AE9 是请求的抽象资源类型，AEB 是编译文件类型；两者不能混写为一个 TGI。尚未捕获本次脚本构建的 FindDatabase/资源工厂加载来源，也没有证明候选包全体内容与内存一致。因此工作台只展示 `abstractResourceTgi`，不填充获胜 package 路径，不声称当前磁盘候选一定被加载。

## 名称与特化的边界

该编译资源的 namesIncluded 为 0，不能从中恢复规则文本名称。现有 `docs/packages/database_main.s3db` 的 instance/property 查询也没有命中这两个 ID；搜索范围仅限这份描述符库。

dev `GB/cEcoRuleBase/Read.c:25–41` 显示：可选名称被读取到临时字符串后释放，不存入规则对象。零售的可选名称读取函数尚未独立定位；不能把 dev 行为无条件提升为零售全部版本的事实，也不能把池内可打印字符串当成规则名称。

dev `cEcoRuleSet/IndexForSpecialisedUnitRule.c` 还显示按 rule ID 与属性实例组合生成特化规则。因此运行态集内索引可能超出磁盘基础规则数量。新字段 `baseRuleWithinSet` 表示该 set 的 ID 表返回索引；不得假定它总与当前 `ruleWithinSet` 相同，也不能仅用 rule ID 归并不同属性特化实例。本次捕获的是两个基础规则，特化实例尚未实机闭合。

## OpenSCP 的直接用途与后续边界

规则日志现在可按稳定 rule ID 或抽象资源 TGI 筛选、高亮，并追到明确导出资源中的规则记录；未知名称保持未知。这比随会话变化的 stateIndex 更适合对比 Mod 修改前后是否执行到同一规则，但版本、规则集、特化上下文仍必须保留。

后续优先工作是：捕获资源工厂/FindDatabase 的加载来源，建立带包摘要的 provenance；将开发者自己的 ER2 源码符号表按构建版本关联 rule ID，保留哈希冲突；再加入特化属性身份与输入输出 bin。当前不主动重建游戏脚本对象、不强制热重载，不以写入磁盘作为已生效证明。

复现：先用 `ui_dump_tgi` 导出指定 AEB，再执行：

```powershell
python tools/dynamic/rule-logs/resolve_ids.py tmp/rule-main-287520926.bin 0x739d42ff 0xf01b0eb5
python tools/dynamic/rule-logs/capture.py --pid <当前PID> --scope both --seconds 10 --out tmp/new-rule-identity
```

捕获不保证出现同一规则；省略索引会重新固定每类第一个匹配目标。
# 后续来源验证

本报告中的候选包结论属于身份核对阶段。随后冷启动并进入城市，已将实际规则集对象关联到 `SimCity-Scripts_287520926.package` 的完整加载链；证据及采集限制见 [来源验证报告](rule-resource-provenance-20261017.md)。
