# OpenSCP Cargo SDK 0.1.0 发布合同

发布状态（2026-11-28）：用户授权正式上传后，SDK `0.1.0` 与四个底层库 `0.12.0`
已发布到 crates.io，支持 `openscp-sdk = "0.1.0"`。
registry 下载后的 Windows x64/i686 消费者各三项通过；校验和及源码记录见
[发布回执](releases/native-sdk-0.1.0.json)。下文 Git 首发及 publish 保护为发布前方案记录。

目标：让 Simoder 等 Rust 宿主通过一个 Cargo 入口读取、检查和生成资源字节，
保持原格式类型身份与字节语义，脱离桌面应用构建。
SDK 独立为 `0.1.0`，桌面应用及原四库仍为 `0.12.0`。

## 首发交付

首发目标为 Git Cargo SDK：包名 `openscp-sdk`，候选标签名 `sdk-v0.1.0`。
消费者固定发布提交完整 SHA 并提交锁文件。验收完成后才创建标签；版本号不表示已发布。

```toml
openscp-sdk = { git = "https://github.com/FluffyChi-Xing/fluffy-open-scp", rev = "<release-commit>" }
```

## 0.1.0 范围

| 必交付 | 验收目标 |
|---|---|
| 门面与 package/properties/rw4 feature | 八种组合通过，format 始终可用 |
| DBPF/DBBF、RefPack、文件/内存读取 | 索引、存储/解码字节、压缩/失败路径回归 |
| 有预算的内存入口 | parse_with_limits 在索引分配前检查输入/条目数；read 限制单资源输出 |
| 原 u32 TGI 类型身份 | checked_id 对超过 u32 的 manifest instance 明确拒绝 |
| PROP、locale、继承与既有 debug_tools | 字段修改语义回读、未知资源字节保留 |
| RW4 与 signature probe | 格式库回归；probe 不被解释为结构验证 |
| 组合错误与机器分类 | 保留原 source；不保证底层 Display 文案稳定 |
| 消费者、文档和 CI | 独立消费者及真实 Simoder formats/mods 测试 |

公开重导出的原库 API 也是消费者可用 API，不能当作私有 helper 随意删除。
0.1.x 补丁保留既有签名、类型身份和 error code；不兼容变化进入 0.2 并给出迁移说明。
更换底层库必须重新验证消费者矩阵和依赖来源。

不纳入首发：Tauri/SQLite、建筑构建和发布、UI 编辑器、Base Luau/USTAR 打包、
MOD manifest/权限/生命周期、字段运算业务规则、冲突赢家、部署和运行期资源热加载。
预算只覆盖显式 DBPF 输入/索引/单 payload，不承诺全进程、并发或全部 RW4/图片预算。

## 平台及发布闸门

目标：Windows x64、Windows i686、Linux x64。MSRV 在固定旧工具链验证前不声明；
CI 使用 stable，发行记录必须包含实际 rustc/cargo 版本。

1. 干净发行提交，SDK 版本独立，完整 SHA 可追溯。
2. `python scripts/check_native_sdk.py` 全通过；原四库 ignored 测试单列。
3. Windows i686 独立消费者和 Linux CI 通过，仅编译成功不算测试通过。
4. 正常依赖无 Tauri/SQLite/registry/store；不能同时出现不同来源的 DBPF 类型。
5. 真实 Simoder x64/i686 formats/mods 测试通过。i686 的 `error 740` 已通过隔离
   测试程序显式 asInvoker manifest 修正，整套 21 项已通过；不需要提升权限。
6. 超限、损坏、截断、instance 溢出、重复 TGI、未知 payload、canonical 语义有测试。
7. Rustfmt、SDK Rustdoc、样例及迁移文档通过。格式验收不冒充游戏运行验收。

## crates.io 后续门槛

用户已注册账号；准备进度及顺序见 [crates.io 首次发布准备](native-sdk-cratesio.md)。

`publish = false` 保持明确，本次不上传 registry。
支持 `openscp-sdk = "0.1"` 之前，需要确认底层 crate 名称所有权、协调发布版本、
许可/包内容审计，为所有 path 依赖提供可解析 registry 版本，并验证打包产物消费者。
当前依赖已补 path+version，底层版本仍需先发布；不能绑定同名无关 registry 包凑通。
这项工作不应通过修改桌面工作区版本实现。

## 当前状态

已实现框架、错误、预算与身份边界、消费者测试与 CI；真实 i686 全套已通过。
代码提交 `1a939a101b5a54fdf0609cdb595a5932b38de7c0` 的 Windows/Linux CI 均已通过，
含 Windows i686 独立消费者，见 [运行记录](https://github.com/FluffyChi-Xing/fluffy-open-scp/actions/runs/46117976186)。
M3 sweep 始终检查原创合成模型包，本地有原版包时追加检查；CI 不宣称做过原版资源差分。
registry 发布预检、上传与 registry 消费者验证现已完成；发布标签指向包内记录的源码提交。
