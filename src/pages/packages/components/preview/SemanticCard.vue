<script setup lang="ts">
import { computed, ref, watch } from "vue";
import FIcon from "@/components/extensions/FIcon.vue";
import { isTauri, tauriApi } from "@/api";
import type { SemanticCard } from "@/api/tauri";
import {
  barGradientCss,
  cardParent,
  keyText,
  textOf,
} from "./semanticCard";

/**
 * 语义预览卡（Alert / SimAction / MapLayer 第一档）：
 * 后端 extract_semantic_card 已按 semantic tag 抽好结构化载荷并解析了
 * text 文案（游戏 Locale 包），本组件只做渲染与图标/Parent 的按需加载。
 */
const props = defineProps<{ card: SemanticCard; packageId: number }>();

/** 三卡共有的 Parent 引用。 */
const parent = computed(() => cardParent(props.card));
/** alert / map-layer 的图标引用。 */
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

    <!-- Parent 面包屑（三卡共用） -->
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
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  max-height: 220px;
  overflow: auto;
  padding: 0;
}
.text-list li {
  font-size: 13px;
  line-height: 1.5;
  list-style: none;
  overflow: hidden;
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
.ref-code {
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
