//! decal 核心片段的**人工清理版**源码。
//!
//! 容器自动重组在 250B 边界偶有丢字（len 校验失败跳字节），核心片段经
//! 人工对照修复后固化于此（原文对照：tmp/dynamic/decal_full_0.txt）。
//! 每条注 `source:` = 容器中的片段名；转译器只消费本表（自动提取版仅
//! 作 diff 参考，不进产物）。

/// (片段名, HLSL 源码) 列表。占位符约定：转译阶段注入声明。
pub const CORE_FRAGMENTS: &[(&str, &str)] = &[
    // ---- PS：量化合成（探针 v5 逐字；GPU 双线性采样 + 阈值后置 + 优先级链）----
    ("decalQuantComposite",
     "vec4 m = texture2D(uSampler0, vUv);
      vec3 col = vec3(0.0);
      float alpha = 0.0;
      if (m.a >= 0.5)      { col = uLayerColors[3].rgb; alpha = 1.0; }
      else if (m.b >= 0.5) { col = uLayerColors[2].rgb; alpha = 1.0; }
      else if (m.g >= 0.5) { col = uLayerColors[1].rgb; alpha = 1.0; }
      else if (m.r >= 0.5) { col = uLayerColors[0].rgb; alpha = 1.0; }
      outColor = vec4(col, alpha);"),
    // ---- VS ----
    ("decalProject",
     // 引擎原文单行：模型位置经体积矩阵进纹理空间
     "Current.texcoord<t0> = mul(modelToTexture, float4(modelPos, 1));"),
    ("decalFloatQuad",
     // 浮空族：UV 烤在顶点数据（索引字节 /255）
     "Current.texcoord<t0>.xyz = Current.indices.yzw * (1.0 / 255.0);"),
    ("decalVS",
     "Current.texcoord<t0>.xyz = In.texcoord<t0>.xyz;"),
    // ---- 数据装载（VS）----
    ("decalMaterialData1",
     "Current.color = In.texcoord4;\nCurrent.color1 = In.texcoord5;"),
    // ---- PS：标准直采 ----
    ("decalClip",
     "float3 textureFloatPosition = In.texcoord<t0>.xyz;\n\
      clip(-textureFloatPosition.z);\n\
      float2 uvOrig = textureFloatPosition.xy * -0.5 + 0.5;\n\
      float2 uv = uvOrig * texXform.xy + texXform.zw;\n\
      float4 decalTexture = tex2D(Sampler<s0>, uv);\n\
      Current.color = decalTexture;"),
    // ---- PS：标准直采（PE quad 适配版，2026-10-04 人工补）----
    // 引擎 decalClip 的 -0.5 镜像面向投影矩阵纹理坐标；PE quad 的几何 UV
    // 构建期已做同一镜像（1-x），直接采 vUv 即引擎等价。用途：涂鸦/焦痕/
    // 海报等**直采族**——引擎按材质路由到 decalProject 直采链（raster RGB
    // 即美术内容，alpha = 喷漆/烧灼衰减），而非量化合成链（raster 通道 =
    // 层权重掩码）。焦痕 decal 误走量化链时层色近黑 → 整块纯黑
    // （2026-10-04 用户图1~3 根因：像素级分析证明纹理 RGB≈0.2-0.35）。
    ("decalClipQuad",
     "vec4 decalTexture = texture2D(uSampler0, vUv);\n\
      Current.color = decalTexture;"),
    // ---- PS：霓虹增亮（SimCityLighting 响应；光照由 uniforms 注入）----
    ("decalNeonBrighten",
     "float3 bumpNormal = normalize(decalWorldDirection);\n\
      float3 shColorDiff = float3(0, 0, 0);\n\
      float3 shColorSpec = float3(0, 0, 0);\n\
      float3 spec = float3(0, 0, 0);\n\
      SimCityLighting(bumpNormal, worldCameraDirection.xyz, gloss, reflectance,\n\
                     specE, specStrength, shColorDiff, shColorSpec, spec);\n\
      Current.color.rgb *= shColorDiff + shColorSpec + spec;"),
    // ---- PS：破洞内景（完整公式，容器原文逐字）----
    ("decalLightInteriorMap",
     "const float kSunContributionAmount = decalMaterialData[0].x;\n\
      const float kLightAmount = decalMaterialData[0].y;\n\
      float3 textureFloatPosition = In.texcoord<t0>.xyz;\n\
      float2 interiorUv = lerp(textureFloatPosition.xy, textureFloatPosition.xy * 0.5, textureFloatPosition.z * 0.5 + 0.5);\n\
      interiorUv = interiorUv * -0.5 + 0.5;\n\
      interiorUv = interiorUv * texXform.xy + texXform.zw;\n\
      float sunMod = saturate(dot(sunSky.mSunDir.xyz, bumpNormal.xyz));\n\
      float3 sunColor = sunMod * sunSky.mSunColor.rgb * shadow;\n\
      shColorDiff -= sunColor;\n\
      shColorDiff *= kLightAmount;\n\
      shColorSpec *= kLightAmount;\n\
      shColorDiff += sunColor * kSunContributionAmount;\n\
      float4 interiorTexture = tex2D(Sampler<s0>, interiorUv);\n\
      float3 interiorTextureLit = interiorTexture.rgb * (shColorDiff + shColorSpec + spec + interiorTexture.a * kInteriorMapSelfLightMax);\n\
      Current.color.rgb = lerp(Current.color.rgb, interiorTextureLit, saturate(decalTexture.a * 2 - 1));\n\
      Current.color.a = saturate(decalTexture.a * 2);"),
    ("kInteriorMapSelfLightMax",
     "static const float kInteriorMapSelfLightMax = 16.000000;"),
    // ---- PS：无光变体的假灯球（d3d9 内部：GetDeferredNormal 由 uniform 法线替代）----
    ("decalWorldDirection",
     "float3 materialLightScale = decalMaterialInfo.x * 16.0 + 1;\n\
      float3 invMaterialLightRadius = decalMaterialInfo.y * 4.0;\n\
      float circleDist = saturate(1 - decalTexture.a * 256.0 / 200.0);\n\
      float circleZ = texturePosition.z * 0.5 + 0.5;\n\
      circleZ *= length(decalWorldDirection) * invMaterialLightRadius;\n\
      float sphereDist = sqrt(circleDist * circleDist + circleZ * circleZ);\n\
      float lightScale = saturate(1 - sphereDist);\n\
      float lightAmount = saturate(dot(normalize(decalWorldDirection), worldNormal)) * materialLightScale;\n\
      Current.color = float4(Current.color * lightScale * lightAmount, 0);"),
    // ---- PS：浮空增亮 ----
    ("decalFloatQuadNoClip",
     "Current.color.rgb *= 2;"),
    // ---- PS：霓虹动画背景（decalLightBackground，容器 line 4103 有损修复版）----
    // decalMaterialData 已经 VS decalMaterialData4 转置（line 4139）：
    // 第 3 列（w 列）= 四路独立动画参数（**不是颜色**）：分量符号选 UV 轴
    //（> 0 → 用 uv.x，否则 uv.y），abs 后整数 = 分块数 animChunks、小数 =
    // 相位 animOffsets（实证 0x23D05B09：±60.0/.3/.6/.9 = 四路 60 块相位
    // 错开的追逐灯）。gameInfo.x → uTime 墙钟。末行原文截断，按 Darken 的
    // 消费语义（animEdge = max(animResults, 0)）修复为 uvCompare - compares。
    // uvOrig 由组合器前奏定义（= vTexcoord0.xy×0.5+0.5；PE quad 几何 UV 已
    // 携带引擎 -0.5 镜像，故此处为正号）。
    ("decalLightBackground",
     "#undef animResults\n\
      #undef useV\n\
      float4 animParameters = decalMaterialData[3];\n\
      float4 useV = float4(animParameters.x > 0.0 ? 0.0 : 1.0,\n\
                           animParameters.y > 0.0 ? 0.0 : 1.0,\n\
                           animParameters.z > 0.0 ? 0.0 : 1.0,\n\
                           animParameters.w > 0.0 ? 0.0 : 1.0);\n\
      animParameters = abs(animParameters) + 0.0001;\n\
      float animTime = frac(uTime * decalMaterialInfo.y + 0.9999);\n\
      float4 animOffsets = frac(animParameters);\n\
      float4 animChunks = max(float4(1.0, 1.0, 1.0, 1.0), floor(animParameters));\n\
      float4 compares = floor((animTime * 3.0 - animOffsets) * animChunks) * (1.0 / animChunks);\n\
      float4 uvCompare = lerp(float4(uvOrig.x, uvOrig.x, uvOrig.x, uvOrig.x),\n\
                              float4(uvOrig.y, uvOrig.y, uvOrig.y, uvOrig.y), useV);\n\
      float4 animResults = uvCompare - compares;"),
    // ---- PS：灯管调光（decalAnimateSDFDisabled，line 3736 有损修复版）----
    // 跑马灯扫掠阈值的暗亮窗：animResults 小于 0 = 已扫过（点亮，全亮
    // materialTubeLightFactor = z×8+1）、未扫到 = 0.1 暗态；阈值两侧
    // smoothstep 软边 = 引擎"字体+花纹从暗渐变到亮"的过渡带（对拍
    // TAKEOUT/CHEAP 截图的扫掠辉光边缘，2026-10-05 八轮）。原文
    // lesser_than(animResults, 0) ? vec4 : vec4 的向量条件三目在 GLSL ES
    // 不合法 → mix+smoothstep 等价改写。decalMaterialInfo.w = 供电，
    // 断电 → 半亮（lerp 0.5）。uAnimEnabled = 0（静态模式）→ 恒 1 全亮，
    // 不吃扫掠窗（精细渲染"动态招牌"开关默认关）。
    // decalMaterialData 为 uniform 不可写 → 调光后权重列落本地 tubeColor0-2
    // （列 0~2 = 输出 RGB 对四掩码通道的权重，转置语义见 decalLightBackground）。
    ("decalAnimateSDFDisabled",
     "float materialTubeLightFactor = decalMaterialInfo.z * 8.0 + 1.0;\n\
      float4 lightFactor = mix(float4(materialTubeLightFactor, materialTubeLightFactor,\n\
                                      materialTubeLightFactor, materialTubeLightFactor),\n\
                               float4(0.1, 0.1, 0.1, 0.1),\n\
                               smoothstep(float4(-0.02, -0.02, -0.02, -0.02),\n\
                                          float4(0.15, 0.15, 0.15, 0.15), animResults));\n\
      float4 powerFactor = lerp(float4(0.5, 0.5, 0.5, 0.5), lightFactor, decalMaterialInfo.wwww);\n\
      powerFactor = mix(float4(1.0, 1.0, 1.0, 1.0), powerFactor, float4(uAnimEnabled, uAnimEnabled, uAnimEnabled, uAnimEnabled));\n\
      float4 tubeColor0 = decalMaterialData[0] * powerFactor;\n\
      float4 tubeColor1 = decalMaterialData[1] * powerFactor;\n\
      float4 tubeColor2 = decalMaterialData[2] * powerFactor;"),
    // ---- PS：SDF 调色板归属解码 + 点亮合成（Darken 行为对拍版）----
    // 2026-10-05 八轮对拍定谳，取代球面衰减版（容器 line 3747-3779 的
    // 逐字移植）：
    // 1) 取证：该族贴图纹素 RGBA 恒等于字典 colors 四行之一（加油站
    //    0x090C71D6：面板 = row0 青 / 油泵 = row1 紫 / 字体 = row3 黄；
    //    dump_decal_4color + 逐纹素比对）——**贴图自带最终色**，转置
    //    权重列 dot 只有在 lightScales 为元素 one-hot 时才还原原色；
    // 2) 球面衰减版 min(2·sdf,1)² 多通道同时点亮（面板吃 B+A 两路 →
    //    红亮盖字、字体被背景淹没）= "动态色块无细节"的根因；
    // 3) one-hot 归属（最近调色板行）后每个元素吃自己的 colors 行与
    //    独立动画通道（w 列 chunks/相位），扫掠以字体+花纹为遮罩从暗
    //    渐变到亮——与引擎 TAKEOUT/CHEAP 截图逐区域对拍一致。
    // 球面衰减的 3D 灯管圆润度（circleZ/hwRatio）对量化贴图无对应物，
    // 随旧版一并移除；uDecalNUS 保留声明供后续校准引用。
    ("decalAnimateSDFDarken",
     "float materialLightScale = decalMaterialInfo.x * 16.0 + 0.25;\n\
      float3 sdfTexel = Current.color.rgb;\n\
      float3 sdfRow0 = float3(decalMaterialData[0].x, decalMaterialData[1].x, decalMaterialData[2].x);\n\
      float3 sdfRow1 = float3(decalMaterialData[0].y, decalMaterialData[1].y, decalMaterialData[2].y);\n\
      float3 sdfRow2 = float3(decalMaterialData[0].z, decalMaterialData[1].z, decalMaterialData[2].z);\n\
      float3 sdfRow3 = float3(decalMaterialData[0].w, decalMaterialData[1].w, decalMaterialData[2].w);\n\
      float4 sdfDist = float4(dot(sdfTexel - sdfRow0, sdfTexel - sdfRow0),\n\
                              dot(sdfTexel - sdfRow1, sdfTexel - sdfRow1),\n\
                              dot(sdfTexel - sdfRow2, sdfTexel - sdfRow2),\n\
                              dot(sdfTexel - sdfRow3, sdfTexel - sdfRow3));\n\
      float sdfBest = min(min(sdfDist.x, sdfDist.y), min(sdfDist.z, sdfDist.w));\n\
      float4 lightScales = float4(sdfDist.x <= sdfBest ? 1.0 : 0.0,\n\
                                  sdfDist.y <= sdfBest ? 1.0 : 0.0,\n\
                                  sdfDist.z <= sdfBest ? 1.0 : 0.0,\n\
                                  sdfDist.w <= sdfBest ? 1.0 : 0.0);\n\
      float3 lightColor = float3(0.0, 0.0, 0.0);\n\
      lightColor.x = materialLightScale * dot(tubeColor0, lightScales);\n\
      lightColor.y = materialLightScale * dot(tubeColor1, lightScales);\n\
      lightColor.z = materialLightScale * dot(tubeColor2, lightScales);\n\
      lightColor *= decalMaterialInfo.w;\n\
      Current.color.rgb = lightColor;"),
    // ---- PS：灯管亮度（场景光叠加）----
    ("decalLightSDF",
     "float3 bumpNormal = normalize(decalWorldDirection);\n\
      float3 shColorDiff = float3(0, 0, 0);\n\
      float3 shColorSpec = float3(0, 0, 0);\n\
      float3 spec = float3(0, 0, 0);\n\
      SimCityLighting(bumpNormal, worldCameraDirection.xyz, gloss, reflectance,\n\
                     specE, specStrength, shColorDiff, shColorSpec, spec);\n\
      Current.color.rgb += shColorSpec + spec;"),
    // ---- PS：灯管开关（decalMaterialInfo.w 供电，断电=半亮）----
    ("decalLightNeonTube",
     "Current.color.a *= decalMaterialInfo.x;"),
    // ---- 共享光照库（SimCityLighting 的均匀光近似——天空环境 + 太阳 N·L；
    //      引擎全量含屏幕空间查表，前向渲染器以 uniforms 注入等价输入）----
    ("SimCityLighting",
     "void SimCityLighting(float3 normal, float3 viewVector, float glossyStrength,\n\
                     float reflectance, float specE, float specStrength,\n\
                     inout float3 shColorDiff, inout float3 shColorSpec,\n\
                     inout float3 specHighlight)\n\
     {\n\
       half sunMod = saturate(dot(sunSky.mSunDir.xyz, normal));\n\
       shColorDiff = mix(uAmbientDiff, sunSky.mSunColor.rgb, sunMod * uDayLight);\n\
       shColorSpec = sunSky.mSunColor.rgb * uSpecularScale;\n\
       float3 halfVector = normalize(sunSky.mSunDir.xyz - viewVector);\n\
       float nDotH = saturate(dot(normal, halfVector));\n\
       float spec = pow(nDotH, specE);\n\
       specHighlight = spec * specStrength * sunMod * sunSky.mSunColor.rgb;\n\
     }"),
];

/// 按 名 查清理片段。
pub fn get(name: &str) -> Option<&'static str> {
    CORE_FRAGMENTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, src)| *src)
}
