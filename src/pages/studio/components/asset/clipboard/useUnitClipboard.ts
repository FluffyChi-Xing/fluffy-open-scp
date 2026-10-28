import type { LotUnitDto } from "@/api/tauri";
import { unitId } from "@/pages/packages/components/property-editor/unitGizmos";
import {
  captureUnitSpec,
  nextIndexOfKind,
  specToUnit,
  type UnitClipboardSpec,
} from "./unitClipboard";

/**
 * 工作台单元剪贴板（组合层）：持有复制规格，把 Ctrl+C / Ctrl+V 语义接到
 * 宿主会话上。粘贴 = 单条 addUnit 命令，随编辑层命令栈可 Ctrl+Z 撤销
 * （Ctrl+Z 本体由工作台既有热键承担）。表现层反馈（toast 文案）由组件按
 * 返回状态映射，本层不碰 UI。
 *
 * 宿主面刻意收窄为四个成员（不依赖整个 WorkbenchAsset），便于测试与复用。
 */

export type UnitCopyStatus = "copied" | "blocked-path-point" | "no-selection";
export type UnitPasteStatus = "pasted" | "empty-clipboard";

export interface UnitClipboardHost {
  /** 当前选中单元（null = 无选中/模型伪单元）。 */
  readonly selectedUnit: LotUnitDto | null;
  /** 生效单元集（粘贴时分配同类下一个 index）。 */
  readonly units: LotUnitDto[];
  /** 编辑层放置（单条可撤销命令）。 */
  addUnit(unit: LotUnitDto): void;
  /** 选中粘贴产物。 */
  selectUnit(id: string | null): void;
}

export function useUnitClipboard(host: UnitClipboardHost) {
  let spec: UnitClipboardSpec | null = null;

  /** Ctrl+C：捕获选中单元；pathPoint 拒绝（blocked-path-point）。 */
  function copy(): UnitCopyStatus {
    const unit = host.selectedUnit;
    if (!unit) return "no-selection";
    const captured = captureUnitSpec(unit);
    if (!captured) return "blocked-path-point";
    spec = captured;
    return "copied";
  }

  /** Ctrl+V：按规格原位重建副本（同位置、选中副本，可连续粘贴堆叠）。
   * 直挂 prop 副本与原件共享 directModelKey 载荷缓存（放置/会话恢复时
   * 已入缓存），无需重新拉取模型。 */
  function paste(): UnitPasteStatus {
    if (!spec) return "empty-clipboard";
    const unit = specToUnit(spec, nextIndexOfKind(host.units, spec.kind));
    host.addUnit(unit);
    host.selectUnit(unitId(unit));
    return "pasted";
  }

  return { copy, paste };
}
