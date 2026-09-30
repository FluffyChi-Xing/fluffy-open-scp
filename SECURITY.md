# 安全策略 / Security Policy

本文档分中英文两部分，内容一致。
This document is provided in Chinese and English; both sections are identical in content.

---

## 中文

### 支持版本

我们只对 `master` 主干（及其产出的最新构建）跟进安全修复；历史提交/构建不接收补丁。

| 版本 | 支持 |
| --- | --- |
| `master` 最新构建 | ✅ |
| 历史提交 / 旧构建 | ❌（请升级） |

### 如何报告漏洞

**请勿以公开 Issue 报告安全问题。**

- 首选：GitHub 仓库 **Security → Report a vulnerability**（私有漏洞报告）。
- 备选：通过 GitHub 私信联系项目所有者 [@FluffyChi-Xing](https://github.com/FluffyChi-Xing)。

我们承诺在收到报告后 **7 天内**给出初步响应；修复节奏视严重程度而定，修复发布前会与你协商披露时间线。

报告请尽量包含：影响描述、复现步骤、可触发的**最小合成样本**（如手工构造的 `.package`）。

> ⚠️ 不要在报告中上传游戏原版资源或你的存档数据——请构造能触发问题的最小合成文件。

### 范围

OpenSCP 是一个**解析不可信第三方容器（`.package`）的本地桌面应用**（Rust 解析 crate + Tauri 2 + Vue 3）。重点关注的风险面：

- **解析器健壮性**：恶意或损坏的 DBPF 容器、RefPack 流、RW4 模型/贴图、属性表输入导致 panic、内存膨胀（OOM）、无限循环（DoS）。
- **导出与写回路径**：路径穿越（如导出文件名逃逸目标目录）、对任意路径的非预期写入。
- **应用层（Tauri/前端）**：IPC 命令被越权调用、前端渲染不可信文本导致的注入升级。
- **供应链**：依赖被投毒或版本劫持。

**不在范围**：游戏本体（SimCity 2013）的漏洞、使用本工具制作的模组内容所造成的影响、对游戏在线服务的任何滥用、社会工程。

### 安全设计基线

- 解析器为独立纯 Rust crate（`crates/`），不依赖 Tauri、可独立模糊测试；`unsafe` 使用严格受限（当前全库仅个别处，均有注释说明）。
- 本工具**不分发任何游戏原版资源**；所有解析与预览均在本地完成。
- 导出/写回只作用于用户显式选择的目标路径。

---

## English

### Supported Versions

We only track security fixes on the `master` trunk (and the latest builds produced from it); historical commits/builds do not receive patches.

| Version | Supported |
| --- | --- |
| Latest `master` build | ✅ |
| Older commits / builds | ❌ (please upgrade) |

### Reporting a Vulnerability

**Please do not report security issues through public GitHub issues.**

- Preferred: the repo's **Security → Report a vulnerability** (private vulnerability reporting).
- Alternative: contact the project owner [@FluffyChi-Xing](https://github.com/FluffyChi-Xing) via GitHub private message.

We aim to acknowledge reports within **7 days**; fix timelines depend on severity, and disclosure will be coordinated with you before the fix ships.

Please include: impact description, reproduction steps, and a **minimal synthetic sample** (e.g. a hand-crafted `.package`) if possible.

> ⚠️ Do not attach original game assets or your save games — craft a minimal synthetic file that triggers the issue instead.

### Scope

OpenSCP is a **local desktop application that parses untrusted third-party containers (`.package`)** (Rust crates + Tauri 2 + Vue 3). Priority risk surfaces:

- **Parser robustness**: malicious or corrupt DBPF containers, RefPack streams, RW4 meshes/textures, and property-list inputs causing panics, memory exhaustion (OOM), or infinite loops (DoS).
- **Export & write-back paths**: path traversal (e.g. export filenames escaping the target directory), unintended writes to arbitrary paths.
- **Application layer (Tauri/frontend)**: unauthorized invocation of IPC commands, injection escalation from rendering untrusted text.
- **Supply chain**: dependency poisoning or version hijacking.

**Out of scope**: vulnerabilities in the game itself (SimCity 2013), impact of mod content built with this tool, abuse of the game's online services, and social engineering.

### Security Design Baseline

- Parsers are standalone pure-Rust crates (`crates/`) with no Tauri dependency, suitable for standalone fuzzing; `unsafe` usage is strictly limited (currently a couple of annotated sites across the workspace).
- This tool **never distributes original game assets**; all parsing and previewing happens locally.
- Exports/writes only target paths the user explicitly selected.
