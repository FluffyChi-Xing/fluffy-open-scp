<script setup lang="ts">
import { computed, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import { isTauri, tauriApi } from "@/api";
import type { SemanticCard } from "@/api/tauri";
import {
  barGradientCss,
  cardParent,
  keyText,
  textOf,
} from "./semanticCard";
import MeshPreviewView from "./MeshPreview.vue";

/**
 * 语义预览卡（Alert / SimAction / MapLayer / Vehicle / Road / Menu）：
 * 后端 extract_semantic_card 已按 semantic tag 抽好结构化载荷并解析了
 * text 文案（游戏 Locale 包），本组件只做渲染与图标/Parent/模型的按需加载。
 * 载具卡的 3D 模式：Vehicle Models → RW4 网格节（0x20009）→ OBJ → three.js。
 */
const props = defineProps<{ card: SemanticCard; packageId: number }>();

/** 三卡共有的 Parent 引用。 */
const parent = computed(() => cardParent(props.card));
/** alert / map-layer / menu 的图标引用。 */
const icon = computed(() =>
  "icon" in props.card ? props.card.icon : null,
);

// ---- 图标（与 property 同包的 PNG 资源，readData → data URL） ----
const iconUrl = ref<string | null>(null);
watch(
  () => [props.packageId, icon.value] as const,
  ([packageId, iconRef]) => {
    iconUrl.value = null;
    if (!iconRef || !isTauri()) return;
    tauriApi.packages
      .readData(packageId, {
        typeId: iconRef.typeId,
        group: iconRef.groupId,
        instance: iconRef.instanceId,
      })
      .then((data) => {
        iconUrl.value = `data:image/png;base64,${data.dataBase64}`;
      })
      .catch(() => {
        iconUrl.value = null;
      });
  },
  { immediate: true },
);

// ---- Parent 名称（解析失败仅显示 TGI） ----
const parentName = ref<string | null>(null);
watch(
  () => [props.packageId, parent.value] as const,
  ([packageId, parentRef]) => {
    parentName.value = null;
    if (!parentRef || !isTauri()) return;
    tauriApi.packages
      .resolveNames(packageId, [
        {
          typeId: parentRef.typeId,
          group: parentRef.groupId,
          instance: parentRef.instanceId,
        },
      ])
      .then((rows) => {
        parentName.value = rows[0]?.displayName ?? null;
      })
      .catch(() => {
        parentName.value = null;
      });
  },
  { immediate: true },
);

// ---- 装载槽资源名批量解析（kResourceID* 实例多在注册表有名字） ----
const slotNames = ref<Map<number, string>>(new Map());
watch(
  () =>
    props.card.kind === "resource-entry"
      ? ([props.packageId, props.card.slots] as const)
      : null,
  async (payload) => {
    slotNames.value = new Map();
    if (!payload) return;
    const [packageId, slots] = payload;
    if (!slots.length || !isTauri()) return;
    try {
      const rows = await tauriApi.packages.resolveNames(
        packageId,
        slots.map((slot) => ({
          typeId: slot.resource.typeId,
          group: slot.resource.groupId,
          instance: slot.resource.instanceId,
        })),
      );
      const map = new Map<number, string>();
      rows.forEach((row, i) => {
        if (row.displayName) map.set(i, row.displayName);
      });
      slotNames.value = map;
    } catch {
      /* 无 tauri / 解析失败：显示哈希 */
    }
  },
  { immediate: true },
);

// ---- MapLayer 数据条色带 → CSS 渐变 ----
const barGradient = computed(() =>
  props.card.kind === "map-layer"
    ? barGradientCss(props.card.barColors)
    : null,
);

const cardTitle = computed(() => {
  if (props.card.kind === "alert") {
    return props.card.texts.map(textOf).join(" ");
  }
  if (props.card.kind === "map-layer") {
    return props.card.name.length
      ? props.card.name.map(textOf).join(" ")
      : null;
  }
  return null;
});

// ---- 载具卡：列表 / 3D 切换，模型 → RW4 网格节（0x20009）→ OBJ ----
const RW4_MESH_TYPE_CODE = 0x20_009;
const show3d = ref(false);
const activeModelIndex = ref(0);
const meshObjs = ref<string[]>([]);
const meshState = ref<"idle" | "loading" | "ready" | "error">("idle");

watch([show3d, activeModelIndex], async ([on, index]) => {
  if (props.card.kind !== "vehicle" || !on) {
    return;
  }
  const model = props.card.models[index];
  if (!model || !isTauri()) {
    meshState.value = "error";
    return;
  }
  meshState.value = "loading";
  meshObjs.value = [];
  try {
    const tgi = {
      typeId: model.typeId,
      group: model.groupId,
      instance: model.instanceId,
    };
    const preview = await tauriApi.packages.readRw4Preview(
      props.packageId,
      tgi,
    );
    // 模型的全部 MESH 节（车身/部件分材质存储）合并渲染
    const meshSections = preview.sections.filter(
      (s) => s.typeCode === RW4_MESH_TYPE_CODE,
    );
    if (!meshSections.length) throw new Error("mesh section not found");
    const details = await Promise.all(
      meshSections.map((s) =>
        tauriApi.packages.readRw4Section(props.packageId, tgi, s.number),
      ),
    );
    meshObjs.value = details
      .map((d) => d.mesh?.objBase64)
      .filter((obj): obj is string => !!obj);
    meshState.value = meshObjs.value.length ? "ready" : "error";
  } catch {
    meshObjs.value = [];
    meshState.value = "error";
  }
});

function selectModel(index: number) {
  activeModelIndex.value = index;
}
</script>

<template>
  <div class="semantic-card">
    <!-- 警报卡 -->
    <template v-if="card.kind === 'alert'">
      <div class="alert-row">
        <div class="alert-icon">
          <img v-if="iconUrl" :src="iconUrl" :alt="$t('package.card.icon')" />
          <FIcon v-else name="Bell" :size="28" aria-label="" />
        </div>
        <div class="alert-body">
          <p class="alert-text">{{ cardTitle || $t("package.card.noText") }}</p>
          <div class="meta-row">
            <span
              v-if="card.durationSeconds != null"
              class="duration-badge"
            >{{
              $t("package.card.durationValue", {
                n: card.durationSeconds,
              })
            }}</span>
          </div>
        </div>
      </div>
    </template>

    <!-- 行动/任务卡 -->
    <template v-else-if="card.kind === 'sim-action'">
      <section class="card-section">
        <h4>{{ $t("package.card.actionTitles") }}</h4>
        <ul v-if="card.titles?.length" class="text-list">
          <li
            v-for="(t, i) in card.titles ?? []"
            :key="`t${i}`"
            :title="textOf(t)"
          >
            {{ textOf(t) }}
          </li>
        </ul>
        <p v-else class="card-empty">{{ $t("package.card.noText") }}</p>
      </section>
      <section v-if="card.failedTitles?.length" class="card-section">
        <h4>{{ $t("package.card.failedTitles") }}</h4>
        <ul class="text-list failed">
          <li
            v-for="(t, i) in card.failedTitles ?? []"
            :key="`f${i}`"
            :title="textOf(t)"
          >
            {{ textOf(t) }}
          </li>
        </ul>
      </section>
    </template>

    <!-- 数据图层卡 -->
    <template v-else-if="card.kind === 'map-layer'">
      <div class="alert-row">
        <div class="alert-icon">
          <img v-if="iconUrl" :src="iconUrl" :alt="$t('package.card.icon')" />
          <FIcon v-else name="MapPin" :size="28" aria-label="" />
        </div>
        <div class="alert-body">
          <p class="alert-text">
            {{ cardTitle || $t("package.card.noText") }}
          </p>
        </div>
      </div>
      <section v-if="barGradient" class="card-section">
        <h4>{{ $t("package.card.barColors") }}</h4>
        <div class="bar-colors" :style="{ background: barGradient }" />
      </section>
      <section v-if="card.legend" class="card-section">
        <h4>{{ $t("package.card.legend") }}</h4>
        <code class="ref-code">{{ keyText(card.legend) }}</code>
      </section>
    </template>

    <!-- 载具/小人卡：列表 / 3D 切换 -->
    <template v-else-if="card.kind === 'vehicle'">
      <div class="view-toggle">
        <button
          type="button"
          :aria-pressed="!show3d"
          @click="show3d = false"
        >{{ $t("package.card.viewList") }}</button>
        <button
          type="button"
          :aria-pressed="show3d"
          :disabled="!card.models.length"
          @click="show3d = true"
        >{{ $t("package.card.view3d") }}</button>
      </div>

      <div v-if="show3d" class="vehicle-3d">
        <FSpinner v-if="meshState === 'loading'" size="sm" :label="$t('common.loading')" />
        <p v-else-if="meshState === 'error' || !meshObjs.length" class="card-empty">
          {{ $t("package.card.meshUnavailable") }}
        </p>
        <MeshPreviewView
          v-else-if="meshObjs.length"
          :obj-base64s="meshObjs"
          class="vehicle-mesh"
        />
        <select
        v-if="card.models.length > 1"
        class="model-select"
        :value="activeModelIndex"
        @change="selectModel(Number(($event.target as HTMLSelectElement).value))"
      >
        <option v-for="(m, i) in card.models" :key="i" :value="i">
          {{ $t("package.card.modelN", { n: i + 1 }) }} · {{ keyText(m) }}
        </option>
      </select>
      </div>

      <div v-else class="card-section">
        <h4 v-if="card.name.length">{{ card.name.map(textOf).join(" ") }}</h4>
        <p v-if="card.lightCount" class="meta-line">
          {{ $t("package.card.lightCount", { n: card.lightCount }) }}
        </p>
        <ul v-if="card.lightNames.length" class="text-list">
          <li v-for="(n, i) in card.lightNames" :key="`l${i}`">{{ n }}</li>
        </ul>
        <section v-if="card.models.length" class="card-section">
          <h4>{{ $t("package.card.models") }}</h4>
          <ul class="text-list">
            <li
              v-for="(m, i) in card.models"
              :key="`m${i}`"
              :title="keyText(m)"
            >
              {{ $t("package.card.modelN", { n: i + 1 }) }} · {{ keyText(m) }}
            </li>
          </ul>
        </section>
      </div>
    </template>

    <!-- 道路卡 -->
    <template v-else-if="card.kind === 'road'">
      <p v-if="card.pathTitle.length" class="alert-text">
        {{ card.pathTitle.map(textOf).join(" ") }}
      </p>
      <div class="meta-row">
        <span v-if="card.pathWidth != null" class="duration-badge">
          {{ $t("package.card.widthMeters", { n: card.pathWidth }) }}
        </span>
        <span v-if="card.flattenTerrain != null" class="duration-badge">
          {{ $t(card.flattenTerrain ? "package.card.flattenOn" : "package.card.flattenOff") }}
        </span>
      </div>
      <section class="card-section">
        <h4>{{ $t("package.card.appearance") }}</h4>
        <ul class="text-list">
          <li v-if="card.appearance" :title="keyText(card.appearance)">
            {{ $t("package.card.appearanceNormal") }} · {{ keyText(card.appearance) }}
          </li>
          <li v-if="card.ghostAppearance" :title="keyText(card.ghostAppearance)">
            {{ $t("package.card.appearanceGhost") }} · {{ keyText(card.ghostAppearance) }}
          </li>
          <li v-if="!card.appearance && !card.ghostAppearance" class="card-empty">
            {{ $t("package.card.noText") }}
          </li>
        </ul>
      </section>
    </template>

    <!-- 菜单条目卡 -->
    <template v-else-if="card.kind === 'menu'">
      <div class="alert-row">
        <div class="alert-icon">
          <img v-if="iconUrl" :src="iconUrl" :alt="$t('package.card.icon')" />
          <FIcon v-else name="ListTree" :size="28" aria-label="" />
        </div>
        <div class="alert-body">
          <p class="alert-text">
            {{ card.title.map(textOf).join(" ") || $t("package.card.noText") }}
          </p>
          <div class="meta-row">
            <span v-if="card.order != null" class="duration-badge">
              {{ $t("package.card.menuOrder", { n: card.order }) }}
            </span>
          </div>
        </div>
      </div>
      <section v-if="card.description.length" class="card-section">
        <h4>{{ $t("package.card.menuDescription") }}</h4>
        <p class="menu-description">
          {{ card.description.map(textOf).join(" ") }}
        </p>
      </section>
    </template>

    <!-- 模拟资源定义卡 -->
    <template v-else-if="card.kind === 'resource-def'">
      <p class="alert-text">
        {{ card.resourceName.map(textOf).join(" ") || $t("package.card.noText") }}
      </p>
    </template>

    <!-- 站点装载条目卡 -->
    <template v-else-if="card.kind === 'resource-entry'">
      <section class="card-section">
        <h4>{{ $t("package.card.slots") }}</h4>
        <table v-if="card.slots.length" class="slot-table">
          <thead>
            <tr>
              <th>#</th>
              <th>{{ $t("package.card.resource") }}</th>
              <th v-for="p in 4" :key="p">{{ $t("package.card.paramN", { n: p }) }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(slot, i) in card.slots" :key="i">
              <td>{{ i + 1 }}</td>
              <td :title="keyText(slot.resource)">
                {{ slotNames.get(i) ?? keyText(slot.resource) }}
              </td>
              <td v-for="(v, j) in slot.values" :key="j">
                {{ v ?? "—" }}
              </td>
            </tr>
          </tbody>
        </table>
        <p v-else class="card-empty">{{ $t("package.card.noText") }}</p>
      </section>
      <div class="meta-row">
        <span v-if="card.enabled != null" class="duration-badge">
          {{ $t(card.enabled ? "package.card.enabled" : "package.card.disabled") }}
        </span>
        <span v-if="card.floatA != null" class="duration-badge">
          <code class="ref-code">0x0D560650</code> = {{ card.floatA }}
        </span>
        <span v-if="card.floatB != null" class="duration-badge">
          <code class="ref-code">0x0D6F3BE1</code> = {{ card.floatB }}
        </span>
      </div>
    </template>

    <!-- Parent 面包屑（共用） -->
    <footer v-if="parent" class="parent-row">
      <span class="parent-label">{{ $t("package.card.parent") }}</span>
      <code class="ref-code" :title="keyText(parent)">{{
        parentName ?? keyText(parent)
      }}</code>
    </footer>
  </div>
</template>

<style scoped>
.semantic-card {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
  padding: 4px 2px;
}
.alert-row {
  align-items: center;
  display: flex;
  gap: 14px;
  min-width: 0;
}
.alert-icon {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: flex;
  flex-shrink: 0;
  height: 56px;
  justify-content: center;
  width: 56px;
}
.alert-icon img {
  image-rendering: auto;
  max-height: 100%;
  max-width: 100%;
}
.alert-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.alert-text {
  font-size: 14px;
  line-height: 1.5;
  margin: 0;
  overflow-wrap: anywhere;
}
.meta-row {
  display: flex;
  gap: 8px;
}
.duration-badge {
  background: var(--surface-hover);
  border-radius: 4px;
  color: var(--muted-foreground);
  font-size: 11px;
  padding: 1px 6px;
  white-space: nowrap;
}
.card-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.card-section h4 {
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.02em;
  margin: 0;
  text-transform: uppercase;
}
.text-list {
  margin: 0;
  max-height: 240px;
  overflow: auto;
  padding: 0;
}
.text-list li {
  display: block;
  font-size: 13px;
  /* 固定 px 行高：避免被全局样式按比例压缩导致相邻行字形重叠 */
  line-height: 20px;
  list-style: none;
  overflow: hidden;
  padding: 2px 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.text-list.failed li {
  color: var(--muted-foreground);
}
.card-empty {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
}
.bar-colors {
  border: 1px solid var(--border);
  border-radius: 4px;
  height: 14px;
  width: 100%;
}
.parent-row {
  align-items: center;
  border-top: 1px solid var(--border);
  display: flex;
  gap: 8px;
  margin-top: 2px;
  min-width: 0;
  padding-top: 8px;
}
.parent-label {
  color: var(--muted-foreground);
  flex-shrink: 0;
  font-size: 11px;
}
.view-toggle {
  display: flex;
  gap: 4px;
}
.view-toggle button {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  font-size: 12px;
  padding: 3px 10px;
}
.view-toggle button[aria-pressed="true"] {
  color: var(--foreground);
  border-color: var(--muted-foreground);
}
.view-toggle button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.vehicle-3d {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.vehicle-mesh {
  height: 300px;
  width: 100%;
}
.model-select {
  align-self: flex-start;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font-size: 12px;
  max-width: 100%;
  padding: 3px 6px;
}
.meta-line {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
}
.slot-table {
  border-collapse: collapse;
  font-size: 12px;
  width: 100%;
}
.slot-table th,
.slot-table td {
  border: 1px solid var(--border);
  padding: 3px 8px;
  text-align: left;
}
.slot-table th {
  color: var(--muted-foreground);
  font-weight: 600;
}
.menu-description {
  font-size: 13px;
  line-height: 20px;
  margin: 0;
  overflow-wrap: anywhere;
}
.ref-code {
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
