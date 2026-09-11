<script setup lang="ts">
import { computed } from "vue";
import * as THREE from "three";
import type { LotUnitDto } from "@/api/tauri";
import { unitId, unitMatrix } from "./unitGizmos";
import { threeToRowMajor } from "./unitEditLayer";

/**
 * 坐标 tab：当前选中 Unit 的变换（lot 空间，Z-up 游戏坐标）。
 * 位置 XYZ 可编辑（写本地 transform override）；旋转/缩放只读展示，
 * 编辑走视口手柄。手柄拖拽期间由 live prop 提供实时数值。
 */
const props = defineProps<{
  unit: LotUnitDto | null;
  /** 手柄拖拽中的实时位置（display-only）；null = 非拖拽。 */
  live?: { position: [number, number, number] } | null;
}>();
const emit = defineEmits<{
  "update-transform": [id: string, matrix: number[]];
}>();

const transform = computed(() => {
  const unit = props.unit;
  if (!unit || !("transform" in unit)) return null;
  const matrix = unitMatrix(THREE, unit.transform ?? { matrix: [] });
  const position = new THREE.Vector3();
  const quaternion = new THREE.Quaternion();
  const scale = new THREE.Vector3();
  matrix.decompose(position, quaternion, scale);
  const euler = new THREE.Euler().setFromQuaternion(quaternion, "XYZ");
  const deg = 180 / Math.PI;
  return {
    position: [position.x, position.y, position.z] as [number, number, number],
    rotation: [
      euler.x * deg,
      euler.y * deg,
      euler.z * deg,
    ] as [number, number, number],
    scale: [scale.x, scale.y, scale.z] as [number, number, number],
    quaternion,
  };
});

/** 展示值：拖拽中优先实时值。 */
const displayPosition = computed(
  () => props.live?.position ?? transform.value?.position ?? null,
);

function setPosition(axis: 0 | 1 | 2, event: Event) {
  const unit = props.unit;
  const value = transform.value;
  if (!unit || !value) return;
  const input = Number((event.target as HTMLInputElement).value);
  if (!Number.isFinite(input)) return;
  const position = new THREE.Vector3(...value.position);
  position.setComponent(axis, input);
  const matrix = new THREE.Matrix4().compose(
    position,
    value.quaternion,
    new THREE.Vector3(...value.scale),
  );
  emit("update-transform", unitId(unit), threeToRowMajor(matrix));
}
</script>

<template>
  <div class="transform-panel">
    <p v-if="!unit" class="panel-empty">{{ $t("package.noUnitSelected") }}</p>
    <template v-else-if="transform">
      <section class="panel-section">
        <h4>{{ $t("package.transformPosition") }}</h4>
        <div class="axis-rows">
          <label v-for="(axis, i) in ['X', 'Y', 'Z']" :key="axis" class="axis-row">
            <span class="axis-chip" :data-axis="axis.toLowerCase()">{{ axis }}</span>
            <input
              type="number"
              step="0.1"
              :value="Number((displayPosition ?? [0, 0, 0])[i].toFixed(3))"
              :aria-label="`${$t('package.transformPosition')} ${axis}`"
              @change="setPosition(i as 0 | 1 | 2, $event)"
            />
          </label>
        </div>
      </section>
      <section class="panel-section">
        <h4>{{ $t("package.transformRotation") }}</h4>
        <p class="readonly-line">
          {{ transform!.rotation.map((v) => `${v.toFixed(1)}°`).join(" · ") }}
        </p>
      </section>
      <section class="panel-section">
        <h4>{{ $t("package.transformScale") }}</h4>
        <p class="readonly-line">
          {{ transform!.scale.map((v) => v.toFixed(3)).join(" · ") }}
        </p>
      </section>
    </template>
    <p v-else class="panel-empty">{{ $t("package.transformNone") }}</p>
  </div>
</template>

<style scoped>
.transform-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
}
.panel-section h4 {
  color: var(--subtle-foreground);
  font-size: 11px;
  font-weight: 600;
  margin: 0 0 6px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.axis-rows {
  display: grid;
  gap: 4px;
}
.axis-row {
  align-items: center;
  display: grid;
  gap: 8px;
  grid-template-columns: 22px 1fr;
}
.axis-chip {
  border-radius: var(--radius-sm);
  color: #fff;
  font-size: 10.5px;
  font-weight: 700;
  line-height: 1;
  padding: 4px 0;
  text-align: center;
}
.axis-chip[data-axis="x"] { background: #c0392b; }
.axis-chip[data-axis="y"] { background: #1e8449; }
.axis-chip[data-axis="z"] { background: #1f618d; }
.axis-row input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  min-height: 26px;
  padding: 0 8px;
  width: 100%;
}
.readonly-line {
  color: var(--foreground);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  margin: 0;
}
.panel-empty {
  color: var(--subtle-foreground);
  font-size: 12px;
  margin: 12px;
}
</style>
