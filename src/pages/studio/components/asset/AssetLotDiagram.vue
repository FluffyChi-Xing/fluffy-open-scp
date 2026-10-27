<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { isTauri } from "@/api";
import { loadLotPreviewSession } from "@/components/raster/lotPreviewSession";
import { useGamePackagesStore } from "@/stores/gamePackages";
import type { LotEditorSession, Tgi } from "@/api/tauri";
import PropertyEditorViewport from "@/pages/packages/components/property-editor/PropertyEditorViewport.vue";

/**
 * Lot 分层示意图卡片（复用 raster 面板 Lot 预览链路）：loadLotPreviewSession
 * 绑定该 Lot 的会话数据（LotMask 四色量化地面/尺寸/边框带），以 ground-only
 * 视口渲染默认模式分层效果。视口随卡片进入可视区才挂载（IntersectionObserver）。
 *
 * 并发预算（2026-10-27 白屏修复）：每张卡片 = 一个独立 WebGL 上下文，浏览器
 * 每页上限 ~16 个，超限时最老的上下文（通常是主画布）被强制回收 → 主视口
 * 白屏、移动视角更白、GC 后自愈且控制台无报错。故：①整库并发卡片视口 ≤ 3；
 * ②卡片视口卸载时对画布主动 loseContext，不等 GC 回收僵尸 canvas。
 */
const MAX_VIEWPORT_SLOTS = 3;
const slots: { active: number; waiters: (() => void)[] } = { active: 0, waiters: [] };
function acquireSlot(): boolean {
  if (slots.active >= MAX_VIEWPORT_SLOTS) return false;
  slots.active += 1;
  return true;
}
function queueForSlot(continuation: () => void): void {
  slots.waiters.push(continuation);
}
function cancelQueued(continuation: () => void): void {
  const at = slots.waiters.indexOf(continuation);
  if (at >= 0) slots.waiters.splice(at, 1);
}
function releaseSlot(): void {
  slots.active = Math.max(0, slots.active - 1);
  const next = slots.waiters.shift();
  if (next) {
    slots.active += 1;
    next();
  }
}

const props = defineProps<{
  packageId: number;
  tgi: Tgi;
}>();

const { t } = useI18n();
const packages = useGamePackagesStore();
const root = ref<HTMLElement | null>(null);
const canvasHost = ref<HTMLElement | null>(null);
const visible = ref(typeof IntersectionObserver === "undefined");
const session = shallowRef<LotEditorSession | null>(null);
const atlas = shallowRef<Partial<LotEditorSession> | null>(null);
const loading = ref(false);
const error = ref("");
/** 持有渲染槽位的卡片才挂载视口（并发预算内）。 */
const hasSlot = ref(false);
let generation = 0;
let visibilityObserver: IntersectionObserver | null = null;
let queued: (() => void) | null = null;

/** 立即释放卡片 GL 上下文（视口卸载依赖 GC 回收 canvas，快速滚动会累积
 * 僵尸上下文触发浏览器上限回收；WEBGL_lose_context 使其即时腾位）。
 * 调用时机（watcher pre-flush / onBeforeUnmount）画布必然仍挂载在
 * canvasHost 下，实时查询即可。 */
function releaseGlContext(): void {
  const canvas = canvasHost.value?.querySelector("canvas");
  if (!canvas) return;
  const gl = (canvas.getContext("webgl2") ??
    canvas.getContext("webgl")) as WebGLRenderingContext | null;
  gl?.getExtension("WEBGL_lose_context")?.loseContext();
}
function giveBackSlot(): void {
  if (queued) {
    cancelQueued(queued);
    queued = null;
  }
  if (!hasSlot.value) return;
  hasSlot.value = false;
  releaseGlContext();
  releaseSlot();
}

onMounted(() => {
  if (!root.value || typeof IntersectionObserver === "undefined") return;
  visibilityObserver = new IntersectionObserver(
    ([entry]) => {
      visible.value = entry.isIntersecting;
    },
    // rootMargin 滞回：视口边缘 ±160px 内的卡片保持挂载，减少慢速滚动
    // 的挂载/卸载抖动；快速甩动仍会及时释放。
    { root: root.value.closest(".lot-library"), threshold: 0.01, rootMargin: "160px 0px" },
  );
  visibilityObserver.observe(root.value);
});
onBeforeUnmount(() => {
  generation += 1;
  visibilityObserver?.disconnect();
  giveBackSlot();
});

watch(
  () => [visible.value, props.packageId, props.tgi] as const,
  async () => {
    const epoch = ++generation;
    if (!visible.value || !isTauri()) return;
    loading.value = true;
    error.value = "";
    try {
      const value = await loadLotPreviewSession(
        { name: "", packageId: props.packageId, tgi: props.tgi },
        packages.opened.map((p) => p.package.packageId),
        () => epoch !== generation,
      );
      if (epoch !== generation) return;
      session.value = value.bound;
      atlas.value = value.atlas;
    } catch (cause) {
      if (epoch === generation) error.value = String(cause);
    } finally {
      if (epoch === generation) loading.value = false;
    }
  },
  { immediate: true },
);

// 槽位状态机：可见 + 会话就绪才申请槽位；失去任一条件即归还并释放上下文。
watch(
  () => [visible.value, session.value !== null, error.value !== ""] as const,
  ([vis, ready, failed]) => {
    const want = vis && ready && !failed;
    if (want && !hasSlot.value) {
      if (acquireSlot()) {
        hasSlot.value = true;
      } else if (!queued) {
        const continuation = () => {
          queued = null;
          hasSlot.value = true;
        };
        queued = continuation;
        queueForSlot(continuation);
      }
    } else if (!want) {
      giveBackSlot();
    }
  },
);

const grouping = {
  lights: [],
  decals: [],
  props: [],
  effects: [],
  spawners: [],
  pathPoints: [],
};
const emptyMap = new Map();
const emptySet = new Set<string>();
const treeIds = new Set<number>();
const pngUrl = (value?: string | null) =>
  value ? `data:image/png;base64,${value}` : null;
</script>

<template>
  <div ref="root" class="lot-diagram">
    <p v-if="loading" role="status">{{ t("rasterEditor.loadingPreview") }}</p>
    <p v-else-if="error" role="alert">{{ t("rasterEditor.previewError") }} {{ error }}</p>
    <div
      v-else-if="visible && hasSlot && session"
      ref="canvasHost"
      class="lot-diagram-host"
    >
      <PropertyEditorViewport
        ground-only
        :model-payload="null"
        :model-lods="[]"
        :active-lod="0"
        render-mode="default"
        :grouping="grouping"
        :lot-size="session.lotSize"
        :lot-tile-period="session.lotTilePeriod ?? [6, 6]"
        :lot-placement="null"
        :lot-colors="session.lotColors"
        :lot-colors-authored="[true, true, true, true]"
        :lot-border-colors="session.lotBorderColors ?? []"
        :lot-border-widths="session.lotBorderWidths ?? [0, 0, 0, 0]"
        :lot-border-pattern-indices="session.lotBorderPatternIndices ?? [0, 0, 0, 0]"
        :lot-color-heights="session.lotColorHeights"
        :lot-border-heights="session.lotBorderHeights"
        :lot-base-tile="session.lotBaseTile ?? 8"
        :lot-overlay-box-offset="null"
        :lot-model-bbox-center="null"
        :lot-mask-png="pngUrl(session.lotMaskPng)"
        :lot-mask-raw-rgba="session.lotMaskRawRgba ?? null"
        :lot-albedo-png="pngUrl(session.lotAlbedoPng ?? atlas?.lotAlbedoPng)"
        :lot-surface-png="null"
        :lot-normal-atlas-png="null"
        :prop-models="emptyMap"
        :prop-tree-ids="treeIds"
        :tree-model-payloads="[]"
        :tree-atlas-png="null"
        :decal-textures="[]"
        :selected-id="null"
        :hidden-units="emptySet"
        :group-visibility="{ lot: true, dimensions: false }"
        model-state="missing"
      />
    </div>
    <p v-else role="status">{{ t("rasterEditor.loadingPreview") }}</p>
  </div>
</template>

<style scoped>
.lot-diagram {
  background: #10192b;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  height: 118px;
  min-height: 0;
  overflow: hidden;
  position: relative;
}
.lot-diagram-host {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}
.lot-diagram :deep(.viewport-pane) {
  flex: 1;
  min-height: 0;
}
.lot-diagram p {
  color: var(--muted-foreground);
  font-size: 10.5px;
  margin: 0;
  padding: 8px;
  text-align: center;
}
</style>
