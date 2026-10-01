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
uniform vec4 uLayerColors[4];
uniform float uNightBoost;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// 招牌/涂鸦（decalProject 系标准链：直采 + 可选 NeonBrighten）
    Sign,
    /// 破洞内景（decalInteriorMap 系）
    Hole,
    /// 全息浮空（decalFloatQuad 系）
    Holo,
    /// SDF 霓虹管（decalAnimateSDF 系）
    Sdf,
}

impl Family {
    pub const ALL: [Family; 4] = [Family::Sign, Family::Hole, Family::Holo, Family::Sdf];

    pub fn name(self) -> &'static str {
        match self {
            Family::Sign => "sign",
            Family::Hole => "hole",
            Family::Holo => "holo",
            Family::Sdf => "sdf",
        }
    }

    /// PS 链（按序拼接——引擎的执行顺序即数据流）。
    fn ps_chain(self) -> &'static [&'static str] {
        match self {
            Family::Sign => &["decalQuantComposite"],
            Family::Hole => &["kInteriorMapSelfLightMax", "decalLightInteriorMap"],
            Family::Holo => &["decalClip", "decalFloatQuadNoClip"],
            Family::Sdf => &["decalAnimateSDFDarken", "decalLightSDF", "decalLightNeonTube"],
        }
    }

    /// 家族专属前奏：链前需要的采样与共享变量声明。
    fn ps_prelude(self) -> &'static str {
        match self {
            // 量化合成自采样自上色；decalClip/NeonBrighten 自含——无需共享
            Family::Sign | Family::Holo => "",
            // interiorMap 读链上首采的 decalTexture + 场景光变量 + 法线
            Family::Hole => r#"
vec3 shColorDiff = vec3(0.0);
vec3 shColorSpec = vec3(0.0);
vec3 spec = vec3(0.0);
vec3 bumpNormal = normalize(uDecalWorldDirection);
vec4 decalTexture = texture2D(uSampler0, vTexcoord0.xy * 0.5 + 0.5);
outColor = decalTexture; // 引擎链上 decalClip 先采样（lerp 基色）
"#,
            // SDF 链：Current.color 初值（sdfDists 四通道）+ 动画输入 + 场景光
            Family::Sdf => r#"
vec3 shColorDiff = vec3(0.0);
vec3 shColorSpec = vec3(0.0);
vec3 spec = vec3(0.0);
vec3 bumpNormal = normalize(uDecalWorldDirection);
outColor = texture2D(uSampler0, vTexcoord0.xy * 0.5 + 0.5);
float texturePositionZ = 0.0;
#define texturePosition vec3(vTexcoord0.xy, texturePositionZ)
"#,
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
    ps.push_str("gl_FragColor = outColor;\n");
    ps.push_str("}\n");

    Ok((VS_PREAMBLE.to_string(), ps))
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
    fn sign_chain_no_unresolved_engine_symbols() {
        let (_, ps) = compose(Family::Sign).unwrap();
        // 引擎符号全部经 #define 或声明对齐（不出现裸引用即无编译错误主因）
        assert!(ps.contains("#define decalMaterialData uDecalMaterialData"));
        assert!(ps.contains("clamp("));
    }
}
