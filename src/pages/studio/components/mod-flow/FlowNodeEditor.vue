<script setup lang="ts">
import FSheet from "@/components/ui/FSheet.vue";
import NodeConfigForm from "./NodeConfigForm.vue";
import { useI18n } from "vue-i18n";
import type { FlowSchema } from "./contracts";
defineProps<{ schema: FlowSchema | null; busy: boolean }>();
const emit = defineEmits<{ save: [schema: FlowSchema] }>();
const open = defineModel<boolean>("open", { default: false });
const { t } = useI18n();
</script>
<template>
  <FSheet v-model:open="open" :label="t('flow.edit')" width="min(520px,94vw)"
    ><header class="editor-heading">
      <h2>{{ schema ? t(`flow.nodes.${schema.kind}`) : t("flow.edit") }}</h2>
      <button @click="open = false">{{ t("flow.close") }}</button>
    </header>
    <NodeConfigForm :schema="schema" :busy="busy" @save="emit('save', $event)"
  /></FSheet>
</template>
<style scoped>
.editor-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
}
.editor-heading h2 {
  font-size: 16px;
  margin: 0;
}
.editor-heading button {
  height: 26px;
  padding: 0 8px;
  font-size: 11px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface);
  color: inherit;
}
</style>
