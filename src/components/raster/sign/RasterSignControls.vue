<script setup lang="ts">
import { shallowRef, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Switch } from "@/components/ui/switch";
import { canPaintDecalLayer, type DecalLayerState } from "@/lib/raster-editor/decalLayers";
import type { SignSettings } from "@/lib/raster-editor/signPaint";

const props = defineProps<{
  settings: SignSettings;
  colors: string[];
  layers: readonly DecalLayerState[];
  disabled: boolean;
  canLetter: boolean;
  graffiti?: boolean;
}>();
const layer = defineModel<number>("layer", { required: true });
const strength = defineModel<number>("strength", { default: 255 });
const emit = defineEmits<{
  settings: [value: SignSettings];
  color: [index: number, color: string];
  lettering: [text: string];
  layerState: [index: number, state: DecalLayerState];
}>();
const { t } = useI18n();
const id = useId();
const lettering = shallowRef("");
const effects = ["steady", "marquee"] as const;
const axes = ["horizontal", "vertical"] as const;
function layerName(index: number) {
  return t(props.graffiti ? "rasterEditor.graffiti.layer" : "rasterEditor.sign.layer", { number: index + 1 });
}
function setSpeed(event: Event) {
  const speed = (event.target as HTMLInputElement).valueAsNumber;
  emit("settings", { ...props.settings, speed });
}
</script>

<template>
  <div class="sign-controls">
    <fieldset v-if="!graffiti" class="sign-field" :disabled="disabled">
      <legend>{{ t("rasterEditor.sign.effect") }}</legend>
      <div class="segments">
        <label v-for="effect in effects" :key="effect">
          <input
type="radio" :name="`${id}-effect`" :checked="settings.effect === effect"
            @change="emit('settings', { ...settings, effect })" />
          <span class="sign-control">{{ t(`rasterEditor.sign.${effect}`) }}</span>
        </label>
      </div>
    </fieldset>
    <template v-if="!graffiti && settings.effect === 'marquee'">
      <fieldset class="sign-field" :disabled="disabled">
        <legend>{{ t("rasterEditor.sign.axis") }}</legend>
        <div class="segments">
          <label v-for="axis in axes" :key="axis">
            <input
type="radio" :name="`${id}-axis`" :checked="settings.axis === axis"
              @change="emit('settings', { ...settings, axis })" />
            <span class="sign-control">{{ t(`rasterEditor.sign.${axis}`) }}</span>
          </label>
        </div>
      </fieldset>
      <label class="speed">
        {{ t("rasterEditor.sign.speed") }}
        <output>{{ t("rasterEditor.sign.rate", { speed: $n(settings.speed) }) }}</output>
        <input type="range" min="0.1" max="2" step="0.1" :value="settings.speed" :disabled="disabled" @input="setSpeed" />
      </label>
    </template>
    <fieldset class="sign-field" :disabled="disabled">
      <legend>{{ t(graffiti ? "rasterEditor.graffiti.layers" : "rasterEditor.sign.layers") }}</legend>
      <div class="palette">
        <div class="layer-column-labels" aria-hidden="true">
          <span />
          <span>{{ t("rasterEditor.decalLayers.visible") }}</span>
          <span>{{ t("rasterEditor.decalLayers.locked") }}</span>
        </div>
        <div v-for="(state, index) in layers" :key="index" class="palette-row sign-control" :class="{ active: layer === index, 'layer-hidden': !state.visible }">
          <button class="layer-select" type="button" :aria-pressed="layer === index" @click="layer = index">
            {{ layerName(index) }}
          </button>
          <input
            type="color" :value="colors[index]" :disabled="state.locked"
            :aria-label="t('rasterEditor.decalColor', { n: index + 1 })"
            @input="emit('color', index, ($event.target as HTMLInputElement).value)" />
          <Switch
            class="h-6"
            :model-value="state.visible" :aria-label="t('rasterEditor.decalLayers.visibilityOf', { layer: layerName(index) })"
            @update:model-value="emit('layerState', index, { ...state, visible: $event })"
          />
          <Switch
            class="h-6"
            :model-value="state.locked" :aria-label="t('rasterEditor.decalLayers.lockOf', { layer: layerName(index) })"
            @update:model-value="emit('layerState', index, { ...state, locked: $event })"
          />
        </div>
      </div>
      <p v-if="!canPaintDecalLayer(layers[layer])" class="layer-help" role="status">
        {{ t("rasterEditor.decalLayers.selectEditable") }}
      </p>
      <p v-else class="layer-help">{{ t("rasterEditor.decalLayers.orderHint") }}</p>
    </fieldset>
    <label v-if="graffiti" class="speed">
      {{ t("rasterEditor.graffiti.strength") }}
      <output>{{ $n(strength / 255, { style: "percent" }) }}</output>
      <input v-model.number="strength" type="range" min="128" max="255" step="1" :disabled="disabled" />
    </label>
    <form v-if="canLetter" class="lettering" @submit.prevent="emit('lettering', lettering.trim())">
      <label :for="`${id}-lettering`">{{ t(graffiti ? "rasterEditor.graffiti.lettering" : "rasterEditor.sign.lettering") }}</label>
      <textarea
        :id="`${id}-lettering`" v-model="lettering" class="sign-control" rows="3" maxlength="240"
        :placeholder="t('rasterEditor.sign.letteringPlaceholder')"
        :disabled="disabled || !canPaintDecalLayer(layers[layer])"
      />
      <button class="sign-control" type="submit" :disabled="disabled || !canPaintDecalLayer(layers[layer]) || !lettering.trim()">{{ t("rasterEditor.sign.addLettering") }}</button>
    </form>
  </div>
</template>

<style scoped>
.sign-controls {
  display: grid;
  gap: var(--sign-group-gap, 16px);
  padding: var(--sign-section-padding, 14px) var(--sign-inset, 16px);
}
.sign-field { border: 0; padding: 0; margin: 0; min-width: 0; }
.sign-field legend { margin-bottom: var(--sign-row-gap, 8px); font-size: 12px; font-weight: 600; }
.segments { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--sign-row-gap, 8px); }
.segments label { position: relative; display: grid; }
.segments input { position: absolute; opacity: 0; }
.segments span { display: grid; place-items: center; text-align: center; cursor: pointer; }
.segments input:checked + span, .palette .palette-row.active {
  background: var(--accent);
  border-color: var(--primary);
}
.segments input:focus-visible + span,
.palette-row :focus-visible { outline: 2px solid var(--primary); outline-offset: 1px; }
.speed { display: grid; grid-template-columns: 1fr auto; gap: var(--sign-row-gap, 8px); font-size: 12px; }
.speed output { font-variant-numeric: tabular-nums; }
.speed input { grid-column: 1 / -1; width: 100%; accent-color: var(--primary); }
.palette { display: grid; gap: var(--sign-row-gap, 8px); }
.layer-column-labels,
.palette .palette-row.sign-control {
  display: grid;
  align-items: center;
  grid-template-columns: minmax(0, 1fr) 28px 36px 36px;
  column-gap: 6px;
  padding: 0 6px 0 0;
}
.layer-column-labels {
  grid-template-columns: minmax(0, 1fr) 36px 36px;
  font-size: 10px;
  color: var(--muted-foreground);
  text-align: center;
}
.palette-row .layer-select {
  align-self: stretch;
  flex: 1;
  min-width: 0;
  padding: 4px 9px;
  color: var(--foreground);
  background: transparent;
  border: 0;
  text-align: start;
  font-size: 12px;
  cursor: pointer;
}
.palette-row input[type="color"] { width: 28px; height: 26px; border: 0; background: transparent; cursor: pointer; }
.layer-hidden :is(.layer-select, input[type="color"]) { opacity: 0.55; }
.layer-help { margin: 8px 0 0; color: var(--muted-foreground); font-size: 11px; line-height: 1.45; }
.sign-field:disabled { opacity: 0.55; }
.lettering { display: grid; gap: var(--sign-row-gap, 8px); }
.lettering label { font-size: 12px; font-weight: 600; }
.lettering textarea { inline-size: 100%; resize: vertical; }
.lettering button { justify-self: end; }
.lettering button:enabled { cursor: pointer; }
.lettering button:disabled { opacity: 0.55; }
</style>
