import { computed, reactive, ref, type ComputedRef } from "vue";
import type * as ThreeNamespace from "three";
import type { LotUnitDto, UnitTransformDto } from "@/api/tauri";
import { unitId } from "./unitGizmos";

/**
 * Property Editor 本地编辑层（PE-重构-2）：后端只读数据之上的
 * 可撤销 override + command 栈。MVP 不写回 DBPF——矩阵以 WPF 行主序
 * 12 floats 保存（与 UnitTransformDto 同构），写回正确性命门由
 * threeToRowMajor ↔ unitMatrix roundtrip 单测锁死。
 */

/** three Matrix4（列主序 elements）→ WPF 行主序 12 floats。 */
export function threeToRowMajor(
  matrix: ThreeNamespace.Matrix4,
): number[] {
  // three .elements 列主序 16 项；行主序 m = [e0 e1 e2 | e4 e5 e6 |
  // e8 e9 e10 | e12 e13 e14]（unitMatrix set 布局的逆映射）。
  const e = matrix.elements;
  return [
    e[0], e[1], e[2],
    e[4], e[5], e[6],
    e[8], e[9], e[10],
    e[12], e[13], e[14],
  ];
}

/** 世界空间平移：行向量约定下平移位于末行（索引 9-11），基向量不动。 */
export function translateRowMajor(
  matrix: number[],
  delta: [number, number, number],
): number[] {
  const next = [...matrix];
  next[9] += delta[0];
  next[10] += delta[1];
  next[11] += delta[2];
  return next;
}

/** 可撤销编辑命令：redo 在 push 时立即执行一次。 */
export interface UnitEditCommand {
  label: string;
  undo(): void;
  redo(): void;
}

export interface UnitEditLayer {
  /** unitId → 行主序矩阵 override（reactive，computed 直接依赖）。 */
  overrides: Map<string, number[]>;
  /** unitId → 字段 patch（元数据编辑：lightType/radius 等，不含 transform）。 */
  fieldOverrides: Map<string, Record<string, unknown>>;
  canUndo: ComputedRef<boolean>;
  canRedo: ComputedRef<boolean>;
  /** override 条数（含被撤销前的历史；UI 显示"本地编辑"状态用）。 */
  editCount: ComputedRef<number>;
  /** 记录并立即应用一条 override 命令（拖拽手柄结束时调用一次）。 */
  setUnitTransform(id: string, matrix: number[]): void;
  /** 记录并立即应用一条元数据字段 patch（命令栈可撤销）。 */
  setUnitFields(id: string, patch: Record<string, unknown>): void;
  undo(): void;
  redo(): void;
  /** 丢弃全部本地编辑（重载会话时调用）。 */
  reset(): void;
}

export function createUnitEditLayer(): UnitEditLayer {
  const overrides = reactive(new Map<string, number[]>());
  const fieldOverrides = reactive(new Map<string, Record<string, unknown>>());
  const undoStack: UnitEditCommand[] = [];
  const redoStack: UnitEditCommand[] = [];
  const version = ref(0);

  function setOverride(id: string, matrix: number[] | null) {
    if (matrix) overrides.set(id, matrix);
    else overrides.delete(id);
  }

  function setFields(id: string, patch: Record<string, unknown> | null) {
    if (patch && Object.keys(patch).length) fieldOverrides.set(id, patch);
    else fieldOverrides.delete(id);
  }

  function pushCommand(command: UnitEditCommand) {
    command.redo();
    undoStack.push(command);
    redoStack.length = 0;
    version.value += 1;
  }

  const canUndo = computed(() => {
    void version.value;
    return undoStack.length > 0;
  });
  const canRedo = computed(() => {
    void version.value;
    return redoStack.length > 0;
  });
  const editCount = computed(() => {
    void version.value;
    const ids = new Set([...overrides.keys(), ...fieldOverrides.keys()]);
    return ids.size;
  });

  return {
    overrides,
    fieldOverrides,
    canUndo,
    canRedo,
    editCount,
    setUnitTransform(id, matrix) {
      if (matrix.length !== 12) return;
      const prev = overrides.get(id) ?? null;
      pushCommand({
        label: `transform:${id}`,
        redo: () => setOverride(id, matrix),
        undo: () => setOverride(id, prev),
      });
    },
    setUnitFields(id, patch) {
      if ("transform" in patch) delete patch.transform;
      const prev = fieldOverrides.get(id) ?? null;
      const next = { ...(prev ?? {}), ...patch };
      pushCommand({
        label: `fields:${id}`,
        redo: () => setFields(id, next),
        undo: () => setFields(id, prev),
      });
    },
    undo() {
      const command = undoStack.pop();
      if (!command) return;
      command.undo();
      redoStack.push(command);
      version.value += 1;
    },
    redo() {
      const command = redoStack.pop();
      if (!command) return;
      command.redo();
      undoStack.push(command);
      version.value += 1;
    },
    reset() {
      overrides.clear();
      fieldOverrides.clear();
      undoStack.length = 0;
      redoStack.length = 0;
      version.value += 1;
    },
  };
}

/**
 * 合并视图：后端 Unit 列表应用 transform override 与字段 override。
 * 无任何 override 的 unit 原样返回（保持引用相等，避免无谓重渲染）。
 */
export function mergeUnitOverrides(
  units: LotUnitDto[],
  overrides: Map<string, number[]>,
  fieldOverrides: Map<string, Record<string, unknown>> = new Map(),
): LotUnitDto[] {
  if (!overrides.size && !fieldOverrides.size) return units;
  return units.map((unit) => {
    const id = unitId(unit);
    const fields = fieldOverrides.get(id);
    let merged = unit;
    if (fields) merged = { ...unit, ...fields } as LotUnitDto;
    const matrix = overrides.get(id);
    // pathPoint 无 transform（位置由 point 字段承载），跳过
    if (!matrix || !("transform" in merged) || !merged.transform) return merged;
    const transform: UnitTransformDto = { ...merged.transform, matrix };
    return { ...merged, transform };
  });
}
