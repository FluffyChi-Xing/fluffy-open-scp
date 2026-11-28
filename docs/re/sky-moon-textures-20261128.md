# 天空、月亮与星空静态调查（2026-11-28）

结论：月亮有可单独替换的纹理目标；整片天空不能据此视作一张可替换全景图。未部署、未启动游戏，运行期验收待定。

## 已确认资源链

开发版反编译 `cSunSkyLightingWorld::UpdateSunMoonTransform`（VA 00B5CE50）创建 `Weather_Sun_Effect`，`UpdateStarsTransform`（VA 00B5D130）创建 `Weather_Stars`。这是开发版代码证据，不代表已验证零售版相同函数实现。

当前安装的 SimCity_App.package 中 EA5118B0:40804300:622B9CD7 可独立解码出对应导出：

- Weather_Sun_Effect，FNV1 C9494498，visual 115，particle 205/206/207。
- particle 207 → material B84B6B55 → shader 74EC0F47 → slot2 texture 78AC883A。
- particle 207：tileCount [1,16]，size [1500]，frameSpeed 约0.066，life [720,720]。不将这些字段直接等同于实际游戏时间或屏幕尺寸。
- SimCity_Game.package 的 2F4E681B:00000000:78AC883A 已解码为 32×512 PNG，目检确认为16幅纵向月相，每帧32×32。
- Weather_Stars，FNV1 1A0785E9，visual 117，particle 208 → material F5E679F9 → shader 1F800EDE → slot2 texture 78A1B67E。
- SimCity_Game.package 的 2F4E681B:00000000:78A1B67E 已解码为8×64 PNG；粒子 tileCount [1,8]。

## 天空模型与大气

SimCity_Graphics.package 的 2F4E681B:00000000:A2886767（注册名 skybox）解码150796字节。model_peek 输出1310顶点、2400三角、一个当前解码器无法解开的材质、4×4和8×8 DXT1内嵌纹理。存在该模型不证明它在当前天空路径中已实例化。docs/rendering.md 所写“没有独立 skydome mesh”缺乏完整依据；已有大气公式/LUT证据仍成立，但不能用它排除天空几何路径。

## 自定义方案与边界

保留原月亮特效和材质，只将目标纹理编码为同类RW4资源，保持纵向16帧布局、透明度和mipmap结构，可以制作最小纹理覆盖包作为验证候选。高清月面例如每帧256×256、总图256×4096只是待验证规格，不是已确认引擎支持结论。只换一幅图片而不处理16帧，可能出现切片错误或月相变化错误。

整体全景天空、星云或银河背景需要进一步追踪天空材质及绘制路径；星星粒子图集不能直接当全景图替换。模糊还可能受到采样、后处理和原版发光影响；不能保证提高贴图分辨率就消除截图中的全部模糊。

## 取证输出

`tmp/sky-investigation-20261128/`：weather.json、moon-atlas.png、stars-atlas.png、sources.json（实际安装包SHA-256及相关解码payload SHA-256）。使用既有 model_peek、effect_swarm_dump、effect_texture_dump 探针；未更改产品源码或游戏目录。
