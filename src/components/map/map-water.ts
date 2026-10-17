import * as THREE from "three";
import type { Region3DData } from "@/lib/region-map";
import { MAP_WORLD_XY_GLSL } from "./map-coordinates";

/** Terrain-clipped water preview. Native waterNormals/Choppy are runtime FFT
 * textures; this small periodic normal field approximates them without an FFT.
 * Shore foam, when available, is decoded from the game's beach_foam resource.
 */
export function createMapWater(data: Region3DData, height: ImageData, foam?: ImageData) {
  const heightMap = new THREE.DataTexture(new Uint8Array(height.data), height.width, height.height);
  heightMap.magFilter = heightMap.minFilter = THREE.LinearFilter;
  heightMap.needsUpdate = true;
  const n = 128;
  const pixels = new Uint8Array(n * n * 4);
  for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
    const u = x / n * Math.PI * 2, v = y / n * Math.PI * 2;
    const dx = Math.cos(3 * u + 2 * v) * 0.32 + Math.cos(7 * u - 5 * v) * 0.13;
    const dy = Math.cos(2 * u - 3 * v) * 0.30 + Math.cos(5 * u + 7 * v) * 0.12;
    pixels.set([Math.round((dx * 0.5 + 0.5) * 255), Math.round((dy * 0.5 + 0.5) * 255), 255, 255], (y * n + x) * 4);
  }
  const normalMap = new THREE.DataTexture(pixels, n, n);
  normalMap.wrapS = normalMap.wrapT = THREE.RepeatWrapping;
  normalMap.magFilter = THREE.LinearFilter;
  normalMap.minFilter = THREE.LinearMipmapLinearFilter;
  normalMap.generateMipmaps = true;
  normalMap.needsUpdate = true;
  const foamMap = new THREE.DataTexture(foam ? new Uint8Array(foam.data) : new Uint8Array([0, 0, 0, 0]), foam?.width ?? 1, foam?.height ?? 1);
  foamMap.wrapS = foamMap.wrapT = THREE.RepeatWrapping;
  foamMap.magFilter = THREE.LinearFilter;
  foamMap.minFilter = THREE.LinearMipmapLinearFilter;
  foamMap.generateMipmaps = true;
  foamMap.colorSpace = THREE.SRGBColorSpace;
  foamMap.needsUpdate = true;
  const parameter = (value: number | null | undefined, fallback: number) =>
    value != null && Number.isFinite(value) && value >= 0 ? value : fallback;
  const timeScale = parameter(data.waterParams?.timeStepFactor, 5);
  const uniforms = {
    heightMap: { value: heightMap }, normalMap: { value: normalMap }, foamMap: { value: foamMap },
    origin: { value: new THREE.Vector2(...data.originWorld) },
    extent: { value: data.size * data.metersPerPixel }, texel: { value: 0.5 / height.width },
    waterZ: { value: data.waterZ }, heightDiv: { value: data.heightDiv }, heightBias: { value: data.heightBias },
    time: { value: 0 },
    specularPower: { value: parameter(data.waterParams?.specularPower, 500) },
    specularScale: { value: parameter(data.waterParams?.specularScale, 11) },
  };
  const material = new THREE.ShaderMaterial({
    uniforms,
    vertexShader: `varying vec3 world;
      void main() { world = (modelMatrix * vec4(position, 1.0)).xyz;
        gl_Position = projectionMatrix * viewMatrix * vec4(world, 1.0); }`,
    fragmentShader: `varying vec3 world;
      uniform sampler2D heightMap, normalMap, foamMap;
      uniform vec2 origin;
      uniform float extent, texel, waterZ, heightDiv, heightBias, time, specularPower, specularScale;
      ${MAP_WORLD_XY_GLSL}
      void main() {
        vec2 mapXY = mapWorldXY(world);
        vec2 uv = (mapXY - origin) / extent + texel;
        vec2 h = texture2D(heightMap, uv).rg;
        float depth = waterZ - ((h.r * 65280.0 + h.g * 255.0) / heightDiv + heightBias);
        float shorelineWidth = max(1.5, fwidth(depth));
        if (depth <= 0.0) discard;
        vec2 p = mapXY / 150.0;
        vec2 wave = (texture2D(normalMap, p + vec2(time * 0.0024, time * 0.0014)).rg
          + texture2D(normalMap, p * 1.73 + vec2(-time * 0.0016, time * 0.0018)).rg - 1.0) * 0.55;
        vec3 normal = normalize(vec3(wave.x, 1.0, -wave.y));
        vec3 viewDir = normalize(cameraPosition - world);
        // Original waterPS Fresnel term. Sky/refraction remain preview approximations.
        float nv = max(dot(normal, viewDir), 0.0);
        float fresnel = clamp(0.925 - sqrt(sqrt(nv * nv * 5.0)), 0.01, 1.0);
        vec3 color = mix(vec3(0.055, 0.22, 0.20), vec3(0.012, 0.063, 0.115), 1.0 - exp(-depth / 24.0));
        color = mix(color, vec3(0.34, 0.48, 0.62), fresnel);
        vec3 reflectedSun = reflect(-normalize(vec3(0.4, 0.85, -0.3)), normal);
        float specular = pow(max(dot(reflectedSun, viewDir), 0.00001), specularPower) * specularScale;
        // beach_foam RGB is the foam color, alpha is coverage (waterPS, fragment 217).
        vec4 foamColor = texture2D(foamMap, mapXY / 48.0 + time * 0.0004);
        float foam = clamp((3.5 - max(0.0, depth - 1.0)) / 7.75, 0.0, 1.0) * foamColor.a;
        foam *= foam;
        specular *= 1.0 - clamp(foam * 2.5, 0.0, 1.0);
        color += vec3(0.8, 0.75, 0.6) * specular;
        float viewDepth = -(viewMatrix * vec4(world, 1.0)).z;
        foam *= 1.0 - smoothstep(2000.0, 10000.0, viewDepth);
        color = mix(color, foamColor.rgb * mix(vec3(1.0), vec3(0.34, 0.48, 0.62), 0.7), foam);
        // Pixel footprint anti-aliasing prevents a hard, flickering shoreline at overview zoom.
        gl_FragColor = vec4(color, smoothstep(0.0, shorelineWidth, depth) * 0.94);
        #include <tonemapping_fragment>
        #include <colorspace_fragment>
      }`,
    transparent: true, depthWrite: false,
  });
  material.addEventListener("dispose", () => { heightMap.dispose(); normalMap.dispose(); foamMap.dispose(); });
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(uniforms.extent.value, uniforms.extent.value), material);
  mesh.rotation.x = -Math.PI / 2;
  mesh.position.set(data.originWorld[0] + uniforms.extent.value / 2, data.waterZ, data.originWorld[1] + uniforms.extent.value / 2);
  return { mesh, update: (seconds: number) => { uniforms.time.value = seconds * timeScale; } };
}
