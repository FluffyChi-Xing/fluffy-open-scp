<script setup lang="ts">
import { onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { save } from "@tauri-apps/plugin-dialog";
import { command } from "@/api/tauri";
import { tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import FButton from "@/components/ui/FButton.vue";
import FIcon from "@/components/extensions/FIcon.vue";

const { t } = useI18n();
const toast = useToast();
const canvas = ref<HTMLCanvasElement>();
const tab = ref("camera");
const camera = reactive<Record<string, number>>({ fov: 55, near: 0.1, far: 1600, x: 0, y: 24, z: 42, pitch: -18, yaw: 180, roll: 0 });
const motion = reactive({ duration: 4, orbit: 0, dolly: 0, easing: 0.5 });
const focus = reactive({ distance: 42, aperture: 2.8, softness: 0.45 });
let renderer: any;
let scene: any;
let previewCamera: any;
let raf = 0;
let target: any;

async function setupPreview() {
  if (!canvas.value) return;
  const THREE = await import("three");
  renderer = new THREE.WebGLRenderer({ canvas: canvas.value, antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  scene = new THREE.Scene();
  scene.background = new THREE.Color("#0b1324");
  previewCamera = new THREE.PerspectiveCamera(camera.fov, 1.6, camera.near, camera.far);
  const grid = new THREE.GridHelper(120, 24, "#29405d", "#172840");
  scene.add(grid);
  target = new THREE.Mesh(new THREE.BoxGeometry(8, 5, 5), new THREE.MeshStandardMaterial({ color: "#4b7db8", roughness: 0.72 }));
  target.position.set(0, 2.5, 0);
  scene.add(target);
  scene.add(new THREE.HemisphereLight("#d6e7ff", "#172034", 2));
  const key = new THREE.DirectionalLight("#fff2d2", 2); key.position.set(10, 20, 12); scene.add(key);
  const tick = () => { raf = requestAnimationFrame(tick); updatePreview(THREE); renderer.render(scene, previewCamera); };
  tick();
  resizePreview();
}
function updatePreview(THREE: any) {
  if (!previewCamera) return;
  previewCamera.fov = camera.fov; previewCamera.near = camera.near; previewCamera.far = camera.far;
  previewCamera.position.set(camera.x, camera.y, camera.z);
  previewCamera.rotation.set(THREE.MathUtils.degToRad(camera.pitch), THREE.MathUtils.degToRad(camera.yaw), THREE.MathUtils.degToRad(camera.roll));
  previewCamera.lookAt(target?.position ?? { x: 0, y: 2, z: 0 });
  if (target) target.rotation.y = THREE.MathUtils.degToRad(motion.orbit);
}
function resizePreview() { if (!canvas.value || !renderer) return; const box = canvas.value.parentElement?.getBoundingClientRect(); if (!box) return; renderer.setSize(box.width, box.height, false); if (previewCamera) { previewCamera.aspect = box.width / box.height; previewCamera.updateProjectionMatrix(); } }
function payload() { return { schema: "openscp.camera-preset.v1", camera: { ...camera }, motion: { ...motion }, focus: { ...focus } }; }
function base64(text: string) { const bytes = new TextEncoder().encode(text); let binary = ""; for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000)); return btoa(binary); }
async function exportPreset() { try { const path = await save({ defaultPath: "openscp-camera-preset.json", filters: [{ name: "JSON", extensions: ["json"] }] }); if (!path) return; await command("write_export_file", { request: { path, dataBase64: base64(JSON.stringify(payload(), null, 2)) } }); toast.success(t("studio.camera.exportSuccess")); } catch (error) { toast.error(error instanceof Error ? error.message : t("studio.camera.exportFailed")); } }
async function deployPreset() { try { const root = await tauriApi.workspace.pickDirectory(t("studio.camera.pickFolder")); if (!root) return; const path = `${root.replace(/[\\/]$/, "")}/OpenSCP-CameraPreset.json`; await command("write_export_file", { request: { path, dataBase64: base64(JSON.stringify(payload(), null, 2)) } }); toast.success(t("studio.camera.deploySuccess")); } catch (error) { toast.error(error instanceof Error ? error.message : t("studio.camera.exportFailed")); } }
watch(() => [camera.fov, camera.near, camera.far, camera.x, camera.y, camera.z, camera.pitch, camera.yaw, camera.roll, motion.orbit], () => resizePreview());
onMounted(() => { void setupPreview(); window.addEventListener("resize", resizePreview); });
onBeforeUnmount(() => { cancelAnimationFrame(raf); window.removeEventListener("resize", resizePreview); renderer?.dispose(); });
</script>
<template>
  <section class="camera-page">
    <header class="page-heading"><div><p class="eyebrow">{{ t("studio.camera.eyebrow") }}</p><h1>{{ t("studio.camera.title") }}</h1><p>{{ t("studio.camera.description") }}</p></div><div class="actions"><FButton variant="secondary" @click="exportPreset"><FIcon name="Download" :size="14" aria-label="" />{{ t("studio.camera.export") }}</FButton><FButton @click="deployPreset"><FIcon name="FolderOutput" :size="14" aria-label="" />{{ t("studio.camera.deploy") }}</FButton></div></header>
    <div class="camera-layout"><section class="preview-card"><div class="preview-toolbar"><span>{{ t("studio.camera.preview") }}</span><span class="preview-badge">{{ Math.round(camera.fov) }}° FOV</span></div><div class="preview-stage"><canvas ref="canvas" /></div><p class="preview-hint">{{ t("studio.camera.previewHint") }}</p></section>
      <section class="inspector"><nav class="tabs" role="tablist"><button v-for="item in [{ id: 'camera', label: t('studio.camera.tabs.camera') }, { id: 'motion', label: t('studio.camera.tabs.motion') }, { id: 'focus', label: t('studio.camera.tabs.focus') }]" :key="item.id" type="button" role="tab" :aria-selected="tab === item.id" :class="{ active: tab === item.id }" @click="tab = item.id">{{ item.label }}</button></nav>
        <div v-if="tab === 'camera'" class="fields"><label v-for="field in [['fov','FOV',20,110,1],['near','Near clip',0.1,20,0.1],['far','Far clip',100,3000,10],['pitch','Pitch',-89,89,1],['yaw','Yaw',-360,360,1],['roll','Roll',-180,180,1]]" :key="field[0]"><span>{{ field[1] }} <output>{{ camera[field[0]] }}</output></span><input v-model.number="camera[field[0]]" type="range" :min="field[2]" :max="field[3]" :step="field[4]" /></label></div>
        <div v-else-if="tab === 'motion'" class="fields"><label><span>{{ t("studio.camera.duration") }} <output>{{ motion.duration }}s</output></span><input v-model.number="motion.duration" type="range" min="0.2" max="20" step="0.1" /></label><label><span>{{ t("studio.camera.orbit") }} <output>{{ motion.orbit }}°</output></span><input v-model.number="motion.orbit" type="range" min="-360" max="360" step="1" /></label><label><span>{{ t("studio.camera.dolly") }} <output>{{ motion.dolly }}m</output></span><input v-model.number="motion.dolly" type="range" min="-100" max="100" step="0.5" /></label></div>
        <div v-else class="fields"><label><span>{{ t("studio.camera.focusDistance") }} <output>{{ focus.distance }}m</output></span><input v-model.number="focus.distance" type="range" min="1" max="300" step="1" /></label><label><span>{{ t("studio.camera.aperture") }} <output>f/{{ focus.aperture }}</output></span><input v-model.number="focus.aperture" type="range" min="0.7" max="16" step="0.1" /></label><label><span>{{ t("studio.camera.softness") }} <output>{{ Math.round(focus.softness * 100) }}%</output></span><input v-model.number="focus.softness" type="range" min="0" max="1" step="0.01" /></label></div>
        <div class="inspector-note"><strong>{{ t("studio.camera.dataTitle") }}</strong><p>{{ t("studio.camera.dataHint") }}</p><code>openscp.camera-preset.v1</code></div>
      </section>
    </div>
  </section>
</template>
<style scoped>
.camera-page { display: grid; gap: 18px; padding-bottom: 40px; }.page-heading { display:flex; justify-content:space-between; align-items:flex-end; gap:20px; }.eyebrow { color:var(--accent); font-size:11px; font-weight:700; letter-spacing:.08em; text-transform:uppercase; }.page-heading h1 { margin:4px 0 6px; font-size:26px; }.page-heading p { margin:0; color:var(--muted-foreground); max-width:680px; }.actions { display:flex; gap:8px; flex:none; }.camera-layout { display:grid; grid-template-columns:minmax(0,1.45fr) minmax(300px,.75fr); gap:14px; min-height:620px; }.preview-card,.inspector { border:1px solid var(--border); border-radius:var(--radius-md); background:var(--surface); overflow:hidden; }.preview-card { display:flex; flex-direction:column; }.preview-toolbar { display:flex; justify-content:space-between; align-items:center; padding:12px 14px; border-bottom:1px solid var(--border); font-size:12px; font-weight:650; }.preview-badge { color:var(--accent); font:11px var(--font-mono,monospace); }.preview-stage { flex:1; min-height:480px; background:#0b1324; }.preview-stage canvas { width:100%; height:100%; display:block; }.preview-hint { padding:10px 14px; font-size:11px; color:var(--muted-foreground); }.inspector { padding:0 16px 16px; }.tabs { display:flex; gap:4px; border-bottom:1px solid var(--border); margin:0 -16px 18px; padding:0 12px; }.tabs button { border:0; border-bottom:2px solid transparent; background:transparent; color:var(--muted-foreground); padding:14px 10px 11px; cursor:pointer; font:12px var(--font-sans); }.tabs button.active { color:var(--foreground); border-color:var(--accent); }.fields { display:grid; gap:18px; }.fields label { display:grid; gap:9px; font-size:12px; color:var(--muted-foreground); }.fields label span { display:flex; justify-content:space-between; }.fields output { color:var(--foreground); font:12px var(--font-mono,monospace); }.fields input { accent-color:var(--accent); width:100%; }.inspector-note { margin-top:28px; padding:12px; border:1px solid var(--border); border-radius:var(--radius-sm); background:var(--background); }.inspector-note strong { font-size:12px; }.inspector-note p { margin:7px 0; color:var(--muted-foreground); font-size:11px; line-height:1.5; }.inspector-note code { color:var(--accent); font-size:10px; }@media(max-width:900px){.camera-layout{grid-template-columns:1fr}.preview-stage{min-height:360px}.page-heading{align-items:flex-start;flex-direction:column}.actions{width:100%}}
</style>
