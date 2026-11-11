import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "@/locales";
import BehaviorLeafSheet from "./BehaviorLeafSheet.vue";

describe("BehaviorLeafSheet", () => {
  const previousLocale = i18n.global.locale.value;
  const node = {
    id: "leaf-tourism-1",
    kind: "behavior-leaf",
    position: [0, 0],
  } as const;

  afterEach(() => {
    i18n.global.locale.value = previousLocale;
  });

  it("uses template placeholders, translates the open sheet, and applies only configured values", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, position: [0, 0] },
        params: { type: "tourism", tourismProfile: "native-tourism-v1" },
        labelKey: "studio.assetWorkbench.logicNode.leafTourism",
        trigger: null,
      },
    });
    try {
      await nextTick();
      expect(
        document.querySelector('[role="dialog"] h2')?.textContent?.trim(),
      ).toBe("编辑观光");
      const attraction = document.querySelector<HTMLInputElement>(
        'input[placeholder="400"]',
      )!;
      expect(attraction.value).toBe("");
      expect(wrapper.emitted("apply")).toBeUndefined();

      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(
        document.querySelector('[role="dialog"] h2')?.textContent?.trim(),
      ).toBe("Edit Tourism");
      expect(
        document.querySelector('[role="switch"]')?.getAttribute("aria-label"),
      ).toBe("Offer medium-wealth guest rooms");

      attraction.value = "275";
      attraction.dispatchEvent(new Event("input", { bubbles: true }));
      document.querySelector<HTMLButtonElement>('[role="switch"]')!.click();
      await nextTick();
      document
        .querySelector("form.behavior-sheet-form")!
        .dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        );
      expect(wrapper.emitted("apply")).toEqual([
        [
          {
            type: "tourism",
            tourismProfile: "native-tourism-v1",
            attraction: 275,
            lodging: true,
          },
        ],
      ]);
    } finally {
      wrapper.unmount();
    }
  });

  it("edits the Copan resident capacity and translates the active control", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-residence-1", position: [0, 0] },
        params: { type: "residence", residenceProfile: "copan-residence-v1" },
        labelKey: "studio.assetWorkbench.logicNode.leafResidence",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const input = document.querySelector<HTMLInputElement>('input[placeholder="1100"]')!;
      expect(input.value).toBe("");
      expect(input.closest("label")?.textContent?.trim()).toBe("住户容量（人数）");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(input.closest("label")?.textContent?.trim()).toBe("Resident capacity");
      input.value = "550";
      input.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      input.closest("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[
        { type: "residence", residenceProfile: "copan-residence-v1", population: 550 },
      ]]);
    } finally {
      wrapper.unmount();
    }
  });
});
