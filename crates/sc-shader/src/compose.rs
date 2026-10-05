//! 家族组合器：family → 完整 VS/PS GLSL。
//!
//! 引擎的最终 shader = 片段链（VS + 数据装载 + PS）按 shader-def 拼接；
//! 组合表按家族路由文档（docs/re/decal-family-routing.md）人工确定，
//! 共 4 条产出链（sign/graffiti 共链）。PS 公共序言 = 引擎符号 uniform +
//! three.js 相容 varying；`outColor` 为片元输出（GLSL1 gl_FragColor）。

use crate::fragments;
use crate::translate::translate;

/// PS 公共序言：引擎符号对齐（SYMBOLS 的 #define 子集）+ 片元输出。
pub const PS_PREAMBLE: &str = r#"// == 由 sc-shader 组合器生成：引擎符号对齐 + three.js 相容序言 ==
precision highp float;

uniform vec4 uDecalMaterialData[4];
#define decalMaterialData uDecalMaterialData
uniform vec4 uDecalMaterialInfo;
#define decalMaterialInfo uDecalMaterialInfo
uniform vec4 uTexXform;
#define texXform uTexXform
uniform vec3 uDecalWorldDirection;
#define decalWorldDirection uDecalWorldDirection
uniform vec3 uWorldNormal;
#define worldNormal uWorldNormal
uniform vec3 uWorldCameraDirection;
#define worldCameraDirection uWorldCameraDirection
uniform float uShadow;
#define shadow uShadow
uniform float uGloss;
#define gloss uGloss
uniform float uReflectance;
#define reflectance uReflectance
uniform float uSpecE;
#define specE uSpecE
uniform float uSpecStrength;
#define specStrength uSpecStrength
uniform vec3 uAmbientDiff;
uniform float uDayLight;
uniform float uSpecularScale;
uniform sampler2D uSampler0;
uniform vec3 uSunDir3;
uniform vec3 uSunColor3;
uniform float uAnimRatio;
#define animRatio uAnimRatio
uniform vec4 uAnimResults;
#define animResults uAnimResults
uniform vec4 uUseV;
#define useV uUseV
uniform float uTime;
uniform float uAnimEnabled;
uniform vec3 uDecalNUS;
uniform vec4 uLayerColors[4];
uniform float uNightBoost;
uniform vec2 uSdfTexSize;

varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;

vec4 outColor = vec4(0.0);
"#;

/// VS 公共序言（quad 自持几何；position/uv/matrices 由 three.js 注入，
/// 不得重复声明）。
pub const VS_PREAMBLE: &str = r#"// == 由 sc-shader 组合器生成：quad VS（three.js 相容）==
varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;
void main() {
  vUv = uv;
  // quad uv ∈[0,1] → 引擎盒坐标 [-1,1]（decalClip 的 -0.5 镜像由此自动成立）
  vTexcoord0 = vec3(uv * 2.0 - 1.0, 0.0);
  vTexcoord4 = vec4(0.0);
  vTexcoord5 = vec4(0.0);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
"#;

/// 破洞投影 VS（2026-10-05 片段重组定谳后重写，二轮修正 tfp.z）：引擎
/// decalInteriorMap 是屏幕空间投影贴花（decalProject 深度重建 + decalClip
/// 体积裁剪），PE 的 CPU 同构 = DecalGeometry 把建筑三角形裁进体积盒生成
/// 贴面网格（与其他族同路）。vTexcoord0 = 引擎 textureFloatPosition：
/// x/y 由投影几何镜像 UV 携带（uv×2−1）；**z = 顶点在盒内的真实进深**
/// （dot(position−origin, axisZ)/depth ∈ [0,1] → [−1,1]）——引擎延迟路径
/// 逐像素深度重建的 VS 同构：立面像素 z≈−1 → 内景 lerp 因子 0 = **全尺寸**
/// 内景图；破洞后露出的楼板/内墙等深部几何 z→+1 → UV 向中心收缩 = 纵深
/// 错觉（游戏"破洞有内部结构"的机制）。一轮曾 z 恒 +1 = lerp 因子恒 1，
/// 只采贴图中心 1/4 放大 = "黑色平斑无立体感"根因。
/// （此前体积盒 VS 的 uBoxHalf 路径随 BackSide 盒渲染一并退役：盒内壁
/// 不随建筑曲面走 = "破洞不贴合/漂浮"根因。）
pub const VS_HOLE_PROJECTED: &str = r#"// == 由 sc-shader 组合器生成：破洞投影 VS（three.js 相容）==
varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;
varying vec3 vObjPos;
uniform vec3 uHoleOrigin;
uniform vec3 uHoleAxisZ;
uniform float uHoleInvDepth;
void main() {
  vUv = uv;
  vObjPos = position;
  // tfp.z = 几何在贴花盒内的真实进深（position = lot 局部坐标，与
  // uHoleOrigin/uHoleAxisZ 同空间）：[0, depth] → [-1, 1]
  float holeDepth = clamp(dot(position - uHoleOrigin, uHoleAxisZ) * uHoleInvDepth, 0.0, 1.0);
  vTexcoord0 = vec3(uv * 2.0 - 1.0, holeDepth * 2.0 - 1.0);
  vTexcoord4 = vec4(0.0);
  vTexcoord5 = vec4(0.0);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// 招牌（量化合成链：raster 通道 = 层权重掩码 × Color1-4）
    Sign,
    /// 涂鸦/焦痕/海报（decalProject 直采链的 PE quad 适配：raster RGB 直采）
    Clip,
    /// 破洞内景（decalInteriorMap 系）
    Hole,
    /// 全息浮空（decalFloatQuad 系）
    Holo,
    /// SDF 霓虹管（decalAnimateSDF 系）
    Sdf,
}

impl Family {
    pub const ALL: [Family; 5] = [
        Family::Sign,
        Family::Clip,
        Family::Hole,
        Family::Holo,
        Family::Sdf,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Family::Sign => "sign",
            Family::Clip => "clip",
            Family::Hole => "hole",
            Family::Holo => "holo",
            Family::Sdf => "sdf",
        }
    }

    /// PS 链（按序拼接——引擎的执行顺序即数据流）。
    fn ps_chain(self) -> &'static [&'static str] {
        match self {
            Family::Sign => &["decalQuantComposite"],
            Family::Clip => &["decalClipQuad"],
            Family::Hole => &["decalLightInteriorMap"],
            Family::Holo => &["decalClip", "decalFloatQuadNoClip"],
            // 十一轮：decalLightNeonTube（alpha ×= info.x）移出链——那是涂鸦
            // 的 decalOpacity[389] 语义，对灯强 x=0.1 的招牌 = alpha ×0.1 整
            // 牌隐形（"LED 不透明度塌了"主嫌疑，§12.5 的 decalLightNeonTube
            // 原文是 rgb += 高光，无 alpha 缩放；供电调光 Disabled 段已做）。
            Family::Sdf => &[
                "decalLightBackground",
                "decalAnimateSDFDisabled",
                "decalAnimateSDFDarken",
                "decalLightSDF",
            ],
        }
    }

    /// 家族专属前奏：链前需要的采样与共享变量声明。
    fn ps_prelude(self) -> &'static str {
        match self {
            // 量化合成自采样自上色；decalClip(Quad)/NeonBrighten 自含——无需共享
            Family::Sign | Family::Clip | Family::Holo => "",
            // 破洞链前奏（2026-10-05 片段重组定谳，shader_frags_decalInteriorMap.txt
            // 逐字对拍）：
            // 1) [379] decalLightInteriorMap 前半——SimCityLighting 场景光照，
            //    破洞纹理（decalProject 的采样，PE 由投影几何镜像 UV 携带 =
            //    引擎 uv = texpos×-0.5+0.5）先受光：
            //    `Current.color.rgb *= shColorDiff + shColorSpec + spec`。
            //    此前缺失本步 → 破洞纹理平涂不受光（夜间不暗、与墙面脱节）。
            // 2) [380] decalInteriorMap 消费的 shColorDiff/shColorSpec/spec/
            //    bumpNormal/decalTexture 在此备齐；textureFloatPosition 由
            //    VS_HOLE_PROJECTED 经 vTexcoord0 供给（z = 盒内真实进深：
            //    立面 −1 全尺寸内景、深部结构 +1 中心收缩 = 纵深）。
            Family::Hole => r#"
vec3 shColorDiff = vec3(0.0);
vec3 shColorSpec = vec3(0.0);
vec3 spec = vec3(0.0);
vec3 bumpNormal = normalize(uDecalWorldDirection);
SimCityLighting(bumpNormal, worldCameraDirection, gloss, gloss, specE, specStrength, shColorDiff, shColorSpec, spec);
vec4 decalTexture = texture2D(uSampler0, vUv);
outColor = decalTexture;
outColor.rgb *= shColorDiff + shColorSpec + spec;
"#,
            // SDF 链：Current.color 初值 = SDF 距离场四通道（addOverlay 的
            // sdfDists 来源）；uvOrig 供 decalLightBackground 选轴比较。
            // 场景光变量（shColorDiff/bumpNormal 等）由 decalLightSDF 片段
            // 自声明，前奏不得重复声明（GLSL 重复定义即编译失败）。
            // **sharp-bilinear 保级锐化**（七轮对拍"只剩字体/字体不全"的
            // 修复，取代 0.5 单阈值二值化）：这些贴图是 4~5 级量化掩码，
            // 中间级别承载背景面板（G 0.6/B 0.65 青底）与油泵图标
            // （R 0.29 二级暗区）——二值化把它们全压成 0，动画招牌只剩
            // 字体。改为 UV 域锐化：把双线性过渡带压缩到 ~1 屏幕像素
            // （斜率 = 每纹素的屏幕像素数），平台级别原样保留（0.29 仍是
            // 0.29），与级别取值无关——字形锐利且所有级别完整存活。
            // 缩小时（pxPerTexel 小于 1）钳回 1 = 退化为普通双线性。
            // coverageA 暂存覆盖率通道（A = 覆盖掩码），供收尾 alpha。
            Family::Sdf => r#"
vec2 uvOrig = vTexcoord0.xy * 0.5 + 0.5;
vec2 sdfTc = uvOrig * uSdfTexSize - 0.5;
vec2 sdfBase = floor(sdfTc);
vec2 sdfFrac = sdfTc - sdfBase;
vec2 sdfSharp = clamp(fwidth(uvOrig) * uSdfTexSize, vec2(1.0), vec2(32.0));
sdfFrac = clamp((sdfFrac - 0.5) * sdfSharp + 0.5, 0.0, 1.0);
outColor = texture2D(uSampler0, (sdfBase + 0.5 + sdfFrac) / uSdfTexSize);
float texturePositionZ = 0.0;
#define texturePosition vec3(vTexcoord0.xy, texturePositionZ)
"#,
        }
    }

    /// PS 尾部：写出前的家族专属收尾。
    fn ps_tail(self) -> &'static str {
        match self {
            // 破洞纹理 = sRGB 颜色纹理（无 Color1-4，GPU 采样自动转线性），
            // 而 ShaderMaterial 不做输出色彩空间编码——线性直出整体变暗
            // ~2.2 gamma（焦痕灰 → 纯黑，2026-10-04"破洞完全黑色"根因）。
            // 手动回 sRGB。sign/holo 纹理为 NoColorSpace 直采直出，无需补偿。
            Family::Hole => {
                "outColor.rgb = pow(max(outColor.rgb, vec3(0.0)), vec3(1.0 / 2.2));\ngl_FragColor = outColor;\n"
            }
            // 直采族：引擎 decal 走延迟光照（夜间只剩环境项），平涂 shader
            // 无光照响应——挂 env 昼夜因子（与 sign 的 uNightBoost 同机制）。
            Family::Clip => {
                "outColor.rgb *= uNightBoost;\ngl_FragColor = outColor;\n"
            }
            // 静态招牌灯箱自发光（七轮对拍"无动画招牌平涂无灯箱感"的
            // 修复）：引擎 decalMaterialInfo.x → materialLightScale =
            // x×16+0.25（casino 0.4 → 6.65，封 4 防过曝）；无 lot 数据
            // （x=0）时钳到 1 = 保持原样，涂鸦等同链条目不受增益影响。
            // 夜间 60% 豁免 nightBoost——灯箱夜间保持亮（引擎 decal 走
            // 延迟光照，招牌属自发光件）。软肩保色相：峰值 ≤1 不动，
            // 超出部分等比压缩到 1（防 ×4 增益饱和成白块）。
            Family::Sign => {
                "float signGain = clamp(decalMaterialInfo.x * 16.0 + 0.25, 1.0, 4.0);\n\
                 outColor.rgb *= signGain * mix(1.0, uNightBoost, 0.4);\n\
                 float signMax = max(outColor.r, max(outColor.g, outColor.b));\n\
                 outColor.rgb /= 1.0 + max(signMax - 1.0, 0.0);\n\
                 gl_FragColor = outColor;\n"
            }
            // SDF 族收尾已随 compose() 的链尾 Reinhard 内联（保色相 + 覆盖率
            // alpha），此处走默认 gl_FragColor 直出。
            _ => "gl_FragColor = outColor;\n",
        }
    }

    fn needs_lighting(self) -> bool {
        matches!(self, Family::Sign | Family::Sdf | Family::Hole)
    }
}

/// 组合一个家族，返回 (vert, frag) GLSL 全文。
pub fn compose(family: Family) -> anyhow::Result<(String, String)> {
    let mut ps = String::from(PS_PREAMBLE);
    if family == Family::Hole {
        // 内景自发光峰值：引擎常量 16 是 HDR 值（靠 tonemap 回收）；PE 无 HDR
        // 管线，挂 env.glow（白天 2.5 / 夜间 16，与建筑内景链同源，
        // 2026-09-30"恒 16 白天过曝"对拍结论）。以 #define 覆盖片段内的
        // 引擎常量名，保持 decalLightInteriorMap 片段逐字。
        ps.push_str("uniform float uInteriorGlow;\n#define kInteriorMapSelfLightMax uInteriorGlow\n");
        // 视线视差（2026-10-05 四轮，用户"破洞各角度完全一样"对拍）：引擎
        // 原版靠**场景内真实几何**（破洞后露出的楼板）让 tfp.z 逐像素变化
        // 产生纵深；PE 建筑模型是空壳（无内部几何）→ z 恒为立面深度 →
        // 内景全尺寸平贴。此处改用与窗户 interior mapping 同族的
        // **视线射线-盒底平面求交**：从立面像素沿视线射到盒内虚拟后墙
        // （z=depth），采样命中点——直视时与原版公式完全一致，斜视时内景
        // 随视角移动（= 游戏窗户假内景的观感）。
        // vObjPos/uHoleCamLot 同为 mesh 局部（= lot 局部）空间；
        // uHoleInvRot = 贴花基矩阵转置（lot 局部 → 贴花系）。
        ps.push_str(
            "varying vec3 vObjPos;\n\
             uniform vec2 uHoleHalfXY;\n\
             uniform float uHoleDepthM;\n\
             uniform mat3 uHoleInvRot;\n\
             uniform vec3 uHoleCamLot;\n\
             vec2 holeParallaxUv(vec3 tfp) {\n\
             \x20 vec3 dirLot = normalize(vObjPos - uHoleCamLot);\n\
             \x20 vec3 dir = uHoleInvRot * dirLot;\n\
             \x20 float zm = (tfp.z * 0.5 + 0.5) * uHoleDepthM;\n\
             \x20 float s = max((uHoleDepthM - zm) / max(dir.z, 0.05), 0.0);\n\
             \x20 vec2 hit = tfp.xy * uHoleHalfXY + dir.xy * s;\n\
             \x20 vec2 norm = hit / uHoleHalfXY;\n\
             \x20 return norm * -0.5 + 0.5;\n\
             }\n",
        );
    }
    if family.needs_lighting() {
        ps.push_str(&translate(fragments::get("SimCityLighting").ok_or_else(|| anyhow::anyhow!("缺 SimCityLighting"))?));
        ps.push('\n');
    }
    ps.push_str("void main() {\n");
    ps.push_str(family.ps_prelude());
    for name in family.ps_chain() {
        let src = fragments::get(name).ok_or_else(|| anyhow::anyhow!("缺片段 {name}"))?;
        ps.push_str(&translate(src));
        ps.push('\n');
    }
    if family == Family::Sdf {
        // 十二轮回归单路径（用户 DIRTY FACTORY 对拍：静态量化合成对 SDF
        // 距离场纹理失效——距离场通道大片 ≥0.5，优先级阈值链把全图归到
        // 最高优先级行 → 红洗。sdf 族纹理不是 sign 族那种多级量化掩码）。
        // 单一路径 = LED addOverlay 管线全程生效，uAnimEnabled 只门控
        // 扫掠调光（decalAnimateSDFDisabled 内 powerFactor = mix(1.0,
        // powerFactor, uAnimEnabled)：关 = 恒 1 静态全亮 addOverlay，
        // 开 = 暗→亮跑马灯）。颜色/遮罩两态一致，不会再双轨漂移。
        // 收尾保色相 Reinhard；alpha 已由 addOverlay 段写为四通道
        // 遮罩 max = 覆盖区不透明、暗态图案不被墙面底色冲淡。
        ps.push_str(
            "float scMax = max(outColor.r, max(outColor.g, outColor.b));\n\
             outColor.rgb /= 1.0 + scMax;\n",
        );
    }
    ps.push_str(family.ps_tail());
    ps.push_str("}\n");

    let vs = match family {
        Family::Hole => VS_HOLE_PROJECTED.to_string(),
        _ => VS_PREAMBLE.to_string(),
    };
    Ok((vs, ps))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_families_compose() {
        for f in Family::ALL {
            let (vs, ps) = compose(f).unwrap_or_else(|e| panic!("{:?}: {e}", f));
            assert!(vs.contains("gl_Position"), "{f:?}");
            assert!(ps.contains("gl_FragColor"), "{f:?}");
            assert!(ps.contains("void main() {"), "{f:?} 缺 main 包装");
            assert!(!ps.contains("Current.color"), "{f:?} 未转译干净");
            assert!(!ps.contains("saturate("), "{f:?} saturate 未转 clamp");
            assert!(!ps.contains("half "), "{f:?} half 未转 float");
            assert!(!ps.contains("float3"), "{f:?} float3 未转 vec3");
            assert!(!ps.contains("float4"), "{f:?} float4 未转 vec4");
        }
    }

    #[test]
    fn hole_chain_has_self_light() {
        let (_, ps) = compose(Family::Hole).unwrap();
        assert!(ps.contains("kInteriorMapSelfLightMax"));
        assert!(ps.contains("interiorUv"));
        // 自发光峰值挂 env.glow（白天 2.5 / 夜间 16），不硬编码引擎 HDR 常量
        assert!(ps.contains("#define kInteriorMapSelfLightMax uInteriorGlow"));
        // 视线视差（空壳建筑的纵深来源）：射线-盒底平面求交函数已注入
        assert!(ps.contains("holeParallaxUv(textureFloatPosition)"));
        assert!(ps.contains("uniform mat3 uHoleInvRot;"));
        assert!(ps.contains("varying vec3 vObjPos;"));
    }

    #[test]
    fn hole_uses_projected_vs_and_srgb_tail() {
        let (vs, ps) = compose(Family::Hole).unwrap();
        // 投影 VS：vTexcoord0.z = 盒内真实进深（引擎延迟路径深度重建的 VS
        // 同构：立面 −1 全尺寸内景、深部 +1 中心收缩 = 破洞内部结构）；
        // z 恒 +1 的旧版只采贴图中心 1/4 放大 = "黑色平斑"根因
        assert!(vs.contains("holeDepth * 2.0 - 1.0"));
        assert!(vs.contains("uHoleOrigin"));
        assert!(vs.contains("uHoleInvDepth"));
        assert!(!vs.contains("uBoxHalf"), "hole VS 残留体积盒 uniform");
        // sRGB 纹理线性采样 → 输出手动回 sRGB（否则破洞整体变暗 ~2.2 gamma）
        assert!(ps.contains("pow(max(outColor.rgb"), "hole PS 缺 sRGB 收尾");
    }

    #[test]
    fn hole_prelude_lights_base_texture() {
        let (_, ps) = compose(Family::Hole).unwrap();
        // [379] decalLightInteriorMap 前半：场景光照 + 破洞纹理先受光
        // （此前缺失 = 破洞纹理平涂不受光、夜间与墙面脱节）
        assert!(ps.contains("SimCityLighting(bumpNormal"));
        assert!(ps.contains("outColor.rgb *= shColorDiff + shColorSpec + spec"));
        // 基色采样走投影几何镜像 UV（= 引擎 uv = texpos×-0.5+0.5）
        assert!(ps.contains("texture2D(uSampler0, vUv)"));
        // texXform 必须有 #define 对齐（缺失 = GLSL 未声明标识符编译失败）
        assert!(ps.contains("#define texXform uTexXform"));
    }

    #[test]
    fn non_hole_ps_has_no_srgb_tail() {
        // sign/holo 纹理 NoColorSpace 直采直出，不需要 gamma 补偿
        let (_, ps) = compose(Family::Sign).unwrap();
        assert!(!ps.contains("pow(max(outColor.rgb"));
    }

    #[test]
    fn clip_chain_direct_sample_with_night_boost() {
        let (_, ps) = compose(Family::Clip).unwrap();
        // 直采族：vUv 直采（PE quad 几何 UV 已携带引擎镜像）+ 昼夜因子
        assert!(ps.contains("texture2D(uSampler0, vUv)"));
        assert!(ps.contains("outColor.rgb *= uNightBoost"));
        // 不得含量化合成（焦痕误走量化链 = 层色纯黑的根因）
        assert!(!ps.contains("uLayerColors[3]"));
    }

    #[test]
    fn sign_chain_no_unresolved_engine_symbols() {
        let (_, ps) = compose(Family::Sign).unwrap();
        // 引擎符号全部经 #define 或声明对齐（不出现裸引用即无编译错误主因）
        assert!(ps.contains("#define decalMaterialData uDecalMaterialData"));
        assert!(ps.contains("clamp("));
    }

    #[test]
    fn sign_tail_lightbox_glow() {
        let (_, ps) = compose(Family::Sign).unwrap();
        // 静态招牌灯箱自发光：materialLightScale = x×16+0.25（封 4）+
        // 夜间 60% 豁免 nightBoost + 保色相软肩
        assert!(ps.contains("signGain"), "sign 缺灯箱自发光增益");
        assert!(
            ps.contains("mix(1.0, uNightBoost, 0.4)"),
            "sign 缺夜间豁免（灯箱夜间应保持亮）"
        );
        assert!(
            !ps.contains("col * uNightBoost"),
            "量化链片段残留夜压（与收尾叠加 = 双重压暗）"
        );
    }

    #[test]
    fn sdf_chain_full_neon_pipeline() {
        let (_, ps) = compose(Family::Sdf).unwrap();
        // 完整霓虹链：动画背景(uTime 跑马灯) → 灯管调光 → SDF 球面衰减 →
        // 场景光叠加 → 供电开关 → 亮部 alpha 收尾
        assert!(ps.contains("fract(uTime"), "sdf 缺 uTime 动画时钟");
        assert!(ps.contains("uDecalNUS"), "sdf 缺盒尺寸 uniform");
        assert!(ps.contains("materialTubeLightFactor"), "sdf 缺灯管调光段");
        assert!(ps.contains("tubeColor0"), "sdf 缺调光后灯管色");
        assert!(
            ps.contains("outColor.rgb /= 1.0 + scMax"),
            "sdf 缺保色相 Reinhard 收尾"
        );
        // addOverlay 遮罩上色（十一轮重写，dev decalSDF[394] 原版语义：
        // 四通道遮罩 + fwidth AA + 调光权重列加权求和，无 one-hot 分类）
        assert!(
            ps.contains("dot(tubeColor0, sdfMask)"),
            "sdf 缺 addOverlay 遮罩上色（dev decalSDF 原版语义）"
        );
        assert!(
            ps.contains("fwidth(outColor)"),
            "sdf 遮罩缺 fwidth 抗锯齿（官方清晰度答案，§12.6）"
        );
        assert!(
            ps.contains("max(max(sdfMask.x, sdfMask.y)"),
            "sdf 缺遮罩 max alpha（覆盖区不透明）"
        );
        assert!(
            !ps.contains("sdfRow0"),
            "sdf 残留 one-hot 调色板归属（纯色块无图案的根因，十一轮已移除）"
        );
        assert!(
            !ps.contains("sphereDistsSqr"),
            "sdf 残留球面衰减（动态色块无细节的复发点）"
        );
        // 十二轮单路径：LED addOverlay 全程生效，uAnimEnabled 只门控扫掠
        // （静态量化合成分支对 SDF 距离场纹理红洗失效，已删）
        assert!(ps.contains("uAnimEnabled"), "sdf 缺动态招牌开关");
        assert!(
            ps.contains("mix(vec4(1.0, 1.0, 1.0, 1.0), powerFactor"),
            "sdf 缺 powerFactor 门控（uAnimEnabled=0 → 恒 1 静态全亮）"
        );
        assert!(
            !ps.contains("if (uAnimEnabled > 0.5) {"),
            "sdf 残留双分支（十二轮已回归单路径）"
        );
        assert!(
            !ps.contains("signGainS"),
            "sdf 残留静态量化合成灯箱收尾（红洗根因，十二轮已删）"
        );
        assert!(
            ps.contains("vec4(0.35, 0.35, 0.35, 0.35)"),
            "sdf 暗态下限未提至 0.35（PE 缺引擎光晕 pass 的补偿）"
        );
        assert!(
            ps.contains("smoothstep(vec4(-0.02"),
            "sdf 缺扫掠软边（暗到亮渐变的复发点）"
        );
        assert!(
            ps.contains("uSdfTexSize"),
            "sdf 缺贴图尺寸 uniform（sharp-bilinear 的纹素坐标输入）"
        );
        assert!(
            !ps.contains("smoothstep(vec4(0.5)"),
            "sdf 残留 0.5 二值化（会压没中间级别）"
        );
        // 场景光变量只许 decalLightSDF 片段声明一次（前奏重复声明 = 编译失败；
        // SimCityLighting 的 inout 形参不含初始化式，用初始化式计数）
        assert_eq!(
            ps.matches("vec3 shColorDiff = vec3(0, 0, 0)").count(),
            1,
            "shColorDiff 重复声明"
        );
        // 动画比较量已 #undef 后落局部变量（不再吃 CPU 端 uniform）
        assert!(ps.contains("#undef animResults"), "sdf 缺动画量局部化");
        // uniform 只允许全局作用域（前奏拼进 main 体，声明必须进序言）
        let body = ps.split("void main() {").nth(1).unwrap();
        assert!(
            !body.contains("uniform "),
            "main 体内出现 uniform 声明（GLSL 编译错误）"
        );
    }
}
