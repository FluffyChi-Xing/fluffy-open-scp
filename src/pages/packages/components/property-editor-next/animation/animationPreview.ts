import { command, type Tgi } from "@/api/tauri";
import { parseLotModelContainer } from "@/lib/three-gltf";
import type { MotionTimeline } from "./motionTimeline";

export interface AnimationSource {
  packageId: number;
  tgi: Tgi;
}

export interface AnimationBinding {
  id: number;
  resourceId: number | null;
  start: number;
  end: number;
  timeline?: MotionTimeline;
}

export function listAnimations(request: AnimationSource) {
  return command<AnimationBinding[]>("list_building_animations", { request });
}

export async function readAnimation(source: AnimationSource, id: number, refined: boolean) {
  const bytes = await command<ArrayBuffer>("read_building_animation", {
    request: { ...source, animationId: id, refined },
  });
  const view = new DataView(bytes);
  const tail = bytes.byteLength - 4;
  const materialSize = view.getUint32(tail, true);
  const modelEnd = tail - materialSize;
  if (modelEnd < 0) throw new Error("Invalid animation material payload");
  const models: { glb: ArrayBuffer; packedMotion: ArrayBuffer | null }[] = [];
  const motionSlot = view.getUint8(0);
  let offset = 1;
  while (offset < modelEnd) {
    if (modelEnd - offset < 8) throw new Error("Truncated animation mesh");
    const size = view.getUint32(offset, true);
    offset += 4;
    if (size === 0 || size > modelEnd - offset - 4) {
      throw new Error("Invalid animation model payload");
    }
    const glb = bytes.slice(offset, offset + size);
    offset += size;
    const motionSize = view.getUint32(offset, true);
    offset += 4;
    if (motionSize % 12 !== 0 || motionSize > modelEnd - offset) {
      throw new Error("Invalid packed vertex animation");
    }
    models.push({ glb, packedMotion: motionSize ? bytes.slice(offset, offset + motionSize) : null });
    offset += motionSize;
  }
  if (!models.length) throw new Error("Missing animation model");
  return {
    models,
    motionSlot,
    materialPayload: materialSize ? parseLotModelContainer(bytes.slice(modelEnd, tail)) : null,
  };
}

export function animationLabel(id: number) {
  return `0x${id.toString(16).toUpperCase().padStart(8, "0")}`;
}
