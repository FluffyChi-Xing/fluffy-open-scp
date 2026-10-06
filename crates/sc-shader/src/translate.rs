//! HLSL 子集 → GLSL（ES 1.00 风格，与 three.js ShaderMaterial 默认一致）。
//!
//! 片段只使用有限语法：floatN/halfN、mul、tex2D、lerp、saturate、dot、
//! clip、frac、max/min/pow/normalize/sqrt/length/abs、for、if。转译规则：
//!
//! | HLSL                | GLSL                          |
//! |---------------------|-------------------------------|
//! | float4/half4        | vec4                          |
//! | mul(a,b)            | (a*b)（矩阵列由声明侧保证）   |
//! | tex2D(Sampler<sN>,u)| texture2D(uSamplerN, u)       |
//! | saturate(x)         | clamp(x, 0.0, 1.0)            |
//! | frac(x)             | fract(x)                      |
//! | clip(x)             | if ((x) < 0.0) discard;       |
//! | lerp/rsqrt          | mix/inversesqrt               |
//! | Current.color       | outColor                      |
//! | In.texcoord<tN>     | vTexcoordN                    |
//! | 引擎符号             | uniform 注入（见 SYMBOLS）    |

/// 引擎符号 → GLSL 声明（uniform / varying），组合器按需注入。
pub const SYMBOLS: &[(&str, &str)] = &[
    // 数据装载（PS uniform）
    ("decalMaterialData", "uniform vec4 uDecalMaterialData[4];\n#define decalMaterialData uDecalMaterialData"),
    ("decalMaterialInfo", "uniform vec4 uDecalMaterialInfo;\n#define decalMaterialInfo uDecalMaterialInfo"),
    ("texXform", "uniform vec4 uTexXform;\n#define texXform uTexXform"),
    ("decalWorldDirection", "uniform vec3 uDecalWorldDirection;\n#define decalWorldDirection uDecalWorldDirection"),
    // 引擎环境（前向近似输入）
    ("sunSky", "uniform vec4 uSunDir;\nuniform vec4 uSunColor;\nstruct cSunSkyInfo { vec4 mSunDir; vec4 mSunColor; };\n#define sunSky uSunSkyInfo\nuniform cSunSkyInfo uSunSkyInfo;"),
    ("worldNormal", "uniform vec3 uWorldNormal;\n#define worldNormal uWorldNormal"),
    ("worldCameraDirection", "uniform vec3 uWorldCameraDirection;\n#define worldCameraDirection uWorldCameraDirection"),
    ("shadow", "uniform float uShadow;\n#define shadow uShadow"),
    // 光照辅助常量（decalNeonBrighten/decalLightSDF 的标量入参）
    ("gloss", "uniform float uGloss;\n#define gloss uGloss"),
    ("reflectance", "uniform float uReflectance;\n#define reflectance uReflectance"),
    ("specE", "uniform float uSpecE;\n#define specE uSpecE"),
    ("specStrength", "uniform float uSpecStrength;\n#define specStrength uSpecStrength"),
    ("uAmbientDiff", "uniform vec3 uAmbientDiff;"),
    ("uDayLight", "uniform float uDayLight;"),
    ("uSpecularScale", "uniform float uSpecularScale;"),
    // 采样器
    ("Sampler<s0>", "uniform sampler2D uSampler0;"),
    // varying
    ("In.texcoord<t0>", "vTexcoord0"),
    ("In.texcoord4", "vTexcoord4"),
    ("In.texcoord5", "vTexcoord5"),
    // 动画输入（SDF 族）
    ("animResults", "uniform vec4 uAnimResults;\n#define animResults uAnimResults"),
    ("useV", "uniform vec4 uUseV;\n#define useV uUseV"),
    ("animRatio", "uniform float uAnimRatio;\n#define animRatio uAnimRatio"),
    ("texturePosition", "vTexcoord0.xyz"),
];

/// 单片段转译。
pub fn translate(hlsl: &str) -> String {
    let mut out = hlsl.to_string();

    // 1. 类型映射（词边界）
    for (from, to) in [
        ("float4x4", "mat4"),
        ("float3x4", "mat43"), // 占位：组合器负责以 mat4+列裁剪呈现
        ("float4", "vec4"),
        ("float3", "vec3"),
        ("float2", "vec2"),
        ("half4", "vec4"),
        ("half3", "vec3"),
        ("half2", "vec2"),
        ("half ", "float "),
    ] {
        out = replace_word(&out, from.trim_end(), to);
    }

    // 2. 采样器
    out = out.replace("tex2D(Sampler<s0>", "tex2D(uSampler0");

    // 3. clip(x) → discard
    // 源片段的 clip 均带尾分号——替换体不再加分号避免双写
    out = replace_call(&out, "clip", &|args| format!("if (({}) < 0.0) discard", args));

    // 4. saturate/frac
    out = replace_call(&out, "saturate", &|a| format!("clamp({}, 0.0, 1.0)", a));
    out = replace_call(&out, "frac", &|a| format!("fract({})", a));
    out = replace_call(&out, "rsqrt", &|a| format!("inversesqrt({})", a));

    // 5. mul(a,b) → (a*b)（双参路径）
    out = replace_call2(&out, "mul", &|a, b| {
        if b.is_empty() {
            format!("({a})")
        } else {
            format!("({a} * {b})")
        }
    });

    // 6. lerp → mix（词替换，参数兼容）
    out = replace_word(&out, "lerp", "mix");

    // 7. In.texcoord<tN> → varying
    for n in 0..8 {
        out = out.replace(&format!("In.texcoord<t{n}>"), &format!("vTexcoord{n}"));
    }
    out = out.replace("In.texcoord4", "vTexcoord4").replace("In.texcoord5", "vTexcoord5");

    // 8. Current.color → outColor
    out = replace_word(&out, "Current.color", "outColor");

    // 9. 引擎符号经 SYMBOLS 的 #define 宏对齐（组合器注入），此处不做词替换
    // 10. tex2D → texture2D（GLSL1 一致写法）；GLSL1 无 static 关键字
    out = replace_word(&out, "tex2D", "texture2D");
    out = replace_word(&out, "static", "");
    // 11. sunSky 结构成员 → 扁平 uniform（three.js Vector3 直传）
    out = out.replace("sunSky.mSunDir.xyz", "uSunDir3");
    out = out.replace("sunSky.mSunColor.rgb", "uSunColor3");
    // HLSL 允许 uniform 派生的局部 const；GLSL ES 要求常量表达式。
    for name in ["kSunContributionAmount", "kLightAmount"] {
        out = out.replace(
            &format!("const float {name} = decalMaterialData"),
            &format!("float {name} = decalMaterialData"),
        );
    }

    out
}

/// 词边界替换（前后非 [A-Za-z0-9_]）。
fn replace_word(s: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with(from) {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            let after = i + from.len();
            let after_ok = after >= s.len() || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
            if before_ok && after_ok {
                out.push_str(to);
                i = after;
                continue;
            }
        }
        out.push(s[i..].chars().next().unwrap());
        i += s[i..].chars().next().unwrap().len_utf8();
    }
    out
}

/// 函数调用替换（支持 1-2 参，含嵌套括号）。
fn replace_call(s: &str, func: &str, f: &dyn Fn(&str) -> String) -> String {
    replace_call2(s, func, &|a, _b| f(a))
}

fn replace_call2(s: &str, func: &str, f: &dyn Fn(&str, &str) -> String) -> String {
    let mut out = String::with_capacity(s.len());
    let pat = format!("{func}(");
    let mut rest = s;
    while let Some(pos) = rest.find(&pat) {
        // 词边界：func 前不能是标识符字符
        let before_ok = pos == 0
            || !rest[..pos]
                .chars()
                .last()
                .map(|c| c.is_ascii_alphanumeric() || c == '_')
                .unwrap_or(false);
        if !before_ok {
            let (a, b) = rest.split_at(pos + func.len());
            out.push_str(a);
            rest = b;
            continue;
        }
        let start = pos + pat.len();
        let bytes = rest.as_bytes();
        let mut depth = 1;
        let mut i = start;
        let mut last_comma = None;
        while i < bytes.len() && depth > 0 {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b',' if depth == 1 => last_comma = Some(i),
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            // 括号不配对（截断源码）——原样保留并停机
            out.push_str(rest);
            return out;
        }
        let args = &rest[start..i];
        let replacement = match last_comma {
            Some(c) => f(args[..c - start].trim(), args[c + 1 - start..].trim()),
            None => f(args.trim(), ""),
        };
        out.push_str(&rest[..pos]);
        out.push_str(&replacement);
        rest = &rest[i + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_and_intrinsics() {
        let glsl = translate("float4 c = saturate(lerp(a, b, t)) * mul(m, float4(p,1));");
        if !glsl.contains("vec4 c = clamp(mix(a, b, t), 0.0, 1.0) * (m * vec4(p,1))") {
            panic!("实际输出: {glsl}");
        }
    }

    #[test]
    fn clip_to_discard() {
        let glsl = translate("clip(-x);");
        if glsl.trim() != "if ((-x) < 0.0) discard;" {
            panic!("实际输出: [{}]", glsl.trim());
        }
    }

    #[test]
    fn nested_calls() {
        let glsl = translate("float a = saturate(lerp(f(x), g(y), 0.5));");
        assert!(glsl.contains("clamp(mix(f(x), g(y), 0.5), 0.0, 1.0)"));
    }

    #[test]
    fn texcoords_and_output() {
        let glsl = translate("float2 uv = In.texcoord<t0>.xy; Current.color = c;");
        if !(glsl.contains("vTexcoord0.xy") && glsl.contains("outColor = c;")) {
            panic!("实际输出: {glsl}");
        }
    }

    #[test]
    fn word_boundary_lerp_not_matched_inside_identifier() {
        let glsl = translate("float lerpXParam = 1;");
        assert!(glsl.contains("lerpXParam")); // 未被词替换破坏
    }
}
