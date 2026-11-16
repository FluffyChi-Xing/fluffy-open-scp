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

  it("translates the retail capacity controls and saves a custom value", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-commercial-1", position: [0, 0] },
        params: { type: "commercial" },
        labelKey: "studio.assetWorkbench.logicNode.leafCommercial",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const shoppers = dialog.querySelectorAll<HTMLInputElement>('input[type="number"]')[1];
      expect(shoppers.value).toBe("9");
      expect(shoppers.closest("label")?.textContent?.trim()).toBe("中收入顾客 ($$)");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(shoppers.closest("label")?.textContent?.trim()).toBe("Medium-wealth shoppers ($$)");
      shoppers.value = "15";
      shoppers.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[{
        type: "commercial", commercialProfile: "ground-mall-mixed-v1",
        shoppers: undefined, shoppersW1: 0, shoppersW2: 15, shoppersW3: 0, goods: 9,
      }]]);
    } finally {
      wrapper.unmount();
    }
  });

  it("shows only capacity fields and directs staffing to building properties", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, position: [0, 0] },
        params: { type: "commercial", shoppers: 20, goods: 20 },
        labelKey: "studio.assetWorkbench.logicNode.leafCommercial",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      expect(dialog.querySelectorAll('input[type="number"]')).toHaveLength(4);
      expect(dialog.textContent).toContain("劳工在建筑属性中设置");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(dialog.textContent).toContain("Set staffing in Building Properties");
      expect(dialog.querySelector('[role="alert"]')).toBeNull();
    } finally {
      wrapper.unmount();
    }
  });

  it("adds another customer tier while keeping the saved tier and goods", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-retail-wealth", position: [0, 0] },
        params: { type: "commercial", commercialProfile: "ground-commercial-w2-v1", shoppers: 6 },
        labelKey: "studio.assetWorkbench.logicNode.leafCommercial",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const fields = dialog.querySelectorAll<HTMLInputElement>('input[type="number"]');
      expect([...fields].map((field) => field.value)).toEqual(["0", "6", "0", "18"]);
      fields[0].value = "25";
      fields[0].dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(dialog.textContent).toContain("共享接待容量 31");
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[
        { type: "commercial", commercialProfile: "ground-mall-mixed-v1", shoppers: undefined,
          shoppersW1: 25, shoppersW2: 6, shoppersW3: 0, goods: 18 },
      ]]);
    } finally {
      wrapper.unmount();
    }
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
      expect(document.querySelector('[role="switch"]')).toBeNull();

      attraction.value = "275";
      attraction.dispatchEvent(new Event("input", { bubbles: true }));
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
          },
        ],
      ]);
    } finally {
      wrapper.unmount();
    }
  });

  it("edits hotel rooms separately from tourism and checks the native bin limit", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, position: [0, 0] },
        params: { type: "hotel" },
        labelKey: "studio.assetWorkbench.logicNode.leafHotel",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const input = dialog.querySelector<HTMLInputElement>('input[placeholder="68"]')!;
      expect(input.closest("label")?.textContent?.trim()).toBe("客房数量");
      i18n.global.locale.value = "en-US";
      input.value = "1001";
      input.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(input.closest("label")?.textContent?.trim()).toBe("Guest rooms");
      expect(dialog.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(true);
      input.value = "84";
      input.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[{ type: "hotel", rooms: 84 }]]);
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
      const recipe = document.querySelector<HTMLButtonElement>('[role="dialog"] .f-select')!;
      expect(recipe.textContent?.trim()).toBe("Copan Building");
      recipe.click();
      await nextTick();
      [...document.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')]
        .find((option) => option.textContent?.trim() === "MegaTower § apartment")!.click();
      await nextTick();
      input.value = "550";
      input.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      input.closest("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[
        { type: "residence", residenceProfile: "megatower-residence-v1", population: 550 },
      ]]);
    } finally {
      wrapper.unmount();
    }
  });

  it("offers three residence tiers and derives shopper capacity from the worker allocation", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-ground-residence", position: [0, 0] },
        params: { type: "residence", residenceProfile: "ground-residence-w1-v1" },
        labelKey: "studio.assetWorkbench.logicNode.leafResidence",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      dialog.querySelector<HTMLButtonElement>(".f-select")!.click();
      await nextTick();
      const options = [...document.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')];
      expect(options.slice(0, 3).map((option) => option.textContent?.trim()))
        .toEqual(["低收入住宅 ($)", "中收入住宅 ($$)", "高收入住宅 ($$$)"]);
      options[2].click();
      i18n.global.locale.value = "en-US";
      await nextTick();
      const workers = dialog.querySelector<HTMLInputElement>('input[placeholder="7"]')!;
      expect(workers.closest("label")?.textContent?.trim()).toBe("Resident worker capacity");
      expect(dialog.querySelector<HTMLInputElement>("input[readonly]")?.value).toBe("3");
      workers.value = "11";
      workers.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(dialog.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(true);
      workers.value = "6";
      workers.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(dialog.querySelector<HTMLInputElement>("input[readonly]")?.value).toBe("4");
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[
        { type: "residence", residenceProfile: "ground-residence-w3-v1", residentWorkers: 6 },
      ]]);
    } finally {
      wrapper.unmount();
    }
  });

  it("translates office fields in the open sheet and rejects an invalid workforce", async () => {
    i18n.global.locale.value = "zh-CN";
    const wrapper = mount(BehaviorLeafSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: {
        node: { ...node, id: "leaf-office-1", position: [0, 0] },
        params: { type: "office" },
        labelKey: "studio.assetWorkbench.logicNode.leafOffice",
        trigger: null,
      },
    });
    try {
      await nextTick();
      const dialog = document.querySelector('[role="dialog"]')!;
      const inputs = Array.from(dialog.querySelectorAll<HTMLInputElement>('input[type="number"]'));
      expect(inputs.map((input) => input.value)).toEqual(["400", "200", "100", "1"]);
      expect(inputs[0].closest("label")?.textContent?.trim()).toBe("低收入岗位");
      i18n.global.locale.value = "en-US";
      await nextTick();
      expect(inputs[0].closest("label")?.textContent?.trim()).toBe("Low-wealth jobs");
      const minimum = inputs[3];
      minimum.value = "701";
      minimum.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      expect(dialog.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(true);
      minimum.value = "10";
      minimum.dispatchEvent(new Event("change", { bubbles: true }));
      await nextTick();
      dialog.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      expect(wrapper.emitted("apply")).toEqual([[{ type: "office", minWorkers: 10 }]]);
    } finally {
      wrapper.unmount();
    }
  });
});
