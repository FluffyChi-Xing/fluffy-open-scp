# 编译后端接口

项目的 `backend.command` 是一个外部编译后端命令数组。打包器会替换三个占位符：

- `{project}`：项目绝对路径
- `{output}`：项目的 `output/` 目录
- `{config}`：项目 `project.json` 的绝对路径

后端必须满足：

1. 只读 `input/` 和项目配置。
2. 只写项目的 `build/`、`reports/`、`output/`。
3. 不安装 Package，不启动游戏，不修改游戏目录。
4. 返回码为 0 才视为成功。
5. 在 `output/` 写入 Package，在 `reports/` 写入后端专用验证报告。
6. 后端报告应记录输入文件的相对路径、大小和 SHA-256，保证同一项目可以复核。
7. 后端只能接收本次运行新生成的 Package，不能复用编译目录中的旧输出。

示例：

```json
{
  "backend": {
    "command": [
      "python",
      "backend_native.py",
      "--project",
      "{project}",
      "--output",
      "{output}"
    ]
  }
}
```

当前 MVP 已经负责命令展开、日志、返回码和结果报告。下一步将把现有 RW4/Property 编译脚本包装成符合此接口的 `backend_native.py`。

当前适配器调用研究流水线的示例：

```json
{
  "backend": {
    "command": [
      "python",
      "backend_native.py",
      "--project", "{project}",
      "--output", "{output}",
      "--pipeline", "C:/path/to/build_tower_hybrid_single.py"
    ]
  }
}
```
