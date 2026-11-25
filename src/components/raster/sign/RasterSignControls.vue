<script setup lang="ts">
import { shallowRef, useId } from "vue";
import { useI18n } from "vue-i18n";
import type { SignSettings } from "@/lib/raster-editor/signPaint";

const props = defineProps<{
  settings: SignSettings;
  colors: string[];
  disabled: boolean;
  canLetter: boolean;
}>();
const layer = defineModel<number>("layer", { required: true });
const emit = defineEmits<{
  settings: [value: SignSettings];
  color: [index: number, color: string];
  lettering: [text: string];
}>();
const { t } = useI18n();
const id = useId();
const lettering = shallowRef("");
const effects = ["steady", "marquee"] as const;
const axes = ["horizontal", "vertical"] as const;
function setSpeed(event: Event) {
  const speed = (event.target as HTMLInputElement).valueAsNumber;
  emit("settings", { ...props.settings, speed });
}
</script>

<template>
  <div class="sign-controls">
    <fieldset class="sign-field" :disabled="disabled">
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
    <template v-if="settings.effect === 'marquee'">
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
      <legend>{{ t("rasterEditor.sign.layers") }}</legend>
      <div class="palette">
        <div v-for="(color, index) in colors" :key="index" class="palette-row sign-control" :class="{ active: layer === index }">
          <button type="button" :aria-pressed="layer === index" @click="layer = index">
            {{ t("rasterEditor.sign.layer", { number: index + 1 }) }}
          </button>
          <input
type="color" :value="color" :aria-label="t('rasterEditor.decalColor', { n: index + 1 })"
            @input="emit('color', index, ($event.target as HTMLInputElement).value)" />
        </div>
      </div>
    </fieldset>
    <form v-if="canLetter" class="lettering" @submit.prevent="emit('lettering', lettering.trim())">
      <label :for="`${id}-lettering`">{{ t("rasterEditor.sign.lettering") }}</label>
      <input :id="`${id}-lettering`" v-model="lettering" class="sign-control" type="text" maxlength="80" :disabled="disabled" />
      <button class="sign-control" type="submit" :disabled="disabled || !lettering.trim()">{{ t("rasterEditor.sign.addLettering") }}</button>
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
.palette .palette-row.sign-control { display: flex; align-items: center; padding: 0; }
.palette-row button {
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
.palette-row input { flex: none; width: 34px; height: 26px; margin-inline-end: 3px; border: 0; background: transparent; cursor: pointer; }
.sign-field:disabled { opacity: 0.55; }
.lettering { display: grid; gap: var(--sign-row-gap, 8px); }
.lettering label { font-size: 12px; font-weight: 600; }
.lettering button { justify-self: end; }
.lettering button:enabled { cursor: pointer; }
.lettering button:disabled { opacity: 0.55; }
</style>
