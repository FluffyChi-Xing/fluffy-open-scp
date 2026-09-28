# tools/dynamic — SimCity (2013) D3D9 动态抓取（Frida）

背景与结论见 `docs/roadmap/migration.md` §65。目标：从活进程中抓取
shader 字节码 / 常量 / 状态流，破解 shader-def（静态翻包原理性不可得）。

## 组成

- `hook_d3d9.js` — Frida agent。设备 vftable 定位（随行观察 / 全堆扫描 +
  多候选流量侦察，实测不猜槽位）+ shader 抓取（create 抓字节码 / bind 对
  在用 shader 对象调 GetFunction 导出，版本 token 内容自校验）。
- `dump_shaders.py` — 主机端：attach/spawn/--wait，落盘 `.bin` + 自动
  反汇编 `.asm`（d3dcompiler_47）+ `shader_manifest.jsonl` 清单。

## 用法

```powershell
# 前置：pip install frida frida-tools（需与游戏同机；64 位 frida 可注入 32 位游戏）

# A. 等待模式（推荐）：先挂上再启动游戏，随行抓设备创建（确定性最高）
python tools/dynamic/dump_shaders.py --wait

# B. attach：游戏进城市后挂
python tools/dynamic/dump_shaders.py

# C. spawn：从进程创建抓加载期全量 CreateShader
python tools/dynamic/dump_shaders.py --spawn "D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCity\SimCity.exe"
```

产物：`tmp/dynamic/shaders/*.bin|*.asm` + `tmp/dynamic/shader_manifest.jsonl`。

## 安全模式（破解版防崩规则）

`safe_attach.py` = 轻量观察模板（≤10 钩子、只计数、不调 NativeFunction）。
**防崩规则（实测教训）**：
1. 钩子数 ≤10——万级 sweep 已证实会崩游戏（装到 ~1600 崩）；
2. 禁止对未验证对象调 NativeFunction（GetFunction 访问违例会扰动游戏线程）；
3. 回调只计数，重活批量回传；
4. scanSync 只读安全，但放空闲期；
5. 破解版自身不稳定（菜单卡 5min/随机退出），崩溃先归因再重试。

## 已知事实（勿重复踩坑）

- 离线版 SimCity.exe 已脱壳、无 ASLR（基址 0x400000）→ Ghidra 地址 =
  运行时 VA 直连。
- frida 17：模块级 API 已删（用 `frida.get_local_device()`）；
  `enumerate_processes()` 漏 SimCity.exe（脚本已用 Toolhelp 兜底）。
- 运行中的进程无法再初始化 D3D（Direct3DCreate9 死锁）→ 自建设备仅限
  设备创建前；同进程反复 attach/detach 会留孤儿 agent，重启游戏解决。
- 旧实例不退会占单实例锁，新实例无窗口；残留进程需提权 taskkill。
- d3d9 头部 0x1000..0x4600 为导入跳转表（伪 vftable 毒区），设备类无 RTTI。
- 侦察线索：d3d9 头部 rdata 混合区多个 vftable 候选；`0x14ec@80` 曾出现
  episodic 的 shader blob 阵发（13 万次/15s）——v15 起自动常驻捕获。
