import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, ref } from "vue";
import { expect, it, vi } from "vitest";
import type { EffectUnit } from "@/api/tauri";
import { useEffectPreview } from "./useEffectPreview";

const readEffectPreview = vi.hoisted(() => vi.fn(async () => ({ effects: [], diagnostics: [] })));
vi.mock("@/api/data-source", () => ({
  createDataSource: () => ({ readEffectPreview }),
}));

it("requests previews for placed effects even when their runtime enabled flag is false", async () => {
  const units = ref<EffectUnit[]>([{
    kind: "effect",
    index: 1,
    transform: null,
    effectId: { typeId: 0, group: 0, instance: 0x660bc390 },
    externalId: null,
    enabled: false,
    fields: [],
  }]);
  const wrapper = mount(defineComponent({
    setup() {
      useEffectPreview(() => 17, ref(true), units);
      return () => h("div");
    },
  }));
  await flushPromises();
  expect(readEffectPreview).toHaveBeenCalledWith(17, [0x660bc390]);
  wrapper.unmount();
});
