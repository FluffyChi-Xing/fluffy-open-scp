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

/// 破洞体积盒 VS（2026-10-04 补）：盒局部坐标归一化 → 引擎纹理空间。
/// quad VS 把 vTexcoord0.z 恒置 0，体积盒拿不到真实进深 →
/// decalLightInteriorMap 的视差收缩（lerp 因子 = z×0.5+0.5）退化为常量 0.5，
/// 是"假内景未实装"观感的直接根因；此处 z = 盒内归一进深（前 −1 → 后 +1）。
pub const VS_HOLE_VOLUME: &str = r#"// == 由 sc-shader 组合器生成：破洞体积盒 VS（three.js 相容）==
varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;
uniform vec3 uBoxHalf; // (半宽, 半高, 半深)
void main() {
  vUv = uv;
  // x 取负对齐引擎 uv = texpos × -0.5 + 0.5 的镜像口径（与 quad 路径一致）。
  vec3 n = clamp(position / max(uBoxHalf, vec3(0.001)), -1.0, 1.0);
  vTexcoord0 = vec3(-n.x, n.y, n.z);
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
            Family::Hole => &["kInteriorMapSelfLightMax", "decalLightInteriorMap"],
            Family::Holo => &["decalClip", "decalFloatQuadNoClip"],
            Family::Sdf => &[
                "decalLightBackground",
                "decalAnimateSDFDisabled",
                "decalAnimateSDFDarken",
                "decalLightSDF",
                "decalLightNeonTube",
            ],
        }
    }

    /// 家族专属前奏：链前需要的采样与共享变量声明。
    fn ps_prelude(self) -> &'static str {
        match self {
            // 量化合成自采样自上色；decalClip(Quad)/NeonBrighten 自含——无需共享
            Family::Sign | Family::Clip | Family::Holo => "",
            // interiorMap 读链上首采的 decalTexture + 场景光变量 + 法线
            Family::Hole => r#"
vec3 shColorDiff = vec3(0.0);
vec3 shColorSpec = vec3(0.0);
vec3 spec = vec3(0.0);
vec3 bumpNormal = normalize(uDecalWorldDirection);
vec4 decalTexture = texture2D(uSampler0, vTexcoord0.xy * 0.5 + 0.5);
outColor = decalTexture; // 引擎链上 decalClip 先采样（lerp 基色）
"#,
            // SDF 链：Current.color 初值 = SDF 距离场四通道（Darken 的
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
float coverageA = outColor.a;
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
            // SDF 霓虹管 = 自发光 HDR（materialLightScale ×16+0.25、扫到处
            // 再 ×tubeFactor ≈ 25~55 倍增益）。引擎靠 hejl tonemap 软肩回收；
            // PE 无 HDR 曝光管线，硬钳 [0,1] 会把文字/面板的色相比（如
            // (13.3,9.3,5.4) 黄字 vs (24.6,5.7,3.2) 红面板）压成同色白块
            // ——六轮对拍"运动色块无图案"的根因。改用**保色相 Reinhard**
            // （rgb/(1+max)：单调、不破坏色相比、保留亮暗扫描对比）。
            // alpha 取覆盖率通道（前奏暂存的 coverageA）：霓虹面板在覆盖
            // 区内恒不透明，暗态图案不被墙面底色冲淡。
            Family::Sdf => {
                "float scMax = max(outColor.r, max(outColor.g, outColor.b));\n\
                 outColor.rgb /= 1.0 + scMax;\n\
                 outColor.a = coverageA;\n\
                 gl_FragColor = outColor;\n"
            }
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
    ps.push_str(family.ps_tail());
    ps.push_str("}\n");

    let vs = match family {
        Family::Hole => VS_HOLE_VOLUME.to_string(),
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
    }

    #[test]
    fn hole_uses_volume_vs_and_srgb_tail() {
        let (vs, ps) = compose(Family::Hole).unwrap();
        // 体积盒 VS：真实进深驱动视差（quad VS 的 z 恒 0 会让内景退化为平面）
        assert!(vs.contains("uBoxHalf"), "hole VS 缺盒体归一化 uniform");
        assert!(!vs.contains("vTexcoord0 = vec3(uv * 2.0 - 1.0, 0.0)"));
        // sRGB 纹理线性采样 → 输出手动回 sRGB（否则破洞整体变暗 ~2.2 gamma）
        assert!(ps.contains("pow(max(outColor.rgb"), "hole PS 缺 sRGB 收尾");
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
        assert!(ps.contains("outColor.a = coverageA"), "sdf 缺覆盖率 alpha");
        // sharp-bilinear 保级锐化（0.5 二值化压没背景面板/油泵的复发点）
        assert!(
            ps.contains("sdfSharp"),
            "sdf 缺 sharp-bilinear 保级锐化（只剩字体/字体不全的复发点）"
        );
        // 调色板归属解码（八轮对拍：球面衰减多通道串色 = 色块根因）
        assert!(ps.contains("sdfRow0"), "sdf 缺调色板归属解码");
        assert!(
            !ps.contains("sphereDistsSqr"),
            "sdf 残留球面衰减（动态色块无细节的复发点）"
        );
        // 动态/静态开关（静态 = powerFactor 恒 1 全亮，默认关）
        assert!(ps.contains("uAnimEnabled"), "sdf 缺动态招牌开关");
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
