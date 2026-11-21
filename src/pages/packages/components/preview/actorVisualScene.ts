import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { ThreeViewer, disposeObject } from "@/lib/three-viewer";
import { normalizeLotModelScene } from "@/lib/three-gltf";
import {
  bakeActorTexture,
  type ActorVisualPayload,
  type ActorPart,
} from "./actorVisualAssets";
import { actorHeadPosition } from "./actorHeadAttachment";
import type { ActorLook } from "./actorLook";
export async function createActorVisualScene(
  host: HTMLElement,
  payload: ActorVisualPayload,
  look: ActorLook,
  scale: number,
  cancelled: () => boolean,
) {
  const roots: THREE.Object3D[] = [],
    textures: THREE.Texture[] = [],
    mixers: THREE.AnimationMixer[] = [];
  const clips: { mixer: THREE.AnimationMixer; clips: THREE.AnimationClip[] }[] =
    [];
  let viewer: ThreeViewer | undefined,
    frame = 0,
    disposed = false;
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    cancelAnimationFrame(frame);
    for (const mixer of mixers) mixer.stopAllAction();
    viewer?.dispose();
    if (!viewer) for (const root of roots) disposeObject(root);
    for (const texture of textures) texture.dispose();
    observer.disconnect();
  };
  const syncTheme = () => {
    viewer?.scene.traverse((o) => {
      if (o.type === "GridHelper") {
        const line = o as THREE.GridHelper;
        const dark = document.documentElement.dataset.theme === "dark";
        for (const m of Array.isArray(line.material)
          ? line.material
          : [line.material]) {
          m.opacity = dark ? 0.32 : 0.55;
          m.transparent = true;
        }
      }
    });
    viewer?.invalidate();
  };
  const observer = new MutationObserver(syncTheme);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });
  try {
    const loader = new GLTFLoader();
    async function part(part: ActorPart) {
      if (!part.source) throw new Error("actor source texture missing");
      const rgba = bakeActorTexture(
        part.source,
        payload.bodyPalette,
        payload.clothesPalette,
        look,
      );
      const texture = new THREE.DataTexture(
        rgba,
        part.source.width,
        part.source.height,
        THREE.RGBAFormat,
      );
      texture.colorSpace = THREE.SRGBColorSpace;
      texture.flipY = true;
      texture.minFilter = texture.magFilter = THREE.LinearFilter;
      texture.needsUpdate = true;
      textures.push(texture);
      const group = new THREE.Group();
      roots.push(group);
      for (const bytes of part.glbs) {
        const parsed = await loader.parseAsync(
          new Uint8Array(bytes).buffer,
          "",
        );
        normalizeLotModelScene(parsed.scene, undefined);
        if (cancelled()) {
          disposeObject(parsed.scene);
          throw new Error("cancelled");
        }
        parsed.scene.traverse((o) => {
          const mesh = o as THREE.Mesh;
          if (!mesh.isMesh) return;
          for (const m of Array.isArray(mesh.material)
            ? mesh.material
            : [mesh.material])
            m.dispose();
          mesh.material = new THREE.MeshStandardMaterial({
            map: texture,
            roughness: 0.8,
            metalness: 0,
            alphaTest: 0.3,
            side: THREE.DoubleSide,
          });
          mesh.frustumCulled = false;
        });
        group.add(parsed.scene);
        if (parsed.animations.length) {
          const mixer = new THREE.AnimationMixer(parsed.scene);
          mixers.push(mixer);
          clips.push({ mixer, clips: parsed.animations });
        }
      }
      return group;
    }
    const body = await part(payload.body);
    let syncHead = () => {};
    if (payload.head) {
      const head = await part(payload.head);
      const joint =
        payload.body.headJoint != null
          ? body.getObjectByName(`joint${payload.body.headJoint}`)
          : null;
      if (!joint) throw new Error("native head attachment joint missing");
      body.add(head);
      syncHead = () => {
        body.updateWorldMatrix(true, true);
        head.position.copy(
          actorHeadPosition(body.matrixWorld, joint.matrixWorld),
        );
        head.quaternion.identity();
        head.updateMatrixWorld(true);
      };
    }
    body.scale.setScalar(Number.isFinite(scale) && scale > 0 ? scale : 1);
    body.updateMatrixWorld(true);
    syncHead();
    if (cancelled()) throw new Error("cancelled");
    viewer = await ThreeViewer.create(host);
    viewer.setModel([body]);
    syncTheme();
    const clipCount = Math.max(0, ...clips.map((c) => c.clips.length));
    const setClip = (index: number) => {
      for (const entry of clips) {
        entry.mixer.stopAllAction();
        const clip = entry.clips[index];
        if (clip && clip.duration > 0)
          entry.mixer
            .clipAction(clip)
            .setLoop(THREE.LoopRepeat, Infinity)
            .play();
      }
      for (const mixer of mixers) mixer.update(0);
      syncHead();
      viewer?.invalidate();
    };
    setClip(0);
    let previous = performance.now();
    if (clipCount)
      frame = requestAnimationFrame(function animate(now) {
        if (disposed) return;
        const dt = Math.min(0.1, Math.max(0, (now - previous) / 1000));
        previous = now;
        if (host.checkVisibility?.({ visibilityProperty: true }) !== false) {
          for (const mixer of mixers) mixer.update(dt);
          syncHead();
          viewer?.invalidate();
        }
        frame = requestAnimationFrame(animate);
      });
    return { dispose, setClip, clipCount };
  } catch (e) {
    dispose();
    throw e;
  }
}
