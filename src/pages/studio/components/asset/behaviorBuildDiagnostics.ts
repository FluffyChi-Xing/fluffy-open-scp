import policy from "@/lib/asset-behavior-policy.json";
import { LOGIC_NODE_SPECS, type BehaviorLeafType } from "./logicGraph";

type Translate = (key: string, values?: Record<string, string>) => string;

export function usesCopanComponents(kinds: readonly BehaviorLeafType[]): boolean {
  return kinds.some(kind => ['park', 'residence', 'tourism', 'commercial', 'office'].includes(kind))
    && kinds.every(kind => ['park', 'residence', 'tourism', 'commercial', 'office', 'tree', 'landvalue'].includes(kind));
}

export function behaviorCombinationConflict(kinds: readonly BehaviorLeafType[]) {
  // The ordinary office + retail recipe covers those two business leaves.
  if (kinds.length === 2 && kinds.includes("office") && kinds.includes("commercial")) return;
  for (const [index, first] of kinds.entries()) {
    const second = kinds.slice(index + 1).find(kind => kind === first ||
      policy.incompatible.some(([head, ...rest]) =>
        (head === first && rest.includes(kind)) || (head === kind && rest.includes(first)),
      ),
    );
    if (second) return [first, second] as const;
  }
}

export function describeBehaviorConflict(first: string, second: string, t: Translate): string {
  function label(kind: string) {
    const spec = LOGIC_NODE_SPECS.find(entry => entry.leafType === kind);
    return spec ? t(spec.labelKey) : kind;
  }
  return t("studio.assetWorkbench.behaviorSheet.buildError.BEHAVIOR_LEAF_CONFLICT_PAIR", {
    first: label(first), second: label(second),
  });
}
