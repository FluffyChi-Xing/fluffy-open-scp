import { shallowMount } from "@vue/test-utils";
import { expect, it } from "vitest";
import { i18n } from "@/locales";
import { RasterDocument } from "@/lib/raster-editor/document";
import RasterPreviewPanel from "./RasterPreviewPanel.vue";
import RasterPreviewCard from "./RasterPreviewCard.vue";
import RasterDecalPreview from "./RasterDecalPreview.vue";

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
