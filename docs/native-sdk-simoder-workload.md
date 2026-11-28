# Native SDK：Simoder 真实调用合同

2026-11-28。源码基线：Simoder-Base `6f98f956cb2d7f2b03dca983510cb0b87768b995`，
Simoder `b2e5ae4431dd9876044c8d5164e49346827f96fe`。下列边界来自本地源码，
不把格式测试或脚本声明当作游戏内已激活的证据。

## 两种负载

Simoder-Base 的 `mod.toml` 是 schema 2、Luau 入口、8 MiB VM 与 2 ms 回调预算。
`scripts/services/ui-manager.luau` 持有 owner/generation、视图槽位、组合与撤销；
`runtime-manager.luau` 和 `lifecycle.luau` 接受宿主授权与任务租约。
这些不是 DBPF 容器服务。其 `docs/PACKAGING.md` 描述的 USTAR 源包、信任选择与
`package-files.json` 的成员选择，也不同于向游戏提供资源的 DBPF overlay。
不能把 Base 的 VM 内存预算误用成所有资源解码预算。

真正调用 OpenSCP 的桥接点在 Simoder：

| 真实源码 | 需要的 SDK 合同 | 仍由消费者负责 |
|---|---|---|
| `simoder-core/src/lib.rs` 的 `Tgi = dbpf::ResourceId` | 同一 u32 TGI 类型身份 | manifest schema 3 的 64 位 instance 校验与显式拒绝转换 |
| `simoder-formats/src/lib.rs` | DBPF/RefPack、PROP、RW4、format，locale 与 ERZ | TOON/TOML 配置及 mod locale ID 分配 |
| `simoder-formats/tests/formats_tests.rs` | float 300→345、其他字段保留、canonical 语义回读 | set/add/multiply 等 patch 操作的业务约束 |
| `simoder-mods/src/overlay.rs` | overlay 编码与内存索引检查 | package-info 签名政策、挂载位置、原版内容准入与引用改写 |
| `simoder-mods/src/assets.rs` | PROP/RW4 结构解析 | 路径隔离、reparse 防护、SHA-256、原版资产复制政策 |
| `simoder-mods/src/patch_registry.rs` | 可复用的原始资源身份/字段模型 | owner 顺序、冲突赢家、generation、启停及恢复 |
| `simoder-mods/src/native_host/tool_registry.rs` | 现有 `properties::debug_tools` | 工具权限、菜单部署与游戏是否真的重读 |

源码位于调用者提供的相邻 checkout；它们没有被 vendoring 或发布到 SDK。

## 根据负载增加的 API

`package::MemoryPackage::parse(&bytes)` 直接复用原 DBPF/DBBF header/index 解析。
这是为了消除 Simoder `inspect_package_bytes_metadata_policy` 的临时文件往返，
并复用原 RefPack 解析，不重新实现格式。`Package::open` 和原调用者保持原行为。

```rust,no_run
use openscp_sdk::{Result, package::MemoryPackage};

fn inspect(bytes: &[u8], max_payload_bytes: usize) -> Result<()> {
    let package = MemoryPackage::parse(bytes)?;
    for entry in package.entries() {
        // 原始存储字节可借用；解码结果按调用方预算分配。
        let stored = package.read_raw(entry)?;
        let payload = package.read(entry, max_payload_bytes)?;
        println!("{}: stored={}, decoded={}", entry.id, stored.len(), payload.len());
    }
    Ok(())
}
```

`MemoryReadError::LimitExceeded` 给出 TGI、声明大小与调用者上限；其他错误保留原 source。
`Error` 现纳入内存读取与 ERZ，分别返回 `openscp.package.memory` 和 `openscp.erz` 分类。
locale 的既有 String 错误仍是底层合同，尚未伪装成统一有结构诊断。

该预算只限制单次 payload 输出分配，不限制输入字节、索引分配、总并发或 RW4/图片解码。
调用者仍需对文件/网络输入和作业总量设限。重复 TGI 保留原索引顺序，`entry` 返回首条；
这不是跨包加载优先级或冲突决策 API。包写入不自动删改 metadata，不隐式选择部署目录。

## 消费者升级形态

保留 `simoder-formats` 门面；把内部四库 import 转成 SDK 的模块别名即可继续现有调用。
但 `simoder-core` 和 `simoder-mods` 还有直接底层依赖：必须同步到同一源码来源/revision，
否则 Cargo 可能同时加载 Git 和 path 两套 `dbpf`，导致同名 TGI 类型无法交换。

内存预检替换建议：先 `MemoryPackage::parse`，再由宿主按原规则统计
`(type=0x00B1B104, group=0x40404000)` 条目。零/一/多条如何准入仍由宿主决定。
原来的 `inspect_package_metadata_policy(&Package)` 可保留给文件调用者。

## 验证与剩余边界

`scripts/check_native_sdk.py` 运行格式回归与八种 feature 组合。
独立 SDK 消费者覆盖实际 scalar patch→canonical→locale→overlay→内存预检，
未知 payload 保留、超预算拒绝与 ERZ 错误分类。

`python scripts/check_simoder_sdk.py D:/rust/packages/Simoder` 会在 OpenSCP `target/`
下建立独立副本，保留真实 core/formats/mods 源码与测试，仅将 formats 的四库依赖
换为 SDK 模块别名，并把 core/mods 直接依赖统一到本地同一来源。
支持 `--target i686-pc-windows-msvc`。不会修改 Simoder 或 Base 工作区、锁文件和固定版本。
这属于真实消费者格式/模组测试，仍不覆盖 Base VM、D3D9 DLL、权限执行与游戏刷新。

本轮结果：真实 Simoder x64 共 21 项测试通过；i686 编译完成且前 19 项通过，
最后 `ui_patch_registry` 测试程序被 Windows `os error 740`（要求提升）阻止启动，
其两项测试未执行，不能报告 i686 全套通过。未提升权限或修改运行环境策略。
独立 SDK 消费者的三项测试在 x64/i686 均通过；DBPF 内存读取两项测试通过，
八种 feature 组合与原四库回归通过（保留一项既有 ignored）。
没有修改 Simoder/Simoder-Base 工作树，也没有启动游戏或验收运行期启用。

UI 编辑器移植、MOD 管理、Base USTAR 打包与 runtime 权限不纳入这个格式 SDK。
后续有第二个消费者需要离线包差异或冲突报告时，再新增显式输入输出的工作流模块。
