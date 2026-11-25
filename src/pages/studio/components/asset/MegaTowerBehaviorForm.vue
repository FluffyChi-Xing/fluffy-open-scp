<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Switch } from "@/components/ui/switch";
import policy from "@/lib/asset-behavior-policy.json";

const props = defineProps<{ modelValue?: unknown; disabled: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: Selection | undefined] }>();
const { t } = useI18n();

const kinds = ["park", "office", "education", "residence", "commercial", "tourism", "hotel"] as const;
type BehaviorKind = (typeof kinds)[number];

interface Behavior {
  kind: BehaviorKind;
  params?: Record<string, unknown>;
}

interface Selection {
  // Legacy document field. It now selects behavior recipes on one Unit.
  floors: Behavior[];
}

const enabled = computed(() => props.modelValue != null);
const selection = computed(() => props.modelValue as Partial<Selection> | undefined);
const selected = computed(() => {
  const value = selection.value;
  return Array.isArray(value?.floors) ? value.floors.filter((item) => item && kinds.includes(item.kind)) : [];
});

function isSelected(kind: BehaviorKind): boolean {
  return selected.value.some((item) => item.kind === kind);
}

function hasOfficeRetailPair(items: Behavior[]): boolean {
  return items.some((item) => item.kind === "office") && items.some((item) => item.kind === "commercial");
}

function conflicts(left: BehaviorKind, right: BehaviorKind): boolean {
  return policy.incompatible.some(([first, ...rest]) =>
    (first === left && rest.includes(right)) || (first === right && rest.includes(left)),
  );
}

function hasConflict(items: Behavior[]): boolean {
  if (hasOfficeRetailPair(items)) return items.length !== 2;
  return items.length > policy.maxLeaves || items.some((item, index) =>
    items.slice(index + 1).some((other) => item.kind === other.kind || conflicts(item.kind, other.kind)),
  );
}

const invalid = computed(() => hasConflict(selected.value));

function unavailable(kind: BehaviorKind): boolean {
  return !isSelected(kind) && hasConflict([...selected.value, { kind }]);
}

function setEnabled(checked: boolean) {
  emit("update:modelValue", checked ? { floors: [{ kind: "commercial" }] } : undefined);
}

function toggleBehavior(kind: BehaviorKind, checked: boolean) {
  const floors = checked
    ? [...selected.value, { kind }]
    : selected.value.filter((item) => item.kind !== kind);
  emit("update:modelValue", { floors });
}
</script>

<template>
  <section class="behavior-config">
    <label class="behavior-switch">
      <Switch
        :model-value="enabled"
        :disabled="disabled"
        :aria-label="t('studio.assetWorkbench.megaTower.enable')"
        @update:model-value="setEnabled"
      />
      <span>{{ t("studio.assetWorkbench.megaTower.enable") }}</span>
    </label>
    <template v-if="enabled">
      <p>{{ t("studio.assetWorkbench.megaTower.hint") }}</p>
      <p v-if="invalid || selected.length === 0" role="alert">{{ t("studio.assetWorkbench.megaTower.conflict") }}</p>
      <fieldset :disabled="disabled">
        <legend>{{ t("studio.assetWorkbench.megaTower.kind") }}</legend>
        <label v-for="kind in kinds" :key="kind" class="behavior-switch">
          <Switch
            :model-value="isSelected(kind)"
            :disabled="disabled || unavailable(kind)"
            :aria-label="t(`studio.assetWorkbench.megaTower.${kind}`)"
            @update:model-value="toggleBehavior(kind, $event)"
          />
          <span>{{ t(`studio.assetWorkbench.megaTower.${kind}`) }}</span>
        </label>
      </fieldset>
    </template>
  </section>
</template>

<style scoped>
.behavior-config, .behavior-config fieldset { display: grid; gap: 0.65rem; }
.behavior-switch { display: flex; align-items: center; gap: 0.5rem; }
</style>
