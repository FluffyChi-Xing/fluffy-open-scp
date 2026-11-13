import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import { expect, it, vi } from "vitest";
import { command } from "@/api/tauri";
import { useToast } from "@/composables/useToast";
import { i18n } from "@/locales";
import { addLogicNode, connectLogic, emptyLogicGraph, ensureCoreNode, leafSpec } from "./logicGraph";
import { useBehaviorConflicts } from "./useBehaviorConflicts";

vi.mock("@/api/tauri", () => ({ command: vi.fn() }));

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

it("rejects incompatible choices and the fifth behavior with a toast", () => {
  const core = ensureCoreNode(emptyLogicGraph(), [0, 0]);
  const hotel = addLogicNode(core.graph, leafSpec("hotel"), [0, 0]);
  const full = (["park", "tourism", "landvalue", "tree"] as const).reduce(
    (graph, type) => addLogicNode(graph, leafSpec(type), [0, 0]).graph, core.graph,
  );
  const toast = useToast();
  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = "en-US";
  const wrapper = mount(defineComponent({
    setup() {
      const { canAdd } = useBehaviorConflicts();
      expect(canAdd(hotel.graph, "tourism")).toBe(false);
      expect(toast.toasts.value.at(-1)).toMatchObject({
        title: "Potential behavior conflict",
        message: expect.stringContaining("Tourism may conflict with Hotel"),
      });
      expect(canAdd(full, "residence")).toBe(false);
      expect(toast.toasts.value.at(-1)?.message).toContain("up to 4");
      return () => null;
    },
  }), { global: { plugins: [i18n] } });
  wrapper.unmount();
  toast.clear();
  i18n.global.locale.value = previousLocale;
});
