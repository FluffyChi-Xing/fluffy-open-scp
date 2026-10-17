<script setup lang="ts">
import { storeToRefs } from "pinia";
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import UiWorkbenchStage from "../UiWorkbenchStage.vue";
import { useUiWorkbenchStore } from "@/stores/uiWorkbench";

const { t } = useI18n();
const store = useUiWorkbenchStore();
const { entered, selectedMenuId } = storeToRefs(store);
onMounted(() => { if (!store.replica) void store.load(); });
</script>

<template>
  <section class="ui-menu-bridge" :aria-label="t('flow.uiMenuBridgeTitle')">
    <header class="bridge-head">
      <div>
        <p class="bridge-eyebrow">{{ t("flow.uiMenuNode") }}</p>
        <h2>{{ t("flow.uiMenuBridgeTitle") }}</h2>
        <p>{{ t("flow.uiMenuBridgeHint") }}</p>
      </div>
      <span class="bridge-port"><FIcon name="PanelTop" :size="13" /> {{ t("flow.uiPreviewPort") }}</span>
    </header>
    <div class="bridge-actions">
      <button type="button" @click="store.addUniversitySidewalkToRoad">
        <FIcon name="Plus" :size="13" /> {{ t("flow.addUniversitySidewalk") }}
      </button>
      <button type="button" class="secondary" @click="store.setViewMode('city')">
        {{ t("flow.openCityMenu") }}
      </button>
      <span v-if="entered && selectedMenuId" class="bridge-state">{{ t("flow.uiMenuConnected") }}</span>
    </div>
    <UiWorkbenchStage class="bridge-stage" />
  </section>
</template>

<style scoped>
.ui-menu-bridge { display: flex; flex-direction: column; gap: 10px; padding: 12px 16px 16px; border-top: 1px solid var(--border); background: var(--surface-elevated); }
.bridge-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.bridge-eyebrow { margin: 0 0 3px; color: var(--primary); font-size: 10px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; }
.bridge-head h2 { margin: 0; font-size: 14px; }
.bridge-head p:last-child { margin: 4px 0 0; color: var(--muted-foreground); font-size: 11px; }
.bridge-port { display: inline-flex; align-items: center; gap: 4px; padding: 4px 7px; border: 1px solid var(--border); border-radius: 999px; color: var(--muted-foreground); font-size: 10px; white-space: nowrap; }
.bridge-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 7px; }
.bridge-actions button { display: inline-flex; align-items: center; gap: 5px; }
.bridge-actions .secondary { color: var(--muted-foreground); }
.bridge-state { color: var(--success); font-size: 11px; }
.bridge-stage { height: 360px; min-height: 260px; }
</style>
