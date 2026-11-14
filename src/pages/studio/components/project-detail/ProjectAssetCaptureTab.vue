<script setup lang="ts">
import { computed, shallowRef, useId } from "vue";
import { useI18n } from "vue-i18n";
import FSelect from "@/components/ui/FSelect.vue";
import FButton from "@/components/ui/FButton.vue";
import LogStream from "@/components/runtime-observer/LogStream.vue";
import VehicleInventory from "./VehicleInventory.vue";
import { useAssetCapture } from "./useAssetCapture";

const props = defineProps<{ project: string }>();
const { t, te } = useI18n();
const {
  processes,
  targets,
  targetKey,
  pid,
  status,
  busy,
  loading,
  error,
  epoch,
  scan,
  start,
  stop,
} = useAssetCapture(props.project);
const metric = shallowRef("");
const metricLabel = useId();
const metricOptions = computed(() =>
  ["", "vehicles", "workers", "tourists", "lighting"].map((value) => ({
    value,
    label: t(`assetCapture.filter.${value || "all"}`),
  })),
);
const buildLabel = useId();
const processLabel = useId();
const selection = computed({
  get: () => pid.value?.toString() ?? "",
  set: (value) => {
    pid.value = value ? Number(value) : undefined;
  },
});
const buildOptions = computed(() =>
  targets.value.length
    ? targets.value.map(({ key, label }) => ({ value: key, label }))
    : [{ value: "", label: t("assetCapture.noTarget") }],
);
const processOptions = computed(() =>
  processes.value.length
    ? processes.value.map(({ name, pid }) => ({
        value: String(pid),
        label: `${name} · ${pid}`,
      }))
    : [{ value: "", label: t("assetCapture.noProcess") }],
);
const running = computed(() => Boolean(status.value?.running));
const failure = computed(() => {
  const code = error.value || status.value?.status?.error;
  if (!code) return "";
  const key = `assetCapture.errors.${code}`;
  return te(key) ? t(key) : t("assetCapture.errors.failed", { detail: code });
});
const summary = computed(() => {
  const sample = status.value?.status;
  if (loading.value) return t("assetCapture.loading");
  if (!targets.value.length) return t("assetCapture.noBuild");
  if (!processes.value.length) return t("assetCapture.noProcess");
  if (!status.value) return t("assetCapture.ready");
  if (sample?.stopReason === "asset_record_limit")
    return t("assetCapture.recordLimit");
  if (!sample?.ready)
    return running.value
      ? t("assetCapture.connecting")
      : t("assetCapture.finished");
  return t(running.value ? "assetCapture.live" : "assetCapture.complete", {
    matched: sample.matched,
    tracked: sample.tracked,
    skipped: sample.skippedSamples,
  });
});
</script>

<template>
  <section class="capture-panel" :aria-label="t('assetCapture.title')">
    <div class="capture-controls">
      <div class="capture-field">
        <label :for="buildLabel">{{ t("assetCapture.build") }}</label>
        <FSelect
          :id="buildLabel"
          v-model="targetKey"
          :options="buildOptions"
          :disabled="busy || running || loading || !targets.length"
        />
      </div>
      <div class="capture-field">
        <label :for="processLabel">{{ t("assetCapture.process") }}</label>
        <FSelect
          :id="processLabel"
          v-model="selection"
          :options="processOptions"
          :disabled="busy || running || loading || !processes.length"
        />
      </div>
      <FButton
        variant="secondary"
        :disabled="busy || running || loading"
        @click="scan"
        >{{ t("assetCapture.refresh") }}</FButton
      >
      <FButton
        v-if="running"
        variant="secondary"
        :disabled="busy"
        @click="stop"
        >{{ t("runtimeObserver.stop") }}</FButton
      >
      <FButton
        v-else
        :disabled="busy || loading || !pid || !targetKey"
        @click="start"
        >{{ t("runtimeObserver.start") }}</FButton
      >
    </div>
    <p class="capture-status" role="status">{{ summary }}</p>
    <p v-if="failure" class="capture-error" role="alert">{{ failure }}</p>
    <VehicleInventory :sample="status?.status" :running="running" />
    <div class="capture-field">
      <label :for="metricLabel">{{ t("assetCapture.metrics") }}</label>
      <FSelect :id="metricLabel" v-model="metric" :options="metricOptions" />
    </div>
    <p v-if="metric === 'lighting'" class="capture-status">
      {{ t("assetCapture.lightingHint") }}
      <span v-if="status?.status?.ready">
        {{
          t("assetCapture.controllerCount", {
            count: status.status.controllers,
          })
        }}
      </span>
    </p>
    <p
      v-if="
        metric &&
        status?.status?.ready &&
        Number(status.status.matched) > 0 &&
        (status.status.metricCounts as Record<string, number> | undefined)?.[
          metric
        ] === 0
      "
      class="capture-status"
    >
      {{ t("assetCapture.metricUnavailable") }}
    </p>
    <p v-if="(status?.records ?? 0) > 1000" class="capture-status">
      {{ t("assetCapture.displayLimit", { records: status?.records }) }}
    </p>
    <LogStream :key="epoch" :rows="status?.recent ?? []" :metric="metric" />
  </section>
</template>

<style scoped>
.capture-panel {
  display: grid;
  gap: 12px;
  padding-block: 16px;
  min-inline-size: 0;
}
.capture-controls {
  display: flex;
  flex-wrap: wrap;
  align-items: end;
  gap: 8px;
}
.capture-field {
  display: grid;
  gap: 4px;
  inline-size: min(280px, 100%);
  font-size: 12px;
}
.capture-status,
.capture-error {
  margin: 0;
  font-size: 12px;
}
.capture-status {
  color: var(--muted-foreground);
}
.capture-error {
  color: var(--destructive);
}
:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
</style>
