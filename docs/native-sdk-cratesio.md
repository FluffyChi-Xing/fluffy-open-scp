# crates.io 首次发布准备

2026-11-28：用户已使用 GitHub 账号注册 crates.io。当前仍为发布准备，未上传 crate。
用户已确认邮箱验证和本机 `cargo login` 完成；未读取或记录令牌内容。

直接查询 crates.io crate API，`openscp-sdk`、`dbpf`、`rw4`、`sc-properties`、
`sc-format` 当前均返回 HTTP 404。这只表示查询时未找到，不能保证发布时可用，
也不表示用户已拥有名称。首发前再次核查。

## 包组合与顺序

| 顺序 | 包 | 版本 | 原因 |
|---|---|---|---|
| 1 | sc-format、dbpf | 0.12.0 | 正常依赖中没有尚未发布的本地库 |
| 2 | rw4 | 0.12.0 | 测试依赖 dbpf；正常依赖无本地库 |
| 3 | sc-properties | 0.12.0 | 正常依赖 dbpf、rw4 |
| 4 | openscp-sdk | 0.1.0 | 依赖以上四库 |

正常本地依赖同时声明 path 和精确 registry version。本地继续使用 workspace 源码，
打包后的 manifest 通过版本解析已发布库。精确版本用于固定公共重导出类型的兼容组合；
更新依赖必须重新验证消费者并发布新的 SDK 补丁。

SDK 的 `publish = false` 在验收完成前保留，实际发布前才移除此保护。
底层库不改名、不改版本；桌面版本不变。`sc-properties` 的 registry dev 依赖
不属于 SDK 正常依赖；最终仍需审查归一化 Cargo.toml 和打包后的验证结果。

## 发布前检查

1. 在 crates.io Account Settings 核实邮箱已验证。
2. 由用户在本机执行 `cargo login` 完成发布认证，令牌不发送到聊天或写入仓库。
3. 核对名称、许可、来源署名、crate 文件清单与体积，避免包含本地游戏资源。
4. 运行 SDK release 合同的所有检查，确认 Linux 与真实 i686 尚待完成的项目。
5. 按顺序执行每个包的 `cargo publish --dry-run`。尚未发布的依赖会使后续包的
   registry 解析失败，这是发布顺序的阻塞，不能报告为 dry-run 通过。
6. 审核通过并获用户发布指令后，按顺序上传，等待依赖可解析，再进行下一包 dry-run/上传。
7. 从 registry 新建独立消费者，仅依赖 `openscp-sdk = "0.1.0"`，运行 x64/i686
   和依赖身份检查，记录源 SHA、crate 校验和、工具链与发布标签。

当前已准备 version+path 依赖；包实际验证、上传和 registry 消费者验收均未完成。
预检记录：sc-format、dbpf 的 `cargo package --offline` 已成功打包并编译验证；
SDK 文件清单已检查。rw4 与 sc-properties 的离线 package 因 registry 中尚无 dbpf
而失败，不能标记已通过。补充 metadata 后仍需在正式发布提交重跑全部预检。
原 Git 首发路线仍可用；本文件明确账号就绪后的 registry 发布步骤。

后续预检：sc-format 与 dbpf 在线 `cargo publish --dry-run --allow-dirty` 通过，
Cargo 明确因 dry-run 取消上传。五库补入根 MIT LICENSE 副本供独立分发。
真实 Simoder i686 测试使用隔离副本的显式 asInvoker manifest 修正 Windows
安装程序检测，21 项全通过；没有更改上游源码或请求管理员权限。Linux CI 仍待执行。

最新验收：`1a939a1` 的 Windows/Linux CI 全通过（含 Windows i686 独立消费者）。
此前 m3_packages 在无游戏样本时末尾断言失败，已修为始终执行合成包 sweep，
有本地 app/DLC0/EP1 时追加原严格检查。源码解析器未修改；本地合成及三包全通过。
[CI 运行记录](https://github.com/FluffyChi-Xing/fluffy-open-scp/actions/runs/46117976186)。

参考：[Cargo 发布指南](https://doc.rust-lang.org/cargo/reference/publishing.html)、
[path 与 version 依赖](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)。
