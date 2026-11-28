import * as THREE from "three";
import type { FocusPlane } from "./cityCameraFocus";

const fullscreenVertex = /* glsl */ `
  varying vec2 screenUv;
  void main() {
    screenUv = uv;
    gl_Position = vec4(position.xy, 0.0, 1.0);
  }
`;

export function createCityCameraEffects(renderer: THREE.WebGLRenderer) {
  const sceneTarget = new THREE.WebGLRenderTarget(1, 1, {
    depthTexture: new THREE.DepthTexture(1, 1, THREE.UnsignedIntType),
  });
  const focusTarget = new THREE.WebGLRenderTarget(1, 1, { depthBuffer: false });
  const inverse = new THREE.Matrix4();
  const focusMaterial = new THREE.ShaderMaterial({
    vertexShader: fullscreenVertex,
    depthTest: false,
    depthWrite: false,
    uniforms: {
      colorMap: { value: sceneTarget.texture },
      depthMap: { value: sceneTarget.depthTexture },
      clip: { value: new THREE.Vector2() },
      plane: { value: new THREE.Vector4() },
      pixel: { value: new THREE.Vector2() },
    },
    fragmentShader: /* glsl */ `
      varying vec2 screenUv;
      uniform sampler2D colorMap;
      uniform sampler2D depthMap;
      uniform vec2 clip;
      uniform vec4 plane;
      uniform vec2 pixel;

      float blurAt(vec2 uv) {
        float depth = texture2D(depthMap, uv).r;
        float linearDepth = clip.x * clip.y / mix(clip.y, clip.x, depth);
        float offset = linearDepth - plane.x;
        return clamp(max((-offset - plane.y) / plane.z, (offset - plane.y) / plane.w), 0.0, 1.0);
      }

      void main() {
        vec4 color = texture2D(colorMap, screenUv);
        float coc = blurAt(screenUv);
        float total = 1.0;
        for (int i = 0; i < 16; i++) {
          float angle = float(i) * 2.39996323;
          vec2 tap = screenUv + vec2(cos(angle), sin(angle)) * sqrt((float(i) + 0.5) / 16.0) * pixel * coc;
          float weight = smoothstep(0.0, 0.2, blurAt(tap));
          color += texture2D(colorMap, tap) * weight;
          total += weight;
        }
        gl_FragColor = color / total;
      }
    `,
  });
  // Retail motionBlurPS reconstructs world position and samples along the
  // difference to the previous world-to-clip transform (seven taps, 0.04 step).
  const motionMaterial = new THREE.ShaderMaterial({
    vertexShader: fullscreenVertex,
    depthTest: false,
    depthWrite: false,
    uniforms: {
      colorMap: { value: sceneTarget.texture },
      depthMap: { value: sceneTarget.depthTexture },
      reprojection: { value: new THREE.Matrix4() },
      strength: { value: 0 },
    },
    fragmentShader: /* glsl */ `
      varying vec2 screenUv;
      uniform sampler2D colorMap;
      uniform sampler2D depthMap;
      uniform mat4 reprojection;
      uniform float strength;

      void main() {
        vec2 stepUv = vec2(0.0);
        if (strength > 0.0) {
          float depth = texture2D(depthMap, screenUv).r;
          vec4 previous = reprojection * vec4(screenUv * 2.0 - 1.0, depth * 2.0 - 1.0, 1.0);
          if (previous.w > 0.0) {
            vec2 previousUv = clamp(previous.xy / previous.w * 0.5 + 0.5, 0.0, 1.0);
            stepUv = 0.04 * strength * (previousUv - screenUv);
          }
        }
        vec4 color = vec4(0.0);
        for (int i = -3; i <= 3; i++) {
          color += texture2D(colorMap, screenUv + float(i) * stepUv);
        }
        gl_FragColor = color / 7.0;
        #include <tonemapping_fragment>
        #include <colorspace_fragment>
      }
    `,
  });
  const screen = new THREE.Scene();
  const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), focusMaterial);
  quad.frustumCulled = false;
  screen.add(quad);
  const screenCamera = new THREE.Camera();
  const dimensions = new THREE.Vector2();

  return {
    render(
      scene: THREE.Scene,
      camera: THREE.PerspectiveCamera,
      focus: FocusPlane | null,
      previous: THREE.Matrix4,
      motionStrength: number | null,
    ) {
      if (!focus && motionStrength === null) {
        renderer.render(scene, camera);
        return;
      }
      renderer.getDrawingBufferSize(dimensions);
      const { x, y } = dimensions;
      sceneTarget.setSize(x, y);
      renderer.setRenderTarget(sceneTarget);
      renderer.render(scene, camera);

      if (focus) {
        focusTarget.setSize(x, y);
        // Adaptive DoF shader declares the maximum CoC diameter as 0.005
        // of the shorter render dimension.
        const radius = Math.min(x, y) * 0.005 * 0.5;
        focusMaterial.uniforms.clip!.value.set(camera.near, camera.far);
        focusMaterial.uniforms.plane!.value.set(
          focus.distance,
          focus.radius,
          focus.nearRamp,
          focus.farRamp,
        );
        focusMaterial.uniforms.pixel!.value.set(radius / x, radius / y);
        renderer.setRenderTarget(focusTarget);
        quad.material = focusMaterial;
        renderer.render(screen, screenCamera);
      }

      renderer.setRenderTarget(null);
      inverse.multiplyMatrices(
        camera.matrixWorld,
        camera.projectionMatrixInverse,
      );
      motionMaterial.uniforms.reprojection!.value.multiplyMatrices(
        previous,
        inverse,
      );
      motionMaterial.uniforms.strength!.value = motionStrength ?? 0;
      motionMaterial.uniforms.colorMap!.value = focus
        ? focusTarget.texture
        : sceneTarget.texture;
      quad.material = motionMaterial;
      renderer.render(screen, screenCamera);
    },
    dispose() {
      sceneTarget.depthTexture?.dispose();
      sceneTarget.dispose();
      focusTarget.dispose();
      focusMaterial.dispose();
      motionMaterial.dispose();
      quad.geometry.dispose();
    },
  };
}
