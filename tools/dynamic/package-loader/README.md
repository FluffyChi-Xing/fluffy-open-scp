# Package loader 启动观察

仅适用于脚本固定 SHA-256 的 SimCity retail EXE。先退出已有游戏；不要叠加其他注入器。脚本挂起启动进程、验证 8 个入口字节并安装观察，再恢复运行。不会自动安装 package 或关闭已恢复的游戏。

```powershell
& 'C:\Users\FluffyCHi_Xing\AppData\Local\Python\pythoncore-3.14-64\python.exe' tools/dynamic/package-loader/runtime_capture.py --out tmp/my-new-loader-run --seconds 35 --target 00B1B104:00000000:4483E856
```

`--exe` 可指定同指纹 EXE 的另一路径，默认是本机实际安装目录。`--out` 必须不存在。省略 target 会进行有界去重采样，不是全量资源日志。产物为 config.json、trace.jsonl、status.json；必须检查 ready/resumed、errors、summary.dropped。进程提前结束可能没有最终 summary，不能宣称零丢失。

脚本不调用未验证 NativeFunction，不改返回值；Interceptor 会改 hook 入口。packed Open 成功、admission 成功、register 成功和 FindRecord/实际 Record 创建应分别判断。OpenRecord(materialize=false) 只是探测。

生成单 TGI 对照包复用现有 writer，不修改资源内容，输出采用独占创建：

```powershell
cargo run -p dbpf --example single_tgi_overlay -- <source.package> <new-output.package> 00B1B104 00000000 4483E856
```

参见 [实机报告](../../../docs/re/package-runtime-verification-20261017.md)。测试包仅用于复制不变 payload 的对照，任意修改仍需资源语义检查。脚本停止观察后保留游戏进程，后续实验前正常退出自己启动的实例，再核对哈希清理自己的 fixture；不要删除现有 mod。
