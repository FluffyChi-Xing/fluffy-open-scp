import { shallowMount } from "@vue/test-utils";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import { RasterDocument } from "@/lib/raster-editor/document";
import RasterPreviewPanel from "./RasterPreviewPanel.vue";
import RasterPreviewCard from "./RasterPreviewCard.vue";
import RasterDecalPreview from "./RasterDecalPreview.vue";
import RasterLotPreview from "./RasterLotPreview.vue";

it("places the live Decal material card after raw RGBA and updates its inputs", async () => {
  const wrapper = shallowMount(RasterPreviewPanel, {
    props: {
      doc: new RasterDocument(2, 1),
      version: 0,
      channel: "rgba",
      profile: "decal-graffiti",
      target: null,
      metersPerPixel: 0.75,
      colors: [],
      decalColors: ["#ff0000", "#00ff00", "#0000ff", "#ffffff"],
      decalAspectRatio: 2,
    },
    global: { plugins: [i18n], renderStubDefaultSlot: true },
  });
  const cards = wrapper.findAllComponents(RasterPreviewCard);
  expect(cards.map((card) => card.props("title"))).toEqual([
    i18n.global.t("rasterEditor.dataRgb"),
    i18n.global.t("rasterEditor.dataAlpha"),
    i18n.global.t("rasterEditor.previewTitle.decal-graffiti"),
  ]);
  const preview = wrapper.getComponent(RasterDecalPreview);
  expect(preview.props("aspectRatio")).toBe(2);
  await wrapper.setProps({
    version: 1,
    decalAspectRatio: 3,
    decalColors: ["#000000", "#ffffff", "#ffffff", "#ffffff"],
  });
  expect(preview.props("version")).toBe(1);
  expect(preview.props("aspectRatio")).toBe(3);
  expect(preview.props("colors")[0]).toBe("#000000");
  await wrapper.setProps({ profile: "rgba" });
  expect(wrapper.findComponent(RasterDecalPreview).exists()).toBe(false);
  wrapper.unmount();
});

it("creates a frozen Lot preview only on refresh and translates the prompt", async () => {
  const doc = new RasterDocument(1, 1);
  doc.pixels.set([255, 0, 0, 0]);
  const wrapper = shallowMount(RasterPreviewPanel, {
    props: {
      doc, version: 0, channel: "rgba", profile: "lot-mask", target: null,
      metersPerPixel: 0.75, colors: [[255, 0, 0, 0]], decalColors: [], decalAspectRatio: 1,
    },
    global: { plugins: [i18n], renderStubDefaultSlot: true },
  });
  expect(wrapper.findComponent(RasterLotPreview).exists()).toBe(false);
  expect(wrapper.text()).toContain(i18n.global.t("rasterEditor.refreshPreviewHint"));
  const lotCard = wrapper.findAllComponents(RasterPreviewCard).at(-1)!;
  expect(lotCard.props("refreshable")).toBe(true);
  lotCard.vm.$emit("refresh");
  await wrapper.vm.$nextTick();
  const preview = wrapper.getComponent(RasterLotPreview);
  const originalRaw = preview.props("raw");
  doc.pixels.set([0, 255, 0, 0]);
  await wrapper.setProps({ version: 1 });
  expect(preview.props("raw")).toBe(originalRaw);
  lotCard.vm.$emit("refresh");
  await wrapper.vm.$nextTick();
  expect(preview.props("raw")).not.toBe(originalRaw);
  const previousLocale = i18n.global.locale.value;
  i18n.global.locale.value = previousLocale === "zh-CN" ? "en-US" : "zh-CN";
  await wrapper.vm.$nextTick();
  expect(lotCard.props("title")).toBe(i18n.global.t("rasterEditor.lot3d"));
  i18n.global.locale.value = previousLocale;
  wrapper.unmount();
});
