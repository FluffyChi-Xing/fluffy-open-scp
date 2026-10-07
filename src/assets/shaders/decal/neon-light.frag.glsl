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

vec2 uvOrig = vUv;
outColor = texture2D(uSampler0, uvOrig);
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

      float materialLightScale = decalMaterialInfo.x * 16.0 + 0.25;
      float sdfTextureLength = max(uDecalNUS.x, uDecalNUS.y);
      float sphereHeight = uDecalNUS.z;
      float hwRatio = sphereHeight * 0.5 / sdfTextureLength;
      float zScale = 1.0;
      if (hwRatio < 1.0) { hwRatio = 1.0; zScale = 1.0 / hwRatio; }
      float circleZ = vTexcoord0.z * zScale;
      vec4 circleDists = clamp(1.0 - outColor * 2.0, 0.0, 1.0) * hwRatio;
      vec4 sphereDistsSqr = circleDists * circleDists + circleZ * circleZ;
      vec4 animEdge = max(animResults, 0.0) * uAnimEnabled;
      vec4 animRatio = mix(vec4(sphereHeight * 0.5 / uDecalNUS.x),
                           vec4(sphereHeight * 0.5 / uDecalNUS.y), useV);
      sphereDistsSqr += animEdge * animEdge * animRatio * 32.0;
      vec4 lightScales = clamp(1.0 - sqrt(sphereDistsSqr), 0.0, 1.0);
      lightScales *= lightScales;
      outColor.rgb = materialLightScale * vec3(dot(decalMaterialData[0], lightScales),
        dot(decalMaterialData[1], lightScales), dot(decalMaterialData[2], lightScales));
      outColor.rgb *= decalMaterialInfo.w;
      outColor.a = 0.0;
    
gl_FragColor = outColor;
}
