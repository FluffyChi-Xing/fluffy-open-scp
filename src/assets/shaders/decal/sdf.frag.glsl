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
uniform vec4 uLayerColors[4];
uniform float uNightBoost;

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

vec3 shColorDiff = vec3(0.0);
vec3 shColorSpec = vec3(0.0);
vec3 spec = vec3(0.0);
vec3 bumpNormal = normalize(uDecalWorldDirection);
outColor = texture2D(uSampler0, vTexcoord0.xy * 0.5 + 0.5);
float texturePositionZ = 0.0;
#define texturePosition vec3(vTexcoord0.xy, texturePositionZ)
vec3 decalNUS = vTexcoord0.xyz;
float materialLightScale = decalMaterialInfo.x * 16.0 + 0.25;
float sdfTextureLength = max(decalNUS.x, decalNUS.y);
float sphereHeight = decalNUS.z;
float hwRatio = sphereHeight * 0.5 / sdfTextureLength;
float zScale = 1;
if (hwRatio < 1)
{
hwRatio = 1;
zScale = 1 / hwRatio;
}
float circleZ = texturePosition.z;
circleZ *= zScale;
vec4 sdfDists = outColor;
float kMaskCenter = 0.5;
vec4 circleDists = clamp(1 - sdfDists * 1.0 / kMaskCenter, 0.0, 1.0) * hwRatio;
vec4 sphereDistsSqr = circleDists * circleDists + circleZ * circleZ;
vec4 animEdge = max(animResults, 0.0);
float lerpXParam = sphereHeight * 0.5 / decalNUS.x;
float lerpYParam = sphereHeight * 0.5 / decalNUS.y;
vec4 animation = mix(vec4(lerpXParam, lerpXParam, lerpXParam, lerpXParam),
vec4(lerpYParam, lerpYParam, lerpYParam, lerpYParam), useV.xyzw);
sphereDistsSqr += animEdge * animEdge * animRatio * 32;
float lightScales = clamp(1 - sqrt(sphereDistsSqr), 0.0, 1.0);
lightScales *= lightScales;
vec3 lightColor = vec3(0, 0, 0);
for (int i = 0; i < 3; ++i)
{
lightColor[i] = materialLightScale * dot(decalMaterialData[i], lightScales);
}
outColor.rgb = lightColor;
vec3 bumpNormal = normalize(decalWorldDirection);
vec3 shColorDiff = vec3(0, 0, 0);
vec3 shColorSpec = vec3(0, 0, 0);
vec3 spec = vec3(0, 0, 0);
SimCityLighting(bumpNormal, worldCameraDirection.xyz, gloss, reflectance,
specE, specStrength, shColorDiff, shColorSpec, spec);
outColor.rgb += shColorSpec + spec;
outColor.a *= decalMaterialInfo.x;
gl_FragColor = outColor;
}
