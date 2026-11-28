<script setup lang="ts">
import {
  defineAsyncComponent,
  computed,
  shallowRef,
  watch,
  onBeforeUnmount,
  nextTick,
} from "vue";
import { useI18n } from "vue-i18n";
import {
  MousePointer2,
  RefreshCw,
  Search,
  Monitor,
  FolderInput,
} from "lucide-vue-next";
import FButton from "@/components/ui/FButton.vue";
import SidebarContent from "@/components/ui/sidebar/SidebarContent.vue";
import SidebarMenu from "@/components/ui/sidebar/SidebarMenu.vue";
import ElementTreeNode from "./ElementTreeNode.vue";
import {
  readElements,
  elementModel,
  type DomFrame,
  type DomNode,
} from "./elements";
import { open } from "@tauri-apps/plugin-dialog";
import { command } from "@/api/tauri";
import { useGameInspector } from "./useGameInspector";
import { runtimeObserver } from "@/api/runtimeObserver";
const MonacoEditor = defineAsyncComponent(
  () => import("@/components/ui/MonacoEditor.vue"),
);
const panel = shallowRef<HTMLElement>();
const props = defineProps<{ pid?: number; active?: boolean }>();
const { t } = useI18n();
const frame = shallowRef<DomFrame | null>(null);
const selected = shallowRef<DomNode>();
const hovered = shallowRef<DomNode>();
const picking = shallowRef(false);
const image = shallowRef("");
const error = shallowRef("");
const busy = shallowRef(false);
const query = shallowRef("");
const model = computed(() => (frame.value ? elementModel(frame.value) : null));
const roots = computed(() =>
  query.value
    ? model.value?.nodes
        .filter((n) =>
          [n.tag, n.classes, n.id, n.text]
            .join(" ")
            .toLowerCase()
            .includes(query.value.toLowerCase()),
        )
        .slice(0, 200)
    : model.value?.children.get(-1),
);
const expanded = computed(() => {
  const result = new Set<number>();
  let n = selected.value;
  while (n && model.value?.byId.has(n.parent) && !result.has(n.parent)) {
    result.add(n.parent);
    n = model.value.byId.get(n.parent);
  }
  return result;
});
const highlight = computed(() => {
  const r = hovered.value?.rect,
    v = frame.value?.viewport;
  return r && v
    ? {
        left: (r[0] / v[0]) * 100 + "%",
        top: (r[1] / v[1]) * 100 + "%",
        width: ((r[2] - r[0]) / v[0]) * 100 + "%",
        height: ((r[3] - r[1]) / v[1]) * 100 + "%",
      }
    : {};
});
const inspector = useGameInspector(
  () => props.pid,
  frame,
  async () => {
    await refresh(false);
  },
  (id) => {
    const node = model.value?.byId.get(id);
    if (node) void selectPicked(node);
  },
);
async function importCapture() {
  try {
    const source = await open({
      directory: true,
      multiple: false,
      title: t("devtool.importCapture"),
    });
    if (typeof source === "string") {
      await inspector.disconnect();
      await command("devtool_import_capture", { source });
      inspector.error.value = "";
    }
  } catch (e) {
    inspector.error.value = String((e as { message?: string }).message ?? e);
  }
}
watch(
  () => props.active,
  (active) => {
    if (active === false) void inspector.control("clear");
  },
);
watch(hovered, (node) => {
  if (inspector.connected.value)
    void inspector.control(node ? "highlight" : "clear", node);
});
let generation = 0;
onBeforeUnmount(() => {
  generation++;
});
async function refresh(withImage = true) {
  if (!props.pid) return;
  const pid = props.pid,
    id = ++generation;
  busy.value = true;
  error.value = "";
  try {
    const data = await readElements(pid);
    if (id !== generation) return;
    frame.value = data;
    selected.value = undefined;
    hovered.value = undefined;
    image.value = "";
    if (!withImage) return;
    const shot = await runtimeObserver.frame(pid);
    if (id === generation)
      image.value = `data:${shot.mime};base64,${shot.dataBase64}`;
  } catch (e) {
    if (id === generation)
      error.value =
        e instanceof Error
          ? e.message
          : String((e as { message?: string }).message ?? e);
  } finally {
    if (id === generation) busy.value = false;
  }
}
watch(
  () => props.pid,
  () => {
    generation++;
    frame.value = null;
    image.value = "";
    selected.value = undefined;
    hovered.value = undefined;
    busy.value = false;
  },
);
async function selectPicked(node: DomNode) {
  query.value = "";
  selected.value = node;
  picking.value = false;
  await nextTick();
  panel.value
    ?.querySelector<HTMLElement>(`[data-node-id="${node.i}"]`)
    ?.scrollIntoView({ block: "nearest", inline: "nearest" });
}
function point(e: MouseEvent, choose = false) {
  if (!picking.value || !frame.value) return;
  const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const x = ((e.clientX - b.left) / b.width) * frame.value.viewport[0],
    y = ((e.clientY - b.top) / b.height) * frame.value.viewport[1];
  const hits = frame.value.boxes.filter(
    (n) =>
      n.rect &&
      n.rect[0] <= x &&
      x < n.rect[2] &&
      n.rect[1] <= y &&
      y < n.rect[3],
  );
  hits.sort(
    (a, b) =>
      (a.rect![2] - a.rect![0]) * (a.rect![3] - a.rect![1]) -
      (b.rect![2] - b.rect![0]) * (b.rect![3] - b.rect![1]),
  );
  hovered.value = hits[0];
  if (choose && hits[0]) {
    const node = model.value?.byId.get(hits[0].i);
    if (node) void selectPicked(node);
  }
}
</script>
<template>
  <section
    ref="panel"
    class="elements-panel"
    @keydown.esc="
      picking = false;
      hovered = undefined;
    "
    @keydown.ctrl.shift.c.prevent="
      inspector.connected.value
        ? inspector.control('pick')
        : (picking = image ? !picking : false)
    "
  >
    <div class="elements-toolbar">
      <FButton
        variant="ghost"
        size="sm"
        class="picker-button"
        :title="t('devtool.pick')"
        :disabled="!image && !(inspector.connected.value && frame)"
        :aria-label="t('devtool.pick')"
        :aria-pressed="picking || inspector.picking.value"
        @click="
          inspector.connected.value
            ? inspector.control('pick')
            : (picking = !picking)
        "
        ><MousePointer2 :size="16"
      /></FButton>
      <slot name="process" />
      <FButton
        size="sm"
        variant="ghost"
        :title="t('devtool.importCapture')"
        :aria-label="t('devtool.importCapture')"
        @click="importCapture"
        ><FolderInput :size="16"
      /></FButton>
      <FButton
        size="sm"
        variant="ghost"
        :disabled="!pid"
        :loading="inspector.connecting.value"
        :title="
          t(
            inspector.connected.value
              ? 'devtool.disconnect'
              : 'devtool.connect',
          )
        "
        :aria-label="
          t(
            inspector.connected.value
              ? 'devtool.disconnect'
              : 'devtool.connect',
          )
        "
        @click="
          inspector.connected.value
            ? inspector.disconnect()
            : inspector.connect()
        "
        ><Monitor :size="16"
      /></FButton>
      <div class="element-search">
        <Search :size="14" aria-hidden="true" />
        <input
          v-model="query"
          class="search"
          :placeholder="t('devtool.search')"
          :aria-label="t('devtool.search')"
        />
      </div>
      <FButton
        size="sm"
        class="snapshot-button"
        variant="secondary"
        :disabled="!pid"
        :loading="busy"
        @click="refresh()"
        ><RefreshCw :size="14" />{{ t("devtool.refresh") }}</FButton
      >
    </div>
    <p v-if="inspector.error.value" role="alert" class="error">
      {{ t("devtool.bridgeError") }} · {{ inspector.error.value }}
    </p>
    <p v-if="error" role="alert" class="error">
      {{ t("devtool.readError") }} · {{ error }}
    </p>
    <p v-if="frame?.truncated" role="status">{{ t("devtool.partial") }}</p>
    <div v-if="frame" class="elements-split">
      <SidebarContent
        class="element-sidebar"
        :aria-label="t('devtool.elements')"
      >
        <SidebarMenu role="list">
          <ElementTreeNode
            v-for="node in roots"
            :key="node.i"
            :node="node"
            :children="model!.children"
            :selected="selected?.i"
            :expanded="expanded"
            @select="selected = $event"
            @hover="hovered = $event"
          />
        </SidebarMenu>
        <p v-if="!roots?.length">{{ t("devtool.noMatches") }}</p>
      </SidebarContent>
      <aside class="details">
        <div
          v-if="image"
          class="game-preview"
          :style="{ aspectRatio: frame.viewport.join('/') }"
          @mousemove="point($event)"
          @click="point($event, true)"
          @mouseleave="hovered = undefined"
        >
          <img :src="image" :alt="t('devtool.preview')" />
          <div v-if="hovered?.rect" class="highlight" :style="highlight" />
        </div>
        <p v-if="image">{{ t("devtool.previewNotice") }}</p>
        <h2>{{ t("devtool.details") }}</h2>
        <div v-if="selected" class="node-editor">
          <MonacoEditor
            :value="JSON.stringify(selected, null, 2)"
            language="json"
            fill
          />
        </div>
        <p v-else>{{ t("devtool.noSelection") }}</p>
      </aside>
    </div>
    <div v-else class="empty">
      {{ pid ? t("devtool.empty") : t("devtool.choose") }}
    </div>
  </section>
</template>
<style scoped>
.elements-panel {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--surface);
  min-height: 65vh;
}
.elements-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  background: var(--surface-elevated);
}
.picker-button {
  width: 30px;
  height: 30px;
  padding: 0;
  flex: none;
}
.picker-button[aria-pressed="true"] {
  color: var(--primary);
  background: var(--sidebar-active);
}
.snapshot-button {
  min-height: 30px;
  flex: none;
  white-space: nowrap;
}
.element-search {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1 1 180px;
  min-width: 140px;
  height: 30px;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--background);
  color: var(--muted-foreground);
}
.element-search:focus-within {
  border-color: var(--ring);
}
.search {
  width: 100%;
  min-width: 0;
  padding: 0;
  border: 0;
  outline: none;
  background: transparent;
  color: var(--foreground);
  font-size: 12px;
  line-height: 1.4;
}
@media (max-width: 600px) {
  .element-search {
    flex-basis: calc(100% - 140px);
  }
}
.elements-split {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(260px, 30%);
  height: 65vh;
}
.element-sidebar {
  overflow: auto;
  padding: 8px;
  background: var(--sidebar);
}
.details {
  overflow: auto;
  border-left: 1px solid var(--border);
  padding: 12px;
  background: var(--surface-elevated);
  font-size: 12px;
}
.details h2 {
  font-size: 12px;
  margin: 16px 0 8px;
}
.node-editor {
  height: 360px;
  min-width: 0;
}
.details pre {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.details p,
.empty {
  color: var(--muted-foreground);
}
.empty {
  display: grid;
  place-content: center;
  min-height: 55vh;
}
.game-preview {
  position: relative;
  width: 100%;
  cursor: crosshair;
}
.game-preview img {
  width: 100%;
  height: 100%;
  object-fit: fill;
}
.highlight {
  position: absolute;
  border: 2px solid var(--primary);
  background: color-mix(in srgb, var(--primary) 12%, transparent);
  pointer-events: none;
}
.error {
  color: var(--danger);
  padding: 8px;
}
@media (max-width: 800px) {
  .elements-split {
    grid-template-columns: 1fr;
    height: auto;
  }
  .element-sidebar {
    height: 50vh;
  }
  .details {
    border-left: 0;
    border-top: 1px solid var(--border);
  }
}
</style>
