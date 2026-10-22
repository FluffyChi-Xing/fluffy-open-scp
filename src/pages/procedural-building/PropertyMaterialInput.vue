<script setup lang="ts">
import { useId } from "vue";
import FButton from "@/components/ui/FButton.vue";
import FSpinner from "@/components/ui/FSpinner.vue";

const input = defineModel<string>({ required: true });
defineProps<{
  loading: boolean;
  errorKey: string;
  loadedName?: string;
  lod?: number;
}>();
const emit = defineEmits<{ resolve: [] }>();
const id = useId();
</script>

<template>
  <section class="property-material" :aria-busy="loading">
    <label :for="id" class="material-label">{{
      $t("proceduralBuilding.propertyMaterial")
    }}</label>
    <input
      :id="id"
      v-model="input"
      class="tgi-input"
      type="text"
      spellcheck="false"
      autocomplete="off"
      :placeholder="$t('proceduralBuilding.propertyTgiPlaceholder')"
      :aria-invalid="!!errorKey"
      :aria-describedby="`${id}-hint ${id}-status`"
      @keydown.enter.prevent="emit('resolve')"
    />
    <p :id="`${id}-hint`" class="material-hint">
      {{ $t("proceduralBuilding.propertyTgiHint") }}
    </p>
    <div
      :id="`${id}-status`"
      class="material-status"
      role="status"
      aria-live="polite"
    >
      <FSpinner
        v-if="loading"
        size="sm"
        :label="$t('proceduralBuilding.propertyResolving')"
      />
      <p v-else-if="errorKey" class="material-error">
        {{ $t(`proceduralBuilding.${errorKey}`) }}
      </p>
      <template v-else-if="loadedName">
        <p>
          {{
            $t("proceduralBuilding.propertyLoaded", { name: loadedName, lod })
          }}
        </p>
      </template>
    </div>
    <div v-if="input" class="material-actions">
      <FButton v-if="errorKey" variant="secondary" @click="emit('resolve')">{{
        $t("proceduralBuilding.propertyRetry")
      }}</FButton>
      <FButton variant="secondary" @click="input = ''">{{
        $t("proceduralBuilding.propertyClear")
      }}</FButton>
    </div>
  </section>
</template>

<style scoped>
.property-material {
  display: flex;
  flex-direction: column;
  gap: 8px;
  border-top: 1px solid var(--border);
  padding-top: 12px;
}
.material-label {
  font-size: 12px;
  font-weight: 600;
}
.tgi-input {
  width: 100%;
  box-sizing: border-box;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: inherit;
  padding: 8px;
  font-family: monospace;
  font-size: 12px;
}
.tgi-input:focus-visible {
  outline: 2px solid var(--brand);
  outline-offset: 2px;
}
.tgi-input[aria-invalid="true"] {
  border-color: var(--destructive, #c44);
}
.material-hint {
  font-size: 11px;
  line-height: 1.6;
  color: var(--muted-foreground);
}
.material-status {
  font-size: 12px;
  overflow-wrap: anywhere;
}
.material-error {
  color: var(--destructive, #c44);
}
.material-actions {
  display: flex;
  gap: 8px;
}
.property-material p {
  margin: 0;
}
</style>
