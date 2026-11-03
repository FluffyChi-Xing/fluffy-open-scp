<script setup lang="ts">
import { computed, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { EffectDetailData, LotUnitDto, Tgi, UnitKeyDto } from "@/api/tauri";
import { createDataSource } from "@/api/data-source";
import EffectTagSection from "../unit-inspector/EffectTagSection.vue";
import { propSemanticCategory } from "./assetCategories";

const props = defineProps<{
  unit: LotUnitDto | null;
  /** 当前 lot 的全部 unit（用于统计同 bin 的槽位/资源）。 */
  units?: readonly LotUnitDto[];
  /** 当前包；选中 effect 时用于按需解析 Swarm 详情。 */
  packageId?: number | null;
  /** 当前 lot 资源 TGI；effect 语义标签创建时记录来源。 */
  sourceTgi?: Tgi | null;
}>();
const { t } = useI18n();
const source = createDataSource();

interface SummaryRow {
  label: string;
  value: string;
  swatch?: string;
}

/** 已定谳的资源 bin 名称（probe/school/garbage 文档）；其余只显示 ID。 */
const RESOURCE_BIN_NAMES: Record<number, string> = {
  0x54ca89f0: "GarbageDisplayCount",
};

function formatKey(key: UnitKeyDto) {
  const hex = (value: number) => value.toString(16).padStart(8, "0").toUpperCase();
  return `T ${hex(key.typeId)} · G ${hex(key.group)} · I ${hex(key.instance)}`;
}
function formatVector(values: readonly number[]) {
  return `(${values.map((value) => value.toFixed(3)).join(", ")})`;
}

const summary = computed<SummaryRow[]>(() => {
  const unit = props.unit;
  if (!unit) return [];
  const rows: SummaryRow[] = [];
  switch (unit.kind) {
    case "light": {
      if (unit.lightType)
        rows.push({ label: t("package.lightType"), value: unit.lightType });
      if (unit.color)
        rows.push({
          label: t("package.lightColor"),
          value: formatVector(unit.color),
          swatch: unit.color
            .map((channel) =>
              Math.round(Math.min(Math.max(channel, 0), 1) * 255)
                .toString(16)
                .padStart(2, "0"),
            )
            .join(""),
        });
      if (unit.outerRadius !== null)
        rows.push({ label: t("package.lightRadius"), value: String(unit.outerRadius) });
      if (unit.innerRadius !== null)
        rows.push({ label: t("package.lightInnerRadius"), value: String(unit.innerRadius) });
      if (unit.diffuse !== null)
        rows.push({ label: t("package.lightDiffuse"), value: String(unit.diffuse) });
      if (unit.length !== null)
        rows.push({ label: t("package.lightLength"), value: String(unit.length) });
      if (unit.cullDistance)
        rows.push({ label: t("package.lightCullDistance"), value: unit.cullDistance });
      if (unit.isVolumetric !== null)
        rows.push({ label: t("package.lightVolumetric"), value: String(unit.isVolumetric) });
      if (unit.debugName)
        rows.push({ label: t("package.lightDebugName"), value: unit.debugName });
      break;
    }
    case "effect": {
      if (unit.effectId)
        rows.push({ label: t("package.effectId"), value: formatKey(unit.effectId) });
      if (unit.externalId)
        rows.push({ label: t("package.effectExternalId"), value: formatKey(unit.externalId) });
      if (unit.enabled !== null)
        rows.push({ label: t("package.effectEnabled"), value: String(unit.enabled) });
      break;
    }
    case "decal": {
      rows.push({ label: t("package.decalCategory"), value: String(unit.category + 1) });
      if (unit.scale !== null)
        rows.push({ label: t("package.decalScale"), value: String(unit.scale) });
      if (unit.depth !== null)
        rows.push({ label: t("package.decalDepth"), value: String(unit.depth) });
      if (unit.materialData)
        rows.push({ label: t("package.decalMaterial"), value: formatVector(unit.materialData) });
      break;
    }
    case "prop": {
      rows.push({ label: t("package.propBin"), value: String(unit.bin) });
      if (unit.slot !== null)
        rows.push({ label: t("package.propSlot"), value: String(unit.slot) });
      if (unit.scale !== null)
        rows.push({ label: t("package.propScale"), value: String(unit.scale) });
      const binSiblings = (props.units ?? []).filter(
        (candidate): candidate is Extract<LotUnitDto, { kind: "prop" }> =>
          candidate.kind === "prop" && candidate.bin === unit.bin,
      );
      const resourceIds = [
        ...new Set(
          binSiblings
            .map((candidate) => candidate.resourceId)
            .filter((value): value is number => typeof value === "number"),
        ),
      ];
      const semantic = propSemanticCategory(
        unit.resourceId,
        unit.displayName ?? "",
      );
      rows.push({
        label: t("package.propSemantic"),
        value: t(`package.propSemanticKinds.${semantic}`),
      });
      // EcoResourceBin：bin 的可见槽位由资源的 amount/capacity 驱动；
      // 离线 property 只有资源引用，没有运行时存量，因此这里标注推断来源。
      rows.push({
        label: t("package.propEcoResourceBin"),
        value: resourceIds.length
          ? resourceIds
              .map((id) => {
                const hex = `0x${(id >>> 0).toString(16).padStart(8, "0").toUpperCase()}`;
                const name = RESOURCE_BIN_NAMES[id >>> 0];
                return name ? `${name} (${hex})` : hex;
              })
              .join(", ")
          : t("package.propUnresolved"),
        swatch: undefined,
      });
      rows.push({
        label: t("package.propBinSlots"),
        value: String(binSiblings.length),
      });
      if (unit.percentFill !== null && unit.percentFill !== undefined)
        rows.push({
          label: t("package.propPercentFill"),
          value: unit.percentFill ? t("package.yes") : t("package.no"),
        });
      if (unit.randomizeSlot !== null && unit.randomizeSlot !== undefined)
        rows.push({
          label: t("package.propRandomizeSlot"),
          value: unit.randomizeSlot ? t("package.yes") : t("package.no"),
        });
      break;
    }
    case "pathPoint": {
      if (unit.point)
        rows.push({ label: t("package.pathPointPosition"), value: formatVector(unit.point) });
      if (unit.tangent)
        rows.push({ label: t("package.pathPointTangent"), value: formatVector(unit.tangent) });
      if (unit.pointIndex !== null)
        rows.push({ label: t("package.pathPointIndex"), value: String(unit.pointIndex) });
      break;
    }
    case "spawner": {
      if (unit.id) rows.push({ label: t("package.spawnerId"), value: formatKey(unit.id) });
      if (unit.count !== null)
        rows.push({ label: t("package.spawnerCount"), value: String(unit.count) });
      if (unit.countRandom !== null)
        rows.push({ label: t("package.spawnerCountRandom"), value: String(unit.countRandom) });
      if (unit.agent)
        rows.push({ label: t("package.spawnerAgent"), value: formatKey(unit.agent) });
      break;
    }
  }
  if ("transform" in unit && unit.transform)
    rows.push({
      label: t("package.unitTransform"),
      value: formatVector(unit.transform.matrix.slice(9, 12)),
    });
  return rows;
});

const fields = computed(() => props.unit?.fields ?? []);
const empty = computed(() => !props.unit);

/** 选中的 effect 单元（非 effect 或未选中时为 null）；语义标签 section 挂它。 */
const effectUnit = computed(() =>
  props.unit?.kind === "effect" ? props.unit : null,
);

/** 选中 effect 时按名字哈希解析 Swarm 集合；与粒子渲染开关无关。 */
const effectHash = computed(() =>
  props.unit?.kind === "effect" ? (props.unit.effectId?.instance ?? null) : null,
);
const detail = shallowRef<EffectDetailData | null>(null);
const detailLoading = ref(false);
const detailFailed = ref(false);
let detailGeneration = 0;

watch(
  [effectHash, () => props.packageId ?? null],
  async ([hash, packageId]) => {
    const generation = ++detailGeneration;
    detail.value = null;
    detailFailed.value = false;
    detailLoading.value = false;
    if (hash === null || packageId === null) return;
    detailLoading.value = true;
    try {
      const resolved = await source.readEffectDetail(packageId, hash);
      if (generation !== detailGeneration) return;
      detail.value = resolved;
    } catch (error) {
      if (generation !== detailGeneration) return;
      console.error("[effect] detail failed", error);
      detailFailed.value = true;
    } finally {
      if (generation === detailGeneration) detailLoading.value = false;
    }
  },
  { immediate: true },
);

const BLOCK_TYPE_NAMES: Record<number, string> = {
  0: "VisualEffect",
  1: "Particles",
  2: "MetaParticles",
  3: "Decal",
  4: "Sequence",
  5: "Sound",
  6: "Shake",
  7: "Camera",
  8: "Model",
  9: "Screen",
  10: "Light",
  11: "Game",
  12: "FastParticles",
  13: "Distribute",
  14: "Ribbon",
  15: "Animate",
  16: "Beam",
};

function blockName(type: number): string {
  return BLOCK_TYPE_NAMES[type] ?? `External ${type}`;
}

function formatCurve(values: readonly number[]): string {
  if (!values.length) return "—";
  if (values.length === 1) return String(values[0]);
  return `${values[0]} → ${values[values.length - 1]}`;
}

const atlasEmitter = computed(
  () => detail.value?.emitters.find((emitter) => emitter.texturePngBase64) ?? null,
);
const atlasPng = computed(() => atlasEmitter.value?.texturePngBase64 ?? null);
const atlasTiles = computed(() => {
  const particle = atlasEmitter.value?.particle;
  return particle ? `${particle.tileCount[0]}×${particle.tileCount[1]}` : "";
});
const atlasFrame = computed(() => {
  const particle = atlasEmitter.value?.particle;
  return particle ? `${particle.frameStart} + ${particle.frameCount}` : "";
});
const atlasFrameStyle = computed(() => {
  const emitter = atlasEmitter.value;
  if (!emitter || !emitter.texturePngBase64) return {};
  const { tileCount, frameStart } = emitter.particle;
  const columns = Math.max(1, tileCount[0]);
  const rows = Math.max(1, tileCount[1]);
  const frame = Math.max(0, frameStart);
  const column = columns > 1 ? (frame % columns) / (columns - 1) : 0;
  const row = rows > 1 ? Math.floor(frame / columns) / (rows - 1) : 0;
  return {
    backgroundImage: `url(data:image/png;base64,${emitter.texturePngBase64})`,
    backgroundSize: `${columns * 100}% ${rows * 100}%`,
    backgroundPosition: `${column * 100}% ${row * 100}%`,
  };
});


</script>

<template>
  <aside class="properties" :aria-label="$t('package.properties')">
    <template v-if="empty">
      <p class="properties-empty">{{ $t("package.noUnitSelected") }}</p>
    </template>
    <template v-else>
      <h3 class="properties-title">{{ $t("package.properties") }}</h3>
      <dl class="summary-list">
        <div v-for="row in summary" :key="row.label">
          <dt>{{ row.label }}</dt>
          <dd>
            <span
              v-if="row.swatch"
              class="swatch"
              :style="{ background: `#${row.swatch}` }"
              aria-hidden="true"
            />
            {{ row.value }}
          </dd>
        </div>
      </dl>
      <section v-if="effectHash !== null" class="effect-detail">
        <h4 class="fields-title">{{ t("package.effectDetailTitle") }}</h4>
        <p v-if="detailLoading" class="effect-note">
          {{ t("package.effectDetailLoading") }}
        </p>
        <p v-else-if="detailFailed" class="effect-note error">
          {{ t("package.effectDetailError") }}
        </p>
        <p v-else-if="!detail" class="effect-note">
          {{ t("package.effectDetailEmpty") }}
        </p>
        <div v-else class="effect-detail-scroll">
          <dl class="summary-list">
            <div>
              <dt>{{ t("package.effectDetailCollection") }}</dt>
              <dd>
                {{ detail.collectionInstance.toString(16).padStart(8, "0").toUpperCase() }}
              </dd>
            </div>
            <div>
              <dt>{{ t("package.effectDetailVisual") }}</dt>
              <dd>#{{ detail.visualIndex }} · {{ detail.blocks.length }} blocks</dd>
            </div>
            <div v-if="detail.unsupportedBlocks">
              <dt>{{ t("package.effectDetailUnsupported") }}</dt>
              <dd>{{ detail.unsupportedBlocks }}</dd>
            </div>
          </dl>

          <h5 class="effect-subtitle">{{ t("package.effectDetailStructure") }}</h5>
          <ul class="effect-blocks">
            <li v-for="(block, index) in detail.blocks" :key="index">
              <span class="block-name">{{ blockName(block.blockType) }}</span>
              <span class="block-meta">
                #{{ block.componentIndex }} · {{ formatVector(block.offset) }} · ×{{ block.scale.toFixed(2) }}
              </span>
            </li>
          </ul>

          <template v-if="detail.emitters.length">
            <h5 class="effect-subtitle">{{ t("package.effectDetailParticle") }}</h5>
            <div
              v-for="(emitter, index) in detail.emitters"
              :key="index"
              class="emitter-card"
            >
              <div class="emitter-title">
                #{{ index }} · component {{ emitter.particle.index }}
              </div>
              <dl class="summary-list">
                <div>
                  <dt>{{ t("package.effectDetailLife") }}</dt>
                  <dd>{{ emitter.particle.life[0] }} – {{ emitter.particle.life[1] }}</dd>
                </div>
                <div>
                  <dt>{{ t("package.effectDetailRate") }}</dt>
                  <dd>{{ emitter.particle.rate.slice(0, 4).join(" ") || "—" }}</dd>
                </div>
                <div>
                  <dt>{{ t("package.effectDetailSize") }}</dt>
                  <dd>{{ formatCurve(emitter.particle.size) }}</dd>
                </div>
                <div>
                  <dt>{{ t("package.effectDetailAlpha") }}</dt>
                  <dd>{{ formatCurve(emitter.particle.alpha) }}</dd>
                </div>
                <div>
                  <dt>{{ t("package.effectDetailTiles") }}</dt>
                  <dd>
                    {{ emitter.particle.tileCount[0] }}×{{ emitter.particle.tileCount[1] }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("package.effectDetailFrame") }}</dt>
                  <dd>
                    {{ emitter.particle.frameStart }} + {{ emitter.particle.frameCount }} @
                    {{ emitter.particle.frameSpeed }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("package.effectDetailDrawMode") }}</dt>
                  <dd>{{ emitter.particle.texture.drawMode }}</dd>
                </div>
              </dl>
            </div>

            <h5 class="effect-subtitle">{{ t("package.effectDetailAtlas") }}</h5>
            <div v-if="atlasPng" class="atlas-preview">
              <div class="atlas-frame" :style="atlasFrameStyle" />
              <div class="atlas-caption">
                {{ atlasTiles }} · {{ t("package.effectDetailFrame") }} {{ atlasFrame }}
              </div>
            </div>
            <p v-else class="effect-note">{{ t("package.effectDetailNoAtlas") }}</p>
          </template>
          <p v-else class="effect-note">{{ t("package.effectDetailEmpty") }}</p>
        </div>
      </section>
      <EffectTagSection
        v-if="effectUnit"
        :effect-id="effectUnit.effectId"
        :source-tgi="sourceTgi ?? null"
      />
      <h4 v-if="fields.length" class="fields-title">
        {{ $t("package.unitFields") }}
      </h4>
      <table v-if="fields.length" class="fields-table">
        <thead>
          <tr>
            <th>{{ $t("package.propertyColumn") }}</th>
            <th>{{ $t("package.valueColumn") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="field in fields" :key="field.hash">
            <td>
              <span>0x{{ field.hash.toString(16).padStart(8, "0").toUpperCase() }}</span>
              <small>{{ field.typeName }}</small>
            </td>
            <td>{{ field.value }}</td>
          </tr>
        </tbody>
      </table>
    </template>
  </aside>
</template>

<style scoped>
.properties {
  border-inline-start: 1px solid var(--border);
  min-height: 0;
  overflow-y: auto;
  padding: 12px;
}
.properties-empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  text-align: center;
  padding-top: 32px;
}
.properties-title {
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  margin: 0 0 8px;
  text-transform: uppercase;
}
.summary-list {
  display: grid;
  gap: 7px;
  margin: 0;
}
/* 键值两列布局：键列按内容自适应并给足下限，值列占剩余宽度 */
.summary-list > div {
  align-items: baseline;
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(96px, max-content) minmax(0, 1fr);
}
.summary-list dt {
  color: var(--subtle-foreground);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
.summary-list dd {
  align-items: center;
  color: var(--foreground);
  display: flex;
  font-size: 12px;
  gap: 6px;
  margin: 1px 0 0;
  overflow-wrap: anywhere;
}
.swatch {
  border: 1px solid var(--border);
  border-radius: 3px;
  display: inline-block;
  flex: none;
  height: 12px;
  width: 12px;
}
.fields-title {
  border-top: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  margin: 14px 0 8px;
  padding-top: 10px;
  text-transform: uppercase;
}
.fields-table {
  border-collapse: collapse;
  font-size: 11px;
  /* 固定布局 + 键列定宽：值列长串（Transform 12 元组等）不再挤压键列 */
  table-layout: fixed;
  width: 100%;
}
.fields-table th {
  border-bottom: 1px solid var(--border);
  color: var(--subtle-foreground);
  font-size: 9px;
  letter-spacing: 0.06em;
  padding: 4px 6px;
  text-align: start;
  text-transform: uppercase;
}
.fields-table th:first-child,
.fields-table td:first-child {
  width: 96px;
}
.fields-table td {
  border-top: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  color: var(--muted-foreground);
  overflow-wrap: anywhere;
  padding: 5px 6px;
  vertical-align: top;
}
.fields-table td:first-child {
  overflow-wrap: normal;
}
.fields-table td span {
  color: var(--foreground);
  display: block;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.fields-table td small {
  color: var(--subtle-foreground);
  font-size: 9px;
}
.effect-detail {
  margin-top: 14px;
}
.effect-detail-scroll {
  /* 属性栏本身可滚动；详情内容再限高，长 block/emitter 列表不会把面板撑爆。 */
  max-height: min(52vh, 460px);
  overflow-y: auto;
  padding-right: 4px;
}
.effect-note {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 4px 0 8px;
}
.effect-note.error {
  color: var(--destructive, #ff6b5e);
}
.effect-subtitle {
  border-top: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 10px;
  letter-spacing: 0.06em;
  margin: 12px 0 6px;
  padding-top: 8px;
  text-transform: uppercase;
}
.effect-blocks {
  display: grid;
  gap: 3px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.effect-blocks li {
  border-top: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  display: flex;
  font-size: 10px;
  gap: 8px;
  justify-content: space-between;
  padding: 3px 0;
}
.block-name {
  color: var(--foreground);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  flex: none;
}
.block-meta {
  color: var(--subtle-foreground);
  overflow-wrap: anywhere;
  text-align: end;
}
.emitter-card {
  border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  border-radius: 4px;
  margin: 0 0 8px;
  padding: 6px 8px;
}
.emitter-title {
  color: var(--muted-foreground);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 10px;
  margin-bottom: 4px;
}
.atlas-preview {
  align-items: flex-start;
  display: flex;
  gap: 10px;
}
.atlas-frame {
  background-color: #000;
  background-repeat: no-repeat;
  border: 1px solid var(--border);
  flex: none;
  height: 96px;
  image-rendering: pixelated;
  width: 96px;
}
.atlas-caption {
  color: var(--subtle-foreground);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 10px;
  line-height: 1.5;
}
</style>
