import { AnimationMixer, LoopOnce, type Mesh } from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { normalizeLotModelScene } from "@/lib/three-gltf";
import { disposeObject, ThreeViewer } from "@/lib/three-viewer";
import type { AnimationBinding, readAnimation } from "./animationPreview";
import { createPackedVertexSampler } from "./packedVertexMotion";
import { sampleMotionTimeline } from "./motionTimeline";

export async function createAnimationScene(
  element: HTMLElement,
  { models, materialPayload, motionSlot }: Awaited<ReturnType<typeof readAnimation>>,
  binding: AnimationBinding,
  cancelled: () => boolean,
) {
  const sceneObjects: Awaited<ReturnType<GLTFLoader["parseAsync"]>>[] = [];
  let viewer: ThreeViewer | undefined;
  let attached = false;
  let disposeMaterials: (() => void) | undefined;
  const mixers: AnimationMixer[] = [];
  const vertexSamplers: { slot: number; sample: (value: number) => void }[] = [];
  const timeline = binding.timeline;
  const slots = timeline ? new Set(timeline.tracks.map((track) => track.slot)) : new Set([motionSlot]);
  const dispose = () => {
    for (const mixer of mixers) {
      mixer.stopAllAction();
      mixer.uncacheRoot(mixer.getRoot());
    }
    if (viewer) viewer.dispose();
    if (!attached) for (const { scene } of sceneObjects) disposeObject(scene);
    disposeMaterials?.();
  };

  try {
    const loader = new GLTFLoader();
    for (const { glb, packedMotion } of models) {
      const parsed = await loader.parseAsync(glb, "");
      sceneObjects.push(parsed);
      if (cancelled()) { dispose(); return; }
      if (packedMotion) {
        const meshes: Mesh[] = [];
        parsed.scene.traverse((object) => { if ((object as Mesh).isMesh) meshes.push(object as Mesh); });
        if (meshes.length !== 1) throw new Error("Packed motion requires one mesh per payload");
        const mesh = meshes[0];
        for (const slot of slots) {
          const sampler = createPackedVertexSampler(mesh.geometry, packedMotion, slot);
          if (sampler) {
            mesh.frustumCulled = false;
            vertexSamplers.push({ slot, sample: sampler });
          }
        }
      }
    }
    for (const { scene } of sceneObjects) normalizeLotModelScene(scene, undefined);
    if (materialPayload) {
      const { applyAnimationMaterials } = await import("./animationMaterials");
      disposeMaterials = await applyAnimationMaterials(sceneObjects.map(({ scene }) => scene), materialPayload);
      if (cancelled()) { dispose(); return; }
    }
    const clips = sceneObjects.flatMap(({ scene, animations }) => {
      const clip = animations[0];
      if (!clip || clip.duration <= 0 || !clip.tracks.length) return [];
      const mixer = new AnimationMixer(scene);
      const action = mixer.clipAction(clip).setLoop(LoopOnce, 1);
      action.clampWhenFinished = true;
      action.play();
      action.paused = true;
      mixers.push(mixer);
      // Static bounds can wrongly cull a part that travels away from its bind pose.
      scene.traverse((object) => { object.frustumCulled = false; });
      return [{ action, mixer, duration: clip.duration }];
    });
    if (!clips.length && !vertexSamplers.length) throw new Error("Missing playable animation tracks");
    const sample = (ratio: number) => {
      const time = binding.start + (binding.end - binding.start) * ratio;
      const values = timeline && sampleMotionTimeline(timeline, time);
      for (const sampler of vertexSamplers) sampler.sample(values ? values[sampler.slot] : time);
      for (const { action, mixer, duration } of clips) {
        action.time = Math.max(0, Math.min(time, duration));
        mixer.update(0);
      }
      viewer?.invalidate();
    };
    viewer = await ThreeViewer.create(element, { interactionPixelRatio: 1 });
    if (cancelled()) { dispose(); return; }
    viewer.setModel(sceneObjects.map(({ scene }) => scene));
    attached = true;
    return { dispose, sample };
  } catch (error) {
    dispose();
    throw error;
  }
}
