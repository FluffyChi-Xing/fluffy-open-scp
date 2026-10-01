// == 由 sc-shader 组合器生成：quad VS（three.js 相容）==
varying vec3 vTexcoord0;
varying vec4 vTexcoord4;
varying vec4 vTexcoord5;
varying vec2 vUv;
void main() {
  vUv = uv;
  // quad uv ∈[0,1] → 引擎盒坐标 [-1,1]（decalClip 的 -0.5 镜像由此自动成立）
  vTexcoord0 = vec3(uv * 2.0 - 1.0, 0.0);
  vTexcoord4 = vec4(0.0);
  vTexcoord5 = vec4(0.0);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
