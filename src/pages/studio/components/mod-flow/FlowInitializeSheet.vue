<script setup lang="ts">
import FDropdownSelect from "@/components/ui/FSelect.vue";
import { ref, shallowRef } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import FSheet from "@/components/ui/FSheet.vue";
const props = defineProps<{
  project: string;
  manifest: Record<string, unknown>;
}>();
const emit = defineEmits<{ initialized: [] }>();
const open = defineModel<boolean>("open", { default: false });
const { t } = useI18n();
const form = ref({
  author: String(props.manifest.author ?? ""),
  description: String(props.manifest.description ?? ""),
  mod_type: String(props.manifest.mod_type ?? "map"),
  workflow: "original",
});
const busy = shallowRef(false);
const error = shallowRef("");
async function initialize() {
  busy.value = true;
  error.value = "";
  try {
    await invoke("code_flow", {
      request: {
        project: props.project,
        action: "initialize",
        payload: {
          ...form.value,
          workflow:
            form.value.mod_type === "map" ? form.value.workflow : "assets",
        },
      },
    });
    open.value = false;
    emit("initialized");
  } catch (e) {
    error.value = typeof e === "string" ? e : JSON.stringify(e);
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <FSheet
    v-model:open="open"
    :label="t('flow.initialize')"
    width="min(480px, 94vw)"
  >
    <form class="initialize-form" @submit.prevent="initialize">
      <h2>{{ t("flow.initialize") }}</h2>
      <p>{{ t("flow.initializeHint") }}</p>
      <label
        >{{ t("flow.author")
        }}<input v-model="form.author" required :disabled="busy"
      /></label>
      <label
        >{{ t("flow.description")
        }}<textarea v-model="form.description" required :disabled="busy" />
      </label>
      <label
        ><span>{{ t("flow.modType") }}</span
        ><FDropdownSelect
          v-model="form.mod_type"
          :disabled="busy"
          :options="
            ['map', 'code', 'assets', 'gameplay'].map((value) => ({
              value,
              label: t(`flow.${value}`),
            }))
          "
      /></label>
      <label v-if="form.mod_type === 'map'"
        ><span>{{ t("flow.workflow") }}</span
        ><FDropdownSelect
          v-model="form.workflow"
          :disabled="busy"
          :options="
            ['original', 'whole-region', 'generated'].map((value) => ({
              value,
              label: t(`flow.${value}`),
            }))
          "
      /></label>
      <p v-if="form.workflow === 'generated' && form.mod_type === 'map'">
        {{ t("flow.generatedHint") }}
      </p>
      <p v-if="form.workflow === 'whole-region' && form.mod_type === 'map'">
        {{ t("flow.alignmentHint") }}
      </p>
      <p v-if="error" role="alert">{{ error }}</p>
      <button :disabled="busy" type="submit">{{ t("flow.initialize") }}</button
      ><button type="button" :disabled="busy" @click="open = false">
        {{ t("flow.close") }}
      </button>
    </form>
  </FSheet>
</template>
<style scoped>
.initialize-form {
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.initialize-form h2 {
  font-size: 18px;
  margin: 0;
}
.initialize-form p {
  font-size: 13px;
  line-height: 1.6;
  color: var(--muted-foreground);
}
.initialize-form label {
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 13px;
}
.initialize-form input,
.initialize-form textarea,
.initialize-form button {
  padding: 9px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface);
  color: inherit;
}
.initialize-form button {
  cursor: pointer;
  min-height: 26px;
  padding: 0 8px;
  font-size: 11px;
}
</style>
