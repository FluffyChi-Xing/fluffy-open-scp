<script setup lang="ts">
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import { Resizable, ResizableHandle, ResizablePanel } from "@/components/ui/resizable";

const { t } = useI18n();
const selected = defineModel<string>({ default: "building" });
const nodes = [
  { id: "building", icon: "Box", label: "studio.assetWorkbench.nodes.building" },
  { id: "materials", icon: "Palette", label: "studio.assetWorkbench.nodes.materials" },
  { id: "lot", icon: "LandPlot", label: "studio.assetWorkbench.nodes.lot" },
];
</script>

<template>
  <section class="asset-workbench" :aria-label="t('studio.assetWorkbench.title')">
    <header class="asset-workbench-head">
      <div>
        <p class="eyebrow">{{ t("studio.assetWorkbench.eyebrow") }}</p>
        <h1>{{ t("studio.assetWorkbench.title") }}</h1>
        <p>{{ t("studio.assetWorkbench.description") }}</p>
      </div>
      <div class="head-actions">
        <button type="button"><FIcon name="Save" :size="14" aria-label="" />{{ t("studio.assetWorkbench.save") }}</button>
        <button type="button"><FIcon name="Maximize2" :size="14" aria-label="" />{{ t("studio.assetWorkbench.focus") }}</button>
      </div>
    </header>
    <Resizable direction="horizontal" auto-save-id="openscp:asset-workbench:v1" class="asset-split">
      <ResizablePanel id="asset-workbench-tree" :default-size="22" :min-size="16">
        <aside class="asset-tree">
          <div class="pane-title">{{ t("studio.assetWorkbench.treeTitle") }}</div>
          <button
            v-for="node in nodes"
            :key="node.id"
            type="button"
            class="tree-node"
            :class="{ active: selected === node.id }"
            @click="selected = node.id"
          >
            <FIcon :name="node.icon" :size="15" aria-label="" />
            <span>{{ t(node.label) }}</span>
          </button>
        </aside>
      </ResizablePanel>
      <ResizableHandle orientation="horizontal" :label="t('studio.assetWorkbench.resizeHorizontal')" />
      <ResizablePanel id="asset-workbench-canvas" :default-size="56" :min-size="34">
        <main class="asset-canvas">
          <div class="canvas-toolbar" role="toolbar" :aria-label="t('studio.assetWorkbench.canvasToolbar')">
            <span class="canvas-badge"><FIcon name="Workflow" :size="14" aria-label="" />{{ t("studio.assetWorkbench.canvas") }}</span>
            <button type="button"><FIcon name="Scan" :size="13" aria-label="" />{{ t("studio.assetWorkbench.fit") }}</button>
            <button type="button"><FIcon name="Plus" :size="13" aria-label="" />{{ t("studio.assetWorkbench.addNode") }}</button>
          </div>
          <div class="canvas-grid">
            <div class="asset-node source"><strong>{{ t("studio.assetWorkbench.nodes.source") }}</strong><span>{{ t("studio.assetWorkbench.sourceHint") }}</span></div>
            <div class="asset-connector" aria-hidden="true"></div>
            <div class="asset-node target"><strong>{{ t(`studio.assetWorkbench.nodes.${selected}`) }}</strong><span>{{ t("studio.assetWorkbench.nodeHint") }}</span></div>
          </div>
        </main>
      </ResizablePanel>
      <ResizableHandle orientation="horizontal" :label="t('studio.assetWorkbench.resizeHorizontal')" />
      <ResizablePanel id="asset-workbench-inspector" :default-size="22" :min-size="18">
        <aside class="asset-inspector">
          <div class="pane-title">{{ t("studio.assetWorkbench.inspectorTitle") }}</div>
          <div class="inspector-card"><span>{{ t("studio.assetWorkbench.selection") }}</span><strong>{{ t(`studio.assetWorkbench.nodes.${selected}`) }}</strong></div>
          <p>{{ t("studio.assetWorkbench.inspectorHint") }}</p>
        </aside>
      </ResizablePanel>
    </Resizable>
  </section>
</template>

<style scoped>
.asset-workbench { display: flex; flex-direction: column; gap: 12px; height: 100%; min-height: 520px; }
.asset-workbench-head { align-items: flex-start; display: flex; justify-content: space-between; gap: 16px; }
.eyebrow { color: var(--primary); font-size: 11px; letter-spacing: .08em; margin: 0 0 5px; text-transform: uppercase; }
h1 { font-size: 22px; margin: 0; }
.asset-workbench-head p:not(.eyebrow) { color: var(--muted-foreground); font-size: 12px; margin: 5px 0 0; }
.head-actions { display: flex; gap: 6px; }
button { align-items: center; background: var(--surface-elevated); border: 1px solid var(--border); border-radius: var(--radius-sm); color: var(--foreground); cursor: pointer; display: inline-flex; font: inherit; font-size: 11px; gap: 5px; min-height: 29px; padding: 0 9px; }
button:hover, .tree-node.active { background: var(--surface-hover); border-color: var(--primary); }
.asset-split { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); flex: 1; min-height: 0; overflow: hidden; }
.asset-tree, .asset-inspector { height: 100%; overflow: auto; padding: 12px; }
.pane-title { border-bottom: 1px solid var(--border); color: var(--muted-foreground); font-size: 11px; font-weight: 700; letter-spacing: .06em; padding-bottom: 9px; text-transform: uppercase; }
.tree-node { justify-content: flex-start; margin-top: 5px; width: 100%; }
.asset-canvas { background-color: color-mix(in srgb, var(--surface) 88%, var(--primary)); background-image: radial-gradient(var(--border) 1px, transparent 1px); background-size: 20px 20px; height: 100%; min-width: 0; position: relative; }
.canvas-toolbar { align-items: center; background: color-mix(in srgb, var(--surface) 90%, transparent); border-bottom: 1px solid var(--border); display: flex; gap: 6px; padding: 8px 10px; }
.canvas-toolbar .canvas-badge { align-items: center; display: inline-flex; flex: 1; font-size: 11px; gap: 5px; }
.canvas-grid { align-items: center; display: flex; height: calc(100% - 46px); justify-content: center; gap: 18px; }
.asset-node { background: var(--surface-elevated); border: 1px solid var(--border); border-radius: var(--radius-md); box-shadow: var(--shadow-sm); display: grid; gap: 7px; max-width: 180px; padding: 13px; }
.asset-node strong { font-size: 12px; }
.asset-node span, .asset-inspector p { color: var(--muted-foreground); font-size: 11px; line-height: 1.5; }
.asset-connector { background: var(--primary); height: 2px; width: 48px; }
.inspector-card { border-bottom: 1px solid var(--border); display: grid; gap: 5px; padding: 14px 0; }
.inspector-card span { color: var(--muted-foreground); font-size: 11px; }
.inspector-card strong { font-size: 13px; }
</style>
