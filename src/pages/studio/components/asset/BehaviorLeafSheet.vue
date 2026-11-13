<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";
import FIcon from "@/components/extensions/FIcon.vue";
import LogicNodeCard from "./LogicNodeCard.vue";
import {
  cloneLogicParams,
  TOURISM_TEMPLATE_DEFAULTS,
  type LeafParams,
  type LogicNode,
  type TourismParams,
  validHotelRooms,
} from "./logicGraph";

const props = defineProps<{
  node: LogicNode;
  params: LeafParams;
  labelKey: string;
  trigger: HTMLElement | null;
}>();
const emit = defineEmits<{
  apply: [patch: Record<string, unknown>];
  close: [];
}>();
const { t } = useI18n();
const keys = "studio.assetWorkbench.behaviorSheet";
const draft = ref<LeafParams>(cloneLogicParams(props.params));
const tourism = computed(() =>
  draft.value.type === "tourism" ? draft.value : null,
);
const description = computed(() => {
  if (tourism.value) return "tourismTemplate";
  if (draft.value.type === "hotel") return "hotelTemplate";
  if (draft.value.type === "residence") return "residenceTemplate";
  if (draft.value.type === "landvalue" && draft.value.landValueProfile) return "landTemplate";
  return "description";
});
const invalidTemplateValue = computed(() => {
  const current = draft.value;
  if (current.type === "hotel") return validHotelRooms(current.rooms) ? null : "hotelRooms";
  if (current.type === "residence") {
    const population = current.population;
    return population != null && (!Number.isInteger(population) || population < 1 || population > 100_000)
      ? "populationInvalid" : null;
  }
  if (current.type === "landvalue" && current.landValueProfile) {
    return [current.value, current.value2].some(
      (value) => value != null && (!Number.isFinite(value) || Math.abs(value) > 100_000),
    ) ? "landInvalid" : null;
  }
  return null;
});
const fields = [
  "attraction",
  "shoppers",
  "quotaW1",
  "quotaW2",
  "quotaW3",
] as const;
const quotas = ["quotaW1", "quotaW2", "quotaW3"] as const;
type TourismField = (typeof fields)[number];

function patchDraft(patch: Record<string, unknown>): void {
  draft.value = { ...draft.value, ...patch };
}

function setNumber(field: TourismField, event: Event): void {
  const input = event.target as HTMLInputElement;
  patchDraft({
    [field]:
      input.value === "" && !input.validity.badInput
        ? null
        : input.valueAsNumber,
  });
}

const invalidField = computed(() => {
  if (!tourism.value) return undefined;
  const current = tourism.value;
  return fields.find((field) => {
    const value = current[field];
    return (
      value != null &&
      (!Number.isFinite(value) ||
        value < (field === "shoppers" ? 1 : 0) ||
        value > 100_000 ||
        (field !== "attraction" && !Number.isInteger(value)))
    );
  });
});
const emptyQuota = computed(() => {
  const current = tourism.value;
  return (
    current != null &&
    quotas.every(
      (field) => (current[field] ?? TOURISM_TEMPLATE_DEFAULTS[field]) === 0,
    )
  );
});

function apply(): void {
  if (invalidField.value || emptyQuota.value || invalidTemplateValue.value) return;
  const patch = tourism.value
    ? ({
        ...tourism.value,
        tourismProfile: "native-tourism-v1",
      } satisfies TourismParams)
    : draft.value;
  emit("apply", { ...patch });
}

function restoreFocus(event: Event): void {
  event.preventDefault();
  props.trigger?.focus();
}
</script>

<template>
  <DialogRoot :open="true" @update:open="!$event && emit('close')">
    <DialogPortal>
      <DialogOverlay class="behavior-sheet-overlay" />
      <DialogContent class="behavior-sheet" @close-auto-focus="restoreFocus">
        <form class="behavior-sheet-form" @submit.prevent="apply">
          <header class="behavior-sheet-header">
            <DialogTitle class="behavior-sheet-title">
              {{ t(`${keys}.title`, { behavior: t(labelKey) }) }}
            </DialogTitle>
            <DialogClose
              as="button"
              type="button"
              class="behavior-sheet-close"
              :aria-label="t(`${keys}.close`)"
            >
              <FIcon name="X" :size="17" aria-label="" />
            </DialogClose>
          </header>

          <div class="behavior-sheet-body">
            <DialogDescription class="behavior-sheet-hint">
              {{
                t(`${keys}.${description}`)
              }}
            </DialogDescription>
            <template v-if="tourism">
              <div class="behavior-sheet-fields">
                <label
                  v-for="field in fields"
                  :key="field"
                  class="behavior-sheet-field"
                >
                  <span>{{ t(`${keys}.${field}`) }}</span>
                  <input
                    type="number"
                    :min="field === 'shoppers' ? 1 : 0"
                    max="100000"
                    :step="field === 'attraction' ? 'any' : '1'"
                    :placeholder="String(TOURISM_TEMPLATE_DEFAULTS[field])"
                    :value="tourism[field] ?? ''"
                    :aria-invalid="
                      invalidField === field ||
                      (emptyQuota && quotas.some((quota) => quota === field))
                    "
                    @input="setNumber(field, $event)"
                  />
                </label>
              </div>
              <p v-if="invalidField" class="behavior-sheet-error" role="alert">
                {{
                  t(`${keys}.invalidNumber`, {
                    field: t(`${keys}.${invalidField}`),
                  })
                }}
              </p>
              <p
                v-else-if="emptyQuota"
                class="behavior-sheet-error"
                role="alert"
              >
                {{ t(`${keys}.quotaRequired`) }}
              </p>
              <div v-if="tourism.lodging" class="behavior-sheet-lodging">
                <p class="behavior-sheet-hint">{{ t(`${keys}.legacyLodging`) }}</p>
                <button type="button" @click="patchDraft({ lodging: false })">
                  {{ t(`${keys}.removeLegacyLodging`) }}
                </button>
              </div>
            </template>
            <LogicNodeCard
              v-else
              :node="node"
              :params="draft"
              :issues="[]"
              :disabled="false"
              embedded
              @update-params="patchDraft"
            />
            <p v-if="invalidTemplateValue" class="behavior-sheet-error" role="alert">
              {{ t(`${keys}.${invalidTemplateValue}`) }}
            </p>
          </div>

          <footer class="behavior-sheet-footer">
            <DialogClose as="button" type="button">{{
              t(`${keys}.cancel`)
            }}</DialogClose>
            <button type="submit" :disabled="!!invalidField || emptyQuota || !!invalidTemplateValue">
              {{ t(`${keys}.apply`) }}
            </button>
          </footer>
        </form>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
.behavior-sheet-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgb(0 0 0 / 38%);
}
.behavior-sheet {
  position: fixed;
  inset-block: 0;
  inset-inline-end: 0;
  z-index: 1001;
  width: min(100%, 420px);
  background: var(--surface);
  color: var(--foreground);
  border-inline-start: 1px solid var(--border);
  box-shadow: 0 8px 36px rgb(0 0 0 / 28%);
  animation: behavior-sheet-enter 180ms ease-out;
}
.behavior-sheet-form {
  height: 100%;
  display: flex;
  flex-direction: column;
}
.behavior-sheet-header,
.behavior-sheet-footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 14px 18px;
}
.behavior-sheet-header {
  justify-content: space-between;
  border-block-end: 1px solid var(--border);
}
.behavior-sheet-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
.behavior-sheet-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 18px;
}
.behavior-sheet-hint {
  margin: 0 0 14px;
  color: var(--muted-foreground);
  font-size: 12px;
  line-height: 1.5;
}
.behavior-sheet-fields {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 150px), 1fr));
  gap: 12px;
}
.behavior-sheet-field {
  display: grid;
  gap: 4px;
  font-size: 12px;
}
.behavior-sheet-field input,
.behavior-sheet-footer button,
.behavior-sheet-close {
  min-height: 32px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--surface-elevated);
  color: inherit;
  padding: 4px 9px;
}
.behavior-sheet-close {
  display: inline-grid;
  place-items: center;
  padding: 4px;
  min-width: 32px;
}
.behavior-sheet-field input {
  width: 100%;
}
.behavior-sheet-error {
  color: var(--destructive);
  font-size: 12px;
}
.behavior-sheet-lodging {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-block: 20px 8px;
  font-size: 13px;
}
.behavior-sheet-footer {
  justify-content: flex-end;
  border-block-start: 1px solid var(--border);
}
.behavior-sheet-footer button[type="submit"] {
  background: var(--primary);
  color: var(--primary-foreground);
}
.behavior-sheet-footer button:disabled {
  opacity: 0.55;
}
@keyframes behavior-sheet-enter {
  from {
    transform: translateX(100%);
  }
  to {
    transform: translateX(0);
  }
}
@media (prefers-reduced-motion: reduce) {
  .behavior-sheet {
    animation: none;
  }
}
</style>
