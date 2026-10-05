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
if (uGraffiti > 0.5) {
vec4 m = texture2D(uSampler0, vUv);
      vec3 col = vec3(0.0);
      float alpha = 0.0;
      if (m.a >= 0.5)      { col = uLayerColors[3].rgb; alpha = m.a; }
      else if (m.b >= 0.5) { col = uLayerColors[2].rgb; alpha = m.b; }
      else if (m.g >= 0.5) { col = uLayerColors[1].rgb; alpha = m.g; }
      else if (m.r >= 0.5) { col = uLayerColors[0].rgb; alpha = m.r; }
      outColor = vec4(col, alpha);
} else {
vec4 m = texture2D(uSampler0, vUv);
      vec3 col = vec3(0.0);
      float alpha = 0.0;
      if (m.a >= 0.5)      { col = uLayerColors[3].rgb; alpha = 1.0; }
      else if (m.b >= 0.5) { col = uLayerColors[2].rgb; alpha = 1.0; }
      else if (m.g >= 0.5) { col = uLayerColors[1].rgb; alpha = 1.0; }
      else if (m.r >= 0.5) { col = uLayerColors[0].rgb; alpha = 1.0; }
      outColor = vec4(col, alpha);
}
float signGain = clamp(decalMaterialInfo.x * 16.0 + 0.25, 1.0, 4.0);
float lightbox = signGain * mix(1.0, uNightBoost, 0.4);
outColor.rgb *= mix(lightbox, uNightBoost, uGraffiti);
float signMax = max(outColor.r, max(outColor.g, outColor.b));
outColor.rgb /= 1.0 + max(signMax - 1.0, 0.0) * (1.0 - uGraffiti);
gl_FragColor = outColor;
}
