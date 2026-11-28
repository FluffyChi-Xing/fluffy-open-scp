# OpenSCP Native Rust SDK

[0.1.0 发布范围与闸门](../../docs/native-sdk-0.1.0-release.md) 定义首发交付与剩余验收。

基于真实下游的 API 与职责划分见 [Simoder 负载合同](../../docs/native-sdk-simoder-workload.md)。

`openscp-sdk` 为社区 Rust 工具提供统一格式入口，复用现有解析器。
SDK 独立为 0.1.0，目前处于候选开发，暂不发布 crates.io；消费者应固定经过验证的
Git revision 并提交自己的 Cargo.lock。现有四库的直接依赖继续可用。

```toml
[dependencies]
# 将 <verified-commit> 替换为通过消费者检查的完整提交 SHA。
openscp-sdk = { git = "https://github.com/FluffyChi-Xing/fluffy-open-scp", rev = "<verified-commit>" }
```

```rust,no_run
use openscp_sdk::{Result, package::Package, properties::PropertyFile};

fn inspect(path: &std::path::Path) -> Result<()> {
    let package = Package::open(path)?;
    for entry in package.entries() {
        if entry.id.type_id == 0x00b1b104 {
            let payload = package.read(entry)?;
            let properties = PropertyFile::parse(&payload)?;
            println!("{}: {} properties", entry.id, properties.values.len());
        }
    }
    Ok(())
}
```

## 功能与 API 合同

| Feature | 公共入口 | 默认 |
|---|---|---|
| 无需 feature | `format::{probe, Format}`、`Error`、`Result` | 是 |
| `package` | `package` 模块、根目录 `ResourceId` | 是 |
| `properties` | `properties` 模块，含 PROP 编解码、继承、locale 与 debug_tools | 是 |
| `rw4` | `rw4` 模块 | 是 |

只需格式探测时设 `default-features = false`；只读包时再开启 `package`。
`properties` 自身依赖 DBPF、RW4 和 image，因此它不是轻量 PROP-only 构建。
模块重导出原库，`ResourceId` 与 `dbpf::ResourceId` 是同一类型，不复制模型或解析实现。
SDK 正常依赖不包含 Tauri、SQLite、registry、编辑器或游戏进程控制。

`ResourceId` 的三个字段均为 u32；64 位清单 instance 不能无检查地截断。
`Package::read_raw` 返回存储字节，`Package::read` 返回解压后的 payload。
`encode_canonical` 是重新编码，不承诺保留原始 PROP 字节；未编辑或未知资源应保留原字节。
`write_uncompressed_overlay` 生成新的确定性未压缩包，不承诺原包字节一致。
文件映射仍遵循底层 `Package` 合同，调用者需避免读取期间替换或截断文件。

`Error` 为可扩展的组合错误，保留原始 source，`code()` 提供机器可读分类。
各模块仍返回原生错误，不会自动附加来源路径或 TGI，调用方应保留操作上下文。
格式 signature probe 仅探测签名，其原生 `ProbeError` 暂不纳入组合错误。
内存包可用 `package::MemoryPackage::parse(&bytes)` 无临时文件读取，
`read(entry, max_decoded_bytes)` 要求显式单资源解码预算；`read_raw` 借用存储字节。
`parse_with_limits(bytes, MemoryParseLimits)` 在索引分配前限制输入大小及条目数。
`resource::checked_id` 显式拒绝超出 u32 的 manifest instance。
PROP 可通过 `ParseLimits` 限制解析；RW4 尚无统一进程内存预算，尤其不能把
x64 测试结果视为已通过 i686 支持验收。

## 验证

运行仓库根目录的 `python scripts/check_native_sdk.py`，检查 feature 组合、
现有四库回归、独立消费者与正常依赖边界。独立样例仅使用合成数据。
`cargo run -p openscp-sdk --example inspect_package -- <file.package>` 可只读列出索引。

初始验证（2026-11-28）：Windows x64 的全部八种 feature 组合和独立消费者测试，
Windows i686 的独立消费者三项测试均通过。四库回归有一项已有 ignored 测试；
部分真实游戏数据测试在缺少本地数据时会提前返回，不作为可移植 fixture 验收。
新增 CI 在 Windows/Linux 运行检查，Windows 额外运行 i686 消费者；Linux CI 尚待远端执行。

Simoder 可保留 `simoder-formats` 门面，逐步把 import 改为 SDK 模块；其宿主配置、
激活状态与运行期语义仍由 Simoder 持有。本轮不改其固定 revision，也不宣称已完成
Simoder x64/i686 升级验收。UI 移植、包冲突扫描、建筑构建与运行期热加载留待独立实现。
