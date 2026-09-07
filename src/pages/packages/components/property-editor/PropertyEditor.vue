<script setup lang="ts">
import { computed, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FSheet from "@/components/ui/FSheet.vue";
import type { Tgi } from "@/api/tauri";
import PropertyEditorOutliner from "./PropertyEditorOutliner.vue";
import PropertyEditorViewport from "./PropertyEditorViewport.vue";
import PropertyEditorProperties from "./PropertyEditorProperties.vue";
import PropertyEditorStatusBar from "./PropertyEditorStatusBar.vue";
import { usePropertyEditorSession } from "./usePropertyEditorSession";

const props = defineProps<{ packageId: number; tgi: Tgi }>();
const open = defineModel<boolean>("open", { default: false });
const {
  session,
  loading,
  loadError,
  modelPayload,
  modelLods,
  activeLod,
  switchLod,
  modelState,
  selectedId,
  grouping,
  lotSize,
  lotPlacement,
  lotMaskPng,
  selectedUnit,
  hiddenUnits,
  groupVisibility,
  load,
  toggleGroup,
  toggleUnit,
} = usePropertyEditorSession(props.packageId, props.tgi);

const renderMode = ref<"default" | "refined">("default");

watch(open, (value) => {
  if (value) void load();
});
const title = computed(() => {
  const assetName = session.value?.assetName;
  if (assetName) return assetName;
  return `0x${props.tgi.instance.toString(16).padStart(8, "0").toUpperCase()}`;
});
const diagnostics = computed(() => session.value?.diagnostics ?? []);
</script>

<template>
  <FSheet v-model:open="open" :label="$t('package.propertyEditor')" width="100vw">
    <div v-if="open" class="editor-root">
      <header class="editor-header">
        <div class="editor-title">
          <strong>{{ $t("package.propertyEditor") }}</strong>
          <small>{{ title }}</small>
        </div>
        <div
          class="render-mode"
          role="group"
          :aria-label="$t('package.renderMode')"
          :title="$t('package.renderModeHint')"
        >
          <button
            type="button"
            :class="{ active: renderMode === 'default' }"
            @click="renderMode = 'default'"
          >
            {{ $t("package.renderModeDefault") }}
          </button>
          <button
            type="button"
            :class="{ active: renderMode === 'refined' }"
            @click="renderMode = 'refined'"
          >
            {{ $t("package.renderModeRefined") }}
          </button>
        </div>
        <span class="editor-readonly">{{ $t("package.propertyEditorReadonly") }}</span>
        <button
          class="editor-close"
          type="button"
          :aria-label="$t('shell.close')"
          @click="open = false"
        >
          <FIcon name="X" :size="15" aria-label="" />
        </button>
      </header>
      <p v-if="diagnostics.length" class="editor-diagnostics">
        <FIcon name="TriangleAlert" :size="13" aria-label="" />
        <span>{{ diagnostics.join(" · ") }}</span>
      </p>
      <div v-if="loading" class="editor-loading">
        <FSpinner size="sm" :label="$t('common.loading')" />
      </div>
      <p v-else-if="loadError" class="editor-error" role="alert">
        {{ $t(loadError) }}
      </p>
      <div v-else-if="session" class="editor-body">
        <PropertyEditorOutliner
          :grouping="grouping"
          :selected-id="selectedId"
          :hidden-units="hiddenUnits"
          :group-visibility="groupVisibility"
          @select="selectedId = $event"
          @toggle-unit="toggleUnit"
          @toggle-group="toggleGroup"
        />
        <PropertyEditorViewport
          :model-payload="modelPayload"
          :model-lods="modelLods"
          :active-lod="activeLod"
          :render-mode="renderMode"
          :grouping="grouping"
          :lot-size="lotSize"
          :lot-placement="lotPlacement"
          :lot-mask-png="lotMaskPng"
          :selected-id="selectedId"
          :hidden-units="hiddenUnits"
          :group-visibility="groupVisibility"
          :model-state="modelState"
          @select="selectedId = $event"
          @toggle-layer="toggleGroup"
          @switch-lod="switchLod"
        />
        <PropertyEditorProperties :unit="selectedUnit" />
      </div>
      <PropertyEditorStatusBar
        v-if="session"
        :grouping="grouping"
        :model-state="modelState"
        :selected-unit="selectedUnit"
      />
    </div>
  </FSheet>
</template>

<style scoped>
.editor-root {
  /* 条件渲染的诊断条/加载/错误会打乱 grid 行序，flex 列与子元素数量无关 */
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.editor-header {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 12px;
  padding: 10px 16px;
}
.editor-title {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}
.editor-title strong {
  font-size: 14px;
}
.editor-title small {
  color: var(--subtle-foreground);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.editor-readonly {
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--subtle-foreground);
  font-size: 11px;
  margin-inline-start: auto;
  padding: 3px 10px;
  white-space: nowrap;
}
.render-mode {
  display: inline-flex;
  margin-inline-start: auto;
}
.render-mode + .editor-readonly {
  margin-inline-start: 0;
}
.render-mode button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  color: var(--muted-foreground);
  cursor: pointer;
  font: inherit;
  font-size: 11px;
  min-height: 24px;
  padding: 2px 10px;
}
.render-mode button:first-child {
  border-end-end-radius: 0;
  border-start-end-radius: 0;
}
.render-mode button:last-child {
  border-end-start-radius: 0;
  border-start-start-radius: 0;
  margin-inline-start: -1px;
}
.render-mode button.active {
  color: var(--foreground);
  opacity: 0.85;
}
.editor-close {
  align-items: center;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  justify-content: center;
  min-height: 28px;
  min-width: 28px;
}
.editor-close:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.editor-diagnostics {
  align-items: center;
  background: var(--surface-elevated);
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  gap: 7px;
  margin: 0;
  overflow: hidden;
  padding: 6px 16px;
}
.editor-loading {
  display: grid;
  flex: 1;
  justify-items: center;
  align-content: center;
  min-height: 0;
}
.editor-error {
  color: var(--danger);
  flex: 1;
  font-size: 12px;
  padding: 24px 16px;
  text-align: center;
}
.editor-body {
  display: grid;
  flex: 1;
  grid-template-columns: 220px minmax(0, 1fr) 300px;
  min-height: 0;
}
@media (max-width: 960px) {
  .editor-body {
    grid-template-columns: 180px minmax(0, 1fr) 240px;
  }
}
</style>
