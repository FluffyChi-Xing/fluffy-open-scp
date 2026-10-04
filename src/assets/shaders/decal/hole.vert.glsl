// == 由 sc-shader 组合器生成：破洞体积盒 VS（three.js 相容）==
varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;
uniform vec3 uBoxHalf; // (半宽, 半高, 半深)
void main() {
  vUv = uv;
  // x 取负对齐引擎 uv = texpos × -0.5 + 0.5 的镜像口径（与 quad 路径一致）。
  vec3 n = clamp(position / max(uBoxHalf, vec3(0.001)), -1.0, 1.0);
  vTexcoord0 = vec3(-n.x, n.y, n.z);
  vTexcoord4 = vec4(0.0);
  vTexcoord5 = vec4(0.0);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
