# Dev Beta 样本盘点与真伪判定（2026-10-04）

> 样本来源：Discord 用户投递，声称来自早期 dev 版本。
> 原始位置 `D:\ea-games\simcity_dev`，已按工作区规则复制到
> `tmp/dev-sample-analysis/`（429MB，含参照 EXE 副本 `ref/`）。
> 本文为三篇分析之一：样本内容清单 + 真伪判定。
> 对比分析见 `dev-beta-vs-retail-diff.md`，对 OpenSCP 未决问题的价值见
> `dev-beta-openscp-leads.md`。

## 一、样本内容清单

### 1.1 顶层文件

| 文件 | 大小 | 内容 |
|---|---|---|
| `SimCity.unwrapped.exe.c` | 71.8MB / 1,897,901 行 | **Hex-Rays 9.5 全量反编译**，含完整 PDB 符号（真实函数名/类名/字段名），是投递者从脱壳 exe + PDB 在 IDA 中产物 |
| `message.txt` | 25KB | `SC::cSimCityApp::Init` 单函数反编译（发件人摘贴，内容与大 .c 一致） |
| `message(1).txt` / `message(2).txt` | 9.8KB | `SC::cToolPathPlacer::Update` 单函数反编译，两文件**逐字节相同**（重复投递） |

### 1.2 `[WIN] SimCity 2013 (Dev Beta) (v253660365) [2013-01-14] (PDB)/`

| 文件 | 大小 | 说明 |
|---|---|---|
| `SimCity.exe` | 10,109,616 B | 原始 dev beta 可执行文件（带 DRM 包装），PE 时间戳 2013-01-15 05:25:17 UTC |
| `SimCity.pdb` | 73,935,872 B | **完整 PDB 调试符号**（核心价值资产） |
| `SimCity.unwrapped.exe` | 10,109,616 B | DRM 脱壳版：布局与原 exe 完全相同，仅入口点 0xa93830 → 0x62ae02（OEP 恢复），去掉包装 stub |
| `DRM encryption key.txt` | 32 B | `E21829921A15DD802DF8B8FF7A0D2B61`（脱壳用密钥，历史性） |
| `Original Files/` | ~209MB | 完整原始安装目录（见 1.3） |

### 1.3 `Original Files/`（原始安装树）

- `SimCity.exe` / `SimCity.pdb` / `SimCity.par`（142B，加密 parcel 文件）
- `version_app.txt`（**构建身份证**，见下）
- `SCUpdate.exe`（166KB，更新器）
- `EAWebkit.dll`（9.6MB）+ **`EAWebkit.pdb`（118MB）**（内嵌浏览器 UI 的符号）
- `D3DCompiler_43.dll`、`D3DX9_43.dll`、`d3dx9_31.dll`
- `Core/`：Origin 激活栈（Qt4 全家桶、Activation.dll、ActivationUI.exe、OpenSSL、msvcrt100）

`version_app.txt` 原文：

```
Description: SimCity RL Release Configuration
Changelist: 707103
Version: 253660365 / 0x0f1e8ccd / 2013_01_14_21_12_45
Built at Mon Jan 14 21:26:00 2013 on EMY1-SPAMBOT
```

## 二、真伪判定：**真**（六条独立证据链）

### 证据 1：PDB 与 EXE 的 GUID 严格互锁（最强证据）

从 exe 的调试目录提取 RSDS 记录，与 PDB 内部流 1 的 GUID 对比
（解析脚本 `tmp/dev-sample-analysis/pdb_guid.py`）：

| 对象 | RSDS/PDB GUID | age | PDB 路径 |
|---|---|---|---|
| dev `SimCity.exe` | `e291cc5d-849b-4e64-a405-3b1f7fe19ab1` | 1 | `c:\BF\CM\SimCity_RL\out\Win32_90\Release\SimCity.pdb` |
| dev `SimCity.unwrapped.exe` | 同上（逐字节一致） | 1 | 同上 |
| **`SimCity.pdb` 内部** | **`e291cc5d-849b-4e64-a405-3b1f7fe19ab1`** | **1** | — |
| 零售 `SimCity.exe` (CoT) | `cb3004aa-431c-4d3a-8cb4-c26ae65dada5` | 1 | `c:\BF\CM\SimCity_PL\out\Win32_90\Release\SimCity.pdb` |
| `SimCity_dump_SCY.exe` | 同零售（一致） | 1 | 同上 |

PDB 内部签名 `0x50f4e83c` = PE 时间戳 `0x50f4e83d` **减 1 秒**——
链接器先写 PDB 再写 PE 的典型时序。GUID 是 128 位随机值，
 exe↔PDB 精确互锁无法伪造（伪造者需要同时改两处且知道算法）。

### 证据 2：构建身份证与反编译代码互证

`message.txt`（dev 反编译）中 `SC::cSimCityApp::Init` 的 `--version` 分支：

```c
sprintf(&v, "App Version %u\n%s\n", 253660365, "RL 707103");
```

与 `version_app.txt` 的 Version 253660365 / Changelist 707103 / RL 配置
**三项全对**。零售版 exe 中的对应字符串为 `"PL 839109"`（另一分支）。

### 证据 3：包装/脱壳对自洽

原 exe 比脱壳 exe 多一个 0xa93000 段（0x12f6 字节 stub），两者其余段
（.text/.rdata/.data/.tls/.rsrc）逐字段相同——这正是"同一份二进制、
一个带 EA DRM 壳、一个已还原 OEP"的关系，与附带 DRM 密钥的故事闭环。

### 证据 4：dev 独有痕迹

- `ReleaseLogForDevs_DO_NOT_SHIP.txt` 日志通道与
  `"DO NOT SHIP AN EXECUTABLE THAT GENERATES THIS LOG!!!"` 字符串
  **存在于 dev exe，在零售 dump 中 0 命中**（已 grep 验证）。
- 构建机名 `EMY1-SPAMBOT`（EA 构建集群命名风格），构建路径
  `c:\BF\CM\SimCity_RL\`（BF = EA 内部构建场目录约定）。

### 证据 5：PDB 源码树内部一致

从 PDB 提取 **5,658 个源文件路径**（`tmp/dev-sample-analysis/out/pdb_source_paths.txt`）：
3,020 个位于 `c:\bf\cm\simcity_rl\`，模块分布自洽——
SCGraphics(55) / SCTool(24) / SCUI(18) / SCTransport(15) / SCZoning(13) /
SCDisaster(10)（游戏本体）+ SPGraphics/SPLib/SPNet（Spark 引擎）+
GBLib（GlassBox）+ UTF*（EA 共享技术栈）+ Bullet/RW/Wwise/FreeType/LZMA
（第三方）。没有零售版后才存在的模块痕迹。

### 证据 6：时间线合理

构建于 2013-01-14 21:26（UTC+?），零售发售 2013-03-05——
**早于发售 7 周**，正是 release 分支（RL）候选构建的时间窗；
"Dev Beta" 与 `Release Configuration` 组合符合 EA 当时 beta 候选的命名。

### 判定结论

**这是一份真实的 SimCity 2013 发售前 RL（release）分支构建，
版本 253660365 / CL 707103 / 2013-01-14，附匹配 PDB。**
不是零售版的重新打包：零售属 PL 分支（CL 839109），两者分支路径、
GUID、符号、代码内容全部不同（详见 diff 篇）。

## 三、各文件可用性评级

| 文件 | 评级 | 用途 |
|---|---|---|
| `SimCity.pdb` | ★★★★★ | RL 分支全部符号/类型/源码路径；可经 DIA/pdbparse 提取全局变量名↔地址映射、结构体布局，终结 DAT_ 无名时代 |
| `SimCity.unwrapped.exe.c` | ★★★★★ | 1.9M 行带名反编译（39,321 个函数体 / 17,346 个限定名 / 1,462 个类），即查即用 |
| `SimCity.exe` + `unwrapped.exe` | ★★★★ | RL 分支二进制本体：常量直读、frida 对拍对照组 |
| `version_app.txt` / message*.txt | ★★★ | 身份证据；message 内容已包含于大 .c |
| `EAWebkit.pdb` | ★★ | 仅对 UI WebKit 层（内嵌浏览器渲染）有用 |
| `DRM encryption key.txt` / `SimCity.par` | ★ | 历史价值；par 可用密钥解密但无新信息预期 |
| `Core/`（Qt 激活栈） | ☆ | 与逆向目标无关 |

## 四、方法备注

- PE/RSDS/PDB 解析脚本：`tmp/dev-sample-analysis/pe_compare.py`、
  `pdb_guid.py`（自包含，可复跑）。
- PDB 字符串提取与符号清单：`tmp/dev-sample-analysis/extract_inventory.py`，
  产物在 `tmp/dev-sample-analysis/out/`。
