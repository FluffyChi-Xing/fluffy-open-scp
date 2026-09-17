<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSheet from "@/components/ui/FSheet.vue";
import type { ModProjectGroup, ModProjectView } from "@/api/tauri";

/**
 * 分组管理（FSheet）：新建 / 改名 / 删除分组。
 * 删除分组只移除分组本身，组内项目回到未分组（后端外键 SET NULL）。
 */
const props = defineProps<{
  groups: ModProjectGroup[];
  projects: ModProjectView[];
}>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{
  create: [name: string];
  rename: [id: number, name: string];
  remove: [id: number];
}>();

const { t } = useI18n();
const newName = ref("");
const renamingId = ref<number | null>(null);
const renamingName = ref("");
const error = ref("");

watch(open, (value) => {
  if (value) {
    newName.value = "";
    renamingId.value = null;
    renamingName.value = "";
    error.value = "";
  }
});

function projectCount(groupId: number): number {
  return props.projects.filter((project) => project.groupId === groupId).length;
}

function errorMessage(cause: unknown): string {
  return cause && typeof cause === "object" && "message" in cause
    ? String(cause.message)
    : String(cause);
}

async function submitCreate() {
  if (!newName.value.trim()) return;
  try {
    await emit("create", newName.value);
    newName.value = "";
  } catch (cause) {
    error.value = errorMessage(cause);
  }
}

async function submitRename() {
  if (renamingId.value === null || !renamingName.value.trim()) return;
  try {
    await emit("rename", renamingId.value, renamingName.value);
    renamingId.value = null;
  } catch (cause) {
    error.value = errorMessage(cause);
  }
}

async function submitRemove(id: number) {
  if (!window.confirm(t("studio.projects.groups.deleteConfirm"))) return;
  try {
    await emit("remove", id);
  } catch (cause) {
    error.value = errorMessage(cause);
  }
}
</script>

<template>
  <FSheet v-model:open="open" :label="$t('studio.projects.groups.manage')">
    <div class="group-manager">
      <header class="head">
        <strong>{{ $t("studio.projects.groups.manage") }}</strong>
        <button
          class="close"
          type="button"
          :aria-label="$t('common.close')"
          @click="open = false"
        >
          <FIcon name="X" :size="14" aria-label="" />
        </button>
      </header>

      <form class="create-row" @submit.prevent="submitCreate">
        <input
          v-model="newName"
          type="text"
          required
          :placeholder="$t('studio.projects.groups.namePlaceholder')"
        />
        <button class="primary" type="submit" :disabled="!newName.trim()">
          <FIcon name="Plus" :size="13" aria-label="" />
          {{ $t("studio.projects.groups.create") }}
        </button>
      </form>

      <p v-if="error" class="error" role="alert">{{ error }}</p>

      <ul class="group-list">
        <li v-for="group in groups" :key="group.id" class="group-row">
          <template v-if="renamingId === group.id">
            <form class="rename-row" @submit.prevent="submitRename">
              <input v-model="renamingName" type="text" required />
              <button
                class="icon-button"
                type="submit"
                :aria-label="$t('common.save')"
              >
                <FIcon name="Check" :size="13" aria-label="" />
              </button>
              <button
                class="icon-button"
                type="button"
                :aria-label="$t('common.cancel')"
                @click="renamingId = null"
              >
                <FIcon name="X" :size="13" aria-label="" />
              </button>
            </form>
          </template>
          <template v-else>
            <span class="group-name">{{ group.name }}</span>
            <span class="group-count">{{ projectCount(group.id) }}</span>
            <button
              class="icon-button"
              type="button"
              :aria-label="$t('common.rename')"
              @click="
                renamingId = group.id;
                renamingName = group.name;
              "
            >
              <FIcon name="Pencil" :size="13" aria-label="" />
            </button>
            <button
              class="icon-button danger"
              type="button"
              :aria-label="$t('common.delete')"
              @click="submitRemove(group.id)"
            >
              <FIcon name="Trash2" :size="13" aria-label="" />
            </button>
          </template>
        </li>
        <li v-if="!groups.length" class="empty">
          {{ $t("studio.projects.groups.empty") }}
        </li>
      </ul>
      <p class="hint">{{ $t("studio.projects.groups.deleteHint") }}</p>
    </div>
  </FSheet>
</template>

<style scoped>
.group-manager {
  display: grid;
  gap: 14px;
  padding: 18px 16px;
}
.head {
  align-items: center;
  display: flex;
  justify-content: space-between;
}
.head strong {
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
.create-row {
  display: flex;
  gap: 8px;
}
.create-row input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  flex: 1;
  font: inherit;
  font-size: 12.5px;
  min-height: 30px;
  padding: 4px 10px;
}
.primary {
  align-items: center;
  background: var(--primary);
  border: 1px solid var(--primary);
  border-radius: var(--radius-sm);
  color: #fff;
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 6px;
  min-height: 30px;
  padding: 0 12px;
}
.primary:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.error {
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  overflow-wrap: anywhere;
}
.group-list {
  display: grid;
  gap: 0;
  list-style: none;
  margin: 0;
  padding: 0;
}
.group-row {
  align-items: center;
  border-top: 1px solid var(--border);
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) auto auto auto;
  min-height: 38px;
  padding: 4px 0;
}
.group-name {
  font-size: 12.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.group-count {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--muted-foreground);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  padding: 1px 8px;
}
.icon-button {
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  min-height: 26px;
  min-width: 26px;
  align-items: center;
  justify-content: center;
}
.icon-button:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.icon-button.danger:hover {
  color: var(--danger);
}
.rename-row {
  display: flex;
  gap: 6px;
  grid-column: 1 / -1;
}
.rename-row input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  flex: 1;
  font: inherit;
  font-size: 12.5px;
  min-height: 28px;
  padding: 2px 8px;
}
.empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  list-style: none;
  padding: 18px 0;
  text-align: center;
}
.hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
}
</style>
