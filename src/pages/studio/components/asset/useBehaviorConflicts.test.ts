import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import { expect, it, vi } from "vitest";
import { command } from "@/api/tauri";
import { useToast } from "@/composables/useToast";
import { i18n } from "@/locales";
import { addLogicNode, connectLogic, emptyLogicGraph, ensureCoreNode, leafSpec, MAX_BEHAVIOR_LEAVES } from "./logicGraph";
import { useBehaviorConflicts } from "./useBehaviorConflicts";

vi.mock("@/api/tauri", () => ({ command: vi.fn() }));

it("sends behavior leaves without private Unit scopes", async () => {
  const core = ensureCoreNode(emptyLogicGraph(), [0, 0]);
  const office = addLogicNode(core.graph, leafSpec("office"), [0, 0]);
  const visits = addLogicNode(office.graph, leafSpec("tourism"), [0, 0]);
  let graph = connectLogic(visits.graph, core.node.id, "asset", "behavior").graph;
  graph = connectLogic(graph, office.node.id, core.node.id, "leaf").graph;
  graph = connectLogic(graph, visits.node.id, core.node.id, "leaf").graph;
  vi.mocked(command).mockResolvedValue({ conflicts: [], unknownBundles: [] });
  let result: ReturnType<typeof useBehaviorConflicts> | undefined;
  const wrapper = mount(defineComponent({ setup() { result = useBehaviorConflicts(); return () => null; } }),
    { global: { plugins: [i18n] } });
  try {
    expect(result!.canAdd(graph)).toBe(true);
    await result!.check(graph, visits.node.id);
    expect(command).toHaveBeenLastCalledWith("asset_build_m1_check_behavior_bins", expect.objectContaining({
      nodes: expect.arrayContaining([
        expect.objectContaining({ id: office.node.id }),
        expect.objectContaining({ id: visits.node.id }),
      ]),
    }));
  } finally { wrapper.unmount(); useToast().clear(); }
});

it("warns through the toast when an added leaf shares a bin with another leaf", async () => {
  const core = ensureCoreNode(emptyLogicGraph(), [0, 0]);
  const park = addLogicNode(core.graph, leafSpec("park"), [0, 0]);
  const visits = addLogicNode(park.graph, leafSpec("tourism"), [0, 0]);
  const attached = connectLogic(visits.graph, park.node.id, core.node.id, "leaf").graph;
  const graph = connectLogic(attached, visits.node.id, core.node.id, "leaf").graph;
  vi.mocked(command).mockResolvedValueOnce({
    conflicts: [{
      otherId: park.node.id,
      bin: "5C453D12",
      name: "OpenShoppers",
      addedBundle: "75D22FD1",
      otherBundle: "6EA8E701",
    }],
    unknownBundles: [],
  });
  const toast = useToast();
  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = "en-US";
  let pending: Promise<void> | undefined;
  const wrapper = mount(defineComponent({
    setup() {
      pending = useBehaviorConflicts().check(graph, visits.node.id);
      return () => null;
    },
  }), { global: { plugins: [i18n] } });
  try {
    await pending;
    expect(command).toHaveBeenCalledWith("asset_build_m1_check_behavior_bins", expect.objectContaining({
      addedId: visits.node.id,
      nodes: expect.arrayContaining([expect.objectContaining({ id: park.node.id, type: "park" })]),
    }));
    expect(toast.toasts.value.at(-1)).toMatchObject({
      tone: "warning",
      title: "Behavior bins overlap",
      message: expect.stringContaining("Tourism and Park both declare OpenShoppers"),
    });
  } finally {
    wrapper.unmount();
    toast.clear();
    i18n.global.locale.value = previousLocale;
  }
});

it("allows additional components up to the shared behavior limit", () => {
  const core = ensureCoreNode(emptyLogicGraph(), [0, 0]);
  const hotel = addLogicNode(core.graph, leafSpec("hotel"), [0, 0]);
  const full = Array.from({ length: MAX_BEHAVIOR_LEAVES }).reduce<typeof core.graph>(
    graph => addLogicNode(graph, leafSpec("park"), [0, 0]).graph, core.graph,
  );
  const toast = useToast();
  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = "en-US";
  const wrapper = mount(defineComponent({
    setup() {
      const { canAdd } = useBehaviorConflicts();
      expect(canAdd(hotel.graph)).toBe(true);
      expect(canAdd(full)).toBe(false);
      expect(toast.toasts.value.at(-1)?.message).toContain(`up to ${MAX_BEHAVIOR_LEAVES}`);
      return () => null;
    },
  }), { global: { plugins: [i18n] } });
  wrapper.unmount();
  toast.clear();
  i18n.global.locale.value = previousLocale;
});

it("blocks a duplicate leaf in every add handler without rejecting distinct component behaviors", () => {
  const park = addLogicNode(emptyLogicGraph(), leafSpec("park"), [0, 0]).graph;
  const toast = useToast();
  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = "zh-CN";
  const wrapper = mount(defineComponent({ setup() {
    const checks = useBehaviorConflicts();
    expect(checks.available(park, "park")).toBe(false);
    expect(checks.canAdd(park, "park")).toBe(false);
    expect(checks.addHint(park, "park")).toContain("编辑已有叶子");
    i18n.global.locale.value = "en-US";
    expect(checks.addHint(park, "park")).toContain("existing behavior");
    for (const type of ["residence", "tourism", "office", "commercial", "landvalue"] as const) {
      expect(checks.available(park, type)).toBe(true);
    }
    return () => null;
  } }), { global: { plugins: [i18n] } });
  wrapper.unmount(); toast.clear(); i18n.global.locale.value = previousLocale;
});

it("warns about commercial uncertainty in the active language without blocking addition", () => {
  const toast = useToast();
  const previous = i18n.global.locale.value;
  const wrapper = mount(defineComponent({ setup() {
    const checks = useBehaviorConflicts();
    for (const locale of ["zh-CN", "en-US"] as const) {
      i18n.global.locale.value = locale;
      expect(checks.available(emptyLogicGraph(), "commercial")).toBe(true);
      expect(checks.canAdd(emptyLogicGraph(), "commercial")).toBe(true);
      expect(toast.toasts.value.at(-1)).toMatchObject({
        tone: "warning", message: i18n.global.t("studio.assetWorkbench.logicNode.leafCommercialCaution"),
      });
    }
    return () => null;
  } }), { global: { plugins: [i18n] } });
  wrapper.unmount(); toast.clear(); i18n.global.locale.value = previous;
});
