# 2026-10-23 SimCity 崩溃：并发采集与 Frida 卸载路径

结论：本次可定位到 **Frida agent 的 DLL 卸载路径发生访问冲突，随后游戏生成崩溃报告时再次异常**。并发启动、失败后卸载多个注入会话是最有力的触发因素。尚不能把具体底层原因写成“两个业务 hook 同时成功安装后互相覆盖”：失败采集器在入口校验阶段就已拒绝安装。

本次只离线分析旧进程 PID 36688 的转储，没有注入重新启动的 PID 58376。

## 1. 时间线（香港时间）

| 时间 | 落盘事实 |
| --- | --- |
| 00:21:44 | 小学资源探针正常 arm，22 条记录，正常 detach |
| 01:02:43 | 水泵资源探针正常 arm，40 秒 704 条记录，正常 detach |
| 01:34:54 | 同时启动维护费、劳工需求、岗位、W1 劳工四个采集器；只有 W1 显示 arm 成功，其他三个 `entry_mismatch` |
| 01:38:05 | 同时启动维护费、劳工需求、岗位三个采集器；维护费 arm 成功，另外两个 `entry_mismatch` 后卸载并 detach |
| 01:38:08 | WER 1000：SimCity.exe PID 36688，`0xC0000005`，地址 `0x41D80404`，模块 unknown |
| 01:38:24 | WER 转储写入完毕 |
| 01:38:48 | 后续采集尝试只返回进程不存在，没有成功注入 |

来源为各次 `tmp/*20261023*/status.json` 与 Windows Application 事件日志。开始时间为采集器初始化时间，并非精确 arm 时间。失败会话不能计为成功执行规则的探针。

## 2. 两次异常，而不是一个规则计算错误

输入文件：`C:\Users\FluffyCHi_Xing\AppData\Local\CrashDumps\SimCity.exe.36688.dmp`，74,460,900 字节。离线解析器在 `probes/runtime/inspect_crash_dump.py`；精简证据在 `docs/re/evidence/simcity-frida-crash-20261023.json`，完整本地解析在 `tmp/crash-analysis-20261023/dump.json`。

### 较早的异常上下文

- 同一故障线程 16476 的栈内保留异常记录 `0x740DFA98`，异常地址 `0x41D80604`，参数 `[0, 0x41D80604]`：**读访问冲突**。
- 相匹配的 x86 CONTEXT 位于 `0x740DFAE8`，EIP 同为 `0x41D80604`，ESP 为 `0x740DFF50`。
- 该 ESP 首项为 `0x76B77B46`，下一项为 `0xBE000000`。
- 匹配转储时间戳和映像大小的本地 x86 KernelBase.dll 静态代码证明，`0x76B77B40` 是 `FreeLibrary` 中调用 `ntdll!LdrUnloadDll` 的指令，返回地址恰为 `0x76B77B46`。
- `0xBE000000` 在转储已加载模块表中对应 `frida-agent.dll`。因此这是卸载 Frida agent 的调用路径，而非小学资源更新函数。

栈扫描本身不等于完整符号化回溯；这里依靠匹配的异常记录、上下文、栈参数和静态调用指令交叉验证。

### WER 记录的第二次异常

- 转储 ExceptionStream 的 EIP 为 `0x41D80404`，参数 `[0, 0x41D80404]`，也是**读访问冲突**。
- ESP `0x740DF7B4` 首项为 `0x76B7FE83`。静态代码对应 `LoadLibraryExW` 内 `LdrLoadDll` 调用后的返回地址。
- 栈中保留游戏返回地址 `0x0091B502`。匹配版本 SimCity.exe 的 `0x0091B4F5` 压入 `DbgHelp.dll` 字符串，`0x0091B4FC` 调用 `LoadLibraryA`，随后查找 `MiniDumpWriteDump`。
- 这说明最终 WER 地址位于游戏异常处理/生成转储过程，不能拿它直接指认 `ApplyBinRule` 崩溃。

转储中有两份已加载 Frida agent，基址分别为 `0xBBCC0000` 与 `0xBE000000`；已卸载模块表还有重复的 agent 记录。这不等于两套业务 hook 当时都已成功 arm，也不能证明是哪次采集遗留的 agent。

## 3. 为什么之前能读，这次却崩了

之前小学和水泵的成功记录显示采集后正常 detach。这次改为并发启动多个 Frida 宿主，而规则采集器没有接入项目原有的单会话锁。

`effects` 与 `unit-effects` 共享 `applyOps` 和 `binFind`。第二个采集器能检测到入口已被修改，抛出 `entry_mismatch`，但此时已经执行过 `device.attach` 和 agent 加载；入口校验只能阻止本脚本安装业务 hook，不能撤销 Frida 注入/卸载带来的原生运行时活动。多个宿主并发清理仍然有风险。

因此，“只观察参数，不修改资源数值”不等于“没有改动进程”：Interceptor 会改函数入口，Frida 自身也有 DLL 加载、卸载和拦截生命周期。这次采集流程改变了这些生命周期的重叠方式，不能用之前若干次成功推导其安全。

**置信边界**：已强证据定位 Frida 卸载及报告生成两条异常路径；并发会话的卸载冲突是高可信触发解释。故障地址的代码页、游戏四个 hook 入口的运行时字节未保留在该 minidump，不能证明具体 trampoline 归属、释放者或某个 Frida 内部缺陷。也没有证据把故障归因于学校规模、维护费规则、资源日志量或存档损坏。

## 4. 修复和验证

- 新增共享 Windows 命名互斥量 `Local\OpenSCP.SimCity.FridaCapture.v1`，从 attach 前持有到清理完成；同时兼容原 `tmp/dynamic/capture.lock` 的存活 PID 检查。锁检查使用完整 HANDLE 类型，避免 64 位 Python 截断句柄；对无法读取的所有者信息保守拒绝。
- 接入规则采集器、日志量级探针、UnitEntry 探针，以及既有 `capture_states.py` / `dump_shaders.py`。其他尚未迁移的注入工具和外部 Frida 进程不受命名锁完整控制，仍须避免同时运行。
- 两个小探针在任何退出路径都尝试 unload/detach，且改为先核对 EXE 哈希/架构、再安装业务 hook。
- 规则采集器记录 detach 原因、意外断开、script unload 和清理后进程存活情况。原先崩溃采集曾留下 `errors=[]`、`detached=false`，现在意外断开或目标退出会报失败，不能再当作“零记录正常完成”。
- 4 个实际 Windows 子进程互斥测试通过：竞争拒绝、正常释放、宿主退出回收、兼容旧 PID/损坏文件拒绝。另有 24 个既有 JS agent 测试通过，Python 编译检查通过。

以上验证没有启动或附加游戏。互斥修复消除已发现的并发流程缺口，不构成注入稳定性的实机保证。后续规则研究先继续离线静态分析；任何新的运行时采集都必须单宿主串行、短窗口，并检查完整清理结果。
