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
    // ---- PS：量化合成（复用已有采样版，SDF 族静态分支用）----
    // 与 decalQuantComposite 同规则，但掩码取自 outColor（= sharp-bilinear
    // 保级采样结果），不重复采样——2026-10-05 用户指令："动画关 = 按静态
    // 招牌渲染"（SDF 族贴图同为多级量化掩码，阈值口径一致）。
    ("decalQuantCompositeFromSample",
     "vec4 m = outColor;\n\
      vec3 col = vec3(0.0);\n\
      float alpha = 0.0;\n\
      if (m.a >= 0.5)      { col = uLayerColors[3].rgb; alpha = 1.0; }\n\
      else if (m.b >= 0.5) { col = uLayerColors[2].rgb; alpha = 1.0; }\n\
      else if (m.g >= 0.5) { col = uLayerColors[1].rgb; alpha = 1.0; }\n\
      else if (m.r >= 0.5) { col = uLayerColors[0].rgb; alpha = 1.0; }\n\
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
    // GLSL 适配（2026-10-05 路由修复后首次真正编译此链）：HLSL `static const`
    // 从 uniform 取值在 GLSL ES 非法（const 要求编译期常量）→ 降为 float；
    // 整数字面量 2/1 提升为 2.0/1.0（GLSL ES 无 float×int 隐式转换）。
    // 内景 UV（2026-10-05 四轮）：原版两行
    //   lerp(tfp.xy, tfp.xy*0.5, tfp.z*0.5+0.5) * -0.5 + 0.5
    // 依赖场景内真实几何的深度梯度（破洞后露出的楼板）；PE 建筑是空壳 →
    // 替换为 compose 注入的 holeParallaxUv（视线射线-盒底平面求交，直视
    // 等价原版、斜视产生窗户式视差）。其余行保持逐字。
    ("decalLightInteriorMap",
     "float kSunContributionAmount = decalMaterialData[0].x;\n\
      float kLightAmount = decalMaterialData[0].y;\n\
      float3 textureFloatPosition = In.texcoord<t0>.xyz;\n\
      float2 interiorUv = holeParallaxUv(textureFloatPosition);\n\
      interiorUv = interiorUv * texXform.xy + texXform.zw;\n\
      float sunMod = saturate(dot(sunSky.mSunDir.xyz, bumpNormal.xyz));\n\
      float3 sunColor = sunMod * sunSky.mSunColor.rgb * shadow;\n\
      shColorDiff -= sunColor;\n\
      shColorDiff *= kLightAmount;\n\
      shColorSpec *= kLightAmount;\n\
      shColorDiff += sunColor * kSunContributionAmount;\n\
      float4 interiorTexture = tex2D(Sampler<s0>, interiorUv);\n\
      float3 interiorTextureLit = interiorTexture.rgb * (shColorDiff + shColorSpec + spec + interiorTexture.a * kInteriorMapSelfLightMax);\n\
      Current.color.rgb = lerp(Current.color.rgb, interiorTextureLit, saturate(decalTexture.a * 2.0 - 1.0));\n\
      Current.color.a = saturate(decalTexture.a * 2.0);"),
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
    // materialTubeLightFactor = z×8+1）、未扫到 = 0.35 暗态（引擎原文 0.1；
    // PE 缺引擎的相位1加法光晕 pass + tonemap/bloom，0.1 暗态在 PE 里整牌
    // 太暗——十二轮用户"动画模式对比度亮度偏低"对拍，提下限补偿，仍保留
    // 暗→亮流动）；阈值两侧
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
                               float4(0.35, 0.35, 0.35, 0.35),\n\
                               smoothstep(float4(-0.02, -0.02, -0.02, -0.02),\n\
                                          float4(0.15, 0.15, 0.15, 0.15), animResults));\n\
      float4 powerFactor = lerp(float4(0.5, 0.5, 0.5, 0.5), lightFactor, decalMaterialInfo.wwww);\n\
      powerFactor = mix(float4(1.0, 1.0, 1.0, 1.0), powerFactor, float4(uAnimEnabled, uAnimEnabled, uAnimEnabled, uAnimEnabled));\n\
      float4 tubeColor0 = decalMaterialData[0] * powerFactor;\n\
      float4 tubeColor1 = decalMaterialData[1] * powerFactor;\n\
      float4 tubeColor2 = decalMaterialData[2] * powerFactor;"),
    // ---- PS：SDF 遮罩上色（decalSDF[394] addOverlay，dev 原版语义 ----
    // 2026-10-05 十一轮重写，按 §12.5 逐字对拍取代八轮的"最近调色板行
    // one-hot 归属"——dev 没有分类步骤：四通道直接当遮罩（maskCenter=0.5
    // + fwidth 屏幕导数抗锯齿 = 官方清晰度答案，§12.6），颜色 = 调光后
    // 权重列与遮罩的**加权求和**：
    //   lightColor_i = materialLightScale · dot(tubeColor_i, mask4)
    // one-hot 把过渡区/多通道区像素硬归单一通道 = "纯色块无图案"根因
    // （用户 09:58 对拍）。alpha = 四通道遮罩 max（覆盖区不透明）。
    ("decalAnimateSDFDarken",
     "float materialLightScale = decalMaterialInfo.x * 16.0 + 0.25;\n\
      float4 sdfFw = min(fwidth(Current.color), float4(0.15, 0.15, 0.15, 0.15));\n\
      float4 sdfMask = smoothstep(float4(0.5, 0.5, 0.5, 0.5) - sdfFw,\n\
                                  float4(0.5, 0.5, 0.5, 0.5) + sdfFw, Current.color);\n\
      float3 lightColor = float3(0.0, 0.0, 0.0);\n\
      lightColor.x = materialLightScale * dot(tubeColor0, sdfMask);\n\
      lightColor.y = materialLightScale * dot(tubeColor1, sdfMask);\n\
      lightColor.z = materialLightScale * dot(tubeColor2, sdfMask);\n\
      lightColor *= decalMaterialInfo.w;\n\
      Current.color.rgb = lightColor;\n\
      Current.color.a = max(max(sdfMask.x, sdfMask.y), max(sdfMask.z, sdfMask.w));"),
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
