import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { defineComponent, nextTick, ref } from "vue";
import { i18n } from "@/locales";
import { massPreset } from "@/lib/procedural-building/massing";
import MassingPanel from "./MassingPanel.vue";

describe("massing editor", () => {
  it("keeps the base and Y junction fixed, edits visible recesses, and updates language", async () => {
    const previous = i18n.global.locale.value;
    i18n.global.locale.value = "zh-CN";
    const mass = ref(massPreset("y"));
    const host = defineComponent({ components: { MassingPanel }, setup: () => ({ mass }), template: '<MassingPanel v-model="mass" />' });
    const wrapper = mount(host, { global: { plugins: [i18n] } });
    const field = (text: string) => wrapper.findAll("label").find(label => label.text() === text)!.find("input");
    const button = (text: string) => wrapper.findAll("button").find(button => button.text().includes(text))!;
    try {
      await field("高度 (m)").setValue(360);
      expect(mass.value.blocks[0].center[2]).toBe(180);
      await field("底部高度 (m)").setValue(12);
      expect(mass.value.blocks[0].center[2]).toBe(192);
      await field("旋转 (°)").setValue(90);
      expect(mass.value.blocks[0].center[0]).toBeCloseTo(0);
      expect(mass.value.blocks[0].center[1]).toBeCloseTo(32);
      await button("添加挖空").trigger("click");
      expect(mass.value.voids[0].center[1]).toBe(-14);
      await field("挖空偏移 X").setValue(7);
      expect(mass.value.voids[0].center[0]).toBe(7);
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(wrapper.text()).toContain("Block masses");
      expect(wrapper.text()).toContain("Cutout 1");
      expect(wrapper.text()).not.toContain("proceduralBuilding.");
      expect(field("Height (m)").element.value).toBe("360");
    } finally { wrapper.unmount(); i18n.global.locale.value = previous; }
  });

  it("does not reuse a live block ID after deletion, or remove another block's cutouts", async () => {
    const mass = ref(massPreset("y"));
    const host = defineComponent({ components: { MassingPanel }, setup: () => ({ mass }), template: '<MassingPanel v-model="mass" />' });
    const wrapper = mount(host, { global: { plugins: [i18n] } });
    try {
      await wrapper.findAll('.block-card')[2].find('.text-button').trigger('click');
      await wrapper.findAll('.remove-button')[1].trigger('click');
      await wrapper.find('.add-block-button').trigger('click');
      expect(new Set(mass.value.blocks.map(b => b.id)).size).toBe(3);
      expect(mass.value.voids[0].blockId).toBe('block-3');
      expect(mass.value.blocks.map(b => b.id)).toEqual(['block-1', 'block-3', 'block-2']);
    } finally { wrapper.unmount(); }
  });
});
