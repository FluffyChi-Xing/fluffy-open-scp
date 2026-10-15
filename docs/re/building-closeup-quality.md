# Property editor 建筑近景采样精度

2026-10-09。范围：building4 精细材质和 property editor 画布。

## 对照依据

本机 dev 源码根目录为 `D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree`。

- `src/SP/cRenderer/Update.c:49`：开启各向异性时设置 8 倍；`src/rw/graphics/GlobalState/D3D9SamplerStateDispatch.c:50` 把线性缩小采样升级为各向异性。当前前端已使用硬件最大值，无需禁用 mipmap 或改成 nearest。
- `src/anonymous/UploadTextureSizes.c`：上传各槽自己的宽高和倒数，不能假定所有图集分辨率相同。
- shader 字面公式来自已提取的 dev shader 容器，不在反编译 C 函数目录内。本地完整片段 `tmp/shaders/cpp_SimCity_App_g40212002_2.txt:19574`、`:19694`、`:19746`：building4 的 Base/Top 使用同一组 `uv_ddx/uv_ddy`；法线解码 `rgb * 2 - 0.9985`，`ApplyNormalMap` 用 `cross(N,T)` 再 `cross(B,N)` 建正交基。该文件约 18885 行的 impostor 变体有两组导数，不与 building4 混用。
- 后端 `package_service.rs` 从 `decode_top_mip_rgba` 导出原尺寸 PNG；未增加缩小步骤。两个现有样本 `1A7490A1`、`9401CB7A` 的法线图宽为 256/512，原资产细节仍是放大上限。

## 修改

1. 在未回绕 UV 上求导后乘当前区域缩放，避免最近邻材质参数表跨列跳变参与导数、选到过低 mip。这是对前端逐片元参数查表的边界修正；区域内部等价于源公式。
2. 窗户模糊反馈后的修正：Top 改用实际窗户 UV 的导数 `dFdx(vTopUv) * (1 + padding) * regionXform2.xy`，不再套用砖墙 Base 的采样足迹。显式包含 padding，避免近乎常数的窗户 UV 被放大数千倍后仍使用错误导数。求导在 fract/clamp 和离散参数采样之外。此项是近景预览增强，与源 building4 的共享导数公式有意不同。
3. 用各张图集的实际 `textureSize` 将坐标钳到区域内半 texel，支持负缩放和不足一个 texel 的区域。旧方案把整块 UV 压缩且边界只有四分之一 texel，改变细节相位并可能混入邻区。这里仅保护最高 mip 的边缘，不声称消除了较低 mip 内已经混合的图集内容。
4. 对插值切线重新正交化；无切线时从原始立面 UV 重建，而不是使用烘焙色图 UV。移除被随即覆盖的默认法线采样，保留既有 1.5 法线强度和材质外观配置。
5. 仅 property editor 请求最高 DPR 2；其它 ThreeViewer 调用仍默认 1.5。在 DPR≥2 时片元数量最多增加约 78%，换取避免低分辨率画布放大造成的细节损失。DPR≤1.5 不增加画布成本。
6. 风扇锯齿反馈后的修正：撤除 Top 的 sharp-bilinear 放大，Base/Top 改为有范围约束的 Catmull-Rom 双三次重建。硬化纹素会放大圆环/斜边的台阶；新方案连续插值 4×4 邻域，并把结果限制到中心 2×2 纹素的范围，防止颜色、调色板坐标、透明度过冲。按真实纹素足迹在 0.5～1 之间平滑混合回线性过滤，缩小时完全使用原 mipmap/各向异性路径。此项为编辑器增强，不能恢复原资产不存在的细节。
7. 2026-10-10 性能修订：仅 tint/coverage 使用有界双三次；法线/AO 和 shader map 恢复引擎线性/各向异性采样。双三次将中心正权重合并为双线性，4×4 重建变成 9 次采样，另用中心 4 texel 限幅，按足迹混回常规采样。两层最大采样数从约 102 降至 32。

## 2026-10-10 五项问题跟进

- 地面：反照率与切线法线分别输出 DataTexture，法线使用线性颜色空间。读取 `0x0D02D58A/B` 的主区/边框高度，按 `overlayGetHeight` 重建边缘坡度；未覆盖区使用 base tile 法线。MeshPhongMaterial 随太阳及局部灯光受光，接收建筑阴影。图案 V 相位沿引擎 −Y，与已翻行的遮罩分开处理。
- 霓虹：保留原 HLSL 的投影加法照明；投影通道撤销 DTO 的颜色 ×2，恢复原始权重。灯管空白片元显式 discard，避免自定义 ShaderMaterial 中无效的 alphaTest 导致透明矩形写深度。该修正不代表已实现引擎延迟 HDR 合成。
- 交互：Property Editor 移动/旋转/缩放期间使用 DPR 1，停止 180ms 后恢复最高 DPR 2；截图临时恢复完整分辨率。
- 灯源：精细模式点灯/聚光灯拾取代理固定半径 1，线灯保留长度但限制横截面。代理不写颜色/深度、不投影；实际灯光影响半径不变。真实 Three Raycaster 测试确认影响范围内、发光体外的射线不会抢选。
- 装配：恢复缺少 LotOverlayBoxOffset 时的声明 bbox 中心回退，显式 `[0,0]` 不替换。**两个用户案例的偏移仍未解决**，此回退不能修复它们。

### 实测

Intel UHD (0xA7A8), ANGLE D3D11, 模型 `90047206`，相同近景相机、1000×800、DPR 1.5，WebGL2 timer query（disjoint=false，各 49 个样本）：旧 16-tap 全图重建 GPU 中位数 **75.43ms**，新路径 **44.69ms**，降低 **40.8%**。这不是游戏 FPS，也不是所有显卡的保证。记录：本地 `tmp/property-render/performance-five.json`。

三个真实 fixture（90047206、657934AC、E8D0CAFA）均无 WebGL/shader 错误，交互 DPR 1→2 实测通过；地面左右相反太阳方向截图确认砖缝/边框动态受光。合成 GPU 7 项采样检查通过。

### 装配问题取证与剩余边界

用户确认的 Property 为 `00B1B104:42E1C000:90047206`、`00B1B104:42E1C000:657934AC`。对应 LotSize 为 48×96、96×96，offset 都是显式 0，placement 缺失，overlay UV 都为完整 0..1。mask 分别是 377C0BF1 (64×128)、616EA69F (128×128)，原始 mip 长度与像素尺寸吻合。顶视图能复现车辆与停车位错开。

源依据：`SC/cZoningGame/CreateUnitLotGraphics.c` (0x8BA1C0) 以单位 lot transform 变换 offset/bbox center，U 取 +X、V 取 −Y，按 LotSize 归一再加 .5；`cUnitLotData/CalculateUnitLotTransform.c` 使用 unitWorld × inverse(placement)。`anonymous/FindNonSimProps.c` (0x6175B0) 通过 `0x08E17ED7` 或同实例 NS group 查找，不是将模拟 Property 任意合并进图形 Property。EP1 两例均只有一份同实例 Property；EP1/Game 反向 Key 扫描只发现已知模型引用，没有另一份 NS Key 指向它们。未发现可支持统一 24m 等补偿的源码或属性，故未加入经验偏移。

地面仍是 CPU 合成：高度边缘的 fwidth 按输出 texel 近似，不能完全等同游戏逐屏幕片元导数；区域颜色/法线仍沿用既有阈值选层。尚未建立运行中游戏的同相机截图/Unit 最终属性对拍，不能据此宣称五项全部完成或编辑器与游戏完全一致。

## 验证与限制

使用现有 `tmp/property-preview.html` 和两套真实 LOTM 样本运行 `tmp/check-building-quality.cjs`，记录同相机近景、远景、斜视及移除切线后的 WebGL 编译结果。对拍图片和 JSON 存于 `tmp/property-render/`（游戏衍生产物不入库）。同时运行 TypeScript、shader 注入、材质环境与模型解析回归测试。

窗户合成边缘检查：撤掉硬化放大后，当前过渡像素为 106（线性为 115；此前硬化版本为 58），优先保留连续的曲线过渡，不再追求最窄的边缘。64/192 灰阶保留且无过冲，缩小采样逐像素不变，GL 错误为 0。

风扇已按用户提供的 `SimCity_Game.package / 0x39838A50` 直接导出：11084 顶点、68 列参数、256×256 图集均与报告吻合。命中风扇为参数列 2，区域比例约 0.199×0.198，即有效图案仅约 51×51 纹素。实际同机位新旧过滤截图在 `tmp/property-render/E8D0CAFA-{old-filter,cubic-final}-fans.png`，裁剪对比为 `fan-filter-comparison.png`。远景、斜视及缺切线路径加上前两套建筑均无 WebGL 错误。此次 9 项 shader/环境测试、类型与 lint 检查通过。

可复用 GPU 回归入口：启动 Vite 后打开 `/tools/check-building-sampling.html`。它从实际材质注入结果提取 GLSL，并回读 GPU 像素验证仿射信号重建（会捕获硬化纹素回归）、颜色/透明度无过冲、缩小采样不变、邻区不串色、镜像及单纹素区域，共 7 项通过。测试只用合成数据，不需要游戏资产。

精确模型导出可使用现有 ignored 测试，额外设置 `SC_FIXTURE_BASE_GAME_ONLY=1` 和 `SC_MODEL_INSTANCE=39838A50`；后者绕过地块继承/变体规则的自动模型选择。仍需 `SC_GAME_DATA` 与 `SC_LOT_INSTANCE`（本次 E8D0CAFA 提供场景）。最初按地块自动解析实际选中了另一个模型，不能拿其 95 列材质作为本报告依据。

本轮是源码公式与本地渲染前后对拍，未运行 dev 游戏生成同机位截图。未生成高清替换贴图，也未把内景 alpha 当作未经确证的高度图。原窗户反馈尚无明确模型 ID，风扇反馈已精确复现。
