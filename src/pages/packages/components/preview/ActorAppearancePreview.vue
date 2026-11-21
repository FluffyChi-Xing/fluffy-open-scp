<script setup lang="ts">
import { computed, shallowRef, useId, watch } from "vue";
import { Switch } from "@/components/ui/switch";
import { command } from "@/api/tauri";
import type { ActorInfo } from "@/api/property-semantics";
import type { ActorModelResolution } from "@/api/actor-models";
import { selectActorLook } from "./actorLook";
import type { ActorLook } from "./actorLook";
import type { ActorVisualPayload } from "./actorVisualAssets";
import ActorVisualViewport from "./ActorVisualViewport.vue";
const props = defineProps<{
  actor: ActorInfo;
  resolution: ActorModelResolution;
  packageId: number;
  modelIndex: number;
}>();
const emit = defineEmits<{ active: [value: boolean] }>();
const enabled = shallowRef(false),
  variant = shallowRef(0),
  failed = shallowRef(false),
  loading = shallowRef(false);
const payload = shallowRef<ActorVisualPayload | null>(null),
  look = shallowRef<ActorLook | null>(null);
const id = useId();
const parts = computed(
  () => props.resolution.steps.find((s) => s.parts)?.parts,
);
watch(
  [enabled, variant, () => props.actor, () => props.modelIndex],
  ([on], _, cleanup) => {
    emit("active", on);
    payload.value = null;
    look.value = null;
    failed.value = false;
    loading.value = on;
    if (!on) return;
    let cancelled = false;
    cleanup(() => {
      cancelled = true;
    });
    void (async () => {
      const seed =
        (props.actor.id ^ Math.imul(variant.value + 1, 0x9e3779b9)) >>> 0;
      const selected = parts.value
        ? selectActorLook(parts.value, props.actor, seed)
        : { body: 0, head: 0, outfit: 0, skin: seed & 255, row: 0 };
      if (!parts.value) {
        const a = props.actor.appearances[0];
        if (a?.skin != null && a.skin >= 0)
          selected.skin =
            a.skin +
            ((seed >>> 16) %
              Math.max(
                1,
                (a.skinMax != null && a.skinMax >= a.skin
                  ? a.skinMax
                  : a.skin) -
                  a.skin +
                  1,
              ));
        if (a?.outfit != null && a.outfit >= 0)
          selected.outfit =
            a.outfit +
            ((seed >>> 16) %
              Math.max(
                1,
                (a.outfitMax != null && a.outfitMax >= a.outfit
                  ? a.outfitMax
                  : a.outfit) -
                  a.outfit +
                  1,
              ));
      }
      const model = props.resolution.models[props.modelIndex];
      const data = await command<ActorVisualPayload>("read_actor_visuals", {
        request: {
          packageId: props.packageId,
          model: model?.tgi ?? null,
          body: parts.value ? selected.body : null,
          head: parts.value ? selected.head : null,
        },
      });
      if (cancelled) return;
      look.value = selected;
      payload.value = data;
    })()
      .catch(() => {
        if (!cancelled) failed.value = true;
      })
      .finally(() => {
        if (!cancelled) loading.value = false;
      });
  },
);
</script>
<template>
  <section class="actor-appearance">
    <div class="appearance-controls">
      <label :for="id"
        ><Switch
          :id="id"
          v-model="enabled"
          :aria-label="$t('package.scene.enableActorVisuals')"
        />{{ $t("package.scene.enableActorVisuals") }}</label
      ><button
        v-if="enabled"
        type="button"
        :disabled="loading"
        @click="variant++"
      >
        {{ $t("package.scene.resampleActor") }}
      </button>
    </div>
    <p v-if="!enabled">{{ $t("package.scene.actorWhiteMode") }}</p>
    <p v-if="loading" role="status">{{ $t("package.scene.visualLoading") }}</p>
    <p v-if="failed" role="status">{{ $t("package.scene.visualFailed") }}</p>
    <template v-if="enabled && payload && look"
      ><p>{{ $t("package.scene.selectedLook", { ...look }) }}</p>
      <ActorVisualViewport
        :payload="payload"
        :look="look"
        :scale="actor.scale ?? 1"
    /></template>
  </section>
</template>
<style scoped>
.actor-appearance {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 8px;
  min-width: 0;
}
.appearance-controls {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
  align-items: center;
}
label {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  line-height: 1.6;
  cursor: pointer;
}
button {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface-elevated);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  padding: 5px 9px;
  cursor: pointer;
}
button:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 3px;
}
button:disabled {
  opacity: 0.5;
  cursor: wait;
}
p {
  margin: 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--muted-foreground);
  overflow-wrap: anywhere;
}
</style>
