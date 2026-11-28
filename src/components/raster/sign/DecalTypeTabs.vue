<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { nextTick } from "vue";

type DecalPaintingProfile = "decal-sign" | "decal-graffiti";
const props = defineProps<{ tabId: string; panelId: string; profile: DecalPaintingProfile; disabled: boolean }>();
const emit = defineEmits<{ select: [profile: DecalPaintingProfile] }>();
const { t } = useI18n();
const available = [
  { profile: "decal-sign", label: "rasterEditor.sign.signs" },
  { profile: "decal-graffiti", label: "rasterEditor.sign.types.graffiti" },
] as const;
const unavailable = ["neon", "interior", "rgba", "hologram", "ground"] as const;

async function selectWithKeyboard(event: KeyboardEvent) {
  if (props.disabled || !["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  const profile = event.key === "Home" ? "decal-sign" : event.key === "End" ? "decal-graffiti"
    : props.profile === "decal-sign" ? "decal-graffiti" : "decal-sign";
  const tabs = event.currentTarget as HTMLElement;
  emit("select", profile);
  await nextTick();
  tabs.querySelector<HTMLElement>('[aria-selected="true"]')?.focus();
}
</script>

<template>
  <div class="decal-type-field">
    <span :id="`${tabId}-label`">{{ t("rasterEditor.sign.tabs") }}</span>
    <div class="decal-tabs" role="tablist" :aria-labelledby="`${tabId}-label`" @keydown="selectWithKeyboard">
      <button
        v-for="entry in available" :id="`${tabId}-${entry.profile}`" :key="entry.profile"
        class="sign-control" type="button" role="tab" :aria-selected="profile === entry.profile"
        :aria-controls="panelId" :tabindex="profile === entry.profile ? 0 : -1"
        :disabled="disabled" @click="emit('select', entry.profile)"
      >
        {{ t(entry.label) }}
      </button>
      <button v-for="type in unavailable" :key="type" class="sign-control" type="button" role="tab" aria-selected="false" disabled>
        {{ t(`rasterEditor.sign.types.${type}`) }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.decal-type-field {
  display: grid;
  gap: var(--sign-row-gap);
  font-size: 12px;
  font-weight: 600;
}
.decal-tabs {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--sign-row-gap);
}
.decal-tabs button {
  text-align: start;
}
.decal-tabs button[aria-selected="true"] {
  border-color: var(--primary);
  background: var(--accent);
  font-weight: 650;
}
.decal-tabs button:disabled { color: var(--muted-foreground); opacity: 0.65; }
</style>
