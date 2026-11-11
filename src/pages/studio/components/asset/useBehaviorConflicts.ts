import { command } from "@/api/tauri";
import { useToast } from "@/composables/useToast";
import { useI18n } from "vue-i18n";
import { leafSpec, leafTypeOf, type BehaviorLeafType, type LogicGraph } from "./logicGraph";

interface BehaviorBinReport {
  conflicts: {
    otherId: string;
    bin: string;
    name: string | null;
    addedBundle: string;
    otherBundle: string;
  }[];
  unknownBundles: string[];
}

export function useBehaviorConflicts() {
  const { t } = useI18n();
  const toast = useToast();
  const key = "studio.assetWorkbench.behaviorConflicts";

  function canAdd(graph: LogicGraph, type: BehaviorLeafType): boolean {
    if (
      (type === "tourism" || type === "residence" || type === "landvalue") &&
      graph.nodes.some((node) => leafTypeOf(graph, node) === type)
    ) {
      toast.warning(t(`${key}.duplicateLeaf`, { behavior: t(leafSpec(type).labelKey) }));
      return false;
    }
    return true;
  }

  async function check(graph: LogicGraph, addedId: string): Promise<void> {
    const attachment = graph.edges.find(
      (edge) => edge.source === addedId && edge.targetHandle === "leaf",
    );
    if (!attachment) return;
    const siblings = graph.edges
      .filter((edge) => edge.target === attachment.target && edge.targetHandle === "leaf")
      .map((edge) => edge.source);
    const nodes = siblings.flatMap((id) => {
      const node = graph.nodes.find((node) => node.id === id);
      const type = node && leafTypeOf(graph, node);
      return type ? [{ id, type, params: graph.params[id] }] : [];
    });
    const added = nodes.find((node) => node.id === addedId);
    if (!added || nodes.length < 2) return;

    try {
      const result = await command<BehaviorBinReport>("asset_build_m1_check_behavior_bins", {
        nodes,
        addedId,
      });
      const first = result.conflicts[0];
      if (first) {
        const other = nodes.find((node) => node.id === first.otherId);
        const details = t(`${key}.detail`, {
          added: t(leafSpec(added.type).labelKey),
          other: other ? t(leafSpec(other.type).labelKey) : first.otherId,
          bin: first.name ?? `0x${first.bin}`,
          addedBundle: first.addedBundle,
          otherBundle: first.otherBundle,
        });
        const extra = result.conflicts.length - 1;
        toast.warning(extra ? t(`${key}.additional`, { details, count: extra }) : details, {
          title: t(`${key}.title`),
          duration: 8000,
        });
      }
      if (result.unknownBundles.length) {
        toast.warning(t(`${key}.unknown`, { bundles: result.unknownBundles.join(", ") }));
      }
    } catch {
      toast.warning(t(`${key}.unavailable`));
    }
  }

  return { canAdd, check };
}
