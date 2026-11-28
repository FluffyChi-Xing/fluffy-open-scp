# SC5 Packager 假内景 UV 修复迁移

日期：2026-11-22。仅迁移 UV 来源判定与写入适配。

## 参照身份

- 输入：`D:/ea-games/simcity_offline/entity/SC5 Asset Packager (2).exe`
- SHA-256：`A21E351681C964FB451F4E6576C3C62A673C324A96599CF46874FE729B8E2995`
- PyInstaller / Python 3.12，静态解包，未执行目标 EXE。
- 解包位置：系统 `%TEMP%/openscp-packager-a21e3516/`，不在仓库内；未复制解包源码、pyc、依赖或资源进 Git。
- 内嵌 `engine/prepare_asset.py` SHA-256：`10c585fd3efaf14a8bc2571002480ff03c457066f3887d67bb73b768bebcb7ce`。

## 原作者修复

`prepare_asset.py:15-22::source_uv_convention` 识别显式 UV 约定；历史
SC5 Asset Editor 的 `openscp.ny-native/1` 导出默认为 native-v-down。
`compile_geometry_contract.py::compile_contract` 对 Base/Top 两层都应用：
native 保留 V，Blender 写入 `1-V`，U 保留。
`asset_transforms.py::room_strip` 采样公式未变。

## Rust native Blend 导入范围

Rust 对应 `asset_build_m1/source_uv.rs`，接入 `author_building40.rs` 主路径及
`native_model.rs::compile_building40_template` 兼容路径。

Rust `.blend` 导出器 `blend_native/export.rs::overlay` 已对房间 Base UV 和
窗框 Top UV 做过 `1-V`，属于 native-v-down。此前 diagnostics 没有声明
uvConvention，旧编译器又无条件翻转，造成双重翻转。

现在新 Rust 导出明确写入 `uvConvention=native-v-down`。编译器只对
`schema=openscp.ny-native/1` 且 `source=OpenSCP native Rust` 启用这条规则，
旧 Rust 导出缺声明时也按 native 处理。显式 Blender 声明仍翻转，未知声明拒绝。
其他来源（包括原版建筑及历史 Editor 来源）保留修改前的 UV 适配行为。
这是将作者的 native-v-down 写入公式映射到 Rust 导出来源；不是照搬其 Editor 来源识别范围。
上一轮仅识别 Editor、Rust 却仍落入 Blender 默认值的实现已修正。

保持材质、房间图集、旋转、碰撞和去重规则。构建缓存版本为
`g3-10-rust-blend-native-uv`，Blend 导入缓存版本为
`openscp.blender.2026-11-22.native-uv`。

## 验证边界

- 独立 rustc harness 直接引用生产 `blend_native/mod.rs` 及 `source_uv.rs`：
  33 项测试通过、1 项需真实 bundle 的测试跳过。包含 Rust 导出声明检查和
  非 Rust 来源保持历史浮点字节检查。
- 独立 Building40 harness 直接引用生产 author_building40、collision、lotm、
  source_uv 模块，提取生产旋转函数及合成 LOTM 测试函数：4 项测试通过。
  实际编译 RW4 并回读顶点 Base/Top V；U 保留；非法 Rust UV 声明拒绝。
  此 harness 绕开 Tauri 运行库，未替代完整应用测试。
- 完整应用测试此前编译成功但启动失败
  `0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND`（沙箱内外均复现）。
- 未部署包、未更改 SimCityData、未启动游戏；运行时外观仍需用户验收。
