<script setup lang="ts">
import { useI18n } from "vue-i18n";
const { t } = useI18n();
import { computed, ref, watch } from "vue";
import {
  validateParameters,
  type ParameterTable,
} from "@/lib/raster-editor/schema";
const props = defineProps<{ modelValue: ParameterTable }>();
const emit = defineEmits<{ "update:modelValue": [value: ParameterTable] }>();
const text = ref("");
const error = ref("");
watch(
  () => props.modelValue,
  (value) => {
    text.value = JSON.stringify(value, null, 2);
    error.value = "";
  },
  { immediate: true, deep: true },
);
const column = ref(0);
const columnIndex = computed(() =>
  Math.min(
    props.modelValue.cols - 1,
    Math.max(0, Math.floor(column.value || 0)),
  ),
);
function edit(index: number, value: string) {
  if (
    !value.trim() ||
    !Number.isFinite(Number(value)) ||
    !Number.isFinite(Math.fround(Number(value)))
  ) {
    error.value = t("rasterEditor.text071");
    return;
  }
  const values = [...props.modelValue.values];
  values[index] = Number(value);
  emit("update:modelValue", { cols: props.modelValue.cols, values });
}
function apply() {
  try {
    emit("update:modelValue", validateParameters(JSON.parse(text.value)));
  } catch (e) {
    error.value = String(e);
  }
}
</script>
<template>
  <section class="parameter-editor">
    <h3>{{ t("rasterEditor.text072") }}</h3>
    <p>
      {{ t("rasterEditor.text073") }} {{ modelValue.cols }}
      {{ t("rasterEditor.text074") }}
    </p>
    <label>
      {{ t("rasterEditor.text075") }}
      <input
        v-model.number="column"
        type="number"
        min="0"
        :max="modelValue.cols - 1"
    /></label>
    <table>
      <thead>
        <tr>
          <th>{{ t("rasterEditor.text076") }}</th>
          <th v-for="c in ['X', 'Y', 'Z', 'W']" :key="c">{{ c }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in 4" :key="row">
          <th>{{ row - 1 }}</th>
          <td v-for="c in 4" :key="c">
            <input
              type="number"
              step="any"
              :aria-label="
                t('rasterEditor.text077', {
                  p0: row - 1,
                  p1: columnIndex,
                  p2: c - 1,
                })
              "
              :value="
                modelValue.values[
                  ((row - 1) * modelValue.cols + columnIndex) * 4 + c - 1
                ]
              "
              @change="
                edit(
                  ((row - 1) * modelValue.cols + columnIndex) * 4 + c - 1,
                  ($event.target as HTMLInputElement).value,
                )
              "
            />
          </td>
        </tr>
      </tbody>
    </table>
    <details>
      <summary>{{ t("rasterEditor.text078") }}</summary>
      <textarea
        v-model="text"
        spellcheck="false"
        :aria-label="t('rasterEditor.text015')"
      /><button @click="apply">{{ t("rasterEditor.text079") }}</button>
    </details>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
</template>
<style scoped>
.parameter-editor {
  padding: 24px;
}
h3 {
  font-size: 16px;
}
p,
label {
  font-size: 12px;
  color: var(--muted-foreground);
}
table {
  width: 100%;
  table-layout: fixed;
  margin-block: 20px;
  font-size: 12px;
}
th,
td {
  padding: 6px;
}
input {
  width: 100%;
}
label input {
  width: 70px;
}
input,
textarea,
button {
  background: var(--surface);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 7px;
}
textarea {
  width: 100%;
  min-height: 200px;
  margin-block: 10px;
  font: 12px monospace;
}
summary {
  cursor: pointer;
  font-size: 12px;
}
[role="alert"] {
  color: var(--destructive);
}
</style>
