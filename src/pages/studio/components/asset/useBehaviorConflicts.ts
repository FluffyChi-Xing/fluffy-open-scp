import { command } from "@/api/tauri";
import { useToast } from "@/composables/useToast";
import { useI18n } from "vue-i18n";
import { MAX_BEHAVIOR_LEAVES, isLeaf, leafSpec, leafTypeOf, type LogicGraph, type BehaviorLeafType } from "./logicGraph";

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

  function additionIssue(graph: LogicGraph, type?: BehaviorLeafType): "duplicate" | "limit" | "casinoComposition" | "controlnetComposition" | "fusionComposition" | undefined {
    const leaves = graph.nodes.filter(isLeaf);
    if (type && leaves.some(node => leafTypeOf(graph, node) === type)) return "duplicate";
    if (leaves.length >= MAX_BEHAVIOR_LEAVES) return "limit";
    const types = leaves.map(node => leafTypeOf(graph, node));
    if (type && ((type === "fusion-power" && types.length > 0) || types.includes("fusion-power"))) {
      return "fusionComposition";
    }
    const canShareControlNet = (kind: BehaviorLeafType | null) => kind === "tree" || kind === "landvalue";
    if (type && ((type === "controlnet" && types.some(kind => !canShareControlNet(kind)))
      || (types.includes("controlnet") && !canShareControlNet(type)))) return "controlnetComposition";
    if ((type === "casino" && (types.length !== 1 || types[0] !== "tourism"))
        || (type && type !== "tourism" && types.includes("casino"))) return "casinoComposition";
  }
  function addHint(graph: LogicGraph, type: BehaviorLeafType): string {
    const issue = additionIssue(graph, type);
    if (issue === "duplicate") return t(`${key}.duplicate`, { behavior: t(leafSpec(type).labelKey) });
    if (issue === "casinoComposition" || issue === "controlnetComposition" || issue === "fusionComposition") return t(`${key}.${issue}`);
    return issue === "limit" ? t(`${key}.limit`, { count: MAX_BEHAVIOR_LEAVES }) : "";
  }
  function available(graph: LogicGraph, type: BehaviorLeafType): boolean {
    return !additionIssue(graph, type);
  }
  function canAdd(graph: LogicGraph, type?: BehaviorLeafType): boolean {
    const issue = additionIssue(graph, type);
    if (issue === "duplicate" && type) {
      toast.warning(addHint(graph, type));
      return false;
    }
    if (issue === "limit") {
      toast.warning(t(`${key}.limit`, { count: MAX_BEHAVIOR_LEAVES }));
      return false;
    }
    if (issue === "casinoComposition" || issue === "controlnetComposition" || issue === "fusionComposition") {
      toast.warning(t(`${key}.${issue}`));
      return false;
    }
    if (type === "commercial") {
      toast.warning(t("studio.assetWorkbench.logicNode.leafCommercialCaution"), { duration: 8000 });
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

  return { canAdd, check, available, addHint };
}
