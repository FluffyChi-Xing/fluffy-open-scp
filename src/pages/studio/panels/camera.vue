<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { RouterLink } from "vue-router";
import { useI18n } from "vue-i18n";
import { open, save } from "@tauri-apps/plugin-dialog";
import { command } from "@/api/tauri";
import { tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import FButton from "@/components/ui/FButton.vue";
import FIcon from "@/components/extensions/FIcon.vue";
import FAlert from "@/components/ui/FAlert.vue";
import PropertyEditor from "@/pages/packages/components/property-editor/PropertyEditor.vue";
import { useGamePackagesStore } from "@/stores/gamePackages";

const { t } = useI18n();
const toast = useToast();
const packages = useGamePackagesStore();
const tab = ref("camera");
const nativeCamera = { fov: 37.5, near: 0.028, far: 2632, x: 60, y: 800, z: 120, pitch: -45, yaw: 149, roll: 0 };
const camera = reactive<Record<string, number>>({ ...nativeCamera });
const locked = ref(true);
const motion = reactive({ duration: 4, orbit: 0, dolly: 0, easing: 0.5 });
const focus = reactive({ distance: 42, aperture: 2.8, softness: 0.45 });
const cameraFields = computed(() => [
  ["fov", t("studio.camera.fields.fov"), 20, 110, 1],
  ["near", t("studio.camera.fields.near"), 0.001, 20, 0.001],
  ["far", t("studio.camera.fields.far"), 100, 3000, 10],
  ["x", t("studio.camera.fields.x"), -120, 120, 1],
  ["y", t("studio.camera.fields.y"), 1, 2000, 1],
  ["z", t("studio.camera.fields.z"), -120, 120, 1],
  ["pitch", t("studio.camera.fields.pitch"), -89, 89, 1],
  ["yaw", t("studio.camera.fields.yaw"), -360, 360, 1],
  ["roll", t("studio.camera.fields.roll"), -180, 180, 1],
] as const);
const openedPackagePaths = computed(() => packages.opened.map((item) => item.package.path.toLowerCase()));
const isNamedPackage = (path: string, name: string) => path.split(/[\\/]/).at(-1)?.replace(/[^a-z]/g, "")?.includes(name) ?? false;
const hasGamePackage = computed(() => openedPackagePaths.value.some((path) => isNamedPackage(path, "simcitygame")));
const hasGraphicsPackage = computed(() => openedPackagePaths.value.some((path) => isNamedPackage(path, "simcitygraphics")));
const packagesReady = computed(() => hasGamePackage.value && hasGraphicsPackage.value);
const gamePackage = computed(() => packages.opened.find((item) => isNamedPackage(item.package.path.toLowerCase(), "simcitygame")));
const graphicsPackage = computed(() => packages.opened.find((item) => isNamedPackage(item.package.path.toLowerCase(), "simcitygraphics")));
function resetNative() { Object.assign(camera, nativeCamera); }
function payload() { return { schema: "openscp.camera-preset.v1", camera: { ...camera }, motion: { ...motion }, focus: { ...focus } }; }
function base64(text: string) { const bytes = new TextEncoder().encode(text); let binary = ""; for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000)); return btoa(binary); }
async function exportPreset() { try { const path = await save({ defaultPath: "openscp-camera-preset.json", filters: [{ name: "JSON", extensions: ["json"] }] }); if (!path) return; await command("write_export_file", { request: { path, dataBase64: base64(JSON.stringify(payload(), null, 2)) } }); toast.success(t("studio.camera.exportSuccess")); } catch (error) { toast.error(error instanceof Error ? error.message : t("studio.camera.exportFailed")); } }
async function deployPreset() { try { const source = await open({ multiple: false, title: t("studio.camera.pickSource"), filters: [{ name: "Package", extensions: ["package"] }] }); if (typeof source !== "string") return; const root = await tauriApi.workspace.pickDirectory(t("studio.camera.pickFolder")); if (!root) return; const path = `${root.replace(/[\\/]$/, "")}/OpenSCP-Isometric-Camera.package`; await command("build_camera_package", { request: { sourcePath: source, outputPath: path, maxZoom: camera.far, maxPitch: camera.pitch, dofStrength: focus.softness * 3 } }); toast.success(t("studio.camera.deploySuccess")); } catch (error) { toast.error(error instanceof Error ? error.message : t("studio.camera.exportFailed")); } }
watch(locked, (value) => { if (value) resetNative(); });
</script>
<template>
  <section class="camera-page">
    <header class="page-heading"><div><p class="eyebrow">{{ t("studio.camera.eyebrow") }}</p><h1>{{ t("studio.camera.title") }}</h1><p>{{ t("studio.camera.description") }}</p></div><div class="actions"><button class="lock-toggle" type="button" :aria-pressed="!locked" @click="locked = !locked"><FIcon :name="locked ? 'Lock' : 'LockOpen'" :size="14" aria-label="" /><span>{{ locked ? t("studio.camera.locked") : t("studio.camera.unlocked") }}</span></button><FButton variant="secondary" @click="exportPreset"><FIcon name="Download" :size="14" aria-label="" />{{ t("studio.camera.export") }}</FButton><FButton @click="deployPreset"><FIcon name="FolderOpen" :size="14" aria-label="" />{{ t("studio.camera.deploy") }}</FButton></div></header>
    <FAlert v-if="!packagesReady" type="warning"><span><strong>{{ t("studio.camera.packagesRequired") }}</strong> · {{ t("studio.camera.packagesHint", { game: hasGamePackage ? "✓" : "×", graphics: hasGraphicsPackage ? "✓" : "×" }) }}</span><template #action><RouterLink to="/packages">{{ t("studio.camera.openPackages") }}</RouterLink></template></FAlert>
    <div class="camera-layout"><section class="preview-card"><div class="preview-toolbar"><span>{{ t("studio.camera.preview") }}</span><span class="preview-badge">{{ Math.round(camera.fov) }}° FOV</span></div><div v-if="packagesReady" class="property-stage"><PropertyEditor :package-id="gamePackage!.package.packageId" :tgi="{ typeId: 0x00b1b104, group: 0x40e1c000, instance: 0x7a7a2427 }" mode="viewer" embedded :external-sidebar="true" :external-toolbar="true" initial-render-mode="refined" /></div><div v-else class="preview-stage empty-preview"><span>{{ t("studio.camera.previewUnavailable") }}</span></div><p class="preview-hint">{{ t("studio.camera.previewHint") }}</p></section>
      <section class="inspector"><nav class="tabs" role="tablist"><button v-for="item in [{ id: 'camera', label: t('studio.camera.tabs.camera') }, { id: 'motion', label: t('studio.camera.tabs.motion') }, { id: 'focus', label: t('studio.camera.tabs.focus') }]" :key="item.id" type="button" role="tab" :aria-selected="tab === item.id" :class="{ active: tab === item.id }" @click="tab = item.id">{{ item.label }}</button></nav>
        <div v-if="tab === 'camera'" class="fields"><label v-for="field in cameraFields" :key="field[0]"><span>{{ field[1] }} <output>{{ camera[field[0]] }}</output></span><input v-model.number="camera[field[0]]" :disabled="locked" type="range" :min="field[2]" :max="field[3]" :step="field[4]" /></label></div>
        <div v-else-if="tab === 'motion'" class="fields"><label><span>{{ t("studio.camera.duration") }} <output>{{ motion.duration }}s</output></span><input v-model.number="motion.duration" :disabled="locked" type="range" min="0.2" max="20" step="0.1" /></label><label><span>{{ t("studio.camera.orbit") }} <output>{{ motion.orbit }}°</output></span><input v-model.number="motion.orbit" :disabled="locked" type="range" min="-360" max="360" step="1" /></label><label><span>{{ t("studio.camera.dolly") }} <output>{{ motion.dolly }}m</output></span><input v-model.number="motion.dolly" :disabled="locked" type="range" min="-100" max="100" step="0.5" /></label></div>
        <div v-else class="fields"><label><span>{{ t("studio.camera.focusDistance") }} <output>{{ focus.distance }}m</output></span><input v-model.number="focus.distance" :disabled="locked" type="range" min="1" max="300" step="1" /></label><label><span>{{ t("studio.camera.aperture") }} <output>f/{{ focus.aperture }}</output></span><input v-model.number="focus.aperture" :disabled="locked" type="range" min="0.7" max="16" step="0.1" /></label><label><span>{{ t("studio.camera.softness") }} <output>{{ Math.round(focus.softness * 100) }}%</output></span><input v-model.number="focus.softness" :disabled="locked" type="range" min="0" max="1" step="0.01" /></label></div>
        <div class="inspector-note"><strong>{{ t("studio.camera.dataTitle") }}</strong><p>{{ t("studio.camera.dataHint") }}</p><code>openscp.camera-preset.v1</code></div>
      </section>
    </div>
  </section>
</template>
<style scoped>
.camera-page { display: grid; gap: 18px; padding-bottom: 40px; }.page-heading { display:flex; justify-content:space-between; align-items:flex-end; gap:20px; }.eyebrow { color:var(--brand); font-size:11px; font-weight:700; letter-spacing:.08em; text-transform:uppercase; }.page-heading h1 { margin:4px 0 6px; font-size:26px; color:var(--foreground); }.page-heading p { margin:0; color:var(--muted-foreground); max-width:680px; }.actions { display:flex; gap:8px; flex:none; }.lock-toggle { align-items:center; background:var(--surface-elevated); border:1px solid var(--border); border-radius:var(--radius-sm); color:var(--foreground); cursor:pointer; display:inline-flex; font:inherit; font-size:12px; gap:7px; min-height:32px; padding:0 10px; transition:background-color 120ms ease, border-color 120ms ease, color 120ms ease; }.lock-toggle:hover { background:var(--surface-hover); border-color:var(--brand); }.lock-toggle:focus-visible { outline:2px solid var(--brand); outline-offset:2px; }.camera-layout { display:grid; grid-template-columns:minmax(0,1.45fr) minmax(300px,.75fr); gap:14px; min-height:620px; }.preview-card,.inspector { border:1px solid var(--border); border-radius:var(--radius-md); background:var(--surface); overflow:hidden; }.preview-card { display:flex; flex-direction:column; }.preview-toolbar { display:flex; justify-content:space-between; align-items:center; padding:12px 14px; border-bottom:1px solid var(--border); font-size:12px; font-weight:650; color:var(--foreground); }.preview-badge { color:var(--brand); font:11px var(--font-mono,monospace); }.property-stage,.preview-stage { flex:1; min-height:480px; background:var(--background); }.property-stage :deep(.embedded-root),.property-stage :deep(.editor-root),.property-stage :deep(.editor-body) { height:100%; min-height:0; }.property-stage :deep(.editor-body) { overflow:hidden; }.preview-stage { display:grid; place-items:center; color:var(--muted-foreground); font-size:12px; padding:24px; text-align:center; }.preview-hint { padding:10px 14px; color:var(--muted-foreground); font:11px/1.5 var(--font-sans); }.inspector { padding:0 16px 16px; }.tabs { display:flex; gap:4px; border-bottom:1px solid var(--border); margin:0 -16px 18px; padding:0 12px; }.tabs button { border:0; border-bottom:2px solid transparent; background:transparent; color:var(--muted-foreground); padding:14px 10px 11px; cursor:pointer; font:12px var(--font-sans); transition:color 120ms ease, border-color 120ms ease, background-color 120ms ease; }.tabs button:hover { color:var(--foreground); background:color-mix(in srgb,var(--brand) 8%,transparent); }.tabs button.active { color:var(--brand); border-color:var(--brand); font-weight:650; }.fields { display:grid; gap:18px; }.fields label { display:grid; gap:9px; font:12px var(--font-sans); color:var(--foreground); }.fields label span { display:flex; justify-content:space-between; }.fields output { color:var(--foreground); font:12px var(--font-mono,monospace); }.fields input { accent-color:var(--brand); width:100%; }.fields input:disabled { cursor:not-allowed; opacity:.55; }.inspector-note { margin-top:28px; padding:12px; border:1px solid var(--border); border-radius:var(--radius-sm); background:var(--background); }.inspector-note strong { font-size:12px; color:var(--foreground); }.inspector-note p { margin:7px 0; color:var(--muted-foreground); font:11px/1.5 var(--font-sans); }.inspector-note code { color:var(--brand); font:11px var(--font-mono,monospace); }@media(max-width:900px){.camera-layout{grid-template-columns:1fr}.property-stage,.preview-stage{min-height:360px}.page-heading{align-items:flex-start;flex-direction:column}.actions{width:100%;flex-wrap:wrap}}
</style>
