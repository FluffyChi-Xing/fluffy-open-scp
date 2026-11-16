<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { tauriApi } from "@/api";
import { command } from "@/api/tauri";
import { useGamePackagesStore } from "@/stores/gamePackages";
import FButton from "@/components/ui/FButton.vue";
import FSelect from "@/components/ui/FSelect.vue";
import CameraCurveEditor from "../components/camera/CameraCurveEditor.vue";
import {
  curveSampleLimits,
  sampleToZoom,
  zoomToSample,
} from "../components/camera/cameraCurveEditing";
import CityCameraFocusControls from "../components/camera/CityCameraFocusControls.vue";
import CityCameraPreview from "../components/camera/CityCameraPreview.vue";
import { focusFields } from "../components/camera/cityCameraFocus";
import {
  cameraFields,
  cameraFrame,
  curveKey,
  hex,
  parseSamples,
  type CameraCurve,
  type CameraFrame,
  type CameraSource,
} from "../components/camera/cityCamera";

const { t } = useI18n();
const packages = useGamePackagesStore();
function openedGamePackage(name: string) {
  return packages.opened.find(
    ({ package: item }) =>
      item.path.split(/[\\/]/).at(-1)?.toLowerCase() === name,
  )?.package;
}
const cachedApp = computed(() => openedGamePackage("simcity_app.package"));
const previewPackageId = computed(
  () =>
    cachedApp.value?.packageId ??
    openedGamePackage("simcity_game.package")?.packageId,
);
const source = shallowRef<CameraSource>();
const path = shallowRef("");
const busy = shallowRef<"open" | "export" | null>(null);
const status = shallowRef("");
const error = shallowRef("");
const instance = shallowRef(0);
const field = shallowRef<number>(cameraFields[0].hash);
const zoom = shallowRef(0);
const heading = shallowRef(0);
const tilt = shallowRef(0.75);
const showSource = shallowRef(false);
const playing = shallowRef(false);
const motionDraft = shallowRef<number>();
const sourceMotion = computed(() => source.value?.motionStrength ?? 1);
const motionStrength = computed({
  get: () => motionDraft.value ?? sourceMotion.value,
  set: (value: number) => {
    motionDraft.value = Math.fround(value);
    showSource.value = false;
  },
});
const motionEdit = computed(() =>
  motionStrength.value === sourceMotion.value
    ? undefined
    : motionStrength.value,
);
const drafts = shallowRef<Record<string, string>>({});

const profiles = computed(() => source.value?.profiles ?? []);
const profile = computed(() =>
  profiles.value.find((item) => item.instance === instance.value),
);
const curves = computed(
  () =>
    profile.value?.curves.filter((entry) =>
      cameraFields.some((item) => item.hash === entry.hash),
    ) ?? [],
);
const focusCurves = computed(
  () =>
    profile.value?.curves.filter((entry) =>
      focusFields.some((item) => item.hash === entry.hash),
    ) ?? [],
);
const focusValues = computed(() =>
  Object.fromEntries(
    focusCurves.value.map((entry) => [
      entry.hash,
      drafts.value[curveKey(instance.value, entry.hash)] ??
        effectiveSamples(instance.value, entry)?.join(" ") ??
        "",
    ]),
  ),
);
const invalidFocus = computed(() =>
  focusCurves.value
    .filter((entry) => !effectiveSamples(instance.value, entry))
    .map((entry) => entry.hash),
);
const modifiedFocus = computed(() =>
  focusCurves.value.some(
    (entry) => drafts.value[curveKey(instance.value, entry.hash)] !== undefined,
  ),
);
const curve = computed(
  () =>
    curves.value.find((item) => item.hash === field.value) ?? curves.value[0],
);
const metadata = computed(() =>
  cameraFields.find((item) => item.hash === curve.value?.hash),
);
const samplesFor = (id: number, hash: number, original: number[]) => {
  const input = drafts.value[curveKey(id, hash)];
  return input === undefined ? original : parseSamples(hash, input);
};
function effectiveSamples(id: number, entry: CameraCurve) {
  const key = curveKey(id, entry.hash);
  const inherited = entry.inherited
    ? profiles.value
        .find((item) => item.instance === 0x9e5a6813)
        ?.curves.find((item) => item.hash === entry.hash)
    : undefined;
  const original = inherited
    ? (samplesFor(0x9e5a6813, inherited.hash, inherited.samples) ??
      inherited.samples)
    : entry.samples;
  return drafts.value[key] === undefined
    ? original
    : parseSamples(entry.hash, drafts.value[key]);
}
const draft = computed({
  get: () =>
    curve.value
      ? (drafts.value[curveKey(instance.value, curve.value.hash)] ??
        effectiveSamples(instance.value, curve.value)?.join(" ") ??
        "")
      : "",
  set: (text) => {
    if (curve.value) editField(curve.value.hash, text);
  },
});
const samples = computed(() =>
  curve.value ? effectiveSamples(instance.value, curve.value) : null,
);
const editedProfiles = computed(() =>
  profiles.value.map((item) => ({
    ...item,
    curves: item.curves.map((entry) => ({
      ...entry,
      samples: effectiveSamples(item.instance, entry) ?? entry.samples,
    })),
  })),
);
const editedProfile = computed(() =>
  editedProfiles.value.find((item) => item.instance === instance.value),
);
const shownProfile = computed(() =>
  showSource.value ? profile.value : editedProfile.value,
);
const zooms = computed(
  () =>
    shownProfile.value?.curves.find(
      (entry) => entry.hash === cameraFields[0].hash,
    )?.samples ?? [],
);
const headings = computed(
  () =>
    shownProfile.value?.curves.find(
      (entry) => entry.hash === cameraFields[6].hash,
    )?.samples ?? [],
);
const zoomIndex = computed(() =>
  Math.max(0, Math.min(zoom.value, zooms.value.length - 1)),
);
const headingIndex = computed(() =>
  Math.max(0, Math.min(heading.value, headings.value.length - 1)),
);
const initialCurveIndex = computed(() => {
  const field = metadata.value?.id;
  const count = (samples.value ?? curve.value?.samples)?.length ?? 1;
  if (!field) return 0;
  if (field === "heading") return Math.min(headingIndex.value, count - 1);
  return zoomToSample(field, zoomIndex.value, zooms.value.length, count);
});
function selectedCurveLimits(index: number) {
  return editedProfile.value && metadata.value
    ? curveSampleLimits(editedProfile.value, metadata.value.id, index)
    : ([-Infinity, Infinity] as const);
}
function selectCurveSample(index: number) {
  const field = metadata.value?.id;
  if (!field) return;
  showSource.value = false;
  if (field === "heading") {
    heading.value = index;
    return;
  }
  const count = (samples.value ?? curve.value?.samples)?.length ?? 1;
  const zoomCount =
    editedProfile.value?.curves.find(
      (entry) => entry.hash === cameraFields[0].hash,
    )?.samples.length ?? 1;
  if (field === "distance" || count > 1)
    zoom.value = sampleToZoom(field, index, count, zoomCount);
  if (field === "minPitch") tilt.value = 0;
  if (field === "maxPitch") tilt.value = 1;
}
const frame = computed(
  () =>
    shownProfile.value &&
    cameraFrame(
      shownProfile.value,
      zoomIndex.value,
      headingIndex.value,
      tilt.value,
    ),
);
const previewFrame = computed<CameraFrame | null>(
  (previous) => frame.value ?? previous ?? null,
);
const invalidDraft = computed(() =>
  profiles.value.some((item) =>
    item.curves.some(
      (entry) => !samplesFor(item.instance, entry.hash, entry.samples),
    ),
  ),
);
const edits = computed(() =>
  profiles.value.flatMap((item) =>
    item.curves.flatMap((entry) => {
      const values = samplesFor(item.instance, entry.hash, entry.samples);
      if (
        !values ||
        (!entry.inherited &&
          values.length === entry.samples.length &&
          values.every((value, i) => value === entry.samples[i]))
      )
        return [];
      if (drafts.value[curveKey(item.instance, entry.hash)] === undefined)
        return [];
      return [{ instance: item.instance, hash: entry.hash, samples: values }];
    }),
  ),
);
const profileOptions = computed(() =>
  profiles.value.map((item) => ({
    value: String(item.instance),
    label: `${t(item.instance === 0x9e5a6813 ? "studio.camera.primary" : "studio.camera.alternate")} · ${hex(item.instance)}`,
  })),
);
const fieldOptions = computed(() =>
  cameraFields
    .filter((item) => curves.value.some((entry) => entry.hash === item.hash))
    .map((item) => ({
      value: String(item.hash),
      label: `${t(`studio.camera.fields.${item.id}`)} · ${hex(item.hash)}`,
    })),
);
const headingOptions = computed(() =>
  headings.value.map((value, index) => ({
    value: String(index),
    label: `${number(value)}°`,
  })),
);
const sourceName = computed(() => path.value.split(/[\\/]/).at(-1));

function changeProfile(id: number) {
  instance.value = id;
  resetPose();
  showSource.value = false;
}
function resetPose() {
  zoom.value = 0;
  heading.value = 0;
  tilt.value = 0.75;
  playing.value = false;
}
function nudgeTilt(delta: number) {
  if (!frame.value?.pitchLocked)
    tilt.value = Math.max(0, Math.min(1, tilt.value + delta));
}
function nudgeZoom(step: number) {
  zoom.value = Math.max(
    0,
    Math.min(zooms.value.length - 1, zoomIndex.value + step),
  );
}
function editField(hash: number, value: string) {
  drafts.value = { ...drafts.value, [curveKey(instance.value, hash)]: value };
  showSource.value = false;
}
function resetFields(hashes: number[]) {
  const next = { ...drafts.value };
  for (const hash of hashes) delete next[curveKey(instance.value, hash)];
  drafts.value = next;
}
function errorCode(cause: unknown) {
  return cause && typeof cause === "object" && "code" in cause
    ? cause.code
    : undefined;
}

async function loadSource(selected: string, packageId?: number) {
  const result = await command<CameraSource>(
    "read_city_camera",
    packageId === undefined ? { sourcePath: selected } : { packageId },
  );
  source.value = result;
  path.value = selected;
  drafts.value = {};
  motionDraft.value = undefined;
  field.value = cameraFields[0].hash;
  changeProfile(result.profiles[0]!.instance);
}

async function loadCachedApp() {
  const app = cachedApp.value;
  if (!app || busy.value) return;
  busy.value = "open";
  error.value = "";
  status.value = "";
  try {
    await loadSource(app.path, app.packageId);
  } catch {
    error.value = "openFailed";
  } finally {
    busy.value = null;
  }
}

let attemptedAppId: number | undefined;
watch(
  [cachedApp, busy],
  ([app, currentBusy]) => {
    if (!app || source.value || currentBusy || attemptedAppId === app.packageId)
      return;
    attemptedAppId = app.packageId;
    void loadCachedApp();
  },
  { immediate: true },
);

async function openSource() {
  busy.value = "open";
  error.value = "";
  status.value = "";
  try {
    const selected = await open({
      title: t("studio.camera.pickSource"),
      filters: [
        { name: t("studio.camera.packageType"), extensions: ["package"] },
      ],
    });
    if (typeof selected !== "string") return;
    const app = cachedApp.value;
    await loadSource(
      selected,
      app?.path === selected ? app.packageId : undefined,
    );
  } catch {
    error.value = "openFailed";
  } finally {
    busy.value = null;
  }
}

async function exportPackage() {
  if (!source.value || invalidDraft.value) return;
  busy.value = "export";
  error.value = "";
  status.value = "";
  try {
    const outputDirectory = await tauriApi.workspace.pickDirectory(
      t("studio.camera.exportDirectory"),
    );
    if (!outputDirectory) return;
    status.value = await command<string>("export_city_camera", {
      request: {
        sourcePath: path.value,
        outputDirectory,
        revision: source.value.revision,
        edits: edits.value,
        ...(motionEdit.value === undefined
          ? {}
          : { motionStrength: motionEdit.value }),
      },
    });
  } catch (cause) {
    const code = errorCode(cause);
    error.value =
      code === "camera_source_changed"
        ? "sourceChanged"
        : code === "camera_export_permission"
          ? "exportPermission"
          : code === "camera_export_game_folder"
            ? "exportGameFolder"
            : code === "camera_curve_order"
              ? "invalidCurveOrder"
              : code === "camera_focus_table"
                ? "focusTableError"
                : code === "camera_edit_invalid"
                  ? "invalidSamples"
                  : "exportFailed";
  } finally {
    busy.value = null;
  }
}

const number = (value: number) =>
  new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(value);
</script>

<template>
  <section class="camera-page">
    <header class="page-heading">
      <div>
        <h1>{{ t("studio.camera.title") }}</h1>
        <p>{{ t("studio.camera.description") }}</p>
      </div>
      <div class="actions">
        <FButton
          v-if="cachedApp && path !== cachedApp.path && source"
          variant="ghost"
          :disabled="busy !== null"
          @click="loadCachedApp"
          >{{ t("studio.camera.useCachedApp") }}</FButton
        >
        <FButton
          variant="secondary"
          :loading="busy === 'open'"
          :disabled="busy !== null"
          @click="openSource"
          >{{
            t(source ? "studio.camera.openMod" : "studio.camera.open")
          }}</FButton
        >
        <FButton
          :disabled="!source || invalidDraft || busy !== null"
          :loading="busy === 'export'"
          @click="exportPackage"
          >{{ t("studio.camera.export") }}</FButton
        >
      </div>
    </header>

    <p v-if="error" class="notice" role="alert">
      {{ t(`studio.camera.${error}`) }}
    </p>
    <p v-if="status" class="notice" role="status">
      {{ t("studio.camera.exportSuccess", { path: status }) }}
    </p>

    <div v-if="!source" class="empty">
      <p v-if="busy === 'open'" role="status">
        {{ t("studio.camera.loadingSource") }}
      </p>
      <template v-else>
        <strong>{{ t("studio.camera.emptyTitle") }}</strong>
        <p>{{ t("studio.camera.emptyHint") }}</p>
      </template>
    </div>
    <template v-else>
      <div class="source-bar">
        <div class="source-name">
          <span>{{ t("studio.camera.source") }}</span>
          <strong :title="path">{{ sourceName }}</strong>
        </div>
        <label for="city-camera-resource" class="profile">
          <span>{{ t("studio.camera.resource") }}</span>
          <FSelect
            id="city-camera-resource"
            :model-value="String(instance)"
            :options="profileOptions"
            :disabled="busy !== null"
            @change="changeProfile(Number($event))"
          />
        </label>
        <span class="change-count">{{
          t("studio.camera.changes", {
            count: edits.length + (motionEdit === undefined ? 0 : 1),
          })
        }}</span>
      </div>
      <p v-if="invalidDraft" role="status" class="notice">
        {{ t("studio.camera.invalidDraft") }}
      </p>
      <div class="layout">
        <section class="card preview">
          <header class="card-heading">
            <h2>{{ t("studio.camera.preview") }}</h2>
            <p>{{ t("studio.camera.previewHint") }}</p>
          </header>
          <CityCameraPreview
            v-if="previewFrame"
            v-show="frame"
            :key="previewPackageId ?? 'settings'"
            :frame="previewFrame"
            :profile="shownProfile"
            :playing="playing && Boolean(frame)"
            :motion-strength="showSource ? sourceMotion : motionStrength"
            :package-id="previewPackageId"
            @tilt="nudgeTilt"
            @zoom="nudgeZoom"
            @reset="resetPose"
          />
          <p v-if="!frame" class="preview-unavailable" role="status">
            {{ t("studio.camera.previewUnavailable") }}
          </p>
          <div class="preview-controls">
            <label class="preview-control">
              <span
                >{{ t("studio.camera.zoomLevel") }}
                <output>{{ zoomIndex + 1 }} / {{ zooms.length }}</output></span
              >
              <input
                :value="zoomIndex"
                type="range"
                min="0"
                :max="Math.max(0, zooms.length - 1)"
                :disabled="zooms.length < 2"
                @input="
                  zoom = Number(($event.target as HTMLInputElement).value)
                "
              />
            </label>
            <label class="preview-control">
              <span
                >{{ t("studio.camera.tilt") }}
                <output>{{
                  frame ? number(frame.pitch) + "°" : "—"
                }}</output></span
              >
              <input
                v-model.number="tilt"
                type="range"
                min="0"
                max="1"
                step="0.01"
                :disabled="!frame || frame.pitchLocked"
              />
            </label>
            <label for="city-camera-heading" class="preview-control">
              <span>{{ t("studio.camera.fields.heading") }}</span>
              <FSelect
                id="city-camera-heading"
                :model-value="String(headingIndex)"
                :options="headingOptions"
                :disabled="!headings.length"
                @change="heading = Number($event)"
              />
            </label>
          </div>
          <dl v-if="frame" class="measurements">
            <div>
              <dt>{{ t("studio.camera.fields.distance") }}</dt>
              <dd>{{ number(frame.distance) }} m</dd>
            </div>
            <div>
              <dt>{{ t("studio.camera.fields.fov") }}</dt>
              <dd>{{ number(frame.fov) }}°</dd>
            </div>
            <div>
              <dt>{{ t("studio.camera.fields.near") }}</dt>
              <dd>{{ number(frame.near) }} m</dd>
            </div>
            <div>
              <dt>{{ t("studio.camera.fields.far") }}</dt>
              <dd>{{ number(frame.far) }} m</dd>
            </div>
          </dl>
          <div class="motion-controls">
            <div class="actions">
              <FButton
                variant="secondary"
                :disabled="!frame"
                :aria-pressed="showSource"
                @click="showSource = !showSource"
              >
                {{
                  t(
                    showSource
                      ? "studio.camera.showDraft"
                      : "studio.camera.showSource",
                  )
                }}
              </FButton>
              <FButton
                variant="secondary"
                :disabled="!frame && !playing"
                :aria-pressed="playing"
                @click="playing = !playing"
              >
                {{
                  t(
                    playing
                      ? "studio.camera.motion.stop"
                      : "studio.camera.motion.play",
                  )
                }}
              </FButton>
            </div>
            <label class="preview-control">
              <span
                >{{ t("studio.camera.motion.strength") }}
                <output>{{ number(motionStrength) }}×</output></span
              >
              <input
                v-model.number="motionStrength"
                type="range"
                min="0"
                max="1"
                step="0.01"
                :disabled="busy !== null"
              />
            </label>
            <p class="hint">{{ t("studio.camera.motion.hint") }}</p>
          </div>
        </section>

        <section class="card inspector">
          <header class="card-heading">
            <h2>{{ t("studio.camera.curves") }}</h2>
            <p>{{ t("studio.camera.curvesHint") }}</p>
          </header>
          <div v-if="curve && metadata" class="inspector-content">
            <label for="city-camera-field" class="preview-control">
              <span>{{ t("studio.camera.field") }}</span>
              <FSelect
                id="city-camera-field"
                :model-value="String(curve.hash)"
                :options="fieldOptions"
                @change="field = Number($event)"
              />
            </label>
            <p v-if="curve.inherited" class="hint">
              {{ t("studio.camera.inherited") }}
            </p>
            <fieldset class="editing" :disabled="busy !== null">
              <CameraCurveEditor
                :key="curveKey(instance, curve.hash)"
                v-model="draft"
                :curve="curve"
                :samples="samples"
                :unit="metadata.unit"
                :initial-index="initialCurveIndex"
                :limits="selectedCurveLimits"
                :disabled="busy !== null"
                @select="selectCurveSample"
              />
              <FButton
                variant="secondary"
                :disabled="drafts[curveKey(instance, curve.hash)] === undefined"
                @click="resetFields([curve.hash])"
                >{{ t("studio.camera.reset") }}</FButton
              >
            </fieldset>
          </div>
          <p v-else class="preview-unavailable">
            {{ t("studio.camera.previewUnavailable") }}
          </p>
          <CityCameraFocusControls
            v-if="focusCurves.length"
            :values="focusValues"
            :invalid="invalidFocus"
            :modified="modifiedFocus"
            :automatic-available="Boolean(profile?.autoFocus)"
            :disabled="busy !== null"
            @edit="editField"
            @reset="resetFields(focusCurves.map((item) => item.hash))"
          />
        </section>
      </div>
    </template>
  </section>
</template>

<style scoped>
.camera-page {
  display: grid;
  gap: 16px;
  padding-block-end: 40px;
}
.page-heading,
.source-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: end;
  justify-content: space-between;
  gap: 14px;
}
.page-heading h1 {
  margin: 0 0 6px;
  font-size: 26px;
}
.page-heading p,
.card-heading p,
.hint,
.empty p {
  margin: 0;
  color: var(--muted-foreground);
}
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.source-bar,
.notice,
.empty,
.card {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
}
.source-bar {
  justify-content: start;
  padding: 12px 16px;
}
.source-name,
.profile,
.preview-control {
  display: grid;
  gap: 6px;
  min-width: 0;
  font-size: 13px;
}
.source-name strong,
.notice {
  overflow-wrap: anywhere;
}
.source-name span,
.profile span,
.change-count {
  font-size: 12px;
  color: var(--muted-foreground);
}
.change-count {
  margin-inline-start: auto;
  align-self: center;
}
.notice {
  margin: 0;
  padding: 12px 16px;
}
.empty {
  display: grid;
  align-content: center;
  justify-items: center;
  gap: 8px;
  min-height: 360px;
  padding: 24px;
  text-align: center;
}
.layout {
  display: grid;
  grid-template-columns: minmax(0, 1.65fr) minmax(310px, 1fr);
  gap: 14px;
  align-items: start;
}
.card {
  min-width: 0;
  overflow: hidden;
}
.card-heading {
  display: grid;
  gap: 4px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border);
}
.card-heading h2 {
  margin: 0;
  font-size: 15px;
}
.card-heading p,
.hint {
  font-size: 12px;
}
.preview-unavailable {
  min-height: 200px;
  display: grid;
  place-items: center;
  padding: 16px;
  text-align: center;
  color: var(--muted-foreground);
}
.preview-controls,
.measurements {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 20px;
  padding: 12px 16px;
}
.preview-control {
  flex: 1 1 130px;
}
.preview-control span {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}
.preview-control input {
  width: 100%;
  accent-color: var(--brand);
  align-self: center;
}
.measurements {
  margin: 0;
  border-top: 1px solid var(--border);
}
.motion-controls {
  display: grid;
  gap: 10px;
  padding: 12px 16px;
  border-top: 1px solid var(--border);
}
.measurements dt {
  font-size: 12px;
  color: var(--muted-foreground);
}
.measurements dd {
  margin: 2px 0 0;
  font: 13px var(--font-mono, monospace);
}
.inspector-content,
.editing {
  display: grid;
  gap: 12px;
}
.inspector-content {
  padding: 14px 16px;
}
.editing {
  min-width: 0;
  padding: 0;
  border: 0;
  margin: 0;
}
.editing button {
  justify-self: start;
}
@media (max-width: 950px) {
  .layout {
    grid-template-columns: 1fr;
  }
}
</style>
