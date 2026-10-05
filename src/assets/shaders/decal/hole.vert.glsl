// == 由 sc-shader 组合器生成：破洞投影 VS（three.js 相容）==
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
