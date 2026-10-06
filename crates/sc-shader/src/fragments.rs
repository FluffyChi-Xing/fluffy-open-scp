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
      outColor = vec4(col * uNightBoost, alpha);"),
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
      Current.color.rgb = lerp(Current.color.rgb, interiorTextureLit, saturate(decalTexture.a * 2.0 - 1.0));\n\
      Current.color.a = saturate(decalTexture.a * 2.0);"),
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
    // ---- PS：SDF 管灯动画（Darken 主体，公式逐字）----
    ("decalAnimateSDFDarken",
     "float3 decalNUS = In.texcoord<t0>.xyz;\n\
      float materialLightScale = decalMaterialInfo.x * 16.0 + 0.25;\n\
      float sdfTextureLength = max(decalNUS.x, decalNUS.y);\n\
      float sphereHeight = decalNUS.z;\n\
      float hwRatio = sphereHeight * 0.5 / sdfTextureLength;\n\
      float zScale = 1;\n\
      if (hwRatio < 1)\n\
      {\n\
        hwRatio = 1;\n\
        zScale = 1 / hwRatio;\n\
      }\n\
      float circleZ = texturePosition.z;\n\
      circleZ *= zScale;\n\
      float4 sdfDists = Current.color;\n\
      float kMaskCenter = 0.5;\n\
      float4 circleDists = saturate(1 - sdfDists * 1.0 / kMaskCenter) * hwRatio;\n\
      float4 sphereDistsSqr = circleDists * circleDists + circleZ * circleZ;\n\
      float4 animEdge = max(animResults, 0.0);\n\
      float lerpXParam = sphereHeight * 0.5 / decalNUS.x;\n\
      float lerpYParam = sphereHeight * 0.5 / decalNUS.y;\n\
      float4 animation = lerp(float4(lerpXParam, lerpXParam, lerpXParam, lerpXParam),\n\
                              float4(lerpYParam, lerpYParam, lerpYParam, lerpYParam), useV.xyzw);\n\
      sphereDistsSqr += animEdge * animEdge * animRatio * 32;\n\
      float lightScales = saturate(1 - sqrt(sphereDistsSqr));\n\
      lightScales *= lightScales;\n\
      float3 lightColor = float3(0, 0, 0);\n\
      for (int i = 0; i < 3; ++i)\n\
      {\n\
        lightColor[i] = materialLightScale * dot(decalMaterialData[i], lightScales);\n\
      }\n\
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
