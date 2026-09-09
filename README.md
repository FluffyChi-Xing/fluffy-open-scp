<p align="center">
  <img src="docs/assets/scp-logo.png" width="180" alt="OpenSCP logo" />
</p>

# OpenSCP

> 基于 Rust + Tauri 2 + Vue 3 的 SimCity (2013) `.package` 资源浏览器与模组工具链 —— 浏览资源结构、预览模型与贴图、编辑属性表、导出 glTF/OBJ/音视频，并逐步构建声明式 Modding Suite。

## 这个项目是什么 / 目标

OpenSCP 是经典 C#/WPF 工具 [SimCityPak](https://github.com/altinctrl/SimCityPak) 的现代化重写，但不止于"重写一个浏览器"。目标分三层：

1. **可靠的格式底座（P0）**：DBPF 容器、RefPack 解压、RenderWare4（RW4）模型/贴图/骨骼/动画、属性表（property list）的精确解析与可保真写回。所有解析器为纯 Rust crate（`crates/`），不依赖 Tauri，可独立测试复用。
2. **可用的资源工作台（进行中）**：包结构树（TGI 浏览）、语法高亮文本查看（Shiki）、RW4 模型 3D 预览（含材质通道）、贴图/属性表/音视频预览、Lot 编辑器会话（灯光/贴花/Prop/路径单位）、首页统计仪表盘（扩展名数量/容量占比、可识别覆盖率）。
3. **模组工作流编排（P1）**：`openscp.mod.toml` 声明式项目、OBJ 导入与 LOD 管理、依赖解析、自动校验、确定性 overlay package 构建。详见 [docs/roadmap/modding-suite.md](docs/roadmap/modding-suite.md)。

## 使用方法 / Usage

### 安装与启动

1. 从发布页下载安装包（MSI / NSIS），或从源码构建（见下节开发说明）；
2. 首次启动后，在 **设置 → 游戏目录** 中指定 SimCity 安装目录（通常为含 `SimCityData\` 的那一层，如 `D:\ea-games\SimCity`）。OpenSCP 会自动扫描并呈现所有 `.package`；
3. 也可以在包列表中直接双击任意 `.package` 打开，无需配置。

### 日常操作

- **浏览**：左侧资源树按目录列出 package；打开后在资源页中按 TGI（Type/Group/Instance）筛选、搜索、分页浏览（大包也流畅，虚拟化渲染）；
- **预览**：点击资源自动识别类型 —— 模型（RW4 3D 视口，含材质/LOD）、贴图（PNG/JPG/TGA/DDS/Raster）、属性表（可读键值 + 编辑）、文本（JSON/HTML/JS 语法高亮）、音频（Wwise → WAV 试听）、视频（VP6 → MP4 播放）、二进制（hex 视图）；
- **Lot 编辑器**：打开 Lot 资源可查看地面图（LotMask 四色量化）、模型 LOD 链、灯光/贴花/Prop 等单位列表与变换矩阵；
- **导出**：模型 → glTF 2.0（`.glb`，含材质/骨骼/动画）或 Wavefront `.obj`；贴图 → PNG/JPG/TGA/DDS；属性表 → JSON/TXT；音频 → WAV；视频 → MP4；
- **首页仪表盘**：统计历史打开 package 的扩展名数量占比（环形图）、容量占比（条形图）、可识别类型覆盖率，可展开查看单包明细。

### ⚠️ 音视频转码需要手动安装外部工具

音频（Wwise → WAV）和视频（VP6 → MP4）预览/导出**依赖两个外部工具，安装包不会自动附带**：

| 工具 | 用途 | 安装方式（任选其一） |
|---|---|---|
| **vgmstream** | Wwise Vorbis 音频 → WAV | `winget install --id vgmstream.vgmstream -e --source winget`；或从 [GitHub Releases](https://github.com/vgmstream/vgmstream) 下载解压；也可放入应用目录 `Tools\vgmstream\` |
| **ffmpeg** | VP6 视频 → MP4 | `winget install --id Gyan.FFmpeg -e --source winget`；或从 [ffmpeg.org](https://ffmpeg.org) 下载；也可放入应用目录 `Tools\ffmpeg\` |

未安装时音频/视频预览会显示工具名与安装命令提示，其余功能不受影响。查找顺序：应用目录 `Tools\` → 系统 `PATH`。

## 项目结构 / Project Layout

```
fluffy-open-scp/
├── src/                    # Vue 3 前端（fluffy-design-pro 应用壳）
├── src-tauri/              # Tauri 2 应用壳（command 绑定层）
├── crates/
│   ├── dbpf/               # DBPF (.package) 容器格式 + RefPack 解压
│   ├── rw4/                # RenderWare4 模型 / 贴图 / 骨骼 / 动画解析
│   ├── sc-properties/      # 属性列表资源 (0x00b1b104) 解析
│   ├── sc-registry/        # database_main.s3db 描述符库只读访问
│   ├── sc-store/           # 应用活动/历史持久化 (SQLite)
│   └── sc-exporter/        # OBJ / glTF / 贴图 / 属性表导出器
└── docs/
    ├── overview/           # 领域知识：渲染管线、文件格式
    ├── rendering.md        # 渲染器源码调研（shader 逆向）
    └── roadmap/            # 迁移与 Modding Suite 规划
```

## 路线图 / Roadmap

- **P0 底座**（大部分已完成）：DBPF/RefPack、RW4、属性表的可靠解析与写回，OBJ 导入、overlay package、精细错误和 roundtrip 测试
- **当前进行**：预览体验完善（模型视口材质通道、日夜循环/供电状态渲染、relief 高度图）、格式覆盖率扩展（EP1 二进制表、GlassBox 数据层反查）
- **P1 Modding Suite**：`openscp.mod.toml` 声明式项目、一键构建、LOD/依赖管理、自动校验、可行动诊断、预览与文件监听
- **P2/P3**：高级材质/LOD/骨骼编辑，游戏联动、依赖生态和插件能力

详细规划：[docs/roadmap/modding-suite.md](docs/roadmap/modding-suite.md)；底层迁移进度：[docs/roadmap/migration.md](docs/roadmap/migration.md)；领域知识入口：[docs/overview/rendering-pipeline.md](docs/overview/rendering-pipeline.md)、[docs/overview/file-formats.md](docs/overview/file-formats.md)、[docs/overview/ui-rendering-and-development.md](docs/overview/ui-rendering-and-development.md)、[docs/overview/glassbox-engine.md](docs/overview/glassbox-engine.md)。

## 开发 / Development

环境要求：Rust（stable，edition 2024）、Node.js ≥ 20、pnpm ≥ 11，以及 Tauri 2 在 Windows 下的常规依赖（WebView2、MSVC Build Tools）。

```bash
pnpm install          # 安装前端依赖
pnpm tauri dev        # 桌面应用开发模式
pnpm tauri build      # 构建发布版
pnpm test             # 前端测试（vitest）
cargo test            # Rust 测试（workspace）
```

浏览器 demo 模式：非 Tauri 环境下访问 vite dev server，在控制台执行
`localStorage.setItem("openscp:local-key", '{"version":1,"mode":"mock"}')` 后刷新，可用合成数据预览全部 UI。

## 致谢 / Credits

- [altinctrl/SimCityPak](https://github.com/altinctrl/SimCityPak) —— 本项目迁移来源（原 CodePlex 项目，原始权利归原作者所有）
- [vgmstream](https://github.com/vgmstream/vgmstream) / [ffmpeg](https://ffmpeg.org) —— 音视频解码
- [fluffy-design-pro](https://github.com/FluffyChi-Xing/fluffy-design-pro) —— UI 应用壳与组件生态

## License

MIT。SimCity 与 SimCityPak 相关的原始权利归其各自作者所有。
