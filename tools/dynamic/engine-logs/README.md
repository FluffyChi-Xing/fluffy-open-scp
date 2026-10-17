# 引擎文本日志启动探针

实测构建：SHA-256 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`，Frida 17.19.0 / x86 游戏 / QJS。未知 EXE 拒绝套用地址。

```powershell
& 'C:\Users\FluffyCHi_Xing\AppData\Local\Python\pythoncore-3.14-64\python.exe' tools/dynamic/engine-logs/capture.py --out tmp/my-new-engine-log-run --seconds 35
```

默认实际安装路径可用 `--exe` 指定同指纹副本。先退出其他游戏实例；输出目录必须不存在。脚本拒绝已有 SimCity 进程，然后挂起启动、逐入口验证 12 字节、安装 6 个 hook、flush、resume。到时自动卸载并 detach，但保留游戏进程，请正常退出后再开始下一次实验。异常提前结束时 summary 可能缺失，不能当零丢弃。

输出 `config.json`、`trace.jsonl`、`status.json`。查看 `summary.errors/dropped`、正文的 `truncated` 和未知 layout。每批 100ms，队列 1024，总记录 10000；这是有界观察，不是全量无损采集。

| channel | 含义 |
|---|---|
| engine.trace | EA::Trace::Server::Trace 收到的格式化后文本；在 reporter 过滤之前 |
| engine.report | 已通过相应 reporter 过滤、交给文件或控制台 reporter 的 LogRecord |
| engine.format | TraceFormatted 的格式模板，未读取变参；不能将其作为完整文本 |
| engine.console | cCheatManager::Output 收到的文本，可能来自 ArgScript 或其他调用方 |
| engine.server | GetServer 返回的实例/虚表身份，不是游戏业务日志 |

不调用 NativeFunction，不改日志级别，不执行控制台命令，不写游戏文件。Interceptor 会修改 hook 入口，因此不是零侵入。helper 的 enabled/filtered 是当时缓存字段，不应反向推断日志一定被丢弃。

相同文本可能同时出现在 trace 和 report；不要当作两次业务事件。recordNumber 只属于相应引擎实例，不是全局唯一键；对象地址可能复用。源文件/行在零售版可能为空，不能编造源码位置。

实机证据与产品方案：[日志开发工作台](../../../docs/re/mod-diagnostics-logging-20261017.md)。本工具尚未接入 OpenSCP UI，也没有提供任意 native 调用或游戏脚本执行功能。
