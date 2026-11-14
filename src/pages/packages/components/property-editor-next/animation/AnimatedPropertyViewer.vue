<script setup lang="ts">
import { defineAsyncComponent, shallowRef, useTemplateRef, watch } from "vue";
import { Switch } from "@/components/ui/switch";
import type { LotModelPayload, Tgi } from "@/api/tauri";
import PropertyViewer from "../../property-editor/PropertyViewer.vue";
import { listAnimations, type AnimationBinding } from "./animationPreview";

const props = defineProps<{
  packageId: number;
  tgi: Tgi;
  previewPayload?: LotModelPayload | null;
}>();
const open = defineModel<boolean>("open", { default: false });
const clips = shallowRef<AnimationBinding[]>([]);
const enabled = shallowRef(false);
const toggle = useTemplateRef<HTMLLabelElement>("toggle");
const refined = shallowRef(false);
const AnimationViewport = defineAsyncComponent(() => import("./BuildingAnimationViewport.vue"));

watch(toggle, (element, _, onCleanup) => {
  const mode = element?.parentElement?.querySelector(".render-mode");
  if (!mode) return;
  // Read the existing toolbar state without changing the frozen editor.
  const refinedButton = mode.querySelectorAll("button").item(1);
  if (!refinedButton) return;
  const update = () => {
    refined.value = refinedButton.classList.contains("active");
  };
  const observer = new MutationObserver(update);
  observer.observe(refinedButton, { attributes: true, attributeFilter: ["class"] });
  update();
  onCleanup(() => observer.disconnect());
}, { flush: "post" });

watch(
  () => [open.value, props.packageId, props.tgi.typeId, props.tgi.group, props.tgi.instance] as const,
  ([visible, packageId, typeId, group, instance], _, onCleanup) => {
    enabled.value = false;
    clips.value = [];
    if (!visible || typeId !== 0x00b1b104) return;
    let current = true;
    onCleanup(() => { current = false; });
    void listAnimations({ packageId, tgi: { typeId, group, instance } })
      .then((result) => { if (current) clips.value = result; })
      .catch(() => { /* An unavailable optional preview leaves the static viewer usable. */ });
  },
  { immediate: true, flush: "sync" },
);
</script>

<template>
  <PropertyViewer v-bind="$props" v-model:open="open">
    <template #toolbar>
      <slot name="toolbar" />
      <label v-if="clips.length" ref="toggle" class="pe-animation-toggle" :title="$t('package.animationPreviewHint')">
        <Switch v-model="enabled" :aria-label="$t('package.animationPreview')" />
        <span>{{ $t("package.animationPreview") }}</span>
      </label>
    </template>
    <template #canvas-before>
      <slot name="canvas-before" />
      <AnimationViewport
        v-if="enabled"
        class="pe-building-animation-branch"
        :source="{ packageId, tgi }"
        :clips="clips"
        :refined="refined"
      />
    </template>
    <template #aside><slot name="aside" /></template>
    <template #tabs><slot name="tabs" /></template>
  </PropertyViewer>
</template>

<style scoped>
.pe-animation-toggle {
  order: 1;
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
  font-size: 12px;
}

:global(.editor-header:has(> .pe-animation-toggle) > .render-mode:not(.spec-channel) ~ *) {
  order: 2;
}

:global(.viewport-column:has(> .pe-building-animation-branch)) {
  position: relative;
}

:global(.viewport-column:has(> .pe-building-animation-branch) > .viewport-pane) {
  visibility: hidden;
}

.pe-building-animation-branch {
  position: absolute;
  inset: 0;
  z-index: 30;
}
</style>
