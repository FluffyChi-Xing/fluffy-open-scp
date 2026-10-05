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
uniform float uInteriorGlow;
#define kInteriorMapSelfLightMax uInteriorGlow
varying vec3 vObjPos;
uniform vec2 uHoleHalfXY;
uniform float uHoleDepthM;
uniform mat3 uHoleInvRot;
uniform vec3 uHoleCamLot;
vec2 holeParallaxUv(vec3 tfp) {
  vec3 dirLot = normalize(vObjPos - uHoleCamLot);
  vec3 dir = uHoleInvRot * dirLot;
  float zm = (tfp.z * 0.5 + 0.5) * uHoleDepthM;
  float s = max((uHoleDepthM - zm) / max(dir.z, 0.05), 0.0);
  vec2 hit = tfp.xy * uHoleHalfXY + dir.xy * s;
  vec2 norm = hit / uHoleHalfXY;
  return norm * -0.5 + 0.5;
}
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

vec3 shColorDiff = vec3(0.0);
vec3 shColorSpec = vec3(0.0);
vec3 spec = vec3(0.0);
vec3 bumpNormal = normalize(uDecalWorldDirection);
SimCityLighting(bumpNormal, worldCameraDirection, gloss, gloss, specE, specStrength, shColorDiff, shColorSpec, spec);
vec4 decalTexture = texture2D(uSampler0, vUv);
outColor = decalTexture;
outColor.rgb *= shColorDiff + shColorSpec + spec;
float kSunContributionAmount = decalMaterialData[0].x;
float kLightAmount = decalMaterialData[0].y;
vec3 textureFloatPosition = vTexcoord0.xyz;
vec2 interiorUv = holeParallaxUv(textureFloatPosition);
interiorUv = interiorUv * texXform.xy + texXform.zw;
float sunMod = clamp(dot(uSunDir3, bumpNormal.xyz), 0.0, 1.0);
vec3 sunColor = sunMod * uSunColor3 * shadow;
shColorDiff -= sunColor;
shColorDiff *= kLightAmount;
shColorSpec *= kLightAmount;
shColorDiff += sunColor * kSunContributionAmount;
vec4 interiorTexture = texture2D(uSampler0, interiorUv);
vec3 interiorTextureLit = interiorTexture.rgb * (shColorDiff + shColorSpec + spec + interiorTexture.a * kInteriorMapSelfLightMax);
outColor.rgb = mix(outColor.rgb, interiorTextureLit, clamp(decalTexture.a * 2.0 - 1.0, 0.0, 1.0));
outColor.a = clamp(decalTexture.a * 2.0, 0.0, 1.0);
outColor.rgb = mix(outColor.rgb, 2.0 - 1.0 / max(outColor.rgb, vec3(1.0)), step(vec3(1.0), outColor.rgb));
outColor.rgb = pow(max(outColor.rgb, vec3(0.0)), vec3(1.0 / 2.2));
gl_FragColor = outColor;
}
