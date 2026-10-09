<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { Handle, Position } from "@vue-flow/core";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import AssetFlowTool from "./AssetFlowTool.vue";
import { BUILDING_SLOTS } from "./assetFlow";
import NodeConfigForm from "./NodeConfigForm.vue";
import type { FlowSchema } from "./contracts";
const props = defineProps<{
  kind: string;
  project: string;
  schemas: FlowSchema[];
  schema?: FlowSchema;
  invalid: boolean;
  busy: boolean;
}>();
const emit = defineEmits<{
  edit: [];
  save: [schema: FlowSchema];
  dirty: [value: boolean];
  resize: [];
}>();
const { t } = useI18n();
const collapsed = shallowRef(false),
  dirty = shallowRef(false);
const source = computed(() =>
  ["map-source", "static-resource", "texture-input"].includes(props.kind),
);
const output = computed(() => props.kind === "output");
const complex = computed(() => props.kind === "static-resource");
function setDirty(value: boolean) {
  dirty.value = value;
  emit("dirty", value);
}
</script>
<template>
  <article
    class="workflow-node"
    :class="{ invalid, source, output, 'map-node': kind === 'map-source', 'asset-node': kind === 'building-asset' }"
  >
    <header class="node-heading">
      <button
        class="collapse nodrag"
        :aria-label="t(collapsed ? 'flow.expandNode' : 'flow.collapseNode')"
        :aria-expanded="!collapsed"
        @click="collapsed = !collapsed"
      >
        <FIcon :name="collapsed ? 'ChevronRight' : 'ChevronDown'" :size="14" />
      </button>
      <h3>{{ kind === 'texture-input' ? `${BUILDING_SLOTS[Number(schema?.config.slot) || 0].id} · ${BUILDING_SLOTS[Number(schema?.config.slot) || 0].label}` : t(`flow.nodes.${kind}`) }}</h3>
      <span
        class="status-dot"
        :class="{ dirty }"
        :title="
          t(
            dirty
              ? 'flow.unsaved'
              : invalid
                ? 'flow.needsSetup'
                : 'flow.configured',
          )
        "
      />
    </header>
    <div class="node-ports">
      <span v-if="!source">{{ t("flow.inputPort") }}</span
      ><span v-if="!output" class="output-label">{{
        t(kind === "map-source" ? "flow.map" : "flow.outputPort")
      }}</span>
    </div>
    <Handle
      v-if="!source && kind !== 'building-asset'"
      type="target"
      :position="Position.Left"
      :style="{ top: '55px' }"
    /><Handle
      v-if="!output"
      type="source"
      :position="Position.Right"
      :style="{ top: '55px' }"
    />
    <div v-if="kind === 'building-asset'" class="asset-ports">
      <div v-for="(slot, index) in BUILDING_SLOTS" :key="slot.id" class="asset-port">
        <Handle :id="slot.id" type="target" :position="Position.Left" :style="{ top: `${94 + index * 28}px` }" />
        {{ slot.id }} · {{ slot.label }}
      </div>
    </div>
    <div v-show="!collapsed">
      <AssetFlowTool v-if="schema && ['texture-input', 'building-asset'].includes(kind)" :project="project" :schema="schema" :schemas="schemas" :busy="busy" @save="emit('save', $event)" @dirty="setDirty" @resize="emit('resize')" />
      <div v-else-if="complex" class="complex-tool nodrag">
        <p>{{ t("flow.assetToolHint") }}</p>
        <button :disabled="busy" @click="emit('edit')">
          <FIcon name="SlidersHorizontal" :size="13" />{{
            t("flow.openEditor")
          }}
        </button>
      </div>
      <NodeConfigForm
        v-else
        :schema="schema ?? null"
        :busy="busy"
        @save="emit('save', $event)"
        @dirty="setDirty"
        @resize="emit('resize')"
      />
    </div>
  </article>
</template>
<style scoped>
.asset-ports { padding: 6px 16px; font-size: 12px; }
.asset-port { height: 28px; display: flex; align-items: center; }
.workflow-node {
  --node-accent: var(--primary);
  width: 340px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  background: var(--surface-elevated);
  color: var(--foreground);
  box-shadow: 0 4px 14px #0002;
}
.workflow-node.asset-node { width: 460px; }
.workflow-node.map-node {
  width: 420px;
}
.workflow-node.output {
  --node-accent: var(--success, #42a58a);
}
.node-heading {
  height: 36px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px 0 6px;
  background: color-mix(in srgb, var(--node-accent) 10%, var(--surface-hover));
  border-radius: 9px 9px 0 0;
  cursor: grab;
}
.node-heading h3 {
  font-size: 13px;
  font-weight: 600;
  margin: 0;
  flex: 1;
}
.collapse {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--muted-foreground);
  cursor: pointer;
}
.collapse:hover {
  background: var(--surface-hover);
}
.node-ports {
  display: flex;
  align-items: center;
  height: 34px;
  padding: 0 14px;
  font-size: 11px;
  color: var(--muted-foreground);
}
.output-label {
  margin-left: auto;
}
.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--success, #42a58a);
}
.invalid .status-dot,
.status-dot.dirty {
  background: var(--warning, #d6a451);
}
.workflow-node :deep(.vue-flow__handle) {
  height: 10px;
  width: 10px;
  background: var(--node-accent);
  border: 2px solid var(--surface-elevated);
}
.complex-tool {
  padding: 0 12px 12px;
}
.complex-tool p {
  font-size: 11px;
  color: var(--muted-foreground);
  line-height: 1.5;
}
.complex-tool button {
  display: flex;
  gap: 6px;
  align-items: center;
  height: 26px;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: 5px;
  font-size: 11px;
  background: var(--surface);
  color: inherit;
  cursor: pointer;
}
</style>
