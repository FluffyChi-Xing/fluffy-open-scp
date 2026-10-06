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
void main() {
vec3 textureFloatPosition = vTexcoord0.xyz;
if ((-textureFloatPosition.z) < 0.0) discard;
vec2 uvOrig = textureFloatPosition.xy * -0.5 + 0.5;
vec2 uv = uvOrig * texXform.xy + texXform.zw;
vec4 decalTexture = texture2D(uSampler0, uv);
outColor = decalTexture;
outColor.rgb *= 2;
gl_FragColor = outColor;
}
