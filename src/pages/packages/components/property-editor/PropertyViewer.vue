<script setup lang="ts">
import type { LotModelPayload, Tgi } from "@/api/tauri";
import PropertyEditor from "./PropertyEditor.vue";

defineProps<{
  packageId: number;
  tgi: Tgi;
  previewPayload?: LotModelPayload | null;
}>();
</script>

<template>
  <PropertyEditor
    v-bind="$props"
    mode="viewer"
  >
    <template v-for="name in ['toolbar', 'aside', 'canvas-before', 'tabs']" :key="name" #[name]>
      <slot :name="name" />
    </template>
  </PropertyEditor>
</template>
