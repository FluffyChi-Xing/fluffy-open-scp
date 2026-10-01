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

- **frida 17 已删除 `Memory.readByteArray`**：调用抛 `TypeError: not a
  function`，被 catch 吞掉 = 扫描假零（09-30 三连扫描空手根因）。替代：
  `addr.readByteArray(len)` 指针方法 / `Memory.scanSync` 抽样（注意页释放
  竞态）/ 外部 `ReadProcessMemory`（`external_device_scan.py`）。

- **D3D9 COM 对象的 vftable 实际住在 rdata（只读数据段），方法指针才指向 exec**。
  按「vftable 地址 ∈ exec」筛候选会把真设备拒掉、反而放进导入跳转表假候选
  （2026-09-30 三连会话零捕获+烧 CPU 的根因，已修：inExec→inMod 放宽 +
  lurk 确认设备即置 deviceFound 抑制 fallbackProbe + 校准槽位 83→78 补 create@80）。
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

## 渲染上下文捕获（2026-10-01，capture_render_context.js + capture_states.py）

目标：decal Tier-3 缺口——编译后最终 shader（=引擎片段链组合的产物，
组合表的终局替代）+ 每族渲染态（blend/depth/cull）+ 常量实值
（decalMaterialData/矩阵/texXform）+ **decal pass 标记**。

```powershell
# attach（游戏进城、镜头对准招牌街后）：
python tools/dynamic/capture_states.py
# 回车 = 开 20s 捕获窗口；数字回车 = 自定义时长；q 回车 = 退出
# 随行：--wait；spawn：--spawn "<exe>"
```

机制与防崩要点：
- 设备定位/字节码落盘 = hook_d3d9.js v7 实测机制的精简移植（lurk +
  15s 巡视器 + .data 行走/全堆扫描兜底）；bind hook 首见对象经
  GetFunction 版本 token 校验后落盘（顶点声明同形物已滤）。
- **新增签名分类**：v7 被动计数器扩展 rs/rsPure/cf/draw 四类签名，
  巡视器按「纯度优先、频次其次」择优武装有源 hook——破解包装层槽位
  可能移位，不猜槽位。
- **decal pass 标记**：静态 hook VA 0x6FD730（Ghidra SC_cVolumeDecalManager
  FUN_006fd730 绘制分发，无 ASLR 直连）。窗口内每条 draw/常量事件带
  decalActive；离线按 ps/vs obj 串 join shader_manifest 即得每族状态剖面。
- 有源 hook 预算：RS×1 / 常量×2 / 绘制×1 / 帧×1 / bind 身份跟踪 +
  静态 decal 分发 1 处。常量去重用便宜签名（首尾 u32）防烧 CPU；
  RS 影子表只记变更；事件仅捕获窗口内发送。
- 建议独立运行（与 dump_shaders.py 并行时 bind 字节码会重复落盘）。
- 离线分析：render_context.jsonl 的 hex 字段 = 16×count 字节常量
  （float32 LE 数组）；start 即引擎常量寄存器号——decalMaterialData
  的寄存器位置由 disasm 对照（d3dcompiler 反汇编在 dump_shaders 管线）。

### 崩溃教训（2026-10-01 首跑，capture_render_context.js v2 修复）

attach 模式首跑致游戏崩溃：堆扫描产出 ~20 个垃圾候选（0x6b50xxxx 堆地址
侥幸过 spotCheck）→ 全部做 111 槽 bind sweep → **对垃圾 COM 对象调
GetFunction → illegal instruction**（README 铁律 2 的活案例）。三处修复：
1. bind sweep 限定强校验候选（validateReport ok）且上限 4 个；
2. GetFunction 前强制 quickComObj（对象自身 vtable 24 槽 exec≥80%）；
3. 候选进 watchDevice 监视也限强校验。
**attach 模式的正信号**：静态 decal 分发 hook（VA 0x6FD730）实锤命中
（decalOn/Off 成对、396 事件）——decal pass 标记机制验证通过。
**推荐流程**：脚本 `--wait` 先挂 → 再启动游戏（lurk 随行观察设备创建，
完全绕开堆扫描路径）。

### d3d9 内部层勘误与双层同录（2026-10-01 反编译追查）

会话捕获的"设备 0x6b501000"= 系统 d3d9.dll 静态 vftable（RVA 0x1000，
文件内 128 指针全部指向 d3d9 代码——已验证）——**是 d3d9 内部 COM 对象
而非 RW4 抽象层**；slot 71 实现带 `state<0x100` 校验，疑似内部状态编号
（含旧 D3DRENDERSTATE 语义，0..6 合法）。捕获的 RS[10]/[11]/[14] 值域
{5,6,7}/{1,2,5}/{1,2,5} 与 D3DBLEND（5=SRCALPHA,6=INVSRCALPHA,1=ZERO,
2=ONE）高度吻合但交错配对有伪影，静态定谳收益递减。**已改双层同录**：
lurk 抓到真 D3D9 设备后额外武装其公共 slot 54（原生 SetRenderState），
事件类型 `d3drs`——离线按时间对齐包装层 rs 事件即得完整翻译表，无需
人工读反编译。
