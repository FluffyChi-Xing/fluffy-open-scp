<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef, watch } from "vue";
import { command, type SemanticKeyRef } from "@/api/tauri";
import { isTauri, tauriApi } from "@/api";
import type { ActorModelResolution } from "@/api/actor-models";
import ActorAppearancePreview from "./ActorAppearancePreview.vue";
import type { ActorInfo } from "@/api/property-semantics";
import MeshPreview from "./MeshPreview.vue";
const props = defineProps<{
  actor?: ActorInfo;
  packageId: number;
  references: SemanticKeyRef[];
}>();
const resolution = shallowRef<ActorModelResolution | null>(null);
const state = shallowRef<"idle" | "loading" | "ready" | "error">("idle");
const meshState = shallowRef<"idle" | "loading" | "ready" | "error">("idle");
const active = shallowRef(0);
const enhanced = shallowRef(false);
const objects = shallowRef<string[]>([]);
let generation = 0;
onBeforeUnmount(() => {
  generation++;
});
const references = computed(() => [
  ...new Map(
    props.references.map((k) => [
      `${k.typeId}:${k.groupId}:${k.instanceId}`,
      k,
    ]),
  ).values(),
]);
const hex = (n: number) => `0x${n.toString(16).padStart(8, "0").toUpperCase()}`;
watch(
  () => [props.packageId, props.references],
  () => {
    generation++;
    enhanced.value = false;
    resolution.value = null;
    state.value = "idle";
    meshState.value = "idle";
    objects.value = [];
    active.value = 0;
  },
);
async function resolve() {
  const run = ++generation;
  state.value = "loading";
  meshState.value = "idle";
  objects.value = [];
  try {
    if (!isTauri()) throw new Error("runtime unavailable");
    const result = await command<ActorModelResolution>("resolve_actor_models", {
      request: {
        packageId: props.packageId,
        references: references.value.map((k) => ({
          typeId: k.typeId,
          group: k.groupId,
          instance: k.instanceId,
        })),
      },
    });
    if (run !== generation) return;
    resolution.value = result;
    active.value = 0;
    state.value = "ready";
    if (result.models.length) await loadModel();
  } catch {
    if (run === generation) state.value = "error";
  }
}
async function loadModel() {
  const model = resolution.value?.models[active.value];
  if (!model) return;
  const run = ++generation;
  meshState.value = "loading";
  objects.value = [];
  try {
    const preview = await tauriApi.packages.readRw4Preview(
      model.packageId,
      model.tgi,
    );
    const sections = preview.sections.filter((s) => s.typeCode === 0x20009);
    const details = await Promise.all(
      sections.map((s) =>
        tauriApi.packages.readRw4Section(model.packageId, model.tgi, s.number),
      ),
    );
    if (run !== generation) return;
    const meshes = details
      .map((d) => d.mesh?.objBase64)
      .filter((v): v is string => !!v);
    if (!meshes.length) throw new Error("no meshes");
    objects.value = meshes;
    meshState.value = "ready";
  } catch {
    if (run === generation) meshState.value = "error";
  }
}
</script>
<template>
  <section class="actor-model">
    <button
      v-if="references.length"
      type="button"
      :disabled="state === 'loading'"
      @click="resolve"
    >
      {{
        $t(
          state === "loading"
            ? "package.scene.modelLoading"
            : "package.scene.resolveModels",
        )
      }}
    </button>
    <p v-else>{{ $t("package.scene.noReferences") }}</p>
    <p v-if="state === 'error'" role="status">
      {{ $t("package.scene.resolveFailed") }}
    </p>
    <template v-if="resolution">
      <p v-if="!actor">{{ $t("package.scene.previewScope") }}</p>
      <ActorAppearancePreview
        v-if="actor"
        :actor="actor"
        :resolution="resolution"
        :package-id="packageId"
        :model-index="active"
        @active="enhanced = $event"
      />
      <div
        v-if="resolution.models.length"
        class="model-options"
        role="group"
        :aria-label="$t('package.scene.selectModel')"
      >
        <button
          v-for="(model, i) in resolution.models"
          :key="i"
          type="button"
          :aria-pressed="active === i"
          @click="
            active = i;
            loadModel();
          "
        >
          {{ hex(model.tgi.instance) }}
        </button>
      </div>
      <p v-if="meshState === 'loading'" role="status">
        {{ $t("package.scene.modelLoading") }}
      </p>
      <p v-if="meshState === 'error'" role="status">
        {{ $t("package.scene.meshFailed") }}
      </p>
      <MeshPreview
        v-if="meshState === 'ready' && !enhanced"
        :obj-base64s="objects"
      />
      <p v-if="!resolution.models.length && !enhanced">
        {{ $t("package.scene.noResolvedMesh") }}
      </p>
      <details class="reference-chain">
        <summary>{{ $t("package.scene.referenceChain") }}</summary>
        <ol class="chain">
          <li v-for="(step, i) in resolution.steps" :key="i">
            <span
              >{{ $t(`package.scene.via_${step.via}`) }} ·
              {{ $t(`package.scene.status_${step.status}`) }}</span
            >
            <code
              >{{ hex(step.tgi.typeId) }}:{{ hex(step.tgi.group) }}:{{
                hex(step.tgi.instance)
              }}</code
            >
            <template v-if="step.parts">
              <p>{{ $t("package.scene.compositeHint") }}</p>
              <dl>
                <template
                  v-for="field in [
                    'bodies',
                    'bodiesMax',
                    'heads',
                    'headsMax',
                    'outfits',
                    'outfitsMax',
                  ] as const"
                  :key="field"
                  ><dt>{{ $t(`package.scene.parts_${field}`) }}</dt>
                  <dd>
                    {{
                      step.parts[field].join(", ") || $t("package.scene.absent")
                    }}
                  </dd></template
                >
              </dl>
            </template>
          </li>
        </ol>
      </details>
    </template>
  </section>
</template>
<style scoped>
.actor-model {
  display: grid;
  gap: 10px;
  padding-block: 8px;
  min-width: 0;
}
button {
  color: inherit;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 6px 10px;
}
button {
  justify-self: start;
  cursor: pointer;
}
button:disabled {
  cursor: wait;
}
button:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 3px;
}
p,
.chain {
  font-size: 12px;
  line-height: 1.6;
}
.chain {
  padding-inline-start: 20px;
}
li {
  margin-block: 8px;
}
code {
  display: block;
  overflow-wrap: anywhere;
}
dl {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 4px 12px;
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.model-options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
}
button {
  font: inherit;
  font-size: 12px;
  min-height: 30px;
  max-width: 100%;
  overflow-wrap: anywhere;
  text-align: start;
}
button:hover:not(:disabled) {
  background: var(--surface-hover);
}
button[aria-pressed="true"] {
  border-color: var(--primary);
  background: color-mix(in srgb, var(--primary) 12%, var(--surface-elevated));
}
p,
.chain,
dl {
  margin: 0;
}
.reference-chain {
  min-width: 0;
  border-top: 1px solid var(--border);
  padding-block-start: 10px;
}
summary {
  cursor: pointer;
  color: var(--muted-foreground);
  font-size: 12px;
  line-height: 1.6;
}
.chain {
  padding-block-start: 6px;
}
dl {
  grid-template-columns: minmax(72px, 0.4fr) minmax(0, 1fr);
}
</style>
