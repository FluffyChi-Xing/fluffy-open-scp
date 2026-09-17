<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdown from "@/components/ui/FDropdown.vue";
import FSheet from "@/components/ui/FSheet.vue";
import type { ModProjectGroup, ModProjectView } from "@/api/tauri";

/**
 * 新建/编辑项目表单（FSheet）。create 模式无 project prop；
 * edit 模式回填并可改分组与状态。
 */
const props = defineProps<{
  groups: ModProjectGroup[];
  /** "create" | 要编辑的项目 */
  project?: ModProjectView | null;
}>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{
  create: [name: string, groupId?: number, description?: string];
  update: [
    id: number,
    fields: {
      name: string;
      groupId?: number;
      description?: string;
      status: string;
    },
  ];
}>();

const { t } = useI18n();

const isEdit = computed(() => Boolean(props.project));
const name = ref("");
const groupId = ref<number | undefined>(undefined);
const groupMenuOpen = ref(false);
const description = ref("");
const status = ref("active");
const error = ref("");
const busy = ref(false);

const selectedGroupName = computed(
  () =>
    props.groups.find((group) => group.id === groupId.value)?.name ??
    t("studio.projects.form.groupNone"),
);

function chooseGroup(id: number | undefined) {
  groupId.value = id;
  groupMenuOpen.value = false;
}

watch(open, (value) => {
  if (!value) return;
  error.value = "";
  name.value = props.project?.name ?? "";
  groupId.value = props.project?.groupId ?? undefined;
  description.value = props.project?.description ?? "";
  status.value = props.project?.status ?? "active";
});

const statuses = ["active", "released", "archived"] as const;

async function submit() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    if (isEdit.value && props.project) {
      await emit("update", props.project.id, {
        name: name.value,
        groupId: groupId.value,
        description: description.value.trim() || undefined,
        status: status.value,
      });
    } else {
      await emit(
        "create",
        name.value,
        groupId.value,
        description.value.trim() || undefined,
      );
    }
    open.value = false;
  } catch (cause) {
    error.value =
      cause && typeof cause === "object" && "message" in cause
        ? String(cause.message)
        : String(cause);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <FSheet v-model:open="open" :label="$t('studio.projects.form.title')">
    <form class="project-form" @submit.prevent="submit">
      <header class="form-header">
        <strong>{{
          isEdit
            ? $t("studio.projects.form.editTitle")
            : $t("studio.projects.form.createTitle")
        }}</strong>
        <button
          class="close"
          type="button"
          :aria-label="$t('common.close')"
          @click="open = false"
        >
          <FIcon name="X" :size="14" aria-label="" />
        </button>
      </header>

      <label class="field">
        <span>{{ $t("studio.projects.form.nameLabel") }}</span>
        <input
          v-model="name"
          type="text"
          required
          :placeholder="$t('studio.projects.form.namePlaceholder')"
        />
        <small>{{ $t("studio.projects.form.nameHint") }}</small>
      </label>

      <div class="field">
        <span>{{ $t("studio.projects.form.groupLabel") }}</span>
        <FDropdown v-model:open="groupMenuOpen" :width="220">
          <template #trigger>
            <button type="button" class="group-trigger">
              <span>{{ selectedGroupName }}</span>
              <FIcon name="ChevronDown" :size="12" aria-label="" />
            </button>
          </template>
          <button type="button" @click="chooseGroup(undefined)">
            <FIcon
              :name="groupId === undefined ? 'Check' : 'Minus'"
              :size="14"
              aria-label=""
            />
            {{ $t("studio.projects.form.groupNone") }}
          </button>
          <button
            v-for="group in groups"
            :key="group.id"
            type="button"
            @click="chooseGroup(group.id)"
          >
            <FIcon
              :name="groupId === group.id ? 'Check' : 'Tag'"
              :size="14"
              aria-label=""
            />
            {{ group.name }}
          </button>
        </FDropdown>
      </div>

      <label class="field">
        <span>{{ $t("studio.projects.form.statusLabel") }}</span>
        <div class="status-row" role="radiogroup">
          <button
            v-for="entry in statuses"
            :key="entry"
            type="button"
            class="status-option"
            :class="{ active: status === entry }"
            @click="status = entry"
          >
            {{ $t(`studio.dashboard.status.${entry}`) }}
          </button>
        </div>
      </label>

      <label class="field">
        <span>{{ $t("studio.projects.form.descLabel") }}</span>
        <textarea
          v-model="description"
          rows="3"
          :placeholder="$t('studio.projects.form.descPlaceholder')"
        ></textarea>
      </label>

      <p v-if="error" class="error" role="alert">{{ error }}</p>

      <div class="actions">
        <button class="ghost" type="button" @click="open = false">
          {{ $t("studio.projects.form.cancel") }}
        </button>
        <button class="primary" type="submit" :disabled="busy || !name.trim()">
          <FIcon :name="busy ? 'Loader2' : 'Check'" :size="14" aria-label="" />
          {{ $t("studio.projects.form.save") }}
        </button>
      </div>
    </form>
  </FSheet>
</template>

<style scoped>
.project-form {
  display: grid;
  gap: 16px;
  padding: 18px 16px;
}
.form-header {
  align-items: center;
  display: flex;
  justify-content: space-between;
}
.form-header strong {
  font-size: 14px;
}
.close {
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  min-height: 26px;
  min-width: 26px;
  align-items: center;
  justify-content: center;
}
.field {
  display: grid;
  gap: 6px;
}
.field > span {
  color: var(--muted-foreground);
  font-size: 11px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.group-trigger {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  justify-content: space-between;
  min-height: 32px;
  padding: 6px 10px;
  width: 100%;
}
.field input,
.field textarea {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12.5px;
  min-height: 32px;
  padding: 6px 10px;
  width: 100%;
}
.field small {
  color: var(--subtle-foreground);
  font-size: 11px;
}
.status-row {
  display: flex;
  gap: 6px;
}
.status-option {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  flex: 1;
  font: inherit;
  font-size: 12px;
  min-height: 30px;
}
.status-option.active {
  border-color: color-mix(in srgb, var(--primary) 60%, var(--border));
  color: var(--foreground);
}
.error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  overflow-wrap: anywhere;
}
.actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.ghost,
.primary {
  align-items: center;
  border-radius: var(--radius-sm);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 6px;
  min-height: 30px;
  padding: 0 12px;
}
.ghost {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--muted-foreground);
}
.primary {
  background: var(--primary);
  border: 1px solid var(--primary);
  color: #fff;
}
.primary:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
</style>
