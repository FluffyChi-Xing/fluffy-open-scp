# Linux 安装包支持调研（只读评估，未改代码）

> 2026-09-25。应 GitHub issue（希望提供 Linux 安装包）所作的现状盘点与改造清单。
> 结论基于对 `src-tauri/`、`crates/`、`src/` 全量扫描与打包配置核查，证据均给出
> `文件:行号`；本文本身是唯一新增产物。

---

## 1. 结论速览

**代码层面的移植成本很低——扫描结果里没有任何阻断性问题**。Tauri 2 + 纯 Rust
crates（无 Windows 专有依赖），Windows 专属代码全部已有 `#[cfg]` 分支或平台无关
实现。真正的工作量集中在**构建/发布流程**：Linux 包必须在 Linux 上构建（不能从
Windows 交叉编译），需要新增 CI 任务、补少量平台化细节（媒体工具路径、游戏目录
默认值）和文档。预估 **2.5–3.5 人日**。

| 维度 | 现状 | 判定 |
|---|---|---|
| 后端平台分支 | `atomic_fs`/`media_tools` 均已有 `#[cfg(windows)]`/`#[cfg(not(windows))]` 双分支 | ✅ 就绪 |
| 第三方依赖 | 无 winreg/windows/winapi；rusqlite 用 `bundled`；memmap2 跨平台 | ✅ 就绪 |
| 前端平台假设 | 无硬编码 Windows 路径/盘符/分隔符 | ✅ 就绪 |
| 打包配置 | `bundle.targets: "all"`（Linux 上=deb+rpm+AppImage），icon.png 已具备 | ✅ 基本就绪 |
| 构建环境 | 无 `.github/`（零 CI）；Linux 需 GTK/WebKitGTK 系统依赖 | ❌ 主要缺口 |
| 平台细节 | 媒体工具 bundled 路径写死 `.exe`；游戏目录默认值是 Windows 路径 | ⚠️ 小改 |

## 2. 现状证据（已就绪部分，无需改动）

### 2.1 打包配置（`src-tauri/tauri.conf.json`）

- `bundle.targets: "all"`：Linux 侧自动产出 **deb + rpm + AppImage**（Windows 上
  仍是 msi+nsis，互不影响）。
- `bundle.resources: ["resources/database_main.s3db"]`：资源经
  `tauri::path::BaseDirectory::Resource` 解析（`package_service.rs:4689`、
  `stats.rs:280`），三平台一致。
- `bundle.icon` 已含 `icons/icon.png`（Linux 只需 PNG，`icon.ico` 供 Windows）。
- `assetProtocol.scope: ["$TEMP/**"]`：`$TEMP` 是 Tauri 作用域变量，Linux 解析为
  `/tmp`，无需改。

### 2.2 后端平台分支（已有）

- **原子写**（`atomic_fs.rs:51` Windows 用 `MoveFileExW`+退避重试；
  `atomic_fs.rs:95` `#[cfg(not(windows))]` 用 `fs::rename`）——Linux 就绪。
- **媒体工具**（`media_tools.rs`）：
  - 进程创建的 `CREATE_NO_WINDOW` 有 `#[cfg(windows)]` 门（`media_tools.rs:99`）；
  - 测试已带 `#[cfg(not(windows))]` 用 `sh -c` 的对照分支（`media_tools.rs:300-330`）；
  - `executable_name()`（`media_tools.rs:221`）在非 Windows 下不带 `.exe` 后缀，
    PATH 兜底查找（`media_tools.rs:60-79`）天然可用 Linux 发行版的
    `ffmpeg` / `vgmstream-cli`。
- **路径防护**：目录校验全部基于 `fs::symlink_metadata().is_symlink()`
  （`settings.rs:79`、`mod_project.rs:102`），跨平台。
- **main.rs** 的 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
  在非 Windows 目标被 rustc 忽略，无需处理。
- **存储**：`openscp.db` 落 `app.path().app_data_dir()`（`lib.rs:90`）；
  `sc-store` 的 rusqlite 使用 **`bundled`** 特性（`crates/sc-store/Cargo.toml:9`，
  源码编译 SQLite，免系统库）；`dbpf` 用 `memmap2`（`crates/dbpf/Cargo.toml:10`），
  两者均跨平台。

### 2.3 前端

全量 grep 无 `C:\` 硬编码、无盘符/分隔符假设；文件树、路径展示全部消费后端
`PathBuf` 的 `to_string_lossy` 结果，分隔符随平台。

## 3. 需要做的调整（按优先级）

### P0 — Linux 构建环境与 CI（核心工作量）

**约束：Tauri 依赖 GTK/WebKitGTK 原生库，Windows 主机无法交叉编译出 Linux 包。**
必须在 Linux 环境（GitHub Actions `ubuntu` runner 或容器）构建。

1. **新增 GitHub Actions workflow**（当前仓库没有任何 CI，`.github/` 不存在）：
   - 建议矩阵：`windows-latest`（现有产物）+ `ubuntu-22.04`（Linux 产物）。
     选 22.04 而非 24.04 作为构建基线：AppImage 工具链与 glibc 兼容面更稳，
     22.04 上构建的包可在 24.04 运行，反之会有 glibc 版本问题。
   - Linux 侧系统依赖（Tauri 2 官方清单）：

     ```bash
     sudo apt-get install libwebkit2gtk-4.1-dev build-essential curl wget file \
       libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
     ```

   - 构建命令：`npm ci && npm run tauri build`；可用
     `--bundles deb,appimage` 精确控制产物（若不想一次出 rpm）。
   - 产物上传：`tauri build` 产出位于
     `target/release/bundle/{deb,rpm,appimage}/`，接 release/upload-artifact。
2. **验收环境**：至少一台 Ubuntu 22.04/24.04 实机或容器跑通安装→启动→打开
   package→预览→overlay 写回（写回是唯一涉及原子替换语义的链路，Unix 分支
   虽有实现但从未在真机跑过）。

### P1 — 代码内两处平台化小改

1. **媒体工具 bundled 路径**（`media_tools.rs:14-15`）：
   `VGMSTREAM_RELATIVE_PATH`/`FFMPEG_RELATIVE_PATH` 写死 `\.exe`。Linux 上
   bundled 探测恒失败，静默走 PATH 兜底（可用但不完整）。改法：常量拆成
   per-platform（`#[cfg(windows)] "Tools/vgmstream/vgmstream-cli.exe"` /
   `#[cfg(not(windows))] "Tools/vgmstream/vgmstream-cli"`），或改为在
   `resolve_tools` 里用 `executable_name` 拼接目录 + 词干。
   注意 `media_tools.rs:260` 的测试同样引用该常量，需随动。
2. **游戏目录默认值**（`settings.rs:12`）：
   `DEFAULT_GAME_DATA_PATH = C:\Games\SimCity\SimCityData`。Linux 上探测结果
   只是「不存在」（无崩溃），但体验上应给 Linux 默认候选：
   - SimCity(2013) 是 Windows 游戏，Linux 用户经 Wine/Proton 运行，游戏目录
     在 prefix 内，典型形如
     `~/.steam/steam/steamapps/common/<game>/...` 或
     `~/.wine/drive_c/...`（EA App 走 `~/EA Games/...`）；
   - 建议：`DEFAULT_GAME_DATA_PATH` 改 `#[cfg]` 双值 + 文档引导用户手动选择
     （探测逻辑 `game_directory_detect` 无需改动）。

### P2 — 打包元数据与体验补齐

1. **deb 元数据**（`tauri.conf.json` → `bundle.linux.deb`）：可选补
   `section`/`priority`/`depends`（Tauri 会自动注入 webkit2gtk 等运行时依赖，
   一般无需手填）。
2. **AppImage 的 Tools/ 目录**：`application_dir()` = `current_exe().parent()`
   （`media_tools.rs:198`）。AppImage 挂载点是只读 squashfs，「把 ffmpeg 放到
   程序旁边」的 Windows 习惯在 AppImage 下不成立。两个选项：
   - 文档引导 Linux 用户装发行版 ffmpeg/vgmstream（PATH 兜底已可用，成本
     最低，**推荐**）；
   - 或增加 `app_data_dir()/Tools` 作为第二探测位（代码 +1 处）。
3. **Wayland**：WebKitGTK 在 Wayland 下偶有缩放/输入法问题；Tauri 2 默认走
   X11/XWayland。如有用户反馈，再考虑加 WEBKIT_DISABLE_COMPOSITING_MODE
   类已知 workaround 文档，不必预做。
4. **文档**：CONTRIBUTING.md 增补 Linux 构建一节（依赖清单 + 命令）；README
   的「平台支持」口径更新；issue 回复口径（Linux 版 = 浏览器/编辑器完整可用，
   游戏本体仍需 Proton/Wine）。

## 4. 明确不需要做的（避免扩大范围）

- 不需要引 `windows` crate 替代品或 cfg 大改——Windows 专有点已隔离。
- 不需要改 DBPF/Property/RW4/ERZ 解析栈——纯字节解析，平台无关。
- 不需要前端改动——无平台假设。
- 不需要 Flatpak/Flathub——issue 只要求安装包；deb+AppImage 已覆盖主流发行
  版。Flatpak 若将来要做，是独立工作量（manifest、runtime、沙箱权限）。
- 不需要签名/公证流程——Linux 侧无强制签名；如需 sha256 校验和，CI 里
  `sha256sum` 顺带产出即可。

## 5. 建议的产物矩阵

| 目标 | 触发 | 产物 | 说明 |
|---|---|---|---|
| Windows（现状） | tag push | nsis/msi（现流程不变） | — |
| Linux deb | tag push | `target/release/bundle/deb/*.deb` | Debian/Ubuntu 系 |
| Linux AppImage | tag push | `target/release/bundle/appimage/*.AppImage` | 免安装、通用发行版 |
| （可选）rpm | tag push | `target/release/bundle/rpm/*.rpm` | Fedora 系；`--bundles` 控制 |

发布流程建议：PR 只跑 `cargo test` + `vue-tsc/vitest`（多平台矩阵顺带验证
Unix 分支不腐烂）；打 tag 才出安装包，Windows 与 Linux 两个 runner 并行，
产物统一挂 GitHub Release 并附 sha256。

## 6. 风险与开放问题

1. **WebKitGTK 运行时碎片化**：deb 用户侧需 webkit2gtk-4.1 运行库（Tauri 的
   deb depends 会带上）；个别老发行版需额外源。AppImage 不受影响（自带但
   用系统 webkit 的方案仍依赖宿主——Tauri 的 AppImage 打包会捆绑
   webkit2gtk，体积 +~100MB，属正常现象）。
2. **glibc 基线**：22.04 构建的 deb/AppImage 在更老发行版（20.04）可能无法
   运行；issue 回复时给出「支持 Ubuntu 22.04+ / Debian 12+」的口径。
3. **游戏数据获取**：OpenSCP 只读文件，功能跨平台无损；但 Linux 用户拿到
   游戏文件本身依赖 Proton/Wine 安装 SimCity。这是产品口径问题，不是代码
   问题——文档写清楚即可。
4. **Unix 原子写分支的实战验证**：`atomic_fs.rs:95` 的 `fs::rename` 分支
   语义与 Windows 版一致（同目录 rename 原子），但从未被真机跑过，纳入
   P0 验收清单。

## 7. 工作量估算

| 项 | 估时 |
|---|---|
| CI workflow（双平台矩阵 + 产物上传 + sha256） | 0.5–1 天 |
| media_tools 平台化 bundled 路径（含测试） | 0.5 天 |
| 游戏目录默认值/引导平台化 | 0.5 天 |
| README/CONTRIBUTING/issue 口径文档 | 0.5 天 |
| Ubuntu 真机/容器验收（安装、预览、写回、媒体预览） | 0.5–1 天 |
| **合计** | **2.5–3.5 人日** |
