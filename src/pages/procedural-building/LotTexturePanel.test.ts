import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { i18n } from "@/locales";
import LotTexturePanel from "./LotTexturePanel.vue";
import { tauriApi } from "@/api";

vi.mock("@/stores/gamePackages", () => ({
  useGamePackagesStore: () => ({ opened: [{ package: { packageId: 7 } }] }),
}));
vi.mock("@/api", () => ({
  tauriApi: {
    raster: {
      listPropertyDocuments: vi.fn(),
      readSource: vi.fn(async () => ({
        width: 1,
        height: 1,
        rgbaBase64: "AAAAAA==",
      })),
      readLotMaterial: vi.fn(async () => ({ colors: [[200, 190, 170, 0]] })),
    },
  },
}));
vi.mock("./lotGround", async (original) => ({
  ...(await original<typeof import("./lotGround")>()),
  lotLayerPng: vi.fn(() => "data:image/png;base64,UE5H"),
}));
afterEach(() => {
  i18n.global.locale.value = "zh-CN";
});

describe("Lot picker", () => {
  it("retains separate Lot properties sharing a mask, pages work, and switches language live", async () => {
    const docs = Array.from({ length: 33 }, (_, i) => ({
      tgi: { typeId: 1, group: i, instance: 5 },
      lotMaskPackageId: 2,
      lotMaskResolved: { typeId: 3, group: 4, instance: 6 },
      lotSize: [136 + i, 80],
    }));
    vi.mocked(tauriApi.raster.listPropertyDocuments).mockResolvedValue(
      docs as never,
    );
    const wrapper = mount(LotTexturePanel, {
      global: { plugins: [i18n], stubs: { FIcon: true } },
    });
    try {
      await flushPromises();
      expect(wrapper.findAll(".lot-entry")).toHaveLength(32);
      expect(wrapper.text()).toContain("原始尺寸 136.0 × 80.0 m");
      await wrapper.find(".lot-entry").trigger("click");
      expect(wrapper.emitted("pick")?.[0][0]).toMatchObject({
        packageId: 2,
        lotPackageId: 7,
        lotTgi: docs[0].tgi,
        lotSize: [136, 80],
      });
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(wrapper.text()).toContain("Original size 136.0 × 80.0 m");
      expect(wrapper.text()).toContain("Next page");
      await wrapper.findAll(".lot-paging button")[1].trigger("click");
      await flushPromises();
      expect(wrapper.findAll(".lot-entry")).toHaveLength(1);
      expect(wrapper.text()).toContain("168.0 × 80.0 m");
      const card = wrapper.find(".lot-entry");
      await card.trigger("pointerdown", {
        button: 0,
        clientX: 10,
        clientY: 20,
      });
      window.dispatchEvent(
        new PointerEvent("pointerup", { clientX: 400, clientY: 300 }),
      );
      expect(wrapper.emitted("drop")?.[0]).toEqual([
        expect.objectContaining({ lotTgi: docs[32].tgi, lotSize: [168, 80] }),
        400,
        300,
      ]);
      await card.trigger("click");
      expect(wrapper.emitted("pick")).toHaveLength(1); // drag must not turn into a second click
      await card.trigger("pointerdown", {
        button: 0,
        clientX: 10,
        clientY: 20,
      });
      window.dispatchEvent(new PointerEvent("pointercancel"));
      window.dispatchEvent(
        new PointerEvent("pointerup", { clientX: 450, clientY: 350 }),
      );
      expect(wrapper.emitted("drop")).toHaveLength(1);
    } finally {
      wrapper.unmount();
    }
  });
});
