<script setup lang="ts">
import type { EcoGamePreview } from "@/api/tauri";
import { useI18n } from "vue-i18n";
const props = defineProps<{ preview: EcoGamePreview }>();
const { t } = useI18n();
const hex = (value: number | null) => value == null ? "—" : `0x${value.toString(16).padStart(8, "0").toUpperCase()}`;
</script>
<template>
  <div class="ecogame-preview">
    <div class="meta"><span>{{ preview.data.typeName }}</span><span>{{ preview.data.totalLength.toLocaleString() }} B</span><span v-if="preview.data.decompressedLength">{{ preview.data.decompressedLength.toLocaleString() }} {{ t('package.ecogame.decompressed') }}</span></div>
    <div class="stats" v-if="preview.data.version != null">
      <div><b>{{ t('package.ecogame.version') }}</b><code>{{ preview.data.version }}</code></div><div><b>{{ t('package.ecogame.environment') }}</b><code>{{ hex(preview.data.environment) }}</code></div><div><b>{{ t('package.ecogame.maps') }}</b><code>{{ preview.data.mapCount }}</code></div><div><b>{{ t('package.ecogame.units') }}</b><code>{{ preview.data.unitCount }}</code></div><div><b>{{ t('package.ecogame.curves') }}</b><code>{{ preview.data.curveCount }}</code></div>
    </div>
    <p v-if="preview.data.error" class="notice">{{ preview.data.error }}</p>
    <section v-if="preview.data.tableWords.length"><h3>{{ t('package.ecogame.tableWords') }}</h3><pre>{{ preview.data.tableWords.map((v) => hex(v)).join(" ") }}</pre></section>
    <p v-if="!preview.data.exact" class="notice">{{ t('package.ecogame.partial') }}</p>
  </div>
</template>
<style scoped>
.ecogame-preview{display:grid;gap:12px}.meta{display:flex;gap:12px;color:var(--muted-foreground);font-size:11px}.stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:8px}.stats div{border:1px solid var(--border);border-radius:8px;padding:8px;display:grid;gap:4px}.stats b{font-size:11px}.stats code{font-size:13px}.notice{color:var(--warning);font-size:12px}.ecogame-preview pre{white-space:pre-wrap;word-break:break-all;background:var(--surface-elevated);padding:12px;border-radius:8px}
</style>
