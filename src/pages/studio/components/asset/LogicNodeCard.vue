<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FSelect from "@/components/ui/FSelect.vue";
import ShiftScheduleSlider from "./ShiftScheduleSlider.vue";
import BlueprintPortRow from "./BlueprintPortRow.vue";
import ResidenceBehaviorFields from "./ResidenceBehaviorFields.vue";
import type { ResidenceParams } from "./residenceBehavior";
import { COMMERCIAL_DEFAULTS, COMMERCIAL_FIELDS, commercialDefaults, type CommercialParams } from "./commercialBehavior";
import { OFFICE_DEFAULTS, OFFICE_FIELDS, type OfficeParams } from "./officeBehavior";
import { Switch } from "@/components/ui/switch";
import {
  LOGIC_NODE_SPECS,
  LAND_TEMPLATE_VALUE,
  leafSpec,
  type BehaviorCoreParams,
  type BehaviorLeafType,
  type BuildingInfoParams,
  type EducationParams,
  type LandvalueParams,
  type LogicNode,
  type LogicParams,
  type ParkParams,
  type TourismParams,
  type TreeParams,
  type UtilityMode,
  type UtilityParams,
} from "./logicGraph";

/**
 * 逻辑编排节点卡（Blender 节点风格）：参数直接编辑，变更经纯函数
 * （logicGraph.ts）回写文档；构建器用同一份参数编译 semantics。
 * 端口：info/core 右侧 out → asset；core 左侧 leaf ← 行为叶子（一对多）。
 */
const props = defineProps<{
  node: LogicNode;
  params: LogicParams;
  /** 该节点的问题码（已按 node 过滤）。 */
  issues: string[];
  disabled: boolean;
  embedded?: boolean;
}>();
const emit = defineEmits<{
  "update-params": [patch: Record<string, unknown>];
  remove: [];
  duplicate: [];
  "add-leaf": [type: BehaviorLeafType];
}>();

const { t } = useI18n();
const commercialId = useId();
const hotelRooms = computed(() => (props.params as { rooms?: number | null }).rooms);
function updateHotelRooms(event: Event): void {
  const input = event.target as HTMLInputElement;
  emit("update-params", { rooms: input.value === "" && !input.validity.badInput ? null : input.valueAsNumber });
}
const commercialParams = computed(() => props.params as CommercialParams);
const retailDefaults = computed(() => commercialDefaults(commercialParams.value.commercialProfile) ?? COMMERCIAL_DEFAULTS);
const retailProfiles = computed(() => [
  { value: "ground-commercial-w1-v1", label: t("studio.assetWorkbench.behaviorSheet.commercialW1") },
  { value: "ground-commercial-w2-v1", label: t("studio.assetWorkbench.behaviorSheet.commercialW2") },
  { value: "ground-commercial-w3-v1", label: t("studio.assetWorkbench.behaviorSheet.commercialW3") },
  ...(!commercialParams.value.commercialProfile
    ? [{ value: "", label: t("studio.assetWorkbench.behaviorSheet.commercialLegacy") }]
    : []),
]);

function patchRetail(field: (typeof COMMERCIAL_FIELDS)[number], event: Event): void {
  const input = event.target as HTMLInputElement;
  const value = input.value === "" && !input.validity.badInput ? null : input.valueAsNumber;
  emit("update-params", { [field]: value });
}
const landFields = computed<readonly (keyof LandvalueParams)[]>(() =>
  (props.params as LandvalueParams).landValueProfile
    ? ["value", "value2"]
    : ["value", "wealth", "value2", "wealth2"],
);

const isCore = computed(() => props.node.kind === "behavior-core");
const isLeaf = computed(() => props.node.kind === "behavior-leaf");
const leafType = computed<BehaviorLeafType | null>(() =>
  isLeaf.value ? ((props.params as { type?: BehaviorLeafType }).type ?? null) : null,
);
const titleKey = computed(() => {
  if (isLeaf.value && leafType.value) return leafSpec(leafType.value).labelKey;
  return (
    LOGIC_NODE_SPECS.find((spec) => spec.kind === props.node.kind)?.labelKey ??
    props.node.kind
  );
});
const icon = computed<string | null>(() => {
  if (isLeaf.value) return null;
  return LOGIC_NODE_SPECS.find((spec) => spec.kind === props.node.kind)?.icon ?? "Cpu";
});

const UTILITIES: { key: "power" | "water" | "garbage"; labelKey: string; hasRate: boolean }[] = [
  { key: "power", labelKey: "studio.assetWorkbench.utility.power", hasRate: true },
  { key: "water", labelKey: "studio.assetWorkbench.utility.water", hasRate: false },
  { key: "garbage", labelKey: "studio.assetWorkbench.utility.garbage", hasRate: false },
];
const modeOptions = computed(() => [
  { value: "inherit", label: t("studio.assetWorkbench.utility.modeInherit") },
  { value: "explicit", label: t("studio.assetWorkbench.utility.modeExplicit") },
]);

function utility(key: "power" | "water" | "garbage"): UtilityParams {
  return (props.params as BehaviorCoreParams)[key];
}
function patchUtility(
  key: "power" | "water" | "garbage",
  patch: Partial<UtilityParams>,
): void {
  emit("update-params", {
    [key]: { ...utility(key), ...patch },
  });
}
function onUtilityMode(key: "power" | "water" | "garbage", mode: string): void {
  patchUtility(key, { mode: mode as UtilityMode });
}
function onNumber(
  patchKey: string,
  event: Event,
  current: Record<string, unknown>,
  fallback: number,
): void {
  const raw = (event.target as HTMLInputElement).value.trim();
  const value = raw === "" ? fallback : Number(raw);
  if (!Number.isFinite(value)) return;
  emit("update-params", { ...current, [patchKey]: value });
}
/** 可空数字：清空 = null（后端按原版默认推导）。 */
function onOptionalNumber(
  patchKey: string,
  event: Event,
  current: Record<string, unknown>,
): void {
  const raw = (event.target as HTMLInputElement).value.trim();
  if (raw === "") {
    emit("update-params", { ...current, [patchKey]: null });
    return;
  }
  const value = Number(raw);
  if (!Number.isFinite(value) || value < 0) return;
  emit("update-params", { ...current, [patchKey]: Math.trunc(value) });
}
function onText(patchKey: string, event: Event, current: Record<string, unknown>): void {
  emit("update-params", {
    ...current,
    [patchKey]: (event.target as HTMLInputElement).value.trim(),
  });
}
function onBool(patchKey: string, event: boolean, current: Record<string, unknown>): void {
  emit("update-params", { ...current, [patchKey]: event });
}
function issueLabel(code: string): string {
  const [base, detail] = code.split(":");
  return t(`studio.assetWorkbench.logicIssue.${base}`, { name: detail ?? "" });
}
const leafOptions = computed(() =>
  LOGIC_NODE_SPECS.filter((spec) => spec.kind === "behavior-leaf").map((spec) => ({
    type: spec.leafType as BehaviorLeafType,
    label: t(spec.labelKey),
  })),
);
</script>

<template>
  <article class="logic-card" :class="{ invalid: issues.length && !disabled, embedded }">
    <header v-if="!embedded" class="logic-head">
      <FIcon v-if="icon" :name="icon" :size="13" aria-label="" />
      <strong>{{ t(titleKey) }}</strong>
      <span class="logic-id">{{ node.id }}</span>
      <button
        type="button"
        class="logic-icon-btn nodrag"
        :title="t('studio.assetWorkbench.logicDuplicate')"
        :disabled="disabled"
        @click="emit('duplicate')"
      >
        <FIcon name="Copy" :size="12" aria-label="" />
      </button>
      <button
        type="button"
        class="logic-icon-btn nodrag"
        :title="t('studio.assetWorkbench.logicRemove')"
        :disabled="disabled"
        @click="emit('remove')"
      >
        <FIcon name="Trash2" :size="12" aria-label="" />
      </button>
    </header>

    <template v-if="!embedded">
      <BlueprintPortRow
        id="out"
        type="source"
        :label="t(isCore ? 'studio.assetWorkbench.portBaseBehavior' : isLeaf ? 'studio.assetWorkbench.portBehavior' : titleKey)"
      />
      <BlueprintPortRow
        v-if="isCore"
        id="leaf"
        type="target"
        :label="t('studio.assetWorkbench.portBehavior')"
      />
    </template>

    <div class="logic-body nodrag nowheel">
      <!-- 建筑基本信息：主输入只有「需要的劳工数」，其余由构建器推导 -->
      <template v-if="node.kind === 'building-info'">
        <label class="logic-field">
          <span>{{ t("studio.assetWorkbench.labor.workers") }}</span>
          <input
            type="number"
            min="0"
            step="1"
            :value="(params as BuildingInfoParams).workers ?? ''"
            :disabled="disabled"
            @change="onOptionalNumber('workers', $event, params as unknown as Record<string, unknown>)"
          />
        </label>
        <p class="logic-hint">{{ t("studio.assetWorkbench.labor.workersHint") }}</p>
        <details class="logic-advanced">
          <summary>{{ t("studio.assetWorkbench.labor.advanced") }}</summary>
          <label v-for="field in (['w1', 'w2', 'w3'] as const)" :key="field" class="logic-field">
            <span>{{ t(`studio.assetWorkbench.labor.${field}`) }}</span>
            <input
              type="number"
              min="0"
              step="1"
              :value="(params as BuildingInfoParams)[field] ?? ''"
              :disabled="disabled"
              @change="onOptionalNumber(field, $event, params as unknown as Record<string, unknown>)"
            />
          </label>
          <label class="logic-field">
            <span>{{ t("studio.assetWorkbench.labor.minWorkers") }}</span>
            <input
              type="number"
              min="0"
              step="1"
              :placeholder="t('studio.assetWorkbench.labor.minWorkersAuto')"
              :value="(params as BuildingInfoParams).minWorkers ?? ''"
              :disabled="disabled"
              @change="onOptionalNumber('minWorkers', $event, params as unknown as Record<string, unknown>)"
            />
          </label>
        </details>
        <div class="logic-field">
          <span>{{ t("studio.assetWorkbench.scheduleTitle") }}</span>
          <ShiftScheduleSlider
            :schedule="
              Array.isArray((params as BuildingInfoParams).schedule) &&
              (params as BuildingInfoParams).schedule.length === 8
                ? ((params as BuildingInfoParams).schedule as number[])
                : [6, 6, 7, 7, 6, 6, 7, 6]
            "
            :disabled="disabled"
            @update:schedule="
              emit('update-params', {
                ...(params as unknown as Record<string, unknown>),
                schedule: $event,
              })
            "
          />
        </div>
        <p class="logic-hint">{{ t("studio.assetWorkbench.scheduleHint") }}</p>
        <p class="logic-hint">{{ t("studio.assetWorkbench.labor.hint") }}</p>
      </template>

      <!-- 基础行为聚合（不可删；inherit = 沿用 donor 闭环） -->
      <template v-else-if="isCore">
        <div v-for="utilitySpec in UTILITIES" :key="utilitySpec.key" class="logic-utility">
          <div class="logic-utility-head">
            <span>{{ t(utilitySpec.labelKey) }}</span>
            <FSelect
              class="logic-select"
              :model-value="utility(utilitySpec.key).mode"
              :disabled="disabled"
              :options="modeOptions"
              @update:model-value="onUtilityMode(utilitySpec.key, String($event))"
            />
          </div>
          <div v-if="utility(utilitySpec.key).mode === 'explicit'" class="logic-utility-params">
            <label v-if="utilitySpec.hasRate" class="logic-field">
              <span>{{ t("studio.assetWorkbench.utility.rate") }}</span>
              <input
                type="number"
                min="0"
                :value="utility(utilitySpec.key).rate ?? ''"
                :disabled="disabled"
                @change="
                  patchUtility(utilitySpec.key, {
                    rate: Number(($event.target as HTMLInputElement).value) || null,
                  })
                "
              />
            </label>
            <label class="logic-field">
              <span>{{ t("studio.assetWorkbench.utility.amount") }}</span>
              <input
                type="number"
                min="0"
                :value="utility(utilitySpec.key).amount ?? ''"
                :disabled="disabled"
                @change="
                  patchUtility(utilitySpec.key, {
                    amount: Number(($event.target as HTMLInputElement).value) || null,
                  })
                "
              />
            </label>
            <p class="logic-hint">
              {{ t("studio.assetWorkbench.utility.capacityHint") }}
            </p>
          </div>
        </div>
        <div class="logic-add-leaf">
          <span>{{ t("studio.assetWorkbench.logicAddLeaf") }}</span>
          <button
            v-for="option in leafOptions"
            :key="option.type"
            type="button"
            class="logic-chip"
            :disabled="disabled"
            @click="emit('add-leaf', option.type)"
          >
            {{ option.label }}
          </button>
        </div>
      </template>

      <!-- 行为叶子 -->
      <template v-else-if="leafType === 'education'">
        <label class="logic-field">
          <span>{{ t("studio.assetWorkbench.leaf.education.capacity") }}</span>
          <input
            type="number"
            min="1"
            :value="(params as EducationParams).capacity"
            :disabled="disabled"
            @change="onNumber('capacity', $event, params as unknown as Record<string, unknown>, 250)"
          />
        </label>
        <div class="logic-checks">
          <label v-for="flag in (['walkPath', 'servicePath', 'pedestrianPath'] as const)" :key="flag">
            <Switch
              :model-value="(params as EducationParams)[flag]"
              :disabled="disabled"
              :aria-label="t(`studio.assetWorkbench.leaf.education.${flag}`)"
              @update:model-value="onBool(flag, Boolean($event), params as unknown as Record<string, unknown>)"
            />
            <span>{{ t(`studio.assetWorkbench.leaf.education.${flag}`) }}</span>
          </label>
        </div>
        <label class="logic-field">
          <span>{{ t("studio.assetWorkbench.leaf.education.board") }}</span>
          <input
            :value="(params as EducationParams).boardInstance"
            placeholder="0x6A1785B4"
            :disabled="disabled"
            @change="onText('boardInstance', $event, params as unknown as Record<string, unknown>)"
          />
        </label>
        <label class="logic-field">
          <span>{{ t("studio.assetWorkbench.leaf.education.presentation") }}</span>
          <input
            :value="(params as EducationParams).presentationPackage"
            placeholder="build/presentation-donor.package"
            :disabled="disabled"
            @change="
              onText('presentationPackage', $event, params as unknown as Record<string, unknown>)
            "
          />
        </label>
        <label class="logic-field">
          <span>{{ t("studio.assetWorkbench.leaf.education.presentationUnit") }}</span>
          <input
            :value="(params as EducationParams).presentationUnit"
            placeholder="0xA826FA3E"
            :disabled="disabled"
            @change="
              onText('presentationUnit', $event, params as unknown as Record<string, unknown>)
            "
          />
        </label>
      </template>

      <template v-else-if="leafType === 'tourism'">
        <label class="logic-field">
          <span>{{ t("studio.assetWorkbench.leaf.tourism.attraction") }}</span>
          <input
            type="number"
            min="0"
            :value="(params as TourismParams).attraction"
            :disabled="disabled"
            @change="onNumber('attraction', $event, params as unknown as Record<string, unknown>, 400)"
          />
        </label>
        <label v-for="w in (['incomeW1', 'incomeW2', 'incomeW3'] as const)" :key="w" class="logic-field">
          <span>{{ t(`studio.assetWorkbench.leaf.tourism.${w}`) }}</span>
          <input
            type="number"
            min="0"
            :value="(params as TourismParams)[w]"
            :disabled="disabled"
            @change="onNumber(w, $event, params as unknown as Record<string, unknown>, 11)"
          />
        </label>
        <label v-if="!(params as TourismParams).tourismProfile" class="logic-field">
          <span>{{ t("studio.assetWorkbench.leaf.tourism.template") }}</span>
          <input
            :value="(params as TourismParams).templateInstance"
            placeholder="0x4E01E311"
            :disabled="disabled"
            @change="onText('templateInstance', $event, params as unknown as Record<string, unknown>)"
          />
        </label>
      </template>

      <template v-else-if="leafType === 'park'">
        <label v-for="field in (['landValue', 'landWealth', 'landValue2', 'landWealth2', 'attraction'] as const)" :key="field" class="logic-field">
          <span>{{ t(`studio.assetWorkbench.leaf.park.${field}`) }}</span>
          <input
            type="number"
            :value="(params as ParkParams)[field]"
            :disabled="disabled"
            @change="onNumber(field, $event, params as unknown as Record<string, unknown>, 0)"
          />
        </label>
      </template>

      <template v-else-if="leafType === 'commercial'">
        <div class="logic-field">
          <label :for="commercialId">{{ t("studio.assetWorkbench.behaviorSheet.commercialProfile") }}</label>
          <FSelect
            :id="commercialId"
            :options="retailProfiles"
            :model-value="commercialParams.commercialProfile ?? ''"
            :disabled="disabled"
            @change="emit('update-params', { commercialProfile: $event || undefined })"
          />
        </div>
        <label v-for="field in COMMERCIAL_FIELDS" :key="field" class="logic-field">
          <span>{{ t(`studio.assetWorkbench.leaf.commercial.${field}`) }}</span>
          <input
            type="number"
            :min="field === 'minWorkers' || field === 'shoppers' || field === 'goods' ? 1 : 0"
            max="100000"
            step="1"
            :value="commercialParams[field] ?? (commercialParams.commercialProfile ? '' : retailDefaults[field])"
            :placeholder="String(retailDefaults[field])"
            :disabled="disabled"
            @change="patchRetail(field, $event)"
          />
        </label>
      </template>

      <label v-else-if="leafType === 'hotel'" class="logic-field">
        <span>{{ t("studio.assetWorkbench.behaviorSheet.hotelRoomCount") }}</span>
        <input
          type="number"
          min="1"
          max="1000"
          step="1"
          placeholder="68"
          :value="hotelRooms ?? ''"
          :disabled="disabled"
          @change="updateHotelRooms"
        />
      </label>

      <template v-else-if="leafType === 'office'">
        <label v-for="field in OFFICE_FIELDS" :key="field" class="logic-field">
          <span>{{ t(`studio.assetWorkbench.leaf.office.${field}`) }}</span>
          <input
            type="number"
            :min="field === 'minWorkers' ? 1 : 0"
            max="100000"
            step="1"
            :value="(params as OfficeParams)[field] ?? OFFICE_DEFAULTS[field]"
            :disabled="disabled"
            @change="onNumber(field, $event, params as unknown as Record<string, unknown>, OFFICE_DEFAULTS[field])"
          />
        </label>
      </template>

      <ResidenceBehaviorFields
        v-else-if="leafType === 'residence'"
        :params="params as ResidenceParams"
        :disabled="disabled"
        @patch="emit('update-params', $event)"
      />

      <template v-else-if="leafType === 'landvalue'">
        <label v-for="field in landFields" :key="field" class="logic-field">
          <span>{{ t((params as LandvalueParams).landValueProfile
            ? `studio.assetWorkbench.behaviorSheet.land-${field}`
            : `studio.assetWorkbench.leaf.landvalue.${field}`) }}</span>
          <input
            type="number"
            :placeholder="(params as LandvalueParams).landValueProfile ? String(LAND_TEMPLATE_VALUE) : undefined"
            :value="(params as LandvalueParams)[field] ?? ''"
            :disabled="disabled"
            @change="onNumber(field, $event, params as unknown as Record<string, unknown>, (params as LandvalueParams).landValueProfile ? LAND_TEMPLATE_VALUE : 0)"
          />
        </label>
      </template>

      <template v-else-if="leafType === 'tree'">
        <label class="logic-checks">
          <Switch
            :model-value="(params as TreeParams).enabled"
            :disabled="disabled"
            :aria-label="t('studio.assetWorkbench.leaf.tree.enabled')"
            @update:model-value="onBool('enabled', Boolean($event), params as unknown as Record<string, unknown>)"
          />
          <span>{{ t("studio.assetWorkbench.leaf.tree.enabled") }}</span>
        </label>
      </template>

      <ul v-if="issues.length" class="logic-issues">
        <li v-for="issue in issues" :key="issue">{{ issueLabel(issue) }}</li>
      </ul>
    </div>
  </article>
</template>

<style scoped>
.logic-card {
  background: var(--flow-node, #282828);
  border: 1px solid var(--flow-border, #141414);
  border-radius: 8px;
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
  color: var(--flow-text, #e6e6e6);
  width: 320px;
}
.logic-card.embedded {
  width: 100%;
  background: var(--surface);
  border: 0;
  box-shadow: none;
}
.logic-card.invalid {
  border-color: #8a6a2a;
}
.logic-head {
  align-items: center;
  background: var(--flow-head-process, #2e6b2e);
  border-radius: 7px 7px 0 0;
  display: flex;
  gap: 6px;
  min-height: 34px;
  padding: 0 8px;
}
.logic-head strong {
  flex: 1;
  font-size: 12px;
}
.logic-id {
  color: rgba(255, 255, 255, 0.55);
  font-family: var(--font-mono, monospace);
  font-size: 9.5px;
}
.logic-icon-btn {
  background: transparent;
  border: 0;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  display: inline-flex;
  padding: 2px;
}
.logic-icon-btn:hover:not(:disabled) {
  color: #fff;
}
.logic-icon-btn:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}
.logic-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
}
.logic-field {
  align-items: center;
  display: grid;
  font-size: 10.5px;
  gap: 6px;
  grid-template-columns: 112px minmax(0, 1fr);
}
.logic-field > span {
  color: var(--flow-text-dim, #9b9b9b);
}
.logic-field input {
  background: var(--flow-field, #1a1a1a);
  border: 1px solid var(--flow-field-border, #3a3a3a);
  border-radius: 4px;
  color: var(--flow-text, #e6e6e6);
  font: inherit;
  font-size: 11px;
  height: 26px;
  min-width: 0;
  padding: 0 6px;
}
.logic-checks {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  font-size: 10.5px;
  gap: 8px;
}
.logic-checks label {
  align-items: center;
  display: inline-flex;
  gap: 4px;
}
.logic-utility {
  border: 1px solid var(--flow-field-border, #3a3a3a);
  border-radius: 5px;
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 6px;
}
.logic-utility-head {
  align-items: center;
  display: flex;
  font-size: 10.5px;
  gap: 6px;
  justify-content: space-between;
}
.logic-select {
  min-width: 110px;
}
.logic-select :deep(.f-select) {
  background: var(--flow-field, #1a1a1a);
  font-size: 10.5px;
  min-height: 24px;
  padding: 0 6px;
}
.logic-utility-params {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.logic-hint {
  color: var(--flow-text-dim, #9b9b9b);
  font-size: 9.5px;
  line-height: 1.4;
  margin: 0;
}
.logic-add-leaf {
  align-items: center;
  border-top: 1px dashed var(--flow-field-border, #3a3a3a);
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding-top: 6px;
}
.logic-add-leaf > span {
  color: var(--flow-text-dim, #9b9b9b);
  font-size: 10px;
  width: 100%;
}
.logic-chip {
  align-items: center;
  background: var(--flow-field, #1a1a1a);
  border: 1px solid var(--flow-field-border, #3a3a3a);
  border-radius: 999px;
  color: var(--flow-text, #e6e6e6);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 10px;
  gap: 3px;
  padding: 2px 7px;
}
.logic-chip:hover:not(:disabled) {
  border-color: #6a6a6a;
}
.logic-chip:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.logic-issues {
  color: #d9a53d;
  font-size: 9.5px;
  list-style: none;
  margin: 0;
  padding: 0;
}
</style>
