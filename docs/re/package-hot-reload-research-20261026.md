# SimCity package 热重载研究与实现边界

日期：2026-10-26

## 结论

对普通 `.package` 资源，当前零售版 SimCity 没有可验证的公开热重载入口。游戏在启动阶段执行 `RegisterPackages -> AddPackageIfValid -> ResourceMan::RegisterDatabase`，随后资源会进入 ResourceMan、PropertyManager、LRU/工厂对象和渲染对象的缓存链。运行中替换磁盘文件只改变下一次打开文件时看到的字节，不会自动重建这些对象。

因此，稳定方案应把“热重载”拆成两层：

1. **工具内即时预览**：OpenSCP 自己重新读取 package、清理前端/后端缓存并刷新预览，用于资产和 mod 的快速迭代。
2. **游戏内验证**：构建隔离的 overlay，原子部署后通过可控的游戏生命周期重新载入。第一阶段采用“保持编辑器进程、自动重启游戏进程并恢复验证上下文”，不注入游戏内部函数。

这能避免退出 OpenSCP，也不会把未确认的引擎内部调用伪装成稳定能力。

## 代码与运行时证据

- `src-tauri/src/game_launch.rs` 的 `deploy_game_mod` 和 `deploy_ui_overlay` 已使用临时文件后 rename 的原子替换，并能报告 `SimCity.exe` 是否运行；它们目前没有触发游戏侧重新注册。
- 同文件的 `launch_game` 只负责启动已有或新的进程，没有游戏会话/城市恢复协议。
- `src-tauri/src/game_assets.rs` 使用进程级 `INDEX` 缓存。`cached_index` 仅在第一次访问时构建；package 文件变化后不会失效。该缓存必须增加基于目录快照或显式 invalidate 的策略，否则工具内预览也会读旧索引。
- `src-tauri/src/package_service.rs` 的 `PackageManager` 持有已打开 `Arc<Package>`、registry 和 locale cache。替换同一路径文件不会更新现有 package handle；必须 close/reopen 或新建 generation。
- `docs/re/package-runtime-verification-20261017.md` 的实机结果证明普通用户 `Packages` 包会被注册，但同 TGI 可能仍由先注册的 `SimCityData` 包命中。部署成功、注册成功和最终资源命中是三个不同状态。
- `docs/re/package-directory-chain-evidence.md` 记录普通包 priority/注册顺序；文件名前缀不能把 `UserData/Packages` 提升到基础 Data 之前。
- `docs/re/package-ecogame-resolution-closure.md` 记录 EcoGame 的特殊路径：`SwitchToScriptsVersion` 有 retail 的 force-like 参数，可绕过 current-version 短路；但已构建脚本按 game/version/add-on 键缓存，外部引用可能阻止淘汰，不能据此承诺完整热重载。

## 不建议的路线

直接向运行中的 `SimCity.exe` 注入并调用 `SwitchToScriptsVersion(force=true)`、`ShutdownGameVersions` 或 `FlushAllCaches` 不适合作为第一版产品能力：

- 地址、调用约定和对象布局只对已验证构建有效，补丁或不同发行版可能失效。
- force 参数只证明绕过一个短路，不等于所有 ResourceMan、Property、脚本、材质和渲染对象都被重建。
- 在城市运行中卸载数据库可能留下外部引用，造成悬空对象、崩溃或半刷新状态。
- 当前 observer 是只读采集器，不是生命周期控制器；不能把观测 hook 当作重载 API。

## 推荐的稳定实现

### A. generation 化的工具内重载

为 package 引入 `generation`：路径、文件大小、修改时间和 SHA-256 组成快照。打开/读取前检查快照；发生变化时：

1. 先完成当前读取任务并停止提交旧 generation 的结果。
2. 清理 `PackageManager` 中该路径的 package、registry 和 locale cache。
3. 重新打开 package、重建索引，生成新的 package id/generation。
4. 让前端丢弃旧 generation 的异步响应，再刷新当前资源和预览。

`game_assets::INDEX` 也应从单例 `Option<AssetIndex>` 改为带 `game_data` 快照的缓存，并提供 `invalidate_game_asset_index`。部署命令成功后由前端调用 invalidate，再重新读取预览。

### B. 游戏验证会话

新增一个“验证会话”状态机，而不是声称任意运行中资源都能刷新：

`idle -> staging -> deployed -> restarting -> ready -> observing -> stopped/failed`

每次构建生成唯一 artifact，写入临时 staging 目录；校验 DBPF、资源依赖和目标安装位置后，再原子替换固定文件。若检测到游戏运行：

1. 保存会话元数据（artifact hash、目标包、目标 TGI、安装目录、游戏 PID、observer profile）。
2. 请求游戏正常退出，等待 PID 消失；超时则报告失败，不强杀。
3. 启动游戏，等待新 PID 出现并确认窗口响应。
4. 可选启动现有 observer，采集注册/命中证据。
5. 将结果标为 `deployed`、`registered`、`hit` 或 `behavior_verified`，禁止把前一状态自动升级成后一状态。

这仍然需要游戏重启，但不需要退出 OpenSCP，也能自动回到同一验证流程；对资产/mod 测试的迭代时间已经从“手动退出、重新定位、重新采集”降到一次按钮操作。

### C. 真正的游戏内热重载作为实验能力

后续若要继续探索，可单独建立构建指纹矩阵和 opt-in 实验命令：仅在匹配的 `SimCity.exe` SHA-256、RVA、入口字节和 ABI 时尝试 force-like 脚本切换；切换前后记录版本表、挂接 priority、缓存命中和对象初始化事件。实验失败或证据不完整时自动回退到会话重启，不影响默认流程。

## 验收标准

每次迭代至少保留：

- artifact 与目标包的前后 SHA-256；
- DBPF 结构/资源依赖校验；
- 游戏 PID 生命周期；
- package 注册记录；
- 指定 TGI 的最终命中源；
- 资源语义或画面行为验证结果。

只有最后一项通过，才显示“游戏内效果已验证”。否则分别显示“已部署”“已注册”或“命中源未确认”。

## 当前优先级

1. 先实现 generation 化缓存失效，保证 OpenSCP 自身预览不陈旧。
2. 把现有部署/启动命令组合成验证会话，加入正常退出、重启、observer 和结果分级。
3. 用小型单 TGI overlay 做自动化回归，覆盖 Data 与 UserData/Packages 的实际命中差异。
4. 最后再评估特定构建上的 native force-reload 实验；它不进入默认稳定路径。
