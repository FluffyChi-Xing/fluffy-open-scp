<script setup lang="ts">
import { computed } from "vue";
import type { BlockMassParameters, BuildingBlock, BuildingVoid, Vec3 } from "@/lib/procedural-building/schema";
import { massPreset, normalizeBlockMass, rotateBlock, MASS_LIMITS, type MassPreset } from "@/lib/procedural-building/massing";

const model = defineModel<BlockMassParameters>({ required: true });
const presets: MassPreset[] = ["single", "y", "courtyard"];
const axes = [0, 1, 2] as const;
const sizeKeys = ["width", "depth", "height"];
const offsetKeys = ["cutoutOffsetX", "cutoutOffsetY", "cutoutOffsetZ"];
const entries = computed(() => model.value.blocks.map(block => ({block, holes: model.value.voids.filter(hole => hole.blockId === block.id)})));
function commit(value: BlockMassParameters) { model.value = normalizeBlockMass(value); }
function number(event: Event): number | null {
  const value = (event.target as HTMLInputElement).valueAsNumber;
  return Number.isFinite(value) ? value : null;
}
function uniqueId(prefix: string, entries: {id: string}[]) {
  let i = 1;
  while (entries.some(entry => entry.id === prefix + i)) i++;
  return prefix + i;
}
function updateBlock(block: BuildingBlock, next: BuildingBlock) {
  commit({...model.value, blocks: model.value.blocks.map(entry => entry.id === block.id ? next : entry)});
}
function setSize(block: BuildingBlock, axis: number, event: Event) {
  const value = number(event); if (value === null) return;
  const size = [...block.size] as Vec3, center = [...block.center] as Vec3;
  const next = Math.max(1, Math.min(MASS_LIMITS.size, value));
  if (axis === 2) center[2] += (next - size[2]) / 2; // Keep the base fixed.
  if (axis === 0 && block.junction) {
    const scale = next / size[0];
    center[0] = block.junction[0] + (center[0] - block.junction[0]) * scale;
    center[1] = block.junction[1] + (center[1] - block.junction[1]) * scale;
  }
  size[axis] = next; updateBlock(block, {...block, size, center});
}
function setCenter(block: BuildingBlock, axis: number, event: Event) {
  const value = number(event); if (value === null) return;
  const center = [...block.center] as Vec3;
  center[axis] = axis === 2 ? Math.max(0, value) + block.size[2] / 2 : value;
  const junction = block.junction ? [...block.junction] as [number, number] : undefined;
  if (junction && axis < 2) junction[axis] += center[axis] - block.center[axis];
  updateBlock(block, {...block, center, junction});
}
function setRotation(block: BuildingBlock, event: Event) {
  const value = number(event); if (value !== null) updateBlock(block, rotateBlock(block, value));
}
function addBlock() {
  if (model.value.blocks.length >= MASS_LIMITS.blocks) return;
  commit({...model.value, blocks: [...model.value.blocks, {id: uniqueId("block-", model.value.blocks), center: [0, 0, 60], size: [48, 24, 120], rotation: 0}]});
}
function removeBlock(block: BuildingBlock) {
  if (model.value.blocks.length <= 1) return;
  commit({blocks: model.value.blocks.filter(entry => entry.id !== block.id), voids: model.value.voids.filter(hole => hole.blockId !== block.id)});
}
function addVoid(block: BuildingBlock) {
  if (model.value.voids.length >= MASS_LIMITS.voids) return;
  commit({...model.value, voids: [...model.value.voids, {id: uniqueId("void-", model.value.voids), blockId: block.id,
    center: [0, -block.size[1] / 2, 0], size: [Math.min(block.size[0] / 2, 24), Math.min(block.size[1] / 2, 12), Math.min(block.size[2] / 2, 36)]}]});
}
function setVoid(hole: BuildingVoid, field: "center" | "size", axis: number, event: Event) {
  const value = number(event); if (value === null) return;
  const vector = [...hole[field]] as Vec3; vector[axis] = value;
  commit({...model.value, voids: model.value.voids.map(entry => entry.id === hole.id ? {...hole, [field]: vector} : entry)});
}
function removeVoid(hole: BuildingVoid) { commit({...model.value, voids: model.value.voids.filter(entry => entry.id !== hole.id)}); }
</script>

<template>
  <section class="mass-panel">
    <h3>{{ $t("proceduralBuilding.sectionBlocks") }}</h3>
    <p class="panel-hint">{{ $t("proceduralBuilding.blocksHint") }}</p>
    <div class="preset-strip" role="group" :aria-label="$t('proceduralBuilding.blockPreset')">
      <button v-for="value in presets" :key="value" type="button" @click="model = massPreset(value)">{{ $t('proceduralBuilding.blockPreset_' + value) }}</button>
    </div>
    <article v-for="({block, holes}, index) in entries" :key="block.id" class="block-card">
      <header class="card-header"><strong>{{ $t("proceduralBuilding.blockLabel", {index: index + 1}) }}</strong><button type="button" class="remove-button" :disabled="model.blocks.length <= 1" :aria-label="$t('proceduralBuilding.removeBlock')" @click="removeBlock(block)">×</button></header>
      <div class="metric-grid">
        <label v-for="axis in axes" :key="axis"><span>{{ $t('proceduralBuilding.' + sizeKeys[axis]) }}</span><input :value="block.size[axis]" type="number" min="1" :max="MASS_LIMITS.size" step="1" @change="setSize(block, axis, $event)" /></label>
        <label><span>{{ $t("proceduralBuilding.rotation") }}</span><input :value="block.rotation" type="number" min="-360" max="360" step="1" @change="setRotation(block, $event)" /></label>
        <label><span>{{ $t("proceduralBuilding.offsetX") }}</span><input :value="block.center[0]" type="number" min="-2000" max="2000" step="1" @change="setCenter(block, 0, $event)" /></label>
        <label><span>{{ $t("proceduralBuilding.offsetY") }}</span><input :value="block.center[1]" type="number" min="-2000" max="2000" step="1" @change="setCenter(block, 1, $event)" /></label>
        <label><span>{{ $t("proceduralBuilding.baseHeight") }}</span><input :value="block.center[2] - block.size[2] / 2" type="number" min="0" :max="2000 - block.size[2] / 2" step="1" @change="setCenter(block, 2, $event)" /></label>
      </div>
      <p v-if="block.junction" class="panel-hint">{{ $t('proceduralBuilding.junctionHint') }}</p>
      <div class="void-list">
        <fieldset v-for="(hole, holeIndex) in holes" :key="hole.id" class="void-card">
          <legend>{{ $t('proceduralBuilding.cutoutLabel', {index: holeIndex + 1}) }}</legend>
          <div class="metric-grid">
            <label v-for="axis in axes" :key="'size' + axis"><span>{{ $t('proceduralBuilding.' + sizeKeys[axis]) }}</span><input :value="hole.size[axis]" type="number" min="0.1" max="2000" step="1" @change="setVoid(hole, 'size', axis, $event)" /></label>
            <label v-for="axis in axes" :key="'center' + axis"><span>{{ $t('proceduralBuilding.' + offsetKeys[axis]) }}</span><input :value="hole.center[axis]" type="number" min="-2000" max="2000" step="1" @change="setVoid(hole, 'center', axis, $event)" /></label>
          </div>
          <p class="panel-hint">{{ $t('proceduralBuilding.cutoutHint') }}</p>
          <button type="button" class="text-button" @click="removeVoid(hole)">{{ $t('proceduralBuilding.removeCutout') }}</button>
        </fieldset>
        <button type="button" class="text-button" :disabled="model.voids.length >= MASS_LIMITS.voids" @click="addVoid(block)">+ {{ $t("proceduralBuilding.addCutout") }}</button>
      </div>
    </article>
    <button type="button" class="add-block-button" :disabled="model.blocks.length >= MASS_LIMITS.blocks" @click="addBlock">+ {{ $t("proceduralBuilding.addBlock") }}</button>
  </section>
</template>

<style scoped>
.mass-panel { display: grid; gap: 10px; }
.mass-panel h3 { margin: 0; font-size: 13px; }
.panel-hint { margin: 0; font-size: 11px; line-height: 1.5; color: var(--muted-foreground); }
.preset-strip { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 5px; }
.preset-strip button, .add-block-button, .text-button { border: 1px solid var(--border); border-radius: var(--radius-md); background: var(--surface); color: inherit; cursor: pointer; font: inherit; font-size: 12px; padding: 7px 8px; }
button:hover:enabled { border-color: var(--brand); color: var(--brand); }
button:disabled { cursor: not-allowed; opacity: .4; }
button:focus-visible, input:focus-visible { outline: 2px solid var(--brand); outline-offset: 2px; }
.block-card { display: grid; gap: 10px; border: 1px solid var(--border); border-radius: var(--radius-md); padding: 12px; }
.card-header { display: flex; align-items: center; justify-content: space-between; font-size: 12px; }
.metric-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.metric-grid label { display: grid; gap: 4px; min-width: 0; font-size: 11px; }
.metric-grid input { width: 100%; min-width: 0; box-sizing: border-box; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--background); color: inherit; padding: 6px 8px; font: inherit; font-size: 13px; }
.void-list { display: grid; gap: 8px; }
.void-card { display: grid; gap: 10px; min-width: 0; margin: 0; padding: 10px; border: 1px solid var(--border); border-radius: var(--radius-sm); }
.void-card legend { font-size: 11px; padding: 0 4px; }
.remove-button { border: 0; background: transparent; color: var(--muted-foreground); cursor: pointer; width: 28px; height: 28px; font-size: 18px; }
.text-button { justify-self: start; }
</style>
