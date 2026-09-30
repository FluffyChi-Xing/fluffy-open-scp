# Runtime 运行时抓取作战状态追踪（2026-09-30）

> 目标：对 SimCity (2013) 运行时做一次性全量抓取，回答所有悬而未决的渲染/对象
> 问题（A-G 清单 + 还原区 R 项）。本文档为**入库追踪表**；原始作战计划与抓取
> 产物在 `tmp/CAPTURE_PLAN.md` 与 `tmp/dynamic/`（本地，不入库）。
> 调试全档案见 `docs/roadmap/migration.md` §66（本地）。

## 一、状态总览

| 类别 | 完成 | 部分 | 无进展 |
|---|---|---|---|
| R 区（还原/修复） | R1、R3 | — | R2、R4、R5 |
| N 区（新主题） | N8（静态破译） | N7 | N1、N2、N3、N4、N5、N6 |
| 管线/工具 | 读原语修复、单会话锁、多设备监视、自愈校准、并集扫描、外部扫描器 | — | 进程内 frida 稳定性（三崩实证） |

## 二、R 区：还原/修复项

| 编号 | 内容 | 状态 | 产出/位置 |
|---|---|---|---|
| R1 | 18 族 decal shader 源码 + 族→VS/PS 路由映射 | ✅ 完成 | 容器格式破译（长度前缀字符串流）+ `tmp/parse_container_tokens.py`；88 vftable 全景 / 6636 块 → `tmp/dynamic/decal_all_families_source.txt`；**族清单：decalProject 系 10 变体=投影上墙，浮空仅 decalFloatQuad/NeonTubeSDF/InteriorMap** |
| R2 | 变体对象 graf/sign + ctx/sd shaderdef 对象 | ⏸ 降级 | §65.10 源码路径已关闭五问；且进程内堆读原语待修（frida17 `Memory.readByteArray` 已删，见 §66.3） |
| R3 | decal atlas 全量缩略图 + 字典普查 | ✅ 完成 | 1648 条目 + `tmp/decal_scan/all_entries_sheet.png`（69×24）；普查产出：全库仅 3 字典（sign `aa8b7058`/graffiti `eefd390c`/hole `1813da18`=INDUSTRIAL LABS 所在，字典在 Game 包、缺的是 raster） |
| R4 | decal_composite 四混合假设对比板 | ☐ 未跑 | 工具就绪：`decal_composite.rs` |
| R5 | parse_variants.py v3 重写 | ☐ 未跑 | 随 R2 降级 |

## 三、N 区：新主题

| 编号 | 问题 | 状态 | 依赖/说明 |
|---|---|---|---|
| N1 | Lot↔模型对齐关系 | ☐ 无进展 | 需渲染设备定位后 hook world 矩阵 |
| N2 | Prop 是什么/怎么渲染 | ☐ 无进展 | 需 D3D 管线（draw/纹理采样） |
| N3 | Effect 是什么/能否模拟 | ☐ 无进展 | 同上 |
| N4 | 生成器（spawner） | ☐ 无进展 | 静态普查可先行（不依赖游戏） |
| N5 | 载具怎么渲染 | ☐ 无进展 | 需 D3D 管线（连续帧矩阵序列） |
| N6 | 表面递进/伪高精度（simcity_material.pdf） | ☐ 无进展 | 需 D3D 管线（zoom 级 draw 序列对比） |
| N7 | DLC0 广告路由实证 | ◐ 静态完成 | 全库仅 3 字典、DLC0 共用 sign 字典 `aa8b7058`；三 material 的 RW4 不在 5 主包（shader-def 运行时组装）——**运行时族判定待管线** |
| N8 | 废弃建筑 mesh 歪斜（引擎形变） | ◐ **静态破译完成** | 形变=顶点着色器 `deformAbandonedVS`：沿 4 片斜切平面（kPlanes，全部朝 +Z 带 0.2 倾角）取最小距离，凹陷部分沿 kMove(0,0,1) 推移 99%；配套 deformRubbleVS（instanceColor 2.2 次幂瓦砾重着色）+ isAbandoned 开关。**数学完整提取（tmp/dynamic/N8_*.hlsl），可逐字实现进 OpenSCP 渲染器；待定：isAbandoned 触发源（存档状态位）** |
| N9 | 半边窗偶发复发 | ☐ 无进展 | §65 已修主因；偶发机制未知，需复现+参数表 dump |
| N10 | frida 读原语/稳定性 | ◐ 部分完成 | `Memory.readByteArray` 已删实锤+修复（指针方法）；进程内堆扫描仍非确定性失明+三崩——**外部 ReadProcessMemory 路线已建成**（`external_device_scan.py`，实测稳定） |

## 四、今日新增判定（证据已入 §66）

1. **族清单铁证**：decalProject 系 10 变体（含 Neon/NeonSDF/SDFLit）=投影上墙；
   浮空分支仅 decalFloatQuad / decalNeonTubeSDF / decalInteriorMap。
2. **字典全景**：DLC0 无自有字典，广告共用 sign 字典；加载期接口
   （`0x6c2b13dc/14ec`）进城后已释放——**进城钩子全静默之谜告破**。
3. **`0x6c2b1490` = 呈现/资源复合设备**（Present 帧率但零状态调用），非渲染执行者。
4. **读原语**：frida17 `Memory.readByteArray` 已删（TypeError 被 catch 吞=扫描假零）；
   替代=指针方法/scanSync/外部 ReadProcessMemory（`external_device_scan.py` 实测稳定）。

## 五、工具修复资产（09-30，均已提交）

| 提交 | 内容 |
|---|---|
| `48a8fab` | 密度接纳（rdata/堆构 vtable 不再误杀）+ deviceFound 漏设 |
| `4e112e9` | 校准自愈（create 空窗 30s 重试）+ 槽位补 80 |
| `83246bc` | 多设备永久监视 |
| `a187e76` | 全槽交通图 |
| `3755137` | bind 武装与 create 解耦 + hookBind 堆构兜底 |
| `5c797c2` | 单会话锁（k32/路径两连修） |
| `6a65db7` | fallbackProbe 并集策略（行走+全堆） |
| `289aa03` | bind 预过滤改结构检查 |
| `ad7ede5`/`322a683` | 密度设备猎手 + 读原语对照测试工具 |
| `8c60952` | 外部 ReadProcessMemory 扫描器 |

## 六、下次会话执行序（一次周期收尾）

1. 游戏关 → `dump_shaders.py --wait` 待机（出生跟随父+子，capture.lock 单会话锁已机制化）；
2. 用户启动 → 加载期计数器跟随（create 流量充裕窗口，自愈校准覆盖长加载）；
3. 进城即已武装（**城内不再新增钩子**）→ 验证落盘 → 飞 A-G 清单
   （A 广告 / B 涂鸦 / C 变焦 / D 水 / **E 废弃歪斜楼** / **F 人群** / **G 船只** + 半边窗复现）；
4. 绑定流 vs decalProject/decalFloatQuad 本体比对 → 族判定 → 进入修复环节。

> **铁律**：城内不再新增任何钩子（三次崩溃实证）；活钩 ≤6；窗口 ≤10s；即时写盘。
