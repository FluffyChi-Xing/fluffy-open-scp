# OpenSCP 逆向技术博客

面向 clone / star / fork 本仓库的读者，讲解 SimCity（2013）与其 GlassBox
引擎的已确认机制。三篇只陈述事实（反编译 / 真实包探针 / 原版对拍定谳的结论）。

| 篇 | 文档 | 内容 |
|---|---|---|
| 1 | [SimCity 游戏整体架构](./simcity-game-architecture.md) | 磁盘布局与包分工、DBPF/RW4/Property 格式、s3db 描述符库、引擎本体在 SimCity.exe 的证据链、脱壳流程（x32dbg+Scylla）、Ghidra 工作台、Frida 动态探针、shader 源码提取、mod 生态规则 |
| 2 | [GlassBox 引擎整体架构](./glassbox-engine-architecture.md) | GB/SC/SP 命名空间分层、子系统注册表与三阶段更新、客户端/服务器分层、property 驱动行为、Eco/Transport/Zoning/Terrain/Disaster 五大子系统、ER2 规则与存档、D3D9 渲染管线与光照、区域地图 |
| 3 | [贴花（Decal）渲染管线](./decal-rendering-pipeline.md) | 贴花字典与 lot 单元字段、投影盒几何（scale=半高、最近命中锚定、U 镜像）、四通道语义（编辑器预览 vs 渲染真相）、引擎批绘机制、前端材质分派与兜底、对拍时间线 |
| 4 | [Lot 地表渲染](./raster-lot-rendering.md) | LotMask 的引擎语义（材质分区选择器）、generic_lot 像素着色器完整算法（阈值 one-hot + 8 级瀑布 + 平色 + 底图格）、cLotInstanceInfo 与 CPU 侧单 quad 构建、朝向约定（180° 旋转 + U 镜像）、通道配对与优先级归属、离线合成校准与证伪记录 |
