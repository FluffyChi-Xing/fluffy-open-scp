// == 由 sc-shader 组合器生成：破洞体积 VS（three.js 相容）==
uniform vec2 uBoxHalfXY;
uniform float uHalfDepth;
varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;
void main() {
  vUv = uv;
  vTexcoord0 = position / max(vec3(uBoxHalfXY, uHalfDepth), vec3(0.001));
  vTexcoord4 = vec4((modelMatrix * vec4(position, 1.0)).xyz - cameraPosition, 0.0);
  vTexcoord5 = vec4(0.0);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
