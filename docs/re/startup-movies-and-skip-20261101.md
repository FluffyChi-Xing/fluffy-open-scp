# 启动视频（Maxis_Logo / Maxis_Intro / EP1_Intro）加载机制与跳过方案

日期：2026-11-01。样本：离线可玩版 `D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow`（`SimCity.exe` 10,971,552 B，PDB/IDA 证据取自 dev 版 `Simcity Development Files Including PDB Debug files`）。所有 TGI 均为 `type:group:instance`（16 进制）。

## 0. 结论速览

1. 三段"视频"其实都是 **同一类资源 `0x376840D7`（RW4 RWMovie + On2 VP6 容器）**，共 3 支，其中 2 支在 `SimCity_App.package`，1 支在语言包 `Locale\<lang>\Data.package`。
2. 运行时**只有一个触发点**：`SimCity_App.package` 里的 GameEntry 脚本包 `67771F5C:40464200:696447EA` 中 `simcity.cCurtainUIHandler`，`Show()` → `PlayIntroMovie()` → `scrui.PlayMovie(id,false)`；EP1 已装则播 `EP1_Intro`，否则播 `Maxis_Intro`。
3. `Maxis_Logo`（5.4 MB / 368 帧）**在当前版本没有任何引用**：exe 无 `Logo` 相关函数或字符串，全部 package 与脚本里也没有对它的调用。它不是被"跳过"的对象，而是被裁掉的素材；画面上先看到的 Maxis 标志是 `Maxis_Intro` 片头本身。
4. 之前那份 `SimCity_SkipIntroMovie_v2.package` 的**补丁内容是对的**（与零售 JS 逐字节比对可复现），但它**永远不会生效**：文件名排在 `SimCity_App.package` 之后，而同 priority 下**先注册者获胜**。
5. 可用方案：把同一补丁放进**排在 `SimCity_App.package` 之前**的包（`000_...`）。已生成并校验：`tmp/startup-video/out/000_SkipIntroMovie.package`（861,686 B，单 TGI，sha256 `8993bad7…c176`）。
6. `EP1_Intro` 位于 `Locale\en-us\Data.package`，该数据库 **priority=1**，高于 `SimCityData` 的 0，因此**用 SimCityData 里的包替换该视频资源是不可能赢的**；只能从脚本层（不调用 PlayMovie）关闭，本方案正是如此。

## 1. 视频资源清单（实测 TOC）

| 名称（IDFromName） | TGI | 所在包 | 大小(B) | 容器头 | 帧数 |
|---|---|---|---|---|---|
| `Maxis_Intro` → `F1271E5A` | `376840D7:00000000:F1271E5A` | `SimCityData\SimCity_App.package` | 136,981,268 | `MVhd`/`vp60`/`SCHl8`/`GSTR` | 0x7BD=1981（≈66.0 s@30） |
| `Maxis_Logo` → `A4299411` | `376840D7:00000000:A4299411` | `SimCityData\SimCity_App.package` | 5,659,912 | 同上 | 0x170=368（≈12.3 s@30） |
| `EP1_Intro` → `55E5565C` | `376840D7:00000000:55E5565C` | `SimCityData\Locale\en-us\Data.package` | 90,944,240 | 同上 | — |
| `EP1_Intro`（繁中） | 同上 | `…\Locale\zh-tw\Data.package` | 90,987,616 | 同上 | — |

`MVhd` 体（32 B）：`"vp60"` + `00 05` + `d0 02`(=720) + 帧数 + … + `b3 92 04 00` + `10 27 00 00`(=10000)。两支视频都是 **未压缩存储**（`comp=0`），这不是巧合：`SP::cMovieSystem::PlayMovie` 里有 `recordInfo.mnCompressionId == 0` 的硬性分支，压缩存放的视频会被直接拒绝播放。

另外两处同名资源不是视频：`2F7D0004:0:A4299411`（App、26,471 B）是 273×187 的 PNG（`Graphics/GameEntry/Hub_Graphics/Maxis_Logo.png`），`2F7D0004:0:A4299411` 在 `SimCity_Updater.package` 也有一份。

## 2. 名称 → ID：`scrui.IDFromName` 不是 `SPIDFromName`

脚本侧（`67771F5C:40464200:7D14BE70`，即 `scrui` 绑定层）原文：

```js
scrui.IDFromName = function(a){
  for (var a = a.toLowerCase(), b = 2166136261, c = 0; c < a.length; ++c)
    b = 403*(b&4095) + (403*(b>>>12&4095) + 0*(b&4095) << 12)
                     + (403*(b>>>24&4095) + 0*(b>>>12&4095) + 1*(b&4095) << 24),
    b ^= a.charCodeAt(c) & 255;
  if (0 > b) b += 4294967296;
  return b;
};
```

它是 FNV-1（prime 0x193=403、基 0x811C9DC5、逐字节转小写），但用 **12 位分肢**在 double 里做乘法，每肢进位被截断，所以和"教科书 32 位 FNV-1"的结果不同。

**修正（2026-11-01 追加）**：引擎的 `SPIDFromName` 就是 `EA::StdC::FNV1_String8(s, 0x811C9DC5, kCharCaseLower)`，实测与上面这段 JS **完全一致**——全游戏只有一套名字哈希（属性 ID / 单位 ID / 资源实例 ID / 命令 token 同源）。最早用 32 位正确进位算出的"不同"只是被教科书实现带偏。决定性交叉校验是 `ExecuteCamera` 的 token 常量（见 §8）：`move=B37B55B2`、`orient=9F54EE66`、`lookat=F797D34B`、`immediate=12167AC8`、`zoom=32983260` 逐个命中，`play=4AE0223F`、`stop=2F056C5D` 命中 `ExecuteVideoCommand`，`OptionEnableAutosave`/`OptionDisableDisasters` 命中 `Properties.txt` 的 `(hash(Name))`。

校验（`tmp/startup-video/idname.py` 为精确移植）：

| 名字 | IDFromName | 说明 |
|---|---|---|
| `Maxis_Intro` | `F1271E5A` | = 130 MB 那支的 instance ✔ |
| `Maxis_Logo` | `A4299411` | = 5.4 MB 那支的 instance ✔ |
| `EP1_Intro` | `55E5565C` | = 语言包那支的 instance ✔ |
| `play` | `4AE0223F` | = 原生 `ExecuteVideoCommand` 里的常量 1256202815 ✔ |
| `SkipIntroMovie` | `FB901AA2` | = 脚本里的 `simcity.kSkipIntroMovie` ✔ |
| `OptionEnableAutosave` / `OptionDisableDisasters` | 与 `Properties.txt` 的 `(hash(Name))` 一致 ✔ |

也就是说：**属性 ID、工具/单位 ID、资源实例 ID 用的是同一套分肢 FNV-1**；老的 `OptionVideoRes` 等是手写常量，不参与这套。

注意 `IDFromName("EP1")==0x3D932B6B`，正好是 `SimCityDataEP1.package` 里脚本 `67771F5C:00000000:3D932B6B` 的 instance —— 脚本文件也是按这个哈希命名的，可用来反查文件名。

## 3. 播放链路（静态闭合）

```
cLauncher3D.Activate → CurtainController.Activate → gCurtainUIHandler.Show()
   └─ if (mSkipIntroMovie==true) SetSkipMovie(true)
   └─ if (mSkipMovie false/"false")  → PlayIntroMovie(); SetSkipMovie(true)
                                      else → OnIntroMovieStop()

PlayIntroMovie()  // 67771F5C:40464200:696447EA
   b = IDFromName("Maxis_Intro")   c = IDFromName("EP1_Intro")
   if (gGlobalUIHandler.mHasEp==1 || cLauncher3D.mHasEPO==1)
        scrui.PlayMovie(c, false)   // EP1 已装
   else scrui.PlayMovie(b, false)

scrui.PlayMovie = function(a,b){ b=b===undefined?true:b;
   var c = scrui.RequestStringFromKey({instance:a, type: b?69081534:929579223});
   scrui.PostGameCommand(scrui.BuildCommand(["video","play",c])); };

RequestStringFromKey -> "group,instance,type"（group 缺省写 "0"）
   ↓ PostGameCommand → 原生 UI 命令分发
ExecuteVideoCommand            // SimCity.exe，dev VA 0x875D50
   Strtok("/") → 第二段哈希 == IDFromName("stop")/("play")
   SC::KeyFromURLString(&key, "g,i,t")     // dev VA 0x6A8BF0，逗号分隔十进制
   SP::cMovieSystem::PlayMovie(&key, &info) // dev VA 0x59E720
       ResourceManager::FindDatabase(key) → 记录必须未压缩
       rw::movie::MoviePlayer2 + VideoDecoder_Vp6 + AudioDecoder_EALayer3 + AudioRenderer_Wwise
       QueuePlaybackRequest(path) → 阻塞到 GetNextFrameRequestId()!=0
       SetLayer(11)，按屏幕比例居中
   ↓ 播完/点击/按键
kMessageStoppedMovie → cCurtainUIHandler.HandleGameEvent → OnIntroMovieStop()
   → ShowPreloadingScreen() → mOnIntroMovieStopCallback(…) → cLauncher3D.InitiatePreload()
```

`PlayMovie` 的 type 常量：`false → 929579223 = 0x376840D7`（唯一有资源的视频类型）；`true → 69081534 = 0x041E19BE`，全安装 0 条资源，所以 `scrui.PlayMovie(id)` 永远播不出东西。

## 4. 三个"跳过"开关

| 开关 | 来源 | 能否离线生效 |
|---|---|---|
| `cLauncher3D.mSkipIntroMovie` ← app property `SkipIntroMovie`(`FB901AA2`) | 由 `["appproperties", 0xFB901AA2]` 走在线/服务端查询，返回值第 [4] 项 | ❌ 离线恒为 false；`Properties.txt` 未声明该属性，本地 `Preferences.prop` 是否被这条查询读取**未验证** |
| `gSessionManager.mSkipMovie` | 持久化在 `SCC` cookie 的 `skipMovie` 字段，且 `LoadPage()`/`SetSkipMovie()` 每次回写 | ⚠️ 首启时内存值是 false，页面加载会把它写回，预置 cookie 不可靠 |
| 脚本 `Show()` 直接跳过 | 本方案 | ✅ 与零售"已看过一次"的路径完全一致 |

顺带：`ep1IntroViewed`（localStorage/cookie）**只影响能否立刻点掉**（`SetupListeners` 的 10 ms 时机），**不会**阻止播放。

## 5. 为什么 `SimCity_SkipIntroMovie_v2.package` 一直无效

它是两条资源：

| TGI | 大小 | 说明 |
|---|---|---|
| `00B1B104:00000000:08EA246B` | 11,844 | App 应用描述符的另一份（与零售版在偏移 2751 之后整体不同，属**其他构建**的描述符，风险项，与跳过无关） |
| `67771F5C:40464200:696447EA` | 861,554 | 已打补丁的 GameEntry 脚本包 |

补丁本身完全正确——把零售的

```js
Show=function(){!0==simcity.cLauncher3D.mSkipIntroMovie&&simcity.gSessionManager.SetSkipMovie(!0);
var a=simcity.gSessionManager.mSkipMovie;
!1==a||"false"==a?(this.PlayIntroMovie(),simcity.gSessionManager.SetSkipMovie(!0)):this.OnIntroMovieStop()};
```

替换为 `Show=function(){this.OnIntroMovieStop()};`（283 B → 77 B，正好 −206 B；用零售包重建后与该包 payload **逐字节相同**，已复现）。

失效原因是**加载顺序**：

- `SP::SetupResources` 以 priority 0 注册 `SimCityData\*.package`；`InternalDatabaseList::Add` 按 priority 降序稳定插入，`FindDatabase` 取**首个命中**，即**同 priority 先注册者获胜**。
- 注册顺序 = `FindFirstFileW/FindNextFileW` 枚举顺序，此卷上表现为按名排序；运行时采样（`docs/re/package-runtime-verification-20261017.md` §1）为 `Oppie_TrainStation, SimCityDataEP1, SimCity_App, …`。
- `SimCity_App.package` < `SimCity_SkipIntroMovie_v2.package`（`'_A'` < `'_S'`），所以**零售包先注册**，补丁包只是队列里第二个候选，永远轮不到。
- 同一份文档 §2 已用实机证明：同一 TGI 的最小包放进 `UserData\Packages` **输给** App；放进 `SimCityData` 且名字带 `000_` 前缀则**赢**（`最终选择 seq 63 测试包`）。

## 6. 交付物

`tmp/startup-video/out/000_SkipIntroMovie.package`

- 单条资源 `67771F5C:40464200:696447EA`，861,554 B，未压缩，`size|0x80000000` 记录布局（与 `crates/dbpf::write_uncompressed_overlay` 一致，该形状已有实机命中证据）。
- 包体 861,686 B，sha256 `8993bad71a82c53245b59c4c663156d76ca68c41868617436b58fb153471c176`。
- 补丁后 `PlayIntroMovie` 无调用点（`this.PlayIntroMovie()` = 0 处），`OnIntroMovieStop` 链完整（`ShowPreloadingScreen` + `mOnIntroMovieStopCallback` → `InitiatePreload`）。

部署：把该文件复制到 `…\SimCityData\`（不要放 `SimCityUserData\Packages`），重启游戏。回滚：删该文件。

建议同时删除旧的 `SimCityData\SimCity_SkipIntroMovie_v2.package`（它既无效，又带一份非本构建的应用描述符）。

重建脚本：`tmp/startup-video/build_skip_pkg.py`（读零售包 → 断言补丁点唯一 → 断言无 `PlayIntroMovie()` 调用 → 写包 → 回读逐字节校验）。

## 6.5 实机验证（2026-11-01，用户执行）

部署 `000_SkipIntroMovie.package`（旧包已 `.disabled`）后重启：**直接进入菜单，没有任何视频**。

由此可确定两件事：

1. `SimCityData` 里 `000_` 前缀的单 TGI 覆盖**确实赢了** `SimCity_App.package`（否则 `Show()` 会走零售分支播出 `Maxis_Intro` 或 `EP1_Intro`），第 5 节的加载顺序推断得到实机闭环。
2. `Maxis_Logo` **没有独立的播放路径**：它不经过被补丁的 `Show()`，若存在原生触发点仍会照常播放。所以第 7 节第 1 条从"待 hook 确认"降级为"静态 + 实机一致：该素材未被使用"。

时间收益：省掉 `kMovieRunLengthMs = 66 s` 的片长，以及 130 MB / 91 MB 影片的读取、`MoviePlayer2` 首帧自旋（`GetNextFrameRequestId` 忙等）与音频渲染器初始化。

待多次复测确认项（都走 `OnIntroMovieStop()` 之后，理论上与零售"播完"路径完全一致）：

- 预加载回调是否照常触发：`OnIntroMovieStop → ShowPreloadingScreen() → mOnIntroMovieStopCallback(...) → cLauncher3D.InitiatePreload`（`SetIntroMovieStopCallback` 在 `ShowBlankScreen()` 之前注册，早于 `Show()`）。
- `FadeOut()` 的 `mLoadCount<2 && tutorialState` 分支（"Welcome Back" 动画）是否正常。
- EP1/COT 存档下是否同样直接进菜单（脚本层已统一，资源差异不再参与）。

## 7. 未闭合项

1. `Maxis_Logo` 视频的"零引用"结论是**静态**的（exe 无 `Logo` 函数/字符串；全部 1..N 个 package 的解压 payload 与 990 个 `0x67771F5C` 脚本均无 `Maxis_Logo`/`PlayMovie` 调用；`0x376840D7` 与三个 instance 常量在 dev/live exe 中均 0 命中）。若实机在跳过 intro 后仍能看到**独立**的 12 s 标志动画，那说明还有一条未捕获的触发路径，需要在 live 进程里 hook `ExecuteVideoCommand`(dev VA 0x875D50) 或 `cMovieSystem::PlayMovie`(dev VA 0x59E720) 记录 key 来定位。
2. `appproperties` 查询的本地来源未闭合：`"appproperties"` 在 dev/live exe 中均无字符串，说明它由 EcoGame 脚本侧处理，脚本包是编译/压缩格式（`0x08068AED` 等），尚未解出。因此"改 `Preferences.prop` 把 `SkipIntroMovie` 置 1"只是假设，未验证。
3. 视频容器 `MVhd/SCHl8/GSTR/MV0K/MV0F/SCDl` 的字段语义只解到"帧数/宽高"，未做完整 demux；若要"用 1 帧视频替换源视频"来达到更彻底的关闭，需要先完成这层解析。

## 8. 主菜单（Launcher3D）背景、摄像机与广告牌

### 8.1 背景不是一个动图，而是一个固定的 3D 场景 + 每个 UI 状态一个写死机位

`simcity.cLauncher3D`（`67771F5C:40464200:696447EA`）里 `mSceneStates` 定义了 8 个状态，每个状态只描述"开哪些面板 + 摄像机在哪 + 用哪个光照预设"：

| 状态 | cameraPos | orientation（度） | distance | 光照预设 | 音频 |
|---|---|---|---|---|---|
| GAME_ENTRY | 60,800,120 | -211,0,0 | 20 | front_end_hub | play_login |
| LOGIN | 60,800,620 | -211,0,0 | 20 | front_end_hub | play_login |
| **HUB_SCENE（主菜单）** | **60,800,120** | **-211,0,0** | 20 | front_end_hub | play_home |
| OPTIONS | 800,0,-41 | 2,0,0 | 20 | front_end_hub | play_options |
| PROFILE | 900,0,-170 | 90,0,0 | 20 | **front_end.prop** | play_profile |
| SIMCITYWORLD | 20,900,140 | -205,0,0 | 20 | front_end_hub | play_world |
| MY_GAMES | 60,650,110 | -212,0,0 | 20 | front_end_hub | play_mygames |
| LOADING | — | — | — | — | — |

`GotoSceneState()` 只是在切面板之后调用 `MoveCamera(false, cameraPos, orientation, distance)` 和 `MoveSun(skyPresetFile, 2)`。证据表明它是真 3D：机位是世界坐标（y=800 在城市上方）、面板切换时机位跳变、有实时车流/无人机/水面/阴影，且 `PROFILE` 会单独切到 `front_end.prop` 光照。

### 8.2 摄像机完全由一条 UI 命令驱动（不是引擎硬锁）

```js
MoveCamera:function(a,b,c,d){ … scrui.PostGameCommand(scrui.BuildCommand(
  ["camera","immediate",a,"move",RequestStringFromVector(b),"orient",RequestStringFromVector(c),d||this.mCurrentCameraDist])) }
MoveSun:function(a,b){ … BuildCommand(["light",a,b||0]) }
```

原生侧 `anonymous namespace::ExecuteCamera`（dev VA 0x877B35，`SimCity-dev-ida.c` 行 1139715）按 `/` 切 token、逐个 `FNV1_String8` 比对，语义如下（token 哈希与 `idname.py` 逐个吻合，见 §2）：

| token | 哈希 | 作用 | 目标属性（immediate=true / false） |
|---|---|---|---|
| `immediate` | 0x12167AC8 | 下一个 token 解析为 bool，决定下面用"瞬时"还是"平滑"变体 | — |
| `move` | 0xB37B55B2 | 位置 x,y,z | 16889143 / 16889140 |
| `orient` | 0x9F54EE66 | 欧拉角 x,y,z（原生转四元数） | 16897311 / 16897398 |
| `lookat` | 0xF797D34B | 注视点 x,y,z | 16889153（+flag） |
| `zoom` | 0x32983260 | 缩放 float | 16897093 / 16897223 |
| 未知 float | 0x4DA6B724 | 另一路 float 属性 | 243550274 / 172070377 |

注意：JS 追加的那个裸数字 `distance`（20）**不匹配任何 token，被原生忽略**；要改视角广窄得用 `zoom`。

因此"突破摄像机限制"不需要改 exe：
- 覆盖 `mSceneStates` 任一状态的 `cameraPos/orientation`（甚至新增一个 `HUB_SCENE2`）即可换机位；
- 或在脚本里持续发 `camera/immediate/false/move/…/orient/…`（配合 `gUIManager` 的鼠标/滚轮事件）做自由相机；
- `light <preset>` 换光照预设（`front_end_hub`、`front_end` 两个 84 B 资源：`00B1B104:40200100:0445A644` / `…:3FED2AA8`，内容是色温/雾/太阳角度一类的 float 表）。
- 场景本身是固定搭好的 hub 城（`00B1B104:40410100:6B6E2DE8` = 名为 `launcher3d` 的 228 B 配置），脚本里没有"加载任意城市当背景"的入口；想在完全不同的背景上做菜单，等于用全屏 UI 面板盖掉 3D 场景，而不是"换场景"。

### 8.3 COT 不改变背景场景，只换 UI 资源

`mHasEPO`/`mHasEp` 的分支只影响：`SimCity_Logo_COT.png` 与 LOGO_BG/LOGO_EP/LOGO_FT 的显隐、`EP1LoadingTips`/`EP1LoadingScreen` 提示表、COT 成就分类、以及 `PlayIntroMovie` 选哪支片。**8 个状态的 `skyPresetFile` 与 `cameraPos` 由 COT 与否完全无关**。截图里的未来风格塔楼来自 hub 场景自身（这些模型在基础包里就有）。

### 8.4 广告牌：3D 定位的 2D 面板，三层内容来源

- 面板：`mPanels.BILLBOARD = {baseControllerName:"simcity.gBillboardController", pos3d:[-25,655.5,80], xform:Matrix3FromAxisAngle([0,0,1],2.6)}`。
- 布局：`Layouts/Launcher3D/Billboard.js` = `67771F5C:0B074E5A:FEF1653E`（**仅 2,781 B**）：`promo_container(239820174)` → `deal_window(239715211)`（图 `Graphics/GameEntry/billboard-placeholder.png`）+ `outgoing_ad(251544765)`，外层金属框 `Graphics/Common/Graphics/Banner_Screen.png`，`ad_transition` 动画做 1 s 淡出。
- 控制器：`simcity.cBillboardController`（`67771F5C:40464200:3525E6EA`，148,664 B）。内容来源优先级：
  1. **广告服务**：`mAdwebUrl`（app property `AdWebServer`=0x0E792922）+ placement `simcityGameHubBannerAd` → 返回广告 JSON，取 `responseMediaURL`（远程 jpg），`PreloadImage("ad-N")`，按 `mAdwebRequestInterval`（默认 30 s）轮播；
  2. **兜底广告数组**：`Panel.mStateController.mConfigurationData.mBillboardAds`（服务端 `/configuration` 下发，字段 `image`/`clickAction`），来自脚本里写死的默认值——两条 `static.prod.simcity.com/Launcher-Default-Images/SimCity-FE_BannerAd_01.jpg`/`_02.jpg`；
  3. **无配置兜底**：`mAdWebRequestInterval=3E13`（等于永不请求）并 `SetWindowImage("maxisLogoBillboard.png")` → 包内 PNG `2F7D0004:00000000:59E2CF2D`（2500×1105 白底 Maxis 标志，已导出 `tmp/startup-video/out/maxisLogoBillboard.png`）。
- 所以截图里"先黑后出现促销图"就是这条链：面板先显示布局里的占位图（黑），拿到广告/兜底图后再 `SetWindowImage` 换上。

改法（从省事到彻底）：
1. **零代码**：覆盖 PNG `2F7D0004:0:59E2CF2D`（离线兜底），或覆盖 `Graphics/GameEntry/billboard-placeholder.png`（占位/首帧）；
2. **改布局**：覆盖 2.7 KB 的 `FEF1653E`，把 `deal_window.images` 指向自己的图、改尺寸/去掉外框；
3. **改脚本**：覆盖 `3525E6EA`，把 `mBillboardAds` 指到本地图（`image` 可以直接写资源名，如 `"Graphics/GameEntry/myad.png"`）并 `mAdwebEnabled=false`，或把 `SetWindowImage` 的目标换成自己的图并加点击动作。

三者都在 `SimCityData` 内用 `000_` 前缀包即可生效（同 §5 的优先级结论）。
