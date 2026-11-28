# SQLite 启动边界冻结（2026-11-22 用户要求）

范围：`D:/openscp/openscp.db` 的位置、`src-tauri/src/lib.rs` 的启动/旧库迁移路径、`internal_dirs` 的持久目录选择、`Store::open` 的连接策略，以及 desktop-store-preflight/dev-desktop 启动检查。正常业务 schema 迁移仍需对应测试，不允许为了渲染任务顺手调整启动策略。

只读错误先核对进程与构建目录 Mandatory Integrity Label，再查文件/目录写权限与 SQLite 扩展错误码。不得直接断言数据库损坏，不得删除 DB/WAL/SHM，不得切换空库掩盖错误，不得降低数据库安全标签。

本次证据：target 与 target/debug 均为继承 Low，重编译主 EXE 同为 Low；数据库为普通完整性。之前仅修 EXE 不能防止重建复发。现启动检查同时覆盖构建目录与 EXE；权限检测命令失败也必须阻止启动。

环境修复：在管理员 Windows PowerShell 执行 `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/repair-desktop-integrity.ps1`，仅把本项目 target 树恢复 Medium，然后从普通终端构建运行。首次直接执行 icacls 被操作系统 Access denied 拒绝；2026-11-22 11:41 经 Windows UAC 提权执行脚本后，读回 target、target/debug、主 EXE 均为 Medium，目录带 OI/CI 继承标志。直接运行主 EXE 后进程持续运行，未再输出 SQLite setup panic；尚未对界面内容做视觉验收。

回归命令：`node --test scripts/desktop-store-preflight.test.mjs`。不应在 Low 进程内构建需要写普通用户数据的桌面程序。若后续再次改变了构建标签，应在启动预检被明确拦截，而不是进入 SQLite 后报泛化只读错误。

2026-11-22 用户明确要求启动不复用旧构建，允许修改启动编排：dev:desktop 每次清理所有本地 workspace 包（第三方 Rust 依赖保留）、关闭增量编译，通过 Tauri beforeDevCommand 启动 Vite --force，不再复用旧 5173 服务。禁止直接运行开发 EXE 作为完整启动验收：必须同时确认前端服务可达。清理 target 时只清内容，保留已修好的 Medium 根目录，否则会再次继承工作区的 Low 标签。
