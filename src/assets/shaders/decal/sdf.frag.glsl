// == 由 sc-shader 组合器生成：引擎符号对齐 + three.js 相容序言 ==
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
uniform float uGraffiti;

varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;

vec4 outColor = vec4(0.0);
void SimCityLighting(vec3 normal, vec3 viewVector, float glossyStrength,
float reflectance, float specE, float specStrength,
inout vec3 shColorDiff, inout vec3 shColorSpec,
inout vec3 specHighlight)
{
float  sunMod = clamp(dot(uSunDir3, normal), 0.0, 1.0);
shColorDiff = mix(uAmbientDiff, uSunColor3, sunMod * uDayLight);
shColorSpec = uSunColor3 * uSpecularScale;
vec3 halfVector = normalize(uSunDir3 - viewVector);
float nDotH = clamp(dot(normal, halfVector), 0.0, 1.0);
float spec = pow(nDotH, specE);
specHighlight = spec * specStrength * sunMod * uSunColor3;
}
void main() {

vec2 uvOrig = vTexcoord0.xy * 0.5 + 0.5;
outColor = texture2D(uSampler0, uvOrig);
float texturePositionZ = 0.0;
#define texturePosition vec3(vTexcoord0.xy, texturePositionZ)
#undef animResults
#undef useV
vec4 animParameters = decalMaterialData[3];
vec4 useV = vec4(animParameters.x > 0.0 ? 0.0 : 1.0,
animParameters.y > 0.0 ? 0.0 : 1.0,
animParameters.z > 0.0 ? 0.0 : 1.0,
animParameters.w > 0.0 ? 0.0 : 1.0);
animParameters = abs(animParameters) + 0.0001;
float animTime = fract(uTime * decalMaterialInfo.y + 0.9999);
vec4 animOffsets = fract(animParameters);
vec4 animChunks = max(vec4(1.0, 1.0, 1.0, 1.0), floor(animParameters));
vec4 compares = floor((animTime * 3.0 - animOffsets) * animChunks) * (1.0 / animChunks);
vec4 uvCompare = mix(vec4(uvOrig.x, uvOrig.x, uvOrig.x, uvOrig.x),
vec4(uvOrig.y, uvOrig.y, uvOrig.y, uvOrig.y), useV);
vec4 animResults = uvCompare - compares;
float materialTubeLightFactor = decalMaterialInfo.z * 8.0 + 1.0;
vec4 lightFactor = mix(vec4(materialTubeLightFactor, materialTubeLightFactor,
materialTubeLightFactor, materialTubeLightFactor),
vec4(0.35, 0.35, 0.35, 0.35),
smoothstep(vec4(-0.02, -0.02, -0.02, -0.02),
vec4(0.15, 0.15, 0.15, 0.15), animResults));
vec4 powerFactor = mix(vec4(0.5, 0.5, 0.5, 0.5), lightFactor, decalMaterialInfo.wwww);
powerFactor = mix(vec4(1.0, 1.0, 1.0, 1.0), powerFactor, vec4(uAnimEnabled, uAnimEnabled, uAnimEnabled, uAnimEnabled));
vec4 tubeColor0 = decalMaterialData[0] * powerFactor;
vec4 tubeColor1 = decalMaterialData[1] * powerFactor;
vec4 tubeColor2 = decalMaterialData[2] * powerFactor;

      vec4 sdfMask = vec4(greaterThan(outColor, vec4(0.5)));
      float remaining = 1.0 - sdfMask.w;
      sdfMask.z = min(sdfMask.z, remaining);
      remaining -= sdfMask.z;
      sdfMask.y = min(sdfMask.y, remaining);
      remaining -= sdfMask.y;
      sdfMask.x = min(sdfMask.x, remaining);
      outColor.rgb = vec3(dot(tubeColor0, sdfMask), dot(tubeColor1, sdfMask), dot(tubeColor2, sdfMask));
      outColor.a = dot(sdfMask, vec4(1.0));
vec3 bumpNormal = normalize(decalWorldDirection);
vec3 shColorDiff = vec3(0, 0, 0);
vec3 shColorSpec = vec3(0, 0, 0);
vec3 spec = vec3(0, 0, 0);
SimCityLighting(bumpNormal, worldCameraDirection.xyz, gloss, reflectance,
specE, specStrength, shColorDiff, shColorSpec, spec);
outColor.rgb += shColorSpec + spec;
float scMax = max(outColor.r, max(outColor.g, outColor.b));
outColor.rgb /= 1.0 + max(scMax - 1.0, 0.0);
gl_FragColor = outColor;
}
