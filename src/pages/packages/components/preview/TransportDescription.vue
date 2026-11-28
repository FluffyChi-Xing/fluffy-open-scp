<script setup lang="ts">
import ActorModelPreview from "./ActorModelPreview.vue";
import type { SemanticKeyRef } from "@/api/tauri";
import type { TransportDetails } from "@/api/property-semantics";
import { keyText } from "./semanticCard";
defineProps<{
  packageId: number;
  details: TransportDetails;
  parent: SemanticKeyRef | null;
}>();
</script>
<template>
  <section class="description">
    <h4>
      {{
        $t(
          details.effects.length
            ? "package.scene.transportTitle"
            : "package.scene.noModelTitle",
        )
      }}
    </h4>
    <p>
      {{
        $t(
          details.effects.length
            ? "package.scene.transportHint"
            : parent || details.graphics.length
              ? "package.scene.indirectHint"
              : "package.scene.noModelHint",
        )
      }}
    </p>
    <template
      v-for="field in ['effects', 'sinks', 'graphics'] as const"
      :key="field"
    >
      <div v-if="details[field].length">
        <strong>{{ $t(`package.scene.${field}`) }}</strong
        ><code v-for="(key, i) in details[field]" :key="i">{{
          keyText(key)
        }}</code>
      </div>
    </template>
    <ActorModelPreview
      v-if="parent || details.graphics.length"
      :package-id="packageId"
      :references="[...details.graphics, ...(parent ? [parent] : [])]"
    />
  </section>
</template>
<style scoped>
.description {
  display: grid;
  gap: 12px;
  font-size: 13px;
}
p {
  color: var(--muted-foreground);
  line-height: 1.6;
}
code {
  display: block;
  overflow-wrap: anywhere;
  font-size: 12px;
  padding-top: 6px;
}
h4,
p {
  margin: 0;
}
h4 {
  font-size: 12px;
  font-weight: 600;
}
section,
details,
dl,
dd {
  min-width: 0;
}
summary {
  line-height: 1.7;
  overflow-wrap: anywhere;
  border-radius: var(--radius-sm);
}
summary:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 3px;
}
code {
  overflow-wrap: anywhere;
}
dl {
  grid-template-columns: minmax(80px, 0.4fr) minmax(0, 1fr);
}
</style>
